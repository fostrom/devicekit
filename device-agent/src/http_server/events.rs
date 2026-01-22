// ----------------------------------
// --- SERVER-SENT EVENTS HANDLER ---
// ----------------------------------

use crate::http_server::{SocketContext, socket::Socket};
use std::{
    sync::atomic::Ordering,
    time::{Duration, Instant, SystemTime},
};

/// Server Sent Events Handler
pub fn handle_event_stream(mut socket: Socket, ctx: &SocketContext) {
    // Increase the write timeout
    let write_timeout = socket.set_write_timeout(Some(Duration::from_secs(60)));
    if write_timeout.is_err() {
        return;
    }

    let (token, broadcast_rx) = ctx.notify.subscribe();

    let mut last_keep_alive = Instant::now();

    let first_event = if ctx.client.connected() {
        "connected"
    } else {
        "disconnected"
    };

    if !socket.send(format!("event: {}\n\n", first_event).as_bytes()) {
        ctx.notify.unsubscribe(token);
        return;
    };

    if ctx.client.connected() {
        if !socket.send(notification("new_mail".to_string(), "".to_string()).as_bytes()) {
            ctx.notify.unsubscribe(token);
            return;
        }
        last_keep_alive = Instant::now();
    }

    loop {
        if ctx.shutdown_flag.load(Ordering::Relaxed) {
            break;
        }

        match broadcast_rx.recv_timeout(Duration::from_millis(500)) {
            Ok((event, data)) => {
                if !socket.send(notification(event, data).as_bytes()) {
                    break;
                }
                last_keep_alive = Instant::now();
            }
            Err(_) => {
                if last_keep_alive.elapsed() >= Duration::from_secs(15) {
                    if !socket.send(keep_alive().as_bytes()) {
                        break;
                    }
                    last_keep_alive = Instant::now();
                }
            }
        }
    }

    ctx.notify.unsubscribe(token);
}

fn keep_alive() -> String {
    let current_time_ms = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
        Err(_) => 0,
        Ok(n) => n.as_millis() as u64,
    };

    format!("event: keep_alive\ndata: {current_time_ms}\n\n")
}

fn notification(event: String, data: String) -> String {
    if event.is_empty() {
        return "".to_string();
    }

    if data.is_empty() {
        return format!("event: {event}\n\n");
    }

    let lines = data
        .split('\n')
        .filter_map(|line| {
            if line.is_empty() {
                None
            } else {
                Some(format!("data: {line}"))
            }
        })
        .collect::<Vec<String>>()
        .join("\n");

    if lines.is_empty() {
        return format!("event: {event}\n\n");
    }

    format!("event: {event}\n{lines}\n\n")
}
