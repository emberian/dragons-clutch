# BUILD_general-lifecycle — General's fifteen actions, executed

Lane BUILD-general-lifecycle, branch `build/general-lifecycle` off `main` at `f7c03e845`, worktree
`/private/tmp/claude-501/-Users-ember-dev-dragons-clutch/ef2920a4-77e9-4597-99e3-94569deb51f7/scratchpad/build-general-lifecycle`.
Built by source inspection; nothing here was compiled, run or deployed. Every path is relative to the tree root.
Written incrementally: each step's section lands with its commit.

## 1. What was built

### Step 1 — the work escrow is a funding action (`FundingOperationV5::Fund`)

The wall: SubmitCandidate's Effect funded the candidate's work escrow with a local `transfer_lamports` out of the
solver, a System-owned signer, and the runtime refused `ExternalAccountLamportSpend` after 678,245 CU. Neither
existing funding operation could carry it: `Create` refuses a state the lifecycle creates in the same commit, and
`Close` drains. The repair is a THIRD operation, Lean-first.

| file | what |
|---|---|
| `formal/dclutch-semantics/DClutchSemantics/EffectProgramV5Abi.lean` (new) | the DCE6 header (32 B), funding action (24 B) and funding seed (40 B) layouts; opcodes create 0 / close 1 / **fund 2**; seed kinds; bounds 16/64/16; the schema preimage and id; `fundShape`/`createShape`/`closeShape` predicates and `valid`; the General work-escrow Fund as `generalWorkEscrowFund` (state 5, payer 6, System 8, target scalar 93 = `SCRATCH_B`, refund identity 18 = `identity::PAYER`); nine theorems (widths 32/24/40 exact, layouts disjoint, opcodes distinct, the Fund is valid and is neither Create nor Close, and five hostiles refuse: seeds, width, refund, no System, aliased System, state == payer); byte witnesses `oneFundHeaderWitness`, `fundActionWitness` and three hostile actions |
| `formal/dclutch-semantics/EmitEffectProgramV5AbiRust.lean` (new) | emits constants, offsets, the schema pair and the witness corpus |
| `formal/dclutch-semantics/DClutchSemantics.lean` | imports the module |
| `crates/dclutch-vm/src/effect/generated_v5_abi.rs` (new) | the emitter's output, hand-written in rustfmt's shape; the guard re-emits and compares |
| `crates/dclutch-vm/tests/effect__effect_v5_lean_generator_fresh.rs` (new) | the freshness + schema-preimage guard, cloned from the V4 one |
| `crates/dclutch-vm/src/effect/mod.rs` | `pub(crate) mod generated_v5_abi` |
| `crates/dclutch-vm/src/effect/v5.rs` | `OPCODE_FUND = 2`, `FundingOperationV5::Fund`, `FundingActionV5::fund(state, payer, system, target_scalar, refund_owner_identity)`; accessors: `payer()`/`system_program()` are `Some` for Create and Fund, `refund_destination()` only Create, `rent_credit()` only Close; `validate_tables` Fund arm (no seeds, no width, no refund, a System program distinct from both parties); `mod fund_tests` (round trip, five hostiles, a Create then a Fund on distinct states, same state refused, and the Lean constants + witnesses agree with the kernel) |
| `crates/dclutch-vm/src/account_profile/v3.rs` | `FundingActionMaskV3::FUND = 4`, `ALL = 7`, `permits_fund()`; decode admits bits ≤ 7. A `FUND` bound is the one case where the lifecycle keeps create/close authority over the coordinate |
| `programs/dclutch-trading-sbf/src/hot_v3/lifecycle.rs` | `require_funding_profile_join_v5`: `Fund if bound.permits_fund()`; `require_funding_runtime_v5`: the Fund arm (state vacant-System or live-Trading at the bound's width, payer signer+writable and `payer.key == refund_owner`, System executable, `amount > rent minimum`); new `apply_funding_top_ups_v5` — System `transfer(payer → state, target − before)`, refuses a state above its target, postcondition `state == target` |
| `programs/dclutch-trading-sbf/src/hot_v3/execute.rs` | `commit_prepared_hot_result_v3`: `apply_funding_top_ups_v5` runs after `apply_funding_creates_v5` and before children (reached through `use lifecycle::*` in `hot_v3.rs`; no import line) |
| `crates/dclutch-trading/src/general/effect_artifacts_v3.rs` | SubmitCandidate's two lamport instructions deleted (count 23 → 21); CloseCandidate's three deleted (3 → 0, see step 2); `general_funding_actions_v5(action)` (one `Fund` row for SubmitCandidate over `GENERAL_STATE_ACCOUNT_COORDINATE_V3`, payer 6, the profile's System coordinate, `SCRATCH_B`, `identity::PAYER`; empty for the other fourteen), `GeneralFundingTableV5`, `general_effect_program_bytes_v5`, `encode_general_effect_program_v5_atomic` (V5 over V4 over V3; decode-back check) |
| `crates/dclutch-trading/src/general/account_rules_v3.rs` | `general_funding_bounds_v3(action)` (SubmitCandidate: the Candidate coordinate, `FUND`, `64 + 224` bytes), `GeneralFundingBoundsV3`, `general_account_profile_funding_bytes_v3`, `encode_general_account_profile_funding_v3_atomic`; CloseCandidate's credit coordinate (8) is a 0-byte writable wallet rule (the solver), see step 2 |
| `crates/dclutch-trading/src/general/artifacts_v3.rs` | the release validator decodes `AccountProfileV3` and `ProgramV5`, requires the bound table == `general_funding_bounds_v3(action)` and the action table == `general_funding_actions_v5(action)`, seeds 0; descriptor schemas are V3/V5; the test fixture builds both envelopes |
| `crates/dclutch-operator/src/general_selected_release_v1.rs` | `encode_account_profile` → the V3 envelope, `encode_effect` → the V5 envelope; the descriptor stamps `account_profile::v3::SCHEMA_RELEASE_ID_V3` and `effect::v5::SCHEMA_RELEASE_ID_V5` |

RULING (provisional): all fifteen actions ship the V3 profile and V5 effect envelopes, fourteen with empty tables, so
the family has one schema per artifact and every reader decodes one shape. Cost: every General artifact re-digests
→ a fresh General founding (cohort-18), which cohort-18 does anyway for step 2.

### Step 2a — a `Payer` close pays the recorded beneficiary's wallet, and carries the cleanup crank

The wall behind CloseCandidate's "same unsatisfiable shape": one coordinate (8, the plan's `rent_credit`) had to be the
market's 128-byte RentCredit record for `apply_lifecycle_closes_v3` and the solver's wallet for the projector and the
Effect. Read against decision 0021: a `Payer` close relaxed the beneficiary EQUALITY but still paid the market's credit,
so every Payer-declared state (the Dealer LP position, the General candidate) refunded its rent to the market's
sponsor. And the Candidate's cleanup crank was a second author for the state's balance (an Effect transfer out of a
state the lifecycle drains first in commit order).

| file | what |
|---|---|
| `crates/dclutch-vm/src/account_profile/lifecycle_v3.rs` | `GUARD_ALWAYS_WITH_CRANK_REWARD = 2`, `PlanGuardV3::AlwaysWithCrankReward { register }` (byte 25 source, 26–27 index, 28–39 zero), Close-only (decode refuses it elsewhere; a Close names a payer iff it carries the guard); `plan_close`: `Credit` unchanged; `Payer` authenticates the credit coordinate BY IDENTITY (`key == declared beneficiary`, writable, not executable) and reads its balance as a wallet; the crank: `payer` coordinate account (credit permission), `crank = scalar(register)`, `state.lamports >= principal + crank`, `rent_credit_after = credit + state − crank`; `CloseStatePlanV3` gains `crank_destination: Option<[u8;32]>`, `crank_lamports` |
| `crates/dclutch-vm/src/account_profile/lifecycle_v3/encode.rs` | `LifecycleGuardInputV3::AlwaysWithCrankReward { register }` (tag 2, byte 25/26) |
| `programs/dclutch-trading-sbf/src/hot_v3/lifecycle.rs` `apply_lifecycle_closes_v3` | `Credit` → the RentCredit record authenticated as today; `Payer` → `credit.key == plan.beneficiary`, writable, non-executable; the crank paid to `accounts[prepared.payer]` (`key == plan.crank_destination`) before the beneficiary; sum conserved |
| `crates/dclutch-trading/src/general/state_artifacts_v3.rs` `close_candidate_shape` | `payer: Some(GENERAL_PRIMARY_PAYER_ACCOUNT_V3)` (the crank destination), `guard: AlwaysWithCrankReward { CANDIDATE_CLEANUP_REMAINING_OBSERVATION }` |
| `crates/dclutch-trading/src/general/account_rules_v3.rs` | CloseCandidate's coordinate 8 is a 0-byte writable wallet rule (step 1's table) |
| `crates/dclutch-trading/src/general/effect_artifacts_v3.rs` | CloseCandidate's Effect moves no lamports (step 1's table) |
| `formal/dclutch-semantics/DClutchSemantics/StateLifecyclePolicyV5Abi.lean` | the three guard tags as Lean facts + distinctness theorem (the 40-byte plan record itself is a Rust-authored layout — pre-existing debt, named in §4) |

