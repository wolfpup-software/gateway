use std::collections;
use std::env;
use std::path;
use std::net;
use std::io;
use std::sync::Arc;
use std::fmt;

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
        None => return println!("argument error:\nconfig params not found."),
    };

    let config = match config::Config::from_filepath(&args) {
        Ok(c) => c,
        Err(e) => return println!("configuration error:\n{}", e),
    };

    let addresses = match create_address_map(&config) {
    	Ok(addrs) => addrs,
    	Err(e) => return println!("address map error:\n{}", e),
    };
    let addresses_arc = Arc::new(addresses);
    
    let cert = match fs::read(&config.cert_filepath).await {
    	Ok(f) => f,
    	Err(e) => return println!("file error:\n{}", e),
    };
    
    let key = match fs::read(&config.key_filepath).await {
    	Ok(f) => f,
    	Err(e) => return println!("file error:\n{}", e),
    };

    let pkcs8 = match Identity::from_pkcs8(&cert, &key) {
    	Ok(pk) => pk,
    	Err(e) => return println!("cert error:\n{}", e),
    };

		let native_acceptor = match native_tls::TlsAcceptor::builder(pkcs8)
			.build() {
				Ok(accptr) => accptr,
				Err(e) => return println!("native_acceptor:\n{}", e),
		};
		
		let tls_acceptor = tokio_native_tls::TlsAcceptor::from(native_acceptor);
    
    let address = format!("{}:{}", config.host, config.port);
    let listener = match TcpListener::bind(address).await {
    	Ok(l) => l,
    	Err(e) => return println!("tcp listener error:\n{}", e),
    };

    loop {
	    let (socket, remote_addr) = match listener.accept().await {
	    	Ok(s) => s,
	    	Err(e) => {
	    		println!("socket error:\n{}", e);
	    		continue;
	    	},
	    };
	    
    	let tls_acceptor = tls_acceptor.clone();
  		let tls_stream = match tls_acceptor.accept(socket).await {
  			Ok(s) => s,
  			Err(e) => {
  				println!("acceptor_error:\n{}", e);
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

// enum for both errors here

pub enum ConfigParseError {
	HeaderError(http::header::InvalidHeaderValue),
	UriError(<http::Uri as TryFrom<String>>::Error),
}

impl fmt::Display for ConfigParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
    	match self {
    		ConfigParseError::HeaderError(io_error) => write!(f, "{}", io_error),
    		ConfigParseError::UriError(json_error) => write!(f, "{}", json_error),
    	}
    }
}


fn create_address_map(config: &config::Config) -> Result<collections::HashMap::<http::header::HeaderValue, http::Uri>, ConfigParseError> {
    // will need to verify hashmap values as uris as well, do after mvp, input pruning / sanitizatio
    let mut hashmap: collections::HashMap::<http::header::HeaderValue, http::Uri> = collections::HashMap::new();
    for (index, value) in config.addresses.iter() {
    	let index_uri = match http::Uri::try_from(index) {
    		Ok(uri) => uri,
    		Err(e) => return Err(ConfigParseError::UriError(e)),
    	};
    	
    	let index_header = match http::header::HeaderValue::try_from(index) {
    		Ok(uri) => uri,
    		Err(e) => return Err(ConfigParseError::HeaderError(e)),
    	};
    	let dest_uri = match http::Uri::try_from(value) {
    		Ok(uri) => uri,
    		Err(e) => return Err(ConfigParseError::UriError(e)),
    	};
    	
    	hashmap.insert(index_header, dest_uri);
    }
    
    Ok(hashmap)
}

