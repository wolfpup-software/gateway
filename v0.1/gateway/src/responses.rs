use std::sync::Arc;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use http_body_util::{Full};
use hyper::{Uri, Response, Request, StatusCode};
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::body::{Incoming as IncomingBody};
use hyper::service::Service;


const HTML: &str = "text/html; charset=utf-8";
const INTERNAL_SERVER_ERROR: &str = "500 internal server error";

pub struct Svc {
	pub addresses: Arc<HashMap<Uri, Uri>>,
}

impl Service<Request<IncomingBody>> for Svc {
	type Response = Response<Full<bytes::Bytes>>;
	type Error = hyper::http::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
	
	fn call(&self, req: Request<IncomingBody>) -> Self::Future {
		// get URI from request
		// build hashable URI
		
		// get hashed uri destination
		
		// if req uses http, use http client
		// otherwise use https client
		
		// get request path and query
		
		// add to destnation path and query
		
		// send the actual request to new destination
		
		Box::pin(async {
		  response_500()
		})
	}
}

fn response_500() -> Result<Response<Full<bytes::Bytes>>, hyper::http::Error> {
	Response::builder()
		.status(StatusCode::INTERNAL_SERVER_ERROR)
		.header(CONTENT_TYPE, HeaderValue::from_static(HTML))
		.body(Full::new(bytes::Bytes::from(INTERNAL_SERVER_ERROR)))
}

