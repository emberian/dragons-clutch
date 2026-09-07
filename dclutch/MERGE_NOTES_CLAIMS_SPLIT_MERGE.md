# MERGE NOTES — claims-split-merge

Branch `build/claims-split-merge`, rebased onto `main` at `9b8026334` (clean rebase, no conflict — the
BUILD doc predicted a textual conflict at `tools/cohort/steps.tsv:85` and there was none; `main`'s retire
row and this branch's `split`/`merge` insertion are three lines apart).

## 1. READY

Every crate this family touches is green with `cargo check --offline --tests`, zero errors:

```
dclutch-claims  dclutch-claims-sbf  dclutch-operator
dclutch-local-successor-bootstrap  dclutch-fractional-atomic-program-test
```

Native tests that run without an SBF toolchain, all passing:

| filter | result |
| --- | --- |
| `-p dclutch-claims --lib complete_set_v1` | 9 passed |
| `-p dclutch-claims --lib conservation` | 67 passed |
| `-p dclutch-claims-sbf --lib claims_conservation_v1` | 6 passed |
| `-p dclutch-operator --lib claims_conservation` | 8 passed |
| `--test claims_conservation the_census_reads_red` | 1 passed |

**What is NOT verified, and must be said plainly: the five ELF campaigns in
`tests/claims_conservation.rs` have never been RUN.** This lane is forbidden SBF builds and no
`dclutch_claims_sbf.so` exists anywhere on this machine, so `SBF_OUT_DIR` cannot be satisfied. They
compile, every address they derive comes from the program's or the operator's own helper rather than a
restatement, and three defects were caught by reading them against the route (§3). That is not the same
as green. **The merge lane must run
`tools/gauntlet/claims-fractional-atomic/run-fractional-atomic.sh` before treating this family's central
claim as demonstrated**, and should expect to spend a round or two on fixture arithmetic.

## 2. SHARED FILES — for the merge lane

Changes this lane did NOT apply because another build branch edits the same file. Exact, one line each.

| file:line | change | also edited by |
| --- | --- | --- |
| `formal/dclutch-semantics/DClutchSemantics.lean:22` | add `import DClutchSemantics.ClaimsConservationV1Abi` in alphabetical position (before `ClaimsLiabilityBasisStateV2Abi`) | `build/{recovery-capture,general-lifecycle,dealer,series,economics,product-shapes,joint-clearing}` — seven branches |
| `tools/cohort/generate-stage-scripts.py:94` | add `"split": ("direct",), "merge": ("direct",),` to `MARKET_KIND` — see §5's B2, the two-source market carries neither split field | `build/failure-arm` |
| `tools/gauntlet/blocked.json` | add `{"route":"claims/claims_conservation_v1::process","class":"unwired","reason":"the ELF campaign exists at programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/claims_conservation.rs and has never been executed: the convergence lane had no SBF toolchain"}` — remove the row once the campaign runs and S17's bindings land | `build/series` |
| `tools/cohort/steps.tsv:87,:88` | the `split` and `merge` rows are already applied on this branch and rebased cleanly onto `main`'s rewritten `retire` row at `:85`; three other branches insert into the same file | `build/{failure-arm,series,founder-bond}` |

Generated artifacts, none of which this lane could regenerate (all need an SBF build or a warm `.lake`):

- `tools/gates/frames-baseline.json:1255-1327` — recapture with `tools/gate frames --capture`. Thirteen
  `claims_conservation_v1::*` symbols leave, eleven arrive; `signed_delta_v3` and `affine_batch_v2` each
  gain one frame from the executor call.
- `tools/gates/emission-coverage.md:14` and its generated-file table — `tools/gate emission --write`.
  The branch's third emission block IS present at `crates/dclutch-claims/check-generated.sh:58,:62`;
  the census header moves 91/85 → 92/86.
- `docs/reference/{refusals,routes,route-witnesses}.md` — `tools/gate reference --converge`, covering
  the thirteen new `0x5300`–`0x530C` rows and the withdrawal note for `ClaimsSbfError::CustodyRequired
  0x5006`, which this branch leaves raised by nothing.
- `crates/dclutch-claims/src/generated_conservation_v1.rs` was **hand-written to the emitter's expected
  output and never emitted**. Run `crates/dclutch-claims/check-generated.sh`; if its third block's
  byte-compare fails, the offsets the route reads are wrong and every figure in this family is suspect.

## 3. Completed since the build wave

1. **Rebased onto `main`** (`git rebase`, clean). `main` carries PROGRAMS-17E's closure burn already —
   `7d45d6ba3` and `b1fe0193d` are both ancestors of this branch's own base `f7c03e845`, so the
   reconciliation the brief asked for was already structural: the executor is the sole author of the
   coordinate write that the closure burn's debit arm goes through, and `semantic_basis_id_v3` is the
   record identity's sole author in `authenticate_basis_record`. Two authors were not found.
2. **`founding_world.rs:37`** — `use dclutch_fractional_atomic_program_test::{…}` → `use crate::{…}`.
   One line cleared twelve of the thirteen errors, exactly as the close-out said.
3. **`founding_world.rs:846`** — `get_fee_for_message(&transaction.message)` →
   `get_fee_for_message(transaction.message.clone())`. The banks client takes a `Message` by value; the
   tree's other caller (`programs/dclutch-core-sbf/tests/retirement_replay_handoff_program_test.rs:1081`)
   already passes by value. The close-out's diagnosis (a `[dependencies]`/`[dev-dependencies]` split
   resolving two `Message` types) was wrong; there is one `Message` and the `&` was simply never compiled.
