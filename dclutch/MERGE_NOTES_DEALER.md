# MERGE_NOTES — DEALER (the scoring-rule Dealer)

Lane CONVERGE-DEALER, 2026-09-06, branch `build/dealer`, rebased onto `main`
(clean rebase, no conflicts; the three build-wave commits replayed unchanged
onto `dfcf02069`). The maker's handoff is `BUILD_DEALER.md`, left as written
with a dated addendum pointing here. Everything below was run.

## 1. READY or NOT

**READY**, with the handovers of §2 and the named absences of §6.

Green, all run in a target directory private to this family
(`CARGO_TARGET_DIR` under this lane's scratch, never a shared one):

| command | result |
|---|---|
| `cargo check -p dclutch-trading-sbf --offline` | green, zero warnings in this family's files |
| `cargo check -p dclutch-trading -p dclutch-trading-sbf -p dclutch-accelerator-sbf --tests` | green |
| `cargo check -p dclutch-accelerator-dealer-program-test -p dclutch-accelerator-program-test --tests` | green |
| `cargo test -p dclutch-trading --lib` | 626 passed (30 in `scoring_rule`) |
| `cargo test -p dclutch-trading --test scoring_rule__generator_fresh` | 1 passed |
| `cargo test -p dclutch-trading-sbf --lib` | 407 passed (11 in `scoring_dealer_v1`) |
| `cargo test -p dclutch-accelerator-sbf --lib` | 16 passed |
| `lake build DClutchSemantics.ScoringRuleAbiV1` | green |
| `npm run abi:scoring-rule:verify` (dclutch-sdk) | exit 0 |
| `npm run abi:coverage` (dclutch-sdk) | every generated module has a verifier |
| `cargo check -p dclutch-local-successor-bootstrap --offline` | green, no warning from either edited file |
| `cargo test -p dclutch-local-successor-bootstrap --bin … scoring_dealer` | 10 passed |
| `tools/gate census` | **PASS** (was red — see §3) |
| `tools/gate budgets` | PASS |
| `tools/gate selftest` | PASS |
| `tools/gate journey` | PASS (328s) — every `tools/` package compiles, the driver included |
| `tools/gate locks` | PASS |
| `tools/gate fmt` | this family's files clean; see below |
| `tools/gate seam` | this family's findings cleared or handed on; see §2.7 and §4.5 |

Red, and whose:

- `tools/gate emission` — `tools/gates/emission-coverage.md` is stale by exactly
  this family's four rows. SHARED generated file (§2.1); left alone deliberately.
- `tools/gate fmt` — two files, `crates/dclutch-operator/src/lib.rs` and
  `tools/local-validator/bootstrap/successor/src/market.rs`. Neither appears in
  `git diff main...HEAD --name-only` for this branch: both are main's, and this
  lane does not format another lane's files.
- `tools/gate citations` — one register row names the wrong documents.
  Pre-existing; this branch adds no citation.
- `tools/gate seam` — two `DOMAIN_RAW_RESTATEMENT` on
  `tools/gauntlet/journey/src/provider.rs` and one `GONE` row on
  `tools/gauntlet/journey/src/resolution.rs`, all main's, plus the three
  positive `UNSET_GUARD_PRESENT` records this family owes (§2.7).
- `tools/gate commands` — NOT RUN: eleven commands in `docs/guides/two-clients.md`
  and `docs/operators/author-a-ticket.md` name the `dclutch` CLI, which is
  neither built nor on PATH. Pre-existing and unrelated; this branch publishes
  no doc command.

Not run and therefore not claimed: any SBF build, any program test on a real
ELF, any CU measurement, any validator, any devnet. See §6.1.

## 2. SHARED FILES — for the merge lane

### 2.1 `tools/gates/emission-coverage.md` (also `build/general-lifecycle`)

Run `tools/gate emission --write`. The diff this family owes is six lines: two
counters (`91 generated files from 85 emitters` → `93 … 87`; `of the 66 guards
over Rust emissions, 65 do` → `67 … 66`) and four rows —

- guard `crates/dclutch-trading/tests/scoring_rule__generator_fresh.rs` |
  cargo-test | yes | `EmitScoringRuleV1Rust.lean`
- guard `packages/dclutch-sdk: lean-emit EmitScoringRuleV1Ts.lean` | lean-emit |
  n/a | `EmitScoringRuleV1Ts.lean`
- generated `crates/dclutch-trading/src/scoring_rule/generated_scoring_rule.rs`
  | `EmitScoringRuleV1Rust.lean`
- generated `packages/dclutch-sdk/lib/generated/scoringRuleV1.ts` |
  `EmitScoringRuleV1Ts.lean`

Verified by regenerating here, reading the diff, and reverting.

### 2.2 `tools/local-validator/bootstrap/successor/src/main.rs` (also `build/{failure-arm,recovery-capture,series,claims-split-merge}`)

**Applied here anyway, and flagged**: three additive one-liners (`mod
scoring_dealer;`, four dispatch arms, one `usage()` line). Every sibling family
adds the same three kinds of line to the same three places, so a textual
conflict is certain and the resolution is to keep every family's. Leaving them
out would have meant shipping a 2,500-line driver module nothing compiles,
which is the defect this convergence exists to end.

### 2.3 `tools/cohort/steps.tsv` and `tools/cohort/README.md` (also `build/{failure-arm,series,claims-split-merge,founder-bond}`; README also `build/{failure-arm,claims-split-merge}`)

NOT applied. The four rows are in §9, verbatim, with their README headings.
They go between `activate-general` (line 65) and `admissions` (line 70), in the
order `dealer-found → dealer-quote → dealer-fill → dealer-withdraw`, each
`blocks`-ing the next: `tools/cohort/check-steps.py` refuses a `blocks` edge
pointing at a row that does not come later, and refuses any row without a
`### <key>` heading at column 0 in the README. They need a `markets[].dealer`
block in `tools/cohort/cohorts/18.json`, which does not exist (only 14–17 do).

**`check-steps.py` validates the driver KIND and never the verb**, so a
misspelled verb passes every gate and fails at 3am as `unknown command:` from
`main.rs`. Cross-check the four by hand against the driver's `usage()`.

### 2.4 `tools/gates/frames-baseline.json`

Owed and NOT captured: `tools/gate frames --at <commit> --capture` needs
`cargo-build-sbf`, which this lane did not run. Both the Trading link and the
accelerator link move (four new Trading routes, one new accelerator arm), so
this branch leaves the ratchet red by construction and says so here rather than
in a commit message nobody greps.

### 2.5 `docs/reference/refusals.md`

Twenty-eight new Trading codes in sub-band `0x4200`, two new accelerator codes
at `0xC10B`/`0xC10C`. Generated: the convergence owner runs `tools/gate
reference --converge` from a clean detached worktree, as AGENTS.md requires.
`tools/gate census --check-unique` already passes here.

### 2.6 `packages/dclutch-sdk/index.ts` (claimed by `build/joint-clearing`)

Nothing owed — see §6.3.

### 2.7 `tools/seam-audit/baseline.json`

NOT written, and deliberately. A `--write` rewrites the whole register, so from
this branch it would also adopt two findings that are main's and NOT this
family's — `DOMAIN_RAW_RESTATEMENT` on
`tools/gauntlet/journey/src/provider.rs:1196` and `:1208` — as accepted,
without the verdict `tools/seam-audit/EXCEPTIONS.md` requires, and carry away
one `GONE` row for `tools/gauntlet/journey/src/resolution.rs` that belongs to
whoever fixed it. Three rows this family owes, all POSITIVE records (the class
exists so the gate fails if the last guard in the file is deleted):

- `UNSET_GUARD_PRESENT` · `crates/dclutch-trading/src/scoring_rule/records_v1.rs:89`
- `UNSET_GUARD_PRESENT` · `crates/dclutch-trading/src/scoring_rule/requests_v1.rs:84`
- `UNSET_GUARD_PRESENT` · `tools/local-validator/bootstrap/successor/src/scoring_dealer.rs:873`

Everything else this family triggered was FIXED, not baselined:
`TRANSACTION_LEVEL_SIGNER_CENSUS` and `PRIVILEGE_PIN_UNEXEMPTED` on
`parse_prefix`, three `SEED_DOMAIN_UNASSERTED` on the emitted PDA domains, and
two `DOMAIN_RAW_RESTATEMENT` in the driver (§4.5, §4.6).

## 3. The collision, which was not the one the brief expected

`tools/gate census` convicted **five** overlaps at once: the build wave put the
scoring Dealer at Trading sub-band `0x4100`, and `SeriesAccountErrorV3`
(`programs/dclutch-trading-sbf/src/series/accounts.rs:44-52`) has held
`0x4100 .. 0x4104` on `main` since before this family existed. A green compile
never mentions it; the chain reports one number, and two owners makes whichever
one a reader assumes a coin flip.

Bands are append-only and a published code is never renumbered. No scoring
Dealer code has ever reached any chain and Series' have, so **this family
moved**: the sub-band offset is `0x200` and the codes are `0x4200 .. 0x421B`.
The offset is Lean-emitted, so the move is Lean-first —
`ScoringRuleAbiV1.refusalSubBandOffset`, both emitters re-run, both guards pass.

The brief's expected collision (`ClaimsSbfError 0x5012`, claimed by two other
families) is not this family's: the scoring Dealer allocates no Claims code.
Its two accelerator codes are `0xC10B` `ScoringRow` and `0xC10C` `ScoringRule`,
appended to `DealerAcceleratorSbfErrorV4`; the census sees no collision there.

