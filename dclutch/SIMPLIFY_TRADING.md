# SIMPLIFY-TRADING

Lane SIMPLIFY-TRADING, 2026-09-04, branch `simplify/trading` from `main` at
`330bbfaba`. Domain: `programs/dclutch-trading-sbf`. Every count below was
measured in this branch's worktree
(`/private/tmp/claude-501/…/scratchpad/simplify-trading`) at the commit
named beside it. Six commits; each is one unit so a reversal is one revert.

| commit | what |
| --- | --- |
| `3ed848701` | the Dealer scenario checkpoint chain, its Custody reservation and selector 9 go (64 files, −39,113) |
| `aa7c63024` | the accelerator program-test lock catches up with an edge it already needed |
| `dee27ebc3` | `hot_v3.rs` split by family into eleven modules with one parent |
| `e8adcc629` | the Dealer island's modules named by concept, not generation (16 renames) |
| `5c27f1af4` | the Series island's pre-interpreter executor and projector go (−1,106) |
| `3fdaa87f9` | hot_v3 comments say what the code does, not the history of deciding it (−184) |

## 1. Deletions

### 1.1 The Dealer scenario checkpoint chain, its Custody reservation, and selector 9 (`3ed848701`)

**What.**

| unit | path | lines |
| --- | --- | --- |
| the seven-step chain (create, page, evaluate, reserve, commit, rollback, cleanup) | `programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs` + seven dispatch arms in `src/lib.rs` | 2,112 + 56 |
| the Custody reservation route (`Reserve`/`Rollback`) | `programs/dclutch-custody-sbf/src/dealer_reservation_v1.rs`, its dispatch line, `authenticate_reservation_frame_v1` | 1,997 + 54 |
| the selector-9 scenario trade in Trading's Dealer island: `v3_trade`, `v3_trade_artifacts`, `v3_trade_profile`, `v3_admitted`, `v3_composer`, `v3_route`, `v3_lifecycle`, `v3_accelerator_accounts`, `v4_scenario_operator`, `v4_scenario_release`, the `DealerProfileV2` projector in `dealer/mod.rs`, `dealer/tests.rs` | `programs/dclutch-trading-sbf/src/dealer/` | 11,880 |
| the trading tests of that closure | `tests/dealer_v3_composer.rs`, `tests/dealer_scenario_profile_vector.rs`, one test in `hot_v3.rs`, two assertions in `tests/dealer_v3_multi_lp.rs` | 800 |
| the accelerator's scenario arm and the program-test that drove the chain | `programs/dclutch-dealer-accelerator-sbf/src/lib.rs` (one arm), `program-test/tests/accepted.rs`, `program-test/src/{dealer_chain,custody_delivery}.rs` | 10,691 |
| the operator's builders | `crates/dclutch-operator/src/dealer_scenario_{checkpoint_v1,hot_v4}.rs` | 4,543 |
| the wire | `crates/dclutch-dealer-codec/src/scenario_{admission,checkpoint,custody_reservation,evaluation_receipt,membership_manifest,reservation_receipt}_v1.rs`, `generated_scenario_{checkpoint_v1,reservation_state_v1,trade_v4}.rs`, three guards in `tests/generator_fresh.rs` | 4,141 |
| the Lean ABIs and emitters | `DealerScenario{CheckpointV1,ReservationStateV1,TradeV4}Abi.lean`, `EmitDealerScenario{CheckpointV1,ReservationStateV1,TradeV4}Rust.lean`, three lakefile entries, three root imports | 1,486 |
| the gauntlet tier and its rows | `tools/gauntlet/dealer-checkpoint/` (bindings, witnesses, programs, fast-lane, runner, README), the `substrates.json` entry, two `Machine` rows in `census/src/phases.rs`, three stale entries in `magic-collisions.json` | 1,290 |
| the browser's seven explorer rows and one console sentence | `apps/dclutch-web/lib/explorer/instructions.ts`, `components/ConsoleDirectory.tsx` | 30 |

