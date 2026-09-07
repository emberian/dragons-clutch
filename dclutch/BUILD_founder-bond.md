# BUILD founder-bond — the founder bond: posted, returned, or paid out

Lane BUILD-founder-bond, branch `build/founder-bond` off `main` at `f7c03e845`
(`/Users/ember/dev/dclutch`), worktree
`/private/tmp/claude-501/-Users-ember-dev-dragons-clutch/ef2920a4-77e9-4597-99e3-94569deb51f7/scratchpad/build-founder-bond`.
Source inspection only: no SBF build, no suite, no devnet. `cargo check -p
dclutch-claims -p dclutch-claims-sbf -p dclutch-operator --offline` green at
every commit; the fifteen crate tests of the new module run green (filtered).

The design is `docs/design/MECHANISM_FOUNDER_BOND_2026_09_04.md` and
`FounderBondV1.lean`; the ruling is decision 0033 (CONFIRMED, mandatory at the
size rule). This document is written incrementally; a section marked *(open)*
is still being built.

## 1. What was built, file by file

### Kernel — `crates/dclutch-claims/src/founder_bond_v1.rs` (new, 700 lines)

The Rust twin of `FounderBondV1.lean`. No new layout, so no emitter: the widths
are the Lean's PARAMETERS and this file names the tree's own constants for
them (`FOUNDER_BOND_WIDTHS_V1`: certificate seat
`RESOLUTION_CERTIFICATE_BYTES_V2` = 312, `CLAIM_CHECK_ESCROW_BYTES_V1` = 256,
Token-2022 `ACCOUNT_BYTES` = 165, `PROTOCOL_POSITION_ADMISSION_BYTES_V2` = 512,
`CLAIM_CHECK_BYTES_V1` = 288, `LIABILITY_BASIS_POSITION_HEADER_BYTES_V2` = 128,
8 per outcome; cap `COMPACTION_CRANK_REWARD_LAMPORTS_V1` = 200,000).

