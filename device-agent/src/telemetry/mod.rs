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
