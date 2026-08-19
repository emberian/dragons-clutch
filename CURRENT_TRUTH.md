# Dragon's Clutch: current truth and control plane

Status date: 2026-08-19. This is the operational entry point for engineering
handoffs. [`PROJECT.md`](PROJECT.md) is the canonical product brief,
[`AGENTS.md`](AGENTS.md) is the authority and correctness policy, and
[`docs/V1_BACKLOG.md`](docs/V1_BACKLOG.md) is the dependency-ordered queue.

This file supersedes current-status, test-count, and next-work claims in
[`GOAL.md`](GOAL.md), [`CODEX_HANDOFF.md`](CODEX_HANDOFF.md),
[`CLAUDE_HANDOFF.md`](CLAUDE_HANDOFF.md), and dated drift reviews. Historical
documents remain useful evidence pointers; they are not a live promotion
ledger.

## 1. Claim vocabulary

These labels are deliberately nontransitive:

| Label | Exact meaning |
| --- | --- |
| **PROVED-MODEL** | A named proof assistant checked a theorem about a named mathematical model. It says nothing about Rust, accounts, CPI, SBF, or runtime unless a separate refinement closes that boundary. |
| **CHECKED-RUST-SUBSET** | A pinned verifier checked a named Rust source subset under recorded assumptions. It says nothing about code outside that subset. |
| **CHECKED-FINITE** | Digest-bound model-computed fixtures agree with production execution on a named finite corpus and named source mutants go red. This is not a universal refinement theorem. |
| **HOST-TESTED** | Ordinary host execution, differential testing, or a bounded finite campaign passed. This is executable evidence, not proof or SBF evidence. |
| **SBF-EXECUTED** | A compiled SBF program executed in a local Agave bank or loopback validator. This is not deployment, public-cluster, audit, or mainnet evidence. |
| **MODEL-ONLY** | A design, reference model, research tool, or cost experiment exists but is not the production transition. |
| **PROPOSED** | A design or policy has not crossed its promotion gate. |
| **IN-FLIGHT** | Shared dirty-worktree bytes are not an accepted baseline. |
| **STOP** | The named surface must refuse or remain undescribed as complete until its acceptance gate closes. |

“Implemented” is too broad for this control plane. Every promoted claim below
names its evidence plane. Passing a differential proves agreement on that
corpus; it does not prove that both implementations express the intended
economics.

## 2. Snapshot boundary

The accepted local history through `ae2e155` includes the native B-spline
kernel and reference seam, compiler and window research, committed Lean model,
pooled cash withdrawal, typed artifact transport, funded order reservations,
global resolution replay separation, isolated native-resolution layout, shared
ABI integration, and the pure occupation accumulator. The worktree may contain
later integration lanes; those bytes are **IN-FLIGHT** until separately
committed and checked.

Runtime evidence is artifact-specific:

- the historical 20-step signed committed walk at `882204f` executed ELF
  `98cac8a1e48f629f15d0efbf6295b2c96df5296f6acf6cec28ca76491da4b391`;
- the focused withdrawal bank campaign executed ELF
  `23139487e1a38de73a7f0077fb87cc28a1f1968a9dc8db0e2f5babcd09ebce41`;
- typed artifact and funded-reservation campaigns executed their then-current
  real SBF ELFs in local `solana-program-test` banks; and
- a clean-source 22-step signed, sequential, genesis-assisted local-validator
  walk at source HEAD `c05fe84` executed the joined global-resolution and
  terminal-withdrawal path against ELF
  `70c33c1cd44b475745b0562a79d9107f1d2101cbf698ebd6c233ca167ebab2e6`.
  All 22 transactions reached confirmed commitment, including two expected
  refusals; 18 watched accounts were reloaded, both owners' Position cash and
  the pooled Hoard token balance ended at zero, and a corrupted step-22 Hoard
  expectation made the gate fail specifically on committed bytes. The walk
  still had 11 genesis prerequisites and is focused **SBF-EXECUTED** evidence,
  not a blank-bank venue or the complete schema-v2 baseline. The exact record
  is [`docs/implementation/COMMITTED_SBF_WALK.md`](docs/implementation/COMMITTED_SBF_WALK.md).