RULING (provisional): a `Payer` close's destination is the beneficiary's wallet at the plan's credit coordinate,
authenticated by identity; the crank reward is a lifecycle-plan fact (one author for the state's balance), never an
Effect instruction. Reversal cost: the CloseCandidate route stays unexecutable and Dealer LP closes keep paying the
market's sponsor.

## 2. THE SEAMS

(filled per step; exact file:line and the one-line change)

- `tools/gates/emission-coverage.md` — regenerate with `tools/gate emission --write`; the new emitter/guard pair is
  `EmitEffectProgramV5AbiRust.lean` ↔ `crates/dclutch-vm/tests/effect__effect_v5_lean_generator_fresh.rs`.
- `crates/dclutch-vm/src/effect/generated_v5_abi.rs` — hand-shaped; if the guard disagrees on whitespace, regenerate
  with `cd formal/dclutch-semantics && lake env lean --run EmitEffectProgramV5AbiRust.lean | rustfmt --edition 2024`.
- `tools/local-validator/bootstrap/successor/src/general_session.rs:2048` `runtime_suffix_accounts_v1` decodes the
  published profile with `AccountProfileV2::decode`; it must become `AccountProfileV3::decode(..).base()` (step 4
  replaces the function).
- `tools/local-validator/bootstrap/successor/src/general_session.rs:611` `encode_profile_v1` and the width recovery
  compare a re-encoded V2 profile against the published bytes; they must re-encode through
  `encode_general_account_profile_funding_v3_atomic` (step 4).
- `packages/dclutch-sdk/lib/generalPlanV5.ts` — the SDK's artifact-id readers are digest-only and need no change; the
  web's `abi:general-v5:verify` re-emits nothing about these schemas.
- Every frame moves: `tools/gates/frames-baseline.json` rows for Trading (the funding arms) — cohort-18 carries them.

## 3. Rulings (provisional; ember rules by reversal)

1. `Fund` is a third funding operation rather than a lifecycle-plan field: the lifecycle owns create/close of a
   state, funding owns lamport movement into and out of one; a top-up is funding's.
2. The Fund's target is a REGISTER (the accelerator's `SCRATCH_B`), never a request field: the amount is derived from
   the submission's own compartments and the rent principal the lifecycle quoted, and a caller may not state it.
