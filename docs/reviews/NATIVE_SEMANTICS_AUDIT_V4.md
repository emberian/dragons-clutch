# Native degree-0--3 semantics audit v4

Date: 2026-08-19

## Verdict

Native B-spline semantics are not yet first-class across every lifecycle seam.
The immutable Terms, exact evaluator, native resolution record, pure kernel,
and internal redemption preserve degree-one through degree-three vectors. The
live Split/Merge/materialize/dematerialize adapter does not: it reconstructs
every persisted aggregate as `BasisMode::FinitePreset` because
`KernelAccount` has no basis-mode field and those instructions receive neither
Terms nor Resolution. This is a P1 representation/refinement gap.

I found no valid-origin value-extraction trace through that gap. In Active
state, a valid derived market has `C >= max_i T_i`. Split adds the same `q` to
`C` and every `T_i`; Merge subtracts it from both; materialize and
dematerialize do not change `C` or `T`; direct bearer burns only reduce a
`T_i`. Thus those transitions preserve the stronger derived bound even though
the adapter checks the weaker preset interpretation. The gap can nevertheless
admit a corrupted or future-reachable aggregate that fails the native
invariant, and a resolved native market can be checked against preset zero
before the phase refusal. It is not merely documentation: the wrong
interpretation executes at `programs/clutch-sbf/program/src/instructions/split.rs:545-592`.

One bounded repair landed with this audit: public
`clutch_solana_reference::derive_payout` is now degree-zero-only and returns
`WrongResolutionMode` for degrees one through three, even when the exact smooth
vector equals a frozen preset. Smooth callers use `derive_payout_vector`; no
public derivation performs preset-membership lowering. Refusal class R-16 is
retained only as an unreachable, reserved numeric registry slot.

## Status and severity

- **PASS**: the current implementation represents and consumes the native fact
  without reinterpreting it.
- **LIMIT**: native meaning is preserved, but the production surface supports
  only a narrower operation or has no joined lifecycle evidence.
- **P1**: release-blocking semantic ownership, representation, or executable
  evidence gap.
- **P2**: capability, documentation, or non-consensus evidence gap.
- **IN FLIGHT**: present only in the dirty concurrent worktree; it is not an
  accepted claim until rebuilt and rerun as one evidence set.

## Lifecycle matrix

