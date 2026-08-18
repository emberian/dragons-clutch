# Engineering plan

## Gate L0: U.S. deployment feasibility

L0 precedes any public-network deployment, solicitation, live market, or real-fund
test. Offline specification, proof, simulation, and local-validator work may
continue, but no later engineering milestone can silently satisfy this legal
gate.

Required closure:

- U.S. derivatives counsel reviews the exact product, facility, clearing,
  intermediary, sanctions, money-transmission, and state-law facts;
- the CFTC Innovation Task Force discussion packet in
  [regulatory/](regulatory/README.md) is reconciled to the then-current design;
- the project identifies whether the viable path is registration, registered
  partnership, fact-specific relief, a bounded non-U.S. or nonpublic scope, or no
  deployment;
- the proposed legal person, users, collateral, products, controls, fees,
  compensation, affiliate activity, client, upgrade authority, and deployment
  conduct are explicit; and
- any formal relief or registration is effective before relying on it.

An agency meeting, pending request, public speech, another person's no-action
letter, source publication, program immutability, or a passing proof suite does
not close L0.

## 1. Program objective

Produce an AGPL, reproducibly built, statically hosted Solana protocol that
compiles objective finite state spaces into fully collateralized basis assets and
clears them coherently. The program ends with publishable source and release
evidence, not an authorized mainnet deployment.

The project earns its distinctiveness through four jointly necessary results:

1. a canonical compiler from authenticated state programs to exhaustive disjoint
   partitions and standard Token-2022 basis assets;
2. exact bounded payoff portfolios over those assets;
3. a clutch-aware auction that clears a coupled probability simplex and can use
   complete-set creation/destruction during matching; and
4. a collateral-generic, prepaid, verification-backed implementation requiring no
   Dragon-operated service.

The implementation lowers the native venue into the bounded transparent relation
specified in [SPECIALIZED_BATCH_RELATION.md](SPECIALIZED_BATCH_RELATION.md), not a
generic exchange VM. The overall component and trust-boundary map is in
[ARCHITECTURE.md](ARCHITECTURE.md); the actionable packet list is in
[V1_BACKLOG.md](V1_BACKLOG.md).

A generic binary prediction market, a DREGG-only market, or a conventional
per-outcome orderbook does not satisfy the objective.

Every stage must leave a useful artifact if a later gate fails. No stage may
silently acquire financial, deployment, wallet, hosted-service, or mainnet-write
authority.

## 2. Architecture and source layout

Proposed repository shape:

```text
Cargo.toml
rust-toolchain.toml
verus-toolchain.toml              # or pinned installer manifest
crates/
  eggcrate/                       # pure no_std/no_alloc safe Rust + Verus
  clutch-partition/               # canonical closed state/partition compiler
  clutch-wire/                    # fixed byte layouts shared with adapter/client
  clutch-model/                   # ordinary Rust reference/oracle harness
  clutch-solana/                  # minimal native SBF adapter
  clutch-accumulator/             # pure source-neutral summary semantics
  clutch-source-pyth/             # adapter, outside Eggcrate proof boundary
  clutch-source-raydium/
  clutch-source-meteora/
  clutch-simplex/                 # pure coupled auction/candidate semantics
  clutch-venue-manifest/          # optional ordinary-Egg venue adapter
  clutch-client-contract/         # generated language-neutral client DTOs
rocq/
  Model.v
  Solvency.v
  Partition.v
  Payoff.v
  Liveness.v
  Accumulator.v
  Simplex.v
  Vectors.v
programs/
  dragons-clutch/                 # entrypoint and account/CPI adapter
apps/
  glass/                          # static client
fixtures/
  kernel/
  solana/
  accumulator/
  venue/
  cross-runtime/
scripts/
  trust-audit
  offline-readiness
  reproduce-sbf
  build-static-release
docs/
```

The exact crate split is subordinate to dependency direction:

```text
Rocq model       Eggcrate
     \             /
      shared semantic vectors
               |
          wire/intent types
               |
      Solana/source adapters
               |
       programs + static client
```

Eggcrate depends on no Solana or application crate. The adapter may depend on
Eggcrate. The verified kernel never depends upward.

## 3. Stage E0: semantic and novelty freeze

### Deliverables

