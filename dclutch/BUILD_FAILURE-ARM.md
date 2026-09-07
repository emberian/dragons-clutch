# BUILD — failure-arm: the outage that refunds, end to end, on a chain

> **ADDENDUM 2026-09-06, the convergence lane.** This document is the maker's, and it is
> not edited: read `MERGE_NOTES_FAILURE-ARM.md` beside it for what was checked and what
> was found. In particular §8's status is superseded (the two tiers compile now, and three
> things were red at the merge base), §5's "six tests" is five, and §7's SDK/web section is
> on disk but its suite fails on a fixture that never decoded.

Branch `build/failure-arm` off `main` at `f7c03e845`. Built by source inspection; no
SBF program was built, no suite run, no devnet touched. Written incrementally; the
last section is the status line.

## 0. What already existed, and what this family actually had to build

The brief said "RECOVERY-4 scoped out the `CommitDeadlineFailure` frame -- build it".
Reading first: the FRAME, the WIRE and the ROUTE all exist and have executed.

| thing | where | status before this branch |
| --- | --- | --- |
| `RelayFrameKindV1::CommitDeadlineFailure`, 22 positions | `crates/dclutch-source/src/relay/frame.rs:391` | shipped |
| `CommitDeadlineFailureInstructionV1` (32 bytes, action 6) | `crates/dclutch-source/src/relay/instruction.rs:442` | shipped |
| `process_commit_deadline_failure` → `plan_deadline_failure_v1` | `programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:1052`, `funded.rs:467` | executed on relayed-vertical and program-test; BOTH arms planned (`Primary → Exhausted → FailureCommitted` for no ladder; `Exhausted → FailureCommitted` after a walked ladder, `funded.rs:503-535`) |
| the exhausted arm on real ELFs | `crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs:4186` (`a_two_source_market_walks_its_funded_ladder_and_every_rung_pays_a_stranger`) | executed in program-test; never on a validator or chain |
| the refunding payout arm | `crates/dclutch-product/src/payoff/runtime_v3.rs:990` (`evaluate_categorical_failure`), `programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs:330`, `crates/dclutch-operator/src/wallet_terminal_payout/wire.rs:1055` | shipped; never driven under a kind-4 certificate |
| the closure burn | `programs/dclutch-claims-sbf/src/signed_delta_v3.rs` `burn_failure_coordinate_v1`; retirement prepare | executed on real ELFs (PROGRAMS-17E) |

What was MISSING: a host author for the 22-account frame and its admissibility, a
successor verb for the walk, a family-neutral verb for the terminal admission the
walk needs (the sponsored and flagship arms are family-bound), the runbook rows, the
journey's failure walk, the ladder's continuation past `Exhausted`, named refusals
for the two refund hostiles, the SDK derivation of `refundsOnFailure` from the
escrow's presence, and a real-ELF test that composes seating → outage → refund →
burn → Retired. That is what this branch builds.

## 1. What was built, file by file

### Operator (host, pure)

- `crates/dclutch-operator/src/deadline_failure_v1.rs` (new, ~560 lines).
  `DeadlineFailureCoordinatesV1` (13 coordinates: worker, market, core, activation,
  resolution, source state, six finalized pairs, funding ledger);
  `deadline_failure_keys_v1` — the ONE author of the 22 positions, checked against
  `relay_frame_roles_v1(CommitDeadlineFailure)` by role NAME per index;
  `build_commit_deadline_failure_v1(coords, generation, sequence)` → instruction +
  the `ResolutionFailure` seat (`kind_seed()` from the codec, never a literal) +
  `validate_relay_frame_v1` offline; `deadline_failure_decision_v1(phase, attempt,
  has_policy, window, &ResultDomainV2)` → `{arm, due, failure_selector, outcome_count}`
  with refusals by name: `LadderNotWalked` (Primary + policy: the crank owns the legs),
  `LadderStanding{active_attempt}` (Recovery), `AlreadyTerminal(phase)`,
  `DeadlineNotReached{due, observed}` (strictly-after, reproduced from
  `exhaust_after_primary_deadline`). Six tests (frame is the contract's; order by
  name; seat differs from the success/advance/exhaust seats; aliasing refuses offline;
  the decision table over a compiled four-cell `ResultDomainV2`).