The checked-in `MANIFEST.baseline.json` remains a historical schema-v1
manifest. No clean schema-v2 baseline, checked release manifest, signed tag,
independent rebuild, public-network deployment, official client URL, or
value-bearing market exists. Nothing here authorizes signing, funding,
deployment, publication, regulator contact, or an “official” claim.

The scoped adversarial review at `f48b13c` found no hidden active P0 in the
reviewed artifact, reservation, replay, withdrawal, or spline transitions. It
did find the cross-cutting predictable-PDA pre-funding P1 and reaffirmed the
source, native-resolution, and settlement STOPs. See
[`docs/reviews/OVERNIGHT_INTEGRATION_REDTEAM.md`](docs/reviews/OVERNIGHT_INTEGRATION_REDTEAM.md).
Artifact stage/final creation closed its part of that P1 at `e7d975b`; the
typed policy/Realm/Profile/market plane closed its part at `ceac012`/`7cf7150`;
and `a274bef` exercised Market, Reservation, and second-owner Position/Replay
prefunds in six real-bank cases. Over-rent donations remain unowned; hostile
owner/data/executable targets refuse; duplicate and late Token-2022 failure
roll back byte-exactly. The known existing creation families have therefore
closed F-01, but every future constructor must preserve the same gate. This is
a review/test result over named commits, not an audit or proof of absence.

## 3. Native claim semantics

Three constructions must remain separate:

1. **Native basis.** Degree zero is an exhaustive, disjoint, ordered
   categorical partition. Degrees one through three are open-clamped B-spline
   Eggs with overlapping local support, nonnegative weights, and exact
   partition of unity.
2. **Exact coefficient algebra.** A payoff in the selected finite spline span
   is represented exactly by coefficients over native Eggs. A vector is not
   automatically an approximation.
3. **Categorical compatibility lowering.** Sampling or integrating a shaped
   payoff over one-hot degree-zero Eggs is an adapter. It must carry an error
   statement when it is not exact and must never redefine the smooth product.

The exact point evaluator in `crates/clutch-bspline` is safe, `no_std`,
allocation-free, float-free Rust for degrees zero through three. It uses exact
rational basis evaluation and deterministic largest-remainder quantization
with lowest-index ties. Host tests and an independent Python `Fraction`/
Cox-de-Boor campaign support it: **HOST-TESTED**, not proved Rust.

The committed Lean file `DragonsClutch.BSpline` at `8c929a9` contains 159
counted declarations, including 116 theorems, with no `sorry`, `admit`, axiom,
`unsafe`, `native_decide`, or `implemented_by`; its reported theorem axioms are
only Lean's `propext`, `Classical.choice`, and `Quot.sound`. It checks
degree-one through degree-three clamped rational constructions, uniform
stored-knot expansion and pane/internal-boundary linkage, exact BasisFuns split
distances, constructive and unique canonical largest-remainder selection,
integer admissibility, local support, residual bounds, solvency, and
complete-set results: **PROVED-MODEL**. Rust parser/control-flow/Fraction/
selection-loop equivalence remains an explicit refinement boundary, and the
linkage theorem does not cover arbitrary nonuniform degree-one grids.

At `be8eba3`, eight Lean-computed fixtures agree byte-for-byte with the
digest-pinned production `BasisSpec::evaluate`, and five mutations of the
actual Rust source compile, execute, and go red. The campaign also reruns the
34,766-case Python differential. This is **CHECKED-FINITE** evidence. It invokes
no Verus theorem and does not prove the whole evaluator, parser/refusal order,
overflow behavior, compiler, SBF, or runtime.

The surrounding layers have different status:

- `research/bspline-shape-compiler` is a host-tested exact-rational research
  compiler for exact-in-span shapes and certified approximations, including
  ranges/tails, tents, capped call/put spreads, and Gaussian proximity:
  **MODEL-ONLY / HOST-TESTED**;
- `research/bspline-window-semantics` compares point, interval, TWAP, and two
  occupation meanings and records why an arbitrary midpoint is invalid:
  **MODEL-ONLY / HOST-TESTED**;
