# Glass chain console

Glass is the dependency-free, static-hostable read-only console for Dragon's
Clutch. It has no configured network, program, release, wallet, signer, or
transaction submission path at startup. A user selects only an operatord and
browser read bounds. The daemon projects one offline-composed, genesis-bound
checked release; the browser cannot supply those chain facts.

The application has three narrow jobs:

1. read the fork-aware untrusted account index exposed by `operatord`;
2. bind and display exact Product compiler proposals emitted by Rust; and
3. validate and inspect a blockhash-free Solana transaction already constructed
   from exact chain-derived state by its Rust semantic owner.

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
/v1/session
/v1/actions
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

The config must name the current
`v3-general-no-keeper-no-selected-candidate` canonical decoder set.
Withdrawn Source V1/V2, historical General account versions, and raw historical
Dealer V1 bodies are not live fallbacks: a selected release containing those
bytes fails closed instead of being reinterpreted or displayed as current
state. SelectedCandidate V1 is not a live browser mapping, and current General
accounts emit no keeper action until a capability-admitted planner exists.
Current Dealer accounts must carry their exact central tag/version and
strict eight-byte global envelope. Current Failure MarketRoot V2 and
interval-consensus work/replay accounts are also decoded through their exact
semantic-owner codecs. The Dealer upload-stage allocation is explicitly
non-production and is not part of live discovery.

When a checked release explicitly selects the `fractional` family, Glass
recognizes only the current physical-Resolution-account/Resolution-data-bound
Policy V3, Ledger V1, Credit V2, and Tombstone V2 layouts. Withdrawn Policy V1,
unprefundable Policy V2, and credit/tombstone V1 layouts are not fallback DTOs,
and their presence does not create redemption capability.

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

For current Source and Structured actions, Glass admits inspection only when
the `/v1/actions` verdict joins the checked manifest/profile, the exact enabled
coordinate, one onchain-derived finalized restart cursor, its complete named
driver/dependency observations, and a still-live exclusive validity slot.
Source material must be a legacy `source-plane-v3` transaction without lookup
tables. Structured material must be a v0 `structured-claim` transaction with
one finalized, digest-bound address lookup table. Its verdict names the
disjoint wrapper execution release and base scheduling/driver release; Glass
checks the wrapper Program/deployment/ELF/manifest release key instead of
pretending the indexed base release owns wrapper instruction bytes. Missing finalized state,
changed driver slots, stale dependencies, expired material, unsupported family
contracts, and release-enabled coordinates without one exact canonical draft
remain visible as refused dispositions; none can be selected. The browser has
no manual payload, account-role, semantic-owner, or transaction assembly input
on this path.

Dealer retirement variants are a separate release surface. Glass recognizes
only the closed `76/1/25/8` active-facility-credit and `76/1/25/9`
unused-future-credit variants, requires the coarse `76/1/25` tuple to remain
absent, and joins the exact discriminator set across acquisition, release,
session, and action projections. Target 8 is selectable only with its exact
48-role finalized observation set; target 9 requires its distinct exact
45-role set. Both require the frozen role names/privileges, one v0 lookup
table, the target-specific semantic owner and runtime admission, exact signer
roles and integer equations, the state-v3 driver observation, and an unexpired
freshness boundary. The liveness-receipt creation target is the sole required
finalized absence. Targets 1–7, a coarse action-25 tuple, incomplete role
frames, or stale observations remain refused.

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
- exact current RegistryProgramReleaseV2, RegistryCapabilityProfileV4,
  QuoteV5, AttachmentV5, and remaining BundleV6 input bodies;
- an optional bounded exact-market search over explicit integer coordinates;
  and
- an explicit expected compiler-release SHA-256 pinned by operatord.

The form calls the pure bounded
`POST /v2/compiler/product-exact-market` endpoint. The same implementation is
available through
`operatord compile-product-exact-market --compiler-release-sha256 HASH` for
stdin/stdout proposal import. Both call Rust `compile_production_payoff_v1`,
the current `assemble_compiled_product_series_bundle_v6`, and, when requested,
the bounded all-support exact atom solver.

The page computes SHA-256 over both canonical sorted-key UTF-8 definition JSON
and the complete validated request, and requires the proposal to bind both plus
the configured expected compiler-release SHA-256. That release hash is a
configuration join, not a measurement of the running binary. The Product
program address comes from the acquired checked-release projection; operatord
requires it to be canonical, nonzero, and equal to the Program coordinate in
RegistryProgramReleaseV2 before deriving the kind-63 BundleV6 artifact PDA.

Glass then displays exact-in-span versus certified-approximation status, all
exact rational error bounds, the canonical 2,352-byte native-basis proposal,
its certificate, and the 528-byte BundleV6 plus all sixteen typed identities.
The bundle capability-profile ID must match the daemon-projected checked
release. An analytic result also carries its exact certification subdivision
depth. An exact-market request additionally returns a canonical work manifest,
an optional verifier certificate, and a BundleV6-bound sidecar. It never claims
a unique price, fair value, or optimal clearing.

The compiler endpoint is loopback-only and has no RPC, wallet, signing,
submission, registration, or persistence path. Its output is always marked
`untrusted-compiler-proposal` with `registrationAuthority: false`. Glass checks
transport shape and exact request joins but deliberately does not become a
second semantic owner by reimplementing Rust codecs. Registration remains the
only authority: the program must reopen the loader-authenticated registry
release, ProfileV4, Source release, every canonical artifact, BundleV6, and any
exact-market evidence, then recompute every identity, PDA, and binding.

## Canonical transaction-material boundary

The browser has no successor transaction builder and accepts no caller-shaped
payload, account-meta, capability, or accounting DTO. It acquires `/v1/actions`
inside the same finalized session bracket as the account projection. Every
verdict must name one exact coordinate enabled by the checked release. A
control remains unavailable until operatord has rerun the semantic owner's
constructor from reacquired onchain bytes and supplied a complete ordered role
projection, explicit signer public identities, balanced exact-integer
equations, and one deterministic zero-blockhash/zero-signature transaction.

Adding a fresh recent blockhash and choosing an authorized fee payer are a
separate wallet-launcher boundary. That launcher must reacquire all named
prestate first. After submission it must discard the draft and reacquire both
`/v1/session` and `/v1/actions`; neither Glass nor operatord advances from an
expected poststate.

## Files

- `chain-client.js`: operatord-only target, canonical `/v1/session`
  attach/restart contract, `/v1/actions` capability/draft contract, and bounded
  projection transport.
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