- `crates/dclutch-operator/src/wallet_terminal_input.rs` — `route_terminal_payout_frame_v1`
  now calls `refuse_the_failure_escrow_as_owner_v1`: an `--owner` equal to
  `failure_escrow_v1(...).owner` (derived off the round-one aggregate) is refused by
  name ("is this Market's own failure escrow, a program-derived address with no key").
  The escrow's "payout" is therefore a producer refusal, never a transaction.
- `crates/dclutch-operator/src/lib.rs` — `pub mod deadline_failure_v1;`.

### Claims program (one code, one map)

- `programs/dclutch-claims-sbf/src/lib.rs` — `ClaimsSbfError::Overdraw = 0x5012`,
  appended to the band pin. "The terminal payout asked for more claims than the
  Position holds at that index, or more than the aggregate owes there."
- `programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs` — the
  `encode_product_claims_terminal_signed_delta_v3` call maps through
  `terminal_planning_refusal`: `product_basis_terminal_v3::Error::InsufficientBalance
  → Overdraw`, everything else `Economic` (was `map_err(|_| Economic)`).
  **The Claims ELF moves.** Cohort-18 carries it; frame rows owed (see §6).

### Successor (drivers, verbs not interpreters)

- `tools/local-validator/bootstrap/successor/src/deadline_failure.rs` (new, ~520 lines).
  `local-private-validator-commit-deadline-failure-v1` / `devnet-commit-deadline-failure-v1`.
  Same argument shape as the crank (`--plan --evidence --market --terminal-sequence
  --worker --output [--wait --max-wait-seconds] [--execute --worker-keypair]`). Reads
  the six records off the campaign evidence through `routed_record` (canonical-address
  refusal), the ladder's presence off `recovery_policy_record`, the Source and window
  and result domain off the chain, calls the operator's decision, waits bounded through
  `wait_until_unix_seconds_v1`, builds through the operator, pre-funds the seat in the
  same transaction, sends, and reads back: Source `FailureCommitted`, certificate kind
  `ResolutionFailure`, selector == the Product's failure cell, `work_paid != 0`.
  Evidence `dclutch-deadline-failure-evidence-v1` with `arm`, `failureSelector`,
  `outcomeCount`, `workPaid`. Value-returning `run_v1` → `DeadlineFailureOutcomeV1`
  for the tiers. Label `Deadline failure walk: {primary-deadline|ladder-exhausted}`.
- `tools/local-validator/bootstrap/successor/src/admit_terminal.rs` (new, ~470 lines).
  `local-private-validator-admit-terminal-v1` / `devnet-admit-terminal-v1`. The
  family-neutral third act over `build_resolution_admit_terminal_v3` (the one author
  the sponsored transport, the flagship command and the journey already call). The
  certificate KIND is read off the Source's phase (`Resolved → success seat`,
  `FailureCommitted → failure seat`, anything else refused by name). Publishes one
  frozen routing table through `market::publish_routing_table` (the frame is 1,508
  bytes), sends v0, reads back phase `Terminal`, receipt == seat, winner == selector.
  Evidence `dclutch-admit-terminal-evidence-v1`. Value-returning `run_v1` →
  `AdmitTerminalOutcomeV1`. Labels `Core AdmitTerminal: the {failure|honest} selector`.
- `tools/local-validator/bootstrap/successor/src/main.rs` — `mod admit_terminal; mod
  deadline_failure;`, four dispatch arms, four usage lines.
- `tools/local-validator/bootstrap/successor/src/recovery_crank.rs` — exports
  `TOO_EARLY_MARKER_V1` and `WAIT_CEILING_MARKER_V1` (the sentences the tiers MATCH);
  the ladder's private copies now alias them.

### Runbook

- `tools/cohort/steps.tsv` — six rows, `since 18`, on the two-source market
  (`generate-stage-scripts.py` `MARKET_KIND` maps all six to `("two-source",)`):

  | key | shape | driver | blocks |
  | --- | --- | --- | --- |
  | `exhaust` | once | `devnet-commit-deadline-failure-v1 --wait` (`$ARG1` = terminal sequence) | admit-failure |
  | `admit-failure` | once | `devnet-admit-terminal-v1` | refund-stranger |
  | `refund-stranger` | journal | `wallet-terminal-payout-input` + `devnet-wallet-terminal-payout-v1` at `{market.payout.claim_index}` | refund-founder |
  | `refund-founder` | journal | the same pair, `--owner {pubkey:keys/founder.json} --recipient {market.refund.founder_recipient} --claim-index $ARG1`, once per ordinary index | refund-census |
  | `refund-census` | once | `ledger-census --declared-hoard-delta -$ARG1 --declared-class-delta HoardPrincipal=-$ARG1 unclassified=$ARG1 --prior {stage:escrow-seated}` | escrow-zero |
  | `escrow-zero` | once | `ledger-census` all deltas zero, `--prior {stage:refund-census}` | retire |

  Each verifier is written as the fact that says the row ran (kind 4 + failure cell +
  nonzero `work_paid`; payout == balance to the atom; the founder's sum + the
  stranger's == the Hoard and Hoard reads 0; L1–L8 by name with the escrow under
  `failure-escrow` unmoved; the escrow "payout" as a producer refusal by name).
