use std::{
    env::var,
    sync::{Arc, atomic::AtomicBool},
    thread::JoinHandle,
    time::Duration,
};

use self::proc::TelemetryProcess;
use crate::moonlight_codec::MoonlightClient;

mod disk;
mod manifest;
mod metrics;
mod network;
mod proc;
mod source;
mod temperature;

const DEFAULT_REFRESH_INTERVAL: Duration = Duration::from_secs(60);
const MIN_REFRESH_INTERVAL_SECS: u64 = 15;
const MAX_REFRESH_INTERVAL_SECS: u64 = 30 * 60;

pub fn start(client: MoonlightClient, shutdown_flag: Arc<AtomicBool>) -> Option<JoinHandle<()>> {
    let some_interval = parse_refresh_interval(var("FOSTROM_COLLECT_TELEMETRY").ok().as_deref());
    some_interval.map(|interval| TelemetryProcess::start(client, shutdown_flag, interval))
}

fn parse_refresh_interval(raw: Option<&str>) -> Option<Duration> {
    let Some(raw) = raw else {
        return Some(DEFAULT_REFRESH_INTERVAL);
    };

    let value = raw.trim();
    if value.is_empty() {
        return Some(DEFAULT_REFRESH_INTERVAL);
    }

    if value.eq_ignore_ascii_case("false")
        || value == "0"
        || value.eq_ignore_ascii_case("off")
        || value.eq_ignore_ascii_case("no")
        || value.eq_ignore_ascii_case("none")
    {
        return None;
    }

    if value.eq_ignore_ascii_case("true")
        || value == "1"
        || value.eq_ignore_ascii_case("on")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("all")
    {
        return Some(DEFAULT_REFRESH_INTERVAL);
    }

    match value.parse::<u64>() {
        Ok(seconds) => Some(Duration::from_secs(
            seconds.clamp(MIN_REFRESH_INTERVAL_SECS, MAX_REFRESH_INTERVAL_SECS),
        )),
        Err(_) => Some(DEFAULT_REFRESH_INTERVAL),
    }
}

pub fn print_manifest() {
    let proc = TelemetryProcess::new();
    println!(
        "{}",
        serde_json::to_string_pretty(&proc.manifest()).unwrap()
    );
}

pub fn print_metrics() {
    let mut proc = TelemetryProcess::new();
    println!("{}", serde_json::to_string_pretty(&proc.metrics()).unwrap());
}