**Invariant defended.** Decision 0031 (the mechanism agenda) and the batch
spine's route table (`docs/design/MECHANISM_BATCH_SPINE_2026_09_04.md` §3.1):
the chain, the reservation and the scenario evaluation are replaced by one
`PlaceOrder` of a schedule order per batch under the scoring Dealer. The
architect's map (§1.7) marks the chain as *conditional on ember's batch-spine
ruling*; the lane brief said to take it, and the commit is one unit so that
ruling costs one revert either way. Nothing devnet-witnessed moves: every one
of the eight routes was witnessed only by `dealer-checkpoint-programtest`
(`docs/reference/routes.md`, `route-witnesses.md`).

**Census control.**
- Reverse dependencies of every deleted module were read before cutting
  (the table in the commit message). `v3_lifecycle` and `v3_route` had zero
  non-test consumers already; `v3_composer`, `v3_admitted`, the `v3_trade*`
  trio and the `v4_scenario_*` pair were consumed only by each other, the
  chain, the deleted operator/program-test files, and tests of themselves.
  `scenario.rs` and `dclutch-dealer-scenario-kernel` stay: the LP lifecycle's
  solvency planner reads them.
- `dclutch-route-census inventory --check-unique` at `3ed848701`: **157
  routes, 357 refusal codes**, 375 magics declared, 5 adjudicated collisions,
  no unadjudicated one. The eight deleted routes are the seven
  `trading/dealer_scenario_checkpoint_v1::*` entries and
  `custody/dealer_reservation_v1::process`. The before figure: the map's §2.1
  says 164; `docs/reference/routes.md` at `main` has 179 rows for 164 routes
  by the register's own 15 witness sub-rows. 164 − 8 ≠ 157 by one, so the
  convergence pass (map §3.4 step 5) prints both sides from one binary; this
  branch's census binary refuses `main`'s tree with an absolute `--root`
  ("dclutch-direct-aot-sbf, dclutch-product-runtime-v2-sbf not in TARGETS")
  and accepts a relative one — a gate-tool defect, noted for that maker.
- Refusal codes are unchanged at 357: Custody's
  `Reservation{Record,Identity,Frame,EscrowPrestate}` (`0x600D..0x6010`) stay
  as withdrawn variants because the band's contiguity assertion
  (`CustodySbfError::ALL`) forbids a gap; each says so in one line. The
  Trading chain raised only shared codes.
- `emission_guard.py --write`: 98 generated / 98 guarded (three fewer files;
  the regeneration also recorded the protocol-parameters guard the checked-in
  census had missed).
- `cargo check --offline --all-targets` green for `dclutch-trading-sbf`,
  `dclutch-dealer-accelerator-sbf`, `dclutch-operator`, `dclutch-custody-sbf`,
  `dclutch-dealer-codec`, the accelerator program-test workspace and the
  census tool.

**Owed.** `tools/frameguard/baseline.json` recapture (Trading, Custody and the
Dealer accelerator links all move); `tools/genref/generate.sh --converge` for
`docs/reference/*` and both `lib/generated/**` mirrors; the accelerator's
selector-9 evaluation now has no Trading commit path, which is the batch
spine's stated end state for it.

### 1.2 The Series island's pre-interpreter executor and projector (`5c27f1af4`)

`series/execute_v3.rs` (577) and `series/projector.rs` (525). Invariant:
decision 0006 — the family-neutral executor selects no module by a Series
tag; `cbdecdb3e` (09-02) already took the sibling `terminal.rs` executors.
Census: a whole-tree read of `execute_v3::`, `series::projector`,
`projector::` finds each named once, at its own `mod` line. `shadow_operator.rs`
and the three `build_series_*_hot_v3` builders stay (0029 item 1 BUILD A; map
§1.4(e)). Routes 157, refusal codes 357, unchanged.

### 1.3 Smaller

- `hot_v3::authenticate_product_runtime_for_record_boxed_v3`: zero callers
  here and at `main` (in `dee27ebc3`).