| Seam | Native fact actually carried/consumed | Result | Status and exact evidence |
|---|---|---|---|
| Market creation | `TermsAccount.basis_degree` selects the 165-byte v2 index record only for degree zero and the 319-byte v3 native record for degrees one through three. Creation reconstructs the kernel with `DerivedBasis` for every positive degree. | No preset lowering at creation. Blank-bank construction currently exercises only degrees zero and one, not two and three and not a joined later lifecycle. | **PASS / P2 evidence**: `programs/clutch-sbf/program/src/instructions/market_init.rs:674-697`, `:802-816`, `:850-892`, `:1068-1104`, `:1272-1279`; `programs/clutch-sbf/svm-tests/tests/blank_bank_lifecycle.rs:499-500`, `:568-650`. |
| Terms and grids | The digest body carries degree, active knots, uniform-spacing declaration, payout-map liveness, denominator anchor, statistic, ambiguity/edge policy, source/evaluator identity, and price-grid identity. Smooth payout maps must be entirely unused. | The basis grid is first-class. `CreateMarket` does not receive/authenticate the referenced `PriceGridAccount`; this can strand trading but cannot change the spline weights because knots live in Terms. | **PASS / P2 availability join**: `programs/solana-layout/src/lib.rs:2641-2742`, `:2744-2803`, `:2811-2994`; CreateMarket roles name Terms at `programs/clutch-sbf/program/src/instructions/market_init.rs:216-239` but no PriceGrid. |
| Evaluator and quantization | `ResolutionTerms` builds a degree-zero classifier or a degree-one--three `BasisSpec`; smooth weights are exact integers and quantized once by the registered largest-remainder rule. | No float, midpoint, endpoint choice, preset search, or outcome-index recovery. Degree two/three refuse non-point evidence; the production record path is point-only for all smooth degrees. Smooth TWAP refuses rather than manufacturing an integer point. | **PASS**: `programs/solana-reference/src/resolution.rs:401-496`, `:637-679`, `:719-792`; production strict point gate at `programs/clutch-sbf/program/src/instructions/observe_resolve.rs:1438-1498`. |
| Public payout derivation APIs | `derive_payout` owns degree-zero index selection; `derive_payout_vector` owns degrees one through three. | The former smooth-to-preset-index bridge was a real public semantic lowering and is repaired in this audit. R-16 remains an unreachable reserved class so registry numbering does not move. | **REPAIRED P1**: `programs/solana-reference/src/resolution.rs:225-266`, `:719-792`, test `:852-859`; focused crate 50/50 green. |
| Accumulator and window modes | `clutch-accumulator` seals an identity-typed `WindowResult`. `clutch-bspline-accumulator` separately accumulates exact quantized basis occupation with explicit `ExactOnly` or named largest-remainder finalization. | Point resolution does not midpoint-evaluate. Occupation semantics are an exact host model, but they are not selectable in live Terms/Resolve and are not a substitute for the point path. | **PASS point / P2 occupation integration**: `crates/clutch-accumulator/src/window.rs:268-317`, `:460-753`; `crates/clutch-bspline-accumulator/src/lib.rs:65-171`, `:213-466`. |
| Split and Merge | Pure `clutch-kernel` transitions are mode-aware. The SBF adapter rebuilds from mode-less `KernelAccount`. | Every persisted market is reconstructed as `FinitePreset`; no Terms or Resolution account is in this seam. Native Active economics happen to coincide for reachable valid states, but the native invariant is not what the adapter checks. | **P1 representation/refinement**: `programs/solana-reference/src/lib.rs:101-103`, `:494-507`, `:677-725`; `programs/clutch-sbf/program/src/instructions/split.rs:545-592`. |
| Materialize and dematerialize | The claim index denotes one native basis Egg under the market's Terms. The transition moves a quantity between internal and bearer form without changing per-Egg total supply. | No payout is evaluated and no categorical cell is selected. It shares the same mode-less reconstruction defect as Split/Merge. | **P1 representation; economics basis-neutral**: same `split.rs:545-592` kernel step; terminology bug at `docs/EVIDENCE_MATRIX.md:25` calls this “categorical supply.” |
| Resolution persistence | Degree-zero stores a payout index. Smooth markets store raw point, denominator, and the full vector in the v3 record; the kernel retains only phase/index-zero compatibility state. Consumers reconstruct the vector ephemerally from the sole persisted owner. | No vector duplication or preset lookup on the native path. | **PASS source / evidence pending current join**: `programs/solana-layout/src/native_resolution.rs:43-83`, `:131-236`; `programs/clutch-sbf/program/src/instructions/observe_resolve.rs:1230-1277`, `:1507-1524`, `:1566-1604`, `:2225-2344`. |
| Source/archive to Resolve | In the current dirty tree, expected domain and window ID come from digest-bound Terms, a SourceSpec and sealed SourceArchive receipt are authenticated, and the legacy numeric buffer must equal the archive projection exactly. | This is not lowering; it removes caller substitution. It was concurrently changed while this audit ran, so the checked-in SBF fixture predates the 9-to-11-account Resolve plane and cannot support an accepted execution claim yet. | **IN FLIGHT / P1 evidence gate**: `programs/clutch-sbf/program/src/instructions/observe_resolve.rs:677-762`, `:2600-2686`; accepted truth before the join is `CURRENT_TRUTH.md:150-171`. |
| Internal redemption | The canonical v3 record is bound to Terms and market, its vector is installed ephemerally as `DerivedBasis`, and exact redemption refuses a remainder. | First-class native payout; no flooring, index reinterpretation, or second vector owner. | **PASS source; previously SBF-executed baseline**: `programs/clutch-sbf/program/src/instructions/observe_resolve.rs:1886-1949`; `programs/clutch-sbf/svm-tests/tests/native_resolution.rs:689-765`. Re-run is required after the in-flight Resolve ABI join. |
| External redemption | The current dirty source branches on Terms degree, binds the same v3 record, reconstructs `DerivedBasis`, burns the exact bearer quantity, and pays an exact lot. | No categorical reinterpretation in source. This lane and its SVM tests are concurrent/in-flight; it must not be promoted from the accepted categorical-only baseline until a new ELF passes. | **IN FLIGHT / P1 evidence gate**: `programs/clutch-sbf/program/src/instructions/external_exit.rs:138-235`, `:345-507`; tests at `programs/clutch-sbf/svm-tests/tests/native_resolution.rs:858-1058`. |
| Orders and reservations | `PortfolioRecord` is an exact coefficient vector over Eggs. Epoch and Reservation bind the Terms digest, grid, width, policy, owner/generation, and exact reserved component/cash amounts. No code selects a terminal category. | Single-Egg orders and coefficient portfolios preserve native basis identity. The name “portfolio” is not categorical lowering. | **PASS placement/reservation**: `programs/clutch-sbf/program/src/instructions/orders_batch.rs:733-787`, `:1848-1907`; `programs/clutch-sbf/program/src/instructions/orders_batch/reservation.rs:15-118`, `:130-190`. |
| Coupled settlement | The live consumer moves one selected Egg and exact cash between bound reservations. A basis Egg is valid for every degree because Terms names what that Egg means. | V1 admits only same-page, two-owner, two-outcome, full-fill, zero-fee, direct single-Egg pairs. Portfolios, partials, virtual legs, and fees refuse; they are not silently compiled to categorical orders. | **LIMIT, not lowering**: `programs/clutch-sbf/program/src/instructions/orders_batch/settlement.rs:237-359`, `:442-665`. |
| Wrapper model and portfolio compiler | Wrapper identity includes Terms digest, degree, denominator, semantic tag, and canonical native underlyings. Terminal payout is the native dot product. The compiler separates native exact/certified approximation from explicitly named categorical compatibility lowering. | Semantics are honest and first-class inside the host models; neither is a live account/compiler/client path. | **PASS model / P2 integration**: `research/structured-claim-wrapper/model.py:67-136`, `:141-227`, `:434-477`; `research/bspline-shape-compiler/src/lib.rs:197-267`, tests `:1451-1520`. |
| Static client | The offline JSON fixture carries only an outcome count and generic redemption/rounding prose. It has no degree, knots, denominator, native semantic identity, or native order/compiler artifact. | It cannot display or author a native market, but it also labels itself a non-live fixture and does not reinterpret a live market. | **P2 capability**: `apps/static-client/terms.json:1-22`, `apps/static-client/app.js:70-134`, `apps/static-client/index.html:56-63`. |
| Harness and joined lifecycle | The committed walk authors degree-zero Terms unconditionally. Native SVM fixtures start from a categorical plane and rewrite/inject smooth accounts; blank-bank creation and native resolve/redemption are separate tests. | There is no public blank-bank CreateMarket -> Split/materialize -> archived Resolve -> internal/external redeem proof for each degree one through three. | **P1 release evidence**: `programs/clutch-sbf/harness/src/main.rs:1255-1298`; `programs/clutch-sbf/svm-tests/tests/native_resolution.rs:138-257`; `programs/clutch-sbf/svm-tests/tests/blank_bank_lifecycle.rs:568-650`. |
| Formal and refinement evidence | Lean models both basis modes, native vector resolution, solvency, and degree-one--three evaluator obligations. The new Verus B-spline lane checks finite fixtures. | The handwritten Rocq kernel remains finite-preset/index-only, Verus production coverage remains a narrow transfer helper, and no theorem joins Terms bytes -> evaluator -> v3 Resolution bytes -> SBF redemption. | **P1 refinement / P2 wording**: `lean/DragonsClutch/Kernel.lean:60-90`, `:315-342`; `lean/DragonsClutch/BSpline.lean`; `rocq/ClutchKernel.v:24-42`, `:114-182`, `:316-374`; `docs/VERIFICATION.md:112-129`; `verus/bspline/README.md`. |