- Freeze semantic names and versioning rules.
- Freeze `MAX_OUTCOMES = 16` as a provisional measured bound.
- Freeze Realm as a generic collateral profile. Use synthetic collateral in tests;
  record DREGG only as one separately configured house Realm.
- Define Template, Instance, and Series identity and their content-addressed
  boundaries.
- Define the closed V1 Source/Window/Statistic/Partition program rather than an
  arbitrary scripting language.
- Prove on paper that every admitted partition is ordered, exhaustive, disjoint,
  nonempty where required, and canonically encoded.
- Define exact payoff vectors, portfolio dot products, scale, bounds, and a single
  rounding point.
- Define exact state/input/error/transition algebra without Solana accounts.
- Define internal/materialized supply and every balance domain.
- Define one-hot payout and at least two candidate failure-vector policies.
- Define exact fee-policy interface with uncertainty and flat controls.
- Include the simplex-dispersion fee from [FEE_GEOMETRY.md](FEE_GEOMETRY.md) and
  prove/refute complete-set, homogeneity, carry, and partition-refinement claims.
- Define a closed deployment-level `RevenuePolicy`; maker/executor rewards,
  development or audit reserves, public goods, and zero-take remain explicit
  alternatives outside Hoard/liveness.
- Preserve the source/deployment/operation/JOSHI release-track separation in
  [DEPLOYMENT_REVENUE_BOUNDARY.md](DEPLOYMENT_REVENUE_BOUNDARY.md).
- Write bounds for every executable multiplication and accumulator aggregate.
- Write canonical JSON and compact binary test-vector formats.
- Specify the simplex-auction intent language, candidate witness, score, and
  verifier. Mark unrestricted all-or-none combinatorial matching out of scope.
- Establish the tractability boundary: which intents are solved exactly, which
  accept the best valid submitted candidate, and which are rejected.

### Required adversarial analyses

- Failure/ambiguity sabotage payoff over every allowed payout vector.
- Direct external burns, unequal supplies, donations, and orphaned token accounts.
- Rational payout divisibility and fragment aggregation.
- Cross-market/source common-mode exposure.
- collateral/reward-token price collapse and zero-volume continuation.
- Every path by which Hoard, fee, rent, and liveness pools might be confused.
- Nonexhaustive/overlapping partitions, boundary aliasing, and semantically equal
  but byte-distinct Templates.
- Basket-intent conflict, candidate withholding, score gaming, and false claims of
  global optimality.

### Gate

No code calls a resolution policy canonical until the finite payout set and
failure incentives are explicit. No code calls the native venue coherent until
the candidate witness proves simplex, limits, and conservation. If no acceptable
failure rule exists, narrow V1 to terminal facts or sources with a stronger
completeness guarantee. If portfolio matching is not cheaply verifiable, retain
single-Egg coupled clearing and defer portfolio intents rather than lying.

## 4. Stage E1: falsifying proof/toolchain spike

This is the first implementation work.

### Eggcrate spike

Implement six tiny components in `no_std`, `no_alloc`, safe Rust:

1. `u64/u128` uncertainty-shaped fee and exact share allocation;
2. closed-enum partition validation and canonical encoding;
3. bounded payoff-vector evaluation with one exact rounding boundary;
4. categorical split/materialize/dematerialize/merge/resolve/redeem transition;
5. one constant-space feed summary transition with coverage and ambiguity;
6. one fixed-size canonical codec.

Each exported function is total and has no caller-visible proof-only precondition.

### Verus proof

Prove:

- overflow and bounds;
- partition order, disjointness, exhaustiveness, and unique cell selection;
- payoff-vector bound and portfolio linearity before the frozen rounding point;
- local well-formedness preservation;
- internal/materialized conservation;
- maximum-liability solvency;
- fee conservation and cap;
- monotone feed boundary;
- codec rejection and round trip.

### Solana wrapper

Write the smallest native SBF wrapper that invokes the exact erased Eggcrate
source. It may use synthetic program-owned accounts initially and must not need a
wallet secret or remote cluster.

### Falsifying checks

- Compile the same executable source with pinned Verus/upstream Rust and Anza SBF.
- Prove no executable `cfg` divergence.
- Run all vectors on ordinary host and `solana-program-test`.
- Archive source/proof/vector/ELF hashes.
- Compare annotated and unannotated CU, stack, heap, and ELF.
- Mutate a partition boundary, payoff coefficient, collateral update, fee
  numerator, range check, and codec length; require proof/test failure.
