# BUILD-SERIES — a Series market's whole lifecycle, built by source inspection

Branch `build/series`, worktree `scratchpad/build-series`, base `f7c03e845` (main, 2026-09-06).
Lane: BUILD-SERIES. Nothing here touched the live tree, built an SBF, ran a suite, or reached a chain.

> **ADDENDUM 2026-09-06 (CONVERGE-SERIES). Read `MERGE_NOTES_SERIES.md` first; this document is
> the maker's dated handoff and is not edited.** Four of its claims are reversed there, with the
> evidence: §1.5's four runbook rows and §4's devnet stub describe a stack whose bottom layer has
> no producer, so the rows are withdrawn and the three verbs renamed to the only cluster their
> campaign admits; §2's S3 names the General certificate authoring site, and no Series one exists;
> §10.5 says `SERIES_TICKET_PHASE_LIMIT_V3` is consumed by `ticket_admission_v1.rs`, which wrote
> its own literal `3`; §10.6 attributes the deleted round-trip test to `series_hot_v3.rs`, and it
> was in `family_hot_campaign.rs`. §0's two-author finding and its repair stand, still unmeasured.

## 0. The state the brief described was already stale, and one of its facts was false

The brief said the Expire route stops at `resolve_carrier_by_representative_v3`'s not-found exit because
`CustodyFrameSpecV1` has no callee role. `272fb867d` (SERIES-5, 2026-09-04 18:12) had already appended the
Custody callee as Expire's own coordinate 81 (`SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5` 81 → 82, no route
start or alias moved) and reported the three program-test rows moving to `Content 0x4003` at 530,018 CU in
the first Custody preflight, `custody-prepare` case 6 — `custody.market != parent.market` and
`generation 1 != 9`.

**That measurement was of a working tree.** `programs/dclutch-trading-sbf/src/hot_v3/series_expiry.rs`
still pinned `SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1 = 81` with a `const _` assert `caller + 1 == 81`, and
`git log -S'SERIES_EXPIRE_LOGICAL_ACCOUNTS_V1: usize = 82'` finds no commit, ever. On the committed
sources the pre-Market authenticator refuses `Content` at its width check (`series_expiry.rs`, the
`runtime_accounts.len() != 81` conjunct), ~200,000 CU before the wall the commit message names. Two
authors of one count, disagreeing, and every commit between them green — the exact defect class the
parsimony attractor names. Both the count and the wall are repaired on this branch; **neither is
measured** (this wave builds no SBF). §6 says which cohort carries the measurement.

## 1. What was built, file by file

### 1.1 The Expire frame has one author (Lean-first)

