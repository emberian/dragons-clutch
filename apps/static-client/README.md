# Glass chain console

Glass is the dependency-free, static-hostable read-only console for Dragon's
Clutch. It has no configured network, program, release, wallet, signer, or
transaction submission path at startup. A user selects only an operatord and
browser read bounds. The daemon projects one offline-composed, genesis-bound
checked release; the browser cannot supply those chain facts.

The application has three narrow jobs:

1. read the fork-aware untrusted account index exposed by `operatord`;
2. bind and display exact Product compiler proposals emitted by Rust; and
3. assemble the outer blockhash-free Solana transaction around exact bytes and
   account roles supplied by their semantic owner.

It is not an explorer, index authority, compiler implementation, wallet, or
release manifest.

## Explicit operator target

The form requires and embeds no more than:

- operatord base URL;
- processed or finalized commitment;
- browser account, response-byte, timeout, and slot-lag bounds.

Cluster/genesis identity, credential-redacted endpoint bindings, the current
decoder set, Program/ProgramData/slot/ELF tuple, canonical capability-manifest
digest, measured source commit, capability-profile identity, decoder families,
and centrally enabled intent triples arrive only in `/v1/acquisition`. Glass
requires exactly one release and rechecks it against `/v1/releases`.

The validator URLs are configuration bindings only. Browser code contacts the
selected operatord URL and only with sequential `GET` requests to:

```text
/v1/health
/v1/acquisition
/v1/releases
/v1/accounts?commitment={processed|finalized}
/v1/keeper/next?commitment={processed|finalized}
/v1/forks
```

Response bodies are byte-budgeted while streaming and then shape-bounded.
Account quantities remain canonical decimal strings. The release join requires
exact program, ProgramData, deployment-slot, ELF, manifest, profile, source,
enabled-intent, and release-key equality.
Rows from other releases are counted and ignored rather than blended.

`operatord chain-serve --config FILE` is the live owner of these routes. Before
binding HTTP it checks `getGenesisHash`, hostile-decodes each selected
Program/ProgramData loader pair, checks the decoded deployment slot, and hashes
the observed ProgramData ELF. It then repeatedly admits bounded finalized
`getProgramAccounts` responses through `RpcIndexEngine`. One bounded ordered
WebSocket owner admits the complete program, block, slot-update, and root
subscription set. It keeps processed reads withdrawn across registration,
release-bracketed scan, replay, disconnect, rollback, and capped reconnect
backoff. Before subscribing, the exact WebSocket connection must answer
`getGenesisHash` with the selected genesis. The acquisition response publishes
credential-safe redacted/SHA-256 bindings for the exact daemon HTTP+WebSocket
coordinates plus the checked release. The browser never receives the raw RPC
URLs and refuses an inconsistent or changing daemon projection. See
[`CHAIN_SERVE.md`](CHAIN_SERVE.md).

The config must name the current `source-v3-current` canonical decoder set.
Withdrawn Source V1/V2, historical General account versions, and raw historical
Dealer V1 bodies are not live fallbacks: a selected release containing those
bytes fails closed instead of being reinterpreted or displayed as current
state. Current Dealer accounts must carry their exact central tag/version and
strict eight-byte global envelope. Current Failure MarketRoot V2 and
interval-consensus work/replay accounts are also decoded through their exact
semantic-owner codecs. The Dealer upload-stage allocation is explicitly
non-production and is not part of live discovery.

When a checked release explicitly selects the `fractional` family, Glass
recognizes only the current Resolution-data-bound Policy V2, Ledger V1, Credit
V2, and Tombstone V2 layouts. Withdrawn fractional V1 layouts are not fallback
DTOs, and their presence does not create redemption capability.

## Projection semantics

The console groups chain-derived account rows into Market, Product, collateral
and liabilities, Source, Series, candidate/clearing, settlement/position,
covered-liquidity, recovery, and unknown-release surfaces. It preserves account body digests, generation
and binding IDs, observed/effective commitment, slot/lag, and one of these fork
states:

- `finalized-scan`;
- `processed-frozen` or `processed-unfrozen`;
- `dead-fork`; or
- `unidentified-fork`.

No absence statement is global: an empty family means only that the current
bounded selected-release projection contained no row. All displayed state and
keeper cursors are untrusted projections. The program must reload and
authenticate complete accounts.

