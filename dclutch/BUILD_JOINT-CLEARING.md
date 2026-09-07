# BUILD_JOINT-CLEARING — the joint clearing, the mechanism cohort's first

Branch `build/joint-clearing` off `main` at `f7c03e845`, worktree
`/private/tmp/claude-501/-Users-ember-dev-dragons-clutch/ef2920a4-77e9-4597-99e3-94569deb51f7/scratchpad/build-joint-clearing`.
Built by source inspection under the BUILD preamble: no SBF build, no suite, no
devnet. Every `file:line` below is this branch's. Written incrementally; the
`Status` line at the top of each section says what is landed and what is owed.

Authorities: `docs/design/MECHANISM_JOINT_CLEARING_2026_09_04.md` (the rule
and the eight KKT conjuncts), decision 0032 CONFIRMED (the residual STRANDS,
the tie-break MINIMISES the vector, the visible book ships and the clearing
is published), decision 0031 (cohort-18's first, since 17 shipped the burn),
`JointClearingV1.lean` (44 theorems before this branch; 55 after).

## 0. The three calls that shape everything (ruled provisionally, §3)

1. **The tie-break is a verifier conjunct, not a selection criterion.** For a
   fixed primal optimum `(f, M)` the certifying price vectors of a
   single-outcome book form a BOX ∩ simplex (each row bounds its own outcome's
   price from one side; slackness pins a residual outcome to zero) and by LP
   duality that box is the whole dual optimal face. Its lexicographic minimum
   is a greedy the chain computes in `O(K)` at the terminal row
   (`JointClearingV1.lexMinFrom`, `runtime_verify.rs::lexmin_price_vector`).
   `NonMinimalPriceVector` refuses everything else, so every certified
   candidate for one book carries ONE vector and the selection policy needs no
   price criterion. 0032 §2b's content — the clearing is a function of the
   book — is kept and strengthened; its FORM (a selection criterion) is not.
2. **The residual strands inside `Close`, not in a sixteenth action.** `Close`
   burns `inventory_i = sets − net_i` out of the candidate's own settlement
   Position through a new Claims command (`ClaimsAction::StrandResidual`,
   `EconomicKernel.strandPost`), pays the surplus, and publishes the clearing.
   No new dispatch arm, no new frame shape beyond `Close` gaining a Claims leg.
3. **The price series lives on the batch record.** `ClearingPriceV1` is a
   tail appended to the batch at byte 224 (`DCGBAT02`, `296 + 16N`), vacant
   until the third status `Cleared`, written once by `Close`. One durable
   account per batch that the page already reads; no new PDA inside the
   heaviest frame of the family.

## 1. What was built, file by file

### 1.1 Lean (`formal/dclutch-semantics/`) — landed, unchecked by lake here

| file | what |
| --- | --- |
| `DClutchSemantics/GeneralOrderV2Abi.lean` (new) | The order record V2 layout: V1 header a byte-identical prefix (`the_v1_header_is_a_prefix`), the shape (`side`, `outcome_lo`, `outcome_hi`, `claims_per_lot`; 24 bytes) at 160..184, the state window at 184..216, rows at 216 (`order_width_is_rows_after_the_fixed_prefix`); `Shape.isInterval`, `Shape.isSingleOutcome`, `Shape.row`/`rowsOf` (the derived rows), `quoteReserve`/`claimReserve` with the buy/sell laws; hostile witnesses. Magic `DCGORD02`, version 2, phase 21, sides buy=1 sell=2. |
| `DClutchSemantics/ClearingPriceV1Abi.lean` (new) | The clearing tail: 72 fixed bytes at 224 (`cleared_candidate_id`, `cleared_slot`, `sets_move`, `sets_quantity`, `filled_lots`, `live_order_count`) then prices `8N` at 296 then residual `8N`; `batchBytes N = 296 + 16N`; statuses collecting=1 closed=2 **cleared=3**; `tailAdmissible` (vacant unless cleared; cleared ⇒ on the simplex and residual only at zero price), `a_cleared_tail_is_on_the_simplex`, `a_cleared_tail_strands_only_at_zero_price`; hostiles. Magic `DCGBAT02`, version 2. |
| `DClutchSemantics/GeneralRuntimeWireV2.lean` (edited) | `candidateFields` += `live_order_count` u32 + `reserved_live` (header 128 → 136); `tailCount` 2 → 3 (prices, inputs, outputs; verified width `160 + 24N`). Two theorem statements updated. |
| `DClutchSemantics/JointClearingV1.lean` (appended, 44 → 55 theorems) | `lexMinFrom`/`lexMin` (the greedy), `sum_le_of_valueAt_le`, `lexMinFrom_length`, `Feasible`, `lexMinFrom_sum` (on the simplex under feasibility), `lexMin_head_is_least` (head minimality); `Order.singleOutcome?`, `Fill.priceBounds`, `Clearing.box`, `Clearing.minimal`; the strand (`strandedSupply`, `stranded_supply_never_exceeds_the_sets`, `stranded_supply_is_the_sets_wherever_priced`, `the_strand_is_the_residual`); witnesses: `jointMint.box = ([0,0],[60,60])`, `lexMin 100 [0,0] [60,60] = [40,60]`, both `[50,50]` and `[60,40]` non-minimal, `[40,60]` valid ∧ minimal; `zeroPriced` minimal at `[40,60,0]`; `transfer` minimal at `[30,70]`. |
| `DClutchSemantics/EconomicKernel.lean` (appended) | `strandPost`, `strandAccepts`, `strand_conserves_hoard`, `strand_burns_exactly_the_residual`, `strand_touches_no_other_outcome`, `strand_keeps_full_backing`, `the_strand_breaks_uniformity_by_exactly_the_residual` — the exception to `uniformSupply`, beside it, as decision 0032 §5 asked; the `zeroPriced` witness after mint and strand. Stated in the refunding-law style (a post-state, no new `Command` constructor). |
| `EmitGeneralOrderV2AbiRust.lean`, `EmitClearingPriceV1AbiRust.lean` (new) | Emitters → `crates/dclutch-trading/src/general/generated_order_v2.rs`, `generated_clearing_price_v1.rs`. |
| `DClutchSemantics.lean` | imports `GeneralOrderV2Abi`, `ClearingPriceV1Abi`. |

Lean caveat: `lake` was not run in this worktree. A read-only check against
the live tree's warm `.lake` was started in the background (log:
`../jc-lean-check.log`); its result is recorded in §7 when it lands. The
proofs use `native_decide`, `simp`, `omega` and the module's own lemmas;
`List.getElem?_set_ne` is the one library lemma assumed by name.

