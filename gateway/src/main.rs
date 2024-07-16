use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use native_tls::Identity;
use std::sync::Arc;
use std::{env, path};
use tokio::fs;
use tokio::net::TcpListener;

use config;

mod requests;
mod service;

#[tokio::main]
async fn main() {
    // create config
    let args = match env::args().nth(1) {
        Some(a) => path::PathBuf::from(a),
        None => return println!("config path not provided as argv[0]."),
    };
    let config = match config::from_filepath(&args).await {
        Ok(c) => c,
        Err(e) => return println!("{}", e),
    };

    // get addresses
    // if URIs fail to parse, the server fails to run.
    let addresses = match config::create_address_map(&config) {
        Ok(addrs) => addrs,
        Err(e) => return println!("{}", e),
    };
    let addresses_arc = Arc::new(addresses);

    // tls cert and keys
    let cert = match fs::read(&config.cert_filepath).await {
        Ok(f) => f,
        Err(e) => return println!("{}", e),
    };
    let key = match fs::read(&config.key_filepath).await {
        Ok(f) => f,
        Err(e) => return println!("{}", e),
    };
    let identity = match Identity::from_pkcs8(&cert, &key) {
        Ok(pk) => pk,
        Err(e) => return println!("{}", e),
    };

    // create tls acceptor
    let tls_acceptor = match native_tls::TlsAcceptor::builder(identity).build() {
        Ok(acceptor) => tokio_native_tls::TlsAcceptor::from(acceptor),
        Err(e) => return println!("{}", e),
    };

    // bind tcp listeners
    let listener = match TcpListener::bind(config.host_and_port).await {
        Ok(l) => l,
        Err(e) => return println!("{}", e),
    };

    loop {
        // rate limiting on _remote_addr
        let (socket, _remote_addr) = match listener.accept().await {
            Ok(s) => s,
            Err(_e) => {
                // log socket error
                continue;
            }
        };

        let io = match tls_acceptor.clone().accept(socket).await {
            Ok(s) => TokioIo::new(s),
            Err(_e) => {
                // log tls error
                continue;
            }
        };

        let service = service::Svc {
            addresses: addresses_arc.clone(),
        };

        tokio::task::spawn(async move {
            // log service error
            Builder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await
        });
    }
}
