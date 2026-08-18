# Glass static client

This is the dependency-free, offline-first client skeleton for Dragon's Clutch.
It is an untrusted projection: static-hostable, inspect-only, and with no chain
capability at all.

- `manifest.json` names clusters, programs, profiles, and unpublished release
  identity.
- `terms.json` contains the canonical display fixture and its SHA-256 digest.
- `embedded-data.js` is a generated verbatim mirror of both files, so the page
  works from `file://` under a `default-src 'none'` policy that permits no
  network connection. `npm test` fails if the mirror drifts.
- `index.html` / `styles.css` provide the inspection surface.
- `app.js` builds unsigned JSON intent previews only. It holds no copy of the
  release data and no hard-coded digest, and it has no RPC, wallet, serializer,
  signer, or submit path.
- [`SERVING.md`](SERVING.md) states which protections require serve-time HTTP
  headers, with an example header set.

Run the local checks without installing anything:

```sh
npm test        # named offline gates, including the embedded-data equality gate
npm run check   # host JavaScript syntax check
npm run embed   # regenerate embedded-data.js after editing manifest/terms JSON
```

Editing `manifest.json` or `terms.json` means running `npm run embed`; editing
`canonicalTerms` also means recomputing the digest in both `terms.json` and
`manifest.json`. `npm test` refuses every one of those omissions.

Open `index.html` directly or serve this directory with any static file server.
The page never requires that server for protocol behavior, but a plain `file://`
open has no Web Crypto, so the digest is displayed as declared and labeled as
not recomputed. See [`SERVING.md`](SERVING.md) for the difference a host makes
and [`docs/implementation/STATIC_CLIENT.md`](../../docs/implementation/STATIC_CLIENT.md)
for the trust boundary and promotion gates.