### 1.2 Generated Rust and guards — landed

| file | what |
| --- | --- |
| `crates/dclutch-trading/src/general/generated_order_v2.rs` | hand-regenerated to the emitter's output; guard `crates/dclutch-trading/tests/general__order_v2_generator_fresh.rs`. |
| `crates/dclutch-trading/src/general/generated_clearing_price_v1.rs` | same; guard `tests/general__clearing_price_v1_generator_fresh.rs`. |
| `crates/dclutch-trading/src/general/generated_runtime_wire_v2.rs` | `CANDIDATE_HEADER_BYTES_V2` 128 → 136, `CANDIDATE_LIVE_ORDER_COUNT_OFFSET_V2 = 128`, `CANDIDATE_RESERVED_LIVE_OFFSET_V2 = 132`, `VERIFIED_CANDIDATE_TAIL_COUNT_V2` 2 → 3. Hand-regenerated; the existing guard proves it. |

### 1.3 The contract crate (`crates/dclutch-trading/src/general/`) — landed

| file | what |
| --- | --- |
| `collection_v1.rs` (rewritten) | `GeneralOrderV2`, `GeneralOrderLayoutV2`, `GeneralOrderHeaderV2` (+`side`, `outcome_lo`, `outcome_hi`, `claims_per_lot`; `derived_row`, `validate_shape`, `is_single_outcome`), `GeneralSignedOrderTermsV2` (184 bytes, rows derived), `general_order_len_v2` (`216 + 16N`), `general_signed_order_terms_len_v2` (184), `general_order_identity_v2` (digest of the header). `GeneralBatchV2`, `GeneralBatchLayoutV2` (+ clearing coordinates), `GeneralBatchStateV2` (+`clearing: GeneralClearingFixedV1`), `BatchStatusV1::Cleared`, `general_batch_len_v2` (`296 + 16N`), `general_batch_price_v2`/`general_batch_residual_v2`, `GeneralBatchV2::clear` (Closed → Cleared once; `AlreadyCleared`), `encode_clearing_into`, `live_order_count()`. Refusals added to `GeneralCollectionErrorV1`: `ShapeNotInterval`, `BundleNotAdmitted`, `RowsDisagreeWithShape`, `InvalidClearing`, `AlreadyCleared`. `authenticate_batch_candidate_v1` holds `candidate.live_order_count` to the batch. `authenticate_order_execution_v2` admits `lots == 0`. Unchanged and verbatim: batch open/admit/cancel/release/close, the occurrence terms (identity preimage untouched, version stays 1). |
| `runtime_width.rs` (edited) | `validate_execution_header` admits `lots == 0`; `CandidateHeaderV2.live_order_count` (+layout, decode, encode, nonzero); `VerifiedCandidateV2` tails are prices, inputs, outputs (`encode_into(header, prices, inputs, outputs, out)`, `encode_le_tails_into(header, prices_le, inputs_le, outputs_le, out)`, `price(i)`; decode refuses an off-simplex tail with `InvalidSimplex`). |
| `mod.rs` (edited) | `GeneralChildEffectV1::StrandResidual = 11`; `moves_collateral` false for it. |
| `escrow_v1.rs` (edited) | `general_child_custody_movement_v1(StrandResidual) = None`. |
| `child_packets.rs` (edited) | `RefundingEscrowV1`; `build_materialize_packets_v2(.., refunding: Option<RefundingEscrowV1>)` selects `Mint/MergeRefundingCompleteSet` with the escrow in the vacant slot (source for a mint, destination for a merge — the slot rule of `FAILURE_ESCROW_SEATING_2026_09_04.md` §1); `build_strand_packets_v1` (Claims leg only, refuses an all-zero residual). |
| `hot_candidate_v3.rs` (edited) | `GENERAL_HOT_COMMON_SCALARS_V3` 151 → 154, `GENERAL_HOT_ITEM_SCALAR_STRIDE_V3` 6 → 7; `scalar::CLEARING_LIVE_ORDER_COUNT = 151`, `CLEARING_SETS_MOVE = 152`, `CLEARING_SETS_QUANTITY = 153`; `item_scalar::PRICE = 6`; `project_general_clearing_into_bank_v3(verified, outcome_count, bank)`. |
| `crates/dclutch-claims/src/lib.rs` (edited) | `ClaimsAction::StrandResidual = 10` (+decode). The SBF arm is a seam (§2). |

### 1.4 The verifier's joint arm (`runtime_verify.rs`) — LANDED (2150 lines)

Completed at the quota wall; §1.4–1.7 below were written by CLOSEOUT-C from the
last commit, not by the maker.

**Five new `RuntimeVerifyErrorV2` variants**, each declared, each raised, each
with a `log_line()` arm:

| variant | declared | raised | the conjunct it refuses |
| --- | --- | --- | --- |
| `RationedInsideLimit` | `:272` | `:1613` | `f_o < q_o ⇒ ℓ_o ≤ a_o·p` — a rationed order must be at its limit |
| `OrderOmitted` | `:275` | `:1016` | completeness at the terminal row |
| `PricedResidual` | `:279` | `:1712` | complementary slackness |
| `NonMinimalPriceVector` | `:282` | `:1782` | the 0032 §2b lex-min tie-break |
| `ShapeNotInterval` | `:285` | `:1439` | the single-outcome shape |

A sixth change: `ClaimImbalance` (`:291`) is **retired in place** — kept as a
word, raised by nothing, documented at `:286-290`.

**Correction to §0.1 of this doc:** it names `runtime_verify.rs::lexmin_price_vector`.
**That symbol does not exist anywhere in the tree.** The real implementation is
`fn require_minimal_price_vector` at `runtime_verify.rs:1748`, raising at `:1782`.
Fix the doc reference, not the code.

**Cursor width.** Main had five tails (indices 0–4) and no tail-count constant.
HEAD adds `PRICE_FLOOR_TAIL = 5` (`:65`) and `PRICE_CEILING_TAIL = 6` (`:67`)
plus a public `RUNTIME_VERIFIER_TAIL_COUNT_V2 = 7` (`:55`); the header stays 288
(`:51`). So `288 + 40N → 288 + 56N`. The floor/ceiling pair is the box the
lex-min greedy runs over at the terminal row — i.e. §0.1's "box ∩ simplex" is
carried on the cursor, not recomputed.

