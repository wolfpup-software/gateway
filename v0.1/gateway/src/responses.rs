use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::collections;

use http_body_util::{combinators::BoxBody, Full,  BodyExt};
use hyper::body::{Incoming};
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::{Response, Request, StatusCode};
use hyper::service::Service;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use native_tls::{TlsConnector};
use tokio::net::TcpStream;
use hyper::client::conn::{http1, http2};

const HTML: &str = "text/html; charset=utf-8";
const INTERNAL_SERVER_ERROR: &str = "500 internal server error";
const BAD_GATEWAY: &str = "BAD_GATEWAY";

type BoxedResponse = Response<
	BoxBody<
		bytes::Bytes,
		hyper::Error,
	>
>;

/*
	below gets host from req headers,
	then uses host to get a cloned dest address from the arc'd address map.
	
	The req is declared as mutable
	the req.uri is used to generate the path_and_query
	
	the path and query of the dest address is updated to include the path and query of
	the original request
	
	the dest request is added to the URI of the original request
	
	that request is sent to the destination server
*/

pub struct Svc {
	pub addresses: Arc<HashMap<String, (http::Uri, String, String)>>,
}

impl Service<Request<Incoming>> for Svc {
	type Response = BoxedResponse;
	type Error = http::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn call(&self, mut req: Request<Incoming>) -> Self::Future {
		println!("{:?}", req);
		// acount for http1 and http2 headers
		let requested_uri = match get_uri_from_host_or_authority(&req, &self.addresses) {
			Some(uri) => uri,
			_ => {
				return Box::pin(async {
					// bad request
					http_code_response(&StatusCode::BAD_REQUEST, &INTERNAL_SERVER_ERROR)
				})
			},
		};
		
		// can get host and addr here as well, no need to generate it
		// clone move on

		let (composed_url, host, addr) = match create_dest_uri(&req, &self.addresses, &requested_uri) {
			Some(uri) => uri,
			_ => {
				return Box::pin(async {
					http_code_response(&StatusCode::BAD_GATEWAY, &INTERNAL_SERVER_ERROR)
				}) 
			},
		};
		
		println!("{}\n{}\n{}", composed_url, host, addr);

		// mutate req with composed_url
		// "X-Forwared-For" could be added here
		*req.uri_mut() = composed_url;

    return Box::pin(async {
		  let version = req.version();
		 	let scheme = match req.uri().scheme() {
  			Some(a) => a.as_str(),
  			// dont serve if no scheme
  			_ => "http",
		  };

			match (version, scheme) {
				(hyper::Version::HTTP_2, "https") => {
					request_http2_tls_response(req, host, addr).await
				},
				(hyper::Version::HTTP_2, "http") => {
					request_http2_response(req, addr).await
				},
				(_, "https") => {
					request_http1_tls_response(req, &host, addr).await
				},
				_ => {
					request_http1_response(req, addr).await
				},
			}
    });
	}
}

fn http_code_response(
	code: &StatusCode,
	body_str: &'static str,
) -> Result<BoxedResponse, http::Error> {
	Response::builder()
		.status(code)
		.header(CONTENT_TYPE, HeaderValue::from_static(HTML))
		.body(Full::new(bytes::Bytes::from(body_str)).map_err(|e| match e {}).boxed())
}

// http, get host from host header
// http2, get host from req uri
// string vs host
fn get_uri_from_host_or_authority(
	req: &Request<Incoming>,
	addresses: &collections::HashMap::<String, (http::Uri, String, String)>,
) -> Option<String> {
	// http 2
	if req.version() == hyper::Version::HTTP_2 {
		let host = req.uri().host()?.to_string();
		return Some(host.to_string());
	}

	// else
  let host_str = match req.headers().get("host") {
  	Some(h) => {
  		match h.to_str() {
  			Ok(hst) => hst,
  			_ => return None,
  		}
  	},
  	_ => return None,
  };
  
	let uri = match http::Uri::try_from(host_str) {
		Ok(uri) => uri,
		_ => return None,
	};
	
	match uri.host() {
		Some(uri) => Some(uri.to_string()),
		_ => None,
	}
}

fn create_dest_uri(
	req: &Request<Incoming>,
	addresses: &collections::HashMap::<String, (http::Uri, String, String)>,
	uri: &str,
) -> Option<(http::Uri, String, String)> {
	let (dest_uri, host, addr) = match addresses.get(uri) {
		Some(dest_uri) => dest_uri,
		_ => return None,
	};
	
	let mut parts = dest_uri.clone().into_parts();
	parts.path_and_query = req.uri().path_and_query().cloned();

	match http::Uri::from_parts(parts) {
		Ok(uri) => Some((uri, host.clone(), addr.clone())),
		_ => None,
	}
}

