use std::collections;
use std::env;
use std::fmt;
use std::path;
use std::sync::Arc;

use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto::Builder;
use native_tls::{Identity};
use tokio::fs;
use tokio::net::TcpListener;

mod responses;

use config;

// scheme and authority does not change
// can clone all of these, would a touple (uri, host, port) suffice?

pub struct BuiltUri {
	uri: http::Uri,
	host: String,
	port: String,
}

pub enum ConfigParseError<'a> {
	HeaderError(http::header::InvalidHeaderValue),
	UriError(<http::Uri as TryFrom<String>>::Error),
	Error(&'a str),
}

impl fmt::Display for ConfigParseError<'_>  {
  fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
  	match self {
  		ConfigParseError::HeaderError(io_error) => write!(f, "{}", io_error),
  		ConfigParseError::UriError(json_error) => write!(f, "{}", json_error),
  		ConfigParseError::Error(error) => write!(f, "{}", error),
  	}
  }
}

// hash map needs to be string or uri
// if string
fn create_address_map(config: &config::Config) -> Result<collections::HashMap::<String, (http::Uri, String, String)>, ConfigParseError> {
  // will need to verify hashmap values as uris as well, do after mvp, input pruning / sanitizatio
  let mut hashmap: collections::HashMap::<String, (http::Uri, String, String)> = collections::HashMap::new();
  // separate into two functions? should be same amount of operations
  for (index, value) in config.addresses.iter() {
  	// this is separate
  	let index_uri = match http::Uri::try_from(index) {
  		Ok(uri) => uri,
  		Err(e) => return Err(ConfigParseError::UriError(e)),
  	};
  	
  	let host = match index_uri.host() {
			Some(uri) => uri,
			_ => return Err(ConfigParseError::Error("did not find authority")),
		};
		
  	let port = match index_uri.port() {
			Some(uri) => uri,
			_ => return Err(ConfigParseError::Error("did not find authority")),
		};
		// create address here
		
		let addr = host.clone().to_string() + ":" + port.as_str();
		

  	let dest_uri = match http::Uri::try_from(value) {
  		Ok(uri) => uri,
  		Err(e) => return Err(ConfigParseError::UriError(e)),
  	};
  	
  	let dest_uri_clone = dest_uri.clone();
  	
  	let dest_host = match dest_uri.host() {
			Some(uri) => uri,
			_ => return Err(ConfigParseError::Error("did not find authority")),
		};
		
  	let dest_port = match dest_uri.port() {
			Some(uri) => uri,
			_ => return Err(ConfigParseError::Error("did not find authority")),
		};
		let dest_addr = dest_host.clone().to_string() + ":" + dest_port.as_str();
		
  	hashmap.insert(host.to_string(), (dest_uri.clone(), dest_host.to_string(), dest_addr));
  }
  
  Ok(hashmap)
}

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
  
  // bind tcp listeners
  let address = format!("{}:{}", config.host, config.port);
  let listener = match TcpListener::bind(address).await {
  	Ok(l) => l,
  	Err(e) => return println!("tcp listener error:\n{}", e),
  };

	// destination addresses
  let addresses = match create_address_map(&config) {
  	Ok(addrs) => addrs,
  	Err(e) => return println!("address map error:\n{}", e),
  };
  let addresses_arc = Arc::new(addresses);
  
  // tls acceptor
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

	let tls_acceptor = match native_tls::TlsAcceptor::builder(pkcs8)
		.build() {
			Ok(native_acceptor) => tokio_native_tls::TlsAcceptor::from(native_acceptor),
			Err(e) => return println!("native_acceptor:\n{}", e),
	};

	// sever loop
  loop {
    let (socket, _remote_addr) = match listener.accept().await {
    	Ok(s) => s,
    	Err(_e) => {
				// log socket error
    		continue;
    	},
    };
    
		let io = match tls_acceptor.clone().accept(socket).await {
			Ok(s) => TokioIo::new(s),
			Err(_e) => {
				// log tls error
				continue;
			},
		};
		
  	let service = responses::Svc{
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

