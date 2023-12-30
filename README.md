# gateway

route encrypted requests for multiple domains to multiple endpoints

## abstract

The requirement was to route external requests to local services through SSL / TLS / https.
The result can serve multiple domains on a single server.

Destination servers can be local, remote, or encrypted. But the intention was to
route external requests to local services

A gateway encapsulates a server through a chosen port 443 by default.
Requests are routed to a server based on the URI authority or host header


This server was made to provide the following:
- a single server to route all 443 requests to other local services
- it is not meant to relay requests to external resources (although you can do that)
- route http2 and http1 requests through a reverse proxy