- `crates/clutch-bspline-accumulator` is the pure fixed-width occupation
  monoid over quantized native basis points, with explicit gaps and exact or
  separately named largest-remainder finalization: **HOST-TESTED**, not source
  authenticated or integrated; and
- `research/fractional-redemption` is a safe fixed-width exact policy model:
  the resolved common lot is `lcm_i D/gcd(D,w_i)` (for example
  `[16,40,8]/64` has common lot 8), while persistent numerator credits preserve
  `D*C >= remaining_weighted_liability + aggregate_credit`. It proves that a
  terminal aggregate numerator remainder cannot be swept without subsidy,
  forfeiture, or a finer unit: **MODEL-ONLY / HOST-TESTED**; and
- `programs/solana-reference` derives native vectors for degrees one through
  three under its conservative evidence rules: **HOST-TESTED reference**.

The 319-byte version-three native Resolution codec is selected by smooth Terms
while degree zero retains the 165-byte version-two preset ABI. At `cae3d90`
atop the source join in `0b96a3a`, degree-one through degree-three **point**
Resolve persists the sole native vector; exact retry rederives it; exact
internal and positionless bearer redemption reconstruct it ephemerally. Bearer
authority is Token-2022 possession and signature, and success burns the exact
lot, transfers the exact payout, and updates mint supply, aggregate liability,
recorded collateral, and token balances once. Nondivisible lots return
`RemainderRequired` before mutation; late transfer failure rolls back the burn.

Resolve now derives and authenticates canonical SourceSpec and sealed
SourceArchive PDAs from immutable identities, verifies their owner, bump,
spec/provider/parser/deployment generation, grid/window, lineage, page
commitment, and sealed cursor, and requires every legacy projection value to
equal the archive. Thus the legacy blob remains transport but no longer value
authority. The joined native real-SBF campaign passed 7/7. For degrees one,
two, and three, resolve/retry/internal/bearer CU were respectively
`1092607/938965/708253/788032`, `1130866/977224/705753/785332`, and
`1166139/1012497/705428/784537`. The independent post-join audit at
`ae2e155` reran 135/135 host tests and those 7/7 cases against focused ELF
`e448f1a9a5fe7c80b2d8ece939dab059ef64ccadab11fa5952328cd31ed35a32`.
That digest is evidence for this focused campaign, not a clean release manifest.

This is not general native settlement yet. Non-point evidence refuses. Source
accounts are still genesis-injected in the focused bank: production has no
onchain create/append/seal route, immediate provider receiver-post/CPI/config
authentication, or live Clock/feed admission, and the deterministic adapter is
a mock. The one-window archive is capped at 32 records. Other post-resolution
consumers still need an explicit audit, the active Kernel mode P1 below remains,
and the current ELF retains final-LTO stack diagnostics in shared dispatch,
`split::kernel_step`, and `observe_resolve::pure_market`. No path may lower a
smooth market to categorical portfolios.

The v4 lifecycle audit also found a P1 active-mode representation gap: both the
public account-shaped Solana reference adapter and SBF Split/Merge/materialize/
dematerialize reconstruct the mode-less persisted Kernel as `FinitePreset`;
the SBF account planes receive neither Terms nor Resolution. No valid-origin
value-extraction trace was found, and the active transitions preserve the
stronger native collateral bound, but both adapters check the wrong invariant
and could admit corrupted or future-reachable state. Bind an immutable
Terms-derived basis-mode projection before release. The same audit repaired
public `derive_payout` to be degree-zero-only; smooth callers must use
`derive_payout_vector` and can no longer cross a preset-membership bridge.

## 4. Capability matrix

