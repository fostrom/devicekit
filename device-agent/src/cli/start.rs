// -------------------------
// --- CLI START HANDLER ---
// -------------------------

use super::{HASH_FILE, PID_FILE, SOCK_FILE, TMP_DIR};
use crate::{
    cli::{AgentConfig, daemon::start_daemon, stop::terminate_agent},
    http_server::{self, SocketContext},
    moonlight_codec::{Creds, MoonlightClient},
    notifycast::NotifyCast,
};
use anyhow::Result;
use std::{
    fs::{create_dir_all, read_to_string, remove_file, set_permissions, write},
    os::unix::{fs::PermissionsExt, net::UnixStream},
    path::Path,
    process,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::channel,
    },
    thread::{JoinHandle, spawn},
};

struct PidFileGuard;

impl PidFileGuard {
    fn create() -> Result<Self> {
        let pid = process::id();
        write(PID_FILE, format!("{pid}\n"))?;
        Ok(Self)
    }
}

impl Drop for PidFileGuard {
    fn drop(&mut self) {
        let _ = remove_file(PID_FILE);
    }
}

struct HashFileGuard;

impl HashFileGuard {
    fn create(creds: &Creds) -> Result<Self> {
        let hash = creds.hash();
        write(HASH_FILE, format!("{hash}\n"))?;
        Ok(Self)
    }
}

impl Drop for HashFileGuard {
    fn drop(&mut self) {
        let _ = remove_file(HASH_FILE);
    }
}

enum Preflight {
    AlreadyStarted,
    StartFresh,
}

/// Called when cmd is `daemon` (which is started below).
/// This function detaches the process and starts the agent in the background.
pub fn start_daemon_child(config: AgentConfig) {
    use nix::unistd::setsid;
    // Detach into a new session; ignore any error
    let _ = setsid();
    // Run the agent in the child process (blocking)
    let _ = start_proc(config);
}

/// Called when cmd is `start` or `run`.
/// This function starts the agent in the foreground
/// or spawns the daemon process, after conducting
/// preflight checks.
pub fn start_agent(config: AgentConfig) {
    create_dir_all(TMP_DIR).expect("failed: Failed to create /tmp/fostrom directory");

    set_permissions(TMP_DIR, PermissionsExt::from_mode(0o700))
        .expect("failed: Failed to set permissions on /tmp/fostrom directory");

    match preflight(&config) {
        Preflight::AlreadyStarted => {
            println!("already_started: The agent is already running with the same configuration.");
        }
        Preflight::StartFresh => if config.start_daemon {
            start_daemon(config)
        } else {
            start_proc(config)
        }
        .unwrap_or_else(|e| eprintln!("failed: {e}")),
    }
}

/// If the Device Agent is already running, compare the credhash
/// to check whether to restart or not.
fn preflight(config: &AgentConfig) -> Preflight {
    if Path::new(SOCK_FILE).exists()
        && Path::new(HASH_FILE).exists()
        && let Ok(_) = UnixStream::connect(SOCK_FILE)
        && let new_hash = config.creds.hash()
        && let Some(prev_hash) = read_to_string(HASH_FILE).ok().map(|s| s.trim().to_string())
        && prev_hash == new_hash
    {
        Preflight::AlreadyStarted
    } else {
        terminate_agent();
        Preflight::StartFresh
    }
}

fn start_proc(config: AgentConfig) -> Result<()> {
    // Create the PID file, and ensure it's deleted on exit.
    // Automatic cleanup is handled by the PidFileGuard's Drop impl.
    let _pid_guard = PidFileGuard::create()?;

    // Create the Hash file, and ensure it's deleted on exit.
    // Automatic cleanup is handled by the HashFileGuard's Drop impl.
    let _hash_guard = HashFileGuard::create(&config.creds)?;

    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let s = shutdown_flag.clone();

    // Setup the notification channel and its broadcast system
    let (notify_chan_tx, notify_chan_rx) = channel();
    let notify = NotifyCast::new();
    let notify_handle = notify.start_listener(notify_chan_rx);

    let mut client = MoonlightClient::new(
        config.creds.fleet_id,
        config.creds.device_id,
        config.creds.device_secret,
        config.connect_mode,
    );

    let client_clone = client.clone();
    ctrlc::set_handler(move || {
        s.store(true, Ordering::SeqCst);
        client_clone.stop();
    })?;

    let socket_context = SocketContext {
        notify,
        client: client.clone(),
        shutdown_flag: shutdown_flag.clone(),
    };

    let mut unix_handle: Option<JoinHandle<()>> = None;
    let mut tcp_handle: Option<JoinHandle<()>> = None;

    // Start the UNIX Server
    if config.enable_unix_socket {
        let ctx = socket_context.clone();
        unix_handle = Some(spawn(move || {
            let _ = http_server::start_unix_server(&ctx);
        }));
    }

    // Start the TCP Server
    if config.enable_tcp_socket {
        let ctx = socket_context.clone();
        tcp_handle = Some(spawn(move || {
            let _ = http_server::start_tcp_server(&ctx);
        }));
    }

    client.start(notify_chan_tx)?;

    // Ensure shutdown flag is set so accept loops exit promptly
    shutdown_flag.store(true, Ordering::SeqCst);

    // Close Threads
    if let Some(h) = unix_handle {
        let _ = h.join();
    }
    if let Some(h) = tcp_handle {
        let _ = h.join();
    }
    let _ = notify_handle.join();

    Ok(())
}