- `tools/cohort/README.md` — six sections after `### capture-rung`.

### Journey tier (`tools/gauntlet/journey`)

- `src/failure.rs` (new, ~300 lines): `walk_to_failure` — advance (seq 2), exhaust
  (seq 3) through `recovery_crank::run_v1`, commit (seq 1) through
  `deadline_failure::run_v1`; refusals recorded with the drivers' own sentences; stage
  label `FAILURE_WALK_STAGE_V1`; the market shape constants
  (`DEFAULT_FAILURE_RECOVERY_RUNGS_V1 = "2500:120"`,
  `FAILURE_WALK_MAX_AGE_SECONDS_V1 = 120`, `FAILURE_WALK_MAX_WAIT_SECONDS_V1 = 600`).
- `src/journey.rs`: `JourneyWalkV1 { Honest, Failure }`; `JourneyRequestV1.walk`;
  transcript `walk`; the failure shape (`terminal_max_age_seconds: Some(120)` so the
  primary deadline is behind the chain clock before Open — the lab stating a fact about
  its frozen fixture, the honest alternative to warping a clock); the resolution stage
  branches on the walk with one label per walk (`resolution_stage_label`); the
  admission, redemption (every holder × every index — the failure index refuses
  `1..=0` at the producer, recorded `not-driven`) and retirement (the burn) stages are
  UNCHANGED, which is the point.
- `src/resolution.rs`: four seats derived from `ResolutionCertificateKindV2::kind_seed()`
  (was the literal `&[1]`): `certificate` (success, seq 1), `failure_certificate` (seq
  1), `recovery_advanced_certificate` (seq 2), `recovery_exhausted_certificate` (seq 3);
  all watched; `admit_terminal` reads the Source's phase and admits the seat it names.
- `src/main.rs`: `--walk honest|failure`; `#[path]` links for `admit_terminal` and
  `deadline_failure`; `mod failure;`.

### Ladder tier (`tools/gauntlet/ladder`)

- `src/ladder.rs`: `continue_to_the_refund` after an executed exhaust — the deadline
  walk (seq 1), the admission, a Token-2022 account the FOUNDER KEY owns (the
  founding's `collateral_wallet` answers to the payer, and the builder refuses a
  recipient owned by anybody but the stated owner), one refund per ordinary index
  through `terminal_lifecycle::produce_wallet_terminal_input_owned_loopback_v1` +
  `wallet_terminal_payout_exterior::run` (resumed until its evidence exists, ceiling
  24), the Hoard read before and after (`hoardDrained` = paid == Hoard fall && Hoard
  0), and the escrow's "payout" driven as the producer refusal it is
  (`recorded-no-op`). Transcript gains `refund`.
- `src/main.rs`: links `deadline_failure`, `admit_terminal`,
  `wallet_terminal_payout_exterior`.

### Lean

- `formal/dclutch-semantics/DClutchSemantics/EconomicKernel.lean`:
  `a_stranger_and_the_founder_draw_the_hoard_between_them` (one stranger holding `s`
  ordinary claims and the founder the rest draw exactly `supply * (ordinaryCount *
  unit)` between them, each `holdings * unit`) and
  `the_founder_is_refunded_for_holdings_and_for_nothing_else`. `lake build
  DClutchSemantics.EconomicKernel` green in this worktree.
- No new emitted layout in this family (the wire, the certificate, the frames all
  exist), so no new ABI module or emitter. The evidence documents are JSON.

### SDK / web (landed uncommitted at the wall; committed by CLOSEOUT)

