# Omission index — what is deliberately not built

Each row names one thing the predecessor did, or one tempting design, that
dClutch does not build, and the accepted treatment instead. A row closes only
by a cited decision record; silence, missing code, a fixture, or an
unimplemented client path cannot close one. Rows are classified as a **hard
invariant** (no profile may weaken it), a **current safe profile** (a
conservative executable choice with a named lifting path), a **likely scar**
(a restriction inherited from convenience, scheduled for a stronger design), or
**open research** (a generalization whose authority is not yet sound).

What completion still owes is not here: that is `docs/MASTER_COMPLETION_CONTRACT.md`
rows C-00..C-16. An `unfinished` capability is never an omission, and the
former `U-` rows of this file (fifteen `unfinished` capabilities, never once
moved in four days) were retired on 2026-09-04 — their subjects are the
contract's rows, which carry the evidence.

## Rejected designs and invariants

| ID | Not built | Class | Accepted treatment | Record |
| --- | --- | --- | --- | --- |
| O-001 | An eager universal Market foundation containing every venue, wrapper, resolution, recovery and funding account | current safe profile | Thin Core; eager atomic creation of the exact selected capability subset | [0001](decisions/0001-thin-market-core.md) |
| O-002 | A cumulative historical action namespace and compatibility for experimental layouts | current safe profile | Pre-release wires have no compatibility privilege; a release states its compatibility and migration policy | — |
| O-003 | One deployed program per family or market, a dynamic capability-to-program map, or an on-chain code-stamping factory | current safe profile | Five state-owning roles; a descriptor may later run through a Registry-authenticated interpreter or a translation-validated AOT accelerator | [0003](decisions/0003-fixed-role-capability-execution.md) |
| O-004 | Closed Rust family dispatch, permanent family enum positions, `N = 2..16` monomorphization | likely scar | Descriptor semantics are canonical; interpretation and checked AOT are alternative strategies | [0006](decisions/0006-family-neutral-hot-dispatch.md) |
| O-005 | Parallel old/new decoders, authority paths, DTO truths or fallback executors | hard invariant | One live writer and one persisted truth per fact; read decoders, migrations and non-authoritative measurement ELFs are allowed | `AGENTS.md` |
| O-006 | Hard-coded DREGG token behaviour or mandatory house collateral | hard invariant | Immutable Realm/profile identity selects collateral; token names never select semantics | — |
| O-007 | Mock provider authority, client/index authority, or caller-authored resolution truth | hard invariant | Mocks are test-only; release state plus provider-authenticated evidence owns truth | [0027](decisions/0027-recovery-is-one-funded-ordered-ladder.md) |
| O-008 | Hoard principal, future fees or unclassified donations funding rent, work, liveness, liquidity, bounty, reserve or treasury | hard invariant | Hoard and anticipated revenue never capitalize liveness; a donation enters only through a typed classification transition | [0024](decisions/0024-sustainable-economics-and-a-governable-parameter-surface.md) |
| O-009 | A cross-asset `total_principal`, implicit conversion, or exchange rate between lamports and Realm collateral | hard invariant | Conserve each asset independently; a valued-risk capability must name oracle, units, timing, rounding and failure behaviour | — |
| O-010 | Undercollateralized claims, uncovered Dealer inventory, protocol credit promises, leverage against anticipated revenue | likely scar | Native claims stay solvent; Dealer may generalize from gross coverage to exact terminal-scenario coverage; defaulting credit is open research | [design](design/dealer-v2-scenario-collateral.md) |
| O-011 | Dealer sponsor RFQ compatibility, mutable administrator repricing, configuration changes that revalue incumbent LPs | likely scar | Consent-bound capital tranches, deterministic accepted pricing, quiescent epoch transitions | [0034](decisions/0034-ensemble-flagship-parameters.md), [design](design/MECHANISM_SCORING_DEALER_2026_09_04.md) |
| O-012 | Fractional native liabilities, residual credits, remainder ledgers, hidden redemption rounding | likely scar | Hidden rounding stays forbidden; exact claim shards and explicit Token-owned remainder instruments are valid | [0011](decisions/0011-structured-v2-physical-route.md) |
| O-013 | Polynomial, B-spline, ramp or tent liabilities inside the elementary claim basis | likely scar | Categorical is the optimized `Q=1` basis. Degree-0 and degree-1 shapes are live on the wire under a certified categorical projection; the unreached capability is curvature (degrees 2 and 3), whose evaluator authority is the ruling [`BASIS_ABI_UNIFICATION_V1`](design/BASIS_ABI_UNIFICATION_V1.md) asks for | [0029](decisions/0029-the-product-list-nine-rulings.md) |
| O-014 | Wrapper nesting, recursive payout authority, wrapper-specific collateral truth, wrapper-owned resolution | likely scar | One native liability/resolution truth; acyclic compositional recipes with canonical flattening and a bounded representation DAG; live wrapper-on-wrapper custody is open research | [0011](decisions/0011-structured-v2-physical-route.md) |
| O-015 | A bespoke holder ledger, privileged wrapper transfer, transfer hooks, permanent delegates, silent supply reconciliation | likely scar | No shadow supply; Token behaviour selected by a versioned profile; risky extensions need their own proofs and rollback campaigns | [design](design/TOKEN_2022_IMMUTABLE_OWNER_DESTINATION_2026_09_02.md) |
| O-016 | A caller or static client supplying executable program identity, physical effect plans, semantic ids or trusted projections | hard invariant | Callers supply hints, witnesses, candidates and physical accounts; release-selected state verifies every authoritative identity | [0016](decisions/0016-checked-release-identity.md) |
| O-017 | Calling the selected General result "optimal clearing" without a checked optimality certificate | hard invariant | Say "best valid submitted candidate" unless a descriptor admits and verifies a named certificate | [0032](decisions/0032-joint-clearing-residual-tie-break-and-seal.md) |
| O-018 | Instructions-sysvar adjacency as the authority joining provider publication to Source consumption | hard invariant | Adjacency is not authority; same-instruction CPI is the conservative Pyth profile | [compost](compost/PYTH_LOCAL_UPGRADED_2026_08_22.md) |
| O-019 | Widening the batch relation toward a general encrypted-exchange computer | hard invariant, load-bearing by ruling | The batch relation is small and specialized on purpose; it is the door the privacy ruling deliberately left open, and it does not close | [0018](decisions/0018-privacy-horizon-not-this-clutch.md), `docs/INTENT.md` §1 |

