# MERGE_NOTES_JOINT-CLEARING-2

The twelfth family, finished and landed. `build/joint-clearing`, rebased twice
onto a moving `main` and merged `--no-ff`. `MERGE_NOTES_JOINT-CLEARING.md` is
the branch's own account of itself and stays as written; this is what the
landing lane had to do that that document could not, and what it found.

## 1. READY — and what changed the verdict

The branch declared itself NOT READY for one reason: its two on-chain writes
have no frame. That is still true and is not fixed here. What changed is that
the family's OTHER two blockers were finishable, and the wall itself is now
named, reasoned and tested rather than deferred:

* the V2 migration across the program-test files the family never compiled —
  **six of them, not four**;
* the batch account's five physical sites, still declaring the V1 prefix width;
* and, found by the gate rather than by a reader, **two wire magic collisions**
  the rename created.

## 2. The queue's resolution, reused

`docs/evidence/BUILD_WAVE_MERGE_2026_09_07.md` §3 records it and it is right:
main's nine refusal-clause enums with joint-clearing's V2 types, all seven hunks
of `hot_candidate_v3.rs`. Mechanically: take main's side, then apply the V1→V2
rename to the type names inside it. Taking either side whole loses the rename or
re-coarsens nine refusals.

The second rebase — onto a main that had since taken `recovery-capture` and
three reference regenerations — took exactly one further conflict:
`crates/dclutch-operator/src/lib.rs`'s sorted `pub mod` list. Union, as eight
times before it.

## 3. THE FIXTURES, and what each walk now asserts

This is the part the queue declined, and it declined it correctly: it is
authoring, not resolution. The joint arm makes an order a single-outcome
INTERVAL, `runtime_verify::current_shape` refuses a row that moves claims at
more than one outcome, and every fixture in the tree used an all-outcomes
vector.

**The rule, stated once because it is one decision in five files:**

1. every order is a ONE-OUTCOME interval on one side, and its per-lot vectors
   are DERIVED from `GeneralOrderHeaderV2::derived_row` rather than written
   beside the shape — the record's own decode refuses a disagreement
   (`RowsDisagreeWithShape`), so a fixture that spells its rows is a second
   author for them;
2. every maker sits exactly ON their own limit and is filled to their own
   maximum, so `QuoteLimit`, `CreditLimit` and the marginal conjunct
   (`RationedInsideLimit`) are each checked at their boundary rather than in
   their interior — where an off-by-one shows;
3. the price vector is the one the box FORCES, never one chosen to pass: the
   whole scale at the traded outcome and zero elsewhere, which is also decision
   0032's lexicographic minimum for that box;
4. `live_order_count` is read off the batch (`GeneralBatchV2::live_order_count`)
   and never typed, because that field IS the completeness conjunct.

**The books balance, and that is Wall A speaking rather than a convenience.**
A clearing that mints complete sets creates claims at EVERY outcome and hands
them to takers at only the ones that have any; every other outcome is a residual
the close must strand, and the strand's Claims burn has no child frame. So:

> A fixture with fewer orders than outcomes cannot mint a complete set and still
> reach a terminal close.

Two of these walk to a terminal close on a real ELF at width 258. They buy and
sell the same lots at one outcome instead: `M = 0`, no residual anywhere, and
the close is reachable at every runtime width. The mint is exercised where it
can be — the operator's differential test at N = 2, 3 and 13, the conjuncts
test's strand at a zero price, and the bundle-builder's lone buyer, which stops
at settlement initialization and never closes.

