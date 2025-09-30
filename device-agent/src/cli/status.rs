// --------------------------
// --- CLI STATUS HANDLER ---
// --------------------------

use super::SOCK_FILE;
use anyhow::{Result, anyhow};
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;

pub fn agent_status() {
    if Path::new(SOCK_FILE).exists() {
        let status = req_status();
        println!("running\n\n{status}");
    } else {
        println!("not_running");
    }
}

pub fn fetch_status() -> Result<()> {
    match UnixStream::connect(SOCK_FILE) {
        Ok(mut stream) => {
            let _ = stream.write_all(b"GET / HTTP/1.1\r\n\r\n");
            let mut buffer = String::new();
            let _ = stream.read_to_string(&mut buffer);

            if buffer.contains("200 OK") {
                Ok(())
            } else {
                Err(anyhow!("Failed to fetch status"))
            }
        }
        Err(e) => Err(anyhow!("error_sending_status_request: {e}")),
    }
}

pub fn req_status() -> String {
    match UnixStream::connect(SOCK_FILE) {
        Ok(mut stream) => {
            let _ = stream.write_all(b"GET / HTTP/1.1\r\n\r\n");
            let mut buffer = String::new();
            let _ = stream.read_to_string(&mut buffer);
            buffer
        }
        Err(e) => format!("error_sending_status_request: {e}"),
    }
}