3. Uniform V3/V5 envelopes across the fifteen (above).

## 4. Stubs and named debt
- The lifecycle action-plan record (40 bytes, `ACTION_PLAN_BYTES`) is Rust-authored; only its header/quote layout is
  in `StateLifecyclePolicyV5Abi.lean`. The crank guard's tag is stated in Lean; its byte placement is not emitted.
- Two General request derivers exist (`dclutch-chain-bundle-builder::general::derive_general_request_v1` for
  program-test corpora; `dclutch-operator::general_hot_v3::derive_general_request_v5` from a route's observed
  accounts). The successor calls the operator's; the harness keeps its own. Not merged here.

## 5. Tests written
- `v5::fund_tests` (4): round trip; five hostiles refused by name; ordering with a Create; the Lean witness corpus
  decodes/refuses as the kernel does. `effect__effect_v5_lean_generator_fresh` (2): the checked-in generated file is
  the emitter's rustfmt'd output; the schema id is the preimage's SHA-256.
- The Lean module's nine `native_decide` theorems.

## 6. Frames and codes expected to move
- Trading link: `require_funding_profile_join_v5`, `require_funding_runtime_v5`, `apply_funding_top_ups_v5`,
  `commit_prepared_hot_result_v3` — cohort-18.
- No refusal code moves; the Fund refuses through `Content`/`Transition`/`Commit` like Create and Close.

---

# CLOSEOUT APPENDIX (lane CLOSEOUT, snapshot at `58a1cf520`, 2026-09-06 21:50)

**This worktree was STILL BEING WRITTEN when the closeout swept it.** The maker
committed five times after the sweep began (`28b3aa20c`, `4b7192e46`, `01a2064c7`,
`7b265a6b5`, `58a1cf520`). Everything above §1's step 2a is the maker's own text
and stops at step 2a; steps 3 onward are described here, from the diff. Read
`git log main..build/general-lifecycle` before trusting either half to be complete.

