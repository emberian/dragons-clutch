# General's escrow, made physical — and the copy of the ruling the chain runs

Lane GEN-ESCROW, 2026-08-27. Commits `4f823c4a`, `31eca2fa`, `ba63fc8c`,
`b6e28707`, `53da0565`. Charter: ADR-0010 §6 items 1 and 3 — make the escrow move, and
author the seven artifact triples.

This is not release evidence. It records what was executed, what it cost, and —
in §5 — exactly what is still missing, including the charter item this lane did
not close.

---

## 1. The finding: the escrow ruling never reached the artifact

ADR-0010 §2 rules that admission MOVES the maker's worst case, so `Collect` runs
`Settlement(order_id) -> Settlement(candidate_id)`, and states that "the old
`External(owner)` route is refused outright".

**That was true of the pure contract and false of the artifact the chain
executes.**

| authority | what it said about `Collect`'s source |
|---|---|
| `collection_v1::admit` / `child_packets::build_row_custody_packets_v2` | `Settlement`, vault context = `order_id`, owner zero |
| `effect_artifacts_v3::build_action` (the emitted EffectProgram template) | `External`, owner nonzero, vault context zero |
| `artifacts_v3::validate_routes` (the join that admits a release) | required `External -> Settlement` |

`Collect` is not one of the actions whose compartment bytes the admitted
EffectProgram patches at runtime — only `Materialize` is — so the template's two
literals are what a chain-executed `Collect` carries.

The two readings are not merely different, they are **mutually exclusive**.
`CustodyRequestV1::validate` requires of a `Transfer`:

```text
(source_compartment == External) == is_zero(source_vault_context)
(source_compartment == External) == !is_zero(source_owner)
```

So an `External` source demands a nonzero owner and a zero vault context, which
is exactly what the packet builder refuses. A frame either side accepts, the
other rejects. **The published artifact still debited the maker's own external
account at settlement time** — decision 0009 §2's live credit regression, in the
only copy that would have executed.

### Executed, not argued

The defect was reproduced before it was fixed, by reading the compartments out of
the emitted bytes:

```text
---- escrow_v1::tests::the_emitted_collect_draws_on_a_settlement_vault_and_never_on_an_external_owner
assertion `left == right` failed
  left: Some((External, Settlement))
 right: Some((Settlement, Settlement))
---- escrow_v1::tests::every_emitted_custody_template_carries_the_compartments_the_one_table_names
assertion `left == right` failed: action Collect emits compartments the escrow table does not name
```

18 of the 20 new tests passed at that point; these two are the defect.

### Why it survived, and the rule that generalises

`build_order_escrow_packets_v1` and `build_row_custody_packets_v2` have no caller
outside their own tests; no `Collect` has ever executed on chain; and each side's
tests assert against its own author. ADR-0010 §5 drew GEN-HOT's lesson as *a
family's own emitter and its own authenticator are not two authorities*. It
generalises one level further:

> **A family's own contract and its own artifact are not two authorities either.**

### The repair

Not correcting the second copy — deleting it.
`escrow_v1::general_child_custody_movement_v1` is now the single place General
states, per child effect, which compartments the atoms move between **and** which
identity keys each side's vault. Both halves, because Custody ties them together.
Read by: `effect_artifacts_v3::build_action`, `artifacts_v3::validate_routes`, and
all four packet builders (`build_row_custody_packets_v2`,
`build_escrow_custody_packets_v1`, `build_materialize_packets_v2`,
`build_surplus_packet_v2`).

| child effect | source | destination |
|---|---|---|
| `CollectCollateral` | `Settlement` @ order | `Settlement` @ candidate |
| `DistributeCollateral` / `PaySurplus` | `Settlement` @ candidate | `External` @ owner |
| `MintCompleteSet` | `Settlement` @ candidate | `HoardPrincipal` @ market |
| `MergeCompleteSet` | `HoardPrincipal` @ market | `Settlement` @ candidate |
| `EscrowCollateral` | `External` @ owner | `Settlement` @ order |
| `ReleaseCollateral` | `Settlement` @ order | `External` @ owner |
| the four Claims legs | — (Positions, not vaults) | — |

