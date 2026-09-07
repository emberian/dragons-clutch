# MERGE_NOTES_JOINT-CLEARING

Branch `build/joint-clearing`, rebased onto `main` at `670a1894f`. The rebase was a plain
`git rebase main` and took no conflicts; commits are UNSIGNED because 1Password refused to sign
while this lane ran.

## 1. READY or NOT

**NOT — but for one reason only, and it is a frame, not a defect.** Everything the branch built now
compiles, elaborates and passes; the family's two on-chain writes (the strand's Claims burn and the
clearing's publication to the batch record) have no frame to happen in, and giving them one is a
cohort-18 AccountProfile change that lands in three files `build/general-lifecycle` also edits.

### 1.1 What compiles and what passes

```
cargo check -p dclutch-trading  --tests --offline                       0 errors, 0 warnings
cargo check -p dclutch-operator --tests --offline                       0 errors
cargo test  -p dclutch-trading  --lib -- general::                    270 passed, 0 failed
cargo test  -p dclutch-operator --lib -- general_hot_v3                23 passed, 0 failed
cargo test  -p dclutch-trading  --test general__joint_clearing_conjuncts  10 passed
cargo test  -p dclutch-trading  --test general__strand_packets             4 passed
cargo test  -p dclutch-operator --test general_joint_clearing_v1_differential 10 passed
cargo test  -p dclutch-trading  --test general__order_v2_generator_fresh          1 passed
cargo test  -p dclutch-trading  --test general__clearing_price_v1_generator_fresh 1 passed
cargo test  -p dclutch-trading  --test general__runtime_wire_v2_generator_fresh   1 passed
cargo test  -p dclutch-trading  --test general__transition_programs_generator_fresh 1 passed
lake build (whole DClutchSemantics library)                           139 jobs, green
cargo fmt -p dclutch-trading -p dclutch-operator                      clean
```

**§1.1a — and this is the finding the BUILD doc's own check could not have made.** The branch arrived
red in `dclutch-claims`, so nothing downstream had ever been compiled, let alone run. With the crate
compiling, the General module's unit tests were **22 red out of 270**. Reverting the hot-bank
widening (§3.3 ruling 5) took that to 17; the rest were fixture and width breakage from the two moved
record layouts, and the one that remains is Wall C. The verbatim first error of the run that matters:

```
---- general::transition_artifacts_v3::tests::every_authored_program_is_byte_identical_to_the_lean_authored_one stdout ----
assertion `left == right` failed: OpenBatch built a program the Lean module did not author
```

A green `cargo check` on this branch says nothing about whether it works. Judge it by the test run.

### 1.1b — one library defect the tests found that this lane fixed only halfway

`crates/dclutch-trading/src/general/local_state_v3.rs`'s Batch envelope arm was **provably dead**:
`decode` required `body.len() == GENERAL_BATCH_BYTES_V1` (224) and then called
`GeneralBatchV2::decode`, which requires exactly `296 + 16N`, and `296 + 16N = 224` has no solution.
No Batch envelope could decode, and `general_local_state_len_v3(Batch, n)` sized the account 224
bytes — too small for the record it wraps. The envelope is now Product-width, mirroring the Order
arm (`general_batch_len_v2` in the length, decode-then-check-length in `decode`, `outcome_count` read
off the body in the atomic encoder, and `is_fixed_width` listing `Selection | Candidate` only).

**The physical-account layer still disagrees with it and is OWED.** Five sites size the batch at 224
with a zero per-outcome stride; the correct shape is the one the Order recipe already has beside
them, base `GeneralBatchLayoutV2::PRICES_BASE` (296) with a stride of 16 (the price cell plus the
residual cell):

- `crates/dclutch-trading/src/general/account_rules_v3.rs:2094` (the `local_state_rule` semantic
  header for OpenBatch / CloseBatch / PlaceOrder / CancelOrder), `:2167` and `:3311` (the
  `ClosedBatch` readonly-evidence rule and its test expectation)
