# V1 execution backlog

Status: offline engineering plan. Checkboxes are work state, not authorization.

## 0. Current boundary

- [x] Repository contains only pre-implementation specifications.
- [x] AGPL-3.0-or-later is the first-party license.
- [x] Gate L0 blocks public-network deployment, real funds, solicitation, and
  author-affiliated operation.
- [ ] No toolchain, account layout, source preset, failure payout, or fee policy
  is yet frozen as a release fact.

The next implementation packet is E0/E1 only. It may use ordinary local files,
offline proof tools, synthetic fixtures, host tests, and a local validator. It may
not use a wallet, secret, RPC write, faucet, devnet, mainnet, paid provider,
financial transaction, or regulator contact.

## 1. Packet A: repository and semantic freeze

### A1: workspace decision record

- [ ] Record exact supported host architecture and offline prerequisites.
- [ ] Research and pin Verus, Rust, Anza SBF, Solana program-test, Rocq, Z3, and
  Token-2022 versions from primary sources.
- [ ] Record licenses and exact commits before adding dependencies.
- [ ] Prove a minimal `no_std` safe-Rust function verifies under Verus and builds
  unchanged for SBF before creating broad crate structure.
- [ ] Decide whether Lean begins as vector checker, finite batch shadow, or seam
  only; do not make it a duplicate V1 implementation by inertia.

Output: toolchain manifest, compatibility report, and proceed/redesign decision.

### A2: semantic registry

- [ ] Assign numeric versions and canonical IDs to Realm, Template, Instance,
  Position, payout, accumulator, relation, fee, allocation, score, and codecs.
- [ ] Freeze exact integer units and proposed maximum widths.
- [ ] Freeze canonical padding, endianness, tag, length, and domain separation.
- [ ] Define one error taxonomy mapped across reference, Eggcrate, and adapter.
- [ ] Define two incompatible synthetic collateral profiles with no mint-specific
  kernel branch.

Output: E0 semantic registry and canonical vector schema.

### A3: open-policy falsifiers

- [ ] Compare at least two finite failure payout policies under sabotage.
- [ ] Choose redemption lot versus persistent remainder credit only after an
  atom-conservation model.
- [ ] Decide empty partition cell policy and Template equivalence.
- [ ] Freeze the minimal portfolio-intent fragment and explicitly reject the
  remainder.
- [ ] Test candidate score, pro-rata rule, fee carry, and self-cross policy on
  exhaustive tiny examples.

Output: decision records or explicit E1 blockers; convenience code does not decide.

## 2. Packet B: Eggcrate falsifier

### B1: minimal kernel

- [ ] `no_std`, `no_alloc`, safe Rust crate with `#![forbid(unsafe_code)]`.
- [ ] Checked fee and exact allocation component.
- [ ] Closed partition validator and unique selection.
- [ ] Bounded payoff dot product with one rounding boundary.
- [ ] Split/materialize/dematerialize/merge/resolve/redeem state fragment.
- [ ] One constant-space observation summary transition.
- [ ] Fixed-size canonical codecs and explicit errors.

### B2: Verus proofs

- [ ] Bound every index and arithmetic intermediate.
- [ ] Close partition and payoff properties.
- [ ] Close local well-formedness and maximum-liability preservation.
- [ ] Close internal/materialized supply conservation.
- [ ] Close fee cap/allocation/carry and accumulator monotonicity.
- [ ] Close codec rejection and round trip.
- [ ] Run the mechanical prohibited-construct audit.

### B3: dual-toolchain test

- [ ] Build the exact erased source for host and SBF.
- [ ] Execute canonical vectors through reference, Eggcrate host, and program-test.
- [ ] Record source/proof/vector/ELF digests.
- [ ] Compare annotated, erased, and unannotated control resources.
- [ ] Run deliberate property-directed mutations.

Output: proceed/redesign/reject report for single-source Verus.

## 3. Packet C: specialized relation laboratory