## A. What exists beyond step 2a

### Step 3 — one author for every General action's runtime frame

- `crates/dclutch-operator/src/general_session_v1.rs` (**new, 1022 lines**).
  `devnet-general-session` could frame exactly ONE of General's fifteen actions:
  its subject derivation read the capability root alone and its runtime-suffix
  resolver knew four coordinates (state, payer, credit, System). This module says,
  for every action and every runtime coordinate, which account belongs there.
  - `GeneralSessionErrorV1` — an input the action needs and the caller did not
    state is refused by name; nothing here signs, reads a chain, or invents a
    coordinate.
  - `GeneralSubjectV1` / `GeneralSubjectStatesV1` / `general_subject_states_v1` —
    the primary/secondary/result state PDAs for an action from the subject the
    caller names (a batch, an order, a candidate), through the family's own
    `GeneralStateRecipeV3` seed recipes; plus
    `general_last_opened_batch_id_v1` and `general_next_batch_id_v1`.
  - `GeneralFrameSourceV1` / `general_frame_sources_v1` — coordinate →source,
    read off the same authors the AccountProfile is emitted from
    (`state_artifacts_v3` for the state prefix and evidence table,
    `effect_artifacts_v3` for the child routes, the Claims/Custody frame specs for
    the child coordinates), so a coordinate here **cannot disagree with the
    profile the chain holds**.
  - `GeneralEscrowChildrenV1::derive` — the escrow children of one order or one
    candidate (the Position pair, the Custody replay, the vault) from
    `ProtocolPositionSeedsV2` / `ProtocolPositionAdmissionSeedsV2` /
    `CustodyReplaySeedsV1` / `CustodyVaultSeedsV1`; `general_hoard_vault_v1`,
    `general_custody_authority_v1`, `GeneralEscrowPartyV1`,
    `GeneralEvidenceAddressV1`.
  - `GeneralFrameInputsV1::resolve` and `general_runtime_suffix_v1` →
    `Vec<GeneralRuntimeAccountV1>`: the physical runtime suffix the route states,
    with the profile's own privileges per ordinal.
- `crates/dclutch-operator/src/lib.rs:81` — `pub mod general_session_v1;`.

### Steps 4–6 — the coarse `InvalidCoordinate` split into named clauses

General's projectors joined hundreds of accusations with `||` and published one
word — `GeneralHotCandidateErrorV3::InvalidCoordinate` — for all of them. Eight
new modules give each conjunct a name, in the shape
`crates/dclutch-trading/src/general/submit_candidate_clause_v3.rs` already had:

| module (all `crates/dclutch-trading/src/general/`) | enum | variants |
| --- | --- | --- |
| `open_batch_clause_v3.rs` | `OpenBatchClauseV3` | 27 |
| `close_batch_clause_v3.rs` | `CloseBatchClauseV3` | 31 |
| `place_order_clause_v3.rs` | `PlaceOrderClauseV3` | 60 |
| `cancel_order_clause_v3.rs` | `CancelOrderClauseV3` | 54 |
| `release_order_clause_v3.rs` | `ReleaseOrderClauseV3` | 41 |
| `verify_candidate_clause_v3.rs` | `VerifyCandidateClauseV3` | 17 |
| `close_candidate_clause_v3.rs` | `CloseCandidateClauseV3` | 34 |
| `settlement_clause_v3.rs` | `SettlementClauseV3` | 77 |
| (pre-existing) `submit_candidate_clause_v3.rs` | `SubmitCandidateClauseV3` | 57 |
| (in `hot_candidate_v3.rs`) | `GeneralRecordV3` | 8 |

**Both coarse codes reached zero raise sites.** `InvalidPlan` — "a record did not
decode", published where every record decoded and the ACTION simply was not one that
geometry serves — is DELETED from `GeneralHotCandidateErrorV3`. `InvalidCoordinate`
is raised by nothing: the only surviving mention in code is its own `log_line()` arm
at `hot_candidate_v3.rs:2873`.

Each clause variant is raised **exactly once** and carries a log line. A mechanical
check by the maker confirmed that all **301 operands of the eighteen original
`||` chains survive verbatim** as named clauses, with exactly one deliberate
deletion: the beneficiary conjunct that step 2a's `Payer` close recipe replaced.