| file | what its book became |
| --- | --- |
| `tools/local-validator/bootstrap/successor/src/general_settlement_fixture.rs` | two buys of 2 lots and one sell of 4 at outcome 0, one claim per lot, the seller signing the only floor in the book — which is what makes the clearing price nonzero |
| `programs/dclutch-accelerator-sbf/program-test/tests/lifecycle.rs` | the same shape, with both buyers' caps moved 2 → 1 so they sit exactly at the clearing price, the mirror of the seller's floor sitting exactly at the fill |
| `programs/dclutch-trading-sbf/program-test/bundle-builder/tests/general_dynamic_spans_v1.rs` | one buyer filled to its maximum: the clearing MINTS, prices outcome zero at the whole scale, and strands at the other three — the one fixture that mints, and it never closes |
| `programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs` | one single-outcome buy of one claim per lot: the smallest order `GeneralOrderV2::decode` admits, since `validate_shape` refuses a zero `claims_per_lot` |
| `programs/dclutch-accelerator-sbf/program-test/tests/{freeze,hot_instruction_v3}.rs`, `tools/local-validator/bootstrap/successor/src/family_hot_campaign.rs` | selection-fold certificates that never stream through the verifier: their price tails must be ADMISSIBLE (on the simplex at the declared scale), not forced by a book |

`SELLER_CREDIT_FLOOR_CLEARS = 1` survives unchanged and for the same reason it
was one: the seller is paid `scale × 1 / scale = 1` per lot at every runtime
width. `SELLER_CREDIT_FLOOR_REFUSES = 2` still refuses, still `CreditLimit`.

**The controls, because a fixture that compiles proves nothing.** Every one of
these runs here, without an SBF toolchain:

* the successor bootstrap's settlement chain advances through EVERY transition,
  close included, at width 4 as well as 1 — and a price vector one step off the
  forced one takes both its tests red (checked, not assumed);
* two NATIVE tests in the accelerator's lifecycle walk, a file that needed an
  ELF for all thirteen of its tests until now: the book clears through the real
  verifier at widths 1 and 258 with the certificate's prices asserted cell by
  cell, its close's plan says `claims_active = false`, and the floor one step
  past the fill still refuses `CreditLimit` by name;
* the bundle-builder's eighteen span tests, which are pure Rust and were **RED
  on this branch** (§4.1).

## 4. Three defects the compile found

### 4.1 Nine of eighteen bundle-builder tests were red, and nobody had run them

`lifecycle envelope: InvalidBody`. The family made the Batch envelope
Product-width in `local_state_v3.rs` and the fixture still wrote
`batch.to_bytes()` — the 224-byte V1 prefix — into an account the envelope sizes
at `296 + 16N`. `GeneralBatchV2::encode_into` is the whole record and is what
the fixture writes now. All eighteen pass.

### 4.2 The five physical sites, and the join that was missing

`MERGE_NOTES_JOINT-CLEARING.md` §1.1b named them and said their tests "pass
today only because nothing cross-checks them against the envelope". Exactly so.
`GENERAL_BATCH_ROW_BASE_V2` (296) and `GENERAL_BATCH_ROW_STRIDE_V2` (16) name
the pair once, `general_batch_len_v2` IS those two numbers, and the five sites
read them.

The join is the deliverable, not the constants:
`account_rules_v3::tests::every_local_state_rule_declares_the_width_its_own_envelope_encodes`
holds every local-state rule's `data_length + N × data_item_stride` to
`general_local_state_len_v3` at four widths, over eleven (action, coordinate)
rows. **Proved red against the old constants first** — `OpenBatch coordinate 5
at width 1` — because a test that has never failed is a test of nothing.

### 4.3 `tools/gate census` refused the branch, twice over

The V1→V2 rename moved the batch record's magic to `DCGBAT02` and the order
record's to `DCGORD02`. **Both were already live on main under other names**:
`DCGBAT02` is `general/lifecycle.rs`'s `BATCH_LIFECYCLE_MAGIC_V2` and
`DCGORD02` is `runtime_manifest.rs`'s per-order manifest ROW header, which a
deployed ELF writes. A wire discriminant selects one thing.

