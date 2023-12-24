use std::sync::Arc;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use http_body_util::{combinators::BoxBody, Full,  BodyExt};
use hyper::{Uri, Response, Request, StatusCode};
use http::uri::{Scheme, Port};
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::body::{Incoming as IncomingBody};
use hyper::service::Service;
use hyper_util::rt::TokioIo;

use hyper_util::client::legacy::Client;
use hyper_util::rt::TokioExecutor;

use native_tls::{Identity, TlsAcceptor, TlsConnector};

use tokio::net::TcpStream;

const HTML: &str = "text/html; charset=utf-8";
const INTERNAL_SERVER_ERROR: &str = "500 internal server error";

fn http_code_response(
	code: &StatusCode,
	po: &'static str,
) -> Result<
	Response<
		BoxBody<
			bytes::Bytes,
			hyper::Error,
		>
	>,
	hyper::http::Error
> {
	Response::builder()
		.status(code)
		.header(CONTENT_TYPE, HeaderValue::from_static(HTML))
		.body(Full::new(bytes::Bytes::from(po)).map_err(|e| match e {}).boxed())
}

/*
	below gets host from req headers,
	then uses host to get a cloned dest address from the arc'd address map.
	
	The req is declared as mutable
	the req.uri is used to generate the path_and_query
	
	the path and query of the dest address is updated to include the path and query of
	the original request
	
	the dest request is added to the URI of the original request
	
	that request is sent to the destination server
	
	 
	$Potential Caveat	
	In dev, the 'uri' of the request was only a path and query.
	I forget if that's true in production for most other libraries.
	
	
*/

pub struct Svc {
	pub addresses: Arc<HashMap<http::header::HeaderValue, http::Uri>>,
}

impl Service<Request<IncomingBody>> for Svc {
	type Response = Response<BoxBody<bytes::Bytes, hyper::Error>>;
	type Error = hyper::http::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
	
	fn call(&self, mut req: Request<IncomingBody>) -> Self::Future {
		println!("{:?}", req);

		// the following three sections can be done in one function
		// try a result that is Result<URI, Response>
		//
		// uri interpretation in separate function
		// digest requested uri
		let requested_uri = match req.headers().get("host") {
			Some(uri) => uri,
			_ => {
				return Box::pin(async {
					// bad request
					http_code_response(&StatusCode::BAD_REQUEST, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};
		
		// uri generation in separate function
		// return None if URI's don't match
		// return URI if complete
		let dest_parts = match self.addresses.get(&requested_uri) {
			Some(sch) => {
				let mut parts = sch.clone().into_parts();
				parts.path_and_query = req.uri().path_and_query().cloned();
				parts
			},
			_ => {
				// bad gateway
				return Box::pin(async {
					http_code_response(&StatusCode::BAD_GATEWAY, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};

		let composed_url = match http::Uri::from_parts(dest_parts) {
			Ok(sch) => sch,
			_ => {
				return Box::pin(async {
					// bad gateway
					http_code_response(&StatusCode::BAD_GATEWAY, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};
		println!("{:?}", composed_url);
		
		
		// add new uri to req
		*req.uri_mut() = composed_url;
		// add ip of request to "X-Forwared-For"
		

		println!("REQ:\n{:?}", req);


		// get in separate function
		// if none return function
		
		// concatenate with no panics
    let host = req.uri().host().expect("uri has no host");
    // bail if no host
    
    
    let port = match req.uri().port_u16() {
    	Some(p) => p,
    	_ => {
    		match req.uri().scheme() {
    			Some(a) => {
    				match a.as_str() {
    					"http" => 80,
    					"https" => 443,
    					"HTTP" => 80,
    					"HTTPS" => 443,
    					_ => 80,
    				}
    			},
    			_ => 80,
    		}
    	}
    };
    
    // bail if no host
    // bail if port is none and scheme is none, there is no way to get a port
    // return a "bad gateway"
    
    let addr = format!("{}:{}", host, port);
    // bail here too
    
    return Box::pin(async move {

    	
    	// upgrade to use https
    	//
    	// https connector uses http by default
    	// legacy client determines http2 or 1
    	// the legacy client seems overwhelming
    	// would rather default on version
    	//
    	// get client based on req vers
    	// create tokio socket io based on scheme
    	
    	/*
    	the legacy client way
	    let https = HttpsConnector::new();
      let client = Client::builder(TokioExecutor::new()).build::<_, BoxBody<bytes::Bytes, hyper::Error>>(https);
      if let Ok(resp) = client.request(req).await {
      	return Ok(resp.map(|b| b.boxed()));
      }
      */
      
      // get TLS or TCP steam based on http or https
      // Result<steam, http_code_response>

      
      let io = match TcpStream::connect(&addr).await {
  			Ok(client_stream) => TokioIo::new(client_stream),
  			// unable to connect
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
 
      
      // do we do https io or not
			match req.uri().scheme() {
				Some(s) => {
					// call a function that takes in and req
					// tokio tls connector
			    // let socket = TcpStream::connect(&addr).await?;
					// let cx = TlsConnector::builder().build()?;
					// let cx = tokio_native_tls::TlsConnector::from(cx);
					// let mut socket = cx.connect("www.rust-lang.org", socket).await?
				}
				_ => {},
			};
			
      
      // this is for http2 connectiopns
      let client_stream = match TcpStream::connect(&addr).await {
  			Ok(s) => s,
  			// unable to connect
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
      
      let cx = match TlsConnector::builder().build() {
  			Ok(s) => s,
  			// unable to connect
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
    	let tls_io = tokio_native_tls::TlsConnector::from(cx);
    	let host = match req.uri().host() {
  			Some(s) => s,
  			// unable to connect
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
      
    	let socket = match tls_io.connect(host, client_stream).await {
  			Ok(s) => TokioIo::new(s),
  			// unable to connect
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
      
      let (mut client, client_conn) = match hyper::client::conn::http2::handshake(TokioExecutor::new(), socket).await {
  			Ok(handshake) => handshake,
  			// unable to handshake
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
      
      tokio::task::spawn(async move {
				if let Err(err) = client_conn.await {
					/* log connection error */
				}
			});
			
      let resp = match client.send_request(req).await {
  			Ok(res) => {
					return Ok(res.map(|b| b.boxed()));
  			},
  			// unable to handshake
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
      
			/*
			
			// valid http1 code
      // https://github.com/hyperium/h2/blob/master/examples/client.rs
      let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
  			Ok(handshake) => handshake,
  			// unable to handshake
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };

      
      // the goal is to get connectors provided by tokio if the scheme is https or https
      // and retrive a sender and a connection depending if connection is http1.1 or http2
      

      tokio::task::spawn(async move {
				if let Err(err) = conn.await {
					/* log connection error */
				}
			});
			
			// successful request
	    if let Ok(r) = sender.send_request(req).await {
				return Ok(r.map(|b| b.boxed()));
	    };
	    */
	    
	    // 502
	    // http_code_response(&StatusCode::BAD_GATEWAY, &INTERNAL_SERVER_ERROR)
    });
	}
}

// 502 bad gateway
// 400 bad request (malformed request)
// 