- `crates/dclutch-trading/src/general/state_artifacts_v3.rs:971` (`primary_shape` semantic bytes) and
  `:1302` (`batch_and_order_shape`'s batch base)

Their tests pass today only because nothing cross-checks them against the envelope. Moving them moves
those tests' expected spans, and both files are ones `build/general-lifecycle` also edits — so it is
the merge lane's, and it must land before a batch account is ever created at the new width.

### 1.2 The wall: the joint clearing's two on-chain writes have no frame

The branch built the strand and the clearing publication and joined neither, and the reason both are
unjoined is the same: **`Action::Close`'s AccountProfile and child-frame list carry neither the
account the clearing is written to nor a Claims route that can burn.**

**Wall A — the strand's Claims leg does not exist in the Close frame.**
`crates/dclutch-trading/src/general/effect_artifacts_v3.rs:189-210` gives `Action::Close` exactly
four child frames: `Custody(Transfer)` (the surplus), `ClaimsProtocolPosition(Close)` (closing the
settlement Position), `Custody(CloseVault)`, `Custody(CloseReplay)`. A residual burn is a
ProtocolPosition *mutation*, not a Position close, so it needs a FIFTH child frame and the account
count at `effect_artifacts_v3.rs:597-632` (Close = `10 + 14 + 15 + 14 + 10 + 1 + 1 = 65`) moves with
it, shifting every downstream coordinate.

`BUILD_JOINT-CLEARING.md` §0.2 says the strand costs "no new frame shape beyond `Close` gaining a
Claims leg". **`Close` already has a Claims leg** — it is `ProtocolPositionActionV2::Close` — and the
strand is a different route. Distrust that sentence.

The consequence is not cosmetic. `runtime_settlement::close` sets `header.claims_active = strands`
(`runtime_settlement.rs:664`), and `hot_candidate_v3::position_geometry`'s Close arm returns
`Err(GeneralHotCandidateErrorV3::InvalidPlan)` for **every** Close with `claims_active`
(`hot_candidate_v3.rs:3809`). So on this branch a clearing that strands can be submitted, verified,
collected, materialized and distributed, and **its settlement can then never close**. That refusal is
correctly fail-closed — accepting it would zero the cursor's inventory while the Position still held
the claims, which is a supply-conservation break — so this lane deliberately did NOT open it. It is
the one thing that must land before cohort-18 can deploy a market whose book strands.

**Wall B — the batch account is not in the Close frame at all.**
`Action::Close`'s 65-account profile has two writable local-state coordinates: 5 (the settlement
cursor, closed) and 6 (the terminal record). Its only readonly evidence is
`GeneralReadonlyEvidenceKindV3::SelectedVerifiedCandidate` at coordinate 9
(`state_artifacts_v3.rs:243`, `:268-271`); `GeneralReadonlyEvidenceKindV3::ClosedBatch` is selected
only by `SubmitCandidate`, `VerifyCandidateRow`, `CloseCandidate` and `Freeze`
(`state_artifacts_v3.rs:262-267`), never by `Close`. So `GeneralBatchV2::clear` and
`encode_clearing_into` cannot be called by the settlement that describes them: there is nothing in
the frame to write. §0.3's "no new PDA inside the heaviest frame of the family" and "written once by
`Close`" are in direct tension, and the doc does not notice.

There is precedent for a third writable state account — `Action::CancelOrder` writes the ORDER record
at coordinate 6 and the BATCH record's `CANCELLED_COUNT`/`COMMITTED_QUOTE_RESERVE` at coordinate 5
(`effect_artifacts_v3.rs:1530-1583`) — so the shape is not novel; it is the family's already-heaviest
frame gaining a 66th account plus a PDA recipe, a rule arm, an effect patch, a census row and a
frames-baseline row. That is a cohort-18 deploy change, not a convergence-lane edit, and it lands in
`account_rules_v3.rs` / `state_artifacts_v3.rs` / `effect_artifacts_v3.rs`, all three of which
`build/general-lifecycle` also edits.

A third option the merge lane should weigh instead: a sixteenth `Action::ClearBatch`, shaped exactly
like `CloseBatch` (which already has the batch writable), taking the selected certificate as readonly
evidence. It leaves the heaviest frame alone and gives both orphans a caller. Decision 0032 §0.2
ruled against a sixteenth action **for the strand**; it says nothing about publication.

**Wall C — the TransitionVM programs that WRITE the two moved records were Lean-authored and still
authored V1. FIXED HERE, and it was the most serious defect on the branch.**
`formal/dclutch-semantics/DClutchSemantics/GeneralTransitionV3.lean:73,77,95` held
`batchRecordMagicWord` (`DCGBAT01`), `batchRecordVersion` (`1`) and `orderRecordMagicWord`
(`DCGORD01`), while the Rust builder emitted `DCGBAT02` and version `2` — so **the OpenBatch program
the chain would run created a batch account the branch's own decoder refuses.** The byte gate at
`transition_artifacts_v3.rs:1458` was red and said exactly that:
`OpenBatch built a program the Lean module did not author`.

The order record needed one thing beyond the constants, and it is the part a byte gate could never
have found, because both authorities agreed on it. The PlaceOrder effect writes the record's version
out of the `one` register (`effect_artifacts_v3.rs:1439-1441`), and `one` in `placeOrder` is the
literal one the program compares the batch status against and clamps the custody flag with. That was
fine while the version was also `1`. At version `2` it writes a record at version 1 into a decoder
demanding 2. `placeOrder` now reloads `one` with a new `orderRecordVersion` as its LAST instruction,
after every conjunct that needs the count: 46 → 47 prelude instructions, 1232 → 1256 bytes, stated in
the Lean's own `authored_section_counts` and `authored_encoded_widths` theorems and in
`general_transition_instruction_count_v3` beside them.