The KKT conjunct functions: `current_shape` `:1516`, `price_bound` `:1540`,
`finalize_current_order` `:1559`, `balance_from_cursor` `:1662`, `signed_sets`
`:1683`, `derive_balance` `:1699`, `require_minimal_price_vector` `:1748`,
`validate_cursor` `:1789`. **The arm is complete** — every conjunct has a raise
site, and there is no `todo!()` in the file. Its gap is tests (§5), not logic.

### 1.5 The settlement close (`runtime_settlement.rs`) — LANDED (1072 lines)

The Close path exists and is the strand's home:
`RuntimeSettlementActionV2::Close = 4` (`:57`), decoded `:66`, dispatched `:444`,
implemented `fn close` `:620`, gated on `SettlementPhaseV2::ReadyToClose` `:630`,
marked terminal `:748`, validated `:838`/`:867`. The residual is read at `:643`
and encoded as the effect's per-outcome quantities at `:802-803` via
`runtime_verified_residual_v2`. The module doc (`:16-28`) states the inversion:
close used to refuse nonzero inventory; it now requires it, prices it, strands it.

**The −597 lines are not a refactor.** Main's inline `#[cfg(test)] mod tests`
(`main:1020-1549`, **529 lines, 8 test functions**) was **deleted**, not moved,
and replaced by an out-of-line `#[cfg(test)] mod tests;` at `:1071-1072` whose
file was never created. See seam S3. The eight lost tests: `order_id`, `row`,
`terminal_fixture`, `initialized_cursor`,
`in_place_initialization_matches_three_bank_contract_at_runtime_widths`,
`settle`, `hostile_n16_runs_collect_materialize_distribute_and_terminal_across_chunks`,
`substituted_order_early_close_and_nonexact_banks_preserve_outputs`.

### 1.6 The solver (operator) — LANDED (512 lines), UNTESTED

`crates/dclutch-operator/src/general_joint_clearing_v1.rs`. **It genuinely
solves; there are no stubs.** Public surface: `BookOrderV1` `:27`, `BookV1` `:50`,
`ClearingV1` `:61`, `FillV1` `:73`, `SolverErrorV1` `:82`;
`clear_book_v1(&BookV1) -> Result<ClearingV1, SolverErrorV1>` `:140` (the greedy
fill plus the pro-rata rationing of ruling 2, ~240 lines);
`lex_min_prices(scale, lo, hi)` `:381` (the `JointClearingV1.lexMinFrom` greedy in
suffix-sum forced-floor form, infeasibility reported not swallowed);
`CandidateRecordsV1` `:406`; `build_candidate_records_v1(...)` `:423`, which lays
the clearing out as Candidate + Page wire records with one Execution row per live
order, zero-lot rows included.

**Zero tests.** And the module doc at `:16-17` claims *"the differential test in
this crate streams every clearing this module produces through the verifier"* —
**that test does not exist.** This is the single largest unbacked claim on the
branch; the sentence should be deleted or the test written (§7 step 3).

Debris to sweep: `:374` is a no-op statement
(`let _ = working.iter().map(|item| item.index).count();`), and `:408` carries a
self-correcting doc comment (`the 128 + 8N-byte … now 136 + 8N-byte Candidate`).

### 1.7 SDK and web — LANDED; program tests and runbook rows — OWED

Built by a child agent that outlived its parent. **It verified every offset,
magic, version, phase byte and tag in both new wires against both the generated
Rust and the Lean ABI modules and found no layout inconsistency** — the three
authors of these two records agree today.

| file | what |
| --- | --- |
| `packages/dclutch-sdk/lib/generalClearingV1.ts` (410) | `decodeGeneralBatchV2` `:198` (`DCGBAT02`, phase 20; V1 prefix 224, clearing fixed 224–296, prices at 296, residual at `296+8N`) — it enforces `tailAdmissible` **on decode**: vacant unless `cleared`, cleared ⇒ on the simplex, residual only at zero price. `clearingPricesV1` `:318` → the derived simplex view or `null`. `decodeGeneralOrderV2` `:335` (`DCGORD02`, phase 21, header 184, state 184–216, rows at 216 stride 16). `generalOrderIdV2` `:408` — sha256 of the 184-byte header alone. Exported from `packages/dclutch-sdk/index.ts:70`. |
| `packages/dclutch-sdk/lib/generalClearingV1.test.ts` (218) | 13 vitest cases, §5. |
| `apps/dclutch-web/components/ClearingPriceHistory.tsx` (89) | `ClearingPriceHistory({ batches })`, a pure projection with no RPC of its own; reads `decodeGeneralBatchV2`/`clearingPricesV1` from the SDK and `shortAddressV1` from `marketDiscovery`. Renders a seven-column table (Sequence, Batch, Cleared slot, Candidate, per-outcome prices with stranded residual, complete-sets move, filled lots) sorted by `sequence`, with `ClearedRow`/`UnclearedRow` and an empty state. Deliberately computes no market-data metric. |
| `apps/dclutch-web/components/ClearingPriceHistory.test.tsx` (105) | 5 cases, §5. |

Still owed: program tests with real-ELF fixtures, the cohort-18 runbook rows, and
a `docs/reference/abi/generalClearingV1.md` page.

## 2. THE SEAMS

(filled in incrementally; the rename table first)

### 2.1 Mechanical renames the contract crate forces (one sed, then `cargo check -p dclutch-trading`)