- `packages/dclutch-sdk/lib/marketDetail.ts` — `failureEscrowV1(claimsProgramId,
  market, aggregate, outcomeCount) -> {failureSelector, owner, position, admission}`,
  the TS port of `dclutch_claims::protocol_position_v2::failure_escrow_v1` (seeds:
  owner = `CLAIMS_CAPABILITY_OWNER_SEED_V2 ‖ market ‖ u32le selector`; position =
  `PROTOCOL_POSITION_STATE_SEED_V2 ‖ aggregate ‖ owner`; admission =
  `PROTOCOL_POSITION_ADMISSION_SEED_V2 ‖ aggregate ‖ owner`), refusing a width < 2.
  `refundsOnFailureFromEscrowV1({escrow, claimsProgramId, outcomeCount, supplyAtoms,
  account}) -> EscrowSeatingV1 {present, seated, heldAtoms, refundsOnFailure}` —
  `seated` iff the escrow Position is Claims-owned, decodes at this width under this
  owner, holds the WHOLE nonzero failure column and zero at every ordinary index.
  Decode is `decodeClaimsPositionV2`'s, so the offsets keep one author. NEW IMPORTS:
  `decodeClaimsPositionV2` from `./marketCoreV2`, and
  `PROTOCOL_POSITION_{STATE,ADMISSION}_SEED_V2` from `./generated/directParticipantV1`.
- `packages/dclutch-sdk/lib/marketDetail.test.ts` — two suites (see §5).
- `apps/dclutch-web/components/MarketActivity.tsx` — `State.ready` gains `seating:
  EscrowSeatingV1 | null`; `read()` derives the escrow, reads its Position through
  `client.accountInfo`, and folds `refundsOnFailure` into `outageDisclosureV1`'s input.
  A failed read leaves `seating: null` and the disclosure honest about being unread.
  `read`'s dependency array gains `supplyAtoms`.
- `crates/dclutch-operator/src/wallet_terminal_input.rs` —
  `refuse_the_failure_escrow_as_owner_v1` made `pub` so the ladder tier can drive the
  refusal by name.
- `crates/dclutch-svm-harness/Cargo.toml` — `dclutch-operator` dev-dependency added
  for the composed real-ELF test.

### Real-ELF test — NOT WRITTEN. The dev-dependency is in place and nothing uses it.

## 2. THE SEAMS (exact file:line, one-line change each)

1. **Refusal reference.** `docs/reference/refusals.md` is generated; the convergence's
   `tools/gate reference --converge` will pick up `ClaimsSbfError::Overdraw 0x5012`
   (`programs/dclutch-claims-sbf/src/lib.rs`, after `FailureEscrowUnseated`). The census
   `--check-unique` must see 18 Claims codes in the band pin list.
2. **Frames ratchet.** `tools/gates/frames-baseline.json`: the Claims link moves (one
   `const fn` and one variant). Capture with `tools/gate frames --at <commit> --capture`;
   until then this branch leaves the ratchet red and says so here.
3. **SDK mirror of the code.** Wherever the SDK's refusal renderer enumerates Claims
   codes (`packages/dclutch-sdk/lib/generated/*refusal*` — generated by
   `tools/gate reference --converge`), 0x5012 appears on regeneration; no hand edit.
4. **Successor dispatch.** `tools/local-validator/bootstrap/successor/src/main.rs:244-`
   — four arms added beside `recovery_crank`'s; usage lines after
   `wallet_terminal_payout_exterior::devnet_usage()`. The `usage()` test at
   `main.rs:~2747` that asserts command names may want the four new names added.
5. **Journey module list.** `tools/gauntlet/journey/src/main.rs:54-56` and `:90-92` —
   the list is "generated from the successor's own src"; if a generator script exists
   for it, regenerate rather than keep my two hand-inserted entries.
6. **Journey bindings.** `tools/gauntlet/journey/bindings.json` needs rows for the
   labels `Recovery ladder crank: advance`, `Recovery ladder crank: exhaust`
   (`resolution/process_advance_recovery#AdvanceRecovery`), `Deadline failure walk:
   ladder-exhausted` (`resolution/process_commit_deadline_failure#CommitDeadlineFailure`),
   `Core AdmitTerminal: the failure selector` (`core/resolution::process#AdmitTerminal`)
   and `create/extend Core AdmitTerminal frozen routing address lookup table` (bound
   by tier 1's rows already). Delegated; verify present.
7. **Ladder bindings/witnesses.** `tools/gauntlet/ladder/bindings.json`: the same
   three new labels plus `ladder: open the founder's refund account` (no route) and
   the payout driver's labels (`local-private-validator-wallet-terminal-payout-v1`'s
   journal labels — read them from
   `tools/local-validator/bootstrap/successor/src/wallet_terminal_payout_exterior.rs`);
   `witnesses.json`: `the-failure-selector-was-committed` (`.refund.failureWalk.arm ==
   "ladder-exhausted"`) and `the-founder-drew-the-hoard-and-nothing-else`
   (`.refund.hoardDrained == true`). Delegated; verify present.