| file | what |
|---|---|
| `formal/dclutch-semantics/DClutchSemantics/SeriesExpireFrameV5Abi.lean` (new, 330 lines) | The 82-coordinate logical frame as one slot list: role, writable, executable, binding (`own` or `alias rep`). Five route windows as `(start, count)` over it; every named coordinate as a position; the alias table, the writable and executable representatives read off the bindings. Theorems (all `native_decide`, all checked against the live tree's warm `lake`): `the_count_is_the_lists_length` (82); `the_windows_tile_the_routes_and_the_callee_is_last` (starts `[6,20,34,44,55]`, counts `[14,14,10,11,26]`, first start = ticket + 1, last window ends at the callee, callee + 1 = count); `named_coordinates_are_positions` (5, 7, 33, 54, 55/26, 55, 56, 57, 69, 70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81); `the_core_window_is_twenty_five_plus_the_caller` (69→0, 70→5, 71→1); `every_custody_window_sees_the_one_future_market` (7 owns, 21/35/54 alias it); `the_aliases_are_backward_and_point_at_representatives` (37 pairs); `the_privileged_representatives_are_exactly_these` (writable `[0,5,14,16,17,33,45,51,55]`, executable `[9,10,19,57,79,81]`); `the_callee_has_one_coordinate`; `aliases_carry_no_privileges`. |
| `formal/dclutch-semantics/EmitSeriesExpireFrameV5Rust.lean` (new) | Emits every constant below. Run against the live tree's lake: exit 0, output byte-identical to the committed emission after `rustfmt --edition 2024`. |
| `formal/dclutch-semantics/DClutchSemantics.lean` | `import DClutchSemantics.SeriesExpireFrameV5Abi` after `SeriesExamples`. |
| `crates/dclutch-trading/src/series/generated_expire_frame_v5.rs` (new, `@generated`) | `SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5: u16 = 82`, `SERIES_EXPIRE_TICKET_COORDINATE_V5 = 5`, `SERIES_EXPIRE_ROUTE_STARTS_V5`, `SERIES_EXPIRE_ROUTE_COUNTS_V5`, `SERIES_EXPIRE_FUTURE_MARKET_COORDINATE_V5 = 7`, `SERIES_EXPIRE_RENT_CREDIT_COORDINATE_V5 = 33`, `SERIES_EXPIRE_PROJECTED_FUTURE_MARKET_COORDINATE_V5 = 54`, `SERIES_EXPIRE_CORE_ROUTE_START_V5 = 55`, `SERIES_EXPIRE_CORE_ROUTE_COUNT_V5 = 26`, `SERIES_EXPIRE_PERMIT_COORDINATE_V5 = 55`, `SERIES_EXPIRE_CORE_RENT_CREDIT_COORDINATE_V5 = 56`, `SERIES_EXPIRE_RENT_PROGRAM_COORDINATE_V5 = 57`, `SERIES_EXPIRE_ROOT_REPLAY_COORDINATE_V5 = 69`, `SERIES_EXPIRE_TICKET_REPLAY_COORDINATE_V5 = 70`, `SERIES_EXPIRE_TEMPLATE_RAW_COORDINATE_V5 = 71`, `..._TEMPLATE_STAGING_ = 72`, `..._OCCURRENCE_RAW_ = 73`, `..._OCCURRENCE_STAGING_ = 74`, `..._TICKET_RAW_ = 75`, `..._TICKET_STAGING_ = 76`, `..._CLOCK_ = 77`, `..._RENT_SYSVAR_ = 78`, `..._SYSTEM_PROGRAM_ = 79`, `SERIES_EXPIRE_PRECOMMIT_CALLER_COORDINATE_V5 = 80`, `SERIES_EXPIRE_CUSTODY_PROGRAM_COORDINATE_V5 = 81`, `SERIES_EXPIRE_WRITABLE_REPRESENTATIVES_V5: [u16; 9]`, `SERIES_EXPIRE_EXECUTABLE_REPRESENTATIVES_V5: [u16; 6]`, `SERIES_EXPIRE_ROUTE_ALIASES_V5: [(u16, u16); 37]`. |
| `crates/dclutch-trading/src/series/mod.rs` | `pub mod generated_expire_frame_v5;` |
| `crates/dclutch-trading/check-generated-series.sh` | Third emitter block: builds the module, re-runs the emitter, pins count 82 / callee 81 / caller 80 / future Market 7 / the starts / the 37-wide alias table, rustfmt-normalises and `cmp`s. |
| `programs/dclutch-trading-sbf/src/series/expire_funding_artifacts_v5.rs` | The local count, ticket/rent-credit/caller/custody coordinates, `ROUTE_STARTS/COUNTS`, `WRITABLE_/EXECUTABLE_REPRESENTATIVES` and the 37-pair `ROUTE_ALIASES` are deleted; `pub use dclutch_trading::series::generated_expire_frame_v5::{…}` re-exports them under the same names, so every importer (`release_v5.rs`, `series_current_acquisition_v5.rs`, `series_terminal_campaign.rs`, the fixture support) is unchanged. The two `const _` asserts stay as the consumer's cross-check of the emission. All eight tests in the module unchanged and green. |
| `programs/dclutch-trading-sbf/src/hot_v3/series_expiry.rs` | The 81 and every `SERIES_EXPIRE_*_ACCOUNT_V1` local now `= frame_v5::… as usize`; the `const _` block restated as the emission's theorems (Core window ends at the callee, callee last, caller = callee − 1). |

### 1.2 The wall: Custody children of a pre-Market Series action bind to the FUTURE Market

`custody_composition_v3::prepare` binds every Custody request to one parent `(market, generation)`
(`custody_composition_v3.rs:262-286`, case 6) and both child walks built that parent from
`envelope.market()/generation()` — the live Series CONTROLLER. The Series escrow's replay, vault and
transfer authority are PDAs of the FUTURE occurrence Market (`custody_v3.rs` projects
`escrow.market()`/`escrow.generation()` = occurrence + 1 into every request; the fixture derives every
escrow PDA from `series.future_market`), `custody-sbf` requires that Market at its frame coordinate 1,
and every Expire Custody window presents it there (Lean: `every_custody_window_sees_the_one_future_market`).

**RULING (provisional): the child composition gains a projected-market authority for the selected
pre-Market Series action; nothing else moves** (not option B, "move the escrow into the parent's
namespace", which would re-derive every escrow PDA, `series_consume.rs:1106`'s projected-state binding,
custody-sbf's vacant-Market path and the Consume `LockHoardAndCloseSource` into the future Hoard).

| file | what |
|---|---|
| `hot_v3/series_expiry.rs` | `SeriesExpiryPremarketFactsV1 { rent_credit, future_market, future_generation }` — derived from finalized records and re-derived addresses, never the fixed Hot Market; `ChildMarketAuthorityV3 { market, generation }`; `custody_child_market_v3(envelope, Option<facts>)` = the envelope for every family, the facts for this one. `try_authenticate_series_expiry_premarket_v1` returns `Option<SeriesExpiryPremarketFactsV1>`; `authenticate_series_expiry_records_and_projection_v1` additionally requires the permit conjunct's `future_market` to equal the vacancy conjunct's re-derived one (two derivations, one binding). `series_expiry_local_replay_overlap_v1` takes the facts instead of `(bool, [u8;32])`. Tests `child_market_tests::{without_premarket_facts_the_child_market_is_the_envelopes, with_premarket_facts_the_child_market_is_the_future_occurrence_market}`. |
| `hot_v3.rs` | `AuthenticatedHotPreludeV3.series_expiry_premarket: Option<SeriesExpiryPremarketFactsV1>` replaces the bool + rent-credit pair. |
| `hot_v3/execute.rs` | The pair replaced at its five sites; `PreparedHotCommitV3.custody_child_market: ChildMarketAuthorityV3` computed once at construction; `execute_child_routes_v3` receives it. |
| `hot_v3/children.rs` | `preflight_child_routes_v3` takes the facts and computes `custody_child_market` once; `execute_child_routes_v3` takes `custody_child_market`; both `CustodyCompositionParentV3` sites use `custody_child_market.{market, generation}`. The Core route's `CoreCompositionParentV3` keeps the envelope: `series_permit_expiry_precommit_v1::authenticate_caller` seeds the caller from the CONTROLLER Market (`header.market()`), and `controller_and_future_markets_are_distinct` re-proves the distinctness from Core's own frame. Outer-only builds get `let _ = …` guards. |

### 1.3 The `Prepared TicketStateV3` producer

| file | what |
|---|---|
| `programs/dclutch-trading-sbf/src/series/prepare_funding_artifacts_v5.rs` | Three new common scalars (8 magic word, 9 schema, 10 profile; `SERIES_PREPARE_COMMON_SCALAR_COUNT_V5` 8 → 11), three `load_const` Transition instructions (4 → 7), four Effect writes on coordinate 5 after the lifecycle `Create` (`write_u64` magic at `SERIES_TICKET_STATE_MAGIC_OFFSET_V3`, `write_u16` schema, `write_u16` profile, `write_identity` the RequestProfile-projected Ticket record id at `SERIES_TICKET_STATE_RECORD_ID_OFFSET_V3`; ops 2 → 6). Phase (`Prepared` = 0) and revision (0) are the zeros a fresh account holds (`SeriesTicketStateV3Abi.the_prepared_tag_is_zero_so_the_magic_is_the_partition`). `commit_prepared_hot_result_v3` applies `apply_lifecycle_creates_v3` before any data effect, so the writes land in a 64-byte account. Test `the_effect_writes_the_first_valid_ticket_state`: the four writes over sixty-four zeros decode as exactly `TicketStateV3::prepared(ticket)`; negative control: the unwritten account refuses. |
| `crates/dclutch-trading/src/series/replay.rs` | `SCHEMA_V3`/`PROFILE_V3` are now `pub` and equal the family emission's `SERIES_TEMPLATE_SCHEMA_V3`/`_PROFILE_V3` (one author); the 30-line NAMED DEBT doc is replaced by the sentence naming the producer. |

### 1.4 One operator path (parsimony)

| file | what |
|---|---|
| `crates/dclutch-operator/src/series_hot_v3.rs` (2,193 → 1,377 lines) | `build_series_{prepare,consume,expire}_hot_v3`, `SeriesOccurrenceHotStateV3`, `SeriesOccurrenceHotReportV3`, `SeriesFinalizedRecordV3`, the artifacts-V3 join, `validate_strategy_selection`, `series_occurrence_hot_bump_hints_v3` and their four tests DELETED. `inspect_current_series_hot_v5` is the one path (callers: `series_current_acquisition_v5`, the fixture support, `series_terminal_campaign`); `CheckedSeriesShadowAcceleratorV3`, `SeriesHotOperatorErrorV3` and every V5 item unchanged. The brief's item 3 asked for callers for the three builders; the tree's own attractor (no parallel legacy/current paths) and the fact that every V3 proof is a V5 proof against the current release made deletion the honest build. |
| `tools/local-validator/bootstrap/successor/src/family_hot_campaign.rs` | The Series arm — a command whose whole body was a refusal naming missing pieces — deleted with its schemas, its test and its usage line. |

### 1.5 The verbs and the runbook

| file | what |
|---|---|
| `tools/local-validator/bootstrap/successor/src/series_terminal_campaign.rs` | `SeriesCampaignPolicyV1 { cluster, expected_act }`, `OWNED_LOOPBACK`, `run_with_policy`; `run` = loopback policy. The expected-act gate sits right after the planner's act is re-authenticated against the re-emitted release (`policy.require_act(planned.action())`), threaded through `acquire_current_series_selected_v1`. The devnet arm refuses by name (see §4). |
| `tools/local-validator/bootstrap/successor/src/series_devnet_v1.rs` (new) | `devnet-series-open-v1` (Prepare), `devnet-series-consume-v1`, `devnet-series-expire-v1`: `--cluster devnet\|owned-loopback` is the verb's, everything else passes through to the campaign; the verb never selects an act. Two tests. |
| `tools/local-validator/bootstrap/successor/src/main.rs` | Three dispatch arms, the usage line, the `mod`; the family-hot Series arm removed. |
| `tools/cohort/steps.tsv` | Rows `found-series`, `series-occurrence`, `series-consume`, `series-expire` (since 17, `once`), each with its verifier stated as decodable facts (SDK ticket decoder, root counters, permit drained, RentCredit rose, journey L1/L7/L8). `cost_sol` is `?` — nothing has priced a Series transaction. |

### 1.6 The register and the fixture

| file | what |
|---|---|
| `tools/gauntlet/blocked.json` | The five Series rows rewritten to the state above: `precommit_v1` (4) discharged, (5) re-anchored to the two-author count and the projected-market authority with the measurement owed; `accelerator/series::*` carries the certificate ruling; the three status-report rows name their verb. |
| `programs/dclutch-trading-sbf/program-test/tests/series_pre_market_expiry_program_test.rs` | Header rewritten: what this branch repairs, and that the number in the paragraph is the next real-ELF run's. No fixture code moved (the fixture already binds the Custody program as coordinate 81 and builds every escrow request under the future Market). |

### 1.7 The Shadow certificate binding (ruling + one seam in code)

| file | what |
|---|---|
| `crates/dclutch-operator/src/series_current_acquisition_v5.rs` | `assemble_strategy_accounts` admits both certificate bindings: `Release` → `validate_artifact(hash(artifact record))` as before; `Semantic` → `validate_semantic_release(artifact.semantic_release_id())`. |

### 1.8 The Series root tail has one author (Lean-first, for the SDK's decoder)

| file | what |
|---|---|
| `formal/dclutch-semantics/DClutchSemantics/SeriesStateV3Abi.lean` (new) | The 64-byte `SeriesStateV3` root tail as a schema over `AbiSchema`: magic 8 @0, schema u16 @8, profile u16 @10, phase u8 @12 (`active` 0, `terminal` 1), currentTicketPrepared u8 @13, reserved 2 @14, nextOccurrence u32 @16, outstandingTicketAccounts u32 @20, revision u64 @24, closeRentRemaining u64 @32, reserved 24 @40. Eleven theorems (`native_decide`, checked against the live lake, exit 0): well-formed, disjoint, covers 64, coordinates canonical, the two reserved spans are exactly decode's two `all_zero` spans, the head span pads the flag to the cursor, the phase byte follows the two header words (the coordinate the SDK generator's regex matched against the wrong record), tags distinct bit indices, names distinct, every field named. |
| `formal/dclutch-semantics/EmitSeriesStateV3Rust.lean` (new) | Emits `SERIES_STATE_BYTES_V3`, `SERIES_PHASE_{ACTIVE,TERMINAL}_V3`, `SERIES_PHASE_LIMIT_V3`, `SERIES_STATE_BODY_OFFSET_V3`, the two reserved widths and eleven `SERIES_STATE_*_OFFSET_V3`. |
| `crates/dclutch-trading/src/series/generated_series_state_v3.rs` (new, `@generated`, 18 constants) | The emission after rustfmt. |
| `crates/dclutch-trading/src/series/replay.rs` | `SERIES_STATE_BYTES_V3` is a re-export; `SeriesPhaseV3`'s discriminants and `decode` arms are the emitted tags; `SeriesStateV3::decode`/`encode` read every offset and reserved width from the emission (ten bare literals, twice each, gone). Behaviour unchanged; the module's tests pass. |
| `crates/dclutch-trading/src/series/mod.rs`, `formal/dclutch-semantics/DClutchSemantics.lean`, `check-generated-series.sh` | module registration, library import, fourth guard block (pins 64 / phase 12 / spans 14 and 40 / terminal 1). |

## 2. THE SEAMS (exact file:line, one-line change each)

Everything in §1 is committed and compiles (`cargo check -p dclutch-trading -p dclutch-trading-sbf -p dclutch-local-successor-bootstrap --tests --offline` green; `dclutch-operator` see §5). What the convergence lane must do beyond merging:

| # | where | the one-line change |
|---|---|---|
| S1 | `tools/gates/emission-coverage.md` | regenerate with `tools/gate emission --write` (do not hand-edit): the new guarded emitter `EmitSeriesExpireFrameV5Rust.lean` → `crates/dclutch-trading/src/series/generated_expire_frame_v5.rs` via `check-generated-series.sh`. |
| S2 | `tools/gates/frames-baseline.json` | `tools/gate frames --at <merge> --capture`: the Trading SBF link moves (series_expiry width check, children walk signatures, the Prepare Effect width). Both new Series artifacts (Prepare Effect/Transition, Expire profile digest unchanged since `272fb867d`) move the **Trading digest** — cohort-17 carries it. |
| S3 | `programs/dclutch-accelerator-sbf/generator/src/lib.rs:240` and `:289` (`source.release_sources.certificate`; field declared `:92`, `:135`), `generator/src/manifest.rs:248, :276, :286, :357, :408, :498`, and the consumer `programs/dclutch-accelerator-sbf/src/series/release.rs:66` (`identity(generated::SERIES_SHADOW_CERTIFICATE_ID_V1)`, whose placeholder is `build.rs:35: [0u8; 32]`) — *coordinates re-derived by CLOSEOUT-B; the maker's `:229` had drifted* | the Series generator authors the certificate with `ExecutionStrategyCertificateV2::new_semantic(…, accelerator semantic_release_id, …)`; the include's `SERIES_SHADOW_CERTIFICATE_ID_V1` then names a certificate that can exist before the ELF. `series-program-test/README.md`'s "until the common authenticated Shadow callback is committed" paragraph is stale: the callback IS committed (`crates/dclutch-trading/src/shadow_accelerator_auth/mod.rs`, used by `accelerator-sbf/src/series/mod.rs::process`); what the harness lacks is a selected include built with a semantic-bound certificate. |
| S4 | `docs/reference/routes.md`, `refusals.md`, `docs/reference/abi/` | `tools/gate reference --converge` (the register rows changed; no code moved). |
| S5 | `tools/cohort/README`/emitted script | the four new rows carry `?` in `cost_sol`; price them from the first loopback run. |
| S6 | `docs/MASTER_COMPLETION_CONTRACT.md` C-07 | blocker (b) discharged (producer written, tested natively); (c) restated: the callback is committed, the caller is `devnet-series-consume-v1` through the planner, the remaining fact is the certificate binding (S3); (d) re-anchored as §0 says. |
| S7 | `packages/dclutch-sdk/scripts/generate-state-machines-v1.mjs:333, :334` | **NOT "unchanged in behaviour" — this THROWS today.** (Corrected by CLOSEOUT-B; the original text here said otherwise.) `replay.rs:81, :83` now read `pub const SCHEMA_V3: u16 = SERIES_TEMPLATE_SCHEMA_V3;` — an identifier, not a literal — and the generator's matcher at `:79-83` is `const NAME: [^=]+ = ([0-9_]+);`. Executing that exact regex against the file gives **no match**, so `scalar('seriesReplay','SCHEMA_V3')` raises `missing Rust scalar seriesReplay.SCHEMA_V3`. `packages/dclutch-sdk/lib/abiVerification.test.ts:42, :79` shells out to every `abi:*:verify`, so **`npm test` in `packages/dclutch-sdk` is RED on this branch.** Repoint both at `scalar('seriesGenerated','SERIES_TEMPLATE_SCHEMA_V3')` / `'SERIES_TEMPLATE_PROFILE_V3'` (both are literals in `crates/dclutch-trading/src/series/generated.rs:2, :3`), then regenerate `packages/dclutch-sdk/lib/generated/stateMachinesV1.ts`. |
| S7b | `packages/dclutch-sdk/scripts/generate-state-machines-v1.mjs:226-240` | the comment that names the phase-byte regex hazard is now moot for `series-ticket` AND a `series-root` row can be read off `generated_series_state_v3.rs` (`SERIES_STATE_PHASE_OFFSET_V3`, the two tags) instead of matching expressions — see S11. |
| S8 | `tools/gauntlet/census` | no route added or renamed; `tools/gate census` should be unchanged — verify. |
| S9 | `docs/design/SERIES_PERMIT_EXPIRY_HOT_REACHABILITY_2026_08_31.md` head | Design 1 (authenticated pre-Market Hot mode) is what exists; add to its head that the Custody children of that mode bind to the future Market (§1.2) — the note's step 7 "run the four Custody cleanup routes" silently assumed it. |

## 3. Rulings (provisional; ember rules by reversal)

1. **The Custody child parent of a selected pre-Market Series action is the authenticated future occurrence Market at occurrence + 1.** Not the controller envelope (which the composition assumed for every family) and not a relocation of the escrow into the controller's namespace. The Core route keeps the controller: its caller authority is seeded from `header.market()` by Core itself.
2. **The Series Shadow certificate binds the accelerator's `semantic_release_id`** (`CertificateArtifactBindingV2::Semantic`), never the ELF digest, so it can be authored before the ELF that embeds it. The ELF digest has its own two authors already: the Registry `ArtifactReleaseV1` and the Loader ProgramData check at callback time. The acquisition admits both bindings (a Release-bound certificate is still the Dealer's and AdmittedAot's).
3. **The occurrence count's founding input is the Template's `occurrence_count`, and nothing else.** The brief asked. `series_proof_count_v3(occurrence_count)` is a per-Template constant compared by EQUALITY in `admit_occurrence_bytes`, the Expire and Prepare artifacts declare or omit the borrowed range from it, and `SeriesStateV3::validate` bounds `next_occurrence` by it. A founding input that could disagree with the Template would be a second author; the founding reads the Template record it installs. The runbook's `found-series` row installs the Template and occurrence 0 in one step for that reason.
4. **One Series operator path (V5).** The three V3 builders are deleted rather than given callers.
5. **The Ticket's rent refund source is `Credit` under decision 0021** — the beneficiary is the Template's immutable refund owner (identity register 8, projected from the Template record), which the Expire transition already requires to equal the RentCredit's `refund_wallet` (`identity_eq(TEMPLATE_REFUND_OWNER, RENT_CREDIT_BENEFICIARY)`); the payer (coordinate 14) never owns the Ticket's rent. The permit is the same: prefunded by the founding, refunded by Expire into the RentCredit whose `refund_wallet` is the Ticket's refund owner. Neither is `Payer`. This is what the Prepare `FundingActionV5::create(…, refund_owner_identity = 8, …)` already encodes; the ruling names it.

## 4. Stubs (each with its reason)

- `series_terminal_campaign::run_with_policy`, devnet arm: **refuses by name** after the input and its digest are admitted and before any key is opened. Reason: every durable journal binds a `SeriesLedgerIdentityV1` (canonical ledger directory + genesis hash) and rereads it before every send; devnet has a genesis hash and no ledger directory, so the identity needs its cluster form (genesis hash + acknowledged origin, the pair `ExpectedClusterV1::Devnet.authenticate` already proves for the Direct verbs). Building it means touching the 8,236-line campaign's journal reader in four places; it is one argument away, not one design away. The loopback arm of the same verbs is real.
- No `todo!()` anywhere else. The Shadow generator's semantic-bound certificate (S3) is a seam, not a stub: the typing exists (`crates/dclutch-market/src/execution_strategy/v2.rs:547 new_semantic`, `:740 validate_semantic_release` — both verified present by CLOSEOUT-B), the producer site is one call.

## 5. Tests written and what each proves

| test | proves |
|---|---|
| Lean: nine theorems in `SeriesExpireFrameV5Abi` (§1.1) | the count, windows, named coordinates, callee position, alias direction and privilege sets are one list's projections — checked with `lake env lean`, exit 0. |
| `hot_v3::series_expiry::child_market_tests::without_premarket_facts_the_child_market_is_the_envelopes` | every live-Market family's Custody parent is unchanged. |
| `…::with_premarket_facts_the_child_market_is_the_future_occurrence_market` | the selected pre-Market action's Custody parent is the future Market at occurrence + 1 and differs from the envelope on both fields — the exact bitmap `0xc` case 6 reported. |
| `series::prepare_funding_artifacts_v5::tests::the_effect_writes_the_first_valid_ticket_state` | the four writes at the declared offsets/widths with the Transition's constants are byte-for-byte `TicketStateV3::prepared(ticket).encode()`; the unwritten account refuses. |
| `series_devnet_v1::tests::{the_three_verbs_are_the_three_occurrence_acts, the_cluster_flag_is_split_off_and_the_rest_passes_through}` | the verb↔act map and the argument split. |
| six pre-existing Series tests re-run (`the_custody_callee_is_one_readonly_executable_past_every_route_range`, `every_child_coordinate_has_one_canonical_identity_and_physical_privilege`, `hot_prefix_and_exact_route_windows_are_pinned`, `prepare_has_one_create…`, `prepare_pins_seed_authority…`, `exact_roundtrip_and_reserved_bytes_refuse`) | the emission re-export and the producer moved nothing they pin. |

Run: `cargo test -p dclutch-trading-sbf -p dclutch-trading --lib --offline -- <names>`: 9 passed, 0 failed.

## 6. What moves, and which cohort carries it

- **Trading digest** (cohort-17): the Prepare Effect and Transition (three scalars, four writes), the pre-Market authenticator's width (81 → 82, now the emission), the child walks' Custody parent for the pre-Market action. The Expire profile bytes are unchanged since `272fb867d`.
- **No Core, Custody, Claims or accelerator ELF moves** on this branch.
- **Owed measurement** (the first lane with an SBF build): the three rows of `series_pre_market_expiry_program_test.rs` on ELFs built from this branch. The claim this branch makes is that they pass `custody-prepare` and reach route 4's Core CPI; the number that replaces the fixture header's paragraph is theirs.

## 7. The census L1–L8 over a Series lifecycle (design; the journey seam)

`tools/gauntlet/journey/src/ledger.rs` states L1–L8 over a Market's account classes and has no Series
stage (`journey.rs:426-566` carries the missing stages as `GapV1`). A Series lifecycle adds four
account classes and two boundaries the laws must be stated over. Per boundary, what each law reads:

| boundary | classes that exist after it | L1 (every atom in a named account) | L7 (lamports close) | L8 (no cross-class movement; declared per-class delta) |
|---|---|---|---|---|
| `found-series` | Series root (Trading-owned, 232 + 64), Template/occurrence/Ticket records (Registry), RentCredit | the escrow does not exist; L1 is over the controller Market only | root rent = `exact_root_rent`; records at their finalized rent | no atom class moves |
| `series-occurrence` (Prepare) | + Ticket replay (64 bytes, Trading-owned, rent from the PAYER, refund owner = Template refund owner — ruling 3.5), Custody replay, escrow vault under the FUTURE Market's Custody authority | `SeriesEscrow` gains exactly `hoard_principal` from the founder's external account | Ticket + replay + vault rent debited from the payer; the root unchanged | declared: `External → SeriesEscrow = hoard_principal`; `HoardPrincipal` delta 0 (no Hoard exists yet) |
| `series-consume` | + occurrence Market (Open + Consumed), Hoard, Claims aggregate/position/admission, failure escrow; − permit, escrow vault, Custody replay | `SeriesEscrow → HoardPrincipal = hoard_principal` in ONE projected `LockHoardAndCloseSource`; L1 holds over the new Market's classes | permit lamports → RentCredit at close; vault + replay rent → RentCredit | declared: `SeriesEscrow −hoard_principal`, `HoardPrincipal +hoard_principal`, every other class 0 |
| `series-expire` | − permit, escrow vault, Custody replay; future Market still vacant | `SeriesEscrow → External(refund owner) = hoard_principal`; the future Market's classes never come to exist | permit balance → RentCredit (or RentCredit + crank per `series_permit_expiry::refund`, cap = one empty account's rent); vault + replay rent → RentCredit | declared: `SeriesEscrow −hoard_principal`, `External +hoard_principal`, `HoardPrincipal` 0 |

