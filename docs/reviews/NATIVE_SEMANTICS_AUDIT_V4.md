# Native degree-0--3 semantics audit v4

Date: 2026-08-19

## Verdict

Native B-spline semantics are now first-class across the implemented runtime
lifecycle seams. Greenfield `KernelAccount` v2 persists the immutable canonical
`BasisMode` selected from fully validated Terms at market creation. Every seam
that already receives Terms cross-checks that projection, while Split, Merge,
materialize, and dematerialize reconstruct the stored mode and require Active
before constructing a pure market. The v3 Resolution record remains the sole
owner of a resolved native vector; Kernel v2 does not duplicate it.

The former active-mode defect was a genuine P1 representation/refinement gap,
not an observed valid-origin extraction exploit. It is repaired here. Hostile
mode flips, opposite categorical/native resolve and redeem calls,
undercollateralized native prestates, and resolved Split-family calls now
refuse with exact classes before mutation. Valid Active derived Split and
materialize preserve `C >= max_i T_i` and preserve the stored mode.

The earlier bounded API repair also remains in force: public
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

## Lifecycle matrix

| Seam | Native fact actually carried/consumed | Result | Status and exact evidence |
|---|---|---|---|
| Market creation | `TermsAccount.basis_degree` selects the v2 index record only for degree zero and the v3 native record for degrees one through three. Creation writes KernelAccount v2 with immutable `FinitePreset` or `DerivedBasis` and validation cross-checks it against Terms. | No preset lowering or caller-selected mode at creation. | **PASS**: `programs/clutch-sbf/program/src/instructions/market_init.rs` functions `terms_basis_degree`, `write_kernel`, `require_payout_set_binding`, and `validate_market_wide`; hostile mode-flip and degree-0--3 mapping tests. |
| Terms and grids | The digest body carries degree, active knots, uniform-spacing declaration, payout-map liveness, denominator anchor, statistic, ambiguity/edge policy, source/evaluator identity, and price-grid identity. Smooth payout maps must be entirely unused. | The basis grid is first-class. `CreateMarket` does not receive/authenticate the referenced `PriceGridAccount`; this can strand trading but cannot change the spline weights because knots live in Terms. | **PASS / P2 availability join**: `programs/solana-layout/src/lib.rs:2641-2742`, `:2744-2803`, `:2811-2994`; CreateMarket roles name Terms at `programs/clutch-sbf/program/src/instructions/market_init.rs:216-239` but no PriceGrid. |
| Evaluator and quantization | `ResolutionTerms` builds a degree-zero classifier or a degree-one--three `BasisSpec`; smooth weights are exact integers and quantized once by the registered largest-remainder rule. | No float, midpoint, endpoint choice, preset search, or outcome-index recovery. Degree two/three refuse non-point evidence; the production record path is point-only for all smooth degrees. Smooth TWAP refuses rather than manufacturing an integer point. | **PASS**: `programs/solana-reference/src/resolution.rs:401-496`, `:637-679`, `:719-792`; production strict point gate at `programs/clutch-sbf/program/src/instructions/observe_resolve.rs:1438-1498`. |
| Public payout derivation APIs | `derive_payout` owns degree-zero index selection; `derive_payout_vector` owns degrees one through three. | The former smooth-to-preset-index bridge was a real public semantic lowering and is repaired in this audit. R-16 remains an unreachable reserved class so registry numbering does not move. | **REPAIRED P1**: `programs/solana-reference/src/resolution.rs:225-266`, `:719-792`, test `:852-859`; focused crate 50/50 green. |
| Accumulator and window modes | `clutch-accumulator` seals an identity-typed `WindowResult`. `clutch-bspline-accumulator` separately accumulates exact quantized basis occupation with explicit `ExactOnly` or named largest-remainder finalization. | Point resolution does not midpoint-evaluate. Occupation semantics are an exact host model, but they are not selectable in live Terms/Resolve and are not a substitute for the point path. | **PASS point / P2 occupation integration**: `crates/clutch-accumulator/src/window.rs:268-317`, `:460-753`; `crates/clutch-bspline-accumulator/src/lib.rs:65-171`, `:213-466`. |
| Split and Merge | Pure `clutch-kernel`, the public account-shaped reference adapter, and SBF `kernel_step` all reconstruct Kernel v2's stored immutable mode. SBF requires Active before constructing, so no resolved vector is needed. | Native Active solvency is checked as native; mode is preserved on every write. Resolved calls refuse before construction and rollback. | **REPAIRED P1 / PASS**: `programs/solana-reference/src/lib.rs` KernelAccount v2 codec and `kernel_market`; `programs/clutch-sbf/program/src/instructions/split.rs::kernel_step`; active native solvency and exact refusal/rollback tests. |
| Materialize and dematerialize | The claim index denotes one native basis Egg under the market's Terms. The transition moves quantity between internal and bearer form without changing per-Egg total supply, using the stored native mode. | No payout is evaluated and no categorical cell is selected. | **REPAIRED P1 / PASS**: shared `split.rs::kernel_step`; degree-1--3 native mode Split/materialize host coverage; terminology bug at `docs/EVIDENCE_MATRIX.md:25` remains P2. |
| Resolution persistence | Degree-zero stores a payout index. Smooth markets store raw point, denominator, and the full vector in the v3 record; the kernel retains only phase/index-zero compatibility state. Consumers reconstruct the vector ephemerally from the sole persisted owner. | No vector duplication or preset lookup on the native path. | **PASS, SBF-executed**: `programs/solana-layout/src/native_resolution.rs:43-83`, `:131-236`; `programs/clutch-sbf/program/src/instructions/observe_resolve.rs:1230-1277`, `:1507-1524`, `:1566-1604`, `:2225-2344`; native SVM 7/7 against the joined ELF. |
| Source/archive to Resolve | Expected domain and window ID come from digest-bound Terms, a SourceSpec and sealed SourceArchive receipt are authenticated, and the compatibility numeric projection must equal the archive exactly. | This is not lowering; it removes caller substitution. The joined source/archive and native-resolution commits were rerun against one ELF. | **PASS, SBF-executed**: `programs/clutch-sbf/program/src/instructions/observe_resolve.rs:677-762`, `:2600-2686`; commit `0b96a3a`; native SVM 7/7. |
| Internal redemption | The canonical v3 record is bound to Terms and market, its vector is installed ephemerally as `DerivedBasis`, and exact redemption refuses a remainder. | First-class native payout; no flooring, index reinterpretation, or second vector owner. | **PASS, SBF-executed**: `programs/clutch-sbf/program/src/instructions/observe_resolve.rs:1886-1949`; `programs/clutch-sbf/svm-tests/tests/native_resolution.rs:689-765`; joined native SVM 7/7. |
| External redemption | The adapter branches on Terms degree, binds the same v3 record, reconstructs `DerivedBasis`, burns the exact bearer quantity, and pays an exact lot. | No categorical reinterpretation. Minimal exact lots for degrees one through three execute; sub-lots and hostile role/mode/window/mint cases refuse and rollback. | **PASS, SBF-executed**: `programs/clutch-sbf/program/src/instructions/external_exit.rs:138-235`, `:345-507`; `programs/clutch-sbf/svm-tests/tests/native_resolution.rs:858-1058`; commit `cae3d90`; joined native SVM 7/7. |
| Orders and reservations | `PortfolioRecord` is an exact coefficient vector over Eggs. Epoch and Reservation bind the Terms digest, grid, width, policy, owner/generation, and exact reserved component/cash amounts. No code selects a terminal category. | Single-Egg orders and coefficient portfolios preserve native basis identity. The name “portfolio” is not categorical lowering. | **PASS placement/reservation**: `programs/clutch-sbf/program/src/instructions/orders_batch.rs:733-787`, `:1848-1907`; `programs/clutch-sbf/program/src/instructions/orders_batch/reservation.rs:15-118`, `:130-190`. |
| Coupled settlement | The live consumer moves one selected Egg and exact cash between bound reservations. A basis Egg is valid for every degree because Terms names what that Egg means. | V1 admits only same-page, two-owner, two-outcome, full-fill, zero-fee, direct single-Egg pairs. Portfolios, partials, virtual legs, and fees refuse; they are not silently compiled to categorical orders. | **LIMIT, not lowering**: `programs/clutch-sbf/program/src/instructions/orders_batch/settlement.rs:237-359`, `:442-665`. |
| Wrapper model and portfolio compiler | Wrapper identity includes Terms digest, degree, denominator, semantic tag, and canonical native underlyings. Terminal payout is the native dot product. The compiler separates native exact/certified approximation from explicitly named categorical compatibility lowering. | Semantics are honest and first-class inside the host models; neither is a live account/compiler/client path. | **PASS model / P2 integration**: `research/structured-claim-wrapper/model.py:67-136`, `:141-227`, `:434-477`; `research/bspline-shape-compiler/src/lib.rs:197-267`, tests `:1451-1520`. |
| Static client | The main offline JSON fixture/UI carries only an outcome count and generic redemption/rounding prose. A separate native B-spline inspection SDK (`native-bspline-v1.js`) now inspects canonical Terms/certificate bytes and emits unsigned, offline runtime-shaped native artifact/CreateMarket previews; it does not build account metas/messages or submit. | The main UI still cannot display or author a native market, and the SDK does not make compiler evidence or a certificate on-chain authority. Neither surface reinterprets a live market or lowers smooth semantics to categories. | **P2 capability / offline SDK boundary**: `apps/static-client/terms.json:1-22`, `apps/static-client/app.js:70-134`, `apps/static-client/native-bspline-v1.js:1-12`, `apps/static-client/native-bspline-market-creation-v1.schema.json:1-43`. |
| Harness and joined lifecycle | The signed 22-step categorical walk consumes Kernel v2 and covers Split/materialize/dematerialize/Merge/Resolve/internal and external redeem. Native SVM covers degree-one--three resolve, retry, internal/external redeem, hostile modes, and rollback against the same rebuilt ELF family. | A public blank-bank joined lifecycle for every smooth degree remains a separate evidence lane; no implemented seam silently lowers meanwhile. | **PASS runtime / P1 joined-evidence residue**: signed walk 22/22 plus falsifiability green; native SVM 7/7 against ELF `c8ff4ac7...`; `programs/clutch-sbf/svm-tests/tests/native_resolution.rs`. |
| Formal and refinement evidence | Lean models both basis modes, native vector resolution, solvency, and degree-one--three evaluator obligations. The new Verus B-spline lane checks finite fixtures. | The handwritten Rocq kernel remains finite-preset/index-only, Verus production coverage remains a narrow transfer helper, and no theorem joins Terms bytes -> evaluator -> v3 Resolution bytes -> SBF redemption. | **P1 refinement / P2 wording**: `lean/DragonsClutch/Kernel.lean:60-90`, `:315-342`; `lean/DragonsClutch/BSpline.lean`; `rocq/ClutchKernel.v:24-42`, `:114-182`, `:316-374`; `docs/VERIFICATION.md:112-129`; `verus/bspline/README.md`. |

