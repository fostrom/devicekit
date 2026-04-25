use crate::telemetry::proc::TelemetryProcess;

mod disk;
mod manifest;
mod metrics;
mod network;
mod proc;
mod source;
mod temperature;

pub fn new() -> TelemetryProcess {
    TelemetryProcess::new()
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
