# reverse_proxy

Route requests to local or upstream servers.

## About

A reverse proxy written in rust using [tokio](https://tokio.rs/) and
[hyper](https://hyper.rs/).

`reverse_proxy` forwards incoming encrypted http1 and http2 requests to upstream servers.

## How to use

### Install reverse_proxy

Execute the following to install `reverse_proxy`.

```sh
git clone https://github.com/wolfpup_software/reverse_proxy
cargo install --path reverse_proxy/reverse_proxy
```

### Create a JSON config

A JSON configuration file is required to run `reverse_proxy`.

#### JSON Schema

All filepaths can be absolute or relative to the config filepath.

```JSON
{
  "host_and_port": "<string>",
  "key_filepath": "<string>",
  "cert_filepath": "<string>",
  "addresses": [
    ["<origin_address>", "<target_address>"]
  ],
  "dangerous_self_signed_addresses": [
    ["<origin_address>", "<target_address>"]
  ]
}
```

A valid configuration example can be found at [`./reverse_proxy.example.json`](./reverse_proxy.example.json`)

#### Properties

| name | definition |
|----------|------------|
| `host_and_port` | the address of the server (ie: 0.0.0.0:3000) |
| `key_filepath` | the filepath of a key from a TLS certificate |
| `cert_filepath` |  the filepath of a cert from a TLS certificate | 
| `addresses` | A key value map of URIs used to route incoming requests to upstream servers. Only the `host` and `port` of a URI will be used for routing requests. |
| `dangerous_self_signed_addresses` (optional)  | allows `reverse_proxy` to make requests to servers with self-signed TLS certificates |

#### Allow self-signed certificates

ONLY USE the `dangerous_self_signed_addresses` property WITH EXTREME CAUTION.

Ideally never.

This optional property is intended to forward requests to servers using self-signed TLS certificates on local networks.

### Run reverse_proxy

```sh
reverse_proxy ./path/to/config.json
```

Open a browser and visit `https://localhost:XXXX`.

## Licence

`reverse_proxy` is released under the BSD 3-Clause License
