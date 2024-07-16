use http::uri::InvalidUriParts;
use hyper::body::Incoming;
use hyper::service::Service;
use hyper::{Request, StatusCode};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::requests;

use config;

const URI_FROM_REQUEST_ERROR: &str = "failed to find upstream URI from request";
const UPSTREAM_URI_ERROR: &str = "falied to update request with upstream URI";

pub struct Svc {
    pub addresses: Arc<HashMap<String, (http::Uri, bool)>>,
}

impl Service<Request<Incoming>> for Svc {
    type Response = requests::BoxedResponse;
    type Error = http::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&self, mut req: Request<Incoming>) -> Self::Future {
        // check if cycle header is available
        // return if present

        let req_uri = match get_host_from_request(&req) {
            Some(uri) => uri,
            _ => {
                return Box::pin(async {
                    // bad request
                    requests::create_error_response(
                        &StatusCode::BAD_REQUEST,
                        &URI_FROM_REQUEST_ERROR,
                    )
                });
            }
        };

        // get target host from requested host
        let (target_uri, is_dangerous) = match self.addresses.get(&req_uri) {
            Some((trgt_uri, is_dngrs)) => (trgt_uri.clone(), is_dngrs.clone()),
            _ => {
                return Box::pin(async {
                    // bad request
                    requests::create_error_response(&StatusCode::NOT_FOUND, &URI_FROM_REQUEST_ERROR)
                });
            }
        };

        if let Err(_) = update_request_with_dest_uri(&mut req, target_uri) {
            return Box::pin(async {
                requests::create_error_response(
                    &StatusCode::INTERNAL_SERVER_ERROR,
                    &UPSTREAM_URI_ERROR,
                )
            });
        };

        // add cycle detection
        // set header wolfpup-cycle-detect

        return Box::pin(async move { requests::get_response(req, is_dangerous).await });
    }
}

// use the same function that creates a hashmap key "config::get_host_and_port"
fn get_host_from_request(req: &Request<Incoming>) -> Option<String> {
    // http 2
    if let Some(s) = config::get_host_and_port(req.uri()) {
        return Some(s);
    };

    // http 1.1
    let host_header = match req.headers().get("host") {
        Some(h) => h,
        _ => return None,
    };

    let host_str = match host_header.to_str() {
        Ok(h_str) => h_str,
        _ => return None,
    };

    let uri = match http::Uri::try_from(host_str) {
        Ok(u) => u,
        _ => return None,
    };

    config::get_host_and_port(&uri)
}

// possibly more efficient to manipulate strings
fn update_request_with_dest_uri(
    req: &mut Request<Incoming>,
    uri: http::Uri,
) -> Result<(), InvalidUriParts> {
    let mut dest_parts = uri.into_parts();
    dest_parts.path_and_query = req.uri().path_and_query().cloned();

    *req.uri_mut() = match http::Uri::from_parts(dest_parts) {
        Ok(u) => u,
        Err(e) => return Err(e),
    };

    Ok(())
}
