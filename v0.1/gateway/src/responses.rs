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

pub struct Svc {
	pub addresses: Arc<HashMap<http::header::HeaderValue, http::Uri>>,
}

impl Service<Request<IncomingBody>> for Svc {
	type Response = Response<BoxBody<bytes::Bytes, hyper::Error>>;
	type Error = hyper::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
	
	fn call(&self, mut req: Request<IncomingBody>) -> Self::Future {
		println!("{:?}", req);
		let req_uri = req.uri();
		let req_headers = req.headers();
		println!("{:?}", req_uri);
		println!("{:?}", req_headers.get("host"));
		
		
		/*
		what if we said:
		give me these three conditions
		Some(requested_uri), Some(dest_uri), ...
		
		then do a match
		if this and that and the other {
			return the box
		}
		
		return 500
		
		
		alternative:
		descriptor is made available
		"host" not provided
		"
		*/
		
		let requested_uri = match req_headers.get("host") {
			Some(uri) => uri,
			_ => {
				return Box::pin(async {
					response_500()
				}) 
			},
		};
		
		println!("{:?}", requested_uri);
		println!("got a hashed uri");
		let dest_uri = match self.addresses.get(&requested_uri) {
			Some(sch) => sch,
			_ => {
				return Box::pin(async {
					response_500()
				}) 
			},
		};

		let pnq = match req_uri.path_and_query() {
			Some(sch) => sch,
			_ => {
				return Box::pin(async {
					response_500()
				}) 
			},
		};
		
		println!("hashed a uri");
		let mut dest_parts = dest_uri.clone().into_parts();
		dest_parts.path_and_query = Some(pnq.clone());
		
		let composed_url = match http::Uri::from_parts(dest_parts) {
			Ok(sch) => sch,
			_ => {
				return Box::pin(async {
					response_500()
				}) 
			},
		};
		println!("{:?}", composed_url);
		
		
		// now send request
		
		// add new uri to req
		*req.uri_mut() = composed_url;
		

		println!("REQ:\n{:?}", req);

    let host = req.uri().host().expect("uri has no host");
    let port = req.uri().port_u16().unwrap_or(80);
    let addr = format!("{}:{}", host, port);
    
    return Box::pin(async {
      let client_stream = match TcpStream::connect(addr).await {
  			Ok(sch) => sch,
				_ => return response_500(),
      };
    	let io = TokioIo::new(client_stream);
    	

      let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
  			Ok(sch) => sch,
				_ => return response_500(),
      };

      tokio::task::spawn(async move {
				if let Err(err) = conn.await {
						println!("Connection failed: {:?}", err);
				}
			});
			
	    match sender.send_request(req).await {
				Ok(sch) => return Ok(sch.map(|b| b.boxed())),
				_ => return response_500(),
	    };
    });
	}
}

fn response_500() -> Result<Response<BoxBody<bytes::Bytes, hyper::Error>>, hyper::Error> {
	let mut res = Response::new(Full::new(INTERNAL_SERVER_ERROR.into()).map_err(|e| match e {}).boxed());
	
	*res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
	Ok(res)
	
	/*
	Response::builder()
		.status(StatusCode::INTERNAL_SERVER_ERROR)
		.header(CONTENT_TYPE, HeaderValue::from_static(HTML))
		.body(Full::new(INTERNAL_SERVER_ERROR.into()).map_err(|e| match e {}).boxed())
		*/
}