`SettlementClauseV3` is deliberately ONE enum over six functions (the bank
environment reader, InitializeSettlement, the Consider/Freeze applier, the
environment validator, its custody shape check, the position geometry) because
they are one pipeline with one caller and one reader; two of those six used to
publish `InvalidPlan` ("a record did not decode") for conditions where every
record decoded and the ACTION simply was not one that geometry serves — that
spelling is gone.

- `crates/dclutch-trading/src/general/mod.rs` — eight new `pub mod` lines.
- `crates/dclutch-trading/src/general/hot_candidate_v3.rs` (**+2229/−689**) —
  every projector rewritten from `||`-joined booleans to per-clause helpers
  (`open_batch_clause(cond, Clause::X)?`, `scalar_u32(scalar::PAGE_INDEX,
  Clause::Y)?`, …). Eight tests each assert an EXACT named
  clause (none weakened to `is_err()`), so a projector that starts refusing for a
  different reason fails rather than passes.
  `GeneralHotCandidateErrorV3` gains nine payload-carrying
  variants: `Record(GeneralRecordV3)`, `OpenBatchCoordinate`,
  `CloseBatchCoordinate`, `PlaceOrderCoordinate`, `CancelOrderCoordinate`,
  `ReleaseOrderCoordinate`, `CloseCandidateCoordinate`, `VerifyCoordinate`,
  `SettlementCoordinate`.
- `programs/dclutch-accelerator-sbf/src/general.rs:1226` `candidate_cause` — the
  two-line log now covers every arm that has words for its own clauses (which,
  after this split, is all of them): outer line = which arm, inner line =
  `clause.log_line()`. Neither sentence is written in the program; each enum's own
  module owns it.

## A2. A ruling and a process finding

