use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use http_body_util::{combinators::BoxBody, Full,  BodyExt};
use hyper::body::{Incoming};
use hyper::header::{CONTENT_TYPE, HeaderValue};
use hyper::{Response, Request, StatusCode};
use hyper::service::Service;
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use native_tls::{TlsConnector};
use tokio::net::TcpStream;

const HTML: &str = "text/html; charset=utf-8";
const INTERNAL_SERVER_ERROR: &str = "500 internal server error";
const BAD_GATEWAY: &str = "BAD_GATEWAY";

type BoxedResponse = Response<
	BoxBody<
		bytes::Bytes,
		hyper::Error,
	>
>;

fn http_code_response(
	code: &StatusCode,
	body_str: &'static str,
) -> Result<BoxedResponse, http::Error> {
	Response::builder()
		.status(code)
		.header(CONTENT_TYPE, HeaderValue::from_static(HTML))
		.body(Full::new(bytes::Bytes::from(body_str)).map_err(|e| match e {}).boxed())
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

impl Service<Request<Incoming>> for Svc {
	type Response = BoxedResponse;
	type Error = http::Error;
	type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
	
	fn call(&self, mut req: Request<Incoming>) -> Self::Future {
		println!("{:?}", req);

		// get uri
		let requested_uri = match req.headers().get("host") {
			Some(uri) => uri,
			_ => {
				return Box::pin(async {
					// bad request
					http_code_response(&StatusCode::BAD_REQUEST, &INTERNAL_SERVER_ERROR)
				})
			},
		};

		let dest_parts = match self.addresses.get(&requested_uri) {
			Some(dest_uri) => {
				let mut parts = dest_uri.clone().into_parts();
				parts.path_and_query = req.uri().path_and_query().cloned();
				parts
			},
			_ => {
				// bad gateway
				return Box::pin(async {
					http_code_response(&StatusCode::BAD_GATEWAY, &BAD_GATEWAY)
				})
			},
		};

		let composed_url = match http::Uri::from_parts(dest_parts) {
			Ok(uri) => uri,
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

fn create_host(req: &Request<Incoming>) -> String {
	req.uri().host().expect("uri has no host").to_string()
}

fn create_address(req: &Request<Incoming>) -> String {
 	let scheme = match req.uri().scheme() {
		Some(a) => a.as_str(),
		// dont serve if no scheme
		_ => "http",
  };
	let host = req.uri().host().expect("uri has no host");
  let port = match req.uri().port_u16() {
  	Some(p) => p,
  	_ => match scheme {
			"http" => 80,
			"https" => 443,
			_ => 80,
		},
	};
  format!("{}:{}", host, port)
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
		_ => return None,
  };
  
  Some(tls_stream)
}

async fn request_http1_response(
	req: Request<Incoming>,
) -> Result<
	BoxedResponse,
	http::Error
> {
	let addr = create_address(&req);
  let io = match create_tcp_stream(&addr).await {
		Some(stream) => stream,
		// unable to connect
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };
 
  let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
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
  let host = req.uri().host().expect("uri has no host");
	let addr = create_address(&req);
	// is scheme https?
	// is scheme https?

  let io = match create_tls_stream(host, &addr).await {
		Some(stream) => stream,
		// unable to connect
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };

  let (mut sender, conn) = match hyper::client::conn::http1::handshake(io).await {
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

	http_code_response(&StatusCode::BAD_GATEWAY, &BAD_GATEWAY)
}

async fn request_http2_response(req: Request<Incoming>) -> Result<
	BoxedResponse,
	http::Error
> {
	// is scheme https?
	let addr = create_address(&req);
  let io = match create_tcp_stream(&addr).await {
		Some(stream) => stream,
		// unable to connect
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };
  
  // this is for http2 connectiopns
  let (mut client, client_conn) = match hyper::client::conn::http2::handshake(TokioExecutor::new(), io).await {
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
  let host = req.uri().host().expect("uri has no host");
	let addr = create_address(&req);
  let io = match create_tls_stream(host, &addr).await {
		Some(stream) => stream,
		// unable to connect
		_ => return http_code_response(&StatusCode::INTERNAL_SERVER_ERROR, &INTERNAL_SERVER_ERROR),
  };
  
  // this is for http2 connectiopns
  let (mut client, client_conn) = match hyper::client::conn::http2::handshake(TokioExecutor::new(), io).await {
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

