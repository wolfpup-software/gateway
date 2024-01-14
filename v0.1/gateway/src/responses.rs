/*
    Relay req to upstream server

    - find host from request
    - use host to get copy of upstream URI from address map
    - replace the path_and_query of the upstream URI with the path_and_query of request URI
    - request URI is replaced by the the destinataion URI
    - updated request is relayed to the upstream server

    Errors can stem from both the current server and the upstream server.
    This server returns HTTP 502 for all failed request local to this code.
    Response body is a semi-informative error. Don't expose internals.
*/

use std::collections;
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::{http1, http2};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use native_tls::TlsConnector;
use tokio::net::TcpStream;

const HTTP: &str = "http";
const HTTPS: &str = "https";
const HTML: &str = "text/html; charset=utf-8";
const HOST: &str = "host";

const AUTHORITY_FROM_URI_ERROR: &str = "could not retrieve URI from upstream URI";
const URI_FROM_REQUEST_ERROR: &str = "could not retrieve URI from request";
const UPSTREAM_URI_ERROR: &str = "could create a upstream URI from request";
const UPSTREAM_CONNECTION_ERROR: &str = "could not establish connection to upstream server";
const UPSTREAM_HANDSHAKE_ERROR: &str = "upstream server handshake failed";
const UNABLE_TO_PROCESS_REQUEST_ERROR: &str = "unable to process request";

type BoxedResponse = Response<BoxBody<bytes::Bytes, hyper::Error>>;

pub struct Svc {
    pub addresses: Arc<HashMap<String, http::Uri>>,
}

impl Service<Request<Incoming>> for Svc {
    type Response = BoxedResponse;
    type Error = http::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, mut req: Request<Incoming>) -> Self::Future {
        // http1 and http2 headers
        let requested_uri = match get_uri_from_host_or_authority(&req) {
            Some(uri) => uri,
            _ => {
                return Box::pin(async {
                    // bad request
                    http_code_response(&StatusCode::BAD_GATEWAY, &URI_FROM_REQUEST_ERROR)
                });
            }
        };

        let composed_url = match create_dest_uri(&req, &self.addresses, &requested_uri) {
            Some(uri) => uri,
            _ => {
                return Box::pin(async {
                    http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_URI_ERROR)
                })
            }
        };
        // mutate req with composed_url
        // "X-Forwared-For" could be added here (insecure)
        *req.uri_mut() = composed_url;

        return Box::pin(async {
            let version = req.version();
            let scheme = match req.uri().scheme() {
                Some(a) => a.as_str(),
                // dont serve if no scheme
                _ => HTTP,
            };

            match (version, scheme) {
                (hyper::Version::HTTP_2, HTTPS) => request_http2_tls_response(req).await,
                (hyper::Version::HTTP_2, HTTP) => request_http2_response(req).await,
                (_, HTTPS) => request_http1_tls_response(req).await,
                _ => request_http1_response(req).await,
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
        .body(
            Full::new(bytes::Bytes::from(body_str))
                .map_err(|e| match e {})
                .boxed(),
        )
}

fn get_uri_from_host_or_authority(req: &Request<Incoming>) -> Option<String> {
    // http2
    if req.version() == hyper::Version::HTTP_2 {
        return match req.uri().host() {
            Some(s) => Some(s.to_string()),
            _ => None,
        };
    }

    // http1.1
    let host_str = match req.headers().get(HOST) {
        Some(h) => match h.to_str() {
            Ok(hst) => hst,
            _ => return None,
        },
        _ => return None,
    };

    let uri = match http::Uri::try_from(host_str) {
        Ok(u) => u,
        _ => return None,
    };

    match uri.host() {
        Some(host) => Some(host.to_string()),
        _ => None,
    }
}

fn create_dest_uri(
    req: &Request<Incoming>,
    addresses: &collections::HashMap<String, http::Uri>,
    uri: &str,
) -> Option<http::Uri> {
    let mut dest_parts = match addresses.get(uri) {
        Some(dest_uri) => dest_uri.clone().into_parts(),
        _ => return None,
    };
    dest_parts.path_and_query = req.uri().path_and_query().cloned();

    match http::Uri::from_parts(dest_parts) {
        Ok(uri) => Some(uri),
        _ => None,
    }
}

// this should be an error, or return an option
// these should both exists otherwise no request
fn create_address(req: &Request<Incoming>) -> Option<(&str, &str)> {
    let host = match req.uri().host() {
        Some(h) => h,
        _ => return None,
    };

    let authority = match req.uri().authority() {
        Some(a) => a.as_str(),
        _ => return None,
    };

    // if authority does not have port
    // add one based on scheme

    Some((host, authority))
}

async fn create_tcp_stream(addr: &str) -> Option<TokioIo<TcpStream>> {
    match TcpStream::connect(&addr).await {
        Ok(client_stream) => Some(TokioIo::new(client_stream)),
        _ => None,
    }
}

// this has multiple "types" of errors
// signal that it is an inappropriate grouping?
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

async fn request_http1_response(req: Request<Incoming>) -> Result<BoxedResponse, http::Error> {
    let (_, addr) = match create_address(&req) {
        Some(stream) => stream,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &AUTHORITY_FROM_URI_ERROR),
    };

    let io = match create_tcp_stream(&addr).await {
        Some(stream) => stream,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_CONNECTION_ERROR),
    };

    let (mut sender, conn) = match http1::handshake(io).await {
        Ok(handshake) => handshake,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_HANDSHAKE_ERROR),
    };

    tokio::task::spawn(async move {
        if let Err(_err) = conn.await { /* log connection error */ }
    });

    if let Ok(r) = sender.send_request(req).await {
        return Ok(r.map(|b| b.boxed()));
    };

    http_code_response(&StatusCode::BAD_GATEWAY, &UNABLE_TO_PROCESS_REQUEST_ERROR)
}