L2–L6 are Market-phase laws and are `inapplicable` by name before `series-consume` (no Market exists; the
journey already retires L4 by phase) and hold as for any Open Market after it. The ticket rent and the
permit's refund source are both `Credit` (ruling 3.5), so L7's refund destination for every Series
account is the RentCredit whose `refund_wallet` is the Ticket's refund owner — one wallet, which is what
makes the L7 row a single equality per boundary rather than a per-payer ledger.

Seam S10 — `tools/gauntlet/journey/src/journey.rs:426-566` (`GapV1`) and `ledger.rs`: a `SeriesStageV1`
that reads the classes above from the successor's completion JSON (`series_terminal_campaign`'s
`SERIES_TERMINAL_CONSERVATION_SCHEMA_V1` already computes the donation-inclusive Retire/Close
conservation; the four rows above are its Prepare/Consume/Expire siblings). Not built here: the journey
does not depend on the Series operator and the harness cannot stand up its own substrate
(`run-journey.sh:459-472`, ledger 2026-09-06).

## 8. The SDK's Series decoders and the explorer's Series cards (what exists, what is a seam)

- `decodeSeriesTicketStateV3` exists (`packages/dclutch-sdk/lib/stateMachines.ts:209`), driven by the
  `series-ticket` row of `generated/stateMachinesV1.ts`, and the explorer renders it through
  `STATE_MACHINE_SUMMARIES['series-ticket']` (`apps/dclutch-web/lib/explorer/accountRecords.ts:2277`).
  Every state of the ticket machine (`Prepared`, `Consumed`, `Expired`) is decodable today; the runbook
  rows' verifiers (§1.5) read them.