## 4. What this lane completed

### 4.1 The fifteen mechanical errors, which were four defects

1. `SIGNED_DELTA_FIXED_ACCOUNT_COUNT_V3` lives in
   `dclutch_claims::frame_spec_v1`, not `signed_delta_v3`
   (`composition_v3.rs:44` carries a second spelling of it — pre-existing debt,
   named here, not fought over).
2. `extern crate alloc;` in `scoring_dealer_v1/mod.rs` binds `alloc` in that
   module only; the ten `alloc::` sites in the child modules named nothing.
   Each child that allocates now declares it, as every other module in this
   program does.
3. `ProtocolPositionRequestV2::capability_outcome` is not a choice: the Dealer's
   Position is a `TradingRecord` owner and `validate()` refuses `NonCanonical`
   for any nonzero outcome on a record owner. It is `0` because the validator
   says so — seam S8's "read one before choosing" had nothing to choose.
4. `CustodyLegV1<'a>` held `&'a [AccountInfo<'a>]` — one lifetime for the slice
   and for the account data — which forced every route's `accounts` to outlive
   its own account data. Splitting it into `<'accounts, 'info>` is what let the
   three routes keep the elided signature the entrypoint hands them. Seam S9's
   fix (rustc's own suggestion, `<'a>` on the routes) does NOT work: it
   propagates into the family-neutral `process_instruction`.

Then one only visible after those cleared: `encode_plan_into` keeps a unique
borrow of the packet for as long as the plan lives, so the fill could not hash
or send the bytes it had just written. The tree already has the shape —
`claims_composition_v3::verify_signed_delta_receipt` encodes, then decodes the
plan from the finished bytes — and the fill now does the same, and checks
`packet_digest` and `claims_program` against the receipt as that function does,
which the maker's version did not.

Also seam S5: `pub mod scoring_dealer_v1;` had been inserted between a doc
comment and the module it documented, so `user_position_admission_v1` had none
under `#![deny(missing_docs)]`. Moved to its alphabetical place.

### 4.2 The founding stopped taking the sponsor's word

`claim_unit_atoms` is the divisor every later route converts through
(`cash_claims`, `debit_atoms`, the withdraw floor) and `outcome_count` is the
width a fill mints a complete set across. Both were unauthenticated, so a
founding could price the Dealer's whole life in a unit the Market does not use
and every conjunct downstream would pass. Both are facts the Market already
published, and the record that publishes them was already in the frame: Claims'
own `Admit` window carries the basis record at coordinate 4
(`dclutch-claims/src/frame_spec_v1.rs:492`), so `authenticate_claim_unit_v1`
costs one decode and no account.

It holds the founding to `semantic_basis_id_v3` — the content address the
MARKET's founding committed to, whose preimage carries the kind, the width and
the payout scale — which is the argument
`market_closure_v1::refunds_on_failure_v1` already makes for the same account.
Three conjuncts, three codes: `Basis` (0x4219), `ClaimUnit` (0x421A),
`OutcomeCount` (0x421B).

### 4.3 The accelerator's Dealer arm

`DealerFillWitnessV1` was emitted from Lean, decoded, and read by nobody.
`programs/dclutch-trading-sbf/src/scoring_dealer_v1/accelerator.rs` is its
consumer: a pure function over (witness, request) running the SAME `admit_fill`
the route links — never a second implementation of R0–R3 that can disagree with
the route about one fill — writing one scalar bank, commit-last. The
accelerator program dispatches it by the witness's own magic (`DCLSFLW1`), as
it already dispatches the Shadow transport, and publishes two codes.

**Why the magic and not the admitted-AOT transport.** That transport selects its
entry by a `u16` at byte 10 of the family request
(`CapabilityProgramSetV1::selector_offset`; the Dealer set's is 10). Byte 10 of
`DCLSFLR1` is `outcome_count`, so a scoring row read through it would select
entry `K ∈ 2..=16` — exactly the equity (1..6) and LP (7..8) selectors. See
§6.2 for what a selector would cost.

### 4.4 Three holes the routes had that a green build never shows

- **The withdraw floor had one phase too few.** The route admitted `Open`,
  `Terminal`, `Retiring`, `Retired` and applied `Φ` to all four. `Ŵ` of a
  redeemed, empty inventory is `−Ŝ`, so a terminal Dealer would strand exactly
  the subsidy in its vault forever with no party it could ever be paid to —
  while the route's own doc comment claimed the residue was the whole balance.
  `DealerFill` admits `Phase::Open` and nothing else, so past Open there is no
  next fill for `Φ` to protect.
- **A Position was whatever account said it was one.**
  `read_inventory_for_withdraw_v1` derived the Position's address under
  `position.owner` — the account's own word — and neither the quote nor the
  withdraw frame carried anything that could contradict it. Any program can
  mint an account that satisfies a derivation under itself. For `DealerQuote`
  that forges a price the chain then publishes; for `DealerWithdraw` it is a
  **fund drain**, because a forged inventory raises `Ŵ`, raises `Φ`, and lets
  the sponsor take the whole vault including the subsidy that backs every
  admitted fill. `authenticate_release_v1` now returns the release's Claims
  role from the read it already made, and four routes use it: `quote` and
  `withdraw` hold the Position's owner to it, `found` and `fill` hold their
  frame's Claims program account to it. `quoteFrame` grew from six to eight
  accounts (activation cache, registry program) because it carried no release
  waist at all — Lean-first, both emitters re-run, both guards green, and
  `quote_frame_is_6` became `quote_frame_is_8`.
- **The fill could price against one Position and move another.** It reads the
  aggregate and both Positions out of its PREFIX and hands Claims a WINDOW
  carrying the same three coordinates, with nothing binding them. Claims,
  authenticating its own frame correctly, would have no way to know. They are
  one account each now, at Claims' own signed-delta coordinates. The founding
  carried the identical defect against its Admit window and has the identical
  fix.

### 4.5 Privileges are the child's fact

`tools/gate seam` named `parse_prefix` for `TRANSACTION_LEVEL_SIGNER_CENSUS`,
which is SEAM_AUDIT #13b's class — the one that killed the three founding-abort
routes at `5ca145e8`. Pulling on it found the same defect twice more.

`is_signer` is a property of the TRANSACTION (the fee payer is message key 0
and reads true at every coordinate that names it) and `is_writable` is a
property of the MESSAGE (one flag per account for the whole transaction). A
frame that pins either exactly is constraining the rest of the caller's
transaction, not its own instruction, and goes dead the moment a builder pays
with an account the frame carries. This route's own founding is that case: the
sponsor signs, and Custody's rent `payer` IS the sponsor, so the sponsor sits
inside two windows. Both privileges are now required in one direction and free
in the other.

And the child's metas were a guess, wrong in both directions. Claims and Custody
compare a frame against their own specs EXACTLY
(`protocol_position_v2.rs:751`, `custody-sbf/lib.rs:1629`). `signer = index ==
0` drops the Payer's signature — Custody declares `Payer` SIGNER_WRITABLE at
`InitializeReplay:9` and `OpenVault:13`, so **the fund's vault could never have
been opened**. `signer = index == 0 || account.is_signer` propagates the
transaction-level bit, so the child sees a signer its spec calls read-only and
refuses. `child_metas_v1` reads the child's own spec — `CustodyFrameSpecV1`,
`ClaimsFrameSpecV1`, `SignedDeltaFrameSpecV3`, all already dependencies —
writes exactly that into the metas, and holds the window to it
one-directionally, refusing `Frame` by name rather than dying inside the CPI
with a privilege-escalation error that names nothing.

`SEED_DOMAIN_UNASSERTED` named all three PDA domains: a seed segment past 32
bytes makes every address underivable, and the failure lands at the derivation,
not the declaration. The assert now travels with the constant, emitted from
Lean beside it, as `generated_protocol_infrastructure.rs` already does.

### 4.6 The four routes got their only caller

Seam S12. `BUILD_DEALER.md` §4.1 stated the founding input as if
`devnet-dealer-found-v1` existed; it did not, and neither did the other three,
so the family's whole outside was four magics nothing could send.

`tools/local-validator/bootstrap/successor/src/scoring_dealer.rs` (2,669 lines,
10 unit tests, run green) is the operator: found, quote, fill, withdraw, each
parsing its own flags, deriving its frame, reporting a preflight that opens no
key and sends nothing, and under `--execute` sending one transaction and then
READING THE FUND BACK and joining it to
`DealerReceiptV1::authenticate_for_request`, so a landed signature is not taken
as proof of an effect.

Nothing economic is typed: `K` and `claim_unit_atoms` come off the Market's own
linked-basis record (so the driver cannot hand `found.rs` a conversion that
route would refuse), the rule off the sealed rule account, the inventory off
the Dealer's Claims Position, and a fill's `receive`/`deliver`/`prices`/`mint`
out of `scoring_rule::solver::solve_buy`, which returns only fills the kernel
admits. `found` and `fill` ride a lookup table the run publishes (41 and 40
unique keys; both past the 1,232-byte legacy packet). `quote` and `withdraw`
send inline.

**It found the blocker in this lane's own work**: `child_metas_v1` required an
observed `is_signer` at the caller-authority coordinate, which is a PDA and
therefore never a top-level signer, so every route would have refused `Frame`
at its first child (§4.5, and the regression test that was proved red first).
The driver was the only reader that had to place the accounts, which is why it
saw what a compile could not.

**Not run against any cluster**: there is no devnet Market with a founded
Dealer to run them against. What is verified is that it compiles with no
warning of its own, that its frames are derived from the Lean privilege tables
and the children's own frame specs rather than typed, and that its ten tests
pass.

Reviewer notes it raised, none of which this lane chased:

- `found.rs` funds neither the Position nor the admission record before Claims'
  `Admit`, unlike `user_position_admission_v1`, which pays two explicit top-ups
  first. The driver reads both accounts' real lamports and data length into the
  request, so a rent shortfall will refuse as Claims' own cause -- but the
  founding likely needs a prefunding step the route does not do.
- `fill.rs` passes the delta Positions as `[dealer, taker]` while the frame
  spec calls that table "runtime **sorted**". If Claims requires sorted owners
  this refuses at plan time with `ValidatedSignedDeltaConstructionV3`'s own
  cause rather than on chain.
- A zero-amount Custody leg has no request and so no derivable authority; the
  driver places the System program at that coordinate. Correct only because
  `invoke_custody_transfer_v1` returns before touching the window, so a route
  that reordered that check would break it silently.

### 4.7 What was ruled provisionally

Four, all reversible, all stated in the source at the site:

1. **The withdraw floor is `Φ` under `Phase::Open` and zero once the Market is
   terminal.** The alternative — keep `Φ` forever — is a permanently unpayable
   balance, which is why this lane ruled rather than deferred.
2. **A Dealer's `K` equals the Market's ordinary outcome count exactly**, where
   ordinary excludes decision 0025's failure column, which the basis record's
   own `refunds_on_failure` names. Not `≤`: a `K` short of the ordinary width
   would mint claims on a strict subset of the outcomes, which is not a
   complete set and is not collateralized by the par the Hoard receives.
3. **The scoring family's accelerator transport is its own magic**, not a
   Dealer program-set selector (§4.3, §6.2). Reversing it means doing the
   selector work; nothing else changes, because the evaluator is the same
   function either way.
4. **The quote route carries the release waist.** A price the chain publishes
   is not a projection, so the frame grew by two accounts rather than the route
   trusting an account's own `owner`.

## 5. Tests added, and what each proves

### Kernel (`crates/dclutch-trading/src/scoring_rule/mod.rs`), run green

| test | what it proves |
|---|---|
| `the_three_conjuncts_of_r0_refuse_by_name` | `Width`, `Deliverable`, `NonCanonical` each by exact discriminant. The wave's tests named R1, R2 and R3; R0 — the canonical form under which the rule reads every vector — had none. |
| `a_price_vector_that_is_not_a_simplex_refuses_by_name` | `PricesNotSimplex` in both directions (a short sum, and a zero coordinate). Distinct from `OffSchedule`, which is a valid simplex that is not this Dealer's price; the note's §5 puts this one first because it is what a hand-written candidate carries. |
| `the_sponsors_loss_never_exceeds_the_subsidy_on_an_adversarial_path` | `bounded_loss`, walked. 512 legs of `b/4` claims, always buying, always on outcomes the scenario will not pay on — the direction that drives outcome 0's price to the floor. At every boundary it re-reads cash and inventory and asserts `solvent`, `potential_step`, and `bounded_loss` in all three scenarios. **Its control is what makes it evidence**: the path must cost the sponsor more than nine tenths of the subsidy, and measured it costs 24,774,458 of 26,591,259 (93%). The 7% it cannot reach is the uniform-price shortfall §3(a) names — the Dealer pays the POST-fill price for the whole leg and the cost function's increment is larger — which is the sponsor's income. An earlier version at 64 legs of `b` reached 74% and failed its own control; that is how the leg size was chosen. |

### Routes (`scoring_dealer_v1/mod.rs`), run green

`every_child_coordinate_carries_the_childs_own_privileges` — all five child
frames, coordinate for coordinate, against the child crate's own spec;
`the_custody_payer_signs_and_the_claims_basis_record_does_not` — the two
coordinates the old rules got wrong, named.

### Accelerator arm (`scoring_dealer_v1/accelerator.rs`), run green

`the_null_fill_admits_and_the_bank_is_the_kernels_verdict` (the bank is the
kernel's answer scalar for scalar); `the_arm_agrees_with_the_kernel_the_route_links`
(the arm and the direct route are one implementation — the whole reason the arm
exists); `a_price_off_the_schedule_refuses_by_name_and_writes_nothing`
(`OffSchedule` by name AND the bank byte-identical, which is the commit-last
claim); `a_fill_that_leaves_a_complete_set_refuses_not_normalized`;
`two_halves_of_different_rows_refuse_join`;
`a_short_wire_refuses_transport_and_a_narrow_bank_refuses_bank`.

### Accelerator program (`programs/dclutch-accelerator-sbf/src/dealer.rs`), run green

`a_truncated_scoring_wire_refuses_scoring_row_by_its_own_code` (the exact
`Custom` discriminant, derived from the enum and cross-checked against
`ACCELERATOR_REFUSAL_BASE + 0x10B`, never written as a number);
`the_scoring_magic_is_neither_admitted_family`.

## 6. What is still absent, named as absent

### 6.1 No program test on a real ELF, and no CU measurement

The brief asked for program tests on real ELFs for quote, fill and withdraw and
for CU per route against the note's 39k / 46k / 93k at K = 2 / 3 / 5. **They do
not exist and this lane did not write them.** Two reasons, both stated rather
than worked around:

- The convergence preamble forbids SBF builds in a family lane, and disk on
  this machine is the binding constraint that has hard-faulted once (37 GiB
  free while this lane ran).
- A Dealer lifecycle fixture is a whole Market — Core founding, a Registry
  release and activation, a Custody realm and replay, a Claims aggregate with
  its basis record, a Position admission, a mint and its token accounts. The
  nearest existing one,
  `programs/dclutch-trading-sbf/program-test/tests/support/series_premarket_expiry_chain_v1.rs`,
  is 3,596 lines. Writing that blind, with no way to run it, produces a file
  that looks like coverage and is not.

What the convergence must run, exactly: a new suite on
`programs/dclutch-trading-sbf/program-test/run-program-test.sh`'s pattern that
`cargo build-sbf`s Trading, Claims, Custody, Core, Registry and the accelerator
into one `SBF_OUT_DIR`, founds a Market, then found → quote → fill → withdraw,
with the note's §5 hostiles each asserting an exact discriminant out of the
`0x4200` sub-band, and `sol_log_compute_units` around each route. The routes
already carry the checkpoint trail:
`scoring-dealer:found:{frame,authenticated,claim-unit,subsidy,records,vault,deposit,position}`,
`scoring-dealer:quote:{frame,authenticated,priced}`,
`scoring-dealer:fill:{frame,authenticated,admitted,claims,custody}`,
`scoring-dealer:withdraw:{frame,authenticated,floor,custody}`.

Compare against 38,949 / 46,468 / 92,543 at K = 2 / 3 / 5 — and note those are
the NOTE's figures from a scratch probe of the ARITHMETIC alone, not of these
routes, so the route figure will be larger by the frame and account work. The
honest first act is to record it, not to assert it matched. No `tools/gates`
budget row was added for the same reason: a budget must be measured+tolerance,
and nothing here is measured.

**The first thing that run will convict**, if anything: the child metas (§4.5).
It is the one change in this branch whose correctness argument is read off two
other programs' specs rather than observed.

### 6.2 The accelerator's admitted-AOT selector

Owed: a selector coordinate in the scoring wire's Lean layout, plus
`DEALER_GLOBAL_SELECTOR_COUNT_V3` 8 → 9, the `dealer_request_schema_v3` match,
and the array parameter of `encode_dealer_global_program_set_v3` (which also
moves `DEALER_GLOBAL_PROGRAM_SET_BYTES_V3`, a released artifact width). Selector
9 is free; nothing in the tree uses 9 or above.

A thing a reviewer should distrust in `programs/dclutch-trading-sbf/src/dealer/release.rs`,
found while measuring that cost and NOT chased: it builds the Dealer program set
with the **V1** encoder (`CapabilityProgramSetV1`) while `hot_v3/execute.rs`
decodes with **V2**. Whether the two encodings are byte-compatible was not
verified here.

### 6.3 The SDK's route decoders

`packages/dclutch-sdk/lib/generated/scoringRuleV1.ts` now has a verifier that
runs, and still has no reader: nothing decodes a `DealerReceiptV1` or reports a
quote's `fresh` (BUILD_DEALER §4.3 says the SDK "reports `fresh`"; that reader
is unwritten). This lane did not write it because
`packages/dclutch-sdk/node_modules` does not exist in this worktree, so neither
`tsc` nor `vitest` can run, and an unrunnable TypeScript decoder is worse than
a named absence. The one-line `packages/dclutch-sdk/index.ts` export is owed
with it.

### 6.4 `FundPhaseV1::Retired` has no producer

`DealerFundV1` carries a two-variant phase and `authenticate_fund_v1` refuses
every route unless it is `Open`. Nothing anywhere writes `Retired`: the reader,
the schema and the refusal exist, and the producer was never written.

This lane did NOT invent one, and the reason is worth recording rather than
guessing at. The obvious producer — "the withdrawal that empties the vault past
terminal marks the fund Retired" — is wrong, because cash can still arrive
after it: the Dealer's Position holds ordinary claims that redeem to collateral
at resolution, and the fund's own `TradingPrincipal` vault is the natural
recipient. Marking the fund Retired at the first zero balance would strand
exactly that.

The route that can honestly write it is a `DealerClose`: the Position is
redeemed and closed, the vault is drained and closed, and the three PDAs' rent
goes back to the sponsor. It does not exist, is not designed, and is not in the
design note's build list. Until it does, `Retired` is a variant the wire can
carry and no chain can produce.

## 7. What a reviewer should distrust in `BUILD_DEALER.md`, because it was checked

| claim | what is actually true |
|---|---|
| §1.3 / §2.1: "sub-band `0x4100`", "25 variants, `0x4100..0x4118`" | The sub-band is `0x4200` and there are 28 variants, `0x4200..0x421B`. `0x4100` was Series'. §3. |
| §1.2: `mod.rs` "27 tests" | There were 13 `#[test]`s in `scoring_rule/mod.rs` (27 is the count across the whole module). It is 16 now, and 30 across the module. |
| §1.3: `quoteFrame` (6) | Eight. The route needed the release waist to prove a Position is a Position. §4.4. |
| §6: `scoring_rule__generator_fresh` "(needs lake)", as if unrun | It runs, and passes. So does the TypeScript twin. `lake` is on this machine and the module's build is cached. |
| §4.2: the residue is "Φ — which, with the inventory redeemed, is the whole balance" | False as arithmetic: `Ŵ(0) = −Ŝ`, so `Φ = cash − Ŝ`. The route now lifts the floor at terminal, which is what makes the sentence true. |
| S9: "rustc's own suggestion is the fix" | It is not; that signature propagates into the family-neutral `process_instruction`. The defect was `CustodyLegV1<'a>`. |
| S8: "the value ... is almost certainly the sentinel the other non-capability callers pass — read one before choosing" | There is nothing to choose: `validate()` refuses `NonCanonical` for any nonzero `capability_outcome` on a `TradingRecord` owner. |
| §5: "No `todo!()` and no `unimplemented!()` anywhere" | True, and checked. But §5's list of what is stubbed was incomplete: it did not name the unauthenticated `claim_unit_atoms`/`outcome_count` pair, the terminal withdraw floor, the unauthenticated Position owner, the unbound prefix/window aliases, or the invented child metas. |
| §7: "cohort-18 carries them" | There is no `tools/cohort/cohorts/18.json`. Only 14–17 exist. |

## 8. The Lean, checked rather than trusted

`lake build DClutchSemantics.ScoringRuleAbiV1` is green and reports exactly two
`sorry`, both in `ScoringRuleV1.lean`: `:1308` (`exp2Neg_below_the_real_value`)
and `:1316` (`exp2Neg_near_the_real_value`). The two log bounds —
`log2Ceil_above_the_real_value` and `log2Ceil_near_the_real_value` — carry none:
they are proven. The exp bounds are restated with `b ≤ 2^62` and a relative
tolerance of `2^(−19)`, and the module says why at `:748`: the `2^(−50)` the
design note carried was measured on the FRACTION and attributed to `Ê`, which
also takes a shift and a floor. Nothing in the Rust or the routes depends on
`2^(−50)`; the chain's sealed `tolerance` is `τ` in PRICE units (default one)
and is a different quantity from either.


## 9. The four runbook rows, verbatim

Fields are TAB-separated, eleven of them, in `check-steps.py`'s order: `key`,
`stage`, `since`, `until`, `replaces`, `shape`, `command`, `args`, `verifier`,
`cost_sol`, `blocks`. Insert between `activate-general` (line 65) and
`admissions` (line 70), in this order. **The generator injects `--rpc-url` and
`--i-mean-devnet` immediately after the verb**, so no row writes either.

```tsv
dealer-found	dealer-found	18	-	-	once	devnet-dealer-found-v1 --execute, after the Market is Open and its General capability is activated; anyone may found, and the signer becomes the sponsor of record	bootstrap devnet-dealer-found-v1 --market {market.address} --campaign-report $HERE/{market.work_dir}/campaign-open.json --dealer-id {market.dealer.dealer_id} --sponsor {pubkey:{market.dealer.sponsor_keypair}} --sponsor-token {market.dealer.sponsor_collateral_account} --liquidity {market.dealer.liquidity} --scale {market.dealer.scale} --tolerance {market.dealer.tolerance} --deposit {market.dealer.deposit_atoms} --evidence $OUT/found.json {execute:--execute --sponsor-keypair $HERE/{market.dealer.sponsor_keypair}}	the fund account exists at the DealerFundV1 PDA with revision 0 and cash equal to the deposit, the rule account's own bytes hash to the fund's rule_digest, the recorded subsidy equals subsidyOf(b, K) recomputed off chain, the fund's claim_unit_atoms equals the Market basis record's payout_scale, and the first quote's prices sum to the rule's scale with every coordinate positive	-0.01	dealer-quote
dealer-quote	dealer-quote	18	-	-	once	devnet-dealer-quote-v1, permissionless: the price series becomes a chain fact rather than a candidate account's private field	bootstrap devnet-dealer-quote-v1 --market {market.address} --campaign-report $HERE/{market.work_dir}/campaign-open.json --dealer-id {market.dealer.dealer_id} --fee-payer {pubkey:keys/campaign-payer.json} --evidence $OUT/quote.json {execute:--execute --fee-payer-keypair $HERE/keys/campaign-payer.json}	the quote account's fund_revision equals the fund's, its prices sum to the scale, and they equal pricesOf recomputed off chain from the Dealer Position's own balances -- not from the fund's cached inventory_minimum, which is what makes this a projection of the chain rather than of the record	-0.00	dealer-fill
dealer-fill	dealer-fill	18	-	-	once	devnet-dealer-fill-v1: the taker buys claims of one outcome, the solver inverts the rule off chain and the route re-admits R0-R3 on chain	bootstrap devnet-dealer-fill-v1 --market {market.address} --campaign-report $HERE/{market.work_dir}/campaign-open.json --dealer-id {market.dealer.dealer_id} --taker {pubkey:{market.dealer.taker_keypair}} --taker-token {market.dealer.taker_collateral_account} --buy-outcome {market.dealer.buy_outcome} --buy-claims {market.dealer.buy_claims} --evidence $OUT/fill.json {execute:--execute --taker-keypair $HERE/{market.dealer.taker_keypair}}	the fund revision advances by exactly one, Phi = cash + W(inventory) recomputed from the post-fill Position does not fall, the Hoard rises by exactly the mint's par and by nothing else, the taker's Position moves by mint + deliver - receive and the Dealer's by receive - deliver, and the aggregate rises by mint on every ordinary outcome and not on the failure coordinate	-0.01	dealer-withdraw
dealer-withdraw	dealer-withdraw	18	-	-	once	devnet-dealer-withdraw-v1: the sponsor takes profit down to the floor while the Market is Open, and one unit past it must refuse	bootstrap devnet-dealer-withdraw-v1 --market {market.address} --campaign-report $HERE/{market.work_dir}/campaign-open.json --dealer-id {market.dealer.dealer_id} --sponsor {pubkey:{market.dealer.sponsor_keypair}} --sponsor-token {market.dealer.sponsor_collateral_account} --amount {market.dealer.withdraw_atoms} --evidence $OUT/withdraw.json {execute:--execute --sponsor-keypair $HERE/{market.dealer.sponsor_keypair}}	the vault falls by exactly the amount and the sponsor's token account rises by it, the fund's cash falls by the same, and the evidence carries the floor Phi the route computed so a reader can check the amount was at or under it rather than taking the landed signature as the proof	-0.00	-
```

Each also needs a `### <key>` heading at column 0 in `tools/cohort/README.md`:
`### dealer-found`, `### dealer-quote`, `### dealer-fill`, `### dealer-withdraw`.

And `tools/cohort/cohorts/18.json` (which does not exist yet) needs a
`markets[].dealer` block carrying exactly the fields the rows name:
`dealer_id` (64 hex), `sponsor_keypair`, `sponsor_collateral_account`,
`liquidity`, `scale`, `tolerance`, `deposit_atoms`, `taker_keypair`,
`taker_collateral_account`, `buy_outcome`, `buy_claims`, `withdraw_atoms`.
`check-steps.py` resolves every `{field}` a row names against the manifest and
refuses one it cannot, so the block and the rows land together.

`deposit_atoms` must be at least `subsidyOf(liquidity, K) * claim_unit_atoms`
or `found.rs` refuses `Subsidy` (0x4206); `withdraw_atoms` must be at or under
the floor `Φ` the fill leaves, or `withdraw.rs` refuses `WithdrawBelowFloor`
(0x4211). Both numbers are the cohort's to choose and both are derivable from
the row before it.
