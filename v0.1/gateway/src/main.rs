use std::env;
use std::path;
use std::sync::Arc;

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use native_tls::Identity;
use tokio::fs;
use tokio::net::TcpListener;

mod config;
mod service;

/*
    split service and responses

    compile and test

    move on to file server
*/

#[tokio::main]
async fn main() {
    // create config
    let args = match env::args().nth(1) {
        Some(a) => path::PathBuf::from(a),
        None => return println!("argument error:\nconfig params not found."),
    };

    let config = match config::from_filepath(&args).await {
        Ok(c) => c,
        Err(e) => return println!("config error:\n{}", e),
    };

    // get addresses
    let host_address = config.host.clone() + ":" + &config.port.to_string();
    // if URIs fail to parse, the server fails to run.
    let addresses = match config::create_address_map(&config) {
        Ok(addrs) => addrs,
        Err(e) => return println!("address map error:\n{}", e),
    };
    let addresses_arc = Arc::new(addresses);

    // tls cert and keys
    let cert = match fs::read(&config.cert_filepath).await {
        Ok(f) => f,
        Err(e) => return println!("cert error:\n{}", e),
    };
    let key = match fs::read(&config.key_filepath).await {
        Ok(f) => f,
        Err(e) => return println!("key error:\n{}", e),
    };
    let pkcs8 = match Identity::from_pkcs8(&cert, &key) {
        Ok(pk) => pk,
        Err(e) => return println!("pkcs8 error:\n{}", e),
    };

    // create tls acceptor
    let tls_acceptor = match native_tls::TlsAcceptor::builder(pkcs8).build() {
        Ok(native_acceptor) => tokio_native_tls::TlsAcceptor::from(native_acceptor),
        Err(e) => return println!("native_acceptor error:\n{}", e),
    };

    // bind tcp listeners
    let listener = match TcpListener::bind(host_address).await {
        Ok(l) => l,
        Err(e) => return println!("tcp listener error:\n{}", e),
    };

    // sever loop
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
            // log response error
            Builder::new(TokioExecutor::new())
                .serve_connection(io, service)
                .await
        });
    }
}
