# Dragon's Clutch engineering handoff

Status date: 2026-08-19. The repository is an advanced local research and SBF
bring-up, not a release, deployment, or complete venue.

## Read order

1. [`AGENTS.md`](AGENTS.md) — authority, provenance, and correctness language.
2. [`PROJECT.md`](PROJECT.md) — canonical product semantics.
3. [`CURRENT_TRUTH.md`](CURRENT_TRUTH.md) — live evidence and STOP ledger.
4. [`docs/V1_BACKLOG.md`](docs/V1_BACKLOG.md) — dependency-ordered execution.
5. The owning design/implementation note for the lane you select.

This file is a compact transition aid. If it disagrees with
`CURRENT_TRUTH.md`, the latter controls. Historical detail remains available in
Git; do not revive an old test count or status sentence as current evidence.

## The semantic correction to preserve

Dragon's Clutch does not top out at one-hot outcome tokens.

- Degree zero is the native categorical basis over an exhaustive, disjoint,
  ordered state partition.
- Degrees one through three are native open-clamped B-spline Eggs. Their local
  supports overlap; exact nonnegative settlement weights sum to the frozen
  denominator.
- Exact coefficients over the selected native basis are the native payoff
  algebra. They are not “portfolio sugar.”
- Sampling or integrating a shaped payoff into degree-zero Eggs is a
  compatibility lowering. It must remain labeled and certified as such when
  approximate.

The control document is
[`docs/design/NATIVE_AND_LOWERED_SEMANTICS.md`](docs/design/NATIVE_AND_LOWERED_SEMANTICS.md).
Do not make a green categorical implementation the product's semantic ceiling.

## Accepted evidence, by plane

### Mathematical model

`lean/DragonsClutch/BSpline.lean` at commit `8c929a9` contains 159 counted
declarations, including 116 theorems, with no `sorry`, `admit`, axiom, `unsafe`,
`native_decide`, or `implemented_by`. It checks the rational clamped basis,
uniform stored-knot/pane/BasisFuns linkage, canonical largest-remainder
existence and uniqueness, integer admissibility, support, solvency, and
complete-set results. This is **PROVED-MODEL** only. Rust parser/control-flow/
Fraction/selection-loop equivalence, arbitrary nonuniform degree-one linkage,
accounts, source, and runtime remain outside it.

At `be8eba3`, eight Lean-computed fixtures match digest-pinned production
`BasisSpec::evaluate` outputs and five actual-source mutants compile, execute,
and go red. Call this **CHECKED-FINITE**, not a Verus result or universal Rust/
SBF refinement.

The older semantic-plane Lean inventory and narrow Verus
`prepare_internal_transfer` result retain their own documented boundaries.
Rocq definitions/typechecking are not a theorem inventory.

### Host and research

- `crates/clutch-bspline`: exact degree-zero through degree-three evaluator,
  safe `no_std`, no allocation/floats, largest-remainder quantization, and an
  independent Python `Fraction`/Cox-de-Boor differential.
- `programs/solana-reference`: conservative native-vector derivation seam;
  degree two/three non-point evidence and derived TWAP refuse.
- `research/bspline-shape-compiler`: exact-rational exact-in-span versus
  certified-approximation compiler for ranges/tails/tents/capped spreads and
  Gaussian proximity.
- `research/bspline-window-semantics`: point/interval/TWAP/occupation comparison
  and counterexamples to endpoint or midpoint shortcuts.
- `crates/clutch-bspline-accumulator`: fixed-width occupation monoid with
  explicit gaps and two separately named finalizers.
- `programs/solana-layout/src/native_resolution.rs`: 319-byte version-three
  native Resolution codec, selected for degree-one through degree-three Terms;
  degree zero retains the explicit 165-byte v2 preset ABI.
- `crates/clutch-liveness`: safe fixed-memory pure admission accounting for
  component market/order reserves, zero-fee work, source/archive sharing,
  replay-safe terminal ownership, anti-spam bounds, and persistent fee carry.
  Its maxima and rates are unmeasured inputs and it has no runtime adapter.
- `research/fractional-redemption`: safe fixed-width exact-lot and persistent-
  numerator-credit policy models. A resolved common lot is
  `lcm_i D/gcd(D,w_i)`; credits preserve the market aggregate liability but
  expose an irreducible terminal remainder absent subsidy, forfeiture, or a
  finer unit. No policy is selected or live.

These are **HOST-TESTED**, **MODEL-ONLY**, or **PROVED-MODEL** as named in
`CURRENT_TRUTH.md`. None is a live native-resolution SBF claim.

### Local SBF runtime

Focused real-bank paths now exist for:

- pooled Token-2022 custody and exact unreserved `WithdrawCash`;
- actual-mint supply truth, ordinary burn-as-forfeiture, and positionless
  transferred-holder `RedeemExternal`;
- typed resumable transport for policy, grid, and Terms artifacts;
- exact cash/internal-Egg order reservation plus one-shot cancellation;
- one narrow `SettlePage` consumption seam for a pre-frozen same-page,
  full-fill, direct single-Egg, zero-fee receipt. It joins selected candidate,
  canonical CandidateFeed, and two ACTIVE reservations; the focused success
  consumed 862,107 transaction CU in the latest joined rerun;