- The Series ROOT tail (`DCLTSSV3`, `SeriesStateV3`: phase Active/Terminal, `current_ticket_prepared`,
  `next_occurrence`, `outstanding_ticket_accounts`, `revision`, `close_rent_remaining`) has NO decoder
  and NO Lean owner: `replay.rs` spells its offsets as bare literals in `encode` and `decode`. A
  subagent on this lane is giving it one (`SeriesStateV3Abi.lean`, `EmitSeriesStateV3Rust.lean`,
  `generated_series_state_v3.rs`, `replay.rs` reading the emission) — see §1.8 when it lands.
- Seam S11 — `packages/dclutch-sdk/scripts/generate-state-machines-v1.mjs:325`: add a `series-root`
  row (magic `SERIES_STATE_MAGIC_V3` from `generated.rs`, bytes/tag offset/reserved spans from
  `generated_series_state_v3.rs`, discriminant `SeriesPhaseV3`, record `SeriesStateV3`, counters
  `next_occurrence` u32 @16, `outstanding_ticket_accounts` u32 @20, `revision` u64 @24,
  `close_rent_remaining` u64 @32). The generator's `Machine` table in
  `tools/gauntlet/census/src/phases.rs:98` wants an admission type: add `SeriesRootAdmissionV1` in
  `crates/dclutch-trading/src/series/ticket_admission_v1.rs`'s shape (states `&[SeriesPhaseV3]`, the
  Active set for Prepare/Consume/Expire, the Terminal set for Close). Then
  `STATE_MACHINE_SUMMARIES` in `accountRecords.ts` gains its one sentence (the map is total over the
  union, so the type error is the reminder).