| Surface | Strongest honest status | Established fact | Boundary / STOP |
| --- | --- | --- | --- |
| Product and Realm model | **PROPOSED** | The product is collateral-generic; DREGG is one optional dogfood profile. Native degree-zero through degree-three claim semantics are the intended ceiling. | No real Realm profile is authenticated, frozen, or released. |
| Core claim kernel | **HOST-TESTED** plus separate **PROVED-MODEL** results | Safe fixed-layout Rust executes split, merge, materialize, dematerialize, resolution, and redemption fragments. Lean checks named model properties. | Lean/Rust correspondence is manual; the full kernel is not verifier-checked. |
| Verus refinement | narrow **CHECKED-RUST-SUBSET** | Pinned Verus checked exact debit/credit conservation and overflow refusal for `prepare_internal_transfer`, with source/call-site digests and red mutations. | Accounts, phases, codecs, CPI, SBF codegen, and runtime are outside the result. |
| B-spline model/executable bridge | **CHECKED-FINITE** | Eight Lean-computed vectors match digest-bound production evaluator outputs; five actual-source semantic mutants compile/execute and disagree. | No Verus invocation or universal Rust/SBF refinement; finite adapter association remains reviewed. |
| General accumulator | **HOST-TESTED** | Source-neutral adjacent summaries, coverage, interval, TWAP, and terminal calculations have bounded tests. | It authenticates no source, clock, archive, or deployment generation. |
| Native spline stack | point Resolve/exits **SBF-EXECUTED**; broader mixed, see §3 | Degree-selected v2/v3 creation, source-joined exact d1–3 point resolution, sole-vector persistence/replay, and exact-lot internal and bearer redemption execute; evaluator, proofs, finite bridge, compiler/occupation, and policy models retain their named planes. | Active Split-family mode binding is P1. Production source ingestion, other consumer audit, non-point semantics, final-LTO repair, and clean joined evidence remain open. |
| Coupled batch relation | **HOST-TESTED** | Exact witness checks, bounded candidate comparison, pairing, conservation, and a bounded streaming verifier have finite/adversarial campaigns. | It supports “best valid submitted candidate,” not globally optimal search. |
| Funded order admission | **SBF-EXECUTED** focused path | `PlaceOrder` creates a canonical pre-fund-safe per-order reservation and encumbers exact cash or internal Eggs. `CancelOrder` tombstones once and releases only that reservation. Split and Withdraw cannot spend reserved cash. | No frozen reservation-set commitment, permissionless lapse, or general candidate-to-entitlement transition exists. |
| Pooled custody and cash exit | **SBF-EXECUTED** focused paths | Endow is the inbound token boundary; Split/Merge/internal redemption are pooled-accounting reclassifications; exact unreserved `WithdrawCash` performs Hoard-to-owner Token-2022 transfer. Donation, reservation, rollback, and two-owner cases execute in a local bank. | A clean joined baseline and full venue settlement campaign remain open. |
| Outcome-token truth and bearer exit | categorical and exact native lots **SBF-EXECUTED** | Actual Token-2022 mint supply is authoritative; ordinary burns are recognized as forfeiture; transferred positionless degree-zero Eggs and exact-lot d1–3 Eggs redeem through `RedeemExternal` in focused local-bank evidence. | Nondivisible native fragments refuse. The total lot/credit policy, full lifecycle, final-LTO repair, and clean committed artifact remain open. |
| Typed artifact transport | **SBF-EXECUTED** | Policy, grid, and Terms use exact typed lengths, ordered 192-byte chunks, restart, seal, abort/reap, idempotent reseal, and rent return. At `e7d975b`, exact rent-shortfall transfer plus PDA-signed allocate/assign closed one-lamport and over-rent stage/final squatting in six real-SBF cases. Native SHA preserves the portable preimage relation. | The transport is not generic and does not authenticate source/archive/clearing artifacts. Excess target lamports remain an unowned donation, not protocol authority. |
| Account construction | initial market plane **SBF-EXECUTED**; wider lifecycle **STOP** | From a bank with no injected Clutch account, a wallet seals policy/grid/Terms, creates Realm/Profile, and creates all initial market state/token PDAs. Degree-zero v2/165 used 916,052 CU and degree-one v3/319 used 909,302 CU. Existing Market/Reservation/later-owner families have real-bank pre-fund and rollback coverage; one narrow Candidate/feed constructor exists. | Terms does not consensus-check referenced Grid existence. Feed/archive/general Epoch/candidate/pot/receipt construction remains incomplete; every new constructor must inherit the pre-fund-safe pattern and tests. |
| Resolution replay | focused **SBF-EXECUTED** | Market-global resolution no longer consumes an owner's replay sequence; exact retry is idempotent and conflicting retry refuses. The subsequent owner redemptions/withdrawal retain their own sequence. The current native Resolve also authenticates the sealed archive described below. | Replay separation alone does not establish production source ingestion or a joined blank-bank lifecycle. |
| Source admission and archive | Resolve join **SBF-EXECUTED**; ingestion **STOP** | A 292-byte content-addressed SourceSpec and 2,560-byte one-window archive bind provider/parser/deployment/spec/grid/window/lineage and a sealed receipt. Resolve derives and authenticates their canonical PDAs and requires the compatibility projection to equal the sealed archive; same-domain value substitution and wrong-archive PDA refuse in the 7/7 native campaign. | Production cannot create, append, or seal the accounts. There is no production provider/parser, immediate provider receiver-post/CPI/config provenance, live Clock/feed admission, or multi-page proof; the focused bank injects mock source state at genesis. |
| Onchain clearing/settlement | narrow **SBF-EXECUTED** seams; full lifecycle **STOP** | `SubmitDirectPage` can pre-fund-safely create one deterministic SUBMITTED Candidate and exact CandidateFeed from a frozen two-order/two-outcome, equal-limit, zero-fee direct page with ACTIVE reservations. Separately, `SettlePage` can consume one pre-frozen same-page, full-fill, direct single-Egg, zero-fee receipt against a selected Candidate, canonical feed, and those reservations. Both focused campaigns pass 2/2 in real SBF; the joined rerun used 1,249,403 and 862,107 transaction CU respectively. | Submission deliberately leaves Epoch FROZEN and score/digest zero and unverified. Candidate-window closure, scoring/selection, receipt/pot/entitlement construction, global reservation-set closure, partial/portfolio/virtual/fee paths, lapse/refund, and terminal sweep remain open, so the two seams are not yet reachable end to end. |
| Signed committed walk | 22-step **SBF-EXECUTED** at `c05fe84` | Fresh local keys signed 22 confirmed sequential transactions through global resolution, internal/bearer redemption, and both owners withdrawing all free cash; 18 watched accounts were reloaded and the corrupted terminal expectation went red. | It is genesis-assisted by 11 prerequisites and omits clearing/settlement. It is not a blank-bank lifecycle or release baseline. |
| Static Glass | **HOST-TESTED** inspect-only prototype | A static client can render local terms and unsigned intent material without owning truth. | No frozen release manifest, complete wallet path, browser/accessibility audit, or official hosted instance. |
| Prepaid liveness accounting | **HOST-TESTED pure kernel** | `clutch-liveness` admits component-wise market/order reserves even at zero fees; models replay-safe work/storage terminal identities, atomic equal-share source/archive joins and refunds, anti-spam bounds, and persistent intent fee carry without treating fees as work capital. | Maxima/rates are unmeasured inputs; there is no account codec, authenticated funding, System transfer, replay adapter, SBF path, or inclusion guarantee. |
| Economics and fees | **MODEL-ONLY / PROPOSED** | Synthetic solvency, cost, fee, manipulation, and allocation experiments exist. | Fee base/rate/split, measured liveness maxima, neutral-failure policy, and recipient policy are not frozen. Hoard principal is never available. |
| Artifact/release evidence | tools **HOST-TESTED**; release **STOP** | Manifest, vector, rebuild, artifact-audit, and local review tools exist. | Current manifest is stale; no clean joined baseline, SBOM/license closure, independent rebuild, audit, or release bundle exists. |

