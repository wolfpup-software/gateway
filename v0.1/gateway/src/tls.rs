/* 
    Copy / Pasted from:
    https://github.com/rustls/hyper-rustls/blob/main/examples/server.rs
*/

use core::task::{Context, Poll};
use futures_util::ready;
use hyper::server::accept::Accept;
use hyper::server::conn::{AddrIncoming, AddrStream};
use std::fs;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync;
use tokio_rustls::rustls::ServerConfig;
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use std::path;

enum State {
    Handshaking(tokio_rustls::Accept<AddrStream>),
    Streaming(tokio_rustls::server::TlsStream<AddrStream>),
}

// tokio_rustls::server::TlsStream doesn't expose constructor methods,
// so we have to TlsAcceptor::accept and handshake to have access to it
// TlsStream implements AsyncRead/AsyncWrite handshaking tokio_rustls::Accept first
pub struct TlsStream {
    state: State,
}

impl TlsStream {
    fn new(stream: AddrStream, config: sync::Arc<ServerConfig>) -> TlsStream {
        let accept = tokio_rustls::TlsAcceptor::from(config).accept(stream);
        TlsStream {
            state: State::Handshaking(accept),
        }
    }
}

impl AsyncRead for TlsStream {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context,
        buf: &mut ReadBuf,
    ) -> Poll<io::Result<()>> {
        let pin = self.get_mut();
        match pin.state {
            State::Handshaking(ref mut accept) => match ready!(Pin::new(accept).poll(cx)) {
                Ok(mut stream) => {
                    let result = Pin::new(&mut stream).poll_read(cx, buf);
                    pin.state = State::Streaming(stream);
                    result
                }
                Err(err) => Poll::Ready(Err(err)),
            },
            State::Streaming(ref mut stream) => Pin::new(stream).poll_read(cx, buf),
        }
    }
}

impl AsyncWrite for TlsStream {
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let pin = self.get_mut();
        match pin.state {
            State::Handshaking(ref mut accept) => match ready!(Pin::new(accept).poll(cx)) {
                Ok(mut stream) => {
                    let result = Pin::new(&mut stream).poll_write(cx, buf);
                    pin.state = State::Streaming(stream);
                    result
                }
                Err(err) => Poll::Ready(Err(err)),
            },
            State::Streaming(ref mut stream) => Pin::new(stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.state {
            State::Handshaking(_) => Poll::Ready(Ok(())),
            State::Streaming(ref mut stream) => Pin::new(stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match self.state {
            State::Handshaking(_) => Poll::Ready(Ok(())),
            State::Streaming(ref mut stream) => Pin::new(stream).poll_shutdown(cx),
        }
    }
}

pub struct TlsAcceptor {
    config: sync::Arc<ServerConfig>,
    incoming: AddrIncoming,
}

impl TlsAcceptor {
    pub fn new(config: sync::Arc<ServerConfig>, incoming: AddrIncoming) -> TlsAcceptor {
        TlsAcceptor { config, incoming }
    }
}

impl Accept for TlsAcceptor {
    type Conn = TlsStream;
    type Error = io::Error;

    fn poll_accept(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<Self::Conn, Self::Error>>> {
        let pin = self.get_mut();
        match ready!(Pin::new(&mut pin.incoming).poll_accept(cx)) {
            Some(Ok(sock)) => Poll::Ready(Some(Ok(TlsStream::new(sock, pin.config.clone())))),
            Some(Err(e)) => Poll::Ready(Some(Err(e))),
            None => Poll::Ready(None),
        }
    }
}



// Load public certificate from file.
fn load_certs(filename: &path::PathBuf) -> io::Result<Vec<rustls::Certificate>> {
    // Open certificate file.
    let certfile = fs::File::open(filename)?;
    let mut reader = io::BufReader::new(certfile);

    // Load and return certificate.
    let certs = rustls_pemfile::certs(&mut reader)
        .map_err(|_| error("failed to load certificate".into()))?;
    Ok(certs
        .into_iter()
        .map(rustls::Certificate)
        .collect())
}

fn error(err: String) -> io::Error {
    io::Error::new(io::ErrorKind::Other, err)
}

// Load private key from file.
fn load_private_key(filename: &path::PathBuf) -> io::Result<rustls::PrivateKey> {
    let keyfile = fs::File::open(filename)?;
    let keys = rustls_pemfile::pkcs8_private_keys(&mut io::BufReader::new(keyfile))?;
    if keys.len() != 1 {
        return Err(error("expected a single private key".into()));
    }

    Ok(rustls::PrivateKey(keys[0].clone()))
}

pub fn create_tls_config(
    cert_filepath: &path::PathBuf,
    key_filepath: &path::PathBuf,
) -> Result<sync::Arc<ServerConfig>, io::Error>  {
    // Load public certificate.
    let certs = load_certs(cert_filepath)?;
    // Load private key.
    let key = load_private_key(key_filepath)?;
    // Do not use client certificate authentication.
    let mut cfg = rustls::ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        .with_single_cert(certs, key)
        .map_err(|e| error(format!("{}", e)))?;
    // Configure ALPN to accept HTTP/2, HTTP/1.1 in that order.
    cfg.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
    Ok(sync::Arc::new(cfg))
}

// // Prepare the TLS client config
// let tls = match ca {
//     Some(ref mut rd) => {
//         // Read trust roots
//         let certs = rustls_pemfile::certs(rd)
//             .map_err(|_| error("failed to load custom CA store".into()))?;
//         let mut roots = RootCertStore::empty();
//         roots.add_parsable_certificates(&certs);
//         // TLS client config using the custom CA store for lookups
//         rustls::ClientConfig::builder()
//             .with_safe_defaults()
//             .with_root_certificates(roots)
//             .with_no_client_auth()
//     }
//     // Default TLS client config with native roots
//     None => rustls::ClientConfig::builder()
//         .with_safe_defaults()
//         .with_native_roots()
//         .with_no_client_auth(),
// };
// // Prepare the HTTPS connector
// let https = hyper_rustls::HttpsConnectorBuilder::new()
//     .with_tls_config(tls)
//     .https_or_http()
//     .enable_http1()
//     .build();