`generated_transition_programs_v3.rs` was re-emitted (`lake env lean --run
EmitGeneralTransitionV3Rust.lean`, `rustfmt --edition 2024`, atomic replace, header validated).
`general__transition_programs_generator_fresh` and all fifteen `transition_artifacts_v3` tests pass.

**One more fact about the certificate, which constrains any of the three.** The clearing tail carries
`live_order_count` (`generated_clearing_price_v1.rs`, `CLEARING_LIVE_ORDER_COUNT_OFFSET_V1 = 288`),
and the verified certificate does NOT: `VERIFIED_CANDIDATE_HEADER_BYTES_V2 = 160` is full, its last
field `price_scale` occupying 152..160. Whatever frame publishes the clearing must therefore hold the
BATCH record, which is the count's one author (`GeneralBatchV2::live_order_count`, checked by
`GeneralBatchV2::clear`). A frame with only the certificate cannot write that field honestly.

## 2. SHARED FILES — for the merge lane

Checked with `git diff --name-only main...build/<family>` across the other eleven build branches.

### 2.1 Files another build branch also changes — NOT applied here, apply at merge

| file:line | the one-line change | who else touches it |
| --- | --- | --- |
| `programs/dclutch-claims-sbf/src/lib.rs:632` | in the `match plan.action()` at `:626-632` (seven arms, no `_` fallback), insert `ClaimsAction::StrandResidual => BasketAction::StrandResidual,` after the `InitializeCompleteSet` arm | `build/failure-arm`, `build/founder-bond` |
| `programs/dclutch-claims-sbf/src/lib.rs:339` | band `0x5` ends at `0x5011 FailureEscrowUnseated`; append `Strand = 0x5012` and `StrandUnseated = 0x5013` — **coordinate the numbers with failure-arm and founder-bond, which also add band-`0x5` codes** | same |
| `programs/dclutch-claims-sbf/src/lib.rs:363` | append both names to the `pin_refusal_band!(ClaimsSbfError, …)` list or the pin fails | same |
| `crates/dclutch-product/src/economic_slice/mod.rs:154` | add a seventh `BasketAction::StrandResidual` after `MergeRefundingCompleteSet`; `moves_complete_set()` must return `false` for it (`:157+`), and the three claims-sbf consumers at `:636`, `:637`, `:638` need arms | nobody — but it cannot land without the claims-sbf arm above, which is contended, so it is held here too |
| `docs/reference/refusals.md:157` | two rows after the `0x5011` row, for the two codes above. Authored, not generated (`tools/gates/reference.py:16` feeds it to `generate-refusal-registry.mjs`) | nobody, but depends on the numbers above |
| `docs/reference/programs.md:17` | the claims row's refusal-code count moves by `+2` | same |
| `tools/gates/emission-coverage.md:58` | two guard rows before the `general__request_profiles_generator_fresh.rs` row: `\| crates/dclutch-trading/tests/general__clearing_price_v1_generator_fresh.rs \| cargo-test \| yes \| EmitClearingPriceV1AbiRust.lean \|` and `\| crates/dclutch-trading/tests/general__order_v2_generator_fresh.rs \| cargo-test \| yes \| EmitGeneralOrderV2AbiRust.lean \|` | `build/general-lifecycle` |
| `tools/gates/emission-coverage.md:146` | two generated-file rows before the `generated_request_profiles_v1.rs` row: `\| crates/dclutch-trading/src/general/generated_clearing_price_v1.rs \| EmitClearingPriceV1AbiRust.lean \|` and `\| crates/dclutch-trading/src/general/generated_order_v2.rs \| EmitGeneralOrderV2AbiRust.lean \|`. The file is byte-gated (`tools/gates/emission.py:381-396`) — running `tools/gate emission --write` and reading the diff is safer than hand-editing | same |
| `tools/cohort/steps.tsv` | no `joint`/`clearing` rows exist; cohort-18 carries every moved width (§6 of the BUILD doc) and those deploy steps are unwritten | `build/failure-arm`, `build/series`, `build/claims-split-merge`, `build/founder-bond` |

### 2.2 Files this branch already rewrote that `build/general-lifecycle` also edits

`crates/dclutch-trading/src/general/{account_rules_v3.rs, effect_artifacts_v3.rs, hot_candidate_v3.rs,
mod.rs, state_artifacts_v3.rs}`. This family cannot leave them alone — the V1→V2 record rename touches
all five and `mod.rs` gains two `mod` declarations — so the merge lane has a real conflict there
either way. In `mod.rs` this branch's only additions are the two four-line generated-module blocks
(`generated_clearing_price_v1`, `generated_order_v2`) and `GeneralChildEffectV1::StrandResidual = 11`.