- one pre-fund-safe `SubmitDirectPage` constructor for a frozen two-order,
  two-outcome, equal-limit, zero-fee direct page. It creates an exact feed and
  SUBMITTED Candidate in 1,249,403 transaction CU, but leaves Epoch FROZEN and
  the score/digest zero and unverified; and
- degree-one through degree-three source-joined point Resolve, sole-vector v3
  persistence, exact retry/conflict, and exact-lot internal and positionless
  bearer redemption. Seven focused real-SBF scenarios pass; nondivisible
  quantities refuse without rounding. Source accounts are still genesis mock
  fixtures, not a live ingestion lifecycle.

The joined signed walk at source HEAD `c05fe84` used ELF
`70c33c1cd44b475745b0562a79d9107f1d2101cbf698ebd6c233ca167ebab2e6`.
It committed 22 signed confirmed transactions, including two expected
refusals, reloaded 18 watched accounts, separated market resolution replay from
owner replay, redeemed internal and bearer claims, withdrew both owners' free
cash, and ended both Position cash balances and the Hoard token balance at
zero. A corrupted terminal Hoard-token expectation failed specifically on
committed bytes.

That is **SBF-EXECUTED local evidence**. The walk injects 11 Clutch-owned
prerequisites and never clears or settles an order epoch. It is neither a
blank-bank lifecycle nor end-to-end venue evidence.

## Accounting model

For one market:

```text
H = actual Hoard collateral tokens
L = retained claim backing
P = aggregate Position cash
R = reserved cash, a subset of P
S = unsolicited unowned surplus

H = L + P + S        and        0 <= R <= P
```

Split, Merge, and internal redemption are token-neutral pooled-accounting
reclassifications. Endow and Withdraw are the owner/Hoard Token-2022 boundary.
External redemption burns a bearer Egg and transfers its exact payout.
Reserved Eggs remain in the claim-supply identity, not the collateral equation.
Hoard donations and direct Egg burns create no fee, sweep right, or treasury
asset. Hoard principal is never rent, liveness, bounty, or revenue.

## Live STOPs

1. Degree-one through degree-three source-joined point Resolve, v3 vector
   persistence/replay, and exact-lot internal and positionless bearer
   redemption are live. Other post-resolution consumer audit, production
   source ingestion, the native window/occupation path, final-LTO stack repair,
   and a total fragment/credit policy remain open.
2. Both the public account-shaped reference adapter and SBF Split/Merge/
   materialize/dematerialize still reconstruct mode-less persisted Kernel state
   as `FinitePreset`. Reachable active states preserve the native bound, but
   this is a P1 representation/refinement gap. Bind a Terms-checked immutable
   basis-mode projection before release.
3. Resolve now derives and authenticates canonical SourceSpec/archive PDAs and
   requires the caller's compatibility projection to equal the sealed archive.
   No production provider/parser, onchain create/append/seal path, immediate
   receiver-post/CPI/config provenance, live Clock/feed admission, or multi-page
   proof exists. The focused bank still injects deterministic mock source state
   at genesis. Typed artifact transport does not solve ingestion.
4. A bank with no injected Clutch account can now seal policy/grid/Terms,
   create Realm/Profile, and create the complete initial degree-selected market
   plane. It cannot create the full source/archive/Epoch/candidate/pot/receipt
   lifecycle. Terms also does not consensus-check referenced Grid existence.
   Existing Market, Reservation, and later-owner families now have real-bank
   pre-fund/rollback coverage; every future constructor must inherit the same
   allocate/assign rule and regression gate.
5. `SubmitDirectPage` now constructs one narrow SUBMITTED Candidate/feed and
   `SettlePage` executes one separately preauthorized direct receipt. Candidate
   completion/scoring/window closure/selection, receipt/pot/entitlement
   construction, frozen global reservation-set closure, partial/portfolio/
   virtual/fee paths, permissionless lapse/refund, and terminal sweep remain
   open; the seams are not reachable end to end.
6. Mandatory work is not fully measured and prepaid under zero future volume;
   fee/failure policy is not frozen.
7. The repository lacks one clean schema-v2 evidence baseline, independent
   rebuild, complete SBOM/license record, external security review, and release
   bundle.
8. Gate L0 remains open. No engineering artifact authorizes public-network use,
   filing, regulator contact, or real funds.

## Recommended execution order

Follow the first dependency-unblocked item in `docs/V1_BACKLOG.md`:

1. settle the shared tree and produce one noncontradictory evidence baseline;
2. audit every native post-resolution consumer and repair the shared final-LTO
   stack survivors without weakening exact-lot or source STOPs;
3. implement the full blank-bank production source/archive lifecycle;
4. join funded reservations to epoch freeze, candidate selection, immutable
   entitlements, and `SettlePage`;
5. measure/prepay liveness and freeze economics only from final instruction
   shapes; and
6. run adversarial, proof, SBF, artifact, independent-build, and release gates.

Each lane must name its falsifier before editing. Run the narrowest test capable
of making the claim red. Never edit a vector or relax a refusal merely to get a
green gate.

## Operational boundary

Run `git status --short` before choosing files: parallel lanes may share this
worktree. Add and commit only explicitly owned paths; ordinary local commits
need no extra permission. Do not push, tag, publish, deploy, contact a public
RPC, sign with a real wallet, fund anything, create a public market, or contact
a regulator without explicit current authorization naming that act.