- Seam S12 — the Template/occurrence/Ticket RECORDS (`DCLTSTV3`, `DCLTSOV3`, `DCLTSKV3`; widths 400,
  288, 256 and every offset already emitted by `EmitSeriesOccurrenceV3Rust.lean`) render as "layout not
  emitted" in the explorer today because `accountRecords.ts` imports nothing from
  `crates/dclutch-trading/src/series/generated.rs`. The SDK's `generate-protocol-constants.mjs` is
  table-driven; three rows there give the explorer the three record specs without a hand-written
  offset.

## 9. Ledger-ready paragraph (for the convergence lane's row in `docs/ledger/2026-09-06.md`)

BUILD-SERIES closed (branch `build/series`, `acbaf98cb`, `0e0bcbb27`, +…): **the Expire frame's count had two
authors and they disagreed** — `272fb867d` moved the profile to 82 and measured the next wall on a working
tree while the committed authenticator still pinned 81 (`git log -S` finds no 82, ever) — so
`SeriesExpireFrameV5Abi.lean` is now the one author (nine theorems, emitted, guarded, re-exported by both);
**the wall itself is repaired**: Custody children of a selected pre-Market Series action bind to the
authenticated FUTURE occurrence Market at occurrence + 1 (`custody_child_market_v3`; ruling, provisional)
while the Core route keeps the controller; **the `TicketStateV3` producer is written** (four Effect writes
after the lifecycle Create, proved byte-exact natively); the three V3 operator builders and the refusing
Series arm of the family-hot campaign are deleted (one path: V5); `devnet-series-{open,consume,expire}-v1`
drive the lifecycle planner with an expected-act gate (the devnet ledger identity is the one named stub);
four runbook rows; the Shadow certificate binds the semantic release (ruling). **Owed: the three
program-test rows on ELFs built from the branch — no lane has built one.**

