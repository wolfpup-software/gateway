# Gateway

Route requests to local or upstream servers.

## Abstract

A gateway / reverse-proxy written in rust using [tokio](https://tokio.rs/) and
[hyper](https://hyper.rs/).

## About

`Gateway` fowwards incoming encrypted http1 and http2 requests to upstream servers.

Upstream TLS / SSL requests must have valid TLS certificates.

## How to use

### Create a config

A JSON configuration file is required to run `gateway`.

Configuration schema:

```
{
  "host": <string>,
  "port": <number>,
  "key_filepath": <string>,
  "cert_filepath": <string>,
  "addresses": {
    <string>: <string>,
  }
}
```

The `host` and `port` properties define the address of the server.

The `key_filepath` and `cert_filepath` properties define the filepath of the
TLS certificate needed to establish TLS connections. They can be relative
to the location of the config.

The `addresses` property defines a key value map of URIs to route incoming
requests to upstream servers. Only the `authority` of a URI will be used
for routing requests.

A valid configuration example can be found at
`gateway/gateway.example.json`

### Install gateway

Execute the following to install `gateway`.

```
git clone https://github.com/herebythere/gateway
cargo install --path gateway/gateway
```

### Run gateway

The `gateway` application accepts one argument from the command line:

- A valid `gateway` JSON configuration file

```
gateway <path_to_configuration_file>
```

Execute the following to generate a self-signed certificate and run `gateway`.

```
bash gateway/generate_tls.sh
gateway gateway/gateway.example.json
```

Open a browser and visit `https://localhost:4000`.

## Licence

BSD 3-Clause License