- The four series-kernel names the parent imported for the expiry path, the
  product-runtime record authenticator's import, the projector's nine imports
  in `dealer/mod.rs`, three scenario-only imports in the accelerator, an
  unused helper and three imports in `tests/dealer_v3_multi_lp.rs`.

## 2. Rewrites

### 2.1 `hot_v3.rs` split by family (`dee27ebc3`)

20,228 lines → a 1,038-line parent and eleven modules under `src/hot_v3/`:

| module | lines | owns |
| --- | ---: | --- |
| `hot_v3.rs` | 1,038 | imports, frame bounds, the CU/heap checkpoint macros, `process_hot_execution_v3`, `is_hot_execution_v3`, re-exports |
| `accelerator.rs` | 1,567 | the accelerator's authenticated view of an invocation |
| `execute.rs` | 1,802 | authenticate → prepare → children → verify → commit |
| `series_expiry.rs` | 1,229 | the pre-Market permit expiry (absorbs `series_expiry_v1.rs`) |
| `direct.rs` | 1,073 | the Direct crosscheck, inline and registered |
| `frame.rs` | 1,003 | the frame, the Market, the root and its roles, the sealed strategy |
| `strategy.rs` | 1,441 | transition, admitted and shadow candidates, effect projection |
| `accounts.rs` | 1,459 | runtime expansion, privilege downgrades, geometry, borrowed witnesses |
| `lifecycle.rs` | 2,434 | rent quotes, static ownership, preplan/replan, funding |
| `children.rs` | 2,437 | the child walk, receipts, disjointness, role carriers |
| `commit.rs` | 443 | commit last |
| `tests.rs` | 4,587 | the unit tests, content unchanged |
| `seal.rs` | 1,094 | as before |

Every function moved whole with its attributes. **Every `#[inline(never)]`
stage is where it was** (69 attributes in `hot_v3.rs` plus 0 in `series_expiry_v1.rs` before; 68 across the same code after, the one on the deleted product-runtime authenticator gone), so the
frame-splitting the ratchet measures is preserved in kind — the stages that
matter are the ones the baseline shows near the 4,096-byte bound:
`authenticate_strategy_from_sealed_boxed_v3` (3,968), `authenticate_and_execute_hot_v3`
(3,904), `execute_authenticated_hot_v3` (3,840), `execute_admitted_candidate_v3`
(3,776), `authenticate_accelerator_invocation_v4` (3,456), `execute_child_routes_v3`
(3,392), `decode_claims_composition_boxed_v3` (3,200); each keeps its
attribute in its new file. Visibility: 154 items a sibling names became
`pub(super)` by a tokenize-once census before the compiler ran, and 220 more
(fields, methods, private-interface types) from `cargo check`'s own
diagnostics; the crate's public surface is re-exported from the parent so no
import outside `hot_v3` changed. Control: `cargo check --offline --all-targets`
green for the crate and its two dependents.

### 2.2 The Dealer island named by concept (`e8adcc629`)

Sixteen renames, bodies unchanged: `v3_artifacts→equity_artifacts`,
`v3_equity→equity`, `v3_equity_claims→equity_claims`,
`v3_equity_operator→equity_request`, `v3_hot_artifact→equity_effect`,
`v3_profile→equity_profile`, `v4_equity_accelerator_accounts→equity_accelerator`,
`v4_equity_release→equity_release`, `v3_lp_artifacts→lp_artifacts`,
`v3_operator→lp_request`, `v4_lp_operator→lp_set_request`,
`v4_lp_accelerator_accounts→lp_accelerator`, `v4_lp_release→lp_release`,
`v3_multi_lp→multi_lp`, `v3_obligation→obligation`, `v3_release→release`. The
three external consumers (the accelerator's `lib.rs`, the operator's two
Dealer builders, three trading tests) are re-pathed in place, not shimmed.
Item names keep their `_v3`/`_v4` suffixes: those are wire and artifact
generations the sealed descriptors carry, and renaming them is cohort-17's
Lean-first work.

### 2.3 Comments (`3fdaa87f9`)

