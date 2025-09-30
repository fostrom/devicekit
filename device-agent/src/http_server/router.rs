// -------------------
// --- HTTP ROUTER ---
// -------------------

use super::{
    cmd::{mail_op, mailbox_next, send_pulse},
    request::{
        Method::{DELETE, GET, HEAD, POST, PUT},
        Req, parse_request,
    },
    response::{FailureResp as FR, Resp},
};
use crate::{
    http_server::{SocketContext, events::handle_event_stream, socket::Socket},
    moonlight_codec::{
        ClientLogic,
        MailAckType::{self, Ack, Reject, Requeue},
        PulseType::{self, Data, Msg, System},
    },
};
use serde_json::json;
use std::io::BufReader;
use std::sync::atomic::Ordering;

/// Pass a TCP/UNIX Stream
/// and this function will handle the request.
/// It'll parse the request, route it, and
/// write the final response back to the stream.
pub fn handle_request(mut socket: Socket, ctx: &SocketContext) {
    let mut buf_reader = BufReader::new(&mut socket);

    let mut resp = match parse_request(&mut buf_reader, &ctx.client) {
        Ok(req) => route(ctx, req),
        Err(resp) => resp,
    };

    if !socket.send(resp.compile(&ctx.client).as_bytes()) {
        return;
    }

    if resp.is_event_stream {
        handle_event_stream(socket, ctx);
    }
}

fn route(ctx: &SocketContext, req: Req) -> Resp {
    match (req.method.clone(), req.path.as_str()) {
        (GET, "/") => Resp::ok(ctx.client.status()),
        (HEAD, "/") => Resp::ok(""),
        (DELETE, "/stop-agent") => exec_stop_agent(ctx),
        (GET, "/events") => Resp::event_stream(),
        (GET, "/mailbox/next") => mailbox_next(&ctx.client, false),
        (HEAD, "/mailbox/next") => mailbox_next(&ctx.client, true),
        (PUT, path) if path.starts_with("/mailbox/ack/") => exec_mail_op(ctx, Ack, req),
        (PUT, path) if path.starts_with("/mailbox/reject/") => exec_mail_op(ctx, Reject, req),
        (PUT, path) if path.starts_with("/mailbox/requeue/") => exec_mail_op(ctx, Requeue, req),
        (POST, path) if path.starts_with("/pulse/datapoint/") => exec_send_pulse(ctx, Data, req),
        (POST, path) if path.starts_with("/pulse/msg/") => exec_send_pulse(ctx, Msg, req),
        (POST, path) if path.starts_with("/pulse/system/") => exec_send_pulse(ctx, System, req),
        _ => FR::not_found("Not Found"),
    }
}

fn exec_stop_agent(ctx: &SocketContext) -> Resp {
    ctx.shutdown_flag.store(true, Ordering::SeqCst);
    ctx.client.stop();
    Resp::ok(json!({"ok": true}))
}

fn exec_mail_op(ctx: &SocketContext, ack_type: MailAckType, req: Req) -> Resp {
    if let Some((_, mail_id_str)) = req.path.trim_start_matches("/mailbox/").split_once("/") {
        match ClientLogic::to_pulse_id(mail_id_str) {
            Ok(mail_id) => mail_op(&ctx.client, ack_type, mail_id),
            Err(e) => FR::bad_request(e),
        }
    } else {
        FR::bad_request("Mail ID missing in path after /mailbox/<action>/")
    }
}

fn exec_send_pulse(ctx: &SocketContext, pulse_type: PulseType, req: Req) -> Resp {
    if let Some((_, pulse_name)) = req.path.trim_start_matches("/pulse/").split_once("/") {
        match is_valid_pulse_name(pulse_name.trim()) {
            true => send_pulse(&ctx.client, pulse_type, pulse_name.to_string(), req.body),
            false => FR::bad_request("Invalid Pulse Name"),
        }
    } else {
        FR::bad_request("Pulse name missing in path after /pulse/<type>/")
    }
}

fn is_valid_pulse_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 255
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
