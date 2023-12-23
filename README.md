# gateway

route encrypted requests for multiple domains to multiple endpoints

## abstract

The requirement was to route external requests to local services through SSL / TLS / https.
The result can serve multiple domains on a single server.

Destination servers can be local, remote, or encrypted. But the intention was to
route external requests to local services

A gateway encapsulates a server through a chosen port 443 by default.
Requests are routed to a server based on the URI authority or host header



