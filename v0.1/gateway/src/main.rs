use std::collections;
use std::env;
use std::path;
use std::net;
use std::io;
use std::sync::Arc;

use http::Uri;
use hyper_util::server::conn::auto::Builder;
use hyper_util::rt::{TokioExecutor, TokioIo};
use tokio::fs;
use native_tls::{Identity, TlsAcceptor};
use tokio::net::TcpListener;

mod responses;

use config;

/* TODO
		- adjust source uris
		- get dest uri
		- create a http or https connection based on schema
		- send request, return response
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
	    	Err(e) => {
	    		println!("socket error: {}", e);
	    		continue;
	    	},
	    };
	    
    	let tls_acceptor = tls_acceptor.clone();
  		let tls_stream = match tls_acceptor.accept(socket).await {
  			Ok(s) => s,
  			Err(e) => {
  				println!("acceptor_error: {}", e);
  				continue;
  			},
  		};
  		
			let io = TokioIo::new(tls_stream);
    	let service = responses::Svc{
    		addresses: addresses_arc.clone(),
    	};
    	
    	tokio::task::spawn(async move {
    		Builder::new(TokioExecutor::new())
    			.serve_connection(io, service)
    			.await
    	});
    } 
}

fn create_address_map(config: &config::Config) -> Result<collections::HashMap::<http::Uri, http::Uri>, <http::Uri as TryFrom<String>>::Error> {
    // will need to verify hashmap values as uris as well, next step
    let mut hashmap: collections::HashMap::<http::Uri, http::Uri> = collections::HashMap::new();
    for (index, value) in config.addresses.iter() {
    	// create uri
    	let index_uri = match http::Uri::try_from(index) {
    		Ok(uri) => uri,
    		Err(e) => return Err(e),
    	};
    	let dest_uri = match http::Uri::try_from(value) {
    		Ok(uri) => uri,
    		Err(e) => return Err(e),
    	};
    	
    	hashmap.insert(index_uri, dest_uri);
    }
    
    Ok(hashmap)
}