### 2.3 Not blocking, but stale and worth one line each

- `packages/dclutch-sdk/scripts/generate-general-successor-v5.mjs:210,212` greps for the literal
  lines `const BATCH_MAGIC: [u8; 8] = *b"DCGBAT01";` and `const ORDER_MAGIC: [u8; 8] = *b"DCGORD01";`
  in `collection_v1.rs`. **Verified: those lines no longer exist** — `collection_v1.rs:92,94` now read
  the magics from the wire modules. `npm run abi:general-v5:verify` will fail until the generator is
  taught the new form.
- `docs/reference/abi/generalSuccessorV5.md:28,30` still document `DCGBAT01`/`DCGORD01` byte for byte.
- `tools/gauntlet/census/src/magics.rs:792-796` hardcodes
  `private("dclutch-trading", "ORDER_MAGIC", "DCGORD01", "…/collection_v1.rs:74")`. **Verified: line 74
  is now `GENERAL_ORDER_HEADER_BYTES_V2` and the live private `ORDER_MAGIC` is at `:94` with value
  `DCGORD02`.** It is a hardcoded-array unit test so it will not fail; its citation and its claim are
  both false, and it cannot simply be updated — pointing both entries at `DCGORD02` would destroy the
  case the fixture exists to test (one name, two DIFFERENT private values).
- `tools/gates/root-targets.tsv` — **verified: no rows are owed.** `tools/gates/tiers.py:676` tags any
  test whose source contains `"lake"` as not-cheap and `root_targets_check` (`:708`) filters to cheap
  ones; zero of the twelve existing `*_generator_fresh.rs` targets appear in that file, and adding
  rows would trip the `ORPHANED` check at `tiers.py:719`. The BUILD doc's S17 is right.

## 3. What this lane completed, and what it ruled provisionally

### 3.1 Made it parse

1. **`crates/dclutch-claims/src/lib.rs:477`** — `ClaimsAction::StrandResidual` gains its own arm in
   the plan shape match: `source && !destination && source_revision && !destination_revision`, stated
   separately from `RedeemNativeTerminal | MergeCompleteSet` because the reason differs (a burn pays
   nobody, so there is no destination to name). It is deliberately NOT in the `complete_set` list at
   `:513-520`, which is what lets the strand carry an uneven quantity vector.
2. **`crates/dclutch-claims/src/lib.rs:635`** — the receipt's `(source_present, destination_present)`
   match gains `StrandResidual => (true, false)`.
3. **`crates/dclutch-trading/src/general/mod.rs`** — the two generated wire modules
   (`generated_clearing_price_v1`, `generated_order_v2`) are declared in the four-line form the
   neighbouring `generated_runtime_wire_v2` block uses. `collection_v1.rs:52-53` and
   `runtime_verify.rs:362-373` were already importing them.
4. **The V1→V2 record rename, tree-wide.** 16 live symbols across 20 files, applied with one
   word-boundary pass (`GeneralOrderV1`, `GeneralOrderLayoutV1`, `GeneralOrderHeaderV1`,
   `GeneralSignedOrderTermsV1`, `general_order_len_v1`, `general_signed_order_terms_len_v1`,
   `general_order_identity_v1`, `GENERAL_ORDER_{HEADER_BYTES,STATE_OFFSET,ROW_BASE,ROW_STRIDE,
   ROW_RECEIVE_OFFSET,ROW_DELIVER_OFFSET}_V1`, `GeneralBatchV1`, `GeneralBatchLayoutV1`,
   `authenticate_order_execution_v1`). `GENERAL_BATCH_BYTES_V1` correctly stays: it is the V1 prefix
   width, 224, and the account width is `general_batch_len_v2(N)`.
5. **`crates/dclutch-trading/src/general/runtime_verify/tests.rs` and `…/runtime_settlement/tests.rs`
   were created**, restoring `main`'s deleted inline modules — 777 and 530 lines. They needed more
   than a field-list update: the joint arm made the row shape SINGLE-OUTCOME (`current_shape`,
   `runtime_verify.rs:1516`, refuses a row that moves claims at more than one outcome), and every one
   of `main`'s fixtures used an all-outcomes `receive`/`deliver` vector. §4 records what changed.
6. Four non-mechanical fixups the rename could not do:
   `candidate_v1.rs:827,902` pass `execution.header().lots` to the widened
   `runtime_manifest_orders_for_row_v2`; `collection_v1.rs:1882`'s round-trip check names
   `GeneralSignedOrderTermsV2::decode` rather than `Self::decode`, whose `'a` the associated function
   cannot supply; and `project_general_clearing_into_bank_v3` takes `live_order_count` rather than
   reading it off a certificate that does not carry it (ruling 3 below).

### 3.2 Joined what could be joined

Nine symbols were named as orphans by the BUILD doc. Where each stands now:

