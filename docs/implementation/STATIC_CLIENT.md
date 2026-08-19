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
- a separate native B-spline inspection SDK that projects canonical Terms,
  checks compiler-artifact structure/digests, and emits exact typed artifact
  upload plus CreateMarket instruction-data bytes from a Rust-generated golden;
- provenance, static-client, and unavailable-chain warnings on every relevant
  surface.

The current terms digest is:

```text
sha256:62b06b2107636686648507e4f9ecd8a4d90733dcebf81177d4a63b25bc698d02
```

That digest covers only the canonical terms object, not its explanatory warning
or surrounding JSON metadata. Any future terms change must update the digest
and the checked release manifest together. It supersedes the earlier
`sha256:a21f6cbb…d8e8` fixture, whose rounding string is corrected below.

## One copy of the release data

The page has to work from `file://` under a `default-src 'none'` policy that
permits no network connection, so the reviewed JSON cannot be loaded at runtime:
a page able to load it would also be able to talk to something else. The data is
therefore mirrored once into `embedded-data.js`, generated from the reviewed
files by `npm run embed`, and the mirror is held equal to them by the named test
`embedded_static_data_equals_reviewed_manifest_and_terms`. `app.js` restates no
digest, note, mint, or binding of its own; a re-introduced literal fails
`app_holds_no_second_copy_of_release_data_or_digest`.

At load the page also cross-checks that `manifest.json` and `terms.json` declare
the same terms digest and, where Web Crypto exists, recomputes the digest from
the canonical terms it is displaying. Any mismatch is shown and the intent
composer refuses to build, rather than presenting a binding the page cannot
stand behind. Composition stays closed until that check settles.

## Terms semantics

The fixture describes the landed kernel, which refuses rather than rounds:

```text
rounding:   exact-integer-payout-or-refuse-on-remainder
redemption: per-outcome-exact-or-refuse-plus-complete-set-exit
```

`crates/clutch-kernel` `redeem` returns `Error::RemainderRequired` when
`quantity * weight % denominator != 0`; it never floors, and
`redeem_complete_set` is the always-exact exit. The previous fixture string
`exact-scaled-integer-floor-at-final-payout-boundary` promised a flooring
boundary that no candidate in
[`POLICY_ANALYSIS_LOTS_FEES.md`](POLICY_ANALYSIS_LOTS_FEES.md) §1 validates and
that `docs/PROTOCOL.md` prohibits; §P1-A of the adversarial review records the
contradiction. The new strings describe only landed behavior and freeze no
divisibility policy: the choice between one-hot-only, redemption lots, and
remainder credits is still open, and `terms.json` says so.

`terms_rounding_matches_kernel_refuse_on_remainder_semantics` fails on any
reintroduced floor/truncate/round language.

## Serving and CSP honesty

The meta CSP in `index.html` carries only directives a `<meta>` policy can
actually enforce: `default-src 'none'`, an explicit `connect-src 'none'`,
`base-uri`, `object-src`, `form-action`, `style-src`, `script-src`, `img-src`.
`frame-ancestors`, `sandbox`, and `report-to`/`report-uri` are silently ignored
in a meta policy and were removed; the page must not appear to promise
clickjacking protection that only a response header can give.
`apps/static-client/SERVING.md` states which protections are header-only, gives
an example header set for a static host, and records that GitHub Pages serves no
custom headers and therefore supplies none of them. The gate is
`meta_csp_carries_only_directives_a_meta_policy_can_enforce` plus
`serving_note_states_the_header_only_protections`.

## Deliberate boundaries

The artifact does not import a wallet adapter, call an RPC endpoint, perform
account discovery, load an index, build account metas or a Solana message,
sign, submit, or run background work. The native module serializes only the
program's canonical unsigned intent data: BeginArtifact, nine WriteArtifact
chunks, SealArtifact, and CreateMarket. It does not turn those bytes into a
transaction. There is no startup effect other than rendering local DOM. The
intent objects include `wallet: not-connected`, `signature: null`, and
`submission: disabled` so an inspection cannot be mistaken for execution.

The JavaScript certificate check is structural and digest-only. Exact
compiler re-verification remains Rust-only, and the certificate is not bound
by current on-chain Terms. See
[`NATIVE_BSPLINE_CLIENT_SCHEMA_V1.md`](NATIVE_BSPLINE_CLIENT_SCHEMA_V1.md).

The selected cluster and program are labels from an explicit manifest. Every
current cluster is `unavailable`, and the program has no ID or ELF digest. The
synthetic profile is a shape-only fixture. The DREGG entry is marked
`reference-only-unchecked`; it does not assert a deployment, mint account, or
mainnet program. These states must remain visible until a separately checked
release manifest exists.

There is no browser fallback digest. Web Crypto is unavailable in an insecure
context (including a plain `file://` open), and a cheap non-cryptographic
checksum displayed under a SHA-256 label reads as verification while providing
none, so the page shows the declared digest and says plainly that it was not
recomputed there. Independent verification should use `terms.json` and the
canonicalization rule above, not trust the page.

## Local checks

Node is the only tool required for the offline checks; no install step is needed:

```sh
cd apps/static-client
npm test
npm run check
```

`npm test` runs named gates: manifest capability disablement, terms-digest
recomputation, kernel-consistent rounding language, embedded-mirror equality,
absence of a second data/digest copy in `app.js`, wallet/RPC/sign/submit symbol
rejection across both browser scripts, meta-CSP honesty, the serving note,
asset presence, local-only asset references, and element-id coverage.
`npm run check` performs the host JavaScript syntax check on every shipped
script. `npm run embed` regenerates `embedded-data.js` after a manifest or terms
edit; it is a repair convenience, never a serve-time build step.

## Promotion gates

This prototype is not ready for a signing surface. A later promotion must add,
at minimum, a reproducible build and lock digest, a complete asset inventory and
SBOM, a checked deployment manifest binding an exact SBF ELF hash to a program
ID and cluster, schema/account-layout digests, independent transaction-byte
fixtures, serve-time header verification against `SERVING.md`, and adversarial
tests for malformed untrusted inputs. Wallet/RPC integration requires a new
explicitly reviewed trust-boundary implementation; it must not be smuggled into
this offline artifact.
