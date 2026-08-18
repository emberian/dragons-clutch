# Glass static client

This is the dependency-free, offline-first client skeleton for Dragon's Clutch.
It is static-hostable and intentionally stops at local inspection:

- `manifest.json` names clusters, programs, profiles, and unpublished release identity.
- `terms.json` contains the canonical display fixture and its SHA-256 digest.
- `index.html` / `styles.css` provide the accessible inspection surface.
- `app.js` builds unsigned JSON intent previews only; it has no RPC, wallet,
  serializer, signer, or submit path.

Run the local checks without installing anything:

```sh
npm test
npm run check
```

Open `index.html` directly or serve this directory with any static file server.
The page never requires that server for protocol behavior. See
[`docs/implementation/STATIC_CLIENT.md`](../../docs/implementation/STATIC_CLIENT.md)
for the trust boundary and promotion gates.
