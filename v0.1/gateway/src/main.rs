use std::collections;
use std::env;
use std::path;
use std::net;
use std::io;
use std::sync::Arc;

use hyper::Uri;
use tokio::fs;
use native_tls::{Identity, TlsAcceptor};
use tokio::net::TcpListener;

use config;

/* TODO
		- create a binary of key
		- create a binary of cert
		- attach arc clone to hyper::Service
*/

#[tokio::main]
async fn main() {
    let args = match env::args().nth(1) {
        Some(a) => path::PathBuf::from(a),
        None => return println!("argument error: no config params were found."),
    };

    let config = match config::Config::from_filepath(&args) {
        Ok(c) => c,
        Err(e) => return println!("configuration error: {}", e),
    };

		// get cert
		
		// get key

		// create tcp listener

    let addresses = match create_address_map(&config) {
    	Ok(addrs) => addrs,
    	Err(e) => return println!("address map error: {}", e),
    };
    let addresses_arc = Arc::new(addresses);
    println!("{:?}", addresses_arc);
    
    let cert = match fs::read(&config.cert_filepath).await {
    	Ok(f) => f,
    	Err(e) => return println!("file error: {}", e),
    };
    
    let key = match fs::read(&config.key_filepath).await {
    	Ok(f) => f,
    	Err(e) => return println!("file error: {}", e),
    };

    let pkcs8 = match Identity::from_pkcs8(&cert, &key) {
    	Ok(pk) => pk,
    	Err(e) => return println!("cert error: {}", e),
    };

		let native_acceptor = match native_tls::TlsAcceptor::builder(pkcs8)
			.build() {
				Ok(accptr) => accptr,
				Err(e) => return println!("native_acceptor: {}", e),
		};
		
		let tls_acceptor = tokio_native_tls::TlsAcceptor::from(native_acceptor);
    
    let address = format!("{}:{}", config.host, config.port);
    let listener = match TcpListener::bind(address).await {
    	Ok(l) => l,
    	Err(e) => return println!("tcp listener error {}", e),
    };
    // make a service that references the addresses arc
    
    //

    loop {
	    let (socket, remote_addr) = match listener.accept().await {
	    	Ok(s) => s,
	    	Err(e) => return println!("socket error: {}", e),
	    };
	    
    	let tls_acceptor = tls_acceptor.clone();
    	
    	// create service
    	
    	tokio::task::spawn(async move {
    		let tls_stream = match tls_acceptor.accept(socket).await {
    			Ok(s) => s,
    			Err(e) => {
    				println!("acceptor_error: {}", e);
    				return;
    			},
    		};
    		
    		
    		/*
    		http1::Builder::new()
    			.serve_connection(tls_stream, service)
    			.await
    		*/
    	});
    } 
}

fn create_address_map(config: &config::Config) -> Result<collections::HashMap::<hyper::Uri, hyper::Uri>, <hyper::Uri as TryFrom<String>>::Error> {
    // will need to verify hashmap values as uris as well, next step
    let mut hashmap: collections::HashMap::<hyper::Uri, hyper::Uri> = collections::HashMap::new();
    for (index, value) in config.addresses.iter() {
    	// create uri
    	let index_uri = match hyper::Uri::try_from(index) {
    		Ok(uri) => uri,
    		Err(e) => return Err(e),
    	};
    	let dest_uri = match hyper::Uri::try_from(value) {
    		Ok(uri) => uri,
    		Err(e) => return Err(e),
    	};
    	
    	hashmap.insert(index_uri, dest_uri);
    }
    
    Ok(hashmap)
}

/*

// A tiny async TLS echo server with Tokio
use native_tls::Identity;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/**
an example to setup a tls server.
how to test:
wget https://127.0.0.1:12345 --no-check-certificate
*/
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Bind the server's socket
    let addr = "127.0.0.1:12345".to_string();
    let tcp: TcpListener = TcpListener::bind(&addr).await?;

    // Create the TLS acceptor.
    let der = include_bytes!("identity.p12");
    let cert = Identity::from_pkcs12(der, "mypass")?;
    let cert = Identity::from_pkcs8(pem, key)?;
    let tls_acceptor =
        tokio_native_tls::TlsAcceptor::from(native_tls::TlsAcceptor::builder(cert).build()?);
    loop {
        // Asynchronously wait for an inbound socket.
        let (socket, remote_addr) = tcp.accept().await?;
        let tls_acceptor = tls_acceptor.clone();
        println!("accept connection from {}", remote_addr);
        tokio::spawn(async move {
            // Accept the TLS connection.
            let mut tls_stream = tls_acceptor.accept(socket).await.expect("accept error");
            // In a loop, read data from the socket and write the data back.

            let mut buf = [0; 1024];
            let n = tls_stream
                .read(&mut buf)
                .await
                .expect("failed to read data from socket");

            if n == 0 {
                return;
            }
            println!("read={}", unsafe {
                String::from_utf8_unchecked(buf[0..n].into())
            });
            tls_stream
                .write_all(&buf[0..n])
                .await
                .expect("failed to write data to socket");
        });
    }
}
*/
