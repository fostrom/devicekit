use crate::{
    moonlight_codec::{Codec, MoonlightPacket},
    moonlight_socket,
};
use anyhow::{Context, Result, anyhow};
use rustls::{ClientConnection, StreamOwned};
use sha2::{Digest, Sha256};
use std::{
    io::{ErrorKind, Read, Write},
    net::{SocketAddr, TcpStream, ToSocketAddrs},
    time::{Duration, Instant},
};

const PROD_HOST: &str = "device.fostrom.dev";
const PROD_PORT: u16 = 8484;

const TOTAL_WAIT_FOR_SERVER_CLOSE: Duration = Duration::from_secs(5);
const READ_TIMEOUT: Duration = Duration::from_millis(250);
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);

pub fn run() -> i32 {
    let total_start = Instant::now();
    let result = run_inner();

    let (status, exit_code) = match &result {
        Ok(()) => ("OK", 0),
        Err(_) => ("FAILED", 1),
    };

    if let Err(e) = result {
        println!("failed: test-conn");
        print_error_chain(&e);
    }

    println!(
        "summary: status={status} exit_code={exit_code} total_elapsed_ms={}",
        total_start.elapsed().as_millis()
    );
    exit_code
}

fn run_inner() -> Result<()> {
    println!("test-conn: target={PROD_HOST}:{PROD_PORT}");
    println!(
        "env: version=v{} os={} arch={}",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    );

    let dns_start = Instant::now();
    let addrs = resolve_prod_addrs().context("dns_lookup_failed")?;
    println!(
        "dns: ok elapsed_ms={} addrs={}",
        dns_start.elapsed().as_millis(),
        addrs
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );

    let start = Instant::now();
    let mut stream = moonlight_socket::tls_open(PROD_PORT).context("tls_open_failed")?;
    let open_elapsed = start.elapsed();

    stream
        .sock
        .set_read_timeout(Some(READ_TIMEOUT))
        .context("set_read_timeout_failed")?;

    println!("tcp: connect_ms={}", open_elapsed.as_millis());
    println!("tcp: read_timeout_ms={}", READ_TIMEOUT.as_millis());

    if let Ok(local) = stream.sock.local_addr() {
        println!("tcp: local_addr={local}");
    }
    if let Ok(peer) = stream.sock.peer_addr() {
        println!("tcp: peer_addr={peer}");
    }

    let hs_start = Instant::now();
    force_tls_handshake(&mut stream, TLS_HANDSHAKE_TIMEOUT).context("tls_handshake_failed")?;
    println!(
        "tls: handshake_ok elapsed_ms={}",
        hs_start.elapsed().as_millis()
    );
    print_tls_details(&stream);

    let close_bytes = Codec::encode(&MoonlightPacket::client_close_connection())
        .context("encode_close_connection_failed")?;

    let write_start = Instant::now();
    stream
        .write_all(&close_bytes)
        .context("write_close_connection_failed")?;
    println!(
        "moonlight: sent_close ok bytes={} elapsed_ms={}",
        close_bytes.len(),
        write_start.elapsed().as_millis()
    );

    let wait_start = Instant::now();
    println!(
        "moonlight: waiting_close_ack timeout_ms={}",
        TOTAL_WAIT_FOR_SERVER_CLOSE.as_millis()
    );
    wait_for_server_close(&mut stream, TOTAL_WAIT_FOR_SERVER_CLOSE)
        .context("wait_for_server_close_failed")?;

    println!(
        "moonlight: recv_close_ack ok waited_ms={}",
        wait_start.elapsed().as_millis()
    );
    Ok(())
}

fn resolve_prod_addrs() -> Result<Vec<SocketAddr>> {
    let mut addrs = (PROD_HOST, PROD_PORT)
        .to_socket_addrs()
        .with_context(|| format!("failed to resolve {PROD_HOST}:{PROD_PORT}"))?
        .collect::<Vec<_>>();
    addrs.sort();
    addrs.dedup();
    Ok(addrs)
}

fn force_tls_handshake(
    stream: &mut StreamOwned<ClientConnection, TcpStream>,
    timeout: Duration,
) -> Result<()> {
    let start = Instant::now();
    while stream.conn.is_handshaking() {
        if start.elapsed() > timeout {
            return Err(anyhow!(
                "tls_handshake_timeout: timeout_ms={}",
                timeout.as_millis()
            ));
        }

        let (conn, sock) = (&mut stream.conn, &mut stream.sock);
        conn.complete_io(sock)
            .map_err(anyhow::Error::from)
            .with_context(|| "tls_complete_io_failed")?;
    }
    Ok(())
}

