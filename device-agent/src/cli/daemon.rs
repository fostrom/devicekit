use super::{STDERR_LOG, STDOUT_LOG, TMP_DIR};
use crate::cli::{AgentConfig, status::fetch_status};
use anyhow::{Result, anyhow};
use nix::{
    sys::signal::{Signal, kill},
    unistd::Pid,
};
use std::{
    env::current_exe,
    fs::{self, File},
    process::{Child, Command, Stdio},
    thread::sleep,
    time::{Duration, Instant},
};

fn open_log_file(path: impl ToString) -> Result<File> {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path.to_string())
        .map_err(|_| anyhow!("Failed to open file: {}", path.to_string()))
}

pub fn start_daemon(config: AgentConfig) -> Result<()> {
    // Resolve current executable path
    let exe = current_exe()?;
    let stdout_file = open_log_file(STDOUT_LOG)?;
    let stderr_file = open_log_file(STDERR_LOG)?;

    // Build child command for daemon mode
    let mut cmd = Command::new(exe);
    cmd.arg("daemon");
    if config.enable_tcp_socket {
        cmd.arg("--tcp");
    }

    let child = cmd
        .current_dir(TMP_DIR)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|_| anyhow!("Failed to start daemon"))?;

    run_readiness_check(child)?;
    println!("started: The agent daemon is running.");
    Ok(())
}

/// Parent: wait for readiness (UNIX socket accepts and GET / returns 200 OK)
fn run_readiness_check(child: Child) -> Result<()> {
    let start = Instant::now();
    let mut ready = false;

    while start.elapsed() < Duration::from_secs(10) {
        if fetch_status().is_ok() {
            ready = true;
            break;
        }

        sleep(Duration::from_millis(100));
    }

    // Operation Timed Out
    if !ready {
        // Perform Cleanup: Kill the child process
        let pid = Pid::from_raw(child.id() as i32);
        let _ = kill(pid, Some(Signal::SIGKILL));
        return Err(anyhow!("Timed out waiting for the agent to become ready."));
    }

    Ok(())
}