4. **`tools/local-validator/bootstrap/successor/src/claims_conservation.rs:302-303`** — `RealmV1`'s
   `collateral_mint` and `token_program` are private with reference-returning accessors. **Two errors the
   BUILD doc's §7 said did not exist** ("Everything else on the branch was checked green by the maker").
5. **`tests/claims_conservation.rs` rewritten** (990 → 1,510 lines) onto the founded world — §4.
6. **S3 applied**: `docs/decisions/0007-namespaced-refusal-codes.md:138`, the
   `ClaimsConservationSbfErrorV1 | — | 0x5300–0x530C` row, ordered by code between
   `SparseNativeTransferSbfErrorV1` and `ClaimsMarketClosureSbfErrorV1`.
7. **Operator economics tests restored** — §4.

### Ruled provisionally (ember rules by reversal)

- **The actor in the ELF campaign is the FOUNDER, not a stranger.** A Position is created by admission,
  so on a freshly founded Market the only holder who can merge is the one the founding admitted. A
  stranger's split needs an admission act first, which is a second campaign. Reversal cost: one
  `protocol_position_v2` admission transaction before the split.
- **The Core Market's `Founding → Open` advance is applied to the account directly**, through the codec
  that owns `CoreState`, with every other field — including the identity the account's own address
  derives from — left as the founding found it. It is Core's own stage and no part of this route's
  subject; driving Core's real transition would import the whole Core opening campaign into a Claims one.
