use super::{
    disk::{Disk, fetch_disks},
    network::{Network, fetch_networks},
    source::TelemetrySource,
    temperature::fetch_temperature,
};
use serde::Serialize;
use sysinfo::System;

#[derive(Debug, Clone, Serialize)]
pub struct Metrics {
    pub uptime: u64,
    pub load_1m: f64,
    pub load_5m: f64,
    pub load_15m: f64,
    pub cpu: f32,
    pub memory_available: u64,
    pub memory_total: u64,
    pub swap_available: u64,
    pub swap_total: u64,
    pub temperature_c: Option<f32>,
    pub disks: Vec<Disk>,
    pub networks: Vec<Network>,
}

pub fn collect_metrics(source: &mut TelemetrySource) -> Metrics {
    source.refresh();

    let (l1, l5, l15) = load_averages();

    Metrics {
        uptime: System::uptime(),
        load_1m: l1,
        load_5m: l5,
        load_15m: l15,
        cpu: sanitize_f32(source.system.global_cpu_usage()),
        memory_available: source.system.available_memory(),
        memory_total: source.system.total_memory(),
        swap_available: source.system.free_swap(),
        swap_total: source.system.total_swap(),
        temperature_c: fetch_temperature(&source.components),
        disks: fetch_disks(&source.disks),
        networks: fetch_networks(&source.networks),
    }
}

fn load_averages() -> (f64, f64, f64) {
    let load = System::load_average();
    (
        sanitize_f64(load.one),
        sanitize_f64(load.five),
        sanitize_f64(load.fifteen),
    )
}

fn sanitize_f64(n: f64) -> f64 {
    if n.is_finite() { n } else { 0.0 }
}

fn sanitize_f32(n: f32) -> f32 {
    if n.is_finite() { n } else { 0.0 }
}
