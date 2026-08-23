# Glass chain console

Glass is the dependency-free, static-hostable read-only console for Dragon's
Clutch. It has no configured network, program, release, wallet, signer, or
transaction submission path at startup. A user must explicitly select every
chain and release coordinate before the page can make a bounded read.

The application has three narrow jobs:

1. read the fork-aware untrusted account index exposed by `operatord`;
2. bind and display exact Product compiler proposals emitted by Rust; and
3. assemble the outer blockhash-free Solana transaction around exact bytes and
   account roles supplied by their semantic owner.

It is not an explorer, index authority, compiler implementation, wallet, or
release manifest.

## Explicit target

The form requires these values and embeds none of them as defaults:

- operatord base URL;
- cluster name and genesis hash;
- validator HTTP and WebSocket URLs used by the daemon's acquisition plan;
- processed or finalized commitment;
- program and ProgramData addresses, deployment slot, ELF SHA-256;
- release-manifest SHA-256, source commit, capability-profile identity; and
- browser account, response-byte, timeout, and slot-lag bounds.

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
exact program, ProgramData, deployment-slot, ELF, and release-key equality.
Rows from other releases are counted and ignored rather than blended.

`operatord chain-serve --config FILE` is the live owner of these routes. Before
binding HTTP it checks `getGenesisHash`, hostile-decodes each selected
Program/ProgramData loader pair, checks the decoded deployment slot, and hashes
the observed ProgramData ELF. It then repeatedly admits bounded finalized
`getProgramAccounts` responses through `RpcIndexEngine`. One bounded ordered
WebSocket owner admits the complete program, block, slot-update, and root
subscription set. It keeps processed reads withdrawn across registration,
release-bracketed scan, replay, disconnect, rollback, and capped reconnect
backoff. The acquisition response echoes the exact daemon HTTP+WebSocket and
release coordinates; the browser refuses any mismatch. See
[`CHAIN_SERVE.md`](CHAIN_SERVE.md).

## Projection semantics

The console groups chain-derived account rows into Market, Product, Source,
Series, candidate/clearing, settlement/position, covered-liquidity, recovery,
and unknown-release surfaces. It preserves account body digests, generation
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
No processed projection is authority-eligible.

Successor coordinates are mirrored from the central registry only to label and
frame construction material. Every family remains `reserved-disabled` in this
client. A decoded family is not executable capability admission, and the
current operatord API does not authenticate the user-declared manifest, source
commit, or capability-profile identity.

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
bundle capability-profile ID must match the explicit release selection. An
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

A selected keeper cursor adds exact joins: the draft's first successor
coordinate must match the known keeper action, and the driver/dependencies must
appear both in explicit metas and the acquired selected-release projection.
Every workflow node states that authoritative accounts must be reloaded.

## Files

- `chain-client.js`: explicit configuration and bounded operatord transport.
- `successor-registry.js`: non-authoritative coordinate mirror and disabled
  capability reasons.
- `successor-builder.js`: exact outer message and unsigned transaction bytes.
- `compiler-proposal.js`: bounded pure-compiler transport and exact proposal
  validation; no compiler math.
- [`COMPILER_TRANSPORT.md`](COMPILER_TRANSPORT.md): exact Rust adapter JSON
  contract and canonicalization rule.
- [`CHAIN_SERVE.md`](CHAIN_SERVE.md): explicit bounded HTTP/WebSocket acquisition,
  processed rollback/reconnect, and finalized projection boundary.
- `app.js`, `index.html`, `styles.css`: DOM presentation.
- `manifest.json` and `terms.json`: retained historical evidence records; no
  shipped script loads them and they are not application defaults.
- [`SERVING.md`](SERVING.md): same-origin/CORS and response-header guidance.

There are no runtime dependencies or asset build step. The repository keeps
local mechanical checks under `test/`, but this implementation was deliberately
not run against a browser, validator, RPC, wallet, or test command as part of
the implementation-only swarm cycle.
