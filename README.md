# Dragon's Clutch

**Verification-target conditional assets for Solana.**

Dragon's Clutch is a proposed permissionless protocol for fully collateralized,
liquidation-free claims over objective onchain facts. Its verification-target Rust kernel is
called **Eggcrate**; a complete exhaustive set of outcome claims is a **Clutch**;
collateral is held in a segregated **Hoard**; final resolution is a **Hatch**.

Collateral is an immutable Realm parameter, not a protocol-wide token. The
proposed house and dogfood Realm references the `$DREGG` mint address:

```text
XkeTXo1125vz5H9svJpGiw4JvLbN8VmMu9cmMvspump
```

Its token program, decimals, authorities, extensions, and current supply are
not frozen chain facts in this repository. Realm admission must authenticate
them onchain against a separately approved collateral profile before use.

The protocol is intended to require no Dragon-operated service. Programs and
state live on Solana; observations, repairs, clearing, finalization, and cleanup
are permissionless paid instructions; the client is a reproducible static bundle
publishable to IPFS and GitHub Pages.

## Status

This repository is in **offline prototype implementation**. It contains bounded
pure-Rust kernel, accumulator, and batch-relation prototypes, a static offline
client, plus deterministic economics, cost, and toolchain labs. It contains no
deployed program, transaction builder, private key, market, or financial
authority. Verus and Rocq are installed and pinned
(see toolchain/PINNED_PROOF_TOOLS.md), but no proof has been closed: the shadow
specifications currently fail to check, so the present Rust code is tested and
linted but not formally verified.
All parameters remain hypotheses until the required proofs, benchmarks,
simulations, and adversarial tests pass.

## Non-negotiable principles

1. Claims are fully funded.
2. Required liveness work is fully prepaid.
3. Revenue is optional upside, never part of an existing market's safety proof.
4. Hoard principal pays only claimants.
5. Outcome claims can be ordinary transferable Token-2022 assets.
6. The native simplex venue uses a cheaper internal representation, clears the
   coupled outcome distribution coherently, and permits one-outcome-at-a-time
   materialization into those standard assets.
7. Every resolution input is objective, frozen, versioned, and recomputable.
8. No debt, leverage account, liquidation, discretionary resolver, custody,
   governance theater, or hidden operator service.
9. The static client is replaceable and untrusted.
10. Verification claims name their trust boundary precisely.

## Documents

- [PROJECT.md](PROJECT.md): canonical product and protocol brief.
- [V1 architecture](docs/ARCHITECTURE.md): dependency direction, semantic
  ownership, trust boundaries, and lifecycle.
- [Product thesis](docs/PRODUCT_THESIS.md): state-space compiler and user value.
- [Competitive position](docs/COMPETITIVE_POSITION.md): prior art, substitutes,
  distinctive conjunction, and bootstrap test.
- [Partition algebra](docs/PARTITION_ALGEBRA.md): basis assets and payoff vectors.
- [Simplex auction](docs/SIMPLEX_AUCTION.md): clutch-aware coupled clearing.
- [Specialized batch relation](docs/SPECIALIZED_BATCH_RELATION.md): the bounded
  transparent market relation and witness/verifier boundary.
- [Fee geometry](docs/FEE_GEOMETRY.md): state-contingent portfolio fee hypothesis.
- [JOSHI execution thesis](docs/JOSHI_EXECUTION_THESIS.md): why the venue could be
  an actual field-model trading and hedging surface.
- [Deployment/revenue boundary](docs/DEPLOYMENT_REVENUE_BOUNDARY.md): source,
  operation, conflicts, revenue policy, and legal stop gates.
- [Regulatory inquiry](docs/regulatory/README.md): pre-deployment CFTC packet,
  primary-source authority matrix, and meeting-request draft.
- [Protocol](docs/PROTOCOL.md): accounts, state transitions, and invariants.
- [Cryptoeconomics](docs/ECONOMICS.md): protected pools, liveness capitalization,
  fees, and adversarial incentives.
- [Cost model](docs/COST_MODEL.md): lower bounds and the cheapest useful design.
- [Shared accumulator plan](docs/ACCUMULATOR_PLAN.md): exact reusable path
  summaries, repair, retention, and source-admission gates.
- [Verification](docs/VERIFICATION.md): Rocq, Verus, SBF, and trusted boundaries.
- [Evidence matrix](docs/EVIDENCE_MATRIX.md): property IDs, proof layers,
  differential vectors, and mutation gates.
- [Static client](docs/STATIC_CLIENT.md): zero-operator frontend architecture.
- [Engineering plan](docs/ENGINEERING_PLAN.md): staged implementation and gates.
- [V1 backlog](docs/V1_BACKLOG.md): executable offline work packets and stop
  conditions.
- [Research agenda](docs/RESEARCH_AGENDA.md): bounded hypotheses, artifacts,
  falsifiers, and promotion decisions.
- [Benchmark plan](docs/BENCHMARK_PLAN.md): resource, relation, accumulator,
  cryptoeconomic, liveness, and client experiments.
- [Toolchain compatibility lab](docs/implementation/TOOLCHAIN_SPIKE.md): exact
  host/SBF probes, reproducibility evidence, and the current Verus stop gate.
- [Economics lab](docs/implementation/ECONOMICS_LAB.md): deterministic solvency,
  liveness, fee, failure, and price-collapse experiments.
- [Cost lab](docs/implementation/COST_LAB.md): 193 offline wire, account, CPI,
  rent, accumulator, and batch-layout scenarios with pinned evidence classes.
- [Static client prototype](docs/implementation/STATIC_CLIENT.md): a no-wallet,
  no-RPC static bundle with inspect-only unsigned intent construction.
- [Rocq specification status](docs/implementation/ROCQ_SPEC_STATUS.md): the
  handwritten state-machine shadow, named proof obligations, and current tool
  availability boundary.
- [Vertical model](docs/implementation/VERTICAL_MODEL.md): one deterministic
  lifecycle joining claims, batch fills, observations, resolution, fees, and
  prepaid liveness while preserving component boundaries.
- [Solana layout prototype](docs/implementation/SOLANA_LAYOUT.md): strict
  versioned account and unsigned-intent codecs, canonical IDs, size inventory,
  and an explicit no-entrypoint/no-CPI boundary.
- [Collateral profiles](docs/implementation/COLLATERAL_PROFILES.md): a
  collateral-generic Realm profile, conservative SPL/Token-2022 allowlist, and
  separate DREGG dogfood vector with no DREGG-only protocol branch.
- [Solana reference adapter](docs/implementation/SOLANA_REFERENCE_ADAPTER.md):
  strict byte parsing plus pure-kernel transitions with every runtime, CPI, and
  authority seam still deliberately fail-closed.
- `crates/clutch-kernel`, `crates/clutch-accumulator`, and
  `crates/clutch-batch`: self-contained offline prototype crates with focused
  tests and explicit nonclaims.
- [Architecture decisions](docs/adr/README.md): proposed and accepted decisions.
- [Provenance](docs/PROVENANCE.md): AGPL, dependencies, fixtures, and generated
  artifact policy.
- [Open questions](docs/OPEN_QUESTIONS.md): decisions intentionally not laundered
  into facts.
- [Security](SECURITY.md): threat model and disclosure posture.

## License

First-party source and documentation are licensed under
[`AGPL-3.0-or-later`](LICENSE). A dependency, fixture-provenance, notice, and
source-offer audit remains mandatory before the first public release.
