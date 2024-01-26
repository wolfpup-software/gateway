/*
    Don't expose internal errors.
*/
use http::Uri;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::{http1, http2};
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioExecutor;
use hyper_util::rt::TokioIo;
use native_tls::TlsConnector;
use tokio::net::TcpStream;

pub type BoxedResponse = Response<BoxBody<bytes::Bytes, hyper::Error>>;

const HTML: &str = "text/html; charset=utf-8";
const AUTHORITY_FROM_URI_ERROR: &str = "failed to retrieve URI from upstream URI";
const UPSTREAM_CONNECTION_ERROR: &str = "failed to establish connection to upstream server";
const UPSTREAM_HANDSHAKE_ERROR: &str = "upstream server handshake failed";
const UNABLE_TO_PROCESS_REQUEST_ERROR: &str = "unable to process request";

pub fn create_error_response(
    status_code: &StatusCode,
    body_str: &'static str,
) -> Result<BoxedResponse, http::Error> {
    Response::builder()
        .status(status_code)
        .header(CONTENT_TYPE, HeaderValue::from_static(HTML))
        .body(
            Full::new(bytes::Bytes::from(body_str))
                .map_err(|e| match e {})
                .boxed(),
        )
}

pub async fn request_http1_response(req: Request<Incoming>) -> Result<BoxedResponse, http::Error> {
    let (_, addr) = match get_host_and_authority(&req.uri()) {
        Some(stream) => stream,
        _ => return create_error_response(&StatusCode::BAD_REQUEST, &AUTHORITY_FROM_URI_ERROR),
    };

    let io = match create_tcp_stream(&addr).await {
        Some(stream) => stream,
        _ => return create_error_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_CONNECTION_ERROR),
    };

    let (mut sender, conn) = match http1::handshake(io).await {
        Ok(handshake) => handshake,
        _ => return create_error_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_HANDSHAKE_ERROR),
    };

    tokio::task::spawn(async move {
        if let Err(_err) = conn.await { /* log connection error */ }
    });

    if let Ok(r) = sender.send_request(req).await {
        return Ok(r.map(|b| b.boxed()));
    };

    create_error_response(&StatusCode::BAD_GATEWAY, &UNABLE_TO_PROCESS_REQUEST_ERROR)
}

pub async fn request_http1_tls_response(
    req: Request<Incoming>,
) -> Result<BoxedResponse, http::Error> {
    let (host, addr) = match get_host_and_authority(&req.uri()) {
        Some(stream) => stream,
        _ => return create_error_response(&StatusCode::BAD_REQUEST, &AUTHORITY_FROM_URI_ERROR),
    };

    let io = match create_tls_stream(&host, &addr).await {
        Some(stream) => stream,
        _ => return create_error_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_CONNECTION_ERROR),
    };

    let (mut sender, conn) = match http1::handshake(io).await {
        Ok(handshake) => handshake,
        _ => return create_error_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_HANDSHAKE_ERROR),
    };

    tokio::task::spawn(async move {
        if let Err(_err) = conn.await { /* log connection error */ }
    });

    if let Ok(r) = sender.send_request(req).await {
        return Ok(r.map(|b| b.boxed()));
    };

    create_error_response(&StatusCode::BAD_GATEWAY, &UNABLE_TO_PROCESS_REQUEST_ERROR)
}

pub async fn request_http2_response(req: Request<Incoming>) -> Result<BoxedResponse, http::Error> {
    let (_, addr) = match get_host_and_authority(&req.uri()) {
        Some(stream) => stream,
        _ => return create_error_response(&StatusCode::BAD_REQUEST, &AUTHORITY_FROM_URI_ERROR),
    };

    let io = match create_tcp_stream(&addr).await {
        Some(stream) => stream,
        _ => return create_error_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_CONNECTION_ERROR),
    };

    let (mut client, client_conn) = match http2::handshake(TokioExecutor::new(), io).await {
        Ok(handshake) => handshake,
        _ => return create_error_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_HANDSHAKE_ERROR),
    };

    tokio::task::spawn(async move {
        if let Err(_err) = client_conn.await { /* log connection error */ }
    });

    if let Ok(res) = client.send_request(req).await {
        return Ok(res.map(|b| b.boxed()));
    };

    create_error_response(&StatusCode::BAD_GATEWAY, &UNABLE_TO_PROCESS_REQUEST_ERROR)
}

pub async fn request_http2_tls_response(
    req: Request<Incoming>,
) -> Result<BoxedResponse, http::Error> {
    let (host, addr) = match get_host_and_authority(&req.uri()) {
        Some(stream) => stream,
        _ => return create_error_response(&StatusCode::BAD_REQUEST, &AUTHORITY_FROM_URI_ERROR),
    };

    let io = match create_tls_stream(&host, &addr).await {
        Some(stream) => stream,
        _ => return create_error_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_CONNECTION_ERROR),
    };

    let (mut client, client_conn) = match http2::handshake(TokioExecutor::new(), io).await {
        Ok(handshake) => handshake,
        _ => return create_error_response(&StatusCode::BAD_GATEWAY, &UPSTREAM_HANDSHAKE_ERROR),
    };

    tokio::task::spawn(async move {
        if let Err(_err) = client_conn.await { /* log connection error */ }
    });

    if let Ok(res) = client.send_request(req).await {
        return Ok(res.map(|b| b.boxed()));
    };
    // to here

    create_error_response(&StatusCode::BAD_GATEWAY, &UNABLE_TO_PROCESS_REQUEST_ERROR)
}

fn get_host_and_authority(uri: &Uri) -> Option<(&str, String)> {
    let host = match uri.host() {
        Some(h) => h,
        _ => return None,
    };

    let scheme = match uri.scheme() {
        Some(s) => s.as_str(),
        _ => http::uri::Scheme::HTTPS.as_str(),
    };

    let port = match (uri.port(), scheme) {
        (Some(p), _) => p.as_u16(),
        (None, "https") => 443,
        _ => 80,
    };

    let authority = host.to_string() + ":" + &port.to_string();

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