## Active-mode gap: classification and landed binding

The pre-repair account stored market, phase, resolved payout index, payout set,
and aggregate supply, but no basis mode. Because the Split-family account list
has neither Terms nor Resolution, `kernel_step` hardcoded `FinitePreset`. This
was an honest layout/adapter representation gap with no valid-origin extraction
trace found, but it was still P1 because the checked invariant was not the
Terms-selected invariant.

The preferred binding is now implemented as a greenfield KernelAccount v2:

1. byte 1 is kernel codec version 2 and the exact length grows by one;
2. byte 35 is the canonical `basis_mode` (`0` finite, `1` derived); any other
   byte and every v1 account refuse;
3. CreateMarket derives this byte only from fully decoded Terms;
4. every live seam that already receives Terms compares degree to stored mode;
5. Split-family reconstructs the stored mode only after requiring Active; and
6. the v3 Resolution account remains the sole persisted native-vector owner.

Adding Terms to every hot Split-family account list was rejected as a larger
ABI/CU change. Inferring mode from Resolution length was rejected because
Resolution is not the immutable semantic owner.

Never infer native mode from `resolved_payout == 0`, from preset membership, or
from a vector equalling a preset. Those facts are not injective.

## Other exact findings

### Joined native evidence is coherent; an earlier dirty-tree run was not

