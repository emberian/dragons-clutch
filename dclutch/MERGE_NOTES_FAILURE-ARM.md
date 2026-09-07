# MERGE NOTES — failure-arm

Branch `build/failure-arm`, rebased onto `main` at `8d38723d7` (a clean `git rebase main`,
no conflicts; main's nine changed files overlapped this family in three — `tools/cohort/steps.tsv`,
`tools/cohort/README.md`, `successor/src/main.rs` — and git merged all three without a hunk in
common). Head `e50674244`. 26 files, +4,544 / −43 against main.

## 1. READY

**READY.** Everything this family touches compiles, `--tests` included, and its tests run.

```
cargo check --offline --tests -p dclutch-operator -p dclutch-local-successor-bootstrap \
  -p dclutch-journey-campaign -p dclutch-ladder-campaign -p dclutch-claims-sbf
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 48.20s
```

```
lake build DClutchSemantics.EconomicKernel      Build completed successfully (8 jobs).
```

Tests run and green (filtered, never a bare `-p` suite):

| filter | result |
| --- | --- |
| `-p dclutch-operator --lib deadline_failure` | 5 passed |
| `-p dclutch-operator --lib failure_escrow` | 3 passed (2 existing + the one this lane added) |
| `-p dclutch-local-successor-bootstrap --bin … deadline_failure` | 4 passed |
| `-p dclutch-local-successor-bootstrap --bin … admit_terminal` | 3 passed |

**THE TWO TIERS COMPILED FOR THE FIRST TIME**, which is what this lane was told to find out, and
three things were red at the merge base. Verbatim, in the order they appeared:

```
error[E0609]: no field `market` on type `MarketAddressesV1`
   --> tools/gauntlet/journey/src/journey.rs:746:35        (698 at the merge base)
746 |     ledger.track_market(addresses.market);
    = note: available fields are: `founding_market`, `found31_market`, `aggregate`, …
```
**This one is MAIN'S**, not this family's: `ledger.track_market` is the producer main added for L4's
`market_phase` arm, and it named a field the struct does not have. The Core Market this campaign
resolves, admits, redeems and retires is `founding_market` (`resolution.rs:186`, `:356`,
`stages.rs:133`, `journey.rs:855`); `found31_market` is the second market the closing census reports
beside it. Whichever family lands first must carry this line — the journey campaign does not compile
without it, and no gate compiles that campaign.

```
error[E0499]: cannot borrow `*rpc` as mutable more than once at a time
   --> tools/gauntlet/ladder/src/ladder.rs:568:17
568 |                 rpc.minimum_balance(ACCOUNT_BYTES)?,
```
`rpc.minimum_balance` inside the argument list of `rpc.send_with_signers`. The rent is a local now.

```
error[E0063]: missing field `walk` in initializer of `journey::JourneyRequestV1`
    --> tools/gauntlet/journey/src/journey.rs:1740:29
```
The wall test predates `JourneyRequestV1.walk`.

### What is RED and is not this family's to make green

- **The frames ratchet.** The Claims link moves (one `#[repr(u32)]` variant and one `const fn`
  reached from `authenticate_and_prepare`), and this branch carries no
  `tools/gates/frames-baseline.json` rows. Deliberate: `tools/gate frames` **builds every one of the
  eight links** with `cargo build-sbf`, which this lane is forbidden, and a capture taken on this
  macOS host would not be the canonical Linux builder's. See §2.7 for the exact command.
- **`tools/gate reference`.** `docs/reference/refusals.md` and the SDK's generated refusal table gain
  `0x5012` on regeneration. AGENTS.md: "The convergence owner runs it; lanes do not."
- **`tools/gate fmt`, in both directions, and both sides are main's.** Measured with the gate's own
  `#[path]` normalisation: one unformatted file outside the baseline,
  `tools/local-validator/bootstrap/successor/src/market.rs` (main's own copy, checked out alone and
  measured, is dirty), and **nine baseline rows that are no longer true**:
  `crates/dclutch-product/src/svm_reader/mod.rs`, `tools/gauntlet/journey/src/provider.rs`,
  `tools/gauntlet/journey/src/resolution.rs`, `tools/gauntlet/relayed-vertical/src/substrate.rs`,
  `tools/local-validator/bootstrap/successor/src/{direct_trade,general_session,series_terminal_campaign,terminal_sequence,user_position_admission}.rs`.
  This family's own six are formatted. This lane does not edit a shared baseline to describe somebody
  else's file.
