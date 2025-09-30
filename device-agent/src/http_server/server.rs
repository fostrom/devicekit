// -------------------
// --- HTTP SERVER ---
// -------------------

use crate::http_server::{SocketContext, socket::Socket};
use anyhow::Result;
use socket2::{Domain, Protocol, Socket as Socket2, Type};
use std::{
    fs::{self, Permissions},
    io::ErrorKind,
    net::{SocketAddr, TcpListener},
    os::unix::{fs::PermissionsExt, net::UnixListener},
    sync::atomic::Ordering,
    thread,
    time::Duration,
};

/// Starts the UNIX Socket Server
///
/// Run this function after the /tmp/fostrom directory has been created
pub fn unix_server(ctx: &SocketContext) -> Result<()> {
    let socket_path = "/tmp/fostrom/agent.sock";
    let _ = fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;
    fs::set_permissions(socket_path, Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;

    loop {
        if ctx.shutdown_flag.load(Ordering::SeqCst) {
            break;
        }

        match listener.accept() {
            Ok((stream, _addr)) => {
                Socket::handle_unix_stream(stream, ctx);
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    let _ = fs::remove_file(socket_path);
    Ok(())
}

/// Starts the TCP Socket Server
pub fn tcp_server(ctx: &SocketContext) -> Result<()> {
    let addr: SocketAddr = "127.0.0.1:8585".parse()?;
    let socket = Socket2::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    socket.set_reuse_address(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    let listener: TcpListener = socket.into();
    listener.set_nonblocking(true)?;

    loop {
        if ctx.shutdown_flag.load(Ordering::SeqCst) {
            break;
        }

        match listener.accept() {
            Ok((stream, _addr)) => {
                Socket::handle_tcp_stream(stream, ctx);
            }
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(50));
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }

    Ok(())
}