During concurrent editing, the pure reference suite passed 50/50 but the SBF
tree was temporarily incoherent:

- `cargo test -p clutch-sbf --lib` passed 132 tests and failed
  `instructions::orders_batch::settlement::tests::narrow_submission_is_deterministic_funded_and_stays_unverified`
  with `Adapter(MismatchedState)` at `settlement.rs:1543`.
- `cargo test --test native_resolution -- --nocapture` against
  `svm-tests/tests/fixtures/clutch_sbf.so` passed 4 and failed 3. Resolve failed
  with `Custom(81)` and exact native external exit with `Custom(8)` because the
  fixture predates the concurrent Resolve account-plane and external-exit
  changes.

Those failures were not native semantic defects; they showed why dirty source
and a stale fixture are not evidence. After source/archive commit `0b96a3a` and
native external commit `cae3d90`, I reran
`BPF_OUT_DIR=programs/clutch-sbf/target/deploy cargo test --test native_resolution -- --nocapture`
against ELF SHA-256
`e448f1a9a5fe7c80b2d8ece939dab059ef64ccadab11fa5952328cd31ed35a32`.
All seven native SVM tests passed. After coupled-settlement commit `1835b79`,
`cargo test -p clutch-sbf --lib` also passed 135/135. The final native
Resolve/internal/external claim is therefore one coherent executed artifact
set. KernelAccount v2 then closed the independent active Split-family
mode-binding P1 and was rerun against a later coherent ELF as recorded below.