## Active-mode gap: exact classification and binding

`KernelAccount` stores market, phase, resolved payout index, finite payout set,
and aggregate supply, but neither immutable basis mode nor a native vector
(`programs/solana-reference/src/lib.rs:494-507`). The Split-family account list
also has no Terms or Resolution. Therefore `kernel_step` has no authenticated
input from which to choose `DerivedBasis` and hardcodes `FinitePreset`
(`split.rs:562-580`). Its adjacent comment says a derived market is
“unrepresentable on chain,” but the instruction does not refuse one; it runs
the finite-preset reconstruction.

This is an honest layout/adapter representation gap with a misleading comment,
not a payout-selection bridge. It is not currently exploitable from a valid
program-founded Active state for the invariant-preservation reason in the
verdict. It remains P1 because the adapter does not refine the Terms-selected
kernel semantics, relies on a reachability argument not represented in the
bytes it checks, and may become exploitable when a future transition changes
only some supplies or collateral.

The minimum durable binding is one of:

1. **Preferred: KernelAccount v2 mode cache.** Append one immutable canonical
   `basis_mode` byte, initialize it from fully validated Terms during
   CreateMarket, and compare it with Terms at every seam that already receives
   Terms. Split-family instructions then reconstruct the stored mode. Terms
   remains the semantic owner; Kernel holds a checked projection. The active
   seam does not need a native vector, because an unresolved derived market's
   vector is zero and the resolved vector remains solely owned by v3
   Resolution.
2. **No duplicated projection: add Terms to all four account planes.** Add the
   read-only Terms PDA, verify digest/bump/market binding, and select mode from
   `basis_degree` on every call. This avoids a mode byte but expands the ABI,
   account count, digest work, transaction size, and CU for the hottest
   lifecycle seam.
3. **Resolution as discriminator.** A v2/v3 Resolution account length can
   identify the ABI, but it is a secondary lifecycle record rather than the
   immutable semantic owner. Prefer Terms or a Terms-checked Kernel projection.