- [ ] Implement a tiny ordinary-Rust relation verifier independent of Solana.
- [ ] Implement exhaustive enumeration for the smallest books.
- [ ] Add single-Egg demand/supply curve candidate construction.
- [ ] Add virtual complete-set conversion and exact conservation closure.
- [ ] Add proportional portfolios only inside a bounded exact-rational comparison.
- [ ] Generate canonical witness, error, tie, remainder, and settlement fixtures.
- [ ] Prove/verifiably check the bounded dot, simplex, asset fold, page resume, and
  deterministic comparison kernels.
- [ ] Report search quality separately from witness verification cost.

Stop if the admitted language requires an unverified optimizer or pressures the
project to call the result globally optimal. A valid single-Egg coupled fragment
is a successful narrowed result.

## 4. Packet D: independent proof shadow

- [ ] Define Rust-independent finite state and transition constructors in Rocq.
- [ ] Prove partition, payoff, supply, solvency, protected-pool, liveness, batch
  conservation, and settlement theorems.
- [ ] Extract an evaluator and compare canonical vectors.
- [ ] Prototype the Lean seam only after the vector/relation schema is frozen.
- [ ] Publish theorem and assumption inventory with exact digests.

Manual refinement remains named. `rocq-of-rust` is an experiment after source
freeze, not a substitute for the hand-written model or a V1 blocker.

## 5. Packet E: hostile Solana adapter

- [ ] Freeze account and instruction layouts only after the kernel survives.
- [ ] Implement strict byte parsing, exact accounts, alias rejection, owner,
  signer, writable, PDA, generation, and replay checks.
- [ ] Keep every Solana/Token/source SDK out of Eggcrate.
- [ ] Implement only kernel-issued CPI intents and verify return behavior.
- [ ] Exercise two synthetic Realm profiles and malicious Token-2022/account cases.
- [ ] Complete one restartable local-validator lifecycle fixture.

This packet still grants no devnet or mainnet authority.

## 6. Packet F: shared accumulator

- [ ] Freeze the smallest associative feature family that supports one useful
  terminal/path product.
- [ ] Prove summary combination, coverage, repair, generation, and retention.
- [ ] Implement synthetic source before evaluating a real adapter.
- [ ] Capitalize every unfinished job under zero future volume.
- [ ] Measure source/window sharing and common-mode exposure.

Stop or narrow when a source, feature, retention rule, or failure policy cannot be
authenticated and bounded.

## 7. Packet G: full transparent batch and Token-2022 seam

- [ ] Add dense public order pages, freezing, candidate work, replacement, Final,
  and lazy settlement.
- [ ] Prove all accepted orders are reserved and scanned exactly once.
- [ ] Prove final pots and reservations are different ownership phases.
- [ ] Implement one-outcome materialization/dematerialization boundary.
- [ ] Benchmark optional ordinary external venue interoperability without making
  it authoritative.

Commit/reveal, confidential orders, FHE/MPC/TEE, unrestricted baskets, and cross-
Market netting remain explicitly out of this V1 packet.

## 8. Packet H: static Glass and release evidence

- [ ] Generate strict wire contracts from one schema owner.
- [ ] Build a static, secret-free, analytics-free client with exact previews.
- [ ] Validate untrusted RPC/index results locally and refuse unknown semantics.
- [ ] Produce reproducible bundle, manifest, SBOM, licenses, provenance, and CSP.
- [ ] Perform accessibility, malicious-client, and offline-distribution tests.
- [ ] Assemble an `offline_candidate` evidence bundle from a fresh clone.

Source release status does not imply a deployment or operating endorsement.

## 9. Gate L0: regulatory mainnet boundary

Engineering work can prepare factual architecture, threat, control, and evidence
materials. It cannot close classification questions. Before any author-affiliated
real-money or public-network path:

- [ ] exact proposed product, users, entity, control, collateral, source, fee,
  compensation, affiliate, client, and deployment facts are frozen;
- [ ] qualified counsel has delivered written analysis across the relevant
  federal and state perimeters;
- [ ] any required registration, partnership, no-action, exemptive, or other
  relief is actually effective and applicable to the exact persons and facts;
- [ ] security audits, surveillance/incident design, capitalization, conflicts,
  and release manifest are complete;
- [ ] the user gives separate explicit current authorization for the named
  network/deployment act.

A meeting request, public comment, pending filing, another person's relief,
devnet result, audit, immutable source, or proof suite does not satisfy this gate.