fn wait_for_server_close<R: Read>(reader: &mut R, total_timeout: Duration) -> Result<()> {
    const EXPECTED_CLOSE_ACK_BYTES: [u8; 2] = [1, 1];

    let start = Instant::now();
    let mut total_read = 0usize;
    let mut total_reads = 0usize;
    let mut received: Vec<u8> = Vec::with_capacity(EXPECTED_CLOSE_ACK_BYTES.len());

    while start.elapsed() < total_timeout {
        let mut buf = [0u8; 8192];
        match reader.read(&mut buf) {
            Ok(0) => {
                return Err(anyhow!(
                    "server_closed_connection_without_close_ack: expected={:?} received={received:?} read_bytes={total_read} reads={total_reads}",
                    EXPECTED_CLOSE_ACK_BYTES
                ));
            }
            Ok(n) => {
                total_reads += 1;
                total_read += n;
                println!("moonlight: rx bytes={n} total_bytes={total_read} reads={total_reads}");

                received.extend_from_slice(&buf[..n]);

                if received.len() == EXPECTED_CLOSE_ACK_BYTES.len() {
                    if received.as_slice() == EXPECTED_CLOSE_ACK_BYTES {
                        return Ok(());
                    }
                    return Err(anyhow!(
                        "unexpected_close_ack_bytes: expected={:?} received={received:?}",
                        EXPECTED_CLOSE_ACK_BYTES
                    ));
                }

                if received.len() > EXPECTED_CLOSE_ACK_BYTES.len() {
                    return Err(anyhow!(
                        "unexpected_extra_bytes_waiting_for_close_ack: expected={:?} received={received:?}",
                        EXPECTED_CLOSE_ACK_BYTES
                    ));
                }
            }
            Err(e) => match e.kind() {
                ErrorKind::WouldBlock | ErrorKind::TimedOut => continue,
                ErrorKind::Interrupted => continue,
                _ => return Err(e).context("read_failed"),
            },
        }
    }

    Err(anyhow!(
        "timeout_waiting_for_close_ack: timeout_ms={} expected={:?} received={received:?} read_bytes={total_read} reads={total_reads}",
        total_timeout.as_millis(),
        EXPECTED_CLOSE_ACK_BYTES
    ))
}

fn print_tls_details(stream: &StreamOwned<ClientConnection, TcpStream>) {
    if let Some(v) = stream.conn.protocol_version() {
        println!("tls: protocol={v:?}");
    } else {
        println!("tls: protocol=unknown");
    }

    if let Some(cs) = stream.conn.negotiated_cipher_suite() {
        println!("tls: cipher_suite={:?}", cs.suite());
    } else {
        println!("tls: cipher_suite=unknown");
    }

    if let Some(alpn) = stream.conn.alpn_protocol() {
        println!("tls: alpn={}", String::from_utf8_lossy(alpn));
    } else {
        println!("tls: alpn=none");
    }

    match stream.conn.peer_certificates() {
        None => println!("tls: peer_certs=none"),
        Some(certs) => {
            println!("tls: peer_certs_count={}", certs.len());
            for (i, cert) in certs.iter().enumerate() {
                let fp = Sha256::digest(cert.as_ref());
                println!("tls: peer_cert_sha256[{i}]={}", hex(fp.as_slice()));
            }
        }
    }
}

fn print_error_chain(err: &anyhow::Error) {
    for (i, cause) in err.chain().enumerate() {
        println!("error[{i}]: {cause}");

        if let Some(ioe) = cause.downcast_ref::<std::io::Error>() {
            println!(
                "error[{i}]_io: kind={:?} raw_os_error={:?}",
                ioe.kind(),
                ioe.raw_os_error()
            );
        }

        println!("error[{i}]_debug: {cause:?}");
    }
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        out.push(HEX[(b >> 4) as usize] as char);
        out.push(HEX[(b & 0x0f) as usize] as char);
    }
    out
}

// -------------
// --- TESTS ---
// -------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex() {
        assert_eq!(hex(&[0x00, 0x0f, 0x10, 0xff]), "000f10ff");
    }
}