### P2: stale terminology and historical claims obscure the boundary

- `docs/EVIDENCE_MATRIX.md:25` says materialize/dematerialize preserves
  “categorical supply”; the invariant is per-native-basis-Egg total supply.
- `programs/clutch-sbf/program/src/lib.rs:19-20` still says “categorical bearer”
  although native external redemption is now SBF-executed.
- `programs/solana-layout/src/native_resolution.rs:3-5` calls v3 a proposed,
  non-live account although CreateMarket and Resolve consume it.
- `programs/solana-layout/src/lib.rs:2862-2867` says degrees two and three are
  unimplemented although the evaluator and native SBF path implement them.
- `docs/implementation/DISTRIBUTIONAL_CLAIMS_DESIGN.md:995-1012`, `:1082-1104`,
  `docs/implementation/RESOLUTION_EVIDENCE_PLAN.md:856-861`, and
  `docs/implementation/SOLANA_REFERENCE_ADAPTER.md:405-419` retain historical
  R-16/preset-bridge claims. They should be labeled as superseded history or
  trued to the degree-zero-only public API.

### P2: native capability is not user-reachable as a product surface

The main static UI cannot inspect basis degree or knots, the native SDK is an
offline byte inspector rather than a transaction builder, the harness constructs
only the checked degree-one fixture, the compiler/wrapper remain host models,
and live coupled settlement cannot consume a coefficient portfolio. None
silently lowers a native market, but together they prevent an end user from
exercising the native lifecycle as one first-class public workflow.

## Dependency-ordered repair plan

1. **Completed:** bind immutable basis mode in KernelAccount v2, cross-check it
   against Terms, reconstruct it across Split-family, and freeze hostile flip,
   wrong-mode, native-solvency, phase-refusal, and rollback behavior.
2. Add one blank-bank joined lifecycle for each degree one, two, and three:
   seal artifacts, CreateMarket, Split, internal transfer/order reservation,
   materialize, archived Resolve, exact internal redeem, exact bearer redeem,
   and sub-lot rollback. This catches both mode loss and genesis-fixture bias.
3. Extend coupled consumption from direct single-Egg fills to exact coefficient
   portfolio fills, preserving Terms identity and refusing every unsupported
   rounding/partial/fee case until its owner exists.
4. Publish a native client schema and compiler artifact that carry Terms digest,
   degree, knots, denominator, coefficients, approximation certificate, and an
   explicit native-versus-compatibility semantic tag.
5. Add refinement evidence for Terms bytes -> `BasisSpec` -> quantized vector ->
   v3 Resolution bytes -> internal/external payout. Either extend Rocq with
   basis mode/vector resolution or state explicitly that it proves only the
   finite-preset legacy. Correct the stale categorical wording and historical
   R-16 claims.

## Checks executed

Green local checks during the audit:

- `crates/clutch-kernel`: 23 tests.
- `programs/solana-layout`: 132 unit tests plus 2 additional/doc tests.
- `programs/solana-reference`: 53 tests after the degree-zero-only and Kernel
  v2 repairs.
- `crates/clutch-bspline-accumulator`: 19 tests.
- `research/structured-claim-wrapper`: 18 Python tests.
- `research/bspline-shape-compiler`: 14 tests.
- `apps/static-client`: 11 Node tests.
- `programs/clutch-sbf` host: 153 tests, including hostile mode flips,
  categorical/native wrong-mode resolution and redemption, Active derived
  solvency, and Split-family rollback.
- `tools/vector-check`: 11 tests; adapter vectors now name `basis_mode`, and
  both vector and manifest digests were regenerated.
- Joined real-SBF native resolution/internal/external redemption: 7 tests
  against ELF SHA-256
  `c8ff4ac7286004cb5d897cc92b05f7a9e386107d295cb1441adcd227e0b35138`.
- Loopback-only signed committed walk: 22 signed transactions, two expected
  refusals, 18 watched accounts, exact reloads green, and corrupted terminal
  expectation red. The gate used only freshly generated disposable test keys
  and genesis/faucet test lamports, contacted only `127.0.0.1`, read no wallet
  configuration, and deployed nowhere. Its disposable keys remained only in
  the isolated temp run directory until the exact ELF and five non-sensitive
  logs were handed off; the entire temp directory was then deleted.

The earlier red dirty-tree commands and their corrected interpretation are
recorded above. No network, deployment, push, key, fund, or external-system
action was performed.