## 5. Accounting truth

For one market let:

```text
H = actual collateral atoms in the Hoard Token-2022 account
L = HoardAccount.collateral_atoms, retained claim backing
P = sum of every Position.cash_atoms
R = sum of every Position.reserved_cash_atoms, with 0 <= R <= P
S = unsolicited unowned Hoard surplus

H = L + P + S
```

Reserved cash is a subset of Position cash, not an additional custody term.
Reserved Eggs remain in the claim-supply identity. Direct token donations
increase only `S`; claim burns may reduce required liability while leaving `L`
conservatively retained. Neither creates a fee, treasury asset, sweep right, or
Position credit.

| Transition | Token effect | Accounting effect |
| --- | --- | --- |
| `Endow(q)` | actor `-q`, Hoard `+q` | owner cash `+q` |
| `Split(q)` | none | free cash `-q`, locked backing `+q`, every native claim supply `+q` |
| `Merge(q)` | none | every native claim supply `-q`, locked backing `-q`, free cash `+q` |
| internal redemption | none | claim `-q`, locked backing `-p`, owner cash `+p` |
| reserve/release | none | exact movement between free and reserved ownership phases |
| `WithdrawCash(q)` | Hoard `-q`, owner `+q` | unreserved owner cash `-q` |
| external redemption | burn Egg; Hoard `-p`, bearer `+p` | external liability `-q`, locked backing `-p` |