| was | is | where |
| --- | --- | --- |
| `GeneralOrderV1` | `GeneralOrderV2` | `hot_candidate_v3.rs`, `account_rules_v3.rs`, `state_artifacts_v3.rs`, `effect_artifacts_v3.rs`, `child_packets.rs`, `escrow_v1.rs` (+`escrow_v1/tests.rs`), `collection_v1/tests.rs`, `programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs`, `programs/dclutch-trading-sbf/program-test/bundle-builder/src/general.rs` |
| `GeneralOrderLayoutV1` | `GeneralOrderLayoutV2` | same |
| `GeneralOrderHeaderV1` | `GeneralOrderHeaderV2` (+4 shape fields at every constructor) | same |
| `GeneralSignedOrderTermsV1` | `GeneralSignedOrderTermsV2` | same |
| `general_order_len_v1` / `general_signed_order_terms_len_v1` / `general_order_identity_v1` | `_v2` | same |
| `GENERAL_ORDER_HEADER_BYTES_V1` / `_STATE_BYTES_V1` / `_STATE_OFFSET_V1` / `_ROW_BASE_V1` / `_ROW_STRIDE_V1` / `_ROW_RECEIVE_OFFSET_V1` / `_ROW_DELIVER_OFFSET_V1` / `GENERAL_SIGNED_ORDER_TERMS_ROW_BASE_V1` | `_V2` (the last one is gone: the signed terms have no rows) | same |
| `GeneralBatchV1` / `GeneralBatchLayoutV1` / `GeneralBatchStateV1` | `_V2` | same, plus `runtime_selection.rs` if it names the layout |
| `GENERAL_BATCH_BYTES_V1` | stays (the V1 prefix width, 224); the account width is `general_batch_len_v2(N)` | every OpenBatch creation width: `state_artifacts_v3.rs`, the account profile's batch width, `account_rules_v3.rs:983-1008` reads keep their offsets |
| `authenticate_order_execution_v1` | `authenticate_order_execution_v2` | `hot_candidate_v3.rs` (VerifyCandidateRow projection) |
| `AuthenticatedOrderTermsV2 { .. }` constructors | + `side`, `outcome_lo`, `outcome_hi`, `claims_per_lot` | every test fixture; `runtime_settlement.rs` tests; `runtime_verify.rs` tests |
| `VerifiedCandidateV2::encode_into(header, inputs, outputs, out)` | `(header, prices, inputs, outputs, out)` | `runtime_verify.rs` terminal step (done in §1.4), any fixture |
| `VerifiedCandidateV2::encode_le_tails_into(header, inputs_le, outputs_le, out)` | `(header, prices_le, inputs_le, outputs_le, out)` | `runtime_verify.rs:870` (done in §1.4) |
| `build_materialize_packets_v2(mint, ctx, n, q, claims, custody)` | `(.., custody, refunding)` | `hot_candidate_v3.rs` Materialize arm; pass `Some(RefundingEscrowV1{..})` when the market's `categorical_refunds_on_failure_v3` is true (read where the founding reads it: `crates/dclutch-product` `ProductBasisV3` payout scale) |
| `CandidateHeaderV2 { .. }` constructors | + `live_order_count` | every candidate fixture; the operator's candidate builder copies `batch.live_order_count()` |

**§2.1 understates the blast radius on both axes**, and the convergence lane
should not trust its "one sed, then `cargo check -p dclutch-trading`" line.
Measured at this HEAD: **17 live V1 symbols, 495 uses across 20 files.** Every
one is a hard compile error, not a warning — `collection_v1.rs` now defines only
the `_V2` forms, so the V1 names have no definition at all.

| symbol | uses | | symbol | uses |
| --- | ---: | --- | --- | ---: |
| `GeneralOrderV1` | 128 | | `general_order_len_v1` | 27 |
| `GeneralBatchV1` | 127 | | `GeneralOrderHeaderV1` | 23 |
| `GeneralBatchLayoutV1` | 68 | | `authenticate_order_execution_v1` | 16 |
| `GeneralOrderLayoutV1` | 66 | | `GeneralSignedOrderTermsV1` | 12 |
| `GENERAL_ORDER_ROW_BASE_V1` | 12 | | `general_signed_order_terms_len_v1` | 12 |
| `GENERAL_ORDER_ROW_STRIDE_V1` | 11 | | `general_order_identity_v1` | 4 |
| `GENERAL_ORDER_HEADER_BYTES_V1` | 4 | | `GENERAL_ORDER_ROW_RECEIVE_OFFSET_V1` | 4 |
| `GENERAL_ORDER_ROW_DELIVER_OFFSET_V1` | 4 | | `GENERAL_ORDER_STATE_OFFSET_V1` | 1 |

Already at zero, nothing to do: `GeneralBatchStateV1`,
`GENERAL_ORDER_STATE_BYTES_V1`, `GENERAL_SIGNED_ORDER_TERMS_ROW_BASE_V1`.

Per-file, descending. **Bold rows are files §2.1 never names** — ten files, 166
uses, a third of the radius, and three of them are in crates §2.1 does not
mention at all (`dclutch-operator`, `dclutch-accelerator-sbf`,
`tools/local-validator`). **The sed must run tree-wide and the check must be
workspace-wide.**

| file | hits |
| --- | ---: |
| `crates/dclutch-trading/src/general/collection_v1/tests.rs` | 95 |
| `crates/dclutch-trading/src/general/account_rules_v3.rs` | 66 |
| `crates/dclutch-trading/src/general/hot_candidate_v3.rs` | 49 |
| **`programs/dclutch-accelerator-sbf/program-test/tests/lifecycle.rs`** | **46** |
| `crates/dclutch-trading/src/general/effect_artifacts_v3.rs` | 44 |
| **`crates/dclutch-operator/src/general_hot_v3.rs`** | **38** |
| `crates/dclutch-trading/src/general/escrow_v1/tests.rs` | 18 |
| `crates/dclutch-trading/src/general/state_artifacts_v3.rs` | 16 |
| **`crates/dclutch-trading/src/general/candidate_v1/tests.rs`** | **16** |
| **`tools/local-validator/bootstrap/successor/src/general_settlement_fixture.rs`** | **14** |
| `programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs` | 14 |
| **`programs/dclutch-trading-sbf/program-test/bundle-builder/tests/general_dynamic_spans_v1.rs`** | **13** |
| `programs/dclutch-trading-sbf/program-test/bundle-builder/src/general.rs` | 12 |
| **`crates/dclutch-trading/src/general/local_state_v3.rs`** | **11** |
| **`crates/dclutch-trading/src/general/candidate_v1.rs`** | **8** |
| **`crates/dclutch-trading/src/general/transition_artifacts_v3.rs`** | **6** |
| `crates/dclutch-trading/src/general/escrow_v1.rs` | 6 |
| **`programs/dclutch-accelerator-sbf/src/general.rs`** | **5** |
| **`crates/dclutch-trading/src/general/gen_seven_v1.rs`** | **5** |
| **`programs/dclutch-accelerator-sbf/program-test/tests/freeze.rs`** | **4** |

Exact lines for the small files, where a `sed -i` is safe:
`escrow_v1.rs` 73, 828, 829, 967, 999, 1000 · `gen_seven_v1.rs` 22, 468, 738,
743, 856 · `transition_artifacts_v3.rs` 37, 712, 714, 717, 867, 869 ·
`accelerator-sbf/src/general.rs` 39, 1014, 1141, 1482, 1496 ·
`accelerator-sbf/program-test/tests/freeze.rs` 22, 164, 173, 679 ·
`state_artifacts_v3.rs` 31, 32, 974, 1308, 1527, 1529, 1532, 1548, 1556, 1558,
1561, 1582, 1584, 1587, 1600, 1653 · `local_state_v3.rs` 10, 216, 282, 474.

