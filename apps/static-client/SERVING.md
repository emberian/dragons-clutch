# Serving the Glass static client

The page is a complete artifact on its own: open `index.html` from a checkout,
or copy the directory to any static host or IPFS gateway. Nothing here needs a
server for protocol behavior.

What a host *does* change is the set of browser protections available. Some
Content-Security-Policy directives are only honored when they arrive as an HTTP
response header, and a `<meta>` policy cannot substitute for them. This note
says exactly which ones, so no document has to imply that the shipped HTML
provides a protection it cannot.

## What the meta policy in `index.html` does enforce

```text
default-src 'none'; base-uri 'none'; object-src 'none'; connect-src 'none';
form-action 'none'; style-src 'self'; script-src 'self'; img-src 'self' data:;
```

- `default-src 'none'` plus the explicit `connect-src 'none'` means the page can
  open no network connection at all — no XHR, no `EventSource`, no WebSocket, no
  beacon. This is also why the reviewed `manifest.json` and `terms.json` are
  mirrored into `embedded-data.js` rather than loaded at runtime: a page that
  could load them could also talk to something else, and it would break on
  `file://` anyway. The mirror is held equal to the reviewed files by the test
  `embedded_static_data_equals_reviewed_manifest_and_terms`.
- `script-src 'self'` and `style-src 'self'` admit only the files in this
  directory: no CDN, analytics, tag manager, or inline script.
- `object-src 'none'` and `base-uri 'none'` remove plugin embedding and
  `<base>`-tag redirection of relative URLs.
- `form-action 'none'` prevents the inspector form from navigating anywhere,
  including if a script fault skipped `preventDefault`.

## What only response headers can enforce

A `<meta http-equiv="Content-Security-Policy">` policy **silently ignores**
`frame-ancestors`, `sandbox`, `report-to`, and `report-uri`. A meta tag also
applies only after parsing begins, so it cannot govern the response itself.
These therefore require host configuration:

| Protection | Directive / header | Why meta cannot do it |
| --- | --- | --- |
| Clickjacking / framing | `frame-ancestors`, `X-Frame-Options` | Ignored in meta; the decision belongs to the response |
| MIME-type sniffing | `X-Content-Type-Options: nosniff` | Header-only |
| Referrer leakage | `Referrer-Policy` | Header-only (a meta `referrer` covers less) |
| Transport pinning | `Strict-Transport-Security` | Header-only |
| Cross-origin isolation | `Cross-Origin-Opener-Policy`, `Cross-Origin-Resource-Policy` | Header-only |
| Feature access | `Permissions-Policy` | Header-only |
| Violation reporting | `report-to` / `report-uri` | Ignored in meta |

## Example header set for a static host

```text
Content-Security-Policy: default-src 'none'; base-uri 'none'; object-src 'none'; connect-src 'none'; form-action 'none'; frame-ancestors 'none'; style-src 'self'; script-src 'self'; img-src 'self' data:; require-trusted-types-for 'script'
X-Frame-Options: DENY
X-Content-Type-Options: nosniff
Referrer-Policy: no-referrer
Permissions-Policy: accelerometer=(), camera=(), geolocation=(), gyroscope=(), microphone=(), payment=(), usb=(), interest-cohort=()
Cross-Origin-Opener-Policy: same-origin
Cross-Origin-Resource-Policy: same-origin
Cross-Origin-Embedder-Policy: require-corp
Strict-Transport-Security: max-age=63072000; includeSubDomains; preload
Cache-Control: no-cache
```

The header CSP is a superset of the meta policy: keep both, so a host that drops
headers still loses only the header-only protections rather than all of them.

## Host notes

- **GitHub Pages** serves no custom response headers. A Pages mirror therefore
  provides *none* of the header-only protections above, including
  `frame-ancestors`. Documents must not describe a Pages mirror as supplying
  them, and a Pages mirror is a convenience mirror, never the integrity root.
- **IPFS gateways** vary by operator and are usually shared-origin; assume no
  header-only protection unless the specific gateway is checked. The CID, not
  the gateway, is the identity.
- **`file://`** has no origin server at all, and Web Crypto is unavailable in
  that context, so the page shows the declared terms digest and states plainly
  that it was not recomputed there. Serve over `http(s)://localhost` (or any
  secure origin) to see the digest recomputed and compared locally.
- Any host that can set headers (Netlify `_headers`, Cloudflare, nginx,
  Caddy, S3+CloudFront function) should ship the block above verbatim.

None of this makes the page authoritative. It stays an untrusted projection
with no network, wallet, signing, or submission capability. Its local
constructor can emit exact unsigned bytes only for named contract boundaries;
that is not chain authentication or execution. The integrity root is the
reviewed source and its digests, never the server that handed you the bytes.
