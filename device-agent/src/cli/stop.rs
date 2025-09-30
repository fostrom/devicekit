// ------------------------
// --- CLI STOP HANDLER ---
// ------------------------

use super::{HASH_FILE, PID_FILE, SOCK_FILE};
use nix::sys::signal::{Signal, kill};
use nix::unistd::Pid;
use std::fs::{read_to_string, remove_file};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::thread::sleep;
use std::time::{Duration, Instant};

pub enum StopMode {
    NotRunning,
    Stopped,
    ForceKilled,
    Failed,
}

pub fn stop_agent() {
    match terminate_agent() {
        StopMode::NotRunning => println!("stopped: The agent was not running."),
        StopMode::Stopped => println!("stopped: The agent has been stopped."),
        StopMode::ForceKilled => println!("stopped: The agent has been stopped by forced kill."),
        StopMode::Failed => eprintln!(
            "timeout: The agent did not stop within the expected time and a PID file wasn't found to force kill the agent or the force kill failed."
        ),
    }
}

pub fn terminate_agent() -> StopMode {
    if Path::new(SOCK_FILE).exists() {
        match UnixStream::connect(SOCK_FILE) {
            Ok(mut stream) => {
                let _ = stream.write_all(b"DELETE /stop-agent HTTP/1.1\r\n\r\n");
                let mut buffer = String::new();
                let _ = stream.read_to_string(&mut buffer);

                if buffer.contains("200 OK") {
                    wait_for_cleanup()
                } else {
                    force_kill_agent()
                }
            }
            Err(_) => force_kill_agent(),
        }
    } else {
        StopMode::NotRunning
    }
}

fn wait_for_cleanup() -> StopMode {
    let wait_start = Instant::now();
    while Path::new(SOCK_FILE).exists() {
        sleep(Duration::from_millis(25));
        if wait_start.elapsed() > Duration::from_secs(5) {
            return force_kill_agent();
        }
    }
    StopMode::Stopped
}

fn force_kill_agent() -> StopMode {
    if Path::new(PID_FILE).exists()
        && let Ok(contents) = read_to_string(PID_FILE)
        && let trimmed = contents.trim()
        && let Ok(raw_pid) = trimmed.parse::<i32>()
        && let pid = Pid::from_raw(raw_pid)
        && let Ok(_) = kill(pid, Some(Signal::SIGKILL))
    {
        let _ = remove_file(SOCK_FILE);
        let _ = remove_file(PID_FILE);
        let _ = remove_file(HASH_FILE);
        StopMode::ForceKilled
    } else {
        StopMode::Failed
    }
}
