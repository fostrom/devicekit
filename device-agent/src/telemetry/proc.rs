use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::channel,
    },
    thread::{JoinHandle, sleep, spawn},
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::Value;

use crate::moonlight_codec::{ClientCmd, MoonlightClient, PulseType, ReturnChanResult};

use super::{
    manifest::{Manifest, collect_manifest},
    metrics::{Metrics, collect_metrics},
    source::TelemetrySource,
};

const MANIFEST_PULSE_NAME: &str = "manifest_v0";
const TELEMETRY_PULSE_NAME: &str = "telemetry_v0";
const SHUTDOWN_TICK: Duration = Duration::from_millis(250);
const PULSE_ACK_TIMEOUT: Duration = Duration::from_secs(10);

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

    pub fn start(
        client: MoonlightClient,
        shutdown_flag: Arc<AtomicBool>,
        refresh_interval: Duration,
    ) -> JoinHandle<()> {
        spawn(move || {
            let mut proc = Self::new();
            proc.thread_loop(client, shutdown_flag, refresh_interval);
        })
    }

    fn thread_loop(
        &mut self,
        client: MoonlightClient,
        shutdown_flag: Arc<AtomicBool>,
        refresh_interval: Duration,
    ) {
        let mut was_connected = false;
        let mut next_metrics_at: Option<Instant> = None;

        while !shutdown_flag.load(Ordering::SeqCst) {
            let connected = client.connected();
            let now = Instant::now();

            if connected {
                if !was_connected {
                    self.emit_manifest(&client);
                    self.emit_metrics(&client);
                    next_metrics_at = now.checked_add(refresh_interval);
                } else if next_metrics_at.is_some_and(|deadline| now >= deadline) {
                    self.emit_metrics(&client);
                    next_metrics_at = Instant::now().checked_add(refresh_interval);
                }
            } else {
                next_metrics_at = None;
            }

            was_connected = connected;
            sleep(SHUTDOWN_TICK);
        }
    }

    fn emit_manifest(&self, client: &MoonlightClient) {
        send_system_pulse(client, MANIFEST_PULSE_NAME, serialize(&self.manifest()));
    }

    fn emit_metrics(&mut self, client: &MoonlightClient) {
        send_system_pulse(client, TELEMETRY_PULSE_NAME, serialize(&self.metrics()));
    }
}

fn serialize<T: Serialize>(value: &T) -> Value {
    serde_json::to_value(value).unwrap()
}

fn send_system_pulse(client: &MoonlightClient, name: &str, payload: Value) {
    let (tx, rx) = channel();
    let pulse = ClientCmd::SendPulse(PulseType::System, name.to_string(), Some(payload), tx);
    client.send_cmd(pulse);

    match rx.recv_timeout(PULSE_ACK_TIMEOUT) {
        Ok(ReturnChanResult::Ok) => (),
        Ok(ReturnChanResult::Err(e)) => {
            eprintln!("telemetry: failed to send {name} pulse: {e}")
        }
        Ok(ReturnChanResult::Timeout) => {
            eprintln!("telemetry: timeout sending {name} pulse")
        }
        Ok(_) => eprintln!("telemetry: unexpected response sending {name} pulse"),
        Err(_) => eprintln!("telemetry: ack channel closed sending {name} pulse"),
    }
}