- Mechanical trust audit rejects prohibited constructs.

### Stop conditions

Stop or redesign if:

- SBF cannot compile the exact verified source subset;
- an economic theorem needs a first-party assumption;
- public preconditions leak into the adapter;
- the toolchains require different executable branches;
- required integer widths cause impractical SBF behavior;
- proof maintenance already dominates the tiny transition surface.

## 5. Stage E2: independent Rocq theorem

### Deliverables

- Hand-written finite transition system independent of Rust syntax.
- Reachability and well-formedness.
- Finite-partition exhaustive/disjoint/unique-selection theorem.
- Basis decomposition and bounded-payoff portfolio theorem.
- Hoard maximum-liability theorem.
- Internal/materialized supply theorem.
- Protected-pool noninterference.
- Liveness booking/double-payment theorem.
- Terminal settlement and remainder conservation.
- Simplex-price normalization and candidate conservation theorem for the minimal
  auction fragment.
- Simplex-dispersion fee translation/scale/partition-refinement theorem.
- Extracted reference evaluator.

### Cross-check

Use the same semantic vector manifest, not the same implementation. Generate
bounded exhaustive traces plus randomized longer traces. Compare Rocq-extracted,
ordinary Rust, Verus-checked Rust, and SBF behavior byte-for-byte.

### Gate

Every Eggcrate transition maps to a named Rocq constructor and error class. Manual
correspondence remains a disclosed assumption. A rocq-of-rust feasibility probe
may translate the frozen tiny kernel, but cannot delay E3 unless explicitly
promoted after zero-admission closure succeeds.

## 6. Stage E3: collateral-generic hostile-byte issuance adapter

### Deliverables

- Fixed account and instruction layouts with version/tag/length checks.
- Realm creation/profile validation for at least two incompatible synthetic token
  profiles; generic behavior must not branch on mint identity.
- Template registration and deterministic Instance creation.
- Market, Hoard, Position, SupplyLedger, outcome-mint, and lifecycle accounts.
- Stored-bump canonical PDAs.
- Internal split/merge.
- Materialize/dematerialize.
- Fully external split/merge as a slower compatibility path.
- One-hot resolve/redeem and selected failure-vector prototype.
- Explicit CPI intents produced by Eggcrate and checked by adapter.

### Adversarial tests

- missing, extra, reordered, duplicated, aliased, readonly, or wrong-owner account;
- false signer, wrong bump, wrong Realm, wrong mint/index/decimals/extension;
- malicious Token-2022 destination and cross-Market substitution;
- maximum values, zero, overflow, partial effect, direct external burn;
- retry, replay, and post-terminal instruction;
- atomic rollback at every CPI boundary.

### Gate

One local-validator walking fixture performs internal split, materialize,
dematerialize, merge, resolve, external and internal redeem, then independently
recomputes all account and supply invariants after process restart.

Run the same walk in two synthetic Realms. A later bounded read-only inspection
may populate a DREGG Realm manifest, but E3 neither requires nor privileges it.

No devnet or mainnet transaction is required to pass E3.

## 7. Stage E4: prepaid shared accumulator

### Deliverables

- FeedSpec, FeedHead, archive-summary page, WindowWork, and WindowResult.
- Exact job booking and reverse-Dutch payout.
- O(1) subscription/reimbursement index.
- Source-neutral interval-summary kernel.
- At least one strongest generic adapter and one long-tail DEX cumulative adapter,
  selected only after source audit.
- Source program/deployment binding and upgrade refusal.
- Coverage, repair, seal, recycle, and deterministic failure semantics.
- Common-mode exposure counter/cap.

### Source order

1. Synthetic cumulative source for exhaustive state tests.
2. Pyth fully verified price/TWAP record where available.
3. One audited Raydium or Meteora native cumulative history.
4. Additional sources only after differential semantics tests.

### Gates

- No source CPI on ordinary read path unless unavoidable.
- One accepted update changes only FeedHead/current page/keeper credit.
- No market fan-out.
- Every identical Window shares one result.
- Raw history is not discarded before every possible live reference expires.
- Missing/upgrade/wide-confidence/quorum cases produce only frozen refusal/failure.
- Booked SOL/reward obligations survive zero future volume and collateral or
  reward-asset price collapse.

