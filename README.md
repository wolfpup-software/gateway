# Gateway

Route requests to local or upstream servers.

## About

A reverse-proxy written in rust using [tokio](https://tokio.rs/) and
[hyper](https://hyper.rs/).

`Gateway` forwards incoming encrypted http1 and http2 requests to upstream servers.

## How to use

### Install gateway

Execute the following to install `gateway`.

```sh
git clone https://github.com/herebythere/gateway
cargo install --path gateway/gateway
```

### Create a JSON config

A JSON configuration file is required to run `gateway`.

Configuration schema:

```JSON
{
  "host_and_port": "<string>",
  "key_filepath": "<string>",
  "cert_filepath": "<string>",
  "addresses": [
    ["<origin_address>", "<target_address>"]
  ]
}
```

The `host_and_port` property defines the address of the server.

The `key_filepath` and `cert_filepath` properties define the filepath of the
TLS certificate needed to establish TLS connections. Filepaths can be absolute or relative
to the config filepath.

The `addresses` property defines a key value map of URIs to route incoming
requests to upstream servers. Only the `host` and `port` of a URI will be used
for routing requests.

A valid configuration example can be found at `gateway/gateway.example.json`

#### Allow self-signed certificates

The `dangerous_self_signed_addresses` allows `gateway` to make requests to servers with self-signed TLS certificates. It's intended for self-signed TLS certificates on local networks.

Add the following property to a `config`
```JSON
{
  "dangerous_self_signed_addresses": [
    ["<origin_address>", "<target_address>"]
  ]
}
```

ONLY USE `dangerous_self_signed_addresses` WITH EXTREME CAUTION, ideally never.

### Run gateway

The `gateway` application accepts one argument from the command line:

- A valid `gateway` JSON configuration file

```sh
gateway "<path_to_configuration_file>"
```

Open a browser and visit `https://localhost:4000`.

## Licence

`Gateway` is released under the BSD 3-Clause License