Non-mechanical residue the sed cannot do (§2.1 flags these and they are real):
the `AuthenticatedOrderTermsV2` and `CandidateHeaderV2` constructors gain fields
at every fixture; `VerifiedCandidateV2::encode_into` / `encode_le_tails_into`
gain a leading `prices` argument (call sites:
`tools/local-validator/bootstrap/successor/src/family_hot_campaign.rs:1009`,
`crates/dclutch-trading/tests/general__selection_decision_corpus.rs:68`);
`build_materialize_packets_v2` gains its trailing `refunding`.
`crates/dclutch-operator/src/general_joint_clearing_v1.rs:437` is the only
`CandidateHeaderV2` site already correct.

### 2.2 BLOCKING — the branch does not parse, and these four are why

`cargo check -p dclutch-trading --offline` at this HEAD fails **before reaching
`dclutch-trading`**, in `dclutch-claims`. Errors verbatim in §7.

| # | file:line | the one-line change |
| --- | --- | --- |
| S1 | `crates/dclutch-claims/src/lib.rs:477` | `let shape_valid = match self.action {` is non-exhaustive: `ClaimsAction::StrandResidual` not covered. Add the arm (a strand carries a source and no destination). |
| S2 | `crates/dclutch-claims/src/lib.rs:635` | `let (source_present, destination_present) = match value.action {` — same variant, same fix; `(true, false)`. |
| S3 | `crates/dclutch-trading/src/general/mod.rs:66` | `collection_v1.rs:52-53` does `use crate::general::generated_clearing_price_v1 as clearing_wire;` and `… generated_order_v2 as order_wire;`, and `runtime_verify.rs:362,364,372,373` reach `generated_order_v2::ORDER_SIDE_*_V2`. **Neither module is declared.** Add both in the four-line form the neighbouring `generated_runtime_wire_v2` block uses (`#[rustfmt::skip]` / `#[allow(dead_code, missing_docs)]` / `#[path = "…"]` / `mod …;`). |
| S4 | `runtime_verify.rs:2150` and `runtime_settlement.rs:1072` | Both now say `#[cfg(test)] mod tests;`, resolving to `src/general/runtime_verify/tests.rs` and `src/general/runtime_settlement/tests.rs`. **Neither file exists** and neither directory exists. The sibling pattern is already in-tree at `collection_v1/`, `escrow_v1/`, `candidate_v1/`, `plan/`, `state_seeds_v3/`. Restore main's inline modules there, updated for the new signatures — §1.5 names the eight settlement tests that were dropped. |

### 2.3 `ClaimsAction::StrandResidual = 10` — the program side

Defined `crates/dclutch-claims/src/lib.rs:240`, decoded `:253`.

| # | file:line | the one-line change |
| --- | --- | --- |
| S5 | `programs/dclutch-claims-sbf/src/lib.rs:632` | `let basket_action = match plan.action() { … }` (`:626-632`) has seven arms and **no `_` fallback**; a tenth `ClaimsAction` breaks it. Insert the arm after `ClaimsAction::InitializeCompleteSet => return Err(ClaimsSbfError::Instruction.into()),`. |
| S6 | `crates/dclutch-product/src/economic_slice/mod.rs:154` | S5's arm has nothing to map to: `BasketAction` has exactly six variants and none is a non-uniform per-coordinate burn. Add a seventh `BasketAction::StrandResidual` after `MergeRefundingCompleteSet`, then audit `impl BasketAction` (`:157+`; `moves_complete_set()` must return false) and the three claims-sbf consumers that take a `basket_action`: `authenticate_failure_escrow` (`:636`), `authenticate_complete_set_growth` (`:637`), `execute_plan_economics` (`:638`). |
| S7 | `programs/dclutch-claims-sbf/src/lib.rs:339` | Band `0x5` currently ends at `0x5011 FailureEscrowUnseated`. Add `Strand = 0x5012` and `StrandUnseated = 0x5013`. |
| S8 | `programs/dclutch-claims-sbf/src/lib.rs:363` | Append both names to the `dclutch_refusal_registry::pin_refusal_band!(ClaimsSbfError, …)` list or the pin fails. |
| S9 | `docs/reference/refusals.md:157` | Two rows after the `0x5011` row. This file is **authored, not generated** — it feeds `generate-refusal-registry.mjs` (`tools/gates/reference.py:16`). |
| S10 | `docs/reference/programs.md:17` | The claims row's refusal-code count moves by +2. |

`docs/reference/routes.md` needs **no** row: it carries one General-family line
(`:124`, the accelerator entry) and no per-action rows.

### 2.4 The orphans — the strand's pieces are built and never joined

This is the largest *functional* gap on the branch, and §1.5's "OWED" heading hid
it. Every piece of decision 0032 §0.2 exists; nothing calls any of them.

| symbol | file:line | note |
| --- | --- | --- |
| `build_strand_packets_v1` | `general/child_packets.rs:630` | **The strand's Claims leg is never built.** `runtime_settlement::close` emits the residual as effect quantities (`:802-803`) and stops. |
| `GeneralBatchV2::clear` | `general/collection_v1.rs:1287` | the Closed → Cleared transition. No caller — **nothing publishes the clearing.** |
| `GeneralBatchV2::encode_clearing_into` | `general/collection_v1.rs:900` | doc references at `:583`, `:804`, `:1285`; no call. |
| `project_general_clearing_into_bank_v3` | `general/hot_candidate_v3.rs:3945` | referenced only from a doc comment at `runtime_settlement.rs:23`. |
| `clear_book_v1` | `operator/general_joint_clearing_v1.rs:140` | the solver itself: one hit, its own definition. |
| `build_candidate_records_v1` | `operator/general_joint_clearing_v1.rs:423` | one hit. |
| `lex_min_prices` | `operator/general_joint_clearing_v1.rs:381` | called once, from `clear_book_v1:322` — reachable only through an orphan. |
| `GeneralChildEffectV1::StrandResidual` | `general/mod.rs:222` | produced only inside the orphaned packet builder; consumed at `escrow_v1.rs:177`. |
| `RuntimeVerifyErrorV2::ClaimImbalance` | `runtime_verify.rs:291` | deliberately retired, documented in place. Not debt. |

### 2.5 The client surface