Processed views are additionally non-final and rollbackable. Dead branches and
transport reconnects are explicit withdrawal events, processed keeper actions
are empty, and the page refuses to construct workflows from processed state.
Well-formed closures and non-executable owner changes become release-specific,
fork-bound removals instead of forcing a reconnect; malformed or ambiguous
changes still fail closed. No processed projection or removal is
authority-eligible.

Successor coordinates are not mirrored into the browser. A semantic-owner
draft must state its exact family tag/version/action bytes, which remain
untrusted construction material. Projected keeper cursors are non-selectable
until operatord exposes a release-authenticated coordinate. A decoded family
is not executable capability admission. The daemon reports the offline
manifest/profile/source/ELF join and its enabled registry coordinates, but that
report remains an untrusted projection and is not a current-account runtime
admission verdict.

Product/Series registration and Owner/Position V3 lifecycle appear as separate
`not-authenticated` capability cards. This avoids treating Product compiler
proposals, Position/Replay codecs, or owner-settlement rows as runtime
admission. Dealer likewise stays visible as indexed liquidity state while its
successor actions remain independently disabled.

## Product compiler boundary

JavaScript performs no payout, spline, approximation, or Product/Series bundle
math. The compiler form accepts:

- an exact rational definition JSON object in which all integers and rational
  components are strings;
- exact canonical fixed-codec Product/Series bundle inputs; and
- an explicit expected compiler-release SHA-256 pinned by operatord.

The form calls the pure bounded `POST /v1/compiler/production-payoff` endpoint.
The same implementation is available through `operatord compile-payoff` for
stdin/stdout proposal import. Both call Rust `compile_production_payoff_v1` and
`assemble_compiled_product_series_bundle_v1`.

The page computes SHA-256 over both canonical sorted-key UTF-8 definition JSON
and the complete validated request, and requires the proposal to bind both plus
the configured expected compiler-release SHA-256. That release hash is a
configuration join, not a measurement of the running binary. It
then displays exact-in-span versus certified-approximation status, all exact
rational error bounds, the canonical 2,352-byte native-basis proposal, its
  certificate, and the 528-byte bundle plus all sixteen typed identities. The
bundle capability-profile ID must match the daemon-projected checked release. An
analytic result also carries its exact certification subdivision depth.

The compiler endpoint is loopback-only and has no RPC, wallet, signing,
submission, registration, or persistence path. Registration remains authority
and must reopen the registry, Source release, and every canonical artifact and
recompute their joins.

## Unsigned construction boundary

`successor-builder.js` is the browser counterpart to the Rust outer
`ProtocolTransactionBuilder`. It accepts one to sixteen explicit instructions.
Each draft names its flow, successor family and action, semantic-owner package,
schema and release digest, lowercase payload bytes, ordered account roles,
required signer public keys, and at least one balanced exact `u128` equation.
Equation units use the Rust categories rather than free text: lamports;
collateral, fee, or wrapper atoms bound to a mint; price units bound to a
positive scale; or Egg atoms bound to a Market identity and outcome.

The builder validates those declarations, adds the three-byte successor
envelope, compiles the canonical legacy Solana message key order, installs an
all-zero recent blockhash, and serializes one zero signature per required
message signer. It emits hex and base64 bytes with an explicit packet-size
limit. It cannot decide semantic payloads, infer account metas, enable a route,
obtain a recent blockhash, sign, or submit.

Projected keeper cursors remain deliberately non-selectable because the read
API does not yet expose a release-authenticated successor coordinate. Exact
family tag/version/action bytes come from the semantic-owner draft and the
output marks them as construction material, never runtime capability. Every
workflow node states that authoritative accounts must be reloaded.

## Files

- `chain-client.js`: operatord-only target and bounded projection transport.
- `successor-builder.js`: exact outer message and unsigned transaction bytes.
- `compiler-proposal.js`: bounded pure-compiler transport and exact proposal
  validation; no compiler math.
- [`COMPILER_TRANSPORT.md`](COMPILER_TRANSPORT.md): exact Rust adapter JSON
  contract and canonicalization rule.
- [`CHAIN_SERVE.md`](CHAIN_SERVE.md): explicit bounded HTTP/WebSocket acquisition,
  processed rollback/reconnect, and finalized projection boundary.
- `app.js`, `index.html`, `styles.css`: DOM presentation.
- [`SERVING.md`](SERVING.md): same-origin/CORS and response-header guidance.

There are no runtime dependencies or asset build step. The repository keeps
local mechanical checks under `test/`, but this implementation was deliberately
not run against a browser, validator, RPC, wallet, or test command as part of
the implementation-only swarm cycle.
