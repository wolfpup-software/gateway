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
	
	 
	$Potential Caveat	
	In dev, the 'uri' of the request was only a path and query.
	I forget if that's true in production for most other libraries.
*/

pub struct Svc {
	pub addresses: Arc<HashMap<http::header::HeaderValue, http::Uri>>,
}

impl Service<Request<Incoming>> for Svc {
	type Response = BoxedResponse;
	type Error = http::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

	fn call(&self, mut req: Request<Incoming>) -> Self::Future {
		// acount for http1 and http2 headers
		let requested_uri = match get_uri_from_host_or_authority(&req) {
			Some(uri) => uri,
			_ => {
				return Box::pin(async {
					// bad request
					http_code_response(&StatusCode::BAD_REQUEST, &INTERNAL_SERVER_ERROR)
				})
			},
		};

		let composed_url = match create_dest_uri(&req, &self.addresses, requested_uri) {
			Some(uri) => uri,
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

    return Box::pin(async {
    	// this could be a separate function
    	// forwarded port
		  let version = req.version();
		 	let scheme = match req.uri().scheme() {
  			Some(a) => a.as_str(),
  			// dont serve if no scheme
  			_ => "http",
		  };

		  // serve response based on (http version, scheme)
			match (version, scheme) {
				(hyper::Version::HTTP_2, "https") => {
					request_http2_tls_response(req).await
				},
				(hyper::Version::HTTP_2, "http") => {
					request_http2_response(req).await
				},
				(_, "https") => {
					request_http1_tls_response(req).await
				},
				_ => {
					request_http1_response(req).await
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

fn get_uri_from_host_or_authority(req: &Request<Incoming>) -> Option<&http::header::HeaderValue> {
  let reference_header = match req.version() {
  	hyper::Version::HTTP_2 => ":authority",
  	_ => "host",
  };
  
	req.headers().get(reference_header)
}

fn create_dest_uri(
	req: &Request<Incoming>,
	addresses: &collections::HashMap::<http::header::HeaderValue, http::Uri>,
	uri: &http::header::HeaderValue,
) -> Option<http::Uri> {
	let dest_parts = match addresses.get(uri) {
		Some(dest_uri) => {
			let mut parts = dest_uri.clone().into_parts();
			parts.path_and_query = req.uri().path_and_query().cloned();
			parts
		},
		_ => return None,
	};

	match http::Uri::from_parts(dest_parts) {
		Ok(uri) => Some(uri),
		_ => None,
	}
}

fn create_address(req: &Request<Incoming>) -> (String, String) {
	let host = match req.uri().host() {
		Some(h) => h.to_string(),
		// dont serve if no scheme
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
		// unable to connect
		_ => None,
  }
}

async fn create_tls_stream(host: &str, addr: &str) -> Option<TokioIo<tokio_native_tls::TlsStream<TcpStream>>> {
  let tls_connector = match TlsConnector::new() {
		Ok(cx) => tokio_native_tls::TlsConnector::from(cx),
		// unable to connect
		_ => return None,
  };

  let client_stream = match TcpStream::connect(addr).await {
		Ok(s) => s,
		// unable to connect
		_ => return None,
  };
  
	let tls_stream = match tls_connector.connect(host, client_stream).await {
		Ok(s) => TokioIo::new(s),
		// unable to connect
		Err(_e) => return None,
  };

  Some(tls_stream)
}

async fn request_http1_response(
	req: Request<Incoming>,
) -> Result<
	BoxedResponse,
	http::Error
> {
	let (_, addr) = create_address(&req);

  let io = match create_tcp_stream(&addr).await {
		Some(stream) => stream,
		// unable to connect
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  let (mut sender, conn) = match http1::handshake(io).await {
		Ok(handshake) => handshake,
		// unable to handshake
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  tokio::task::spawn(async move {
		if let Err(_err) = conn.await {
			/* log connection error */
		}
	});

	// successful request
  if let Ok(r) = sender.send_request(req).await {
		return Ok(r.map(|b| b.boxed()));
  };

  // default to error response
	http_code_response(&StatusCode::BAD_GATEWAY, &BAD_GATEWAY)
}

async fn request_http1_tls_response(req: Request<Incoming>) -> Result<
	BoxedResponse,
	http::Error
> {
	println!("making http1 tls request");
	let (host, addr) = create_address(&req);
	
  let io = match create_tls_stream(&host, &addr).await {
		Some(stream) => stream,
		// unable to connect
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };
	println!("connected");
  let (mut sender, conn) = match http1::handshake(io).await {
		Ok(handshake) => handshake,
		// unable to handshake
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };
	println!("handshake!");
  tokio::task::spawn(async move {
		if let Err(_err) = conn.await {
			/* log connection error */
		}
	});

	// successful request
  if let Ok(r) = sender.send_request(req).await {
		return Ok(r.map(|b| b.boxed()));
  };

	http_code_response(&StatusCode::BAD_GATEWAY, &BAD_GATEWAY)
}

async fn request_http2_response(req: Request<Incoming>) -> Result<
	BoxedResponse,
	http::Error
> {
	// is scheme https?
	let (_, addr) = create_address(&req);
	
  let io = match create_tcp_stream(&addr).await {
		Some(stream) => stream,
		// unable to connect
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  // this is for http2 connectiopns
  let (mut client, client_conn) = match http2::handshake(TokioExecutor::new(), io).await {
		Ok(handshake) => handshake,
		// unable to handshake
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
) -> Result<
	BoxedResponse,
	http::Error
> {
	// is scheme https?
	let (host, addr) = create_address(&req);
  let io = match create_tls_stream(&host, &addr).await {
		Some(stream) => stream,
		// unable to connect
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  let (mut client, client_conn) = match http2::handshake(TokioExecutor::new(), io).await {
		Ok(handshake) => handshake,
		// unable to handshake
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