- **`check-steps.py --prove-frozen`** is red at `cohort-14: DOES NOT reproduce cohort-14.tsv`, and it
  is red on main too — I ran main's own `check-steps.py`, `steps.tsv`, `README.md` and `frozen/`
  out of `git show` into a scratch directory and got the same exit 1. The differing rows are
  `arm-relay`, `admissions`, `fill` and `census`, all `since` ≤ 14 and none of them this family's;
  since-18 rows are not selected by the cohort-14 view at all.

## 2. SHARED FILES — for the merge lane

**A code-number collision, and it is the one thing here that cannot be resolved by taking both sides.**

1. `programs/dclutch-claims-sbf/src/lib.rs` — `ClaimsSbfError::Overdraw = 0x5012` after
   `FailureEscrowUnseated`, and `Overdraw` appended to the `pin_refusal_band!` list.
   **`build/founder-bond` claims the same `0x5012` for `FounderBondFrame`.** Both are unlanded, so
   whichever lands second takes `0x5013`: decision 0007 forbids renumbering a code that has shipped,
   and permits it before. Do not merge both and leave two variants at one discriminant — the band pin
   is a `const _: () = assert!(…)` and will not compile, but the *doc comments* would both claim the
   number and a reader would believe the first one they read.
2. `programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs:426` —
   `.map_err(|_| ClaimsSbfError::Economic)?` becomes `.map_err(terminal_planning_refusal)?`, with the
   `const fn terminal_planning_refusal` added after `authenticate_and_prepare`. Only
   `product_basis_terminal_v3::Error::InsufficientBalance` changes code; everything else stays
   `Economic`.
3. `tools/cohort/steps.tsv` — six rows `exhaust`, `admit-failure`, `refund-stranger`,
   `refund-founder`, `refund-census`, `escrow-zero`, all `since 18`, placed **between `payout` and
   `retire`**. The position is load-bearing, not cosmetic: `check-steps.py` refuses a `blocks` edge
   that does not point later in the file, and `escrow-zero` blocks `retire`. Contested by
   `build/claims-split-merge` (`split`, `merge`), `build/founder-bond` (`refund-scale-seated`,
   `founder-bond`, `refund-bond-walk`, and an edit to `retire`) and `build/series` (four rows). All
   are appends at distinct rows; take every side and re-run `check-steps.py --cohort 18`.
4. `tools/cohort/README.md` — six sections after `### capture-rung`, one per row. `check-steps.py`
   fails a row the README does not document and a README key the table does not have.
5. `tools/cohort/generate-stage-scripts.py` — six `MARKET_KIND` rows mapping the failure arm to
   `("two-source",)`, plus **a guard fix that is not this family's alone** (§3.4). Uncontested by
   every other build branch.
6. `tools/local-validator/bootstrap/successor/src/main.rs` — `mod admit_terminal;` and
   `mod deadline_failure;` in the sorted list, four dispatch arms after `recovery_crank`'s two, and
   four `println!` usage lines after `wallet_terminal_payout_exterior::devnet_usage()`. Contested by
   `build/claims-split-merge`, `build/recovery-capture` and `build/series`, all additive in the same
   three places. The command names are constants, never literals:
   `deadline_failure::COMMAND_{,DEVNET_}V1` = `local-private-validator-commit-deadline-failure-v1` /
   `devnet-commit-deadline-failure-v1`; `admit_terminal::` the same shape for `admit-terminal`. The
   runbook rows name those exact strings; I cross-checked them.
7. `tools/gates/frames-baseline.json` — **owed, uncaptured, uncontested.** On the canonical Linux
   builder, at the merged commit:
   `tools/gate frames --at <commit> --capture /tmp/a.json` then a SECOND, independent
   `tools/gate frames --at <commit> --capture /tmp/b.json`, then
   `tools/gate frames accept --first /tmp/a.json --second /tmp/b.json --output tools/gates/frames-baseline.json`.
   Two captures of one commit is what `accept` requires and what "never one" means: a single capture
   records whatever the tree happened to say that run.