184 lines of provenance narration left `hot_v3` and its modules by a stated
rule: a paragraph that dates a measurement, names a commit, a lane or a cohort,
or says what a line "used to" do goes; a paragraph that states a rule
(refuses, must, never, on purpose, only) stays even when it also carries a
date; the first paragraph of every doc comment stays. The census before the
pass: 79 of 539 comment blocks (1,241 lines) carried such narration; 160
paragraph lines were pure history under that rule. The rest is mixed and is
named here rather than hand-edited at this budget.

## 3. Moved wire

- The Dealer global `CapabilityProgramSet` has eight entries (selectors 1–8);
  selector 9's request schema is gone. The Dealer family is out of the release
  set, so no devnet capability re-founds; the next cohort that deploys it
  seals the eight-entry set.
- Trading's ELF: every commit here moves it (deleted routes, relocated
  symbols). Custody's ELF moves with the reservation route. The Dealer
  accelerator's ELF moves with its arm. **Cohort-16 carries all three
  redeploys if it takes this branch; nothing here waits for cohort-17.**
- Refusal codes: none renumbered, none withdrawn. Magics: seven Trading
  selectors and the reservation's family go with their routes.

## 4. Line counts (tree root `simplify-trading`, base `330bbfaba` → head)

| tree | before | after |
| --- | ---: | ---: |
| `programs/dclutch-trading-sbf/src` | 103,329 | 88,341 |
| `src/hot_v3.rs` | 20,276 | 1,038 (+ 20,358 in `src/hot_v3/`) |
| `src/dealer/` | 27,311 in 27 files | 15,820 in 17 files |
| `src/series/` | 20,151 in 28 files | 19,045 in 26 files |
| `programs/dclutch-trading-sbf/tests` | 1,998 | 1,475 |
| whole branch vs base | — | 99 files, +19,534 / −59,880 |

## 5. Deliberately left, and why

- **The registered-Direct V4 branch** (`hot_v3/direct.rs`'s `RegisteredCreation`
  arm, `crates/dclutch-direct-codec/src/registered_*_v4.rs`, 7,999 lines):
  the map §1.4(e) reads it as the open C-04 dispatch wall the contract says to
  dispatch, and the coordinator amended the brief to "amend to the batch
  spine's RFQ, not delete". Left whole.
- **The Series island's generation chain** (`artifacts_v3→v4`,
  `release_v4→v5`, the `*_v5` funding trio): the map assigns the flatten to
  this lane, but the SERIES lane committed to these files within the day
  (`272fb867d`, `2cf96117a`) and the merge needs the byte-identical ELF
  control only the convergence pass has. `shadow_operator.rs` stays as
  0029's owed seam.
- **`release` under `equity_release`/`lp_release`** in the Dealer island: one
  concept in two files (V3 descriptors finalized into V4); a merge, not a
  rename, and owed with the same control.
- **`direct-aot`** and its two contract crates, `tools/gauntlet/aot-cu`,
  `tools/direct-translation-validator`: out of the release set, consumed only
  by their own harnesses; the map assigns the deletion to the
  generation-deletion maker.
- **Pre-existing never-used warnings** under the default feature set
  (`fixed_role_word_v1`, `HEAP_HEADER_BYTES`, `heap_error_v1`, the
  `coordinate` binding in `custody_composition_v3.rs`, the
  `DIRECT_TOKEN_SETUP_FEE_BASIS_POINTS_V1` import): each is used under
  `hot-cu-profile` or in tests. `CurrentDeploymentAuthenticationV2::AttestedAccelerator`
  is matched in four arms and constructed nowhere — a producer-missing
  variant, named here rather than deleted.
- **The 45-clause refusal sites** (`TradingSbfError::Content`, 1,712 raise
  sites after the cut): splitting them is wire work per decision 0007 and the
  map's §4 item 2 proposes a census gate; not started here.
- **Generated mirrors**: `docs/reference/*`, `apps/dclutch-web/lib/generated/**`,
  `packages/dclutch-sdk/lib/generated/**` are regenerated by the convergence
  lane's `genref --converge`; the `abi:*:verify` tests are red until then.
