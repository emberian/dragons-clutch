# Agent instructions

This is Dragon's Clutch, a separate greenfield Solana protocol. Do not import,
copy, or depend on code from JOSHI, joshibot, leanuweave, minidregg, breadstuffs,
Oracle Pit, or historical DREGG prototypes unless the user explicitly changes
that decision and provenance/license review passes.

## Authority

Default work is offline and read-only with respect to external systems.

- Never read wallet/private-key dotfiles or browser sessions.
- Never sign, submit, deploy, create a market, buy a token, fund an account, or
  mutate an external production system without an explicit current authorization
  naming the act. The user's own dev machines (persvati, hbox) are ordinary
  build/test infrastructure, not gated remotes; on hbox use `swarm-build` and
  respect co-tenant workloads.
- Ordinary local commits are default work and need no authorization; keep them
  coherent and add named files explicitly while parallel lanes are live.
  Pushing, tagging, publishing, or declaring a release requires explicit user
  direction. This supersedes any stricter commit language in historical
  handoff documents.
- Public RPC reads require an explicit task and must remain bounded.
- No mainnet program or frontend URL may be described as official without a
  checked release manifest.

## Correctness vocabulary

- Do not call the program “formally verified” without naming the exact theorem,
  source digest, toolchain, assumptions, and unverified adapter/runtime boundary.
- Fixtures, simulations, and devnet executions are not mainnet evidence.
- Future fee revenue is never liveness capitalization.
- Hoard principal is never a fee, bounty, rent, reserve, or treasury source.
- Collateral is selected by an immutable Realm. DREGG is a house/dogfood profile,
  never a hard-coded branch or a requirement for other Realms.
- A state partition must be exhaustive, disjoint, ordered, and canonical before it
  can mint liabilities.
- Portfolio payoffs and simplex prices use exact scaled integers and one named
  rounding boundary.
- Say “best valid submitted candidate,” not “optimal clearing,” unless a checked
  optimality certificate exists.
- Static clients and indexes are untrusted projections of onchain state.

## Kernel policy

The Eggcrate crate is `no_std`, `no_alloc`, safe Rust, fixed-layout, and total.
First-party kernel code forbids `unsafe`, FFI, `assume`, `admit`, axioms,
`external_body`, `assume_specification`, executable `cfg(verus_only)`, floats,
dynamic allocation, and unchecked casts. Public functions validate untrusted input
and return explicit errors rather than exposing proof-only preconditions.

Keep Solana SDK, Token-2022, oracle SDKs, CPI, and account memory out of Eggcrate.
They belong in the small adapter and its separately named trust boundary.

## Evidence and edits

- Prefer primary official sources and pin versions/commit hashes for unstable
  tooling.
- Preserve exact integer units and make rounding explicit.
- Add adversarial tests with every new invariant or parser.
- Keep one semantic owner for each persisted fact and avoid parallel DTO truths.
- Use reproducible fixtures with source/derivation manifests.
- Do not weaken a refusal to make an integration test pass.
