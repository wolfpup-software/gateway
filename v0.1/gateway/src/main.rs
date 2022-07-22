
use std::env;
use std::path;
use std::net;
use hyper::server::conn::{AddrIncoming, AddrStream};
use hyper::{Server, Body, Request, Method, Response, StatusCode};
use hyper::service::{make_service_fn, service_fn};
use std::io;

use config;

mod tls;

#[tokio::main]
async fn main() {
    let args = match env::args().nth(1) {
        Some(a) => path::PathBuf::from(a),
        None => return println!("argument error: no config params were found."),
    };

    let config = match config::Config::from_filepath(&args) {
        Ok(c) => c,
        Err(e) => return println!("configuration error: {}", e),
    };
    println!("{:?}", config);


    let host = match config.host.parse() {
        Ok(h) => h,
        _ => return println!("configuration error: unable to parse host."),
    };

    let addr = net::SocketAddr::new(host, config.port);

    let tls_config = match tls::create_tls_config(
        &config.cert,
        &config.key,
    ) {
        Ok(h) => h,
        Err(e) => return println!("configuration error: {}", e),
    };

    let incoming = match AddrIncoming::bind(&addr) {
        Ok(h) => h,
        _ => return println!("configuration error: to bind address."),
    };
    

    let service = make_service_fn(|_| async { Ok::<_, io::Error>(service_fn(echo)) });
    let server = Server::builder(tls::TlsAcceptor::new(tls_config, incoming));

    // run server
    if let Err(e) = server.serve(service).await {
        println!("server error: {}", e);
    }
}

// Custom echo service, handling two different routes and a
// catch-all 404 responder.
async fn echo(req: Request<Body>) -> Result<Response<Body>, hyper::Error> {
    let mut response = Response::new(Body::empty());
    match (req.method(), req.uri().path()) {
        // Help route.
        (&Method::GET, "/") => {
            *response.body_mut() = Body::from("howdy!");
        }
        // Catch-all 404.
        _ => {
            *response.body_mut() = Body::from("howdy!");
        }
    };

    Ok(response)
}

// The closure inside `make_service_fn` is run for each connection,
// creating a 'service' to handle requests for that specific connection.
// let make_service = make_service_fn(move |_| {
//     let client = client_main.clone();

//     async move {
//         // This is the `Service` that will handle the connection.
//         // `service_fn` is a helper to convert a function that
//         // returns a Response into a `Service`.
//         Ok::<_, Error>(service_fn(move |mut req| {
//             let uri_string = format!(
//                 "http://{}{}",
//                 out_addr_clone,
//                 req.uri()
//                     .path_and_query()
//                     .map(|x| x.as_str())
//                     .unwrap_or("/")
//             );
//             let uri = uri_string.parse().unwrap();
//             *req.uri_mut() = uri;
//             client.request(req)
//         }))
//     }
// });

// let mut request = Request::builder();
// request.uri("https://www.rust-lang.org/")
//        .header("User-Agent", "my-awesome-agent/1.0");
