// -----------
// --- CLI ---
// -----------

mod daemon;
mod parser;
mod start;
mod status;
mod stop;
mod test_conn;

use crate::moonlight_codec::{ConnectMode, Creds};
use start::{start_agent, start_daemon_child};
use status::agent_status;
use std::process::exit;
use stop::stop_agent;

pub static TMP_DIR: &str = "/tmp/fostrom";
pub static HASH_FILE: &str = "/tmp/fostrom/config.hash";
pub static PID_FILE: &str = "/tmp/fostrom/agent.pid";
pub static SOCK_FILE: &str = "/tmp/fostrom/agent.sock";
pub static STDOUT_LOG: &str = "/tmp/fostrom/stdout.log";
pub static STDERR_LOG: &str = "/tmp/fostrom/stderr.log";

#[derive(Debug, Clone)]
pub struct AgentConfig {
    pub creds: Creds,
    pub enable_unix_socket: bool,
    pub enable_tcp_socket: bool,
    pub connect_mode: ConnectMode,
    pub start_daemon: bool,
}

#[derive(Debug, Clone)]
pub enum ParsedAction {
    Start(AgentConfig),
    Daemon(AgentConfig),
    Stop,
    Status,
    TestConn,
}

pub fn exec() {
    if let Some(action) = parser::parse() {
        match action {
            ParsedAction::Start(config) => start_agent(config),
            ParsedAction::Daemon(config) => start_daemon_child(config),
            ParsedAction::Stop => stop_agent(),
            ParsedAction::Status => agent_status(),
            ParsedAction::TestConn => exit(test_conn::run()),
        }
    }
}