Adjudicated by decision 0007's rule one wire object over, which is what the
gate's own message asks for: the discriminant that has never reached a chain or
a published reference is not yet a fact and may move; one that has shipped may
not. Both of this family's record magics are new, so both moved:

    batch record   DCGBAT02 → DCGBTCH2
    order record   DCGORD02 → DCGSORD2   ("signed order")

Chosen for **Hamming distance**, not for looks: `DCGORDR2` would have differed
from the manifest row's `DCGORD02` in ONE byte, which is a poor discriminant for
exactly the reader this gate protects.

Lean is the author of both, so both moved there first, and with them
`GeneralTransitionV3`'s two little-endian magic WORDS — the programs that WRITE
the records, which is the trap the family fell into once already (its Wall C).
Three generated Rust files re-emitted through their own emitters: **the diff is
sixteen bytes and nothing else**, which is the control that says a magic moved
and a layout did not.

## 5. The two on-chain writes

Not seated. Named, reasoned where a reader will hit them, and tested.

**Wall A — the strand's burn.** `SettlementClauseV3::PositionCloseGeometry` said
"a close has no position geometry", which describes the shape and not the cause.
Under the joint arm that arm is reachable for exactly one reason:
`runtime_settlement::close` sets `claims_active` on the closes whose clearing
left a residual. It is `PositionCloseStrandUnseated` now; its doc names the
fifth child frame and the 65 → 66 account count that would seat it; and
`a_close_that_strands_is_refused_by_name_and_one_that_does_not_projects`
asserts both halves, because without the positive control the test would pass on
a `position_geometry` that refused every Close.

`ClaimsSbfError::StrandUnseated = 0x5014` — the first free code after
`founder-bond`'s `0x5013` — is the same accusation one program over, for a
strand packet submitted by hand. It has a producer, which is what the queue
declined to allocate a code without. It is deliberately NOT
`ClaimsSbfError::Instruction`: that code means the packet is not a plan this
program dispatches and sends its reader to the encoder; this one means the plan
decoded, its shape is the one the kernel accepts, and the SEAT is missing.

**Wall B — the clearing's publication.** There is no plan to refuse, and that is
the hazard rather than the absolution: `publish_clearing_v1` produces a record no
chain wrote. Its doc now states the frame facts exactly — `Action::Close`'s two
writable local-state coordinates are the settlement cursor and the terminal
record, its only readonly evidence is `SelectedVerifiedCandidate`, and
`ClosedBatch` is selected by four actions that may not write — and names the two
shapes that could seat it, including the one decision 0032 did NOT rule out: a
sixteenth `ClearBatch` action, shaped like `CloseBatch`, which already has the
batch writable and would leave the heaviest frame alone.

The fail-closed fact is asserted rather than described:
`the_batch_account_the_chain_writes_carries_a_vacant_clearing_tail` holds the
Product-width account to a zero price and a zero residual at every outcome. **A
batch that claimed a clearing did not come from this protocol.**

The three `BasketAction::StrandResidual` arms the aborted merge derived are
still NOT in the tree, and that is deliberate: nothing executes a strand, and a
`BasketAction` with no executor is the producer-missing pattern this wave spent
its night diagnosing.

## 6. Green at the landing

```
cargo check --workspace --tests --offline      0 errors (both wasm crates included)
cargo test -p dclutch-trading --lib          640 passed
cargo test -p dclutch-trading --tests         19 targets, all green
cargo test -p dclutch-operator --lib         385 passed
lake build                                   149 modules, exit 0
tools/gate census                            PASS  165 routes, 452 refusal codes
tools/gate emission                          PASS  103 generated, 103 guarded, 0 unguarded
tools/gate cheap                             nine of ten; seam the tenth
check-steps.py                               cohorts 15, 16, 17, 18 all green
npm test (packages/dclutch-sdk)              934 passed, 2 failed (§7)
npm test (apps/dclutch-web)                1052 passed, 12 failed (§7), was 20 FILES
```

The two `sorry`s in `ScoringRuleV1.lean` are still the only ones and are still
not this family's.