**RULING (the coordinator's, provisional; ember rules by reversal).**
`GeneralHotCandidateErrorV3::InvalidCoordinate`'s discriminant **stays as a
WITHDRAWN variant, and is not renumbered** — band contiguity under decision 0007,
the same treatment Custody's four reservation codes got when their routes were
deleted. Its doc comment should say that nothing raises it and that an author
reaching for it must write a clause enum instead. Reversal cost: renumbering an
in-band Trading code that has already shipped in a deployed link.

**PROCESS FINDING: this worktree did not have one author, and its commit
boundaries are not honest.** The family maker and a child agent both worked in it,
and a sibling lane's `git add -A` swept four of the child's new files into a commit
about something else. Concretely, `0c6f38f13` ("operator: one author for every
General action's runtime frame") contains, besides its own
`crates/dclutch-operator/src/general_session_v1.rs` and `lib.rs`, the child's
`cancel_order_clause_v3.rs`, `close_batch_clause_v3.rs`, `open_batch_clause_v3.rs`
and `place_order_clause_v3.rs`. **The content is right; only the attribution is
wrong.** Commits whose boundaries can be trusted on this branch:
`9a16601a7` (step 1), `60c9fa83c` (step 2a), `68b43301c`, `49b10cf8e`, `28b3aa20c`,
`4b7192e46`, `01a2064c7`, `7b265a6b5`, `58a1cf520`. Do not bisect across
`0c6f38f13` expecting the operator change alone. The general rule this pays for: a
worktree shared by a maker and its own children is not a worktree with one author,
and `git add -A` in one is not the safe operation it is in a private tree.

## B. THE SEAMS (beyond the maker's §2)

1. **`crates/dclutch-trading/src/general/account_rules_v3.rs:1700`** — the ONE
   compile error on this branch: `pub fn encode_general_account_profile_v3_atomic`
   has no doc comment and `crates/dclutch-trading/src/general/mod.rs:2` is
   `#![deny(missing_docs)]`. One `///` line fixes it.
2. **`crates/dclutch-trading/src/general/effect_artifacts_v3.rs:82`** — unused
   import `GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3` (CloseCandidate's Effect no
   longer moves lamports). Drop it from the `use`.
3. **`GeneralHotCandidateErrorV3` gained nine variants.** Every `match` on it must
   grow arms. The two in-tree matchers are
   `programs/dclutch-accelerator-sbf/src/general.rs` (**done on this branch**) and
   `programs/dclutch-trading-sbf/program-test/bundle-builder/src/general.rs`
   (**NOT audited** — it is a program-test corpus builder outside the `-p
   dclutch-trading-sbf` check, so its break will not surface until the harness is
   compiled).
4. **`crates/dclutch-operator/src/general_session_v1.rs` has NO CALLER.** `lib.rs`
   declares the module and nothing in `crates/`, `programs/` or `tools/` calls
   `general_subject_states_v1`, `general_frame_sources_v1`,
   `general_runtime_suffix_v1` or `GeneralEscrowChildrenV1::derive`. The intended
   caller is the successor's session driver, and the two exact edits are the
   maker's §2 items:
   `tools/local-validator/bootstrap/successor/src/general_session.rs:2056`
   (`AccountProfileV2::decode(published_profile)` → `AccountProfileV3::decode(..).base()`)
   and `:611` `encode_profile_v1` plus its two callers at `:558-559` and the width
   recovery at `:1063` (re-encode through
   `encode_general_account_profile_funding_v3_atomic`). Until those land the whole
   1022-line module is a producer nothing consumes.
5. **`tools/gates/emission-coverage.md`** — regenerate (`tools/gate emission
   --write`) for the new pair `EmitEffectProgramV5AbiRust.lean` ↔
   `crates/dclutch-vm/tests/effect__effect_v5_lean_generator_fresh.rs`.
6. **Frames ratchet** — the Trading link moves (`require_funding_profile_join_v5`,
   `require_funding_runtime_v5`, `apply_funding_top_ups_v5`,
   `apply_lifecycle_closes_v3`'s Payer arm) and the Accelerator link moves (the
   `candidate_cause` match). `tools/gates/frames-baseline.json` recapture owed;
   cohort-18 carries them.
7. **Every General artifact re-digests** (uniform V3/V5 envelopes across the
   fifteen, §3 ruling 3), so a fresh General founding is required — cohort-18.

## C. STOPPED HERE

`cargo check -p dclutch-operator --offline` (run by CLOSEOUT in a target dir
private to this family — see the warning below): **RED, exactly one error**, and
it is a lint, not a shape:

```
error: missing documentation for a function
    --> crates/dclutch-trading/src/general/account_rules_v3.rs:1700:1
     |
1700 | / pub fn encode_general_account_profile_v3_atomic(
1701 | |     action: Action,
1702 | |     widths: GeneralExternalAccountWidthsV3,
1703 | |     scratch: &mut [u8],
1704 | |     output: &mut [u8],
1705 | | ) -> Result<()> {
     | |_______________^
note: the lint level is defined here
    --> crates/dclutch-trading/src/general/mod.rs:2:9
   2 | #![deny(missing_docs)]
```

plus one warning (seam B-2). `dclutch-vm`, `dclutch-market`, `dclutch-product` and
every other dependency compiled clean; `dclutch-trading-sbf`, `dclutch-operator`
and `dclutch-accelerator-sbf` were never reached because `dclutch-trading` stopped
the build, so **this branch has been type-checked only as far as
`dclutch-trading`.**

> **A trap the next lane should not fall into.** The first run of this check used a
> target dir shared with the other closeout families and reported FIVE errors —
> `FundingActionMaskV3::FUND`, `FundingActionV5::fund` and
> `LifecycleGuardInputV3::AlwaysWithCrankReward` "not found". All three exist in
> this worktree. Cargo had reused a `dclutch-vm` rmeta built from a SIBLING
> worktree (the units hash the same across worktrees and this branch's `dclutch-vm`
> sources are older than that artifact), so the errors were about a `dclutch-vm`
> that is not this one. **Give every worktree its own `CARGO_TARGET_DIR`.**

The next three steps, concretely:

1. **Add the `///` and drop the unused import** (seams B-1, B-2), then re-run
   `cargo check -p dclutch-operator` in a private target dir to reach
   `dclutch-trading-sbf`, `dclutch-accelerator-sbf` and `dclutch-operator` for the
   first time. `hot_candidate_v3.rs` changed by 2,229 lines and has never been
   compiled; expect real work there.
2. **Wire the successor to `general_session_v1`** (seam B-4): the two
   `general_session.rs` edits, then delete whatever subject/suffix derivation they
   replace, so the module stops being a second author.
2b. **Give the withdrawn `InvalidCoordinate` its doc** (ruling A2): one comment
   saying nothing raises it and that a new accusation gets a clause enum, so the
   next author does not re-widen it.
3. **Grow the bundle-builder's match** (seam B-3) —
   `programs/dclutch-trading-sbf/program-test/bundle-builder/src/general.rs` — and
   then run the emission gate (B-5) so the V5 ABI guard and
   `emission-coverage.md` agree before the frames recapture (B-6).