- **The census lives in the test file.** `ConservationCensusV1` does not exist anywhere in the tree on
  either side (the BUILD doc's S18 is stale about this and the close-out corrected it). Lifting the
  journey's `ledger.rs` into a library both consume is named debt, not hidden: this file restates the
  eight laws over the accounts a program-test can read, and carries their positive control.

## 4. Tests added, and what each proves

### `programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/claims_conservation.rs` (rewritten)

| test | proves | runnable here |
| --- | --- | --- |
| `a_split_and_its_merge_are_a_round_trip_over_the_eight_laws` | the family's whole claim: a split moves collateral into the Custody vault through the delegated wire and mints a complete set, its merge returns the atoms and burns the set, the aggregate's supply vector is back where it started, and L1–L8 hold at four boundaries. Also that the escrow takes the failure coordinate and the holder never does. | no — needs `SBF_OUT_DIR` |
| `a_split_without_the_collateral_refuses_balances` | `0x5306`: a stated prestate that is not what the account holds refuses BEFORE the transfer. The conjunct that stops claims existing against a vault that never received their backing. | no |
| `a_merge_that_names_the_failure_coordinate_as_its_own_refuses_identity` | `0x5302`: an actor who offers the Market's own escrow Position as their holder Position is refused by the Position's recorded owner. | no |
| `a_categorical_merge_of_an_incomplete_set_refuses_holding` | `0x5308`: the executor runs over candidates before any collateral moves, so a merge of more sets than the holder holds refuses on the holding and never reaches Custody. On a CATEGORICAL Market — the control that this route is not a refunding-only one, and the campaign's only test of the shape that seats no escrow. | no |
| `the_reversed_transfer_pair_refuses_identity` | `0x5302`: the two token accounts named in the opposite roles. The guard on the direction-free vault seeds — the defect the first draft had, where the vault was derived from the request's SOURCE side, which on a split is the External side. | no |
| `the_census_reads_red_when_a_law_is_broken` | **the positive control.** Each of the eight laws is driven past its own boundary and the exact verdict required — including that an INAPPLICABLE names its reason rather than passing. Without it the round trip's eight greens would read the same whether or not a law had been broken. | **yes — passes** |

The campaign is also `world_with_extra_collateral`'s first caller. Before this it had zero, which the
close-out named as the 912-line extraction's dead surface.

Three defects were found by reading the new campaign against the route, and fixed before it was committed:

- `claims_replay_instruction` read the aggregate from `world.shared.claims_market_bytes` — the narrow
  fixture's own body, whose `custody_context` is `[0x62; 32]`. The FOUNDING writes
  `hashv([PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ticket_context])` instead, and every Custody coordinate
  hangs off that field. It now reads the chain.
- The failure-coordinate hostile seated the escrow at the frame's `POSITION` coordinate but left the
  request naming the founder's Position, so the privilege pass would have refused `Accounts` (`0x5301`)
  three stages before the conjunct under test. The request now names it too.
- The incomplete-merge hostile recomputed a caller-authority PDA the route never reaches, and — worse —
  its overreaching request was not REPRESENTABLE: `pre_hoard - (QUANTITY + 1) * scale` underflowed, so
  `to_bytes()` would have panicked in the fixture and proved something about `validate` rather than
  about the holding. It now donates to the vault first, on a categorical Market, and `plan()` reads the
  escrow through an `Option` because a categorical Market's failure escrow is derived, named in every
  frame, and never created.

### `crates/dclutch-operator/src/claims_conservation_v1.rs`

Five economics tests the wave deleted and never replaced, restored against the new surface, plus one
the fix below owes. All eight in `-p dclutch-operator --lib claims_conservation` pass; all runnable here.

| test | proves |
| --- | --- |
| `a_split_moves_quantity_times_basis_scale_and_round_trips` | `collateral_atoms == quantity * basis_scale`, both stated poststates are the prestates moved by exactly that, and the bytes decode back to a request that re-validates. Catches a scale dropped, or applied to one side only. |
| `a_merge_returns_the_same_collateral_class_it_took` | the split's poststate fed back as a second observation, merged at the same quantity, lands on the original balances; the two compartments reverse; the merge carries no `ApproveChecked` and the split's authorizes exactly `collateral_atoms`. |
| `an_uncoverable_act_refuses_rather_than_saturating` | three named refusals rather than a clamp: `Contract(ExternalBalanceMismatch)`, `Backing`, `Contract(HoardBalanceMismatch)`. |
| `a_unit_scale_hides_the_set_versus_atom_distinction_and_a_real_scale_does_not` | **the control.** At `basis_scale == 1` atoms and sets are the same number and a scale bug is invisible; at `basis_width - 1` they differ, and `held_complete_sets_after` must carry SETS while the balance delta and the approval carry ATOMS. Mutation-checked: making `held_complete_sets_after` carry atoms fails only this test. |
| `a_zero_quantity_or_scale_is_refused` | `Contract(InvalidQuantity)`, and `Basis` for a forged zero-scale record — the compiler will not write one, so a forged record is the only way a zero scale reaches the planner. |
| `a_split_past_the_carried_principal_cap_refuses_at_plan_time` | the defect below, closed. |

**Defect found and fixed while restoring them.** The module head said the planner reads "the principal
cap off Core's own state" and `plan_claims_conservation_v1`'s doc promised it "refuses on any coordinate
the route would refuse, by the same name". It read `core.principal_cap_sets` **nowhere** and never called
`ClaimsConservationRequestV1::admit_capacity`, while the on-chain route does
(`claims_conservation_v1.rs:280`). Measured before the fix: with `principal_cap_sets: 1` and an aggregate
reporting 10 outstanding sets, a 5-set split planned clean — a capped Market planned splits the chain
refuses at `PrincipalCapacity`. The planner now captures `principal_v1`'s result, reads the cap, and
refuses under a new `ClaimsConservationOperatorErrorV1::PrincipalCapacity`.

One fact the new test had to learn rather than assume: `MarketPrincipalCapSetsV1::Absent` (a zero cap) is
**unreachable through Core**. `CoreState::valid_static` (`crates/dclutch-market/src/generated.rs:517`)
requires `principal_cap_sets != 0` in every phase, so a zero cap does not encode; the route's
refuses-every-positive-count reading of `Absent` defends a bent account, not a live case.

Also fixed in the same file: the twenty-one `accounts[COORDINATE] = …` assignments the wave's rewrite
introduced in the frame assembly, each of which is a `clippy::indexing_slicing` site in a crate that
DENIES that lint. They are now seated through one checked `seat` closure.

## 5. What to distrust in `BUILD_CLAIMS_SPLIT_MERGE.md`, because it was checked

| claim | what is actually true |
| --- | --- |
| §2 S3: "**⚠ MERGE COLLISION** — the `build/founder-bond` branch edits three *other* rows of this same eleven-row table… Four edits, one table, two branches." | **False, twice.** `git diff main...build/founder-bond` does not touch `docs/decisions/0007-namespaced-refusal-codes.md` at all, and no other build branch does either. The table has **nine** rows, not eleven. The seam was applied here as an ordinary unshared one. |
| §7: "Everything else on the branch was checked green by the maker at its own commits (`dclutch-claims`, `dclutch-claims-sbf`, `dclutch-operator`, `dclutch-local-successor-bootstrap`)" | `dclutch-local-successor-bootstrap` was **red**, two errors, at `claims_conservation.rs:302-303`. Nothing on this branch had ever compiled that crate. |
| §7: the thirteenth error is because "the same commit moved `solana-sdk` and `solana-transaction` out of `[dev-dependencies]` into `[dependencies]`, and the library half resolves a different `Message` than the test half did" | There is one `Message`. `BanksClient::get_fee_for_message` takes it **by value** in `solana-banks-client-4.3.0-beta.2`, and the tree's other caller already passes by value. The borrow was simply never compiled. |
| "Rebase hazard… `tools/cohort/steps.tsv` — `main` rewrote line **85**… **Textual conflict very likely.**" | The rebase was clean. |
| §5 / B1: "`tests/claims_conservation.rs` … is now red three ways" | All three are RUN-TIME, not compile-time: the branch compiled `--tests` green before the rewrite. The 29-account frame's `assert_eq!` panics in the builder; the two refusal assertions name codes nothing raises. Nothing about it failed `cargo check`. |
| §1 / §5: "`check-steps.py --cohort 17` PASSES (41 steps)" | True and misleading, and worse than the close-out's B2 said. **No market in cohort-17 carries both fields the `split`/`merge` rows need.** Markets 0 and 1 (`direct`) have `split_collateral_account` and `split_sets` and **no `linked_basis_record`**; market 2 (`general`) has `linked_basis_record` and **no `split_collateral_account`**; market 3 (`two-source`) has `split_sets` only. So the doc's fix ("add `linked_basis_record` to markets 0 and 1") is necessary but not sufficient — the two-source market must also leave the fan-out. **This lane could not fill the gap: `--linked-basis-record` is a required argument naming a chain-discovered address, and this lane never touched devnet.** The cohort operator must read the two direct markets' linked basis records off the founding evidence and write them into `tools/cohort/cohorts/17.json`. |
| §8 step 3: the Lean's fifteen new theorems | Still never elaborated. `DClutchSemantics.lean` is edited by seven other build branches, so the import stayed unapplied here (§2) and `lake` still never sees the module. Four of the theorems lean on `native_decide` and the maker flagged `the_merge_keeps_the_vault_backing`'s `List.getElem?` step. **§1's table names them as if they hold; nothing in this tree has checked that.** |
| §1: "`complete_set_v1.rs:323 failure_selector_v1` … has zero external callers" | It has one now: the rewritten campaign reads the failure coordinate off it rather than re-spelling "the last one". The SBF founding route and the SDK affordance still derive it themselves, so the two-authors finding stands. |
