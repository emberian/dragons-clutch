# Glass static-client implementation

Glass is now a chain-attached read-only console, not the earlier embedded
fixture site. Its shipped bundle contains no `manifest.json`, `terms.json`,
successor-registry mirror, or default chain/release truth.

## Data path

```text
explicit user selection
        │
        ├── cluster/genesis + validator acquisition URLs + decoder set
        ├── program/ProgramData/deployment slot/ELF
        ├── manifest/source/capability-profile declarations
        └── commitment + response/account/time/staleness bounds
        │
        ▼
bounded sequential GETs to operatord /v1/*
        │
        ├── release-coordinate equality
        ├── selected-release filtering
        ├── canonical decimal integer transport
        └── fork/finality/staleness annotation
        │
        ▼
untrusted Market/Product/collateral/Source/Series/candidate/
settlement/liquidity/recovery projection
```

The browser never contacts the configured validator RPC or WebSocket URL. It
hash-binds their exact selected values to credential-redacted endpoint
identities echoed by operatord. Operatord authenticates the expected genesis on
HTTP before serving and again on each WebSocket generation before subscribing.
Release rows authenticate program, ProgramData, deployment slot, ELF hash, and
derived release key against explicit selection. Manifest hash, source commit,
and capability profile remain declared-only and are displayed as such.

Reads are `GET` only, sequential, credential-free, no-store, redirect-refusing,
timeout-bounded, response-byte-budgeted while streaming, and shape-bounded
after JSON parsing. The current endpoint contract is documented in
[`apps/static-client/README.md`](../../apps/static-client/README.md).

## Projection rules

All 64/128-bit quantities crossing JSON remain canonical decimal strings and
are compared with `BigInt`; application snapshots serialize the original
strings. Every account keeps its observation slot, commitment, body digest,
decode status, branch identity, generation, and semantic bindings.

Processed rows are joined to the fork graph and labeled frozen, unfrozen,
dead, or unidentified. Finalized-scan rows are kept distinct. Staleness is the
configured maximum lag from the greatest observed projection/fork/root slot,
not a claim about wall-clock freshness. Empty state groups mean only “absent
from this bounded selected-release result.”

Family decoding and runtime capability are separate. No JavaScript registry
mirror exists. The browser requires the daemon to echo the exact current
Source-V3/current-account decoder-set identity, then shows decoded families as
state only. Every action remains `not-authenticated` until a release-bound
runtime capability verdict exists.

The live Dealer decoder recognizes only the globally tagged current graph,
including State V2, funded dependencies V2, LP pages V2, Lease/Pot/Epoch V2,
terminal work, tombstone, exit-ticket, and action-receipt accounts. Historical
raw V1 semantic bodies and the explicitly non-production upload stage are not
treated as live persisted chain accounts.

The Failure projection decodes the full shared-Market admission state within
MarketRoot V2 and the exact current interval-consensus work/replay layouts.
Recognizing an outer tag or reserved allocation is not enough to display either
as chain state.

The optional `fractional` release family is likewise account-owned: it decodes
only the current Resolution-data-bound Policy V2, Ledger V1, Credit V2, and
Tombstone V2 codecs. Withdrawn V1 reinterpretations are never projected, and
the browser does not infer redemption capability from the decoded accounts.

## Product compiler proposal

The canonical math owner is Rust
`research/bspline-shape-compiler::production::compile_production_payoff_v1`.
JavaScript does not duplicate its rational integerization, spline compilation,
certification, payout, or bundle assembly.

The page accepts an exact rational definition and an external proposal. It
canonicalizes only the JSON transport and cryptographically binds the resulting
UTF-8 SHA-256 to the proposal. The proposal must include:

- an explicit compiler release SHA-256 and Product Terms ID;
- exact-categorical, exact-smooth, or analytic-smooth classification;
- exact-in-span or certified-approximation status under the Rust rules;
- the exact 2,352-byte `NativeClaimBasisV1` bytes and ID;
- the recompilable smooth certificate bytes/ID where applicable;
- all eight exact rational `ErrorCertificate` bounds for analytic output; and
- the exact 528-byte `CompiledProductSeriesBundleV1`, its ID, and its exact
  sixteen typed identity fields.

The bundle capability-profile ID must equal the selected release declaration.
This check is useful but remains non-authoritative: registration must reopen and
authenticate the Product registry, Source release, and canonical bodies and
recompute every ID/join. The repository does not yet expose the JSON proposal
serializer through operatord or CLI; `compiler-proposal.js` freezes the browser
consumption seam for that small Rust adapter. The complete interchange contract
is in
[`apps/static-client/COMPILER_TRANSPORT.md`](../../apps/static-client/COMPILER_TRANSPORT.md).

## Unsigned transaction construction

Semantic crates own every payload codec and ordered account role. The browser
accepts their exact construction material and implements only the outer legacy
Solana transaction boundary used by Rust `ProtocolTransactionBuilder`:

- validates canonical 32-byte base58 addresses and distinct account roles;
- validates semantic-owner package/schema/release digest;
- requires exact successor family/action/flow ownership;
- requires at least one balanced exact `u128` equation per instruction;
- prepends the three-byte successor envelope;
- derives canonical payer/signer/writable/read-only key ordering;
- compiles the legacy message with an all-zero recent blockhash;
- emits one 64-byte zero signature per required message signer; and
- refuses output above an explicit packet byte limit.

The output schema is
`dragons-clutch/operator/unsigned-protocol-transaction/v4`, wrapped by a
resumable workflow node that preserves selected release and observation
coordinates. Exact family tag/version/action bytes must come from the semantic
owner and are labeled construction material, not a browser capability verdict.
Projected keeper rows remain non-selectable until operatord exposes an
authenticated coordinate.

No browser input can set admission true. There is no blockhash acquisition,
wallet access, signature request, simulation, or submission.

## Remaining live seams

Before this can become a live trading launcher, separate reviewed components
must provide:

1. operatord release-manifest/source/capability-profile authentication and a
   release-bound admission response;
2. a semantic-owner adapter that supplies exact transaction drafts and a
   release-authenticated capability verdict (the pure Rust payoff compiler
   endpoint/CLI already emits compiler and Product/Series bundle proposals);
3. a wallet launcher that reloads authoritative accounts, simulates exact
   effects, acquires a recent blockhash, presents signer roles, signs, and
   submits; and
4. an independently checked deployment/release manifest for any public URL.

Keeping these seams absent is intentional. A static client and index are
untrusted projections, and construction bytes are not evidence of executable
capability or successful chain execution.
