# Glass static client

This is the dependency-free, offline-first unsigned protocol console for
Dragon's Clutch. It is a static-hostable untrusted projection with no network,
wallet, signing, or submission capability. It is no longer limited to fixture
description: it can validate user-supplied protocol projections, reconcile
exact owner-level settlement/fee arithmetic, and export bytes for the protocol
contracts that own an exact wire.

> **Historical snapshot:** the bundled capability/evidence ledger predates the
> 2026-08-22 architecture review. Its offline fixtures and byte compiler remain
> testable, but its lifecycle-status prose is not current repository truth. See
> the root `CURRENT_TRUTH.md` and `docs/reviews/ARCHITECTURE_REVIEW_2026-08-22.md`.

- `manifest.json` preserves the historical evidence snapshot, profiles, and
  unpublished release identity.
- `protocol-contracts.js` is the present contract inventory. It names explicit
  localnet/devnet/testnet/mainnet-beta construction targets, the exact source
  revisions for General V2 owner settlement, fees, SourcePlane V3, liveness,
  Product/Series, and structured claims, and a reason for every disabled
  capability. These branch anchors are implementation provenance, not a joined
  release manifest.
- `terms.json` contains the canonical display fixture and its SHA-256 digest.
- `embedded-data.js` is a generated verbatim mirror of both files, so the page
  works from `file://` under a `default-src 'none'` policy that permits no
  network connection. `npm test` fails if the mirror drifts.
- `index.html` / `styles.css` provide the protocol control, projection,
  construction, and historical evidence surfaces.
- `protocol-client.js` owns the real-protocol UI. Its cluster/program/release
  configuration is local and ephemeral; even a complete form remains
  user-supplied and non-official. It accepts no private keys and never contacts
  the displayed endpoint.
- `app.js` retains the bundled historical evidence and terms-fixture inspector.
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

## Projection boundary

The protocol state importer accepts
`dragons-clutch.account-projection.v1`. A separate collector must supply the
cluster, observation slot, release/ProgramData binding, and one provenance row
per account containing its address, owner program, complete body SHA-256, slot,
and semantic kind. The browser checks the envelope and protocol relationships;
it does not authenticate any of those observations against a validator.

Recognized state surfaces are:

- owner-sorted General V2 settlement rows, including aggregate buy/sell price
  units, the single owner-level ceil/floor boundary, explicit seller zero-fee
  rows, reservation funding, and projected Position cash;
- the selected fee record, exact recipient conservation, and ordinary treasury
  Position custody;
- SourcePlane V3 release/parser/spec/generation and head/page/window/result
  lineage;
- all seven prepaid liveness compartments in canonical order, with principal,
  work, rent, and donation quantities kept distinct;
- the five Series funding components, ordinal/lapse phase, and principal versus
  donation balances;
- structured descriptor, Token-2022 supply, backing, surplus, and retirement
  visibility.

All exact quantities are decimal strings. Semantic 32-byte identities are
lowercase hex so owner row order is byte order rather than locale or base58
text order. Static clients and indexes remain untrusted projections of onchain
state.

## Unsigned construction boundary

The constructor emits three deliberately different products:

- Source/Series V2 actions emit the complete exact successor request envelope
  and action payload. The central executable capability set is still empty, so
  the output states that the runtime is expected to refuse it.
- `DCLINT01` liveness transitions emit the exact 272-byte inner contract. No
  outer action or account-meta table is claimed.
- structured-claim actions emit their exact 192-, 72-, or 48-byte family-local
  payload. The central registry has not allocated those local actions or the
  proposed `0x88/1` descriptor account, so no outer request is fabricated.

General V2 settlement/fees and SourcePlane V3 remain construction-disabled
until their central payload, account-meta, and runtime contracts are frozen
together. Every output has an absent signer, an empty signature list, and no
submission path.

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