| # | file:line | the one-line change |
| --- | --- | --- |
| S11 | `apps/dclutch-web/components/GeneralWorkspace.tsx:3` | `ClearingPriceHistory` is imported by **nothing** (`apps/dclutch-web/app/general/page.tsx:1-4` renders `GeneralWorkspace`, which reads the *V1* local-state projection from `generalPlanV5.ts:161`). Add the import and a render site. |
| S12 | **no file — a TS emitter does not exist** | `packages/dclutch-sdk/lib/generalClearingV1.ts` **hand-keeps its byte offsets**: the Lean emitters for these two wires (`EmitGeneralOrderV2AbiRust.lean`, `EmitClearingPriceV1AbiRust.lean`) target Rust only. The offsets are a second author of a fact Lean owns. The real fix is a TS emitter beside each Rust one, emitting `packages/dclutch-sdk/lib/generated/generalClearingV1.ts`, with an `abi:general-clearing:verify` script in `packages/dclutch-sdk/package.json` (only the `--check` form registers as a guard). Until then the drift is unguarded — and note the offsets **are correct today**, verified three ways (§1.7). |
| S13 | `packages/dclutch-sdk/lib/generalClearingV1.ts:208`, `:342`, `:343` | Identity encoding: the tree's convention (ruling 8, §3) is base58 for account **addresses** and hex only for content **digests**. These three call `id32` (`:174`), which returns hex, for `batch Market`, `order owner` and `order Market`. Switch them to the base58 form — `pubkey()` in `lib/bytes.ts:54`, the same thing `generalPlanV5.ts:403`'s `pubkeyHex` does despite its name. **Leave alone** (correctly hex): `productId`, `configId`, `batchId`, `clearedCandidateId` (`:303`) and `generalOrderIdV2` (`:409`), which are digests — that is what `generalPlanV5.ts:407`'s `idHex` is for. |
| S14 | `packages/dclutch-sdk/scripts/generate-general-successor-v5.mjs:210,212` | The generator greps for the literal lines `const BATCH_MAGIC: [u8; 8] = *b"DCGBAT01";` and `const ORDER_MAGIC: [u8; 8] = *b"DCGORD01";` in `collection_v1.rs`. **Those exact lines no longer exist** — `:92-94` now read the magics from the wire modules. `npm run abi:general-v5:verify` will fail. |
| S15 | `docs/reference/abi/generalSuccessorV5.md:28,30` | Still documents `DCGBAT01`/`DCGORD01` byte-for-byte; stale for the V2 records. A `generalClearingV1.md` page is owed. |

No manifest changes are needed: `crates/dclutch-operator/Cargo.toml:43` already
has `dclutch-trading` (and `:40`/`:48` `dclutch-claims`/`dclutch-product` for the
strand leg); `packages/dclutch-sdk/package.json:11-17` resolves
`@dclutch/sdk/generalClearingV1` through a wildcard `exports` with no per-file
list, and lint/test pick the new files up by directory.

### 2.6 Emission guards and registries

| # | file:line | the one-line change |
| --- | --- | --- |
| S16 | `tools/gates/emission-coverage.md:58` and `:146` | Four rows: two guard rows (`crates/dclutch-trading/tests/general__clearing_price_v1_generator_fresh.rs` and `…general__order_v2_generator_fresh.rs`, kind `cargo-test`, gated yes) before the `general__request_profiles` row, and two generated-file rows for `generated_clearing_price_v1.rs` / `generated_order_v2.rs` before the `generated_request_profiles_v1.rs` row. The file is byte-gated (`tools/gates/emission.py:381-396`, *"emission-coverage.md no longer describes this tree"*). Run `tools/gate emission --write` and read the diff. |
| S17 | `tools/gates/root-targets.tsv` | **No rows — §2's earlier premise was wrong here.** `tools/gates/tiers.py:676` tags any test whose source contains `"lake"` as not-cheap, and `root_targets_check` (`:708`) filters to cheap ones; both new guards call `Command::new("lake")`. Zero of the twelve existing `*_generator_fresh.rs` targets appear in that file. **Adding rows would trip the `ORPHANED` check at `tiers.py:719`.** |
| S18 | `tools/gauntlet/census/src/magics.rs:792-796` | Hardcodes `private("dclutch-trading", "ORDER_MAGIC", "DCGORD01", "…/collection_v1.rs:74")`. At HEAD `:74` is no longer that constant. It is a hardcoded-array unit test so it will not *fail*, but its file:line is dangling and its claim about "the tree's live instance" is false. Note also that `DCGBAT02` and `DCGORD02` were **already live on main** under different names (`general/lifecycle.rs:18`, `general/runtime_manifest.rs:17`); the branch adds a second public export of each value, one step closer to the two-exports-of-one-name rule at `magics.rs:806-818`. |
| S19 | `tools/gates/wire-vector-pins.tsv` | No fixture exists under `packages/dclutch-sdk/fixtures/` for either wire — the TS tests build bytes inline. Either add a `DCLUTCH_WRITE_WIRE_VECTOR=1` authoring test plus a row, or record the decision not to. Named debt, not blocking. Related to S12. |
| S20 | `tools/cohort/steps.tsv` | No `joint`/`clearing` rows. §6 assigns every moved width to cohort-18; those deploy steps are unwritten. |

### 2.7 Width rows

The three runtime widths are **derived** from the generated wire constants and
follow automatically once S3 lands. Only batch and order carry literal or aliased
widths, and at those sites the width move and the §2.1 rename are the same edit.

| width | main → HEAD | authority at HEAD | rows that must move |
| --- | --- | --- | --- |
| batch | `224` → `296 + 16N` | `general_batch_len_v2` `collection_v1.rs:1327`; the V1 prefix stays as `GENERAL_BATCH_BYTES_V1 = 224` `:64` | `state_artifacts_v3.rs:971`, `:1302`; `account_rules_v3.rs:2094`, `:2167`, `:3311` |
| order | `192 + 16N` → `216 + 16N` | `general_order_len_v2` `:1957`; `GENERAL_ORDER_ROW_BASE_V2 = 216` `:81` | `state_artifacts_v3.rs:974`, `:1308`; `account_rules_v3.rs:2097`, `:2197`; `local_state_v3.rs:216`, `:282`, `:474`; `effect_artifacts_v3.rs:1490`, `:1502` |
| candidate header | `128` → `136` | `generated_runtime_wire_v2.rs`, surfaced `runtime_width.rs:32` | all derived — `runtime_width.rs:299,320,390,407,1401` |
| verifier cursor | `288 + 40N` → `288 + 56N` | `runtime_verify.rs:51`+`:55`, `runtime_verifier_len_v2` `:752` | `general_settlement_fixture.rs:412`, `:671` |
| certificate | `160 + 16N` → `160 + 24N` | `verified_candidate_len` `runtime_width.rs:1428-1432` | `general_settlement_fixture.rs:414`, `:675`; `family_hot_campaign.rs:1009`; `tests/general__selection_decision_corpus.rs:68` |

