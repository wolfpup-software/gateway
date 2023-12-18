# gateway

Traffic https requests to local services

## abstract

Decrypt https requests before deploying to local services. This avoids decrypting at every service point and avoids services being aware of certificates.

This server is meant to be the single point at 443 and 80.
