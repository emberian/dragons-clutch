# MERGE_NOTES_general-lifecycle

Lane CONVERGE-general-lifecycle, branch `build/general-lifecycle`, rebased onto `main` at
`8d38723d7` (clean, no conflicts: main's twenty commits and this family's thirty-three files are
disjoint).

## 1. READY

Every crate this family touches is green, `--tests` included, in one private
`CARGO_TARGET_DIR`:

| package | `cargo check` | `--tests` | tests run |
| --- | --- | --- | --- |
| `dclutch-vm` | green | green | `effect::v5` 11, `account_profile::v3` 5, `account_profile::lifecycle_v3` 13, `effect__effect_v5_lean_generator_fresh` 2 |
| `dclutch-trading` | green | green | `general::*` 270 |
| `dclutch-trading-sbf` | green | green | `hot_v3` 62 |
| `dclutch-accelerator-sbf` | green | green | `general` 3 |
| `dclutch-operator` | green | green | `general_session_v1` 3, `general_selected_release_v1` 20 |
| `dclutch-chain-bundle-builder` | green | — | (seam B-3 refuted, see §5) |
| `dclutch-local-successor-bootstrap` | green | green | `general_session` 8 |

Zero errors, zero test failures. The Lean side builds and both its guards pass with the real
toolchain (`lake build DClutchSemantics.EffectProgramV5Abi`, then the emitter through `rustfmt`).

**What is NOT proven here**: no SBF link was built and nothing was run against a validator or
devnet. Every claim above is fixture-level or unit-level evidence.

## 2. SHARED FILES — for the merge lane

Checked: **no other build branch touches either file**
(`git diff --name-only main...build/<family>` over all eleven siblings returns zero hits for both),
so the first is applied on this branch rather than handed on.

- `tools/gates/emission-coverage.md` — **APPLIED on this branch.** Regenerated with
  `tools/gate emission --write`. 91→92 generated, 92 guarded, **unguarded stayed 0**; the one new row
  is `crates/dclutch-vm/tests/effect__effect_v5_lean_generator_fresh.rs` ↔
  `EmitEffectProgramV5AbiRust.lean`. If a sibling family also regenerates this file, take the union
  of the rows, not either version whole.
- `tools/gates/frames-baseline.json` — **HANDED ON.** The Trading link moves
  (`require_funding_profile_join_v5`, `require_funding_runtime_v5`, `apply_funding_top_ups_v5`,
  `apply_funding_candidates_v5`'s new Fund arm, `apply_lifecycle_closes_v3`'s Payer arm) and the
  Accelerator link moves (`candidate_cause`). Recapture with
  `tools/gate frames --at <commit> --capture tools/gates/frames-baseline.json` after the SBF build.
  This lane is forbidden SBF builds, so the ratchet is left red deliberately.
