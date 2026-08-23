# Serving the Glass chain console

Glass remains plain HTML, CSS, and JavaScript. It can be copied to any static
host, but chain reads require the explicitly selected operatord endpoint to be
reachable under the page's Content Security Policy and browser origin rules.

The easiest local/live topology is same-origin serving:

```text
browser  GET /index.html, /*.js, /*.css
browser  GET /v1/*
                 │
                 └── operatord --static apps/static-client
```

This avoids inventing a second API proxy. If assets and operatord use different
origins, operatord or a reviewed reverse proxy must provide narrow CORS headers
for the exact static origin. The browser always uses `credentials: omit`; do
not solve CORS by enabling credentials or a wildcard origin. The current Rust
operatord implementation does not itself emit CORS headers.

## Meta policy

`index.html` includes:

```text
default-src 'none'; base-uri 'none'; object-src 'none';
connect-src 'self' https: http://127.0.0.1:* http://localhost:*;
form-action 'none'; style-src 'self'; script-src 'self';
img-src 'self' data:;
```

This allows only same-origin reads, explicit HTTPS endpoints, and explicit
loopback HTTP. Application validation is stricter: plaintext operatord/RPC/WS
URLs are accepted only for `127.0.0.1` or `localhost`; public validator and
operator URLs require HTTPS/WSS. A public HTTP page also cannot call HTTPS/WSS
incorrectly configured by the daemon, and an HTTPS page cannot call loopback
HTTP in every browser policy. Use a secure same-origin reverse proxy for a
public deployment.

`script-src` and `style-src` admit only local files. There are no CDN assets,
analytics, wallet adapters, inline event handlers, service workers, or dynamic
script imports.

## Response-header protections

A meta CSP silently ignores `frame-ancestors`, `sandbox`, reporting directives,
and all non-CSP response headers. A host that controls headers should add:

```text
Content-Security-Policy: default-src 'none'; base-uri 'none'; object-src 'none'; connect-src 'self' https: http://127.0.0.1:* http://localhost:*; form-action 'none'; frame-ancestors 'none'; style-src 'self'; script-src 'self'; img-src 'self' data:; require-trusted-types-for 'script'
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Permissions-Policy: accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=()
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
Strict-Transport-Security: max-age=63072000; includeSubDomains
Cache-Control: no-cache
```

Do not add `Cross-Origin-Embedder-Policy: require-corp` unless every selected
cross-origin operatord deployment is deliberately compatible with it.

## Static-host notes

- GitHub Pages cannot configure the response headers above and is therefore a
  convenience asset mirror, never the integrity root. Its HTTPS origin can
  read a different HTTPS operatord only when the daemon/proxy explicitly admits
  that Pages origin through CORS.
- `file://` can render the page but is not the supported chain-attached mode.
  Origin/CORS rules vary and Web Crypto may be unavailable; compiler and local
  projection digest joins fail closed when SHA-256 cannot be computed.
- An IPFS gateway is also only an asset transport. Shared-origin and header
  behavior vary by gateway; the CID does not authenticate a program release.
- Local operation should prefer operatord's static-file option or a narrow
  same-origin reverse proxy. A generic file server can render the UI, but it
  does not supply `/v1/*`.

No serving topology makes the projection authoritative. A checked release
manifest and onchain account authentication remain separate requirements, and
the page still has no wallet, signing, blockhash, or submission path.
