// --------------------
// --- HTTP REQUEST ---
// --------------------

use super::response::{FailureResp as FR, Resp};
use crate::moonlight_codec::MoonlightClient;
use serde_json::Value;
use std::{
    collections::HashMap,
    io::{BufRead, BufReader, ErrorKind, Read, Write},
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
}

#[derive(Debug, Clone)]
pub struct Req {
    pub method: Method,
    pub path: String,
    #[allow(dead_code)]
    pub headers: HashMap<String, String>,
    pub body: Option<Value>,
}

pub fn parse_request(
    buf_reader: &mut BufReader<impl Read + Write>,
    client: &MoonlightClient,
) -> Result<Req, Resp> {
    let (method, path) = parse_request_line(buf_reader)?;
    let headers = parse_request_headers(buf_reader)?;

    // Skip header validation for root and /stop-agent routes.
    if (method == Method::GET && path == "/") || (method == Method::DELETE && path == "/stop-agent")
    {
        return Ok(Req {
            method,
            path,
            headers,
            body: None,
        });
    }

    // Authenticate all other routes.
    validate_headers(&headers, client)?;
    let body = parse_request_body(buf_reader, &headers)?;

    Ok(Req {
        method,
        path,
        headers,
        body,
    })
}

fn parse_request_line(
    buf_reader: &mut BufReader<impl Read + Write>,
) -> Result<(Method, String), Resp> {
    let request_line = match read_line(buf_reader)? {
        None => return Err(FR::bad_request("Empty Request")),
        Some(line) => line,
    };

    let mut line_iter = request_line.split_whitespace();
    let http_method = line_iter.next().unwrap_or_default().to_uppercase();
    let http_path = line_iter.next().unwrap_or_default().to_string();
    let http_version = line_iter.next().unwrap_or_default().to_uppercase();

    if http_version != "HTTP/1.1" {
        return Err(FR::version_not_supported());
    }

    if http_path.is_empty() || !http_path.starts_with('/') {
        return Err(FR::bad_request("Invalid HTTP Path"));
    }

    let http_method = match http_method.as_str() {
        "GET" => Method::GET,
        "HEAD" => Method::HEAD,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        _ => return Err(FR::bad_request("Unsupported HTTP Method")),
    };

    Ok((http_method, http_path))
}

fn parse_request_headers(
    buf_reader: &mut BufReader<impl Read + Write>,
) -> Result<HashMap<String, String>, Resp> {
    const MAX_HEADERS: u8 = 64;

    let mut headers = HashMap::new();

    loop {
        let line = match read_line(buf_reader)? {
            None => break,
            Some(line) => line,
        };

        // split on first ':'
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| FR::bad_request("Malformed header line"))?;

        let name = name.trim().to_ascii_lowercase();
        let value = value.trim().to_string();

        if name.is_empty() {
            return Err(FR::bad_request("Empty header name"));
        }

        if headers.len() >= MAX_HEADERS as usize {
            return Err(FR::bad_request("Too many headers"));
        }

        headers.insert(name, value);
    }

    Ok(headers)
}

fn parse_request_body(
    buf_reader: &mut BufReader<impl Read + Write>,
    req_headers: &HashMap<String, String>,
) -> Result<Option<Value>, Resp> {
    // Reject chunked transfer-encoding explicitly.
    if let Some(te) = req_headers.get("transfer-encoding")
        && te.to_ascii_lowercase().contains("chunked")
    {
        return Err(FR::bad_request("Transfer-Encoding: chunked not supported"));
    }

    let content_length = req_headers.get("content-length");
    let content_type = req_headers.get("content-type");

    if content_length.is_none() {
        return Ok(None);
    }

    let content_length = content_length
        .unwrap()
        .parse::<u64>()
        .map_err(|_| FR::bad_request("Invalid Content-Length Header"))?;

    if content_length == 0 {
        return Ok(None);
    }

    if content_length > 64 * 1024 {
        return Err(FR::bad_request("Request Body Too Large"));
    }

    if content_type.is_none() {
        return Err(FR::bad_request("Missing Content-Type Header"));
    }

    let content_type = content_type.unwrap();

    if !content_type.to_lowercase().starts_with("application/json") {
        return Err(FR::bad_request("Content-Type Must Be application/json"));
    }

    let mut body_buf = vec![0; content_length as usize];

    buf_reader
        .take(content_length)
        .read_exact(&mut body_buf)
        .map_err(|_| FR::bad_request("Failed to read request body"))?;

    Ok(Some(serde_json::from_slice(&body_buf).map_err(|_| {
        FR::bad_request("Failed to parse JSON body")
    })?))
}

fn validate_headers(
    headers: &HashMap<String, String>,
    client: &MoonlightClient,
) -> Result<(), Resp> {
    let fleet_id = headers
        .get("x-fleet-id")
        .ok_or_else(|| FR::bad_request("Header X-Fleet-ID is missing"))?;

    if fleet_id.is_empty() {
        return Err(FR::bad_request("Header X-Fleet-ID is empty"));
    }

    if fleet_id != &client.fleet_id {
        return Err(FR::unauthorized("Fleet ID mismatch"));
    }

    let device_id = headers
        .get("x-device-id")
        .ok_or_else(|| FR::bad_request("Header X-Device-ID is missing"))?;

    if device_id.is_empty() {
        return Err(FR::bad_request("Header X-Device-ID is empty"));
    }

    if device_id != &client.device_id {
        return Err(FR::unauthorized("Device ID mismatch"));
    }

    Ok(())
}

fn read_line<RW: Read + Write>(reader: &mut BufReader<RW>) -> Result<Option<String>, Resp> {
    const MAX_LINE_LENGTH: usize = 8 * 1024; // 8 KiB per header line

    let mut out = Vec::with_capacity(256);

    loop {
        let buf = match reader.fill_buf() {
            Ok(b) => b,
            Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(ref e) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                return Err(FR::bad_request("Timed out reading line"));
            }
            Err(_) => return Err(FR::bad_request("Failed reading line")),
        };

        if buf.is_empty() {
            if out.is_empty() {
                return Err(FR::bad_request("Unexpected EOF while reading line"));
            } else {
                return Err(FR::bad_request("Unexpected EOF in line"));
            }
        }

        match buf.iter().position(|&b| b == b'\n') {
            Some(nl_idx) => {
                // Exclude trailing '\r' (or multiple '\r's), then enforce max length
                let mut end = nl_idx;
                while end > 0 && buf[end - 1] == b'\r' {
                    end -= 1;
                }

                if out.len() + end > MAX_LINE_LENGTH {
                    return Err(FR::bad_request("Line too long"));
                }

                out.extend_from_slice(&buf[..end]);

                // Consume through the newline
                reader.consume(nl_idx + 1);

                break;
            }
            None => {
                // No newline in current buffered chunk; enforce max length before appending
                if out.len() + buf.len() > MAX_LINE_LENGTH {
                    return Err(FR::bad_request("Line too long"));
                }

                let len = buf.len();
                out.extend_from_slice(buf);
                reader.consume(len);
            }
        }
    }

    if out.is_empty() {
        return Ok(None);
    }

    let line = std::str::from_utf8(&out).map_err(|_| FR::bad_request("Invalid Encoding"))?;

    Ok(Some(line.to_string()))
}