| symbol | state |
| --- | --- |
| `clear_book_v1` | **joined** — `crates/dclutch-operator/tests/general_joint_clearing_v1_differential.rs`, the test the module doc already claimed existed |
| `build_candidate_records_v1` | **joined** — same |
| `lex_min_prices` | **joined** — same, plus a direct assertion against `JointClearingV1`'s own witness |
| `GeneralBatchV2::clear` | **joined off chain** — `general_joint_clearing_v1::publish_clearing_v1`; on chain it is Wall B |
| `GeneralBatchV2::encode_clearing_into` | **joined off chain** — same, and again as the way a closed batch's full-width record is produced |
| `build_strand_packets_v1` | **exercised, not called** — `crates/dclutch-trading/tests/general__strand_packets.rs`. See the finding below: this is not this branch's debt |
| `project_general_clearing_into_bank_v3` | **deleted**, with the four bank registers it wrote — they had no writer that could run and no frame to land in, and they were breaking the Lean register-schema join. Ruling 4 |
| `GeneralChildEffectV1::StrandResidual` | produced only inside the packet builder above; consumed at `escrow_v1.rs:177` |
| `RuntimeVerifyErrorV2::ClaimImbalance` | a documented retirement, not debt. Confirmed |

**A finding the BUILD doc's §2.4 gets wrong by omission.** `build_strand_packets_v1` has no caller —
and neither does any of its siblings. `build_materialize_packets_v2`, `build_row_packets_v2` and
`build_surplus_packet_v2` have **zero callers on `main`** as well (`git grep` over `main`, excluding
`child_packets.rs` itself, returns nothing), and the on-chain Claims CPI is composed by
`programs/dclutch-trading-sbf/src/claims_composition_v3.rs` from the EffectProgram route, not from
this module. So `child_packets` is a contract surface the program does not consume, and the strand's
builder is exactly as joined as every other builder in it. Listing it beside `GeneralBatchV2::clear`
made a tree-wide shape look like a family regression.

### 3.3 Ruled provisionally (ember rules by reversal)

1. **The Close's refusal of a stranding plan is left closed.** `position_geometry`'s Close arm
   (`hot_candidate_v3.rs:3809`) refuses every Close with `claims_active`, which today means every
   Close that strands. Opening it without Wall A's fifth child frame would let the settlement zero
   its cursor inventory while the Position still held the claims — a supply-conservation break in
   exchange for a green path. Reversal cost: the arm is three lines (one Position, source present,
   destination absent, aggregate and source direction `2` — byte-identical to the Merge geometry
   above it) and it must land in the same commit as the child frame, never before.
2. **`GeneralClearingFixedV1.sets_move` becomes a named enum.** It was `pub sets_move: u8` whose only
   admissible values were the private `clearing_wire::CLEARING_MOVE_*_V1` constants — a public field
   no caller outside `collection_v1.rs` could fill. It is now `GeneralClearingMoveV1 {None, Mint,
   Merge}` with `tag()`/`decode()`, in the shape `BatchStatusV1` beside it already had, and an
   unknown byte is refused as `InvalidClearing` at decode instead of falling through a `_ => false`.
   Reversal cost: one enum, six call sites, all inside `collection_v1.rs`.
3. **`live_order_count` is the batch's, and no certificate reader may invent it.** The first
   symptom was a compile error: `project_general_clearing_into_bank_v3` read
   `header.live_order_count` off the VERIFIED CERTIFICATE, which has no such field
   (`VERIFIED_CANDIDATE_HEADER_BYTES_V2 = 160` is full at `price_scale` 152..160) — the field is on
   the CANDIDATE header. Widening the certificate ABI would be a Lean edit, an emitter run this lane
   could not do, and a `160 + 24N → 168 + 24N` move through every fixture; so the rule is the other
   way round. **The batch record is the count's one author** (`GeneralBatchV2::live_order_count` =
   admitted less cancelled), the verifier holds the candidate's copy to it at `SubmitCandidate` and
   again at the terminal row (`OrderOmitted`), and `GeneralBatchV2::clear` refuses a published
   clearing whose count disagrees. Any future publisher must hold the batch record; a frame with only
   the certificate cannot write that field honestly. `publish_clearing_v1` obeys this; the deleted
   bank projection (ruling 4) could not have.