The market collateral cap bounds locked claim backing, not unrelated free cash
or unsolicited surplus. Local instructions enforce exact deltas and at least
`H >= L`; the full equality is an inductive market-wide obligation.

## 6. Non-negotiable STOP ledger

1. **Bind native mode across the active lifecycle:** cache a Terms-checked
   immutable basis mode in Kernel state or present/authenticate Terms on every
   Split/Merge/materialize/dematerialize seam. Add mode-flip, wrong-Terms,
   derived-active-solvency, and resolved-native phase-refusal tests.
2. **Complete native live semantics:** degree-selected blank-bank creation,
   source-joined point Resolve, sole-vector persistence, exact replay, and
   exact-lot internal and bearer redemption are live for degrees one through
   three. Audit every post-resolution consumer, repair the final-LTO stack
   survivors, and freeze the fragment/credit policy promised to bearers.
3. **Authenticated source/archive ingestion:** pin one concrete source program,
   parser, and deployment profile; publicly create, append, and seal its
   canonical history; authenticate the immediate provider/config state and
   Clock admission; and remove genesis-injected mock source prerequisites.
4. **Complete blank-bank lifecycle:** typed artifacts, Realm/Profile, the
   initial degree-selected market plane, Reservation, and later-owner state now
   tolerate admitted pre-funding with focused real-bank rollback evidence. Add
   a consensus Terms-to-Grid existence join, preserve that constructor rule for
   every new account family, then publicly create and fund source/feed/archive, Epoch/pages,
   candidate/checkpoint, pot/receipt, and cleanup state.
5. **Coupled settlement:** one narrow direct subset can now construct a
   SUBMITTED Candidate/feed, and one separately preauthorized direct receipt
   can be consumed in SBF. The full gate still must freeze the exact live
   reservation set, complete and score claims, close the candidate window,
   verify and select the best valid submitted candidate, create every immutable
   entitlement before resolution, support the admitted intent/fee shapes, and
   consume/refund everything exactly once.
6. **Prepaid liveness and economics:** measure final SBF paths and capitalize
   every mandatory unfinished action at admission under zero future volume.
   The pure admission kernel supplies exact component/accounting relations but
   no measured maxima or runtime funding path. Principal and owner assets never
   fund this work.
7. **Evidence promotion:** rerun the joined host, proof, SBF, Token-2022,
   committed-walk, stack, artifact, and negative gates on one clean tree; emit
   and verify schema-v2 evidence, then obtain independent rebuild and security
   review before a release claim.
8. **Gate L0:** exact legal/entity/control/deployment facts, qualified advice,
   any required relief, and separate current user authorization remain outside
   engineering. No meeting, filing, proof, or local run closes this gate.

## 7. Handoff loop

1. Read `AGENTS.md`, `PROJECT.md`, this file, and `docs/V1_BACKLOG.md`.
2. Run `git status --short`; shared dirty bytes belong to their active owners.
3. Take the first dependency-unblocked STOP and state the exact falsifier.
4. Run the narrowest host/proof/runtime test capable of refuting the change.
5. Commit coherent local paths explicitly. Do not push, tag, publish, deploy,
   sign, fund, use public RPC, or contact a regulator without current authority.
6. Promote only from artifacts produced by the final joined bytes. Never let a
   categorical fallback impersonate native smooth semantics, and never let a
   green model or host test impersonate SBF/runtime evidence.