`crates/dclutch-vm/check-generated-account-profile.sh` guards the **generic**
`ACCOUNT_PROFILE_*` ABI and encodes no General record width — untouched by this
branch. There is no per-family width table under `crates/dclutch-vm/src/`; the
General widths live only in `state_artifacts_v3.rs` and `account_rules_v3.rs`.
The likeliest place a hardcoded `224` or `192 + 16N` survives is
`programs/dclutch-trading-sbf/program-test/bundle-builder/tests/general_dynamic_spans_v1.rs`
(13 rename hits, and it is the dynamic account-sizing span table).

## 3. Ruled provisionally

1. The tie-break is a verifier conjunct (§0.1); the selection policy gains no
   criterion. Reversal cost: delete one conjunct, add `MinimizePriceVector`
   to `SelectionCriterion` — one Lean criterion, one comparison arm.
2. Rationing among orders exactly marginal at the price is NOT enforced by the
   chain (pro-rata needs the marginal total before the rows stream; the
   verifier is one pass). It stays a solver discretion broken by
   `MinimizeCandidateId`; the operator's solver implements 0032 §2b's pro-rata
   rule so the canonical candidate is the pro-rata one. Named debt.
3. Cohort-18 admits single-outcome orders only; the layout carries the
   interval, `BundleNotAdmitted` refuses `lo < hi` by name until the interval
   minimality conjunct exists (an LP sequence, not a box).
4. The rows stay on the order wire as derived transport (the emitted
   PlaceOrder effect program writes them); deleting them is cohort-19's and
   moves `GeneralTransitionV3.lean`'s PlaceOrder program.
