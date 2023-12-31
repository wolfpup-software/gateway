# gateway

Route requests to local or upstream servers.

## abstract

A reverse-proxy written in rust using [tokio](https://tokio.rs/) and
[hyper](https://hyper.rs/).

## Create a config

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

Change the `host` property to serve from a specific host.

Change the `port` property to serve from a different port.

Change the `directory` property to target an alternative directory. The `directory` property can be an absolute or relative path. A relative path is relative to the location of the JSON configuration file.

A valid configuration example can be found at
`gateway/v0.1/gateway.example.json`

## Install gateway

Execute the following to install `gateway`.

```
git clone https://github.com/herebythere/gateway
cargo install --path gateway/v0.1/gateway
```

## Run gateway

The `gateway` application accepts one argument from the command line:

- A valid `gateway` JSON configuration file

```
gateway <path_to_configuration_file>
```

Execute the following to host the `./demo` directory using `gateway`.

```
gateway gateway/v0.1/gateway.example.json
```

Open a browser and visit `http://localhost:<config.port>`.

## Licence

BSD 3-Clause License