A single table means neither end fixes the value alone, so the independent pin is
restored by `a_release_whose_transfer_names_another_compartment_is_refused`: it
patches the EMITTED bytes to a real, live, correctly-shaped neighbouring
compartment (`HoardPrincipal`) and requires the join to refuse. Vault-to-vault on
purpose — an `External` swap would be caught by Custody's own decode and would
prove nothing about this join.

## 2. The addressing is now checked on chain

ADR-0010 §2 rests "a maker can never be paid more than they escrowed" on the
vault being keyed by the order's own identity. **Nothing required the vault
presented in the frame to BE that one.** The vault context reaches the register
bank from the AccountProfile's projection of caller-supplied Custody accounts;
the order identity reaches it from the authenticated manifest row. A `Collect`
could name order A in its semantics and draw on order B's vault.

Two transition conjuncts now bind them, on each of the three actions whose
direction is fixed at authoring time:

| action | conjuncts added |
|---|---|
| `Collect` | `SOURCE_VAULT_CONTEXT == ORDER`, `DESTINATION_VAULT_CONTEXT == CANDIDATE` |
| `Distribute` | `SOURCE_VAULT_CONTEXT == CANDIDATE`, `CUSTODY_DESTINATION_OWNER == OWNER` |
| `Close` | `SOURCE_VAULT_CONTEXT == CANDIDATE`, `CUSTODY_DESTINATION_OWNER == BENEFICIARY` |

`Materialize` patches its compartments at runtime from the authenticated
complete-set move, so which side is the Hoard is not a constant of its artifact.
It is named here rather than half-checked there.

Witness: `collect_refuses_a_vault_that_is_not_the_one_its_row_names` folds the
REAL emitted transition artifact through the real TransitionVM at N=1 and N=258,
accepts the named vaults, and refuses a substituted one at each side.

**Honest scope:** the campaign in §4 does not execute this. The accelerator
evaluates the pure General transition and projects a register bank; the
TransitionVM artifact is executed by Trading's `process_hot_execution_v3`, and no
General bundle runs through Hot (GEN-HOT). The fold above is a real execution of
the real artifact, not through the ELF.

## 3. The work escrow's accounting gets a referent

ADR-0010 §6 item 3 names the gap as movement. The sharper gap was underneath it:
**nothing bound the accounting to a balance either.**
`GeneralCandidateV1::validate_capitalization` compares `verification_remaining`
against a number it derives from the same record, so a submission whose account
holds nothing at all re-proves its capitalization at every transition and passes
every time.

`escrow_v1` supplies the referent and the movements:

```text
escrow_lamports == rent_floor + verification_remaining + cleanup_remaining
```

re-proven at every transition, plus:

- `WorkEscrowFundingPlanV1` — vacant account, exact in both directions;
- `WorkEscrowDrawPlanV1` — built from the observed balance AND the successor
  record together, so a transition cannot advance its record and leave the
  account untouched, or the reverse; the rent floor is a floor and never a
  compartment;
- `WorkEscrowClosePlanV1` — cleanup crank to whoever performed it, unspent
  verification compartment AND the record's rent back to the solver, three
  credits required to sum to what the account held (ADR-0010 §6 item 3's rent
  ownership, routed);
- `OrderEscrowPlanV1` — deposit / refund / residual, each with the balance its
  direction requires, and `authenticate_collect_from_escrow_v1` for the
  settlement draw.

### Hostiles, all executed

