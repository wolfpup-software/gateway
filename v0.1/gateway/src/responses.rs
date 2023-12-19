use std::sync::Arc;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use http_body_util::{combinators::BoxBody, Full,  BodyExt};
use hyper::{Uri, Response, Request, StatusCode};
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::body::{Incoming as IncomingBody};
use hyper::service::Service;
use hyper_util::rt::TokioIo;

use tokio::net::TcpStream;

const HTML: &str = "text/html; charset=utf-8";
const INTERNAL_SERVER_ERROR: &str = "500 internal server error";

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
					http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};
		
		let mut dest_parts = match self.addresses.get(&requested_uri) {
			Some(sch) => sch.clone().into_parts(),
			_ => {
				// bad gateway
				return Box::pin(async {
					http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};
		dest_parts.path_and_query = req.uri().path_and_query().cloned();

		let composed_url = match http::Uri::from_parts(dest_parts) {
			Ok(sch) => sch,
			_ => {
				return Box::pin(async {
					// bad gateway
					http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};
		println!("{:?}", composed_url);
		
		
		// now send request
		
		// add new uri to req
		*req.uri_mut() = composed_url;
		

		println!("REQ:\n{:?}", req);

		// concatenate with no panics
    let host = req.uri().host().expect("uri has no host");
    let port = req.uri().port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);
    
    return Box::pin(async {
      let io = match TcpStream::connect(addr).await {
  			Ok(sch) => TokioIo::new(sch),
  			// unable to connect
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };
    	
    	// upgrade to use http
      let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
  			Ok(sch) => sch,
  			// unable to handshake
				_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
      };

      tokio::task::spawn(async move {
				if let Err(err) = conn.await { /* log connection, return gateway 502 or 504 */ }
			});
			
	    if let Ok(r) = sender.send_request(req).await {
				return Ok(r.map(|b| b.boxed()));
	    };
	    
	    // 502
	    http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR)
    });
	}
}

// 502 bad gateway
// 400 bad request (malformed request)
// 
fn http_code_response(code: &StatusCode, po: &'static str) -> Result<Response<BoxBody<bytes::Bytes, hyper::Error>>, hyper::http::Error> {
	Response::builder()
		.status(code)
		.header(CONTENT_TYPE, HeaderValue::from_static(HTML))
		.body(Full::new(bytes::Bytes::from(po)).map_err(|e| match e {}).boxed())
}