5. The residual strands inside `Close` (§0.2); `Close` gains a Claims leg.
6. The clearing lives on the batch record (§0.3).
7. The refunding variant of the in-batch mint uses the existing
   `Mint/MergeRefundingCompleteSet` Claims actions with the escrow in the
   vacant slot; the strand on a refunding market burns ordinary coordinates
   only (the escrow's failure claims are the escrow's, never the batch's).
8. The solver's algorithm: the note's §1.6 greedy for a single-outcome book,
   then the lex-min price greedy, then pro-rata rationing by lots with the
   remainder by ascending order id. Not the LP: no interval orders this cohort.
9. The note's option for the output page (D6) is not taken here; nothing in
   this branch depends on it.

8. **(CLOSEOUT, provisional — ember rules by reversal.)** **Identity encoding in
   the SDK follows the sibling convention in `generalPlanV5.ts`: base58 for
   account ADDRESSES, hex for CONTENT DIGESTS.** The tree already splits these
   two: `generalPlanV5.ts:403`'s `pubkeyHex` returns
   `new PublicKey(value).toBase58()` despite its name, and is used for `market`
   and `owner`; `:407`'s `idHex` returns hex and is used for `batchId`,
   `productId`, `configId`. The child agent that built `generalClearingV1.ts`
   used hex for all of them per its brief, giving the tree two conventions.
   Seam S13 names the exact three lines. Reversal cost: three call sites.

## 4. Stubbed, and why

**Nothing is marked.** No `todo!()`, `unimplemented!()`, `TODO`, `FIXME`, `XXX`,
`OWED` or `stub` appears in any of the 20 new or changed Rust, TS and TSX files.

That is worth stating plainly, because it is the opposite of reassuring: this
branch's incompleteness is **structural, not annotated**. Four kinds of hole,
none of which any grep for a marker will find:

1. **Two module files that do not exist** (`runtime_verify/tests.rs`,
   `runtime_settlement/tests.rs`) behind `mod tests;` declarations — seam S4.
2. **Two `mod` declarations that were never added** for modules the code already
   imports — seam S3.
3. **Nine orphans** (§2.4): the strand's every piece built, none joined.
4. **Two claims in doc comments that are false** — `general_joint_clearing_v1.rs:16-17`
   promises a differential test that does not exist, and this doc's own §0.1
   named `lexmin_price_vector`, which does not exist either.

The one deliberate no-op is `RuntimeVerifyErrorV2::ClaimImbalance`
(`runtime_verify.rs:291`), retired in place and documented at `:286-290`. That
is a named retirement, not debt.

## 5. Tests written, and what each proves

**Net Rust test balance: −8.** Two generator-freshness guards were added; eight
settlement tests and the whole `runtime_verify` inline module were deleted. There
are **zero tests for the 512-line operator solver and zero for the five new
verifier conjuncts** — the two pieces of new logic that most need them.

### Rust (2)

| test | proves |
| --- | --- |
| `tests/general__order_v2_generator_fresh.rs:12` `checked_in_general_order_v2_is_exact_lean_output` | the committed `generated_order_v2.rs` is byte-identical to what `EmitGeneralOrderV2AbiRust.lean` prints — i.e. the hand-regeneration was faithful. |
| `tests/general__clearing_price_v1_generator_fresh.rs:12` `checked_in_general_clearing_price_v1_is_exact_lean_output` | the same for the clearing tail. |

### TypeScript — `packages/dclutch-sdk/lib/generalClearingV1.test.ts` (13)

`decodeGeneralBatchV2` (`:126`): decodes a vacant closed batch with no clearing
`:127` · decodes a cleared batch against **the design note's own witness** —
prices `[60,40,0]`, residual `[0,0,8]` `:134` · `clearingPricesV1` returns null
for an uncleared batch `:151` · refuses a cleared status over a vacant tail
`:155` · refuses a closed status over a nonvacant tail `:161` · refuses prices
off the simplex `:167` · refuses a residual on a priced outcome `:172`.

Those last four are the TS twin of Lean's `tailAdmissible`,
`a_cleared_tail_is_on_the_simplex` and `a_cleared_tail_strands_only_at_zero_price`
— i.e. the reader refuses exactly what the ABI module forbids, which is the
property the family most needs held at the client edge.

`decodeGeneralOrderV2` (`:178`): decodes a placed single-outcome buy `:179` ·
decodes a placed sell across a wider interval `:186` · refuses rows that disagree
with the shape `:192` · refuses an empty interval `:197` · refuses an unknown
side `:202` · derives the identity as the sha256 of the 184-byte header alone
`:207` (the twin of `general_order_identity_v2`).

**A real finding from that last group.** The TS decoder gives an unknown `side`
tag **its own named reason** (`generalClearingV1.ts:352`, "General order V2 side
tag is unknown"). The program folds the same condition into the generic
`ShapeNotInterval` (`collection_v1.rs:2129-2130`,
`OrderSideV2::decode(...).ok_or(GeneralCollectionErrorV1::ShapeNotInterval)?`) —
the same code `:1561` raises for a genuine shape violation. **The program is the
coarser of the two.** Whoever splits that code should take the TS reader's
granularity, not the other way round: a bad side byte and a bad interval are
different operator mistakes.

### TSX — `apps/dclutch-web/components/ClearingPriceHistory.test.tsx` (5)

renders nothing-to-show when no batch has opened `:67` · lists an uncleared batch
by its status alone `:71` · renders a cleared batch's sequence, cleared slot,
per-outcome prices, move and stranded residual `:78` · orders rows by sequence
regardless of input order `:91` · **presents no market-data metric — raw facts
only** `:99`, which is the aliveness-rule the tree holds its surfaces to.

## 6. Frames and codes expected to move, and which cohort carries them

- Every General artifact digest (the order layout is in the signed terms; the
  hot bank widened 151 → 154 common scalars and stride 6 → 7): **cohort-18**.
- `RuntimeVerifyErrorV2` gains five variants (§1.4); they reach the log
  through `log_line()` and the chain through Trading's family refusal in band
  `0x4` — no new code number is allocated by this branch (the accelerator
  publishes one canonical refused acknowledgement; the variant is the log
  line). `GeneralCollectionErrorV1` gains five (internal, mapped through
  `GeneralHotCandidateErrorV3`).
- `ClaimsAction::StrandResidual` needs an SBF arm in `dclutch-claims-sbf`
  and one or two codes in band `0x5` (`Strand`, `StrandUnseated`): named in §2.
- The verifier cursor widens `288 + 40N` → `288 + 56N`; the verified
  certificate `160 + 16N` → `160 + 24N`; the candidate header 128 → 136; the
  order record `192 + 16N` → `216 + 16N`; the batch `224` → `296 + 16N`.
  Every General account-profile width row moves: cohort-18.

## 7. Checks run here

- `lake`: background read-only check against the live tree's cache, result
  pending at the time of that section; **it never landed** and no Lean on this
  branch has been elaborated. The 55 `JointClearingV1` theorems, the two new ABI
  modules and the `EconomicKernel` strand laws are unchecked.
- `cargo check -p dclutch-trading --offline` (CLOSEOUT-C, shared target dir):
  **RED, exit 101.** It does not reach `dclutch-trading` — it fails in the
  dependency `dclutch-claims`. The first (and only) two errors, verbatim:

```
error[E0004]: non-exhaustive patterns: `ClaimsAction::StrandResidual` not covered
   --> crates/dclutch-claims/src/lib.rs:477:33
    |
477 |         let shape_valid = match self.action {
    |                                 ^^^^^^^^^^^ pattern `ClaimsAction::StrandResidual` not covered

error[E0004]: non-exhaustive patterns: `ClaimsAction::StrandResidual` not covered
   --> crates/dclutch-claims/src/lib.rs:635:59
    |
635 |         let (source_present, destination_present) = match value.action {
    |                                                     ^^^^^^^^^^^^ pattern `ClaimsAction::StrandResidual` not covered

error: could not compile `dclutch-claims` (lib) due to 2 previous errors
```

  **So the state of `dclutch-trading` is UNKNOWN**, not green and not red: the
  compiler never opened it. Seams S1 and S2 are two match arms in the family's
  own contract crate, and until they land no check of this branch says anything
  about the 495 renames, the two missing `mod` declarations, or the two missing
  test files. Expect S1/S2 to be the first ten minutes and S3/S4 the next hour.

## 8. STOPPED HERE — the next three steps

The maker got the design, the Lean, the contract crate, the verifier arm, the
close, the solver and the whole client surface written. What it did not get to is
**joining them** — and the branch's own doc headings ("OWED", "IN PROGRESS") hid
that the pieces were finished rather than missing.

1. **Make it parse, in exactly this order** — each step unblocks the next, and
   nothing downstream can be judged before them:
   S1 + S2 (the two `ClaimsAction::StrandResidual` match arms,
   `crates/dclutch-claims/src/lib.rs:477` and `:635`) → S3 (the two `mod`
   declarations in `general/mod.rs:66`, without which `collection_v1.rs` and
   `runtime_verify.rs` cannot resolve their wire modules) → S4 (create
   `runtime_verify/tests.rs` and `runtime_settlement/tests.rs`, restoring main's
   inline modules) → the §2.1 rename **tree-wide**, not `-p dclutch-trading`,
   then `cargo check --workspace`. Budget for the four non-mechanical signature
   fixups the sed cannot do.
2. **Join the strand (§2.4).** `runtime_settlement::close` must call
   `build_strand_packets_v1`, then `GeneralBatchV2::clear` and
   `encode_clearing_into`, so that the clearing is actually published and the
   residual actually burns. This needs S5 and S6 first (`BasketAction` has no
   strand image, so the claims-sbf arm has nothing to map to), then S7–S10 for
   the two band-`0x5` codes. Without this step the branch has a verifier that
   proves a clearing and a chain that never records one — the family's whole
   point, unwired.
3. **Write the operator's differential test**, the one
   `general_joint_clearing_v1.rs:16-17` already claims exists: stream every
   clearing `clear_book_v1` produces through `runtime_verify`'s conjuncts and
   assert none is refused. That is the single test that would justify both the
   solver and the five new verifier variants, and it is the cheapest way to find
   out whether ruling 1's lex-min greedy and ruling 2's pro-rata actually agree
   with the chain. Delete the doc sentence if the test is not written.

Deferred deliberately, and in this order after the above: S11 (mount
`ClearingPriceHistory`), S13 (the three base58 fields), S12 (the TS emitter that
removes the second author of the offsets), S14/S15 (the stale `abi:general-v5`
generator and its ABI page), then S16–S20 (the census, magic-register and cohort
rows). None of them can be judged while the branch does not compile, and S12 in
particular should wait until the offsets have survived one real integration —
emitting a TS twin of a layout that is about to move would just create a third
author to fix.