## 7. Owed

* **The browser, and it is the largest.** `abi:general-v5:verify` is RED.
  `packages/dclutch-sdk/lib/generated/generalSuccessorV5.ts` mirrors the General
  batch and order records at V1 — `GENERAL_BATCH_MAGIC_V1`,
  `GeneralBatchLayoutV1`'s offsets, `GENERAL_ORDER_HEADER_BYTES_V1` — and those
  records no longer exist. The browser is FAIL-CLOSED on them:
  `generalPlanV5.ts` compares the magic before decoding, so it refuses rather
  than showing the wrong thing. But it cannot read a General batch or order at
  all until `generate-general-successor-v5.mjs`, `generalPlanV5.ts` and
  `apps/dclutch-web/lib/explorer/accountRecords.ts` are swept onto the V2
  decoders **this family already wrote** in `packages/dclutch-sdk/lib/generalClearingV1.ts`.
  That is a browser lane, not a landing.
* **`crates/dclutch-trading/src/general/lifecycle.rs` is fully orphaned.** Every
  public type it declares — `BatchLifecycleV2`, `CandidateLifecycleV2`,
  `CandidateFundingV2`, `LifecycleError`, `BatchPhaseV2` — has ZERO consumers
  tree-wide; only its own tests touch it. It is what holds `DCGBAT02`, so a live
  wire magic is reserved by a record nothing creates. That is somebody's
  deletion, and AGENTS.md already says whose.
* **The frame ratchet, red and now moved again.** The batch account's geometry
  (`224 + 0N` → `296 + 16N`) is in every General lifecycle recipe and account
  rule, and the Claims dispatch gained a `#[repr(u32)]` variant reachable from
  `process_generic_plan`. Two independent SBF captures on the canonical builder,
  as the wave already owed for seven families.
* **The SDK's ABI-coverage baseline GREW by three rows**, deliberately:
  `generalClearingV1.ts` states two magics and six byte coordinates in its own
  words. That is the hand-mirror this family shipped without an emitter and the
  ratchet's exit is seam S12 — two `Emit*Ts.lean` modules and two
  `package.json` scripts, not a new mechanism.
* `runtime_verify.rs:234` casts the tail count `usize → u32` and clippy names
  it; the fix is a public constant's type and this lane was landing. The
  family's other two clippy findings (`div_ceil`) are fixed. Everything else
  clippy says about `dclutch-trading` is `dealer`, `scoring_rule`, `intent_v3`
  and `ordinary_*`, all red on main before this branch existed.
* `packages/dclutch-sdk/lib/stateMachines.test.ts` is red on main and is NOT
  this family's: it expects two machines where `series`' `series-root` and
  `series-ticket` now answer.
* **Twelve browser failures that main had been HIDING**, and they are named
  rather than swept. `packages/dclutch-sdk/lib/marketDetail.ts` imported two
  names twice from one module — a PARSE error, from the wave's own landing-6
  union resolution — so twenty `apps/dclutch-web` test files failed to load and
  ran zero tests between them. Fixed here, because the silence was hiding a
  regression of this lane's own:
  `components/ClearingPriceHistory.test.tsx` still spelled `DCGBAT02`. What the
  fix REVEALED is somebody's: seven stale `abi:*:verify` mirrors, two
  `capabilityMachineDerivation` cases, two `explorerCoverage` cases naming nine
  `DCLS*` dealer/scoring magics the explorer does not render, and the stale
  SBOM. None is this family's; all were invisible before.
* **A coordination cost worth recording.** This lane checked `build/joint-clearing`
  out in the LIVE tree, and while it was there another lane's commits landed on
  the branch instead of on `main` — `c0fc1027a` (the census guard-body fix,
  already on main, so the rebase skipped it) and the ledger row that arrives
  inside this merge. The live tree is where lanes commit; a landing lane that
  moves it off `main` should say so on the board first. This one did not.