4. **The hot bank's clearing registers are reverted.** The branch widened
   `GENERAL_HOT_COMMON_SCALARS_V3` 151 → 154 (`CLEARING_LIVE_ORDER_COUNT`, `CLEARING_SETS_MOVE`,
   `CLEARING_SETS_QUANTITY`) and `GENERAL_HOT_ITEM_SCALAR_STRIDE_V3` 6 → 7 (`item_scalar::PRICE`),
   and `project_general_clearing_into_bank_v3` was their only writer. Three facts converged on
   deleting them:
   (a) `transition_artifacts_v3.rs:1630`, `the_lean_register_schema_is_the_one_the_rust_bank_declares`,
   is an explicit JOIN test — "if either side renumbers, the emitted programs address a bank the
   other side does not have" — and it was RED, because `GeneralTransitionV3.lean` still says 151/6;
   (b) the registers had no writer that could ever run: `apply_general_hot_candidate_v3` does not
   receive the verified certificate, so the projection could not be called from the arm its own doc
   comment named; and (c) even written, they had nowhere to land — the batch account is not in the
   Close frame (Wall B). A Lean-owned register bank does not move for registers nothing writes.
   Reverting also fixed four `artifacts_v3` geometry failures. Reversal cost: the four constants and
   one 60-line function, to be restored in the same commit as the frame that reads them.
5. **`GeneralBatchV2::encode_into` is added.** `to_bytes()` returns the 224-byte V1 prefix — what the
   OpenBatch/CloseBatch effects write — and `decode` requires the whole `296 + 16N`, so
   `decode(&to_bytes())` refused by length and took eight `hot_candidate_v3` fixtures, a
   `local_state_v3` envelope and a `collection_v1` round-trip with it. `encode_into` writes the whole
   record with the vacant tail and refuses a cleared batch by name; `to_bytes`'s contract is
   unchanged. Reversal cost: one four-line method.