async fn request_http1_tls_response(req: Request<Incoming>) -> Result<BoxedResponse, http::Error> {
    let (host, addr) = match create_address(&req) {
        Some(stream) => stream,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &AUTHORITY_FROM_URI_ERROR),
    };

    let io = match create_tls_stream(&host, &addr).await {
        Some(stream) => stream,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_CONNECTION_ERROR),
    };

    let (mut sender, conn) = match http1::handshake(io).await {
        Ok(handshake) => handshake,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_HANDSHAKE_ERROR),
    };

    tokio::task::spawn(async move {
        if let Err(_err) = conn.await { /* log connection error */ }
    });

    if let Ok(r) = sender.send_request(req).await {
        return Ok(r.map(|b| b.boxed()));
    };

    http_code_response(&StatusCode::BAD_GATEWAY, &UNABLE_TO_PROCESS_REQUEST_ERROR)
}

async fn request_http2_response(req: Request<Incoming>) -> Result<BoxedResponse, http::Error> {
    let (_, addr) = match create_address(&req) {
        Some(stream) => stream,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &AUTHORITY_FROM_URI_ERROR),
    };

    let io = match create_tcp_stream(&addr).await {
        Some(stream) => stream,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_CONNECTION_ERROR),
    };

    let (mut client, client_conn) = match http2::handshake(TokioExecutor::new(), io).await {
        Ok(handshake) => handshake,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_HANDSHAKE_ERROR),
    };

    tokio::task::spawn(async move {
        if let Err(_err) = client_conn.await { /* log connection error */ }
    });

    if let Ok(res) = client.send_request(req).await {
        return Ok(res.map(|b| b.boxed()));
    };

    http_code_response(&StatusCode::BAD_GATEWAY, &UNABLE_TO_PROCESS_REQUEST_ERROR)
}

async fn request_http2_tls_response(req: Request<Incoming>) -> Result<BoxedResponse, http::Error> {
    let (host, addr) = match create_address(&req) {
        Some(stream) => stream,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &AUTHORITY_FROM_URI_ERROR),
    };

    let io = match create_tls_stream(&host, &addr).await {
        Some(stream) => stream,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_CONNECTION_ERROR),
    };

    let (mut client, client_conn) = match http2::handshake(TokioExecutor::new(), io).await {
        Ok(handshake) => handshake,
        _ => return http_code_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_HANDSHAKE_ERROR),
    };

    tokio::task::spawn(async move {
        if let Err(_err) = client_conn.await { /* log connection error */ }
    });

    if let Ok(res) = client.send_request(req).await {
        return Ok(res.map(|b| b.boxed()));
    };

    http_code_response(&StatusCode::BAD_GATEWAY, &UNABLE_TO_PROCESS_REQUEST_ERROR)
}