## 10. Does it parse? And what else is red? (CLOSEOUT-B, 2026-09-06)

### 10.1 Rust: green

`cargo check -p dclutch-trading --offline` on `bef26e024`: **GREEN**, `Finished dev profile in 9.42s`,
zero warnings from the branch's own files. That covers §1.8's last-written work — the untracked
`generated_series_state_v3.rs` emission and the `replay.rs` rewrite that reads it — which was the code
in flight when the wave stopped. The rest of §1 was checked by the maker at its own commits
(`-p dclutch-trading-sbf -p dclutch-local-successor-bootstrap --tests`). No referenced-but-undefined
identifier exists anywhere on the branch; every symbol resolves in-tree.

**Green Rust is the least interesting fact about this branch.** Six things are red, and none of them is
a compiler error — which is exactly why the maker did not see them.

### 10.2 SIX HARD BREAKS — a gate or script goes red on this tree, unrebased

| # | where | what happens |
|---|---|---|
| **H1** | `crates/dclutch-trading/check-generated-series.sh:84` | **The branch's own guard refuses the branch's own emission.** `verify()` (`:24-32`) tests `wc -l` on the RAW emitter stdout, before rustfmt. `EmitSeriesExpireFrameV5Rust.lean` makes 28 `emitConst` calls (2 lines each) + 1 header = **57 raw lines**; the floor passed at `:84` is **60**. `test 57 -ge 60` fails under `set -eu` → the script aborts. The committed file is 95 lines only because rustfmt expands the 37-pair alias array from one raw line to 39. **Fix: lower the floor to ≤ 57.** The other three blocks are correct (`:42` 70≤89, `:65` 30≤33, `:100` 20≤37). |
| **H2** | `packages/dclutch-sdk/scripts/generate-state-machines-v1.mjs:333, :334` | S7, above. `npm test` in `packages/dclutch-sdk` throws. |
| **H3** | `tools/cohort/steps.tsv:92, :93, :94, :95` | All four new rows name driver kind **`bootstrap-private`, which does not exist**. `tools/cohort/check-steps.py:71-72` lists `bootstrap, bootstrap-public, bootstrap-offline, solana, script, simulator, sh`; `generate-stage-scripts.py:572` raises `driver kind 'bootstrap-private' has no emission`. The four occurrences on this branch are the only ones in the tree. **Fix: use `bootstrap`.** |
| **H4** | `tools/cohort/steps.tsv:93` | The `blocks` field reads `series-consume series-expire` (space-separated); `check-steps.py:215` splits on **comma**, so it sees one token that is not a key. **Fix: `series-consume,series-expire`.** |
| **H5** | `tools/cohort/README.md` | `check-steps.py:296-300` requires a `### <key>` heading per row. **None of `### found-series`, `### series-occurrence`, `### series-consume`, `### series-expire` exists** — the branch never touched the file (the last heading is `### escrow-seated` at `:1053`). §1.5 claims the runbook rows landed; the README half did not. |
| **H6** | `tools/gate fmt` | Five branch-introduced rustfmt-dirty files, each verified clean at `main` and dirty at `HEAD`, none in `tools/gates/fmt-baseline.txt`: `crates/dclutch-trading/src/series/mod.rs:28-43` (rustfmt reorders the `mod` declarations — `generated_expire_frame_v5` must precede `generated_series_state_v3`), `crates/dclutch-trading/src/series/replay.rs:19`, `programs/dclutch-trading-sbf/src/series/prepare_funding_artifacts_v5.rs:485, :550, :1052, :1061` (`:553` is a 112-char closure), `tools/local-validator/bootstrap/successor/src/series_devnet_v1.rs:124`, `crates/dclutch-operator/src/series_hot_v3.rs:792` (the 860-line deletion left the `mod tests` import block out of order). |

