// ---------------------
// --- HTTP RESPONSE ---
// ---------------------

use crate::moonlight_codec::MoonlightClient;
use httpdate::fmt_http_date;
use serde_json::json;
use std::{collections::HashMap, time::SystemTime};

const DEVICE_AGENT_VSN: &str = env!("CARGO_PKG_VERSION");
const SERVER_NAME: &str = concat!("Fostrom-Device-Agent/v", env!("CARGO_PKG_VERSION"));

pub struct Resp {
    status_code: StatusCode,
    headers: HashMap<String, String>,
    body: String,
    pub is_event_stream: bool,
}

impl Resp {
    pub fn ok(body: impl ToString) -> Self {
        Resp::new(StatusCode::Ok, body)
    }

    pub fn new(status_code: StatusCode, body: impl ToString) -> Self {
        let mut resp = Resp {
            status_code,
            headers: HashMap::with_capacity(24),
            body: body.to_string(),
            is_event_stream: false,
        };

        resp.push_default_headers();

        resp
    }

    pub fn event_stream() -> Self {
        let mut resp = Self::ok("");

        resp.add_header("Content-Type", "text/event-stream; charset=utf-8")
            .add_header("Connection", "keep-alive")
            .add_header("X-Accel-Buffering", "no");

        resp.is_event_stream = true;

        resp
    }

    pub fn add_header(&mut self, key: impl ToString, value: impl ToString) -> &mut Self {
        self.headers.insert(key.to_string(), value.to_string());
        self
    }

    pub fn set_body(&mut self, body: impl ToString) -> &mut Self {
        self.body = body.to_string();
        self
    }

    fn push_default_headers(&mut self) {
        self.add_header("Cache-Control", "no-cache")
            .add_header("Connection", "close")
            .add_header("Content-Type", "application/json; charset=utf-8")
            .add_header("X-Powered-By", "Fostrom")
            .add_header("X-API-Version", 1)
            .add_header("X-Protocol", "Moonlight")
            .add_header("X-Protocol-Version", 1)
            .add_header("Server", SERVER_NAME)
            .add_header("X-Agent-Version", DEVICE_AGENT_VSN);
    }

    pub fn compile(&mut self, client: &MoonlightClient) -> String {
        let body_len = self.body.len();

        self.add_header("X-Connected", client.connected())
            .add_header("X-Device-ID", client.device_id.clone())
            .add_header("X-Fleet-ID", client.fleet_id.clone())
            .add_header("Content-Length", body_len)
            .add_header("Date", fmt_http_date(SystemTime::now()));

        if self.is_event_stream {
            // For event streams, we do not want to set a Content-Length header
            self.headers.remove("Content-Length");
        }

        let mut resp = String::with_capacity(1024 + body_len);

        // Example: `HTTP/1.1 200 OK\r\n`
        resp.push_str("HTTP/1.1 ");
        resp.push_str(self.status_code.to_http());
        resp.push_str("\r\n");

        // Example: `Content-Type: application/json\r\n`
        for (key, value) in &self.headers {
            // Replace any CR or LF characters in header values to prevent header injection
            let value = value.replace(['\r', '\n'], " ");
            resp.push_str(&format!("{key}: {value}\r\n"));
        }

        // Header and Body separator
        resp.push_str("\r\n");

        // Push the Body
        resp.push_str(&self.body);

        resp
    }
}

// -------------------------
// --- FAILURE RESPONSES ---
// -------------------------

/// Standard Failure Responses
pub struct FailureResp {}

impl FailureResp {
    pub fn make(status_code: StatusCode, error_msg: impl ToString) -> Resp {
        Resp::new(status_code, json!({"error": error_msg.to_string()}))
    }

    pub fn bad_request(error_msg: impl ToString) -> Resp {
        Self::make(StatusCode::BadRequest, error_msg)
    }

    pub fn unauthorized(error_msg: impl ToString) -> Resp {
        Self::make(StatusCode::Unauthorized, error_msg)
    }

    pub fn forbidden(error_msg: impl ToString) -> Resp {
        Self::make(StatusCode::Forbidden, error_msg)
    }

    pub fn not_found(error_msg: impl ToString) -> Resp {
        Self::make(StatusCode::NotFound, error_msg)
    }

    pub fn timeout() -> Resp {
        Self::make(StatusCode::Timeout, "Operation timed out")
    }

    pub fn internal_server_error(error_msg: impl ToString) -> Resp {
        Self::make(StatusCode::InternalServerError, error_msg)
    }

    pub fn version_not_supported() -> Resp {
        Self::make(
            StatusCode::VersionNotSupported,
            "HTTP Version Not Supported",
        )
    }
}

// --------------------
// --- STATUS CODES ---
// --------------------

pub enum StatusCode {
    Ok,                  // 200
    BadRequest,          // 400
    Unauthorized,        // 401
    Forbidden,           // 403
    NotFound,            // 404
    Timeout,             // 408
    VersionNotSupported, // 505
    InternalServerError, // 500
}

impl StatusCode {
    pub fn to_http(&self) -> &str {
        match self {
            StatusCode::Ok => "200 OK",
            StatusCode::BadRequest => "400 Bad Request",
            StatusCode::Unauthorized => "401 Unauthorized",
            StatusCode::Forbidden => "403 Forbidden",
            StatusCode::NotFound => "404 Not Found",
            StatusCode::Timeout => "408 Request Timeout",
            StatusCode::VersionNotSupported => "505 HTTP Version Not Supported",
            StatusCode::InternalServerError => "500 Internal Server Error",
        }
    }
}
