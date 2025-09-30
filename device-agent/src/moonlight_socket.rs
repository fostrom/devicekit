use anyhow::{Result, anyhow};
use either::Either;
use rustls::{
    ClientConfig, ClientConnection, RootCertStore, StreamOwned, pki_types::CertificateDer,
};
use std::{
    io::{ErrorKind, Read, Write},
    net::{Shutdown, TcpStream},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, TryRecvError},
    },
    thread::JoinHandle,
    time::Duration,
};

use crate::moonlight_codec::{ClientEvent, ConnectMode, GeneralErrors};

type TlsStream = StreamOwned<ClientConnection, TcpStream>;
type Stream = Either<TcpStream, TlsStream>;

pub fn connect(
    connect_mode: ConnectMode,
    mailbox_chan: Sender<ClientEvent>,
    write_chan: Receiver<Vec<u8>>,
) -> Result<(JoinHandle<()>, impl FnOnce())> {
    // Shutdown Flag
    // An AtomicBool shared between the socket thread and the caller.
    // The caller gets a close() function which simply sets the
    // shutdown flag to true, causing the thread loop to terminate
    let shutdown_flag = Arc::new(AtomicBool::new(false));
    let shutdown_flag_for_thread = shutdown_flag.clone();
    let close = move || shutdown_flag.store(true, Ordering::SeqCst);

    let mut stream: Stream = match connect_mode {
        ConnectMode::Local(port) => Either::Left(tcp_open(port)?),
        ConnectMode::Prod => Either::Right(tls_open(8484)?), // default Prod port
    };

    let handle = std::thread::spawn(move || {
        // Main Transport Loop:
        // 1. Reads from write_chan and writes to socket
        // 2. Reads from socket and writes to mailbox_chan
        // 3. In case either fails, the loop breaks and the socket is closed
        //
        // Reads block for 50ms.
        // Writes are immediate but can block up to 2s if the kernel buffer is full
        // or the network is slow.
        while !shutdown_flag_for_thread.load(Ordering::SeqCst) {
            if push_bytes_to_socket(&write_chan, &mut stream).is_err() {
                break;
            }

            if pull_bytes_from_socket(&mailbox_chan, &mut stream).is_err() {
                break;
            }
        }

        match stream {
            Either::Left(mut stream) => tcp_close(&mut stream),
            Either::Right(mut stream) => tls_close(&mut stream),
        }

        let _ = mailbox_chan.send(ClientEvent::TransportClose);
    });

    Ok((handle, close))
}

fn pull_bytes_from_socket(mailbox_chan: &Sender<ClientEvent>, stream: &mut Stream) -> Result<()> {
    match socket_read(stream) {
        Err(e) => Err(e),
        Ok(None) => Ok(()),
        Ok(Some(bytes)) => {
            let e = ClientEvent::TransportRecv(bytes);
            mailbox_chan
                .send(e)
                .map_err(|_| anyhow!(GeneralErrors::ChannelWriteFailed))
        }
    }
}

fn push_bytes_to_socket(write_chan: &Receiver<Vec<u8>>, stream: &mut Stream) -> Result<()> {
    loop {
        match write_chan.try_recv() {
            Ok(bytes) => socket_write(stream, &bytes)?,
            Err(TryRecvError::Empty) => return Ok(()),
            Err(TryRecvError::Disconnected) => {
                return Err(anyhow!(GeneralErrors::ChannelReadFailed));
            }
        }
    }
}

fn make_tcp_socket(addr: String) -> Result<TcpStream> {
    let socket = TcpStream::connect(addr)?;

    // Disable TCP Buffering
    socket.set_nodelay(true)?;

    // Should block on read and write
    socket.set_nonblocking(false)?;

    // However, should only block for 50ms while reading
    socket.set_read_timeout(Some(Duration::from_millis(50)))?;

    // And should only block for 2s while writing
    // in case the connection is genuinely slow
    // or the Kernel TCP buffer is full
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;

    Ok(socket)
}

fn tcp_open(port: u16) -> Result<TcpStream> {
    make_tcp_socket(format!("127.0.0.1:{port}"))
}

/// Open a TLS connection to device.fostrom.dev at the given port
fn tls_open(port: u16) -> Result<TlsStream> {
    let socket = make_tcp_socket(format!("device.fostrom.dev:{port}"))?;
    let conn = ClientConnection::new(tls_conf(), "device.fostrom.dev".try_into()?)?;
    Ok(StreamOwned::new(conn, socket))
}