Never infer native mode from `resolved_payout == 0`, from preset membership, or
from a vector equalling a preset. Those facts are not injective.

## Other exact findings

### P1: executable evidence is not one coherent artifact set

During this audit, the pure reference suite passed 50/50, but the concurrent
SBF tree was not coherent:

- `cargo test -p clutch-sbf --lib` passed 132 tests and failed
  `instructions::orders_batch::settlement::tests::narrow_submission_is_deterministic_funded_and_stays_unverified`
  with `Adapter(MismatchedState)` at `settlement.rs:1543`.
- `cargo test --test native_resolution -- --nocapture` against
  `svm-tests/tests/fixtures/clutch_sbf.so` passed 4 and failed 3. Resolve failed
  with `Custom(81)` and exact native external exit with `Custom(8)` because the
  fixture predates the concurrent Resolve account-plane and external-exit
  changes.

These dirty-tree failures are not classified as native semantic defects. They
invalidate any claim that the current source, tests, and ELF are one executed
evidence set. Rebuild the SBF artifact and rerun after the source/archive,
native external, and order-settlement lanes join.

### P2: stale terminology and historical claims obscure the boundary

- `docs/EVIDENCE_MATRIX.md:25` says materialize/dematerialize preserves
  “categorical supply”; the invariant is per-native-basis-Egg total supply.
- `programs/clutch-sbf/program/src/lib.rs:19-20` distinguishes “categorical
  bearer” while the dirty native external lane is landing.
- `programs/solana-layout/src/native_resolution.rs:3-5` calls v3 a proposed,
  non-live account although CreateMarket and Resolve consume it.
- `programs/solana-layout/src/lib.rs:2862-2867` says degrees two and three are
  unimplemented although the evaluator and native SBF path implement them.
- `programs/clutch-sbf/program/src/instructions/split.rs:562-571` says derived
  markets are unrepresentable, but the function actually executes them after a
  finite-preset reconstruction.
- `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md:995-1012`, `:1082-1104`,
  `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md:856-861`, and
  `docs/implementation/SOLANA_REFERENCE_ADAPTER.md:405-419` retain historical
  R-16/preset-bridge claims. They should be labeled as superseded history or
  trued to the degree-zero-only public API.

### P2: native capability is not user-reachable as a product surface

The static client cannot inspect basis degree or knots, the harness constructs
only degree zero, the compiler/wrapper remain host models, and live coupled
settlement cannot consume a coefficient portfolio. None silently lowers a
native market, but together they prevent an end user from exercising the
native lifecycle as one first-class public workflow.

## Dependency-ordered repair plan

1. Bind immutable basis mode in every active lifecycle reconstruction. Prefer a
   Terms-checked `KernelAccount` v2 mode projection; otherwise add Terms to all
   Split/Merge/materialize/dematerialize account lists. Add hostile mode-flip,
   wrong-Terms, derived-active-solvency, and resolved-native phase-refusal tests.
2. Finish the SourceSpec/SourceArchive/Resolve join and native external lane as
   one ABI change; then rebuild the ELF and rerun host plus SVM evidence. Do not
   promote current dirty source independently of its fixture.
3. Add one blank-bank joined lifecycle for each degree one, two, and three:
   seal artifacts, CreateMarket, Split, internal transfer/order reservation,
   materialize, archived Resolve, exact internal redeem, exact bearer redeem,
   and sub-lot rollback. This catches both mode loss and genesis-fixture bias.
4. Extend coupled consumption from direct single-Egg fills to exact coefficient
   portfolio fills, preserving Terms identity and refusing every unsupported
   rounding/partial/fee case until its owner exists.
5. Publish a native client schema and compiler artifact that carry Terms digest,
   degree, knots, denominator, coefficients, approximation certificate, and an
   explicit native-versus-compatibility semantic tag.
6. Add refinement evidence for Terms bytes -> `BasisSpec` -> quantized vector ->
   v3 Resolution bytes -> internal/external payout. Either extend Rocq with
   basis mode/vector resolution or state explicitly that it proves only the
   finite-preset legacy. Correct the stale categorical wording and historical
   R-16 claims.

## Checks executed

Green local checks during the audit:

- `crates/clutch-kernel`: 23 tests.
- `programs/solana-layout`: 132 unit tests plus 2 additional/doc tests.
- `programs/solana-reference`: 50 tests after the degree-zero-only repair.
- `crates/clutch-bspline-accumulator`: 19 tests.
- `research/structured-claim-wrapper`: 18 Python tests.
- `research/bspline-shape-compiler`: 14 tests.
- `apps/static-client`: 11 Node tests.

The two red dirty-tree SBF commands and their interpretation are recorded
above. No network, deployment, push, key, fund, or external-system action was
performed.