8. `docs/reference/refusals.md` and `packages/dclutch-sdk/lib/generated/*refusal*` —
   `tools/gate reference --converge` from a detached worktree at HEAD. No hand edit. The census's
   `--check-unique` then sees 18 Claims codes rather than 17 (19 if founder-bond lands too).
9. `crates/dclutch-operator/src/wallet_terminal_input.rs` — `refuse_the_failure_escrow_as_owner_v1`
   (public), its call from `route_terminal_payout_frame_v1`, and one test. Contested by
   `build/founder-bond`; the additions are at the end of the impl section and the end of the test
   module.
10. `apps/dclutch-web/components/MarketActivity.tsx`, `packages/dclutch-sdk/lib/marketDetail.ts`,
    `packages/dclutch-sdk/lib/marketDetail.test.ts` — contested by `build/founder-bond`, which
    touches all three. §3.6 and §5.4 say what changed and why it must not be reverted to the
    build-wave form.
11. `tools/gauntlet/journey/src/{journey,resolution,main}.rs`, `tools/gauntlet/ladder/{src/*,README.md,witnesses.json}`
    — contested by `build/recovery-capture`, which touches the same journey files and the whole
    ladder tier.
12. `tools/gauntlet/journey/bindings.json` — **byte-identical to main again.** §3.5.
13. `crates/dclutch-operator/src/lib.rs` — two `pub mod` lines, in sorted position (rustfmt reorders
    module declarations and the build wave appended them).

## 3. Completed since the build wave, and ruled provisionally

1. **The two tiers compile** (§1), and the journey campaign's merge-base red is main's line.
2. **`tools/cohort/cohorts/18.json` written.** It did not exist, so the six rows could not run.
   Every deploy-time fact is blank because a cohort is a full redeploy with fresh identities
   (AGENTS.md), market `S` carries the `payout` the stranger is paid under and the new
   `refund.founder_recipient`, and there is one `required_commits` entry for this family.
   `check-steps.py --cohort 18`: 45 steps, each documented, each naming a verifier, every field
   resolved. `generate-stage-scripts.py --cohort 18`: 75 scripts, 0 absolute paths, 0 credentials.
   **The merge lane must rewrite `required_commits.failure-arm.rev`** — it names `82bbb10c1`, this
   branch's own commit, which the rebase into main will renumber; `preflight.sh` refuses a rev that
   is not an ancestor of the deploy commit, by name, which is the reminder.
3. **Three defects in the build wave's runbook rows**, none of them findable by reading:
   - `check-steps.py`: *"step refund-stranger: the manifest has no field
     'market.payout.claim_index'"*. The `verifier` column is resolved against the manifest with no
     emit-time placeholders — only `args` gets those — so a `{market.*}` in a verifier is a field
     nobody answers. Reworded.
   - *"step escrow-zero blocks retire, which does not come after it"*. Fixed by the row move (§2.3).
   - The emitted `39-retire-1.sh`, a **Direct** market's retirement, was guarded on
     `escrow-zero-S/GREEN`.
4. **RULED: a per-market row is blocked only by what ran for its own market.**
   `generate-stage-scripts.py` took a per-market blocker's WHOLE market list whenever the two rows'
   kinds did not overlap, so `retire` (every kind) waited on `escrow-zero` (two-source only) for
   every Direct market. The guard now binds the same market or is not emitted.
   **CONTROL: cohort-17's 39 rows emit BYTE-IDENTICAL under main's generator and under this one**
   (`diff -r` over both output directories, no differences), so the change is confined to the new
   cross-kind edge.
