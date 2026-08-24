# dClutch agent instructions

dClutch is a greenfield Solana protocol for fully collateralized bounded-state
claims. The neighboring `~/dev/dragons-clutch` repository is compost: it may be
studied for requirements, invariants, counterexamples, and measurements, but it
is not a source tree to copy wholesale.

## Authority and safety

- Default work is offline. Never read wallet/private-key dotfiles or browser
  sessions.
- Never sign, submit, deploy, fund, publish, push, tag, or mutate an external
  system without explicit current authorization naming that act.
- Local commits are ordinary work. Add named files explicitly while parallel
  work is live.
- Public RPC reads require an explicit task and must remain bounded.
- Never describe a deployment or frontend as official without a checked release
  manifest.

## Provenance

- Do not import, copy, or depend on code from JOSHI, joshibot, leanuweave,
  minidregg, breadstuffs, Oracle Pit, or historical DREGG prototypes.
- Dragon's Clutch code may be transplanted only through the process in
  `COMPOST.md`: name the invariant, source commit and path, license/provenance,
  new semantic owner, and adversarial tests. Prefer a fresh implementation from
  the written invariant.
- Never merge an old implementation merely to preserve sunk work. Git history
  grafting happens only after dClutch has a coherent independent architecture.

## Architecture

- Keep the universal Market Core small. Optional venues, liquidity, wrappers,
  bearer mints, and recovery depth are capability children, not universal
  ontology.
- Persist economic facts and replay authority, not an offchain workflow graph.
- One semantic owner per persisted fact. A separate concept does not
  automatically deserve a separate account.
- Market capabilities are immutable and canonically identified. Deferred
  physical creation must be precommitted and prepaid.
- Hoard principal is never fees, rent, bounty, insurance, work funding, reserve,
  or treasury capital. Future revenue is never present capitalization.
- Static clients, routers, matchers, and indexes are untrusted projections.

## Correctness vocabulary

- A state partition must be exhaustive, disjoint, ordered, and canonical before
  it can mint liabilities.
- Portfolio payoffs and simplex prices use exact scaled integers with one named
  rounding boundary.
- Say "best valid submitted candidate," never "optimal clearing," without a
  checked optimality certificate.
- Fixtures, simulation, local-validator execution, and devnet execution are
  distinct evidence levels and are not mainnet evidence.
- Do not call the protocol formally verified without naming the theorem, source
  digest, toolchain, assumptions, and unverified runtime boundary.

## Kernel policy

The first-party kernel is `no_std`, `no_alloc`, safe Rust, fixed-layout, and
total. It forbids `unsafe`, FFI, floating point, dynamic allocation, unchecked
casts, proof-only preconditions, and hidden executable alternatives. Public
functions validate untrusted input and return explicit errors.

Solana SDK, Token-2022, oracle SDKs, CPI, account memory, RPC, and transaction
construction belong outside the kernel in explicitly named adapters.

## Project conduct

- Work from one canonical integration branch. Delegated lanes are short-lived,
  bounded, and either touch disjoint files or coordinate before editing.
- Build vertical executable slices. A slice includes kernel semantics, adapter,
  operator construction, and an honest user-visible status; no layer may claim
  completion alone.
- Do not substitute repeated audits for implementation. Reviews must end in a
  concrete decision, deletion, test, or queued implementation owner.
- Do not preserve parallel legacy/current authority paths. When a successor is
  accepted, delete the superseded path in the same convergence cycle.
- Every fixed bound is labeled as mathematical, chain-derived, measured-profile,
  or provisional. Provisional bounds require a lifting plan.
- Add adversarial tests with each invariant or parser. Do not weaken a refusal
  to make an integration test pass.