## 8. Stage E5: simplex auction and ordinary-venue interoperability

### Deliverables

- Internal collateral/Egg reservation through Eggcrate-owned transitions.
- Sharded dense Epoch pages for single-Egg limit orders and the admitted bounded
  portfolio-intent language.
- Freeze/cancel state machine and exact frozen order-set digest.
- Integer simplex price vector whose components are nonnegative and sum exactly
  to `PRICE_SCALE`; fees are outside that identity.
- Permissionless bonded candidate witnesses containing fills, any virtual
  complete-set splits/merges, remainder allocation, and exact public score.
- Paginated `O(m*n)` or better verification over the frozen pages with bounded
  ClearWork; no solver trust and no unverified offchain state transition.
- Deterministic feasibility and lexicographic score comparison, with a bounded
  proposal window in which a strictly better valid candidate replaces the head.
- Atomic Final flip and lazy idempotent settlement.
- Maker-duration classification, uncertainty-shaped fee distribution, and
  portfolio-aware execution receipts.
- Materialize/dematerialize adapter for ordinary Egg trading on Manifest. Manifest
  is interoperability, not the authority for the coupled auction or Hatch.

### Gates

- Every accepted order is fully reserved.
- Every order is scanned exactly once for the final proposal.
- Every final price vector lies on the simplex and every filled single-Egg or
  portfolio intent satisfies its exact limit.
- Candidate accounting conserves collateral and every Egg, including virtual
  complete-set conversion.
- Candidate comparison is deterministic. Documentation says “best valid submitted
  candidate,” never “globally optimal,” unless an independently checked optimality
  certificate is implemented.
- No page balance is simultaneously an order reserve and final settlement pot.
- Invalid candidates cannot permanently block clearing.
- Opposite settlement order and retry produce identical balances.
- Self-cross and split-order adversaries cannot earn a net protocol subsidy.
- External materialization is optional and one-outcome-at-a-time.
- A generic per-Egg orderbook control cannot produce incoherent native semantics;
  the client clearly distinguishes external spot prices from the coupled auction's
  distribution.

Commit/reveal is a separately versioned feature and cannot enter E5 merely as a
cost optimization. Unrestricted arbitrary baskets, integer winner determination,
and cross-Market netting are future research. A future LP/primal-dual certificate
or a proven totally-unimodular restricted intent language must earn promotion.

## 9. Stage E6: static Glass

### Deliverables

- Fully static application and generated strict wire contracts.
- User-selected RPC and wallet.
- Realm/Market/manifest/program/ELF identity display.
- Market discovery from exact addresses and untrusted candidate indexes.
- Template compiler, partition table, payoff composer, distribution/simplex,
  Position, Clutch, materialization, auction, feed, Window, Hatch, and redemption
  surfaces.
- Exact explanations showing how one Egg portfolio becomes a crash hedge, range,
  tail, or capped directional claim without implying leverage or guaranteed
  liquidity.
- Distinct displays for coupled auction prices, external venue prices, and any
  no-arbitrage discrepancy; never silently fuse them.
- Paid permissionless-work queue.
- Exact transaction preview and expected postconditions.
- IPFS CID release and GitHub Pages mirror build.
- Local downloadable/offline-cached distribution.

### Gates

- No server endpoint, secret, analytics, remote JS, or privileged index.
- Unknown semantics refuse.
- RPC/index lies do not pass local account validation.
- Static bundle matches the release manifest.
- Keyboard, screen reader, motor-access, reduced-motion, and text-equivalent QA.
- A malicious-client test proves the onchain program rejects every attempted
  economic invariant violation.

## 10. Stage E7: economic and load laboratory

### Fee experiments

Replay/simulate immutable policy arms:

- uncertainty `kappa`: 0, .001, .002, .004, .007, .010;
- flat midpoint-equivalent: 0, 5, 10, 20, 35, 50 bp;
- atomic simplex-dispersion versus decomposed per-Egg fee;
- maker shares: 33%, 50%, 60%, 67%;
- executor shares/caps; treasury floor;
- order splitting, maker/taker role changes, wash loops, external-route leakage.

Measure contribution, depth, all-in spread/slippage/fee, fill latency, active
wallet retention, price-coherence error, maker concentration, and self-trade loss.

### Liveness experiments