5. **RULED: the journey's four new census bindings are removed, and the ladder's three witnesses
   stay.** The build wave's close-out added bindings for `Recovery ladder crank: advance` / `exhaust`,
   `Deadline failure walk: ladder-exhausted` and `Core AdmitTerminal: the failure selector`. Two
   reasons, and the second is not a matter of taste:
   - the tree's rule, stated four times in the ledger, is that **bindings are folded from what the
     ledger OBSERVED** — "a binding authored from what a campaign OUGHT to touch is exactly the false
     green `tools/gauntlet/retirement-checkpoint/` was built to remove";
   - `tools/gauntlet/census/src/ledger.rs:457` makes a binding that matched no transaction a PROBLEM,
     and the **honest** walk produces none of those four labels. Those rows did not merely overstate
     coverage, they turned the honest walk's census red.
   **Recover them for the fold after the first observed `--walk failure` run:**
   `git show 9e11e925d^:tools/gauntlet/journey/bindings.json` — the last four entries, with their
   routes, programs and provenance notes intact.
   The ladder's three `witnesses.json` entries stay: a witness is an assertion about the tier's own
   transcript with a stated default, not a coverage claim about a route, and that tier already
   carries one deliberately red on its capture fixture
   (`every-crank-frame-is-the-relay-contract-eighteen`). **Still owed there:**
   `tools/gauntlet/ladder/bindings.json` has no rows for the refund continuation's labels
   (`ladder: open the founder's refund account`, the deadline walk, the admission, and
   `wallet_terminal_payout_exterior`'s journal labels), so the exhaust walk's census will report
   unbound transactions on its first run. That is the honest discovery state and the fold comes after
   it, not before.
6. **The SDK fixture and the web card** — §5.4. Both were reader-facing or suite-facing wrongs, both
   fixed here rather than handed on.
7. **The journey reads the walk outcome it used to drop.** `FailureWalkOutcomeV1` was returned into
   `_outcome`; `cargo check` said so. After the terminal admission the campaign now refuses, by name
   and as a hard error, if Core's `terminal_winner` is not the selector the deadline driver read off
   the finalized `ResultDomainV2`, if the Market's `terminal_receipt` is not the seat the walk
   minted, or if the walk's `work_paid` is zero.
8. **Two claims with no consumer deleted**: the `dclutch-operator` dev-dependency in
   `crates/dclutch-svm-harness/Cargo.toml` (its comment described a real-ELF test that was never
   written; `Cargo.lock` is back to main's bytes and `cargo metadata --locked` is green), and the
   four journey bindings above.

### Still owed, named rather than done

- **The real-ELF composition test** (seating → outage → refund → burn → Retired) in
  `crates/dclutch-svm-harness`. It needs the SBF ELFs and a program-test run, both outside this
  lane's authority. Re-add the `dclutch-operator` dev-dependency **in the commit that writes the
  test**, not before.
- **The SDK suite has still never been run against this branch.** There is no `node_modules` in this
  worktree and no root workspace; `npm ci` needs the network.
  `cd packages/dclutch-sdk && npm ci && npx vitest run lib/marketDetail.test.ts` is the command, and
  §5.4 says what it would have found before this lane's fix.
- The ladder's refund payouts are in each `refund-N-journal/` and not folded into the ladder's
  `evidence.json` (`spine::harvest_dir` is journey-private) — the build wave's §2.11, unchanged.
- `INITIALIZE_ACCOUNT_3` has a third spelling in `tools/gauntlet/ladder/src/ladder.rs` — the build
  wave's §2.10, unchanged.

## 4. Tests added, and what each proves

**`crates/dclutch-operator/src/wallet_terminal_input.rs`:
`the_derived_failure_escrow_is_refused_as_a_payout_owner_by_name`.**

The escrow's payout is a no-op a census records, never a transaction, and the producer that says so
had no test. It cannot pass vacuously, and that is built into the test rather than asserted about it:
an ordinary holder is produced for, ANOTHER Market's escrow is produced for, and only this Market's
own escrow refuses — so a producer that refused everything fails the first `expect`, and one that
refused nothing fails the `expect_err`. The refusal is asserted by four conjuncts of its sentence,
never `is_err()`. The escrow is derived from the aggregate the test already built, the same way
`refuse_the_failure_escrow_as_owner_v1` derives it, and the failure cell is asserted to be the
width's last coordinate rather than the literal `2`.

**`packages/dclutch-sdk/lib/marketDetail.test.ts`** — not a new test, a fixture repair that makes two
existing assertions capable of passing. §5.4.

**The three cross-authority checks in `journey.rs`** are not `#[test]`s; they are refusals on the
campaign's own path, which is the only place the facts they compare exist.

## 5. What to distrust in `BUILD_FAILURE-ARM.md`, because I checked it

1. **"Six tests" for `deadline_failure_v1.rs` (§5). There are five.** `the_frame_is_the_relay_contracts_and_this_builder_only_fills_it`, `every_filled_position_is_the_role_the_contract_declares`, `the_plan_writes_the_failure_seat_and_no_other_kinds`, `an_aliased_frame_refuses_offline_by_name`, `the_decision_names_the_arm_the_second_and_the_selector`. The other four `fn`s in that module are fixtures. `cargo test … deadline_failure` prints `5 passed`. Everything else §5 claims is real: the successor's 4 and 3 run and pass, and the Lean builds.
2. **"Seams §2.6 and §2.7 are therefore DONE, not delegated — verify rather than re-add" is true and was the wrong thing to do.** §3.5 above.
3. **§7's "the SDK port + its tests … committed" is true; the implied green is not.** The doc's own §8 says no check was run on the SDK, and when the suite is run two assertions fail. Read §8 and not §7.
4. **The SDK/web port is correct where it derives and wrong where it concludes.** Both halves were established by a reader who reimplemented the derivation from scratch rather than reading it:
   - `failureEscrowV1` matches `dclutch_claims::protocol_position_v2::failure_escrow_v1` on every
     axis — seeds, the little-endian u32 selector, the owner→position→admission order, the width
     refusal — and an independent pure-Node PDA implementation fed the generated seed constants
     reproduces cohort-16.1's real devnet addresses (`Hq6sF5pv…`, `7FQCfc4R…`, `4WUZ2qZK…`) exactly.
     A big-endian selector or a swapped `(owner, aggregate)` gives different addresses, so every axis
     is load-bearing. The generated seed constants are byte-identical to the Rust and all three
     `abi:*:verify` generators exit 0 against this tree.
   - `positionBytes()` in the test **never wrote the u16 state version at offset 8**, so
     `decodeClaimsPositionV2`'s `header()` refused `state version 0 is unsupported`,
     `refundsOnFailureFromEscrowV1` caught it and returned the absent seating, and
     `expect(seated).toEqual({present: true, seated: true, …})` and `expect(beside.present).toBe(true)`
     could never have passed — while every `false` around them passed for the wrong reason. Fixed
     from the GENERATED constant.
   - **`MarketActivity.tsx` printed the sentence `outageDisclosureV1` exists to prevent.** It passed
     the seating straight into a parameter documented as `ProductBasisFactsV3.refundsOnFailure`, a
     payout-scale fact, whose own body comment says deriving the payee from the seating alone "would
     tell a buyer on a refunding market that the founder takes everything, which is the opposite of
     what would happen" — and whose test at `marketDetail.test.ts:426` names exactly the market that
     breaks it. The implication runs one way: only a refunding founding (v6) seats the whole failure
     column, so SEATED is evidence; UNSEATED is not, and a market founded to refund before that
     founding existed reads unseated and would have been described to a buyer as paying its founder
     everything. The card now passes `true` or the disclosure's honest `null`, and
     `EscrowSeatingV1.refundsOnFailure`'s doc — which claimed the seating is "stronger than the
     payout scale's intent" — says which direction it is stronger in.
5. **The second Lean corollary is a restatement, by its own admission.**
   `the_founder_is_refunded_for_holdings_and_for_nothing_else` is
   `an_admitted_founding_makes_every_refund_exact` applied directly, and its docstring says "Stated
   as the trivial identity it is". Not vacuous and not wrong — but §1's "two Lean corollaries" is one
   new fact (`a_stranger_and_the_founder_draw_the_hoard_between_them`, which the ladder's witness
   cites) and one name for an old one.
6. **§0's table is accurate.** The frame, the wire, the route, both planned arms, the refunding
   payout arm and the closure burn all exist where the doc says they do, and I read each.
   `product_basis_terminal_v3::Error::InsufficientBalance` exists and is the only variant
   `terminal_planning_refusal` re-codes.
7. **§2.4 and §2.5 are done and I verified them rather than trusting them.** The four dispatch arms
   and four usage lines are in `successor/src/main.rs` and name constants, not literals; the
   journey's `#[path]` set is 71 links against the successor's 73 `src/*.rs` files, and the two
   missing names are exactly the two the header says are excluded
   (`founding_submission_journal`, `owned_loopback_capture`).
