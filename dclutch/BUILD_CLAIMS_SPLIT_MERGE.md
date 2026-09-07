# BUILD — claims-split-merge

Branch `build/claims-split-merge` off `main` at `f7c03e845`. Built by source inspection; nothing here has been
compiled, built to SBF, or run against a chain. The convergence lane compiles and enmeshes it through the seams in §2.

The ruling this family executes (orchestrator, 2026-09-05, provisional): **an LBV2 Market's outstanding principal
lives in the Custody HoardPrincipal vault (the token account), not in a header scalar; ONE LBV2 complete-set executor
shared by signed_delta, affine_batch and conservation.**

## 1. What was built, file by file

### The executor (one author for every claim movement)

- `crates/dclutch-claims/src/complete_set_v1.rs` (new, ~560 lines). `no_std`, no alloc, safe.
  - `apply_coordinate_v1(bytes, header, outcome, delta)` — THE live writer of a claim on an LBV2 record: checked
    offset, checked movement, one write. `CoordinateDeltaV1 { Neutral | Credit(u64) | Debit(u64) }` with `From`
    for both batch vocabularies (`SignedDeltaV3`, `SignedMagnitudeV2`). Underflow is `Holding`, overflow `Overflow`.
  - `apply_complete_set_v1(aggregate, holder, escrow: Option, CompleteSetActV1 { direction: Mint|Merge, quantity,
    refunding })` — the kernel's `splitPost`/`mergePost` on the categorical shape and `refundingSplitPost`/
    `refundingMergePost` on the refunding one; the failure selector from `refunding_failure_index` (sole author);
    every coordinate of the aggregate ±q, the holder at ordinary coordinates, the escrow at the failure coordinate;
    all three revisions advance by one; returns `CompleteSetPostV1`.
  - `principal_v1(aggregate, vault_atoms, basis_scale)` — L4's LBV2 form: `vault_atoms >= max_k supply[k] *
    basis_scale`, an INEQUALITY (a stranger's donation to the vault cannot brick the Market). Returns the outstanding
    set count (the largest coordinate; every coordinate on an open Market).
  - `held_complete_sets_v1(position, refunding)` — min over the coordinates a merge burns (the SDK affordance's
    Rust twin, used by the operator's plan-time `Holding` refusal).
  - Nine tests: categorical round trip is the identity but for revisions; refunding seats the failure coordinate in
    the escrow and the merge burns it there; incomplete-set merge refuses `Holding` (both shapes); escrow
    required/forbidden; zero quantity / overflow; join; vault backing (donation admitted, shortfall refused,
    non-uniform supply backs its largest coordinate); held sets; both delta vocabularies convert.
- `crates/dclutch-claims/src/lib.rs` — `pub mod complete_set_v1;`.
- `programs/dclutch-claims-sbf/src/signed_delta_v3.rs` — private `apply_coordinate` (the one `burn_failure_coordinate_v1`
  also reaches) now calls `complete_set_v1::apply_coordinate_v1`; refusal stays `SignedDeltaSbfErrorV3::Candidate`.
- `programs/dclutch-claims-sbf/src/affine_batch_v2.rs` — same, refusal `AffineBatchSbfErrorV2::Candidate`.

### The route, rewritten on one family

- `programs/dclutch-claims-sbf/src/claims_conservation_v1.rs` (rewritten, ~900 lines). Reads the aggregate and both
  Positions as LBV2 only; the principal off the vault through Custody's `TokenAccount::parse_base_or_immutable_owner`;
  claims through the executor over candidate copies, committed last. Flow: decode → privileges → aggregate (PDA from
  the recorded bump, header joins, revision pin) → basis record (digest pin + `semantic_basis_id_v3 == basis_id` +
  width + `payout_scale == basis_scale`; PROGRAMS-17E's one-account proof — NO Product-graph walk) → Core (phase by
  name, then `authenticate_core_market_v3` for the product digest, release set, generation, cap) → holder + escrow
  (derived; seated on refunding) → balances (both token accounts read and held to the request's stated prestate) →
  `principal_v1` (L4) → split: cap → candidates → [split: CPI, verify balances, commit | merge: commit, CPI, verify].
  - The vault's seeds come from the contract's direction-free `hoard_vault_seeds()`. The first draft derived them
    from the Custody request's SOURCE side — on a split the External side — so every split would have refused
    `Identity` at the custody frame had CLAIMS-18 reached it (test `the_vault_seeds_do_not_depend_on_the_direction`).
  - The CPI pair is ordered by `transfer_pair` from the request Custody decodes (CLAIMS-17's defect, kept fixed).
  - `derive_hinted` reproduces the aggregate and Position PDAs from their recorded bumps (signed_delta's pattern).
  - The owner may be a writable signer (the single-wallet fee payer); the first draft pinned `!is_writable`, which
    no single-wallet submitter can satisfy.

### The contract crate

- `crates/dclutch-claims/src/conservation/mod.rs` — the twenty-nine literal offsets, the width, the magic and the
  two direction tags now come from `include!("../generated_conservation_v1.rs")`; `frame_v1` submodule owns the
  21 frame coordinates (`OWNER=0 … REALM_STAGING=20`, `CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1=21`), re-exported by
  the program and read by the operator: one table.
- `crates/dclutch-claims/src/generated_conservation_v1.rs` (new, Lean-emitted shape, hand-emitted here to the
  emitter's exact output; the guard re-emits and byte-compares).
- `crates/dclutch-claims/check-generated.sh` — third emission block (`ClaimsConservationV1Abi` →
  `EmitClaimsConservationV1Rust.lean` → `generated_conservation_v1.rs`, minimum 40 lines).

### Lean

- `formal/dclutch-semantics/DClutchSemantics/ClaimsConservationV1Abi.lean` (new): the `DCLCNS01` request as 32
  placements; `request_schema_well_formed`, `request_layout_disjoint`, `request_layout_covers_its_width` (= 592,
  tiles), `request_coordinates_are_canonical` (every literal the Rust wrote), `request_begins_with_the_shared_prologue`,
  `direction_tags_differ`.
- `formal/dclutch-semantics/EmitClaimsConservationV1Rust.lean` (new): the emitter.
- `formal/dclutch-semantics/DClutchSemantics/EconomicKernel.lean` (appended, before `end DClutch.Economic`): the
  LBV2 instance — `vaultPrincipal basisScale state = hoard * basisScale`; `vaultBacks` (L4's LBV2 form);
  `the_split_credits_the_vault_by_the_sets_collateral`, `the_merge_debits_the_vault_by_the_sets_collateral`,
  `the_refunding_acts_move_the_vault_as_the_categorical_ones_do`, `the_split_keeps_the_vault_backing`,
  `the_merge_keeps_the_vault_backing`, `the_refunding_split_keeps_the_vault_backing`,
  `the_refunding_merge_keeps_the_vault_backing`, `a_uniform_supply_is_backed_by_its_set_count`. The executor IS
  `splitPost`/`mergePost` / `refundingSplitPost`/`refundingMergePost`; the theorems it cites are named in its module
  head. NOT CHECKED with `lake` (no warm `.lake` in the worktree); the merge-backing proof's `List.getElem?` step is
  the one most likely to need a tactic adjustment.

### Operator and successor

- `crates/dclutch-operator/src/claims_conservation_v1.rs` (rewritten, ~470 lines). `plan_claims_conservation_v1(
  ClaimsConservationProgramsV1, ClaimsConservationObservedV1<'_>, ClaimsConservationActV1)` reads the chain's bytes
  (Core state, aggregate, Position, escrow, basis record, replay, vault, actor's token account, mint) and derives
  EVERY request coordinate under the LBV2 domain; refuses at plan time by the route's names (`Phase`, `Holding`,
  `Escrow`, `Backing`, `Replay`, `Token`, `Basis`, …); emits the `ApproveChecked` (via the tree's one Token author,
  `dclutch_custody::token_svm::instruction::approve_checked`) + conserve pair over the 21-account frame; the caller
  authority PDA seeded by the SHA-256 of the exact Custody wire (delegated for a split, plain for a merge).
- `tools/local-validator/bootstrap/successor/src/claims_conservation.rs` (new, ~520 lines): four verbs —
  `devnet-split-v1`, `devnet-merge-v1`, `local-private-validator-split-v1`, `local-private-validator-merge-v1` —
  one `run`; reads at finalized, calls the operator, dry-runs by default, `--execute --owner-keypair` sends
  (owner signs and pays), reads the poststate back and holds it to the request's stated figures, writes
  `dclutch-claims-conservation-evidence-v1` with before/after blocks. Wired into `main.rs` (mod + four arms + usage)
  and the journey's `#[path]` list.

### Runbook

- `tools/cohort/steps.tsv` — rows `split` and `merge` (since 17, shape `once`, args `bootstrap devnet-split-v1 …` /
  `devnet-merge-v1 …`; `split` blocks `merge`); verifiers read the vault, aggregate, Position, escrow and replay back
  against the act's own signature. `tools/cohort/README.md` — `### split`, `### merge`. `tools/cohort/cohorts/17.json`
  — `split_sets` and `split_collateral_account` on the two direct markets. `check-steps.py --cohort 17` PASSES
  (41 steps) and `--prove-frozen` still reproduces both frozen tables.

### Program-test — the founding world extracted (the work in flight at the wall)

- `programs/dclutch-claims-sbf/program-test/fractional-atomic/src/founding_world.rs` (**new, 912 lines**,
  uncommitted when the wave stopped; committed by CLOSEOUT at `a224126b7`). The whole founding fixture lifted
  out of `tests/claims_founding.rs` (which shrank 849 lines, 1,224 → 375) so that a second campaign — the
  split/merge one — can stand up the same world instead of re-encoding it. Public surface: the eight program
  ids and the fixture constants (`CLAIM_COUNT = 4`, `GENERATION = 41`, `QUANTITY = 7`, the four context
  digests, `REFUND_WALLET`, `RENT_BENEFICIARY`); `FoundingShapeV1 { … }` with `basis()` and `seats_escrow()`;
  `HostileV1`; `FoundingWorld` (:190); `world(shape, hostile) -> (ProgramTest, FoundingWorld)` (:215) and
  `world_with_extra_collateral` (:221); `founding_instruction(&FoundingWorld) -> Instruction` (:731);
  `Outcome` (:791); and the async drivers `submit` (:802), `submit_with` (:816), `found` (:902).
- `src/lib.rs` — `pub mod founding_world;` **DONE** (third line of the module list).
- `Cargo.toml` — `dclutch-program-test-evidence`, `solana-sdk = "=3.0.0"` and `solana-transaction = "=4.2.0"`
  moved from `[dev-dependencies]` up into `[dependencies]`, with the reason written at the site: the founding
  world submits transactions and records evidence, so it needs them from `src/`, not only from `tests/`.
- `tests/claims_founding.rs` — now imports `dclutch_fractional_atomic_program_test::founding_world::{…}`
  (:102) and keeps five tests (§5).

**This file does not compile — see §7. One import is why.**

### SDK, web — NOT BUILT on this branch.

S14–S16 name them; the subagent that was to land `packages/dclutch-sdk/lib/claimsConservation.ts`,
`scripts/generate-claims-conservation-v1.mjs` and `<MarketCompleteSetPanel/>` never wrote a file. There is
nothing to review and nothing to delete: the three seams are the whole record.

## 2. THE SEAMS (exact file:line, one-line change)

| # | Where | Change |
|---|---|---|
| S1 | `programs/dclutch-claims-sbf/src/lib.rs:590` | dispatcher arm unchanged (`is_claims_conservation_v1` → `process`); NOTHING to do, but the arm's comment cites the migration-only route below it — leave. |
| S2 | `programs/dclutch-claims-sbf/src/lib.rs` refusal census | the new `ClaimsConservationSbfErrorV1` (0x5300–0x530C) is `#[repr(u32)]` + `pin_refusal_band!` in `claims_conservation_v1.rs`; `tools/gate census` (`dclutch-route-census inventory --check-unique`) picks it up by scanning; if the inventory keys sub-bands by file, add the file. |
| S3 | `docs/decisions/0007-namespaced-refusal-codes.md:129-139` | add row `| ClaimsConservationSbfErrorV1 | — | 0x5300–0x530C |` to the Claims sub-band table (the enum is at `programs/dclutch-claims-sbf/src/claims_conservation_v1.rs:126`, `Instruction = 0x5300` at `:128` through `Commit = 0x530C` at `:162`, contiguous). **⚠ MERGE COLLISION** — the `build/founder-bond` branch edits three *other* rows of this same eleven-row table: `ClaimsSbfError` `0x5000–0x500B` → `0x5012`, `ClaimsFoundingSbfErrorV5` `0x5180–0x5190` → `0x5191`, `ClaimsMarketClosureSbfErrorV1` `0x5500–0x5505` → `0x5507`. Four edits, one table, two branches. Land them in one commit; a textual merge will resolve cleanly and still be wrong if either branch's rows are dropped. |
| S4 | `docs/reference/refusals.md` / `routes.md` | generated: `tools/gate reference --converge` at convergence; route `claims/claims_conservation_v1::process` keeps magic `DCLCNS01`, phase gate `market: Open`. |
| S5 | `tools/gates/frames-baseline.json:1255-1327` | the thirteen `claims_conservation_v1::*` symbols change (rewritten route; `authenticate_market_and_records`, `apply_economics`, `escrow_revision_of` are gone; `authenticate_aggregate`, `authenticate_basis_record`, `authenticate_core`, `authenticate_balances_and_backing`, `build_candidates`, `verify_balances_after`, `token_balance` appear); recapture with `tools/gate frames --at <commit> --capture`. Claims ELF moves; `signed_delta_v3`/`affine_batch_v2` frames may move by the executor call (one extra frame each, tiny). |
| S6 | `tools/gates/emission-coverage.md:14` (guard row) and the generated-file table | **CONFIRMED REAL**: the branch's third block IS in `crates/dclutch-claims/check-generated.sh` (`:58`, `:62` — `verify EmitClaimsConservationV1Rust.lean generated_conservation_v1.rs 40`), and `tools/gate guards` will run it, but row `:14` still reads `EmitClaimsLiabilityBasisStateV2Rust.lean, EmitClaimsMarketClosureV1Rust.lean` — the new emitter is invisible to the census. Regenerate with `tools/gate emission --write` (the file is byte-gated by `--verify`; never hand-edit). The header count moves by one file and one emitter. |
| S7 | `formal/dclutch-semantics/DClutchSemantics.lean:22` | **CONFIRMED REAL** (CLOSEOUT-B): the root imports every module explicitly (`import DClutchSemantics.ClaimsLiabilityBasisStateV2Abi` is at `:22`) and `ClaimsConservationV1Abi` **is not there**. Add `import DClutchSemantics.ClaimsConservationV1Abi` in alphabetical position. Until it lands the module is not in the library and `lake build` never elaborates it — which is also why §1's "NOT CHECKED with `lake`" has stayed true by construction. |
| S8 | `crates/dclutch-claims/src/conservation/tests.rs` | tests that name the removed literal consts compile unchanged (same identifiers via `include!`); if any test asserted `CLAIMS_CONSERVATION_REQUEST_MAGIC_V1 == *b"DCLCNS01"` it still holds (the emitted bytes are that string). |
| S9 | `programs/dclutch-claims-sbf/Cargo.toml` | no new deps (`dclutch-custody` already; `TokenAccount` from `token_svm`). |
| S10 | `crates/dclutch-operator/Cargo.toml` | no new deps (`dclutch-custody` token_svm instruction, `dclutch-registry`, `dclutch-product` svm already). |
| S11 | `tools/local-validator/bootstrap/successor/src/main.rs:14,250,2348` | DONE on the branch: `mod claims_conservation;`, four match arms before the replay arms, usage line. |
| S12 | `tools/gauntlet/journey/src/main.rs:72` | DONE: `#[path]` module `claims_conservation` beside `claims_custody_replay`. |
| S13 | `tools/cohort/README.md` command index (if a table lists successor verbs) | add the two verbs; `check-steps.py` is already green. |
| S14 | `packages/dclutch-sdk/index.ts` | `export * from './lib/claimsConservation';` (subagent B lands it; verify). |
| S15 | `packages/dclutch-sdk/package.json` generate/verify script lists | register `scripts/generate-claims-conservation-v1.mjs` where `generate-claims-custody-replay.mjs` is (subagent B). |
| S16 | `apps/dclutch-web/components/MarketDetailWorkspace.tsx:~945` | mount `<MarketCompleteSetPanel …/>` after `<MarketTradePanel …/>` (subagent B). |
| S17 | `tools/gauntlet/claims-fractional-atomic/bindings.json` | AFTER the first green fold of `tests/claims_conservation.rs`, author bindings for `claims/claims_conservation_v1::process` (executed + refused) from the ledger, never from what the campaign ought to touch. |
| S18 | `tools/gauntlet/journey/src/ledger.rs` | the program-test carries its own `ConservationCensusV1` (L1–L8 restated); convergence should lift `ledger.rs` into a lib both consume — named debt, not hidden. |
| S19 | `crates/dclutch-operator/src/lib.rs:57` | `pub mod claims_conservation_v1;` exists; the module's public surface changed (input/plan types renamed) — grep for `ClaimsConservationInputV1` consumers (none on main besides its own tests). |

## 3. Ruled provisionally (ember rules by reversal)

1. **The principal is READ, never stored, and L4 is an inequality.** `vault_atoms >= max supply * basis_scale`.
   A donation to the vault is excess backing; a shortfall refuses `Backing 0x5307` before any act.
2. **No Product-graph walk on split/merge.** The linked basis record's bytes prove the shape, width and scale
   (PROGRAMS-17E's `semantic_basis_id_v3` finding); Core's identity proves the product record digest. Frame 29 → 21,
   ~40k CU saved per act. Reversal cost: seven accounts back in the frame and `authenticate_runtime_product_basis_core_with_rent_v3`.
3. **Sub-band 0x5300 for the route's own refusals; the two escrow codes stay `ClaimsSbfError`'s 0x5010/0x5011.**
   One accusation, one code across founding, the batch gate, the closure and this route.
4. **No fee on split or merge** (decision 0024: no take before mainnet). The request has no fee field; adding one
   is a wire change under 0007's ratchet.
5. **The owner pays the fee and signs alone**; the successor verbs take `--owner-keypair` only. A sponsored split is
   a later shape (the frame admits a distinct fee payer already, since the owner's writability is free).
6. **The Claims-role replay is a precondition**, not created by the route (`devnet-claims-custody-replay-v1` first);
   the verb refuses by name with that remedy.
7. **The categorical frame still names the escrow slot** and requires the derived address (vacant is fine).
8. **Revisions advance inside the executor** (aggregate, holder, escrow) — the act is one transition.

## 4. Stubbed, and why

- Nothing is `todo!()`. Two facts are deliberately NOT built:
  - No route receipt / `set_return_data`: the transaction's own `DCLCNS01` bytes plus Custody's receipt are the
    evidence; a Claims receipt would be a third author of the same numbers. Add one if the census needs it.
  - `FailureEscrowUnseated` on this route is reachable only from a fixture that founds refunding and then unseats
    the escrow, which no route can do; the code stays for the founding-time shape and is not hostile-tested here.

## 5. Tests written and what each would prove

**Native, in `crates/dclutch-claims`** (the executor's own; the maker ran them green before the rewrite of
the route):

| test | proves |
| --- | --- |
| `complete_set_v1::tests` (nine) | the categorical round trip is the identity but for the three revisions; a refunding split seats the failure coordinate in the escrow and a merge burns it there; an incomplete-set merge refuses `Holding` in BOTH shapes; escrow required / escrow forbidden; zero quantity and overflow; the join; vault backing (a donation is admitted, a shortfall refused, a non-uniform supply is backed by its LARGEST coordinate); `held_complete_sets_v1`; both delta vocabularies convert. |
| `the_vault_seeds_do_not_depend_on_the_direction` (in `programs/dclutch-claims-sbf/src/claims_conservation_v1.rs`) | the seed bug the first draft had: deriving the vault from the Custody request's SOURCE side made every split refuse `Identity` at the custody frame. This test is the only thing standing between that defect and its return. |

**Program-test, on the real ELF** — `tests/claims_founding.rs`, five tests, all standing on the new
`founding_world`:

| test | proves |
| --- | --- |
| `a_refunding_founding_seats_the_escrow_on_the_real_elf` (:116) | the refunding founding seats the failure escrow — the precondition every split/merge conservation act reads. |
| `a_categorical_founding_leaves_the_escrow_accounts_vacant` (:213) | the categorical shape seats nothing, so the route's "escrow forbidden" arm is reachable from a real founding and not only from a unit fixture. |
| `the_two_shapes_differ_in_the_payout_scale_and_nothing_else` (:263, sync) | the two `FoundingShapeV1` arms are one bit apart — the control that makes the two tests above a comparison rather than two unrelated worlds. |
| `a_refunding_founding_whose_escrow_is_not_the_markets_own_refuses` (:317) | hostile: a foreign escrow. |
| `a_refunding_founding_whose_escrow_rent_is_not_prepaid_refuses` (:344) | hostile: an underfunded escrow. |

### The two ELF program-tests this branch turns RED

**Correction (CLOSEOUT-B): `tests/claims_conservation.rs` DOES exist** — 39 KB, untouched by the branch,
and it is the CLAIMS-18 witness campaign whose finding this family's ruling repeals. It is now red three
ways:

1. `:600-624` builds the **29-account** frame with the seven Product-graph accounts, and `:626-630`
   asserts `accounts.len() == dclutch_claims_sbf::claims_conservation_v1::CLAIMS_CONSERVATION_ACCOUNT_COUNT_V1`
   — a constant that is now **21**. The builder panics before any submission.
2. `:743 a_conserving_split_on_a_founded_refunding_market_refuses_economic` expects `ClaimsSbfError::Economic`
   (`0x5005`). **Nothing in the tree raises `Economic` any more.**
3. `:776 the_same_frame_with_an_economic_slice_aggregate_refuses_identity` expects `ClaimsSbfError::Identity`
   (`0x5002`); the route's own refusals are now `0x5300–0x530C`.

Its module head (`:25-45`) still narrates the 29-account two-reader disagreement as current.

**This file is what `founding_world.rs` was extracted FOR, and the extraction never reached it** — see
§1's dead-surface note: `world_with_extra_collateral` (`founding_world.rs:221`), the sole reason the
912-line lift happened, has **zero callers**. Rewriting `claims_conservation.rs` onto it is the same
step as making the extraction pay for itself.

So the honest statement of the gap: **the route this family is about is exercised by one program-test
that asserts the defect the family repealed, and by nothing else.** That is larger than the compile
error in §7, and it is the thing S17 (gauntlet bindings, gated on "the first green fold") is waiting on.

### Net test balance, and what left with the rewrite

**+17 added** (9 executor, 3 SBF route, 2 operator, 3 successor), **−7 deleted**, **2 ELF program-tests
turned red**. The deletions are not all fallout:

- SBF route, two deleted (`the_escrow_takes_the_slot_the_categorical_action_leaves_empty`,
  `collateral_that_is_not_the_exact_product_refuses`) — frame/product-graph assertions the 21-account
  rewrite invalidated. **Neither was replaced.**
- Operator, **five** deleted (`a_split_moves_quantity_times_basis_scale_and_round_trips`,
  `a_merge_returns_the_same_collateral_class_it_took`, `an_uncoverable_act_refuses_rather_than_saturating`,
  `a_unit_scale_hides_the_set_versus_atom_distinction_and_a_real_scale_does_not`,
  `a_zero_quantity_or_scale_is_refused`), two added (`the_frame_constants_are_the_routes_own`,
  `a_zero_coordinate_refuses_by_name`). **The economic content of the operator's test suite was removed
  and replaced by two shape assertions** — net −3 over a rewritten 749-line module. The scale-vs-atom
  distinction in particular is exactly the confusion `principal_v1`'s `basis_scale` multiply can
  reintroduce. Restore those five against the new surface.

## 6. Frames and codes expected to move; the cohort

- Claims ELF moves (the route, the two executor calls). Custody, Core, Registry unchanged. Cohort-17 carries the
  Claims link (it is already re-releasing Claims for PROGRAMS-17E); `split`/`merge` rows are `since 17`.
- Codes: thirteen new at 0x5300–0x530C; none renumbered. `CustodyRequired 0x5006` is now raised by nothing (the
  route's Custody wire refusals are `CustodyWire 0x530A`); it stays allocated (0007: bands are append-only) and the
  refusals reference should mark it withdrawn.

## 7. Does it parse? (CLOSEOUT-B, 2026-09-06)

`cargo check -p dclutch-fractional-atomic-program-test --offline` on `a224126b7`: **RED, 13 errors.**
Everything else on the branch was checked green by the maker at its own commits (`dclutch-claims`,
`dclutch-claims-sbf`, `dclutch-operator`, `dclutch-local-successor-bootstrap`); this is the one crate the
wave was mid-write in when the quota ran out.

The first three, verbatim:

```
error[E0433]: cannot find module or crate `dclutch_fractional_atomic_program_test` in this scope
  --> programs/dclutch-claims-sbf/program-test/fractional-atomic/src/founding_world.rs:37:5
   |
37 | use dclutch_fractional_atomic_program_test::{
   |     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ use of unresolved module or unlinked crate `dclutch_fractional_atomic_program_test`

error[E0308]: mismatched types
   --> programs/dclutch-claims-sbf/program-test/fractional-atomic/src/founding_world.rs:846:30
    |
846 |         .get_fee_for_message(&transaction.message)
    |          ------------------- ^^^^^^^^^^^^^^^^^^^^ expected `Message`, found `&Message`

error[E0308]: mismatched types
   --> programs/dclutch-claims-sbf/program-test/fractional-atomic/src/founding_world.rs:415:70
    |
415 |         &CustodyAuthoritySeedsV1::new(shared.core_market.to_bytes(), release_set).as_slices(),
    |          ----------------------------                                ^^^^^^^^^^^ expected `[u8; 32]`, found `[u8]`
```

**All thirteen are two mistakes, and eleven of them are one.** The extraction moved the file from `tests/`
into `src/` and left its imports in the `tests/` idiom: at `src/founding_world.rs:37` it does
`use dclutch_fractional_atomic_program_test::{campaign_support::…, narrow_fixture::…}` — importing its own
crate by name, which resolves from an integration test and never from inside the library. That unresolved
import poisons `activation_cache` (whose real signature is
`campaign_support.rs:243: pub fn activation_cache(input: &ReleaseSetInputV1<'_>) -> ([u8; 32], Vec<u8>)`),
so the `release_set` bound from it degrades to `[u8]` and every one of its eleven uses (`:415, :424, :435,
:456, :475, :493, :530, :584, :605, :655, :707`) reports `expected [u8; 32], found [u8]`. **The fix is one
line**: `use crate::{campaign_support::{…}, narrow_fixture::{…}};`. Do not "fix" the eleven type errors —
they are not real.

The thirteenth is independent: `:846`'s `banks.get_fee_for_message(&transaction.message)` now wants
`Message` by value, because the same commit moved `solana-sdk = "=3.0.0"` and `solana-transaction = "=4.2.0"`
out of `[dev-dependencies]` into `[dependencies]`, and the library half resolves a different `Message` than
the test half did. Pass `transaction.message.clone()`, or keep the borrow and re-check which `Message` the
`[dependencies]` copy names.

## 7.1 Three more breaks that no `cargo check` can see (CLOSEOUT-B)

| # | where | what happens |
|---|---|---|
| **B1** | `programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/claims_conservation.rs` | red three ways — see §5. Untouched by the branch. |
| **B2** | `tools/cohort/steps.tsv:87, :88` vs `tools/cohort/cohorts/17.json` | the `split`/`merge` rows name `{market.linked_basis_record}` and `{market.split_collateral_account}`. Rows absent from `MARKET_KIND` fan out over `DEFAULT_KINDS = ("direct","two-source")` (`generate-stage-scripts.py:110`) — markets 0, 1 and 3. But `markets[0]` (`:112-113`) and `markets[1]` (`:165-166`) have `split_collateral_account` and **no `linked_basis_record`**, and `markets[3]` (`market-s`, `:235`) has `split_sets` **only**. `generate-stage-scripts.py:521` raises `market 'X' has no field 'market.linked_basis_record'`. **`check-steps.py` cannot catch this** — `{market.*}` is `EMIT_TIME` (`:78-81`), shape-checked only, which is why §1's "`check-steps.py --cohort 17` PASSES (41 steps)" is true and misleading. Fix: add `linked_basis_record` to markets 0 and 1, and either give `market-s` both fields or add `"split": ("direct",), "merge": ("direct",)` to `MARKET_KIND`. |
| **B3** | `docs/reference/routes.md:139`, `docs/reference/route-witnesses.md:235`, `tools/gauntlet/blocked.json` | the route reads `NEVER-EXECUTED, no stated reason`. Either land the bindings (S17, gated on B1) or record a `{"route":"claims/claims_conservation_v1::process","class":"unwired","reason":…}` row. An unexplained never-executed route is the state 0007's discipline exists to prevent. |

Two corrections to §2's own rows: **S6**'s census figures are `91 files / 85 emitters → 92 / 86` (not
101 → 102), with the guard row at `emission-coverage.md:14` and a generated-file row inserted at `:91`.
**S18 is stale**: `ConservationCensusV1` does not exist anywhere in the tree, on either side — the debt
is not "lift the program-test's census into a shared lib", it is "no census type exists yet".

### Rebase hazard — `main` has moved 17 commits past the base

The branch's base is `f7c03e845`; `main` is now `131e084da`. Three of the nine files `main` changed
overlap this branch:

- `tools/cohort/steps.tsv` — `main` rewrote line **85** (the `retire` row's verifier: `a7cffbcb1`,
  `388317b6d`, `62512032b`). The branch's insertion hunk is `@@ -84,6 +84,8 @@`, whose context includes
  that line. **Textual conflict very likely.**
- `tools/cohort/README.md` — `main` inserted +64 lines at `@@ -680 @@`; the branch inserts at 819.
  Auto-merges, but every line number cited in §2 above 680 shifts.
- `tools/local-validator/bootstrap/successor/src/main.rs` — `main` added three arms at `@@ -209 @@` and a
  usage line at `@@ -2317 @@`; the branch adds at 251 and 2348. Auto-merges (>3 lines apart), but S11's
  cited numbers shift by +4.

Also re-check `docs/evidence/witnesses/cohort-17-discovered.json` and `docs/ledger/2026-09-06.md`, both
moved on `main`, for claims about cohort-17 the two new runbook rows contradict.

### Dead surface

`founding_world.rs` exports the whole founding fixture and `tests/claims_founding.rs` imports **seven**
names from it (`CLAIM_COUNT`, `CLAIMS_PROGRAM_ID`, `FoundingShapeV1`, `HostileV1`, `QUANTITY`, `found`,
`world`). Everything else is dead until B1 lands — most pointedly `world_with_extra_collateral` (`:221`),
**the sole reason the extraction happened**. (The identically-named constants in `fractional_atomic.rs`,
`fractional_compaction.rs` and `claims_conservation.rs` are each file's own local copies, not imports —
they look like callers and are not.) In the executor, `complete_set_v1.rs:323 failure_selector_v1` — the
declared "sole author of the failure selector" — has zero external callers: the SBF founding route and
the SDK affordance still derive it themselves. That is two authors, which is the thing this family is
against.

## 8. STOPPED HERE — the next three steps, concretely

1. **Fix `founding_world.rs:37` to `use crate::{…}`, settle `:846`'s `Message`, and re-check.** One line
   clears eleven of thirteen errors. Until this compiles, `tests/claims_founding.rs` cannot run, so the
   branch's five founding program-tests — which establish the escrow precondition the whole conservation
   route reads — are proving nothing. Cheapest high-value step on any of the four branches.

2. **Rewrite `tests/claims_conservation.rs` onto `founding_world::world_with_extra_collateral`.** This is
   one step, not two: it turns the branch's only ELF witness from red to green, it makes the 912-line
   extraction pay for itself, and it is what S17's gauntlet bindings are gated on. Build the split (the
   `ApproveChecked` + conserve pair over the 21-account frame, then the vault, aggregate, both Positions
   and the escrow read back) and its inverse merge, and assert `principal_v1`'s inequality survives a
   stranger's donation to the vault. Delete the module head's 29-account narration and the two tests that
   expect `Economic`/`Identity`; the route's codes are `0x5300–0x530C` now. **Only then are the thirteen
   new codes observed rather than declared** — and only then can `blocked.json` stop saying
   NEVER-EXECUTED (B3).

3. **Check the Lean and restore the operator's economics tests.** `EconomicKernel.lean`'s nine new LBV2
   theorems and `ClaimsConservationV1Abi.lean`'s six were written **without a warm `.lake`** and have
   never been elaborated; four lean on `native_decide`, and the maker flagged
   `the_merge_keeps_the_vault_backing`'s `List.getElem?` step. §1's table cites them by name as if they
   hold. Add `import DClutchSemantics.ClaimsConservationV1Abi` at `DClutchSemantics.lean:22` (S7) and run
   `crates/dclutch-claims/check-generated.sh` — its third block re-emits `generated_conservation_v1.rs`
   and byte-compares, and **that file was hand-written to the emitter's expected output, never emitted**.
   If they differ, the offsets the route reads are wrong. In the same pass restore the five operator
   tests §5 says were deleted: the scale-vs-atom distinction is exactly what `principal_v1`'s
   `basis_scale` multiply can silently reintroduce.

Then B2 (the cohort-17 market fields, which `check-steps.py` structurally cannot catch), the rebase onto
`131e084da` (expect a conflict at `steps.tsv:85`), S3 (the 0007 row — **coordinate with
`build/founder-bond`, which edits three other rows of the same table**), S6 (`tools/gate emission --write`,
91/85 → 92/86), S5 (`tools/gate frames --capture` — thirteen symbols out, eleven in), S4
(`tools/gate reference --converge`, which also covers the thirteen refusal rows across five generated
files and re-runs `generate-market-phase-admission.mjs`), and the withdrawal note for
`ClaimsSbfError::CustodyRequired 0x5006`, which this branch leaves raised by nothing.

Not started at all: the SDK (`lib/claimsConservation.ts`, `scripts/generate-claims-conservation-v1.mjs`,
the `package.json` script pair, the `index.ts` export) and the web (`MarketCompleteSetPanel`). S14–S16
are the whole record of them.
