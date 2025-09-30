// --------------------------
// --- HTTP SERVER MODULE ---
// --------------------------

mod cmd;
mod events;
mod request;
mod response;
mod router;
mod server;
mod socket;

use anyhow::Result;
pub use socket::SocketContext;

pub fn start_unix_server(ctx: &SocketContext) -> Result<()> {
    server::unix_server(ctx)
}

pub fn start_tcp_server(ctx: &SocketContext) -> Result<()> {
    server::tcp_server(ctx)
}
