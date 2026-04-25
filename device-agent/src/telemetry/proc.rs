use std::{
    thread::{sleep, spawn},
    time::Duration,
};

use super::{
    manifest::{Manifest, collect_manifest},
    metrics::{Metrics, collect_metrics},
    source::TelemetrySource,
};

pub struct TelemetryProcess {
    source: TelemetrySource,
}

impl TelemetryProcess {
    pub fn new() -> Self {
        TelemetryProcess {
            source: TelemetrySource::new(),
        }
    }

    pub fn manifest(&self) -> Manifest {
        collect_manifest(&self.source)
    }

    pub fn metrics(&mut self) -> Metrics {
        collect_metrics(&mut self.source)
    }

    fn emit_manifest(&self) {
        _ = self.manifest();
    }

    fn emit_metrics(&mut self) {
        _ = self.metrics();
    }

    fn start_thread(refresh_interval: Duration) {}

    fn thread_loop(&mut self, refresh_interval: Duration) {
        self.emit_manifest();

        loop {
            self.emit_metrics();
            sleep(refresh_interval);
        }
    }
}