// this should be an error
// this function shouldn't have to change
fn create_address(req: &Request<Incoming>) -> (String, String) {
	let host = match req.uri().host() {
		Some(h) => h.to_string(),
		// dont serve if no scheme?
		_ => "".to_string(),
  };

 	let scheme = match req.uri().scheme() {
		Some(a) => a.as_str(),
		// beware of defaults
		_ => "http",
  };
  let port = match req.uri().port_u16() {
  	Some(p) => p,
  	_ => match scheme {
			"https" => 443,
			_ => 80,
		},
	};

	let addr = host.clone() + ":" + &port.to_string();
  (host.to_string(), addr)
}

async fn create_tcp_stream(addr: &str) -> Option<TokioIo<TcpStream>> {
  match TcpStream::connect(&addr).await {
		Ok(client_stream) => Some(TokioIo::new(client_stream)),
		_ => None,
  }
}

async fn create_tls_stream(
	host: &str,
	addr: &str,
) -> Option<TokioIo<tokio_native_tls::TlsStream<TcpStream>>> {
  let tls_connector = match TlsConnector::new() {
		Ok(cx) => tokio_native_tls::TlsConnector::from(cx),
		_ => return None,
  };

  let client_stream = match TcpStream::connect(addr).await {
		Ok(s) => s,
		_ => return None,
  };
  
	let tls_stream = match tls_connector.connect(host, client_stream).await {
		Ok(s) => TokioIo::new(s),
		_ => return None,
  };

  Some(tls_stream)
}

async fn request_http1_response(
	req: Request<Incoming>,
	addr: String,
) -> Result<
	BoxedResponse,
	http::Error
> {

  let io = match create_tcp_stream(&addr).await {
		Some(stream) => stream,
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  let (mut sender, conn) = match http1::handshake(io).await {
		Ok(handshake) => handshake,
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  tokio::task::spawn(async move {
		if let Err(_err) = conn.await {
			/* log connection error */
		}
	});

  if let Ok(r) = sender.send_request(req).await {
		return Ok(r.map(|b| b.boxed()));
  };

	http_code_response(&StatusCode::BAD_GATEWAY, &BAD_GATEWAY)
}

async fn request_http1_tls_response(
	req: Request<Incoming>,
	host: &str,
	addr: String,
) -> Result<
	BoxedResponse,
	http::Error
> {
	
  let io = match create_tls_stream(host, &addr).await {
		Some(stream) => stream,
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  let (mut sender, conn) = match http1::handshake(io).await {
		Ok(handshake) => handshake,
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  tokio::task::spawn(async move {
		if let Err(_err) = conn.await {
			/* log connection error */
		}
	});

  if let Ok(r) = sender.send_request(req).await {
		return Ok(r.map(|b| b.boxed()));
  };

	http_code_response(&StatusCode::BAD_GATEWAY, &BAD_GATEWAY)
}

async fn request_http2_response(
	req: Request<Incoming>,
	addr: String,
) -> Result<
	BoxedResponse,
	http::Error
> {
  let io = match create_tcp_stream(&addr).await {
		Some(stream) => stream,
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  let (mut client, client_conn) = match http2::handshake(TokioExecutor::new(), io).await {
		Ok(handshake) => handshake,
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  tokio::task::spawn(async move {
		if let Err(_err) = client_conn.await {
			/* log connection error */
		}
	});

  if let Ok(res) = client.send_request(req).await {
		return Ok(res.map(|b| b.boxed()));
  };

	http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR)
}

async fn request_http2_tls_response(
	req: Request<Incoming>,
	host: String,
	addr: String,
) -> Result<
	BoxedResponse,
	http::Error
> {
  let io = match create_tls_stream(&host, &addr).await {
		Some(stream) => stream,
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  let (mut client, client_conn) = match http2::handshake(TokioExecutor::new(), io).await {
		Ok(handshake) => handshake,
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  tokio::task::spawn(async move {
		if let Err(_err) = client_conn.await {
			/* log connection error */
		}
	});

  if let Ok(res) = client.send_request(req).await {
		return Ok(res.map(|b| b.boxed()));
  };

	http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR)
}