| refusal | witness |
|---|---|
| an escrow that holds nothing still passes `validate_capitalization` | `an_escrow_that_holds_nothing_re_proves_its_own_accounting_and_still_refuses` |
| under- and over-funding | `submission_funding_is_exact_in_both_directions` |
| a non-vacant submission account | `a_submission_account_that_is_not_vacant_refuses_to_be_funded_again` |
| record and balance disagree after a draw | `a_draw_whose_successor_record_disagrees_with_the_balance_refuses` |
| a crank drawn past its compartment | `a_crank_cannot_be_drawn_past_the_work_the_escrow_was_sized_for` |
| a crank paid out of the rent floor | `a_crank_cannot_be_paid_out_of_the_rent_floor` |
| a close paid from the wrong compartment | `a_close_paid_out_of_the_verification_compartment_refuses` |
| a second close | `a_second_close_out_has_nothing_to_conserve` |
| cross-ORDER escrow | `an_escrow_keyed_by_another_order_is_refused` |
| cross-BATCH escrow | `an_order_from_another_batch_cannot_reach_this_batchs_escrow` |
| double refund | `a_refunded_escrow_cannot_be_refunded_again` |
| a residual exceeding the reserve | `a_residual_release_returns_the_balance_and_can_never_exceed_the_reserve` |
| settle without escrow | `a_collect_cannot_draw_on_an_escrow_that_does_not_hold_the_debit` |

## 4. The extended campaign

`programs/dclutch-general-accelerator-sbf/program-test/run-program-test.sh`, pure
`solana-program-test`, no validator.

**19/19 (3 + 2 + 10 + 4), zero frame diagnostics.**

The campaign built a batch, admitted three orders, closed it, submitted a
candidate and verified every row — and not one balance existed anywhere in it.
`MakerFundingV1` was a declared number and the work escrow was a field that
compared itself against itself. `terminal_fixture` now carries an
`EscrowLedgerV1` through the whole run, and every transition constructs its exact
movement against an observed balance and advances the ledger ONLY through that
plan's own postcondition check.

| stage | what executes |
|---|---|
| admission ×3 | `OrderEscrowPlanV1` deposit + per-outcome claim check; then `committed_quote_reserve` is asserted to equal the sum of balances actually held — it was a sum of promises, and ADR-0010 §2 calls it a bound that nothing measured |
| submission | `WorkEscrowFundingPlanV1`; then `authenticate_work_escrow_v1` |
| verification ×3 | `WorkEscrowDrawPlanV1` per row, out of the candidate's own escrow |
| `Collect` ×3 (real ELF) | the effect the transition produced is decoded, its order resolved BY IDENTITY, and `authenticate_collect_from_escrow_v1` requires that order's own vault to hold the debit; the vault is then debited by exactly the amount the effect carried |
| after `Close` | a post-window release per order, quoting no amount; the consideration crank; `close_out` |

Conservation is asserted at every step and at the end: total atoms and total
lamports never change, every vault ends at zero, the escrow account ends at zero,
and five cranks are paid out of a capacity sized for exactly five.

**What this is NOT.** No lamport moved on a chain. The mover is Trading's
`commit_output_lamports_v3`, and General has no route to reach it — see §5.

### Evidence rows, N=1 and N=258

Accounts, legacy packet extent and scratch pages are **unmoved** by everything in
this lane; CU moves by a constant per action (§4a).

| action | N=1 CU | N=258 CU | accounts (1 / 258) | legacy packet (1 / 258) | pages (1 / 258) |
|---|---:|---:|---|---|---|
| Consider | 36,097 | 74,861 | 33 / 47 | 811 / 1,273 | 3 / 17 |
| Freeze | 32,643 | 65,054 | 31 / 45 | 745 / 1,207 | 3 / 17 |
| InitializeSettlement | 61,320 | 164,453 | 89 / 103 | 867 / 1,329 | 3 / 17 |
| Collect | 56,979 · 58,161 · 58,184 | 146,935 · 147,362 · 148,156 | 70 / 84 | 848 / 1,310 | 3 / 17 |
| Materialize | 53,159 | 141,390 | 68 / 82 | 814 / 1,276 | 3 / 17 |
| Distribute | 56,930 · 58,135 · 58,166 | 144,573 · 145,794 · 146,596 | 70 / 84 | 848 / 1,310 | 3 / 17 |
| Close | 61,322 | 155,774 | 87 / 101 | 833 / 1,295 | 3 / 17 |

Packet extents 745–867 at N=1 and 1,207–1,329 at N=258, against Solana's
1,232-byte legacy maximum: **six of seven N=258 actions are still over it**, so
`blocked.json`'s ALT/v0 clause stands unchanged. The binding action is
`InitializeSettlement` at 164,453 of 1,400,000 — 11.75% of the compute ceiling.
Compute is not the wall; the packet is.

