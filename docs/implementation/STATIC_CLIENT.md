# Glass static-client implementation

This note describes the small offline artifact in `apps/static-client/`. It is
an implementation companion to [`docs/STATIC_CLIENT.md`](../STATIC_CLIENT.md),
not a deployment or program-release manifest.

## What exists

The client is plain HTML, CSS, and browser JavaScript with no package runtime
dependencies. It is suitable for a static host (including GitHub Pages or an
IPFS gateway) and can also be opened from a local checkout. `manifest.json` and
`terms.json` are intentionally human-readable release inputs. The page presents:

- explicit cluster entries, program entries, and collateral-profile entries;
- a release identity panel that marks source, bundle, and IPFS identity as
  unpublished until a checked release binds them;
- an immutable terms fixture whose `canonicalTerms` object is hashed with
  sorted-key, compact UTF-8 JSON and SHA-256;
- a local intent inspector that emits a deterministic JSON description only;
- provenance, static-client, and unavailable-chain warnings on every relevant
  surface.

The current terms digest is:

```text
sha256:a21f6cbb1ab3b06afc7c8625f3388835843edb17c48173e8fb57df8b7e0dd8e8
```

That digest covers only the canonical terms object, not its explanatory warning
or surrounding JSON metadata. Any future terms change must update the digest
and the checked release manifest together.

## Deliberate boundaries

The initial artifact does not import a wallet adapter, call an RPC endpoint,
perform account discovery, load an index, serialize Solana wire bytes, sign,
submit, or run background work. There is no startup effect other than rendering
local DOM. The intent object includes `wallet: not-connected`, `signature: null`,
and `submission: disabled` so an inspection cannot be mistaken for an executable
transaction.

The selected cluster and program are labels from an explicit manifest. Every
current cluster is `unavailable`, and the program has no ID or ELF digest. The
synthetic profile is a shape-only fixture. The DREGG entry is marked
`reference-only-unchecked`; it does not assert a deployment, mint account, or
mainnet program. These states must remain visible until a separately checked
release manifest exists.

The small browser fallback digest is FNV-1a only for environments without Web
Crypto (not release evidence). On a secure static host, Web Crypto computes the
displayed SHA-256. Independent verification should use `terms.json` and the
canonicalization rule above, not trust the page.

## Local checks

Node is the only tool required for the offline checks; no install step is needed:

```sh
cd apps/static-client
npm test
npm run check
```

`npm test` parses both manifests, recomputes the terms digest, checks all chain
capabilities are disabled, and rejects obvious wallet/RPC/sign/submit symbols
in the browser bundle. `npm run check` performs the host JavaScript syntax check.

To preview the page, serve this directory with any static file server or open
`index.html` directly. A server is only a convenience for browser restrictions;
the page itself has no server dependency.

## Promotion gates

This prototype is not ready for a signing surface. A later promotion must add,
at minimum, a reproducible build and lock digest, a complete asset inventory and
SBOM, a checked deployment manifest binding an exact SBF ELF hash to a program
ID and cluster, schema/account-layout digests, independent transaction-byte
fixtures, CSP verification, and adversarial tests for malformed untrusted
inputs. Wallet/RPC integration requires a new explicitly reviewed trust-boundary
implementation; it must not be smuggled into this offline artifact.