- **Cohort-18 is required before any General route executes.** Every General artifact re-digests:
  all fifteen actions now ship the V3 account-profile envelope and the V5 effect envelope (the
  maker's §3 ruling 3). A cohort-17 market cannot serve these artifacts.

## 3. What this lane completed, and what it ruled

### Made it compile — four defects, not the one the closeout measured

The closeout reported "RED with exactly one error — a missing `///`". There were four, and the
first was not a missing `///`.

1. `crates/dclutch-trading/src/general/account_rules_v3.rs` — the maker inserted
   `general_funding_bounds_v3` **between** `encode_general_account_profile_v3_atomic`'s
   fourteen-line doc block and its function. The `deny(missing_docs)` error was the visible half;
   the invisible half was a funding-bound table documented by prose about an encoder. The doc block
   was moved back, not rewritten — the text is `main`'s, verbatim.
2. `crates/dclutch-trading/src/general/effect_artifacts_v3.rs` — unused import dropped.
3. `programs/dclutch-trading-sbf/src/hot_v3/lifecycle.rs:1585` —
   `apply_funding_candidates_v5`, the candidate model the commit compares its poststate against, was
   the **third** match on `FundingOperationV5` and the maker grew only two. Its new Fund arm mirrors
   `apply_funding_top_ups_v5` exactly: a state above its target refuses, the payer is debited the
   difference, the state becomes the target, the width is untouched.
4. `crates/dclutch-operator/src/general_session_v1.rs:37` — a private module path
   (`dclutch_custody::frame_spec_v1`) where the crate re-exports at its root. This module had never
   been compiled.

Plus one `--tests`-only break: a `lifecycle_v3` test predating `crank_destination`/`crank_lamports`.

### The orphaned module now has its caller, and its second author is deleted

`general_session_v1.rs` (1,022 lines) had no consumer, and the command it was written for was still
the second author. Both halves are closed in
`tools/local-validator/bootstrap/successor/src/general_session.rs`:

- `runtime_suffix_accounts_v1` was an if-else chain over four coordinates that refused every other
  one as unmappable. It is now an adapter over `general_runtime_suffix_v1`, reading the chain facts
  back out of the fixed frame the command already built and validated.
- `open_batch_state_address_v1` derived the batch address a second time from the same
  `GeneralBatchOccurrenceTermsV1` the module calls. **Deleted**; `general_subject_states_v1` is the
  one author. The now-unused imports of `GeneralBatchOccurrenceTermsV1`, `GeneralBatchOpeningV1`
  and `GeneralStateAddressSeedsV3` are the proof.
- `encode_profile_v1` re-encodes through `encode_general_account_profile_funding_v3_atomic`, which
  is what the chain now publishes. A V2 base compared against a V3 record would have reported every
  width **unlocatable**, not a shorter record — a confusing failure this never reached because the
  path had not been run.

**RULING (provisional).** `GeneralFrameInputsV1` could not be constructed by an honest caller: it
required an aggregate, a mint, a realm record, Claims, Custody, the token program and the Rent
program as plain `Pubkey` fields, and a command that frames `OpenBatch` observes none of them, so
calling the module at all meant inventing seven addresses. They are now one `GeneralChildChainV1`
behind an `Option`, refused by name (`InputMissing("child-route chain frame")`). Seven of the
fifteen actions invoke a child route; the other eight reach none of it. Reversal cost: a caller
must fabricate seven addresses to frame an action that names none of them.

**RULING (provisional).** `COMPOSABLE_ACTIONS_V1` stays `[OpenBatch]`. The subject derivation is no
longer the wall — `general_subject_states_v1` derives all fifteen, and `CloseBatch`'s subject is the
batch the root's own `next_batch_sequence` last opened, needing no chain read — but admitting
`CloseBatch` is a devnet claim and this lane ran no devnet. The refusal text and its test now say
that instead of the stale "needs the open Batch account read back". **Admitting `CloseBatch` is a
one-line change plus one devnet run**; that is the cheapest real widening available to the next lane.

### `InvalidCoordinate` is withdrawn (the coordinator's ruling, recorded)

The discriminant is held, not renumbered — a deployed Trading link published it and decision 0007's
bands are append-only. Its doc had called it "the landing place for a conjunct written before it has
a clause enum", which invites re-widening the code the family just spent nine enums splitting; it now
states the withdrawal and that a new accusation gets a clause enum. Its `log_line` says the same.
Confirmed zero raise sites: every surviving mention in `crates/`, `programs/` and `tools/` is doc
prose in the nine clause modules.

### The Lean work-escrow Fund was asserting a false fact

`generalWorkEscrowFund` declared `systemProgram := 8` and its own doc claimed "the coordinates are
the 2026-09-06 profile's". SubmitCandidate's System program is coordinate **11** — the first past the
three readonly evidence accounts, which are what start at 8. Nothing executable was wrong, because
`general_system_program_account_v3` derives the number and the Rust table calls it; that is exactly
the failure a Lean witness exists to catch, and this one was asserting the wrong thing instead.
Fixed, rebuilt, re-emitted. Four witness arrays moved one byte each; the schema preimage and id did
**not** move; all nine `native_decide` theorems still hold; the checked-in
`generated_v5_abi.rs` is the emitter's `rustfmt` output again.

## 4. Tests, and what each proves

Nothing was weakened to make anything pass. Every expected value below was read out of the running
code first, never guessed.

**Tests repaired (each had never been run, and each was measuring something other than its name).**

- `dclutch_vm::effect::v5::fund_tests::the_lean_constants_are_this_kernels` — **could not have
  passed.** Its base fixture declared two common scalars and one common identity, with a doc comment
  claiming that was "wide enough for the registers a Fund names", while the Lean witness it decodes
  names `SCRATCH_B` (scalar 93) and `identity::PAYER` (18). `validate_tables` checks the register
  range *before* the per-operation shape, so the canonical witness refused `ActionTable` — and all
  three hostiles refused for that same reason: a seed count, a live width and a refund destination
  were each "proven" to refuse by a base too narrow to look at them. `base_v4_with` now takes the
  register file. Proves: the Lean Fund witness is what this kernel decodes as a Fund, and each
  hostile refuses for its own single perturbation.
- `lifecycle_v3::payer_refunded_states_name_the_funding_and_refuse_every_stranger` — step 2a made a
  `Payer` close pay the beneficiary's wallet at the credit coordinate; the test still put the
  market's RentCredit record there. The credit-coordinate account is now an argument. **One
  assertion is new and it is the one that convicts decision 0021's defect**: with the market's
  record at the coordinate and the payer declared, an owned close now REFUSES. That is the case
  that used to succeed and send a participant's rent to the market's sponsor.
- `effect_artifacts_v3::submit_candidate_effect_resolves_exact_creation_funding_at_runtime_widths` —
  asserted operations 21 and 22 (the `TransferLamports` and `RequireLamportsEq` step 1 deleted) over
  a hardcoded `0..23`. It now walks `fixed_operation_count()`, refuses past the end by name, asserts
  that **no** operation moves lamports (so a future author cannot put the runtime-refused spend
  back), and asserts the Fund row that replaced them. Coverage followed the authority instead of
  being deleted with it.

**The last three bare `is_err()` in `hot_candidate_v3.rs` now name their clause.** Each hid more
than one answer:

- the five SubmitCandidate hostiles reach five different refusals —
  `Submission(OutsideWindow)`, `SubmitCoordinate(LifecycleCreated)`, `SubmitCoordinate(LifecycleBump)`,
  `SubmitCoordinate(LifecycleBeneficiary)`, `SubmitCoordinate(IdentitySubmissionCandidate)` — and one
  is not a coordinate clause at all but the candidate contract's own window refusal;
- the Materialize test's two substitutions reach `SettlementCoordinate(EnvironmentScalarOutcomeCount)`
  and `SettlementCoordinate(EnvironmentRequestDigest)`; `is_err()` accepted either answer for either
  input.

Those two are also **the only exact `SettlementClauseV3` assertions in the tree** (77 variants, and
it was the one clause enum of the nine with no exact assertion). All nine now have one, and
`hot_candidate_v3.rs` contains no `is_err()`.

## 5. What a reviewer should distrust in `BUILD_general-lifecycle.md`

Checked against the source, not taken on the doc's word.

**Verified**: SubmitCandidate's Effect carries no lamport instruction (23→21; the two surviving
`transfer_lamports`/`require_lamports_eq` in the file are `VerifyCandidateRow`'s). One Fund row for
SubmitCandidate, `EMPTY` for the other fourteen. One `FUND` bound, width `64 + 224 = 288`. The
release validator compares both tables element-wise and pins seeds to zero. `close_candidate_shape`
carries the payer and the `AlwaysWithCrankReward` guard. CloseCandidate's Effect moves no lamports
(3→0). The Payer arm's identity/writable/non-executable checks, and the close's lamport sum, is
conserved. `packages/dclutch-sdk/lib/generalPlanV5.ts` is digest-only and needs no change (the
browser mirrors neither wire; no `DCE5`/`DCE6` magic exists under `apps/dclutch-web/src`).

**Wrong, and corrected here**:

- §1 step 1's table and the Lean module said the General work-escrow Fund's System program is
  **coordinate 8**. It is **11**. 8 is the readonly-evidence start. Fixed in Lean; the doc line is
  still wrong.
- **Closeout §C: "RED, exactly one error, and it is a lint".** Four errors, and the first was a
  fourteen-line doc block attached to the wrong function.
- **Closeout §B-3: the bundle-builder's `match` on `GeneralHotCandidateErrorV3` "NOT audited"** and
  expected to break. **Refuted**: `programs/dclutch-trading-sbf/program-test/bundle-builder/src/general.rs:1439`
  has no `match` — it is `impl Fn(GeneralHotCandidateErrorV3) -> BuilderError` formatting the error
  with `{error:?}`. Nine new variants need no arms. `cargo check -p dclutch-chain-bundle-builder`
  is green with no change. **No work is owed here.**
- **§2 step 2a: "CloseCandidate's coordinate 8 is a 0-byte writable wallet rule".** The wallet rule
  is real but it is at **coordinate 7** (`GENERAL_PRIMARY_RENT_CREDIT_ACCOUNT_V3`; `two_state` at
  `account_rules_v3.rs:2018` excludes CloseCandidate). Coordinate 8 is the readonly ClosedBatch
  evidence account. The code is right; the doc names the wrong coordinate.
- **§2 step 2a: the crank is "paid to `accounts[prepared.payer]` before the beneficiary".** The
  *netting* is the kernel's, and the runtime writes the crank **last**
  (`lifecycle.rs:2025` state, `:2028` credit, `:2031` payer). Conservation holds either way; the
  ordering sentence does not describe the code.
- **§5's test list** presents `v5::fund_tests` and the Lean theorems as passing evidence. The Lean
  theorems did hold; `the_lean_constants_are_this_kernels` could not have passed and had not been
  run. Treat "tests written" in that doc as "tests typed".

**Not found at all** (raised in the wave brief, no referent in the tree):

- *"the escrow children installed at founding so PlaceOrder's fourteen empty coordinates hold"* —
  neither half exists. There is no founding-time installation of Position/Custody children anywhere
  in the ledgers or the BUILD doc; the only ledger phrase is the wave charter's own line "the escrow
  at founding", which names this lane's assignment and refers to step 1's work escrow — the Fund —
  which is built. And PlaceOrder has no count of fourteen: its 103 coordinates hold 55 route
  aliases, 10 `Exact(0)` vacant-account rules and 9 deliberately zero-width opaque rules. "Empty" is
  not a word the source uses; its vocabulary is vacant, opaque and alias. **RULING (provisional):
  nothing is owed here.**

## 6. Named debt, left deliberately

- `programs/dclutch-trading-sbf/src/hot_v3/tests.rs` contains **63** bare `is_err()`. Pre-existing
  and outside this family (the branch adds 2 lines to that file). Not touched.
- The lifecycle action-plan record (40 bytes) is Rust-authored; only its header/quote layout is in
  Lean, and the crank guard's tag is stated in Lean but its byte placement is not emitted. The
  maker's §4 named this; it is still true.
- Two General request derivers still exist (`dclutch-chain-bundle-builder::general` and
  `dclutch-operator::general_hot_v3`). The successor calls the operator's; the harness keeps its
  own. Not merged.
- `crates/dclutch-operator/src/wallet_terminal_payout/wire.rs` has an unreachable match arm and
  three unused bindings. Pre-existing, another family's file, not touched.
