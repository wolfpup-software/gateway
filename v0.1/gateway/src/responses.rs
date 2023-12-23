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
	I forget if that's true in production.
	
	- if http schema than http connector
	- if https schema than https
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

		let requested_uri = match req.headers().get("host") {
			Some(uri) => uri,
			_ => {
				return Box::pin(async {
					// bad request
					http_code_response(&StatusCode::BAD_REQUEST, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};
		
		// combine destination uri with
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
					http_code_response(&StatusCode::BAD_REQUEST, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};
		println!("{:?}", composed_url);
		
		
		// now send request
		
		// add new uri to req
		*req.uri_mut() = composed_url;
		// add ip of request to "X-Forwared-For"
		

		println!("REQ:\n{:?}", req);

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
    
    return Box::pin(async {
      let io = match TcpStream::connect(addr).await {
  			Ok(client_stream) => TokioIo::new(client_stream),
  			// unable to connect
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
    	
    	// upgrade to use http
    	//
    	// https connector uses http by default
    	// legacy client determines http2 or 1
      let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
  			Ok(handshake) => handshake,
  			// unable to handshake
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };

      tokio::task::spawn(async move {
				if let Err(err) = conn.await {
					/* log connection, return gateway 502 or 504 */
				}
			});
			
			// successful request
	    if let Ok(r) = sender.send_request(req).await {
				return Ok(r.map(|b| b.boxed()));
	    };
	    
	    // 502
	    http_code_response(&StatusCode::BAD_GATEWAY, &INTERNAL_SERVER_ERROR)
    });
	}
}

// 502 bad gateway
// 400 bad request (malformed request)
// 