| item | Lean | Rust |
| --- | --- | --- |
| rent shape | `rentFor rate bytes` | `rent_for_v1` = `funded_rent_minimum_v2` (0030's author) |
| size rule | `bondSize` | `bond_size_v1(widths, FounderBondSizeInputV1 { rate, outcomes, crank_reward_cap, ladder_funding })`; `founding_bond_size_v1(rate, outcomes, ladder)` at the tree's widths |
| founding conjunct | `founded` | `founded_v1(escrow_lamports, recorded_rent, bond)` |
| observed bond | `observedBond` | `observed_bond_v1(lamports, recorded_rent)` |
| exit | `exit?` | `exit_v1(refunds_on_failure, winner, failure_selector) -> Option<FounderBondExitV1>` |
| one redemption's draw | `draw`, `Walk.step` | `FounderBondDrawPlanV1::new(FounderBondDrawObservationV1) / validate_post` |
| outstanding | `Walk.outstanding` | `ordinary_outstanding_v1(market_view, bytes, failure_selector)` |
| the close | (build conjunct) | `FounderBondClosureV1::new(FounderBondClosureObservationV1)` refuses `OrdinaryClaimsOutstanding` on the exhausted arm while claims stand; returns `rent / returned_to_founder / surplus_to_refund_wallet` |

Tests (15, green): the Lean's decided witnesses to the lamport — 4,031,465 at
6,333; 3,273,400 at 5,080; 4,410,800 at 6,960; seat 2,786,520; opener advance
4,287,441; repayment 3,042,496; shortfall 1,244,945; one rung 7,223,297, two
10,415,129 — the cohort-13 table in both orders (`[0, 4031465]` and
`[4031464, 1]`), five partitions of 1.5e9 claims each paying the bond exactly,
honest draws zero, the failure coordinate draws zero, a quantity beyond the
outstanding refuses, the postcondition refuses a lamport astray, and the close
by exactly one exit.

### Founding — `programs/dclutch-claims-sbf/src/founding_v5.rs`

- `ClaimsFoundingSbfErrorV5::FounderBondUnderfunded = 0x5191` (band 5 / 0x180,
  pinned in `pin_refusal_band!`, log line
  `claims founding v5: refused, founder bond underfunded`).
- `authenticate_founder_bond(rent, position_width, position_rent_principal,
  observed_position_lamports, claim_count)`, called from
  `authenticate_escrow_seating` when the record refunds on failure, AFTER the
  existing `Rent` refusal (escrow below its own rent). The rate is derived the
  way the funding ledger derives the rate it records —
  `derive_funded_rent_rate_v2(rent.minimum_balance(0), position_width,
  position_rent_principal)`, so the bond is provably priced at the rate the
  escrow's admission persists. `founding_bond_size_v1(rate, claim_count, 0)`;
  `founded_v1` or refuse.
- Nothing new is WRITTEN: the escrow admission already persists
  `observed_position_lamports` (= rent + bond + dust) and
  `position_rent_principal` (= rent), which is the bond's record.
- The frame did not move (33). The request and receipt wires did not move.

### Terminal settlement — `programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs`, `crates/dclutch-claims/src/terminal_settlement_v3.rs`

- Crate constants: `TERMINAL_SETTLEMENT_FOUNDER_BOND_ACCOUNT_COUNT_V3 = 3`,
  `..._ESCROW_ACCOUNT_V3 = 36` (writable escrow Position),
  `..._ADMISSION_ACCOUNT_V3 = 37` (read-only escrow admission),
  `..._RECIPIENT_ACCOUNT_V3 = 38` (writable recipient),
  `TERMINAL_SETTLEMENT_WITH_FOUNDER_BOND_ACCOUNT_COUNT_V3 = 39`.
- `ClaimsSbfError::FounderBondFrame = 0x5012` (`programs/dclutch-claims-sbf/src/lib.rs`).
- `process`, `execute_enclosing_authenticated`, `execute_claim_check_compaction`
  admit 36 or 39 accounts (`frame_width_admitted`).
- `authenticate_founder_bond_arm` (in `authenticate_and_prepare`, after the
  certificate scenario, with the aggregate bytes borrowed): no tail → `None`
  unless refunding AND `Failure` scenario → `FounderBondFrame`; tail on a
  non-refunding Market → `Accounts`; privileges; escrow/admission are the
  derived pair (`FailureEscrowIdentityV1::derive`, `0x5010` otherwise);
  recipient is DERIVED — `input.recipient_owner`, or under
  `ParentAuthorityV3::ClaimCheckCrank` the claim-check PDA
  `ClaimCheckSeedsV1::new(aggregate, owner)`; recorded rent read off the
  admission (`position_rent_principal`, owner/kind/outcome checked); exit from
  the winner and cross-checked against the scenario; `ordinary_outstanding_v1`
  over the aggregate BEFORE this redemption's debit; `FounderBondDrawPlanV1`.
- `apply_founder_bond_draw` (in `execute`, after Custody paid, before the
  replay digest): checked debit/credit, `validate_post`, `sol_log_64(draw,
  remaining_before, remaining_after, 0, 0)`. The receipt wire and its
  `post_resource_digest` preimage are UNCHANGED (the operator reconstructs the
  receipt byte for byte); the draw's evidence is chain state and the log.

### Compaction — `programs/dclutch-claims-sbf/src/claim_check_compaction_v1.rs`, `crates/dclutch-claims/src/claim_check_conservation_v1.rs`

- `COMPACT_BOND_ESCROW_ACCOUNT_V1 = 42`, `COMPACT_BOND_ADMISSION_ACCOUNT_V1 = 43`,
  `COMPACT_WITH_BOND_ACCOUNT_COUNT_V1 = 44`; `process_compaction` admits 42 or
  44 and, with the pair, wraps the terminal call in a 39-account Vec whose
  tail is `[escrow_position, escrow_admission, claim_check_account]` — the
  sleeper's share lands on the claim-check address.
- `ClaimCheckCompactionObservationV1::founder_bond_draw` (observed as the
  claim-check address's rise across the settlement): excluded from the dust
  that reduces the top-up, so the minted record carries rent + draw and
  redemption sweeps it whole to the holder; a draw with no record to carry it
  refuses `Conservation`. The five other constructors pass 0 (operator planner,
  fractional route, tests).

### Closure — `programs/dclutch-claims-sbf/src/market_closure_v1.rs`

- `ClaimsMarketClosureSbfErrorV1::OrdinaryClaimsOutstanding = 0x5507`, pinned.
- `burn_failure_escrow_column_v1` takes the authenticated `CoreState`;
  `admit_founder_bond_disposition_v1` runs after the residue equality and
  before the burn: recorded rent off the escrow admission (owner/kind/outcome
  checked, `0x5010` otherwise), `exit_v1(true, core.terminal_winner,
  failure_selector)`, `ordinary_outstanding_v1`, `FounderBondClosureV1::new` →
  `OrdinaryClaimsOutstanding` by name; `sol_log_64(exit, rent, returned,
  surplus, 0)`. The pair then surrenders its whole balance to the aggregate as
  before, which is the honest return (rent + bond to the refund source) and
  the exhausted surplus.
- Frames unchanged: 11/12 categorical, 14/15 refunding.

### Lean — `formal/dclutch-semantics/DClutchSemantics/FounderBondV1.lean` (**+57 lines**, not a new file)

**Read this before citing the module.** The file is on `main` already at 536 lines, imported at
`DClutchSemantics.lean:55`, carrying the size rule, the two exits, the pro-rata walk (`draw`, `Walk`,
`an_exhausting_walk_pays_the_bond_exactly` — any partition of the ordinary claims among holders, in any
order, pays the bond to the last lamport with **no divisibility hypothesis**), and the cohort-13 walk
witnesses. Whole file: 39 theorems, **zero `sorry`**. The branch (`3e63410c7`) appends **one section**,
lines 536-590 — the close's admission — and its commit message is accurate: five theorems, zero sorry.

What the branch adds, verbatim:

| line | item |
|---|---|
| — | `def closeAdmits : Exit → Nat → Bool` — `.honest` always; `.exhausted` only when `outstanding = 0`. |
| — | `an_honest_close_is_always_admitted : closeAdmits .honest outstanding = true := rfl` |
| — | `an_exhausted_close_refuses_while_claims_stand (standing : 0 < outstanding) : closeAdmits .exhausted outstanding = false` — **hostile: a close before the walk finishes.** |
| — | `an_exhausted_close_is_admitted_once_the_walk_is_done : closeAdmits .exhausted 0 = true := rfl` |
| — | `an_admitted_exhausted_close_carries_none_of_the_bond (positive : 0 < walk.outstanding) (partition : redemptions.sum = walk.outstanding) : closeAdmits .exhausted (walk.run redemptions).outstanding = true ∧ (walk.run redemptions).remaining = 0` — **the load-bearing one.** What the founder's refund source receives above rent on the exhausted arm is a LATE DONATION, never the bond. This is `FounderBondClosureV1::new`'s twin. |
| `:581` | `a_compacted_redemption_draws_what_the_holder_would_have (remaining outstanding quantity : Nat) : draw remaining outstanding quantity = draw remaining outstanding quantity := rfl` |
| — | `example : closeAdmits .exhausted (cohortFifteenWalk.run [1499999800, 200]).outstanding = true := by decide` |

**One of the five is vacuous.** `a_compacted_redemption_draws_what_the_holder_would_have` (`:581`) is
`X = X`. It states nothing. The fact it wants — that the compaction crank's draw does not depend on who
signed — is a property of the *Rust* route's arguments, not of `draw`, and belongs in
`claim_check_compaction_v1.rs` as an assertion that the crank passes the sleeper's own
`(remaining, outstanding, quantity)`. **So the honest count for this branch is four theorems, not five.**
Delete it or restate it over `redemptionDraw`'s arguments.

**Never elaborated.** No lane on this branch ran `lake`.

### Operator, driver, census, runbook, SDK, web (landed; uncommitted at the wall, committed by CLOSEOUT)

| file | what |
| --- | --- |
| `crates/dclutch-operator/src/wallet_terminal_payout_v3.rs` (+462) | The bond's host-side author. `WalletTerminalPayoutFounderBondRouteV3` (:78) and `…InputV3` (:222) are `Option`s on the route and the input; `founder_bond_plan` (:499) returns `Option<FounderBondDrawPlanV1>` from the kernel; `founder_bond_draw: u64` on the report (:269). The frame width is a two-valued constant: `expected_accounts = if route.founder_bond.is_some() { TERMINAL_SETTLEMENT_WITH_FOUNDER_BOND_ACCOUNT_COUNT_V3 } else { TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 }` (:454). Two new error variants: `WalletTerminalPayoutErrorV3::FounderBondRoute` (:311, the route/input/`refunds_on_failure` triple disagreeing — raised at :516, :973, :988, :995) and `::FounderBond(FounderBondErrorV1)` (:313, the kernel's own refusal passed through). The verifier holds the poststate to `post.founder_bond != expected.founder_bond` (:868). |
| `crates/dclutch-operator/src/wallet_terminal_payout/wire.rs` (+114) | The tail's three metas and the escrow pair's derivation (`ClaimCheckEscrowSeedsV1` / `ClaimCheckVaultSeedsV1` off the aggregate, :381-397). |
| `crates/dclutch-operator/src/wallet_terminal_input.rs` (+32) | The routed frame derives the escrow pair when the basis refunds and adds the bond recipient. |
| `crates/dclutch-market-retirement-v1-operator/src/lib.rs` (+107) | `CheckpointMarketRetirementReportV1` gains `founder_bond_lamports` and `founder_bond_exit`, projected from the escrow prestate — S8, done. |
| `packages/dclutch-sdk/lib/marketDetail.ts` (+239) | `FounderBondExitV1` (:460), `FounderBondV1` (:473); `founderBondV1(input)` (:533) reads the bond as **the escrow's balance above its recorded rent** and returns `null` for *unread*, never for *none*; `failureEscrowAccountsV1` (:446) derives the Position + admission pair from the market's own aggregate, beside the pre-existing `failureEscrowOwnerV1` (:425). `OutageDisclosureV1` gains a **required** `founderBond` (:258) and the headline is now built before `Object.freeze` and appends the bond's sentence (:392-397) — "what an outage PAYS and what it COSTS are one answer" — **so any golden text assertion on `headline` changes**. A categorical market's sentence is returned **without reading an account**. |
| `packages/dclutch-sdk/lib/marketDetail.test.ts` (+224) | Thirteen `it(…)` cases — see §5. |
| `apps/dclutch-web/components/MarketDetailWorkspace.tsx` (+84) | A **fourth RPC round trip per page load** (:493-518, `client.multipleAccounts([position, admission], next.floorSlot)`), held as `FailureEscrowReadingV1 | null`; the bond is derived once at :635 and rendered at :916-918. Passes `refundsOnFailure: null` **on purpose** (:621-641), so the exit sentence never lights up on the page today — see the stub below. |
| `apps/dclutch-web/components/MarketActivity.tsx` (+21) | New `founderBond?: FounderBondV1 | null` prop on both `MarketActivity` (:123, :139) and the exported `MarketActivityView` (:215), folded into `outageDisclosureV1`. Note `MarketActivityView` is referenced only by its own file, so the new prop has **no component-level test** despite the doc comment claiming one. |
| `crates/dclutch-svm-harness/tests/market_retirement_v1_lifecycle.rs` (+317) | Sub-agent 2's harness walk — §5. |
| `programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/claims_founding.rs` (+210) | Sub-agent 1's founding program-test — §5. |
| `tools/gauntlet/journey/src/ledger.rs` (+41) | `founder_bond_lamports` on the `failure-escrow` row; L7 unchanged — S10, done. |
| `tools/local-validator/bootstrap/successor/src/market.rs` (+109) | The founding prefunds the escrow Position at `position_rent + founding_bond_size_v1(rate, claim_count, ladder).bond` — S9, done. |
| `tools/cohort/steps.tsv` | Rows `founder-bond` and `refund-bond-walk` (both `since 18`); the seated row funds the bond and `retire` carries its exits — S11, done (`ab0416bb2`). |

## 2. THE SEAMS *(growing)*

| # | file:line | one-line change |
| --- | --- | --- |
| S1 | `programs/dclutch-claims-sbf/src/lib.rs` `ClaimsSbfError` | `FounderBondFrame = 0x5012` — done on branch; the reference (`docs/reference/refusals.md`) regenerates through `tools/gate reference --converge` |
| S2 | `programs/dclutch-claims-sbf/src/founding_v5.rs` `ClaimsFoundingSbfErrorV5` | `FounderBondUnderfunded = 0x5191` — done; reference regen |
| S3 | `programs/dclutch-claims-sbf/src/market_closure_v1.rs` `ClaimsMarketClosureSbfErrorV1` | `OrdinaryClaimsOutstanding = 0x5507` — done; reference regen |
| S4 | `tools/gates/frames-baseline.json` | the Claims link's frame rows move (`authenticate_founder_bond_arm`, `apply_founder_bond_draw`, `admit_founder_bond_disposition_v1`, `authenticate_founder_bond` are new `#[inline(never)]` symbols); capture with `tools/gate frames --at <commit> --capture` on the convergence commit |
| S5 | `crates/dclutch-refusal-registry` census (`tools/gate census`) | three new codes inside band 5, no band change; `--check-unique` must pass |
| S6 | `crates/dclutch-operator/src/wallet_terminal_payout_v3.rs:1241` | **DONE**: the three tail metas append on a refunding Market; the frame width is `TERMINAL_SETTLEMENT_WITH_FOUNDER_BOND_ACCOUNT_COUNT_V3` vs `TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3` at `:454`. |
| S7 | `crates/dclutch-operator/src/wallet_terminal_input.rs` routed frame | **DONE**: the escrow pair is derived when the basis refunds and the bond recipient is added. |
| S8 | `crates/dclutch-market-retirement-v1-operator/src/lib.rs:255 CheckpointMarketRetirementReportV1` | **DONE**: `founder_bond_lamports` / `founder_bond_exit` projected from the escrow prestate. |
| S9 | `tools/local-validator/bootstrap/successor/src/market.rs:12436 prefund_founding_accounts_v1` and `:13576 authenticate_founding_prefunding_v1` | **DONE**: the escrow Position is funded at `position_rent + founding_bond_size_v1(rate, claim_count, ladder).bond`. |
| S10 | `tools/gauntlet/journey/src/ledger.rs` observe | **DONE**: `founder_bond_lamports` on the `failure-escrow` row; L7 unchanged. |
| S11 | `tools/cohort/steps.tsv` | **DONE** (`ab0416bb2`): rows `founder-bond` and `refund-bond-walk` (both since 18); the seated row funds the bond and `retire` carries its exits. Prices are `?` — the first loopback run sets them. |
| S12 | `packages/dclutch-sdk/lib/marketDetail.ts`, `apps/dclutch-web/components/{MarketActivity,MarketDetailWorkspace}.tsx` | **DONE**: `founderBondV1`, `failureEscrowAccountsV1`, the bond's sentence folded into `outageDisclosureV1`'s headline, the page's one escrow-pair read. |
| S13 | `programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/claims_founding.rs` | **DONE** (sub-agent 1): `world` funds the escrow at `position_rent + bond`; two new hostiles `FounderBondOneLamportShort` and `FounderBondAbsent`. |
| S14 | `crates/dclutch-svm-harness/tests/market_retirement_v1_lifecycle.rs:1462 seed_refunding_failure_escrow_v1` | **DONE** (sub-agent 2): real principals recorded, the bond seeded, and the refund wallet asserted to rise by rent + bond. |
| S15 | the failure-arm family's `CommitDeadlineFailure` frame / `refund-*` rows | the bond's exhausted exit is the terminal payout's own tail; the failure arm need not build a second draw — coordinate by naming `TERMINAL_SETTLEMENT_FOUNDER_BOND_*` |
| S16 | `docs/reference/routes.md`, `refusals.md`, `tools/gauntlet/CU_BUDGETS.md` | regenerate; budgets for `found` (+~500 CU provisional), payout (+2,000–5,000 provisional), closure (+~1,500 provisional) once measured |

### S17–S24 — the downstream call sites the operator lane deliberately did NOT edit

Threading `founder_bond: Option<…>` through `WalletTerminalPayoutRouteV3`/`InputV3` and
`founder_bond_lamports`/`founder_bond_exit` through `CheckpointMarketRetirementReportV1` makes every
existing constructor of those structs incomplete. The lane that added the fields listed the sites and
left them; each is a literal field addition, no logic:

| # | file:line | one-line change |
| --- | --- | --- |
| S17 | `tools/local-validator/bootstrap/successor/src/wallet_terminal_payout_exterior.rs:1532` | add `founder_bond: None,` |
| S18 | `tools/local-validator/bootstrap/successor/src/aggregate_retirement_exterior.rs:1467` | add `founder_bond_lamports: 0, founder_bond_exit: None,` |
| S19 | `tools/local-validator/bootstrap/successor/src/aggregate_retirement_journal.rs:1887` | add `founder_bond_lamports: 0, founder_bond_exit: None,` |
| S20 | `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs:7837` | add `founder_bond: None,` |
| S21 | `programs/dclutch-claims-sbf/tests/rational_representation_v2_program_test.rs:7843` | add `founder_bond: None,` |
| S22 | `crates/dclutch-operator/src/claim_check_v1.rs:274` | `CLAIM_CHECK_COMPACTION_ACCOUNT_COUNT_V1` is measured off the **36**-account frame, so a refunding Market's compaction builds 45 metas and refuses `Binding` at `:545`. Fail-closed, and a **named stub** — see §4. The fix is a second constant, not a widened one. |
| S23 | `crates/dclutch-operator/src/wallet_terminal_payout/wire.rs:1216` | `ManifestRouteV3` does not carry the tail keys, so a browser reading the manifest cannot see the escrow pair; add them beside the existing route keys. |
| S24 | `apps/dclutch-web/lib/generated/capabilitySurfaceV1.ts` | regenerate with `npm run abi:capability-surface` — `marketDetail.ts` now imports the generated direct-participant module and the committed surface predates it. |
| S25 | `apps/dclutch-web/components/MarketDetailWorkspace.tsx` | `refundsOnFailure` still has **no producer** on the market page; it is passed `null` on purpose (§4), which is why the page can state the bond's AMOUNT but never its EXIT. |

**S17–S21 are compile breaks, not cosmetics.** They are why the one green `cargo check` in §7 does not
mean the workspace builds: it checked `dclutch-operator`, and every site above is in a crate downstream
of it.

### S26–S33 — the rest, derived from the diff by CLOSEOUT-B

| # | file:line | the one-line change |
| --- | --- | --- |
| S26 | `tools/cohort/steps.tsv:61` | the `refund-bond-walk` row's invocation column is `-` — the campaign that walks a forced outage does not exist, and there is **no `blocked.json` row** saying it is undrivable. Either write the campaign or add the blocker; an empty column is neither. |
| S27 | `tools/cohort/steps.tsv:60` vs `programs/dclutch-claims-sbf/src/founding_v5.rs:1727` | the new `founder-bond` census row asserts the escrow reads back at `recorded rent + founding_bond_size_v1(rate, claim_count, **ladder**)` while the chain conjunct passes `ladder_funding = 0` (ruling R2). **A laddered refunding market is green on chain and red at this row.** Make the row read the chain's rule, or make R2's host-side Λ the row's input. |
| S28 | `docs/reference/refusals.md` (insert after `:157`, `:195`, `:232`) plus `packages/dclutch-sdk/lib/generated/{refusalRegistryV1,routeCensus}.ts` and `docs/reference/abi/{refusalRegistryV1,routeCensus}.md` | **none of the three new codes is mirrored anywhere.** All five files are generated: `tools/gate reference --converge`, then `npm run abi:refusal-registry`. Note `packages/dclutch-sdk/lib/refusals.test.ts:29` only asserts `> 150`, so it will not catch the omission. Insertion points all fall after existing provenance lines, so no line drift. |
| S29 | `packages/dclutch-sdk/scripts/generate-wallet-terminal-payout-v3.mjs:56, :60-74` | the generator's scalar list is **hard-coded** and does not include `TERMINAL_SETTLEMENT_WITH_FOUNDER_BOND_ACCOUNT_COUNT_V3` or the three tail-coordinate constants, so `packages/dclutch-sdk/lib/generated/walletTerminalPayoutV3.ts:14` still emits only `TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3 = 36`. **The generator must change**, then `npm run abi:wallet-terminal`. A browser cannot build the 39-account frame until it does. |
| S30 | `tools/gates/frames-baseline.json` | the four new `#[inline(never)]` symbols are **absent**: `authenticate_founder_bond` (`founding_v5.rs:1713`), `authenticate_founder_bond_arm` (`terminal_settlement_v3.rs:493`), `apply_founder_bond_draw` (`terminal_settlement_v3.rs:623`), `admit_founder_bond_disposition_v1` (`market_closure_v1.rs:1055`). `tools/gate frames --at <commit> --capture`. |
| S31 | `tools/gauntlet/CU_BUDGETS.json` / `.md:78` | only `dcltgmf3-stage-4-claims-foundingv5` exists; no budget row moved for founding (+ the bond conjunct), payout (+ the draw) or closure (+ the disposition). |
| S32 | `crates/dclutch-claims/src/terminal_settlement_v3.rs:65,67,69,72,74` | the five tail constants: `TERMINAL_SETTLEMENT_FOUNDER_BOND_{ESCROW,ADMISSION,RECIPIENT}` are **hardcoded 36 / 37 / 38** rather than `ACCOUNT_COUNT + n`. If `TERMINAL_SETTLEMENT_ACCOUNT_COUNT_V3` ever moves they desync silently. Make them arithmetic. |
| S33 | `programs/dclutch-claims-sbf/src/lib.rs:349` | doc comment cites `TERMINAL_SETTLEMENT_FOUNDER_BOND_ACCOUNT_COUNT_V3` (the tail *size*, 3) where it means `TERMINAL_SETTLEMENT_WITH_FOUNDER_BOND_ACCOUNT_COUNT_V3` (39). One word. |
| S34 | `docs/decisions/0007-namespaced-refusal-codes.md:129-139` | the Claims sub-band table's three affected rows must extend: `ClaimsSbfError` `0x5000–0x500B` → `0x5012`, `ClaimsFoundingSbfErrorV5` `0x5180–0x5190` → `0x5191`, `ClaimsMarketClosureSbfErrorV1` `0x5500–0x5505` → `0x5507`. **⚠ MERGE COLLISION** — the `build/claims-split-merge` branch adds a **new row** to this same eleven-row table (`ClaimsConservationSbfErrorV1 | — | 0x5300–0x530C`). Four edits, one table, two branches. Land them in one commit; a textual merge will resolve cleanly and still be wrong if either branch's rows are dropped. |

### The dead half of the kernel

`crates/dclutch-claims/src/founder_bond_v1.rs` is 875 lines and **roughly half of its public surface has
zero callers outside the file**. What the programs actually reach: `founding_bond_size_v1`
(`founding_v5.rs:1727`), `founded_v1` (`:1733`), `exit_v1` + `ordinary_outstanding_v1`
(`terminal_settlement_v3.rs:576,597`; `market_closure_v1.rs:1088,1098`),
`FounderBondDrawPlanV1::{new,draw,remaining_before,remaining_after,validate_post}`
(`terminal_settlement_v3.rs:598,641,648,655,656`),
`FounderBondClosureV1::{new,exit,rent,returned_to_founder,surplus_to_refund_wallet}`
(`market_closure_v1.rs:1101,1108,1112-1114`), `observed_bond_v1` (host only,
`market-retirement-v1-operator/src/lib.rs:1489`), and the two enums.

What nothing reaches: `FOUNDER_BOND_CERTIFICATE_SEAT_BYTES_V1` (`:38`),
`FOUNDER_BOND_TOKEN_ACCOUNT_BYTES_V1` (`:43`), `FOUNDER_BOND_POSITION_BYTES_PER_OUTCOME_V1` (`:50`),
`FounderBondWidthsV1` (`:78`), `FOUNDER_BOND_WIDTHS_V1` (`:98`), `rent_for_v1` (`:112`),
`FounderBondSizeInputV1` (`:120`), `seat_prepay_v1` (`:152`), `opener_advance_v1` (`:157`),
`first_crank_swept_v1` (`:164`), `first_crank_repayment_v1` (`:185`), `first_crank_shortfall_v1` (`:198`),
`bond_size_v1` (`:211` — word-exact grep: **zero** call sites; the apparent hits are substring matches on
`founding_bond_size_v1`), `draw_v1` (`:327`), `FounderBondClosureV1::surrendered()` (`:521`, zero callers
**including tests**), and the `seat_prepay` / `first_crank_shortfall` / `ladder_funding` fields of
`FounderBondSizeV1` (`:139` — only `.bond` is ever read).

Most of these are the size rule's decomposition and are legitimately internal — but they are `pub`, so
the census counts them as surface. Either make them `pub(crate)` or give the decomposition a caller (the
`founder-bond` runbook row's verifier is the obvious one: it wants exactly `seat_prepay +
first_crank_shortfall + ladder_funding` to show the reader *why* the number is what it is). This is the
kind of thing the parsimony attractor names, and it is cheap to settle now.

### The un-wired compaction arm

The program's 44-account compaction arm exists and **nothing in the tree can build it**:
`programs/dclutch-claims-sbf/src/claim_check_compaction_v1.rs:599, :601, :609`
(`COMPACT_BOND_ESCROW_ACCOUNT_V1`, `COMPACT_BOND_ADMISSION_ACCOUNT_V1`,
`COMPACT_WITH_BOND_ACCOUNT_COUNT_V1`) have zero consumers outside their own file, and the arm at `:630` /
`:661` is unreachable. The host side is S22. This is a producer-missing pattern in miniature: the route,
the constants and the refusal exist; the builder does not.

### Frames still pinned at 36 that a 39-account terminal can reach

`terminal_settlement_v3.rs:181`'s `execute_enclosing_authenticated` now admits 36 **or** 39 via
`frame_width_admitted` (`:138`) — but **no Trading composer builds 39**. The widths still pinned at 36:
`crates/dclutch-claims/src/fractional_claim_check_v1.rs:782`
(`FRACTIONAL_COMPACT_TERMINAL_FRAME_V1`), `:841` (`FRACTIONAL_COMPACT_ACCOUNT_COUNT_V1 = 50`, fixed),
`crates/dclutch-claims/src/fractional/hot_v2.rs:732`,
`crates/dclutch-operator/src/fractional/selected_release_v4.rs:1551`,
`programs/dclutch-trading-sbf/src/claims_composition_v3.rs:2754`,
`programs/dclutch-trading-sbf/src/hot_v3/children.rs:74`. The branch **asserts** at
`programs/dclutch-claims-sbf/src/fractional_claim_check_v1.rs:1498` that a Fractional reserve carries no
bond, and **no test verifies that assertion**. If it is wrong, six pinned widths are wrong with it.

### Already-wired points a convergence lane must re-check after rebase

New **required** struct fields — every literal construction on `main` breaks:

- `crates/dclutch-claims/src/claim_check_conservation_v1.rs:113` `founder_bond_draw: u64` — set today at `claim_check_conservation_v1.rs:607`, `fractional_claim_check_conservation_v1.rs:518`, `operator/src/claim_check_v1.rs:497`, `claim_check_compaction_v1.rs:933`, `fractional_claim_check_v1.rs:1498`.
- `wallet_terminal_payout_v3.rs:153, :218, :266, :269, :657, :682` — five fields; patched at `:1510, :1700, :1900, :1941`, `wire.rs:1522-1524, :1847, :1968-1969`.
- `market-retirement-v1-operator/src/lib.rs:304, :312` — two report fields; `:224-232` `FailureEscrowStateV1::Seated` gains `recorded_rent`, `founder_bond`, `failure_selector`.
- `tools/local-validator/bootstrap/successor/src/market.rs:10369` `FoundingOuterV1.founder_bond_lamports`; `:12529` prefunding and `:13671` its authentication now expect `position_rent + bond` — **this is the host side of `0x5191`, and any other founding driver that prefunds an escrow will refuse on chain.**

Wire and baseline compatibility:

- `wire.rs:164-172` — three new optional `PlanInputV1` JSON fields (`founderBondEscrowPosition/Admission/Recipient`) with `skip_serializing_if`: wire-compatible with cohort-16/17 plan JSON, but golden vectors that round-trip `PlanInputV1` must be re-checked.
- `tools/gauntlet/journey/src/ledger.rs:383` — new `ObservationV1.founder_bond_lamports` with serde `default` (old journals reload); `:1317` changes the **L7 verdict message text**, which any journal-diff baseline will see.
- `packages/dclutch-sdk/lib/marketDetail.ts:258` — `OutageDisclosureV1.founderBond` is **required**, and the headline text changed (`:392-397`).

**No Cargo or package.json change is owed** — `git diff main...HEAD -- '*Cargo.toml' '*package.json' '*.lock'`
is empty and every dependency edge already existed. **No referenced-but-undefined identifier exists** on
the branch; every symbol resolves in-tree.

## 3. Provisional rulings

**R1 — the rate's source when the founding's rate record is absent.** The
Claims founding is a CREATING site under decision 0030: it reads the sysvar,
prices the bond at that rate, and RECORDS what it paid in the escrow
admission's `position_rent_principal` in the same instruction — so on a
seated escrow the rate record is never absent. Every later reader (the payout
draw, the closure, the census, the page) reads the recorded principal off the
admission and, when it needs the rate itself, recovers it by
`funded_rent_rate_from_minimum_v1(principal, position_width)` — exact division
or refuse, 0030's own recovery shape. A Market whose admission is gone has no
escrow and no bond to read. The founding's rate is proved consistent with the
record by deriving it from two readings (`derive_funded_rent_rate_v2` over the
zero-length minimum and the Position's own principal): a cluster whose rent is
not affine refuses `Rent` at founding rather than recording a rate that prices
some later width wrong.

**R2 — the ladder term at the chain's conjunct.** The recovery policy and the
capability manifest are not in the Claims founding frame; the manifest lives
in Core's Found window and reaching it from Claims is a Trading-composer and
driver frame change plus a rung-kind identity the RECOVERY family has not yet
authored (`market.rs:2293`). Decision 0033 §6 leaves Λ's netting open. So:
the chain enforces `rent + S + F ≤ lamports` (Λ = 0 at the conjunct), the host
funds `rent + S + F + Λ` with Λ read off the policy's rung allocations the way
`authenticate_ladder_entry_adjacency_v1` already finds them, and the census
reads the escrow back against the FULL rule. `a_rung_never_lowers_the_bond`
makes the chain's floor sound. Lifting plan: one trailing read-only manifest
account on the Claims founding frame plus `ladder_funding_v1(manifest,
rung_kind)` once rung kinds exist — labelled provisional, chain-derived floor.

**R3 — a categorical founding posts no bond.** The bond's account is the
escrow's own (0033 §4), a categorical founding seats none, and its shape (the
outage pays whoever holds the failure column) is the legacy shape decision
0025 §5 keeps deliberately for byte-identity; the runbook has founded nothing
but refunding records since cohort-16 (`refund-scale`). Mandatory binds every
Market that has an escrow to hold the bond in. Reversal is one conjunct:
`refunds_on_failure == false` refuses a new discriminant at founding.

**R4 — the honest exit fires at closure, not at Terminal.** The note wanted
the return at Terminal; PROGRAMS-17E's shape A closes the escrow only inside
the closure burn (the ordering is forced by `protocol_position_v2`'s close),
and the brief rules the bond arm INSIDE that route. The founder's capital is
held until retirement's prepare packet. A Terminal-time return would be a
second route moving lamports out of a live Position; not built.

**R5 — the receipt wires do not move.** The draw and the disposition are
evidenced by chain state (escrow and recipient lamports, the admission's
recorded principal, the winner) and by `sol_log_64` lines; neither the
terminal receipt nor the closure receipt gains a field, because the operator
reconstructs both byte for byte and cohort-16/17 receipts must stay readable.
The host-side classification lives in the operator's reports (S8).

**R6 — the compaction recipient is the claim-check address.** A sleeper's
draw lands on the vacant claim-check address before the record is minted,
`founder_bond_draw` keeps it out of the sweep's dust, and redemption pays the
record whole to the holder. Crediting `recipient_owner` (the claim-check
escrow PDA) would have paid the escrow's closer.

## 4. Stubs (each with its reason)

No `todo!()` and no `unimplemented!()` anywhere on the branch. Three things are
deliberately not built, and each fails closed rather than silently:

1. **The compaction planner's bond draw.** `crates/dclutch-operator/src/claim_check_v1.rs:482`
   passes `founder_bond_draw: 0` and builds the 42-account frame; `:274`'s
   `CLAIM_CHECK_COMPACTION_ACCOUNT_COUNT_V1` is measured off the 36-account
   frame, so on a refunding Market the planner builds 45 metas and the route
   refuses `Binding` at `:545`. A sleeper's compaction on an EXHAUSTED
   refunding Market is therefore impossible, not wrong. Reason: the planner
   needs the escrow pair and the outstanding count at plan time, which is the
   same read `wallet_terminal_payout_v3` does — one function, not one design.
   Named in a doc comment at the site (S22).
2. **`refundsOnFailure` on the market page.** `MarketDetailWorkspace.tsx` passes
   `null` on purpose and the file says why: the page has no producer for that
   bit, and `founderBondV1` reports the amount with `exit: null` rather than
   guessing. The SDK test `states the amount but never the exit when the payout
   scale was not read` pins that this stays deliberate (S25).
3. **`a_compacted_redemption_draws_what_the_holder_would_have`** (`FounderBondV1.lean:581`) is a vacuous
   `rfl` on `draw r o q = draw r o q` — one of the five theorems this branch adds. It is not a stub the
   maker labelled; it is a theorem that looks like a proof and is not. Named here so it is deleted rather
   than cited.
4. **The ladder term Λ is hardcoded zero at the chain conjunct** —
   `programs/dclutch-claims-sbf/src/founding_v5.rs:1727` passes `ladder_funding = 0` to
   `founding_bond_size_v1`. That is ruling R2 and it is sound as a floor, but see S27: the new
   `founder-bond` runbook row asserts the escrow reads back at `rent + founding_bond_size_v1(rate, claims,
   **ladder**)`, so **a laddered refunding market is green on chain and red at that census row.**
5. **`refund-bond-walk` has `-` in its invocation column** (`tools/cohort/steps.tsv:61`): the campaign that
   walks a forced outage does not exist, and there is no `blocked.json` row saying so.

## 5. Tests written and what each proves

**Native kernel** — `crates/dclutch-claims/src/founder_bond_v1.rs`, `#[cfg(test)]` at `:541`, fifteen
tests, run green by the maker:

| line | test | proves |
|---|---|---|
| `:569` | `the_widths_are_the_trees_own` | every width in `FOUNDER_BOND_WIDTHS_V1` is the tree's own constant, never retyped — the module refuses to be a second author. |
| `:588` | `the_position_width_is_the_trees_own` | Position header + per-outcome match `liability_basis_state_v2`. |
| `:605` | `the_size_rule_reproduces_the_lean_witnesses` | `bond_size_v1` at 6,333 / 5,080 / 6,960 equals the Lean's decided figures, 4,031,465 included. **This is the Rust↔Lean join.** |
| `:635` | `a_rung_never_lowers_the_bond` | the ladder term is monotone (R2's floor). |
| `:649` | `a_founding_one_lamport_short_refuses` | `founded_v1(rent+bond-1, …) == false`. |
| `:659` | `a_zero_rate_refuses_by_name` | rate 0 → `FounderBondErrorV1::Rent`. |
| `:668` | `the_exit_is_a_function_of_the_winner` | `exit_v1` honest / exhausted / `None`. |
| `:705` | `the_cohort_thirteen_table_walks_exactly_in_both_orders` | `[200, 1499999800]` → `[0, 4031465]`, reversed → `[4031464, 1]`, both summing to the bond — the rounding boundary against a number the chain actually produced. |
| `:717` | `any_partition_of_the_outstanding_claims_pays_the_bond_exactly` | the telescoping identity over arbitrary partitions, nothing left over. |
| `:740` | `no_redemption_draws_the_bond_on_an_honest_terminal` | honest exit draws 0. |
| `:748` | `the_failure_coordinate_draws_nothing` | `claim_index == failure_selector` → 0. |
| `:767` | `a_redemption_beyond_the_outstanding_claims_refuses` | `ExceedsOutstanding`. |
| `:784` | `the_postcondition_refuses_a_lamport_astray` | `validate_post` rejects off-by-one balances. |
| `:811` | `the_closure_disposes_of_the_bond_by_exactly_one_exit` | honest → founder, exhausted → surplus, `OrdinaryClaimsOutstanding` while claims stand. |
| `:848` | `the_outstanding_claims_are_read_off_the_aggregate` | `ordinary_outstanding_v1` sums every coordinate but the selector. |

**Operator** — `crates/dclutch-operator/src/wallet_terminal_payout_v3.rs`, two new tests plus four
fixture helpers (`:1421 fixture_with_scale`, `:1970 escrow_admission_bytes`, `:2013 refunding_fixture`,
`:2035 refunding_input`):

| line | test | proves |
|---|---|---|
| `:2052` | `a_refunding_market_missing_its_bond_tail_is_refused_by_name` | three ways route / input / `refunds_on_failure` can disagree, all → `FounderBondRoute`. It does not build a 36-account frame and hope. |
| `:2107` | `the_bond_tail_is_appended_in_order_and_draws_the_kernel_share` | 39 metas; tail order `[new(escrow), readonly(admission), new(recipient)]`; **the first 36 byte-identical to the frame that shipped** — and it proves that by stripping the tail from *the same route* (`without_tail.founder_bond = None; payout_accounts(without_tail, …)`) rather than building a separate categorical fixture, so the comparison cannot drift; the draw recomputed from the kernel, not the operator's arithmetic; and `founder_bond: None` in the poststate → `Postcondition`. |

**Founding program-test on the real ELF** (sub-agent 1) —
`programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/claims_founding.rs`:

| line | test | proves |
|---|---|---|
| `:623` | (the fixture) | the bond is derived by `derive_funded_rent_rate_v2` + `founding_bond_size_v1` **in the fixture** rather than typed, with the reason at the site: a fixture that typed the bond as a literal would pass whatever the route asked for. New hostiles `FounderBondOneLamportShort` (`:273`) and `FounderBondAbsent` (`:276`); `FoundingWorld` gains `escrow_position_rent` / `bond` (`:304-305`). |
| `~:1083` | `a_refunding_founding_seats_the_escrow_on_the_real_elf` (amended) | **the bond as the difference of two fields of one account**: escrow lamports `== escrow_position_rent + bond`, while the admission's `position_rent_principal == escrow_position_rent` and `observed_position_lamports == escrow_position_rent + bond`. Nothing on chain stores the bond; this is where the branch establishes that the difference IS the bond. |
| `:1294` | `a_founding_one_lamport_short_of_the_bond_refuses_by_name` | the ELF refuses `0x5191` **and** the validator log carries `"claims founding v5: refused, founder bond underfunded"`. |
| `:1328` | `an_escrow_holding_its_rent_but_no_bond_refuses_the_bond_not_the_rent` | two worlds: escrow at exactly rent → `FounderBondUnderfunded`; escrow at zero → `Rent`; and **asserts the two discriminants differ**. This is the test that stops `0x5191` from becoming a second name for a rent refusal. |

**Retirement harness** (sub-agent 2) — `crates/dclutch-svm-harness/tests/market_retirement_v1_lifecycle.rs`:

| line | test | proves |
|---|---|---|
| `:1476` | `seed_refunding_failure_escrow_v1` (rewritten) | now returns `(escrow, rent, bond)`; seeds the Position at `position_rent + founding_bond_size_v1(funded_rent_rate(position_width), RETIREMENT_CLAIM_COUNT, 0).bond`, **records real principals in the admission (was `1/1/1/1`)** and reads the balances back off the bank. The reason is at the site: the closure reads the bond as `lamports - position_rent_principal`, so a fixture that recorded `1` would call the whole balance the bond. `assert!(bond > 0, "a mandatory bond that is zero proves nothing")` is the file's positive control. |
| `:1756` | `a_refunding_market_retires_once_the_closure_burns_its_failure_column` (amended) | **the bond returns with the rent**: `plan.failure_escrow_rent_lamports == escrow_rent + bond` (`:1774`), the checkpoint rises by `escrow_rent + bond` (`:1854`), `plan.expected_refund_delta > escrow_rent + bond` (`:1928`). |
| `:2473` | `an_exhausted_close_with_an_ordinary_claim_standing_refuses_by_name` | one ordinary claim standing + exhausted winner → `0x5507`; **and the byte-identical prestate with an ordinary winner → `Liability`**, the positive control proving the escrow identity, the residue equality and the bond arm all passed before the refusal. Helper `:2400 set_terminal_winner_v1` moves the winner in BOTH the CoreState and the Source closure receipt. |

**SDK** — `packages/dclutch-sdk/lib/marketDetail.test.ts`, `describe('founderBondV1')` at `:463`, thirteen
cases. The three that carry the design: `:507` *reads cohort-15's bond as the escrow's balance above its
recorded rent* (the same difference the program-test asserts, from the other side of the wire); `:575`
*refuses rather than calling an unread escrow a founder who staked nothing* (absent ≠ zero — the failure
the whole null-vs-zero discipline exists to prevent); `:554` *says a categorical market posted no bond,
**without reading an account*** (R3 made observable). The rest: `:500` reads the admission at the kernel's
offsets · `:518` names the honest exit · `:531` names the exhausted exit and the pro-rata sentence · `:542`
reports a partly drawn bond as the difference it is · `:600` an escrow at its own rent reports `'0'` as a
read fact · `:617` states the amount but never the exit when the payout scale was not read (stub 2) ·
`:625` refuses an impossible failure outcome · `:631` carries the bond into the outage headline · `:655`
derives the escrow pair from the market's own aggregate.

### Tests that do NOT exist — the coverage the family is missing

- **Nothing exercises the 39-account terminal frame or `0x5012` on a real ELF or in-process harness.**
  `claims_founding.rs` is the only program-test that mentions the bond at all;
  `authenticate_founder_bond_arm` and `apply_founder_bond_draw` are proved by the operator's unit test
  and by nothing that runs the program.
- **Nothing exercises the SBF compaction bond arm** (`COMPACT_WITH_BOND_ACCOUNT_COUNT_V1`, the
  `claim_check_before` delta at `claim_check_compaction_v1.rs:689`).
- **Nothing watches a real escrow drain to zero across two redemptions.** The exhausted walk is proved in
  Lean, computed in the kernel's `:705`/`:717`, and observed nowhere. That is `refund-bond-walk`'s job
  (S26), and its invocation column is `-`.
- No component test for the new `founderBond` prop on `MarketActivityView` / `MarketDetailWorkspace`.
- No test for `ledger.rs`'s `founder_bond_lamports` or the successor's `ladder_funding_v1` (the
  successor's own unit test at `market.rs:17333` sets `founder_bond_lamports: 0`).

## 6. Frames, codes, cohort

Codes: `0x5191`, `0x5012`, `0x5507` — band 5, no new band. Frames that move:
the Claims link (four new `#[inline(never)]` stages). Program frames: founding
33 (unchanged); terminal settlement 36 | 39; compaction 42 | 44; closure
11/12 | 14/15 (unchanged). Cohort: **18** — the Claims ELF moves, so a
re-release and a re-found under decision 0012; cohort-17 is founded and
cannot carry it.

## 7. Does it parse? (CLOSEOUT-B, 2026-09-06)

`cargo check -p dclutch-operator --offline` on `03f4c108b`: **GREEN**, `Finished dev profile in 29.58s`,
zero errors. That covers the largest block of work in flight at the wall — the +462 in
`wallet_terminal_payout_v3.rs`, the +114 in `wallet_terminal_payout/wire.rs`, the +32 in
`wallet_terminal_input.rs` and everything they pull in (`dclutch-claims`, `dclutch-claims-sbf`,
`dclutch-trading-sbf`). The maker had `-p dclutch-claims -p dclutch-claims-sbf -p dclutch-operator` green
at every earlier commit.

**Green here does not mean the workspace builds.** The check stops at `dclutch-operator`; S17–S21 are five
literal field additions in crates *downstream* of it (the successor bootstrap, the Claims program-tests),
and each is a hard compile break the moment they are built. Expect them.

Four warnings the branch owns, in `wallet_terminal_payout/wire.rs`:

- `:74` unused import `StateBumpsV1`.
- `:402` **unreachable pattern** — `RecipientRouteV1::Wallet | RecipientRouteV1::ClaimCheckEscrow => {}`
  sits below a `RecipientRouteV1::ClaimCheckEscrow => { … }` arm at `:381` that matches all of it. Harmless
  today (the trailing arm is the no-op), but it is the shape a route acquires when a new arm is inserted
  above a catch-all and the catch-all is not re-read. Delete the dead alternative.
- `:1193` and `:1229` unused variable `rent` — two functions whose rent parameter the bond's rewrite made
  dead. The same shape the deploy-lifecycle branch left in `core_effect.rs`; drop the parameters and their
  arguments, or say why they stay.

Not checked here (one check per family is the closeout budget): the SDK (`npm test` over
`marketDetail.test.ts`), the web build, `dclutch-svm-harness`, the Claims program-test, and the Lean —
**`FounderBondV1.lean` has never been elaborated by `lake`**, which matters more than usual because its
theorems are cited by name in §1 and one of them is vacuous (§4.3).

### Rebase hazard — `main` has moved past the base

The branch's base is `f7c03e845`; `main` is at `19d5d2060` and still advancing. Of the nine files `main`
changed, **one overlaps**: `tools/cohort/steps.tsv`, where `main` rewrote the `retire` row's verifier —
and this branch amends `retire` too (`ab0416bb2`). **Expect a conflict on that exact row**, and note that
`build/series` and `build/claims-split-merge` also insert rows into this file. Three branches, one TSV.
Everything else the branch touches is untouched on `main`.

Also relevant: `main` changed `aggregate_retirement_exterior.rs` and `aggregate_retirement_journal.rs` —
the two files S18 and S19 say need `founder_bond_lamports: 0, founder_bond_exit: None`. Re-derive those
two line numbers after the rebase rather than trusting `:1467` and `:1887`.

## 8. STOPPED HERE — the next three steps, concretely

1. **Close S17–S21 and build the workspace.** Five literal field additions —
   `wallet_terminal_payout_exterior.rs:1532` (`founder_bond: None`),
   `aggregate_retirement_exterior.rs:1467` and `aggregate_retirement_journal.rs:1887`
   (`founder_bond_lamports: 0, founder_bond_exit: None`),
   `rational_representation_v2_program_test.rs:7837` and `:7843` (`founder_bond: None`) — then
   `cargo check --workspace --tests`. Until this runs, the two sub-agent tests (the founding program-test
   and the retirement harness walk) that carry the branch's whole empirical claim **have not been
   compiled, let alone run**. They are the point of the family: one asserts the bond as the difference of
   two fields of one account on the real ELF, the other that the difference comes back with the rent.

2. **Elaborate the Lean, and delete the vacuous theorem.** `lake env lean FounderBondV1.lean` — the
   branch's five appended theorems have never been elaborated, and they are the close's admission, the arm
   `market_closure_v1.rs` implements. Then delete
   `a_compacted_redemption_draws_what_the_holder_would_have` (`:581`) and either restate it over
   `redemptionDraw`'s arguments or move the fact to `claim_check_compaction_v1.rs` as an assertion that the
   crank passes the sleeper's own triple. A theorem that reads as a proof and proves nothing is worse than
   the gap it papers over.

3. **Run the bond's own walk once.** Nothing on any tier has watched a real escrow drain to zero across
   two redemptions; `an_exhausting_walk_pays_the_bond_exactly` is proved and computed and never observed.
   The `refund-bond-walk` runbook row (S11, since 18) is that observation, and cohort-18 is where it lives
   because the Claims ELF moves. Its `?` price gets set by the first loopback run. Do this before S22 (the
   compaction planner's 45-meta `Binding` refusal) — the walk is what tells you whether the planner's
   missing draw is a gap or a wrong number.

Then the mechanical remainder, in cost order: S28 (`tools/gate reference --converge` + `npm run
abi:refusal-registry` — **all three codes are currently mirrored nowhere**), S29 (the wallet-terminal
generator's hard-coded scalar list, without which no browser can build the 39-account frame), S30
(`tools/gate frames --capture` for the four new symbols), S31 (CU budgets), S24 (`npm run
abi:capability-surface`), S23 (`ManifestRouteV3` tail keys), S32/S33 (the hardcoded 36/37/38 and the
one-word doc-comment slip).

Two more that are cheap now and expensive later: **S27** — the `founder-bond` census row and the chain
conjunct disagree about the ladder term, so a laddered refunding market is green on chain and red at the
row; and **the dead half of the kernel** (§2's "The dead half") — roughly half of `founder_bond_v1.rs`'s
public surface has no caller, and the `founder-bond` verifier is the natural one to give the size rule's
decomposition, so a reader sees *why* the number is what it is.

S25 — a producer for `refundsOnFailure` on the market page — is the one that turns the page's "the founder
staked X" into "the founder staked X and it went *there*".