### 4a. The one measured cost, and where it came from

Three artifact dispatchers still had catch-alls (§5 item 3). Making them
exhaustive changed code the accelerator ELF runs, and cost a **constant** per
action, identical at N=1 and N=258 — so no slope moved:

```text
Freeze +0   Close +8   Materialize +8   Collect +26   Consider +26
Distribute +27   InitializeSettlement +53
```

Accounts, packet extent and scratch pages identical in all 23 rows. The binding
action goes 164,400 → 164,453; 11.74% → 11.75% of the ceiling.

Everything else in this lane — the compartment correction, the vault-context
conjuncts, the whole escrow ledger — moved **zero** CU, zero accounts, zero
packet bytes and zero pages. `Collect` at N=258 reproduced
146,909 / 147,336 / 148,130 exactly across the compartment change.

## 5. What General still lacks

Not "only the validator-evidence tier". Precisely:

1. **The seven artifact triples. NOT CLOSED BY THIS LANE.** `OpenBatch`,
   `PlaceOrder`, `CancelOrder`, `CloseBatch`, `SubmitCandidate`,
   `VerifyCandidateRow` and `ReleaseOrder` have Lean-owned tags, reserved
   `CapabilityProgramSetV2` coordinates and complete authenticated pure
   transitions, and no TransitionVM program, EffectProgram, AccountProfile or
   Lean-emitted RequestProfile. This lane did not author them, and did not author
   a partial one: a triple that cannot execute is parked, not landed, and the
   tree has already paid for that distinction twice this cycle.

   What it did instead is make the remaining work exact, and pay down its first
   row. The dependency order and every site are below; `escrow_v1` already names
   the Custody movement each of the three escrow verbs performs, which is the
   first line of their EffectProgram.

   | # | site | today | must become |
   |---|---|---|---|
   | 1 | `formal/…/GeneralRequestProfilesV1.lean` `actions` + `EmitGeneralRequestProfilesV1Rust.lean` | seven | fourteen, then regenerate `generated_request_profiles_v1.rs` via `tools/atomic-generate` |
   | 2 | `specialization.rs:29` | `&[]` | seven new profile consts |
   | 3 | `state_artifacts_v3.rs` evidence + lifecycle counts (`:168`, `:176`, `:200`, `lifecycle_counts`, `lifecycle_binding_count`, `lifecycle_current_rent_quote_count`) | zeros | real counts; these fix the account prefix everything else derives from |
   | 4 | `effect_artifacts_v3.rs` `general_effect_route_count_v3` **and** its duplicate `route_count`, the `(action, route)` frame table, instruction counts, template bytes, receipt deps, custody callee, account count, `build_action` | zeros / `UnauthoredAction` | seven `build_*` fns |
   | 5 | `account_rules_v3.rs` operation count + `general_account_profile_rule_v3` dispatcher (and `GENERAL_MAX_ACCOUNT_PROFILE_OPERATIONS_V3` if any action needs more than nine) | zero | real rules |
   | 6 | `transition_artifacts_v3.rs` instruction counts, `append_common`'s kind selector, `append_action`, `append_item` | `(0,0,0)` / `UnauthoredAction` | real programs; the kind selector needs a third answer |
   | 7 | `artifacts_v3.rs:validate_routes` | `Err(Effect)` | per-action route assertions |
   | 8 | `release_v3.rs:~298` `UnjoinedProfile`, plus the two `GENERAL_ACTION_PROGRAM_COUNT_V3`-sized arrays | refuses a `Complete` profile | admits it |
   | 9 | `crates/dclutch-operator/src/general_hot_v3.rs` `derive_general_request_v5` / `derive_settlement_request_v5` | `ChainState` | real requests |
   | 10 | `programs/dclutch-general-accelerator-sbf/src/lib.rs:~373` `evaluate_candidate` | `State` | seven evaluation arms |
   | 11 | `effect_artifacts_v3.rs` `unauthored_actions!`, `general_action_artifacts_authored_v3`, `require_authored_action_v3`, and three `UnauthoredAction` variants | exist | deleted — the macro going away is the completion signal, and `tests/unauthored_actions_v1.rs` goes with it |

   Also required and not on that list: the four new record kinds
   (`GeneralBatchV1`, `GeneralOrderV1`, `GeneralCandidateV1`, the verifier
   cursor) need `GeneralLocalStateKindV3` variants and register-bank coordinates.
   `GENERAL_HOT_COMMON_SCALARS_V3` is 90 and `GENERAL_HOT_COMMON_IDENTITIES_V3`
   is 40; growing either moves **every** General artifact identity, so it is one
   batched regeneration and it should be the same one that carries this lane's
   `Collect` effect and `Collect`/`Distribute`/`Close` transition digests.