fn tcp_close(stream: &mut TcpStream) {
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}

fn tls_close(stream: &mut TlsStream) {
    stream.conn.send_close_notify();
    let _ = stream.flush();
    let _ = stream.sock.shutdown(Shutdown::Both);
}

fn socket_write(stream: &mut Stream, data: &[u8]) -> Result<()> {
    stream.write_all(data)?;
    Ok(())
}

fn socket_read(stream: &mut Stream) -> Result<Option<Vec<u8>>> {
    let mut buf = [0u8; 8192];

    match stream.read(&mut buf) {
        Ok(0) => Err(anyhow!("Connection Terminated by Server")),
        Ok(n) => Ok(Some(buf[..n].to_vec())),
        Err(e) => match e.kind() {
            ErrorKind::WouldBlock | ErrorKind::TimedOut => Ok(None),
            ErrorKind::Interrupted => socket_read(stream),
            _ => Err(e.into()),
        },
    }
}

// ----------------------------------------
// --- CERTIFICATE STORE AND TLS CONFIG ---
// ----------------------------------------

const ISRG_ROOT_X1: CertificateDer =
    CertificateDer::from_slice(include_bytes!("../certs/isrg-root-x1.der"));

const ISRG_ROOT_X2: CertificateDer =
    CertificateDer::from_slice(include_bytes!("../certs/isrg-root-x2.der"));

fn tls_conf() -> Arc<ClientConfig> {
    let mut root_store = RootCertStore::empty();
    root_store.add(ISRG_ROOT_X1).unwrap();
    root_store.add(ISRG_ROOT_X2).unwrap();

    let config = ClientConfig::builder()
        .with_root_certificates(root_store)
        .with_no_client_auth();

    Arc::new(config)
}

// -------------
// --- TESTS ---
// -------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        sync::mpsc::channel,
        thread,
        time::Duration,
    };

    #[test]
    fn test_connect_local() {
        // Spin up a local TCP server bound to an ephemeral port
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind TCP listener");
        let port = listener.local_addr().unwrap().port();

        // Server thread: verify client->server and server->client traffic, and close behavior
        let server_handle = thread::spawn(move || {
            let (mut stream, _addr) = listener.accept().expect("accept connection");
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .expect("set server read timeout");

            // Expect bytes from the client
            let mut buf = vec![0u8; b"ping-client".len()];
            stream
                .read_exact(&mut buf)
                .expect("server read_exact from client");
            assert_eq!(&buf, b"ping-client");

            // Send bytes to the client
            stream
                .write_all(b"pong-server")
                .expect("server write to client");

            // Wait for client close; read() should return 0 on orderly shutdown
            let mut tmp = [0u8; 1];
            let _ = stream.read(&mut tmp);
        });

        // Client side channels and connection
        let (mailbox_tx, mailbox_rx) = channel();
        let (write_tx, write_rx) = channel();
        let (handle, close) =
            connect(ConnectMode::Local(port), mailbox_tx, write_rx).expect("client connect");

        // Send data to server via the client's write channel
        write_tx
            .send(b"ping-client".to_vec())
            .expect("send to client write channel");

        // Expect data from server via mailbox channel
        let event = mailbox_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("receive TransportRecv");

        if let ClientEvent::TransportRecv(bytes) = event {
            assert_eq!(bytes, b"pong-server".to_vec());
        } else {
            panic!("expected TransportRecv event");
        }

        // Close the client connection and expect a TransportClose
        close();

        let close_event = mailbox_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("receive TransportClose");

        match close_event {
            ClientEvent::TransportClose => {}
            _ => panic!("expected TransportClose"),
        }

        // Ensure threads exit cleanly
        handle.join().expect("client thread join");
        server_handle.join().expect("server thread join");
    }

    #[test]
    fn test_tls_open_pong() {
        // Connect to production TLS endpoint to ensure certificates are correct
        let mut stream = tls_open(8484).expect("tls open");

        // Allow sufficient time for handshake + server reply
        stream
            .sock
            .set_read_timeout(Some(Duration::from_secs(5)))
            .expect("set read timeout");

        // Send any payload
        stream.write_all(&[1, 0]).expect("write to tls server");
        stream.flush().expect("flush after write");

        let mut buf = vec![];
        stream.read_to_end(&mut buf).unwrap();
        assert_eq!(buf.len(), 0);

        // Close gracefully
        tls_close(&mut stream);
    }
}
