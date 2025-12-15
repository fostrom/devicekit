// ------------------
// --- CLI PARSER ---
// ------------------

use super::{AgentConfig, ParsedAction};
use crate::moonlight_codec::{ConnectMode, Creds};
use anyhow::{Error, Result, anyhow};
use std::env::{args, var};

const DEVICE_AGENT_VSN: &str = concat!("v", env!("CARGO_PKG_VERSION"));

const HELP_TEXT: &str = concat!(
    "Fostrom Device Agent ",
    "v",
    env!("CARGO_PKG_VERSION"),
    r#"

The Fostrom Device Agent is used by our Device SDKs.
Visit the Fostrom Docs [https://fostrom.io/docs/] for more information.

USAGE:
    start               Start in daemon mode
      --tcp               Enable TCP socket (default: false)
    run                 Start in blocking mode
      --tcp               Enable TCP socket (default: false)
    status              Get the agent's status
    stop                Stop the device agent
    version             Print version
    help                Print this help text"#
);

pub fn parse() -> Option<ParsedAction> {
    let args = args()
        .collect::<Vec<String>>()
        .iter()
        .skip(1)
        .map(|s| s.to_lowercase())
        .collect::<Vec<String>>();

    if args.is_empty() {
        return help();
    }

    if args.len() == 1 && (args[0] == "help") {
        return help();
    }

    if args.len() == 1 && (args[0] == "version") {
        println!("{DEVICE_AGENT_VSN}");
        return None;
    }

    if args.len() == 1 && args[0] == "stop" {
        return Some(ParsedAction::Stop);
    }

    if args.len() == 1 && args[0] == "status" {
        return Some(ParsedAction::Status);
    }

    if !args.is_empty() && (args[0] == "run" || args[0] == "start" || args[0] == "daemon") {
        let start_daemon = args[0] == "start" || args[0] == "daemon";
        let start_tcp = args.contains(&"--tcp".to_string());
        match get_agent_config(start_daemon, start_tcp) {
            Ok(config) => {
                if args[0] == "daemon" {
                    return Some(ParsedAction::Daemon(config));
                } else {
                    return Some(ParsedAction::Start(config));
                }
            }
            Err(e) => {
                eprintln!("{e}");
                return None;
            }
        }
    }

    eprintln!("Unknown Command: {}", args.join(" "));
    eprintln!();
    eprintln!("{HELP_TEXT}");
    None
}

fn help() -> Option<ParsedAction> {
    println!("{HELP_TEXT}");
    None
}

pub fn get_agent_config(start_daemon: bool, start_tcp: bool) -> Result<AgentConfig> {
    let (fleet_id, device_id, device_secret, connect_mode) = read_env()?;
    let prod = matches!(connect_mode, ConnectMode::Prod);
    let creds = Creds::new(fleet_id, device_id, device_secret, prod)?;

    Ok(AgentConfig {
        creds,
        enable_unix_socket: true,
        enable_tcp_socket: start_tcp,
        connect_mode,
        start_daemon,
    })
}

fn env_error() -> Error {
    anyhow!(
        "To start the Fostrom Device Agent, you need to pass the following environment variables:\n\t$FOSTROM_FLEET_ID\t\tThe 8-character Fleet ID\n\t$FOSTROM_DEVICE_ID\t\tThe 10-character Device ID\n\t$FOSTROM_DEVICE_SECRET\t\tThe 36-character Device Secret, begins with `FOS-`\n\nYou can find these in the Fostrom Console under your device's settings."
    )
}

fn read_env() -> Result<(String, String, String, ConnectMode)> {
    let fleet_id = var("FOSTROM_FLEET_ID").map_err(|_| env_error())?;
    let device_id = var("FOSTROM_DEVICE_ID").map_err(|_| env_error())?;
    let device_secret = var("FOSTROM_DEVICE_SECRET").map_err(|_| env_error())?;

    // Check if FOSTROM_LOCAL_MODE is set
    let local = var("FOSTROM_LOCAL_MODE").unwrap_or("false".to_string());
    let local = local == "true" || local == "1";

    let connect_mode = if local {
        ConnectMode::Local(8484)
    } else {
        ConnectMode::Prod
    };

    Ok((fleet_id, device_id, device_secret, connect_mode))
}