One more that is not a gate but is worse: `tools/cohort/steps.tsv:92` names
`local-private-validator-series-lifecycle-prefix-**v2**`, and **the tree has only `-v1`**
(`series_lifecycle_campaign.rs:26-27`). The same row passes `--session $DCLUTCH_SESSION`, an environment
variable that appears nowhere else under `tools/`.

### 10.3 And the runbook rows cannot run even after H3–H6 are fixed

§4's named stub — `series_terminal_campaign.rs:1580-1594`, `run_with_policy`'s devnet arm returning
`refusal("devnet Series verbs need SeriesLedgerIdentityV1's cluster form …")` — fires under the **default**
cluster: `series_devnet_v1.rs:95` defaults to `ExpectedClusterV1::Devnet`, and all four new `steps.tsv`
rows pass `--cluster devnet`. So the three verbs the branch adds refuse, by their own stub, on the path
the runbook drives them down. The stub is honest and its reason is sound; what §4 did not say is that it
is **load-bearing for everything §1.5 claims**.

### 10.4 Stale byte-gated artefacts (regenerate; never hand-edit)

`tools/gates/emission-coverage.md:4` (header, `91 generated files from 85 emitters` → 93 / 87) and `:53`
(the guard row still lists only `EmitSeriesOccurrenceV3Rust.lean, EmitSeriesTicketStateV3Rust.lean`) —
`tools/gate emission --write`, then `--fixpoint` (no new `fixpoint-debt.tsv` row is expected: both new
emissions are rustfmt-normalised by their guard). `docs/reference/routes.md:125, :126, :158, :200, :201,
:202` mirror the five `blocked.json` reasons the branch rewrote, plus a unicode fix at `:251` —
`tools/gate reference --converge`. `tools/gates/frames-baseline.json` — the Trading link moves at
`hot_v3::children::execute_child_routes_v3` (`:8971`, **3392 of 4096 — 704 bytes of headroom**, and the
branch adds a by-value `ChildMarketAuthorityV3` parameter), `preflight_child_routes_v3` (`:8995`), the
thirteen `hot_v3::series_expiry::*` symbols (`:8286-8364`, signatures moved from `[u8;32]`/`bool` to
`Option<SeriesExpiryPremarketFactsV1>`), and `emit_series_prepare_funding_artifacts_v5` (`:9674`, +3
transition ops / +4 effect ops) — `tools/gate frames --at <merge> --capture`, **and confirm
`execute_child_routes_v3` stays under 4096**. `tools/seam-audit/baseline.json` is keyed by path and
symbol, not line; four touched keys, and the 860-line deletion in `series_hot_v3.rs` may DROP findings —
a repaired-but-still-listed row is a ratchet failure in that tool's convention.

### 10.5 Emitted constants with no consumer

Five of the branch's new emitted constants have zero Rust callers. Two of them matter:

- `generated_expire_frame_v5.rs:11` `SERIES_EXPIRE_FUTURE_MARKET_COORDINATE_V5 = 7` — **the single most
  load-bearing coordinate of §1.2's ruling** (every Custody window's `CoreMarket`, the vacant future
  Market). It is pinned by the guard and named in the Lean, and no authenticator reads it:
  `hot_v3/series_expiry.rs` reads the *projected* alias at 54 instead. Have
  `require_series_expiry_future_market_vacancy_v1` read coordinate 7 from the emission, so the Custody
  windows' `CoreMarket` and the child parent have **one** author. That is the whole point of §0.
- `generated_series_state_v3.rs:9` `SERIES_PHASE_LIMIT_V3 = 2` — its stated purpose has no consumer; the
  ticket sibling `SERIES_TICKET_PHASE_LIMIT_V3` is consumed by `ticket_admission_v1.rs`. **S11's
  `SeriesRootAdmissionV1` bitset is exactly the consumer** — one seam closes both.

The other three are dead weight: `SERIES_EXPIRE_CLOCK_COORDINATE_V5 = 77` (`:43`, zero callers, not even
guard-pinned — Trading names Clock nowhere and Core reads its own frame),
`SERIES_EXPIRE_RENT_SYSVAR_COORDINATE_V5 = 78` (`:45`, same), and `SERIES_STATE_BODY_OFFSET_V3 = 16`
(`generated_series_state_v3.rs:11` — `replay.rs` reads the eleven per-field offsets and never the body
offset). Consume or delete the `emitConst` lines (`EmitSeriesExpireFrameV5Rust.lean:67, :69`).

### 10.6 A deleted test with no replacement

The 860-line cut in `crates/dclutch-operator/src/series_hot_v3.rs` (§1.4, "one path: V5") took four tests
with it. Three covered the deleted builders and go with them. The fourth,
`every_series_occurrence_action_round_trips_through_the_kernel_decoder`, was **the only in-tree proof that
a Series family request round-trips through `SeriesActionRequestV3::decode`**, and nothing replaced it.
The V5 path deserves the same test.