2. **Lamport movement has a discipline and still has no mover.** §3's plans are
   executed end-to-end in the campaign; the physical layer that assigns the
   lamports is Trading's `commit_output_lamports_v3`, reached by an admitted
   EffectProgram declaring an output-lamports coordinate. That is downstream of
   item 1. Naming it, not calling it fail-closed.

3. **Three dispatcher catch-alls, now closed** — `b6e28707`. Two returned the
   settlement answer for an unauthored action; the third,
   `general_state_lifecycle_bytes`, had no match at all and **emitted a lifecycle
   policy artifact for an action with no triple**. None was reachable, because
   reachability is a property of the callers and a catch-all is a promise about
   the callees. `tests/unauthored_actions_v1.rs` is the check that does not
   depend on which is true today.

4. **The census row flipped at N=1 and not at the canonical width** —
   `53da0565`. `tools/gauntlet/general/` now exists: `record()` in both
   campaigns' `submit`, eight bindings on the one route the accelerator exposes,
   eight witnesses, a `run-general.sh` with a frame-diagnostic gate and a ledger
   lock, and the four fast-lane clauses merged into the evidence document.
   `general-accelerator/process_instruction` reads
   `EXECUTED (18x via general-accelerator-programtest)` with one of its five
   refusal codes observed, and `blocked.json`'s entry is deleted.

   **The width restriction is the design, not an economy.** At N=258 six of seven
   actions do not fit a packet and ProgramTest cannot notice, so recording one
   would flip a route to EXECUTED on a frame no validator would accept. The
   campaign still runs and measures both widths; it records only N=1, gated
   inside the fixture on the width rather than on a test name, with a witness
   pinning that nothing else got recorded. What is still owed: the canonical
   width needs the ALT/v0 route, and the real path needs a General Hot bundle
   that does not exist.

5. **General has no rows in `tools/gauntlet/CU_BUDGETS.json`.** The table in §4
   is this document's, not the census's, and the new tier deliberately carries no
   `cu-budget` witness — one naming a campaign with no budget entries is a red
   `NOCAMPAIGN` row, which would be a worse statement than silence. Pinning
   budgets is the CU-BUDGET lane's file and its call.

6. **`ExpireSettlement` has no gen-3 counterpart**, and **the claim escrow's
   Position lifecycle** still has nothing that creates or closes the
   `(market, order_id)` Position (ADR-0010 §6 items 4 and 5, unchanged).

7. **`GeneralClearing.lean` still does not model the collection or candidate
   halves** (ADR-0010 §6 item 6, unchanged).

## 6. Reproduction

```sh
cargo test -p dclutch-general-adapter-contract
#   lib 176 · unauthored_actions_v1 6 · root_lifecycle_projection_v3 2
#   request_profiles_generator_fresh 1
programs/dclutch-general-accelerator-sbf/program-test/run-program-test.sh
```

The second needs `cargo build-sbf`; it builds the accelerator and the test-caller
ELFs into a temporary directory and runs the four suites with `--nocapture`,
which is how the evidence rows in §4 reach stdout. `tools/gauntlet/general/run-general.sh`
wraps the same build and test with the frame-diagnostic gate, the witnesses and
`census observe`. The program-test carries its own
`Cargo.lock` and passes `--locked`; that lock still named `sha2` after the
`dclutch-sha256-adapter` migration removed it from the adapter, so **the campaign
could not be run at all from a checkout of main** until `31eca2fa`.
