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
- `native-bspline-v1.js` is a dependency-free offline inspection SDK for the
  native degree-0 through degree-3 basis. It consumes the Rust-generated
  compiler fixture, projects canonical Terms bytes, structurally checks a
  shape certificate, and emits exactly 11 Terms-upload intent-data strings
  (one BeginArtifact, nine WriteArtifact, one SealArtifact) plus a separate
  CreateMarket intent-data string. It still has no account-meta/message
  builder, wallet, RPC, signer, or submit path. The analytic certificate
  remains offline evidence and is not committed by current Terms.
- `native-bspline-market-creation-v1.schema.json` describes the unsigned JSON
  preview. Digests cover the documented binary codecs, not this JSON object.
- [`SERVING.md`](SERVING.md) states which protections require serve-time HTTP
  headers, with an example header set.

Run the local checks without installing anything:

```sh
npm test        # named offline gates, including Rust-fixture byte equality
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