### 10.7 The tree now says things that are no longer true

`tools/local-validator/bootstrap/successor/README.md:585` still documents the deleted
`local-private-validator-series-hot-campaign-v1` as "its Series sibling" (outside `commands.py`'s
`DEFAULT_ROOTS`, so no gate fires — which is why it will rot).
`programs/dclutch-trading-sbf/program-test/tests/support/series_premarket_expiry_v1.rs:666`'s
MEASURED-2026-09-01 comment still reads `bindings=81 geometry.logical=81`; the frame is 82 — **this is
§0's defect, in a comment, still there.** `docs/MASTER_COMPLETION_CONTRACT.md:95` (C-07) still states the
81-coordinate frame, the `0x4001` diagnosis, and an "uncommitted Shadow callback" that is committed at
`crates/dclutch-trading/src/shadow_accelerator_auth/mod.rs`.
`docs/evidence/PROTOCOL_CAPABILITY_FLOW_FRONTIER_2026_08_29.md:78` names three deleted builders (dated
evidence — arguably leave).

### 10.8 What was checked and is correct

Both new `.rs` files are declared in `series/mod.rs:33, :43`; both new `.lean` files are imported at
`DClutchSemantics.lean:118, :124`; the lakefile needs **no** change (the lib globs `DClutchSemantics.+`
and every `Emit*.lean` runs as `lake env lean --run`, never `lake exe`); no `Cargo.toml` is owed; the
successor's `mod series_devnet_v1;` (`main.rs:47`) and its three dispatcher arms (`:242-250`) plus usage
(`:2378`) are wired, and the deleted `SERIES_COMMAND_V1` arm leaves no dangling reference. The
`custody_child_market` threading is complete at **every** call site — `execute.rs:1506, :1562, :1665`,
`children.rs:436, :440, :541, :664, :833, :1111` — with no stale-arity caller anywhere.
`series_expiry.rs:40-95`'s five `const _` asserts all hold at 82 and no literal `81` survives as a width.
**No new refusal variant anywhere**, so the refusals register and the SDK code table are untouched. Zero
`sorry`, zero `todo!()`.

### 10.9 Rebase hazard — `main` has moved past the base

The branch's base is `f7c03e845`; `main` is at `19d5d2060` and still advancing. Of the nine files `main`
changed, **three overlap this branch**:

- `tools/local-validator/bootstrap/successor/src/series_terminal_campaign.rs` — **the real one.** `main`
  edited it and so did the branch (§1.4, and §4's devnet stub lives at `:1580-1594`). Expect a conflict,
  and resolve it knowing that §10.3 says the stub is load-bearing for the four runbook rows.
- `tools/cohort/steps.tsv` — `main` rewrote the `retire` row's verifier around line 85; the branch inserts
  four rows just below. **Textual conflict likely** — and note `build/founder-bond` and
  `build/claims-split-merge` also insert rows in this file. Three branches, one TSV.
- `tools/local-validator/bootstrap/successor/src/main.rs` — `main` added arms near `:209` and a usage line
  near `:2317`; the branch adds at `:242-250` and `:2378`. Should auto-merge; every cited line shifts.

Also re-check `docs/evidence/witnesses/cohort-17-discovered.json` and `docs/ledger/2026-09-06.md`, both
moved on `main`.

## 11. STOPPED HERE — the next three steps, concretely

1. **Fix the six hard breaks in §10.2 — all six are one-line or one-file, and none needs a build.**
   H1 (the guard's own line floor, `check-generated-series.sh:84`: 60 → 57) is the most embarrassing and
   the cheapest: **the branch's emission guard has never passed.** H2 repoints two `scalar()` calls and
   unblocks the SDK's test suite. H3/H4 are a word and a comma in `steps.tsv`. H5 is four headings in
   `tools/cohort/README.md`. H6 is `cargo fmt`. Do these before anything else: they are the difference
   between a branch a convergence lane can evaluate and one where every gate is red for reasons unrelated
   to the work.

2. **Build the Trading SBF ELF from this branch and run the three owed rows.** Everything in §1.1–§1.3 is
   a claim about a program that no lane has linked. `cargo build-sbf` the Trading program, then run
   `programs/dclutch-trading-sbf/tests/series_pre_market_expiry_program_test.rs` (three rows). The claim:
   they clear the `custody-prepare` width check that `SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 = 82` now
   authors on both sides, clear case 6 (`custody.market != parent.market`, `generation 1 != 9`) because
   the child binds to the future occurrence Market, and reach route 4's Core CPI. If they do, the number
   that replaces the fixture header's stale paragraph (§0, and the stale comment at
   `series_premarket_expiry_v1.rs:666`) is theirs. **If they do not, §0's repair is wrong and nothing
   downstream is safe** — this is the one step that can refute the family. While the ELF is in hand,
   capture `tools/gate frames --capture` (§10.4) and confirm `execute_child_routes_v3` is still under 4096.

3. **Give coordinate 7 a reader, and decide the devnet stub.** §10.5's
   `SERIES_EXPIRE_FUTURE_MARKET_COORDINATE_V5` is emitted, guarded, proved in Lean, and read by nothing —
   while `series_expiry.rs` derives the same fact from the projected alias at 54. That is the two-authors
   defect §0 is about, reproduced inside the repair. One call site fixes it. Then decide §10.3: either
   build `SeriesLedgerIdentityV1`'s cluster form (the maker says it is "one argument away, not one design
   away" — four places in the campaign's journal reader) or move the four runbook rows to the loopback
   arm, which is real. As written the rows refuse.

Then: S3 (the semantic-bound Series certificate — `joined_artifacts.rs:251` is the only Series
authoring site in the tree, and `::new_semantic` has no Series caller, so §1.7's new acquisition arm is
**dead until it lands**), S11/S12 (the SDK `series-root` row and the three record specs — what makes the
explorer stop saying "layout not emitted" for `DCLTSSV3`/`DCLTSTV3`/`DCLTSOV3`/`DCLTSKV3`), S10 (the
journey's `SeriesStageV1` — the `series-expire` verifier asserts an L1/L7/L8 census that nothing can
compute), and §10.6's replacement round-trip test.