6. **The tie-break stays a verifier conjunct**, as the BUILD doc's §0.1 ruled and decision 0032
   confirmed — NOT the selection criterion the design note recommends
   (`MECHANISM_JOINT_CLEARING_2026_09_04.md` §1.4, "applied by the selection policy among *certified*
   candidates only"). The note's own text marks that as "**Ruling owed (ember): the tie-break**", so
   the branch is resolving an open question rather than contradicting a settled one, and the conjunct
   form is strictly stronger. It is now tested both ways (§4).

## 4. Tests added, and what each proves

24 new Rust tests in three files, all passing, plus the two restored in-crate modules (1,307 lines,
`main`'s deleted `runtime_verify` and `runtime_settlement` inline tests) reshaped to the new
single-outcome row law. The 18 TypeScript and TSX cases the build wave wrote are unrun here (see the
end of this section).

### Rust — `crates/dclutch-trading/tests/general__joint_clearing_conjuncts.rs` (10)

Every fixture is `N = 2` or `N = 3`, `price_scale = 100`, one lot moving 100 claims, so a limit in
quote atoms per lot is numerically a price in scale units and every conjunct is checkable by hand.

| test | proves |
| --- | --- |
| `the_crossing_book_verifies_at_the_price_its_own_box_forces` | the positive control. Without it the nine refusals below could all be refusals of a fixture that never verified |
| `a_certificate_that_omits_a_live_order_is_refused_as_order_omitted` | `OrderOmitted` — the completeness conjunct at the terminal row |
| `prices_that_do_not_sum_to_the_scale_never_become_a_candidate` | `RuntimeWidthErrorV2::InvalidSimplex` at the Candidate record, before any row streams |
| `a_residual_on_a_priced_outcome_is_refused_as_priced_residual` | `PricedResidual` — complementary slackness — and, as its control, that the same fills with the residual outcome at zero verify |
| `a_price_vector_inside_the_box_but_off_its_minimum_is_refused_as_non_minimal` | `NonMinimalPriceVector` on a book whose box is the WHOLE simplex, so the tie-break is the only thing choosing. A vector outside the box refuses as `InvalidCursor` instead and would not have tested the conjunct |
| `an_order_left_short_strictly_inside_its_limit_is_refused_as_rationed_inside_limit` | `RationedInsideLimit` — the marginal conjunct |
| `a_seller_paid_below_their_floor_is_refused_as_credit_limit` | `CreditLimit`, the field the order record did not have before this branch |
| `a_buyer_charged_above_their_cap_is_refused_as_quote_limit` | `QuoteLimit`, its twin, so the pair is visibly two accusations and not one |
| `an_interval_order_is_refused_as_shape_not_interval_until_a_later_cohort` | `ShapeNotInterval` for `lo < hi` — provisional ruling 3 of the BUILD doc, held by a test |
| `a_row_that_disagrees_with_its_authenticated_shape_is_refused_as_shape_not_interval` | `ShapeNotInterval` for a row that moves claims where the terms say it does not |

### Rust — `crates/dclutch-trading/tests/general__strand_packets.rs` (4)

`build_strand_packets_v1` run against the Claims crate that will accept or refuse it: the packet is
Claims-only with no Custody leg, its plan decodes as `ClaimsAction::StrandResidual` with the
settlement owner as source and no destination, an all-zero residual and a wrong-width residual are
both refused as `ChildPacketError::Coordinate`, and — the one that matters — the SAME uneven quantity
vector that `ClaimsPlanV1::new` accepts for a strand is refused as `Error::InvalidQuantityVector` for
a merge. That pair is the strand's exception to uniformity, asserted rather than asserted-about.

### Rust — `crates/dclutch-operator/tests/general_joint_clearing_v1_differential.rs` (10)

The differential the solver's module doc already promised. Solve, lay the clearing out as Candidate
and Page records, stream every row through `evaluate_runtime_consider_row_v2`, and hold the
certificate to the solver's own answer: a crossing book, a mint, a strand at a zero-priced outcome
(N=3), and a thirteen-outcome book; each also streamed one row per page, because the cursor is
persisted and re-decoded at every page boundary and a clearing that verifies as one page and not as
N is a cursor defect. Two hostiles assert `NonMinimalPriceVector` and `OrderOmitted` end to end, and
`lex_min_prices` is checked against `JointClearingV1.lexMin 100 [0,0] [60,60] = [40,60]` and against
an infeasible box it must report rather than round away.

The last two tests are the whole arc on real records: open a batch, admit and escrow two signed
orders whose identities are the records' own digests, close it, solve THAT book, verify the clearing
through the chain, publish it onto the batch with `publish_clearing_v1`, and read the prices and the
residual back off the published account — then prove a batch clears once (`AlreadyCleared`) and that
a certificate for another batch is refused as `Substitution`.

### Lean — elaborated, and one proof was broken

`lake` WAS run here, and it is cheap. A build worktree has no `.lake`, so:

```
rsync -a /Users/ember/dev/dclutch/formal/dclutch-semantics/.lake/ \
         <worktree>/formal/dclutch-semantics/.lake/
cd <worktree>/formal/dclutch-semantics && lake build
```

517 MB of cache, and the whole `DClutchSemantics` library then builds in this worktree in about
twenty seconds (139 jobs, green). The cache was deleted again when this lane finished, because disk
is the binding constraint on this machine; recreate it with the two lines above. Result of the run:

| module | result |
| --- | --- |
| `DClutchSemantics.JointClearingV1` | **elaborates.** The tie-break greedy, the box, the strand laws and every witness. No `sorry` |
| `DClutchSemantics.GeneralOrderV2Abi` | **elaborates** |
| `DClutchSemantics.EconomicKernel` | **elaborates**, the appended `strandPost` laws included |
| `DClutchSemantics.ClearingPriceV1Abi` | **DID NOT ELABORATE.** `ClearingPriceV1Abi.lean:90:2: No goals to be solved` — `batch_width_is_two_ninety_six_plus_sixteen_per_outcome` ran `omega` after a `simp only` that had already closed the goal. Fixed here (the `omega` is deleted) and it now builds |

That is the shape of unchecked Lean: not a wrong theorem, a module that never compiled, so NONE of its
theorems held — including `a_cleared_tail_is_on_the_simplex` and
`a_cleared_tail_strands_only_at_zero_price`, the two the SDK decoder mirrors.

### The three emission guards — run, and green

With `.lake` warm, the guards that actually invoke `lake` were run:

```
general__clearing_price_v1_generator_fresh   1 passed
general__order_v2_generator_fresh            1 passed
general__runtime_wire_v2_generator_fresh     1 passed
```

So the three hand-regenerated files (`generated_clearing_price_v1.rs`, `generated_order_v2.rs`,
`generated_runtime_wire_v2.rs`) ARE byte-identical to their Lean emitters' output. And with the Rust
side now anchored to a Lean module that actually elaborates, `packages/dclutch-sdk/lib/
generalClearingV1.ts`'s 44 hand-kept offsets were compared against it field by field: **every one
agrees.** The SDK's numbers are right today; what they lack is a guard (seam S12, and the driver for
it already exists — `packages/dclutch-sdk/scripts/lean-emit.mjs` runs a Lean TS emitter against
`DClutchSemantics/TsEmit.lean`, so the work is two `Emit*Ts.lean` modules and two `package.json`
scripts, not a new mechanism). The BUILD doc's
"hand-regenerated to the emitter's output" is confirmed, and the candidate header's `128 → 136` and
the certificate's `tailCount 2 → 3` are Lean-authored, not hand-asserted.

### Not run in this worktree

- **The TypeScript and TSX tests.** No `node_modules` exists here and installing one is a gigabyte on
  a filesystem with 45 GiB free. The SDK change in §3 is three call sites and the existing cases do
  not assert an encoded identity form, but `npm test -w packages/dclutch-sdk` is owed.
- **The program tests at N=2 and N=13 with a CU reading.** `programs/dclutch-trading-sbf/program-test`
  reads its ELFs from `SBF_OUT_DIR` and `run-program-test.sh` builds them with `cargo build-sbf`; the
  convergence preamble forbids SBF program builds in this lane. What the note's figure actually says
  should also be carried forward, because it is easy to misread: **9.4 M is a whole-BATCH critical
  path at 2 LIVE ORDERS** (§3.3, `14 tx × ≈ 0.67 M`), not a per-transaction number, and the note
  writes `K` for the outcome width and `N` for live orders (§3, lines 295-297) — the tree's usual
  `N` is `K` there. This branch changes no transaction COUNT; it widens the verifier cursor
  `288 + 40N → 288 + 56N`, the certificate `160 + 16N → 160 + 24N`, the candidate header `128 → 136`
  and the settlement bank by `8·(3 + N)` bytes, all of which are per-transaction copies at
  K ≤ 13, against a measured per-transaction 0.67 M and a 1,399,700 ceiling.

## 5. What a reviewer should distrust in `BUILD_JOINT-CLEARING.md`

Checked against the diff, not taken on the doc's word.

1. **§0.2, "No new dispatch arm, no new frame shape beyond `Close` gaining a Claims leg."** False.
   `Close` already has a Claims leg (`ProtocolPositionActionV2::Close`), and the strand needs a fifth
   child frame it does not have. Wall A, §1.2.
2. **§0.3, "written once by `Close`" and "no new PDA inside the heaviest frame".** These cannot both
   hold: the batch account is not in `Close`'s profile at all. Wall B, §1.2.
3. **§0.1 names `runtime_verify.rs::lexmin_price_vector`.** Confirmed: no such symbol exists. The
   real one is `fn require_minimal_price_vector` at `runtime_verify.rs:1748`, raising at `:1782`.
   The doc's own §1.4 already corrects this; the §0 text was never fixed.
4. **§2.1's "one sed, then `cargo check -p dclutch-trading`".** Its own §2.1 addendum already says the
   radius is 20 files, and it is right: three of them are in crates §2.1 never mentions
   (`dclutch-operator`, `dclutch-accelerator-sbf`, `tools/local-validator`). Trust the addendum, not
   the heading. Two further files the addendum ALSO misses:
   `crates/dclutch-trading/src/general/admitted_accelerator_v3.rs` and
   `crates/dclutch-trading/src/general/runtime_selection.rs`, both of which call the widened
   `VerifiedCandidateV2::encode_into`.
5. **§2.4's orphan list reads as nine family regressions.** Four of them are: `clear`,
   `encode_clearing_into`, `project_general_clearing_into_bank_v3` and the solver's three. One is not:
   `build_strand_packets_v1` shares its status with every sibling in `child_packets`, on `main` and
   before this branch. §3.2.
6. **§1.6, "there are no stubs" in the solver.** True of `todo!()`, and it hid two things: a
   `let sets = *net.iter().max()?.max(&sets).min(&sets).max(&sets);` that evaluates to its own input
   (`.max(x).min(x)` is `x` for every value), and a `let _ = working.iter().map(…).count();` whose
   only purpose was to read a `Working.index` field nothing else used. Both are gone; the first is
   replaced by the derivation the verifier actually makes (`M` is the greatest net claim flow) with
   `SolverErrorV1::Infeasible` when the mint walk disagrees.
7. **§1.7's "verified every offset … and found no layout inconsistency".** The offsets themselves
   check out. What the child did not check is the ENCODING convention: three fields that are account
   ADDRESSES were rendered as hex. Fixed here (§3), and the BUILD doc's own closeout ruling 8 names
   the same three lines.
8. **§5's "Net Rust test balance: −8".** It was −8 for `runtime_settlement` alone; the whole
   `runtime_verify` inline module (777 lines) went with it. Both files are restored here.
9. **§7's "the proofs use `native_decide`, `simp`, `omega`" reads as a description of proofs that
   work.** One of the four modules did not compile at all (§4). `lake` is cheap here — rsync the live
   tree's `.lake` into the worktree and the four modules build in about twenty seconds — and a build
   wave that says "lake was not run" three times in one doc should be read as "these theorems are
   claims", not as "these theorems are unverified but probably fine".
10. **What the doc does NOT say, and should have.** `GeneralTransitionV3.lean` is the author of the
   programs that WRITE the batch and order records, and the branch moved both records' magic and
   version without touching it (Wall C, fixed here). §4.2's "New, by file" table lists neither the
   Lean transition module nor any AccountProfile file — which is the same blind spot twice: the
   branch changed two persisted layouts and never asked which Lean module WRITES them or which frame
   CARRIES them.
11. **The doc's structure invites the mistake it made.** "§1 What was built" is a table of files with
   checkmarks; "§2 THE SEAMS" is a table of one-line edits. Nothing in it asks *what runs this*. Every
   one of the four real defects this lane found — the uncompiled ABI module, the V1-authoring
   transition programs, the order version sharing the `one` register, the hot bank widened for
   registers with no frame — is invisible to both tables and visible to one `cargo test` and one
   `lake build`, together about ninety seconds.