- recorded/synthetic base and priority fee distributions;
- keeper competition, theft, failure, congestion, censorship delay;
- P50/P99/P99.9 landing costs;
- 1.2x/1.5x/2x+ bounty schedules;
- shared-feed subscriber arrival/refund order;
- provider outage and source upgrade;
- collateral/reward-token price collapse and zero-volume Market.

### Auction experiments

- single-Egg-only exact solver baseline;
- complete-set virtual conversion on/off;
- proportional payoff-intent families versus decomposed legs;
- solver competition, delayed/withheld candidates, invalid witnesses, and ties;
- quality gap against an offline exhaustive/LP oracle on small books;
- coherent-simplex error and complete-set arbitrage duration versus independent
  per-Egg books;
- candidate verification CU/account/rent cost as orders, outcomes, and shards grow.

### JOSHI/agent experiments

- strict Template/Instance/partition/payoff artifact export with no code dependency
  on the JOSHI repository;
- chronological belief vector versus final simplex calibration;
- payoff-vector edge after every executable cost and failure haircut;
- existing spot/LP book wealth mapped over the same states;
- tail/range hedge improvement against spot/perp controls;
- full-simplex inventory-aware quote shadowing;
- remove all JOSHI-originated orders before treating the venue as an independent
  sensor;
- counterparty-depth and source-influence kill tests from
  [JOSHI_EXECUTION_THESIS.md](JOSHI_EXECUTION_THESIS.md).

This stage is read-only/shadow. User-signed or automated trading remains a separate
authority and legal promotion, not an engineering benchmark.

### Capacity experiments

Run the complete cost matrix in [COST_MODEL.md](COST_MODEL.md). Publish actual CU,
transaction/account/trace size, rent, lock contention, and refusal thresholds.

### Promotion rule

Choose the lowest fee and cheapest layout satisfying safety, reliability,
market-quality, and maintainer-contribution constraints. No simulated profit or
volume upgrades a security claim.

The measurement schemas, falsifiers, and result-directory contract are specified
in [BENCHMARK_PLAN.md](BENCHMARK_PLAN.md). Proof-property IDs and cross-runtime
evidence requirements are tracked in [EVIDENCE_MATRIX.md](EVIDENCE_MATRIX.md).

## 11. Stage E8: public source release candidate

### Required closure

- Full offline readiness from a fresh clone.
- Clean Verus and Rocq proof gates and theorem inventory.
- Trust/assumption audit.
- Canonical cross-runtime vectors and mutation tests.
- Reproducible SBF ELF and static bundle hashes.
- Complete dependency licenses, AGPL source offer, SBOM, and fixture provenance.
- Independent security review or explicit unaudited status.
- Static client published by immutable CID and reproducible locally.
- Deployment guide that defaults to immutable program authority.
- No bundled secret, private RPC key, deploy authority, or live Market.
- Exact release-track label from
  [DEPLOYMENT_REVENUE_BOUNDARY.md](DEPLOYMENT_REVENUE_BOUNDARY.md); source release
  must not imply author-affiliated operation or JOSHI trading.

### Release statuses

- `research_prototype`: proofs/fixtures incomplete; no real funds.
- `offline_candidate`: all local gates pass; not deployed or audited.
- `devnet_candidate`: explicitly authorized devnet experiment only.
- `audited_source_candidate`: independent findings reconciled.
- `reference_deployment`: only after a separate explicit authorization and exact
  release/deployment manifest.

Publishing source never implies the authors deployed, endorse, or operate a
particular instance.

## 12. First execution packet for a new Codex

The next coding agent should do **E0 plus E1 only**:

1. inspect this entire repository and all documents;
2. validate current official Verus, Rocq, Solana/SBF, and Token-2022 tool versions;
3. freeze a minimal workspace and toolchain without copying old project code;
4. implement the six-component Eggcrate falsifier, including partition and payoff
   algebra;
5. implement the minimal local SBF wrapper;
6. build canonical vectors and mutation/trust audits;
7. build a host-only minimal simplex candidate/verifier prototype for tiny books;
8. report whether both the single-source dual-toolchain architecture and the
   proposed distinctive algebra survive.

It must not build a polished client, deploy, use a wallet, perform a paid query,
or expand into scalable accumulator/venue work before the E1 stop conditions are
evaluated. The tiny host candidate/verifier is an algorithmic falsifier, not E5.
