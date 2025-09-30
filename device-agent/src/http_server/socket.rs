// ------------------------
// --- TCP/UNIX SOCKETS ---
// ------------------------

use crate::http_server::router::handle_request;
use crate::moonlight_codec::MoonlightClient;
use crate::notifycast::NotifyCast;
use std::io::{Read, Result, Write};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::thread::spawn;
use std::{net::TcpStream, os::unix::net::UnixStream, time::Duration};

const RW_TIMEOUT: Option<Duration> = Some(Duration::from_secs(5));

/// A simple enum to abstract over TCP and UNIX socket streams
///
/// This allows us to write code that can handle both types of streams
/// without duplicating code for each type. Even though both the sockets
/// implement the Read + Write traits, some methods like
/// set_read_timeout and set_write_timeout are not available.
#[derive(Debug)]
pub enum Socket {
    TCP(TcpStream),
    UNIX(UnixStream),
}

#[derive(Debug, Clone)]
pub struct SocketContext {
    pub client: MoonlightClient,
    pub notify: NotifyCast,
    pub shutdown_flag: Arc<AtomicBool>,
}

impl Socket {
    pub fn handle_tcp_stream(stream: TcpStream, ctx: &SocketContext) {
        stream.set_nodelay(true).ok();
        Self::TCP(stream).handle_request(ctx);
    }

    pub fn handle_unix_stream(stream: UnixStream, ctx: &SocketContext) {
        Self::UNIX(stream).handle_request(ctx);
    }

    /// Spawns a new thread to handle the request
    fn handle_request(self, ctx: &SocketContext) {
        // Set read and write timeouts to avoid hanging connections
        let read_timeout = self.set_read_timeout(RW_TIMEOUT);
        let write_timeout = self.set_write_timeout(RW_TIMEOUT);

        // If setting the timeouts fails, drop the connection
        if read_timeout.is_err() || write_timeout.is_err() {
            return;
        }

        let ctx = ctx.clone();
        spawn(move || handle_request(self, &ctx));
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> Result<()> {
        match self {
            Self::TCP(s) => s.set_read_timeout(dur),
            Self::UNIX(s) => s.set_read_timeout(dur),
        }
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> Result<()> {
        match self {
            Self::TCP(s) => s.set_write_timeout(dur),
            Self::UNIX(s) => s.set_write_timeout(dur),
        }
    }

    pub fn write_all(&mut self, buf: &[u8]) -> Result<()> {
        match self {
            Self::TCP(s) => s.write_all(buf),
            Self::UNIX(s) => s.write_all(buf),
        }
    }

    pub fn flush(&mut self) -> Result<()> {
        match self {
            Self::TCP(s) => s.flush(),
            Self::UNIX(s) => s.flush(),
        }
    }

    pub fn send(&mut self, buf: &[u8]) -> bool {
        self.write_all(buf).is_ok() && self.flush().is_ok()
    }
}

impl Read for Socket {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        match self {
            Self::TCP(s) => s.read(buf),
            Self::UNIX(s) => s.read(buf),
        }
    }
}

impl Write for Socket {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        match self {
            Self::TCP(s) => s.write(buf),
            Self::UNIX(s) => s.write(buf),
        }
    }

    fn flush(&mut self) -> Result<()> {
        match self {
            Self::TCP(s) => s.flush(),
            Self::UNIX(s) => s.flush(),
        }
    }
}