## Current physical profile restrictions

Not omissions: fixed bounds with a named lifting path, each classified in
[the cliff doctrine](design/CLIFF_DOCTRINE_V1.md).

| ID | Current profile | Class | Lifting path |
| --- | --- | --- | --- |
| P-001 | Product V1 admits at most sixteen outcomes | current safe profile | PURCHASABLE: erase the remaining width dispatch, measure contiguous runtime views, page only where packet, account or CU evidence requires it |
| P-002 | Capability manifest profile 1 admits sixteen entries and sixteen dependencies per entry | current safe profile | A finalized ordered paged graph with one aggregate commitment |
| P-003 | The checked transition program has a finite instruction bound | current safe profile | SESSION-SPLITTABLE: wider profiles, staged computation certificates and AOT; the final economic commit stays bounded and atomic |
| P-004 | Bearer/Structured V1 uses zero-decimal, narrowly admitted Token-2022 layouts | likely scar | Keep the strict default; add separately versioned Token behaviour profiles |
| P-007 | The capability seal's persisted layout is hand-authored Rust, not Lean-emitted and byte-guarded | likely scar | The same emitter-plus-`check-generated.sh` shape the capability program contract already has, applied to `DCLTCSL1`, and the TypeScript mirrors pinned to it ([0005](decisions/0005-per-market-authentication-cache.md) item 1) |
| P-008 | The write-once `ProtocolInfrastructureProfileV1` makes Registry and Rent structurally non-upgradable in place | likely scar, lifting ruled | Profile succession (`V2`, a ceremony strictly stronger than initialize) is built and red-proofed on real ELFs and has never executed on a chain; its route is `blocked by rule` in `docs/reference/routes.md`. Devnet is disposable by ruling, so the ceremony is a capability, not a blocker ([ruling](design/PROFILE_UPGRADE_RULING_2026_08_31.md), [0012](decisions/0012-devnet-iteration-substrate.md)) |

Closed rows, kept by identifier only: **P-005** (the generation-scoped
RentCredit, lifted 2026-08-27 by `LifecycleRentCreditV2`) and **P-006** (the
write-once seal now has a close route that pays the closer, ruled 2026-08-31,
[0005](decisions/0005-per-market-authentication-cache.md)).

## Maintenance rule

A decision that rejects a design, accepts a successor, or adds a profile
restriction updates this file in the same commit. A row leaves this file only
with the record that closes it named beside its identifier.
