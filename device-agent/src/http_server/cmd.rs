// --------------------------
// --- MOONLIGHT COMMANDS ---
// --------------------------

use super::response::{FailureResp as FR, Resp};
use crate::moonlight_codec::{
    ClientCmd, ClientLogic, MailAckType, MoonlightClient, PulseType, ReturnChanResult as R,
};
use serde_json::{Value, json};
use std::{
    sync::mpsc::{Receiver, RecvTimeoutError, channel},
    thread::sleep,
    time::{Duration, Instant},
};

fn wait_for_connected(client: &MoonlightClient) -> bool {
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if client.connected() {
            return true;
        }
        sleep(Duration::from_millis(25));
    }
    client.connected()
}

fn make_request(
    client: &MoonlightClient,
    cmd: ClientCmd,
    result_rx: Receiver<R>,
) -> Result<R, Resp> {
    // wait until a connection is established before making a request
    if !wait_for_connected(client) {
        return Err(FR::forbidden(
            "not_connected: Device Agent is still connecting to Fostrom",
        ));
    }

    client.send_cmd(cmd);

    match result_rx.recv_timeout(Duration::from_secs(10)) {
        Err(RecvTimeoutError::Timeout) => Err(FR::timeout()),
        Err(_) => Err(FR::internal_server_error("Failed to receive response")),
        Ok(R::Timeout) => Err(FR::timeout()),
        Ok(R::Err(msg)) => Err(FR::forbidden(msg)),
        Ok(r) => Ok(r),
    }
}

pub fn mail_op(client: &MoonlightClient, ack_type: MailAckType, mail_id: u128) -> Resp {
    let (result_tx, result_rx) = channel();

    match make_request(
        client,
        ClientCmd::MailOp(ack_type, mail_id, result_tx),
        result_rx,
    ) {
        Err(resp) => resp,
        Ok(R::MailAckSuccessful(mail_available)) => {
            let mut r = Resp::ok(json!({"ok": true, "mail_available": mail_available}));
            r.add_header("X-Mail-Available", mail_available);
            r
        }
        Ok(_) => FR::internal_server_error("Unexpected Response"),
    }
}

pub fn mailbox_next(client: &MoonlightClient, header_only: bool) -> Resp {
    let (result_tx, result_rx) = channel();
    let cmd = ClientCmd::MailboxNext(header_only, result_tx);
    match make_request(client, cmd, result_rx) {
        Err(resp) => resp,

        Ok(R::Mail(None)) => {
            let mut r = Resp::ok("");
            r.add_header("X-Mailbox-Size", 0)
                .add_header("X-Mailbox-Empty", true);
            r
        }

        Ok(R::Mail(Some(mail))) => {
            let mut r = Resp::ok("");

            r.add_header("X-Mailbox-Size", mail.mailbox_size)
                .add_header("X-Mailbox-Empty", false)
                .add_header("X-Mail-ID", ClientLogic::uuidv7_str(mail.pulse_id))
                .add_header("X-Mail-Name", mail.name)
                .add_header("X-Mail-Has-Payload", mail.payload.is_some());

            if header_only || mail.payload.is_none() {
                r
            } else {
                r.set_body(mail.payload.unwrap());
                r
            }
        }

        Ok(_) => FR::internal_server_error("Unexpected Response"),
    }
}

pub fn send_pulse(
    client: &MoonlightClient,
    pulse_type: PulseType,
    name: String,
    payload: Option<Value>,
) -> Resp {
    let (result_tx, result_rx) = channel();
    let cmd = ClientCmd::SendPulse(pulse_type, name, payload, result_tx);

    match make_request(client, cmd, result_rx) {
        Err(resp) => resp,
        Ok(R::Ok) => Resp::ok(json!({"ok": true})),
        Ok(_) => FR::internal_server_error("Unexpected Response"),
    }
}