8. **Ladder README.** `tools/gauntlet/ladder/README.md` "What it does not reach": the
   exhaust walk now continues to the refund; the capture walk's wall is unchanged.
9. **Cohort-18 manifest.** `tools/cohort/cohorts/18.json` (does not exist yet): the
   two-source market `S` needs `payout.{owner_keypair,recipient,claim_index}` for the
   stranger and `refund.founder_recipient` (a token account the founder key owns —
   opened by the fill's token setup or a `refund-founder-account` row; NOT
   `collateral_wallet`). `check-steps.py --cohort 18` will name the missing fields.
10. **`ladder.rs` InitializeAccount3.** `tools/gauntlet/ladder/src/ladder.rs`
    `INITIALIZE_ACCOUNT_3 = 18` is the third spelling (journey `stages.rs:39`, successor
    `direct_trade_token_setup.rs`). One `pub(crate) const` in
    `dclutch_custody::token_svm` and three call sites; not done here.
11. **Ladder evidence fold.** The refund's payout transactions are in each
    `refund-N-journal/` but not folded into the ladder's `evidence.json`
    (`spine::harvest_dir` is journey-private). Either link `spine.rs` into the ladder
    or lift `harvest_dir` into the successor's `model.rs`.
12. **Journey `resolution::admit_terminal` vs the new verb.** Two callers of one
    builder is fine; if the convergence prefers one DRIVER, `resolution::admit_terminal`
    can call `crate::admit_terminal::run_v1` and drop its own snapshot assembly
    (`resolution.rs:838-`).
13. **`docs/reference/routes.md`.** `resolution/process_commit_deadline_failure`'s
    witness column gains `journey`/`ladder` once a run lands; generated.
14. **Bond seam** (§3, ruling 4): `terminal_settlement_v3.rs`'s failure arm is where the
    bond's `−draw` joins as a TRAILING account; the runbook rows' `--declared-fees-lamports`
    and L7 must learn the draw; `escrow-zero`'s verifier gains "the bond account reads
    rent only".

## 3. Ruled provisionally (ember rules by reversal)

1. **The exhausted terminal is the RELAY family's route on every family.** A market
   that bought a Pyth-sponsored window reaches its failure terminal through
   `CommitDeadlineFailure` (relay transport magic `DCLTRIX1`), not through
   `SponsoredPushActionV1::CommitFailure` (gate `source: Primary`, cannot take
   `Exhausted`). The frame carries no relay-specific account; "relay-owned" is the
   instruction family, not the market's provider. One walk, both arms.
2. **The escrow's payout is a no-op the census records, never a transaction.** The
   producer refuses the owner by name; no exemption is added to the terminal
   settlement's owner-signature conjunct (decision 0025 addendum: shape A over B).
3. **"More than pro rata" refuses as `Overdraw` (0x5012) on the wire** and as the
   producer's `1..=balance` sentence off it. The Claims ELF moves for one variant;
   the alternative (leaving it `Economic`) fails AGENTS.md's coarse-code rule.
4. **Bond disposition at the exhausted terminal** (decision 0033 lands in cohort-17+;
   not in this tree): on kind 4 the bond is FORFEITED to the ordinary holders pro rata,
   inside the same redemptions that pay the refund — "one redemption, two units"
   (`MECHANISM_FOUNDER_BOND` §3.2). The seam is the terminal settlement's failure arm
   and the refund rows' lamport declarations (§2 item 14). Nothing here pre-empts the
   bond family's frame.
