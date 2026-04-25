use std::{
    thread::{sleep, spawn},
    time::Duration,
};

use super::{
    manifest::{Manifest, collect_manifest},
    metrics::{Metrics, collect_metrics, diff_metrics},
    source::TelemetrySource,
};

pub struct TelemetryProcess {
    source: TelemetrySource,
    last_metrics: Option<Metrics>,
}

impl TelemetryProcess {
    pub fn new() -> Self {
        TelemetryProcess {
            source: TelemetrySource::new(),
            last_metrics: None,
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
        let mut new_metrics = self.metrics();

        let metrics = if let Some(last_metrics) = self.last_metrics.clone() {
            diff_metrics(&last_metrics, &mut new_metrics)
        } else {
            new_metrics
        };

        self.last_metrics = Some(metrics.clone());

        // emit metrics here
        _ = metrics;
    }

    fn thread_loop(&mut self) {
        self.emit_manifest();

        loop {
            self.emit_metrics();
            sleep(Duration::from_secs(60));
        }
    }
}