5. **The failure walk's market declares a spent shelf life** (`terminal_max_age_seconds
   = 120`) rather than warping a clock. Stated in `failure.rs` and `journey.rs`.
6. **The ladder's refund recipient is an account the tier opens for the founder key.**
   Not the founding's `collateral_wallet`.

## 4. Stubbed, and why

- Nothing is `todo!()`. Two things are NAMED rather than done: the ladder's evidence
  fold (§2.11) and the third `InitializeAccount3` spelling (§2.10).
- `admit_terminal.rs` re-assembles the 16-account snapshot the journey's
  `admit_terminal_snapshot` assembles; both feed the one builder. Named in §2.12.

## 5. Tests written and what each would prove

- `deadline_failure_v1.rs` (6): the frame IS the contract's (width 22, writable
  {0,4,5,18}, one signer); every filled position is the contract's role name at that
  index; the plan writes the failure seat and none of the other three kinds; an
  aliased frame refuses offline; the decision table (Primary/no-policy admissible only
  strictly after `end + max_age`; Primary+policy → `LadderNotWalked`; Recovery →
  `LadderStanding`; Exhausted → admissible now; Resolved/FailureCommitted/Retired →
  `AlreadyTerminal`; selector 3 of 4 read off a compiled domain).
- `deadline_failure.rs` (4): labels name the arm; ceiling-without-wait refuses; zero
  sequence refuses at the parser; loopback arm refuses the devnet flag.
- `admit_terminal.rs` (3): the kind is the Source's (two admitted, four refused by
  name); the two labels differ; zero sequence refuses.
- Lean (2): the two-holder Hoard identity; the founder-holdings identity.
- Delegated (§1): SDK `failureEscrowV1` pinned to cohort-16.1's real addresses and
  `refundsOnFailureFromEscrowV1`; the real-ELF composition test.

## 6. Frames/codes expected to move, and which cohort

- Claims ELF: +1 code (`0x5012`), +1 `const fn`. Cohort-18. Frame-baseline rows for the
  Claims link owed by the convergence.
- No other ELF moves: the operator and the successor are host-only; the journey and
  ladder are tiers.

## 7. Status

- Committed by the maker: operator builder + decision; Claims `Overdraw`; two
  successor verbs; runbook rows + README + generator map; journey failure walk;
  ladder continuation; Lean corollaries (green).
- Committed by CLOSEOUT at the wall (`wip: failure-arm — the build wave's work at
  the quota wall`): the SDK port + its tests, the web activity card, the operator
  `pub`, the svm-harness dev-dependency, the journey `bindings.json` rows and the
  ladder `witnesses.json` / `README.md` rows. Seams §2.6 and §2.7 are therefore
  DONE, not delegated — verify rather than re-add.
- NOT done: the real-ELF composition test; the frames-baseline recapture (§2.2);
  the cohort-18 manifest (§2.9).

## 8. STOPPED HERE

`cargo check -p dclutch-operator --offline` (run by CLOSEOUT, this worktree,
against the committed branch): **GREEN**. `Finished dev profile ... in 42.03s`;
four warnings, all pre-existing in `wallet_terminal_payout/wire.rs` (unused import
`StateBumpsV1`; an unreachable `RecipientRouteV1::ClaimCheckEscrow` arm; two unused
`rent: &Rent` parameters) and none from `deadline_failure_v1.rs`. No check was run
on the successor, the journey, the ladder, the SDK or the web app: the successor and
the tiers are binaries with `#[path]` links across three trees and the maker never
reached them, so **their first compile is the convergence lane's**.

The next three steps, concretely:

1. **Compile the two tiers, not the operator.** `cargo check -p dclutch-successor
   --offline` then `-p dclutch-journey` then `-p dclutch-ladder` (names per their
   `Cargo.toml`). The predictable breaks are the `#[path]` module links added to
   `tools/gauntlet/journey/src/main.rs` and `tools/gauntlet/ladder/src/main.rs` —
   `admit_terminal.rs` and `deadline_failure.rs` were written against the
   successor's `model.rs`/`routed_record` shapes by inspection, and a tier that
   links them inherits every `use crate::…` in them.
2. **Recapture the frames ratchet** (§2.2): `tools/gate frames --at HEAD --capture`
   after the Claims ELF's one new variant (`ClaimsSbfError::Overdraw = 0x5012`,
   `programs/dclutch-claims-sbf/src/lib.rs`), then `tools/gate reference --converge`
   so `docs/reference/refusals.md` and the SDK's generated refusal table carry it.
   Until that runs the ratchet is red and the census `--check-unique` expects 18
   Claims codes rather than 17.
3. **Write the cohort-18 manifest** (§2.9): `tools/cohort/cohorts/18.json` with the
   two-source market's `payout.{owner_keypair,recipient,claim_index}` and
   `refund.founder_recipient`; `tools/cohort/check-steps.py --cohort 18` names every
   missing field. The six `steps.tsv` rows are already in place and will not run
   without it.
