# The build wave's merge — 2026-09-07

Eleven families were declared READY by the convergence wave of 2026-09-06
(`docs/ledger/2026-09-06.md`, "THE CONVERGENCE WAVE"). **Ten landed on `main`.
One was aborted and skipped.** A twelfth, `recovery-capture`, was still
converging and was never this queue's.

Every landing is a `--no-ff` merge, each green before the next started, each
published separately through `tools/cut.sh`. The queue began at `67ecd841b`
and ends at `09068cfa9`.

## 1. What landed, in order

| # | family | branch head | merge | cut | conflicts |
|---|---|---|---|---|---|
| 1 | `deploy-lifecycle` | `0f9352009` | `31de5e2bd` | `9284f45c6` | none |
| 2 | `failure-arm` | `84160212d` | `74a2d00c8` | `d2789cb9e` | 1 |
| 3 | `general-lifecycle` | `b65da2465` | `4bfe6c6b2` | `c88ed58d3` | none |
| 4 | `claims-split-merge` | `dc5eb6e4a` | `367749979` | `411e913fb` | 1 |
| 5 | `economics` | `070ed5425` | `5042f9a08` | `bb26f05f5` | 1 |
| 6 | `founder-bond` | `4276e0216` | `98eb0deb8` | `d2dad1d52` | 8 |
| — | **`joint-clearing`** | `8a44429f5` | **ABORTED** | — | 2 resolved, then §3 |
| 7 | `dealer` | `c54079b25` | `1ec574ce5` | `98b45714b` | none |
| 8 | `series` | `c8ddfdb2f` | `ff082ad91` | `0eeebae32` | none |
| 9 | `product-shapes` | `eaa77b868` | `d5768f5d1` | `abf3719d2` | 2 |
| 10 | `batch-spine` | `cb02c7281` | `b5f277e63` | `5ad100d1e` | none |

The order was the wave's own advice and it paid for itself twice.
`deploy-lifecycle` went first because three branches claimed
`ResolutionError = 0x801E` and it took one number where the others took three
and ten; on landing 9 exactly one branch moved exactly ten numbers.
`failure-arm` went second so it could own `ClaimsSbfError = 0x5012` and leave
`founder-bond` the contiguity refusal at landing 6. `general-lifecycle` went
third because it carries cohort-18's General envelopes, which two later
families' runbook rows are selected into. `batch-spine` went last because
landing it is a deploy gate (§5).

## 2. The conflicts, and how each was resolved

**Refusal codes.** Two collisions, both resolved by the rule decision 0007
states: a code that has never reached a chain or a published reference is not
yet a fact, so it may be renumbered, and a code that has shipped may not.

* `ClaimsSbfError`: `failure-arm`'s `Overdraw` kept `0x5012`;
  `founder-bond`'s `FounderBondFrame` took `0x5013`. It happened exactly as
  `founder-bond`'s notes predicted — both branches append last in the enum and
  last in the `pin_refusal_band!` list, so the textual merge put two variants
  at one discriminant and rustc refused with *discriminant value assigned more
  than once*. Fail-closed by construction; the merge was one literal. The macro
  then caught a second thing on its own: the union of two single-name hunks in
  the pin list leaves a missing comma.
* `ResolutionError`: `deploy-lifecycle`'s `FundedRent` kept `0x801E`, and
  `product-shapes`'s ten moved to `0x801F`–`0x8028`, contiguous behind it.
  Checked rather than assumed: nothing else in the tree cites any of those
  eleven numbers as a literal — every reader derives them from the enum or the
  registry, and the Lean grammar names them rather than numbering them.

**Decision 0007's Claims sub-band table.** Three families extend it and none
wrote it. Rewritten once at landing 6 from the enums themselves rather than
from any family's notes: `ClaimsSbfError` `0x5000`–`0x5013`,
`ClaimsFoundingSbfErrorV5` `0x5180`–`0x5191`, `ClaimsMarketClosureSbfErrorV1`
`0x5500`–`0x5507`, with `ClaimsConservationSbfErrorV1` `0x5300`–`0x530C`
confirmed unchanged. `claims-split-merge`'s BUILD doc claimed a collision here
with `founder-bond`; the wave already recorded that claim as FALSE and it is.

**`tools/local-validator/bootstrap/successor/src/main.rs`.** Four families add
`mod` lines, dispatch arms and `usage()` lines to the same three places. Two
textual conflicts (landings 2 and 4), both resolved as the union, in landing
order.

**`crates/dclutch-operator/src/lib.rs`.** A sorted `pub mod` list; three
families insert. Union each time.

**`formal/dclutch-semantics/DClutchSemantics.lean`.** Seven branches add import
lines and the union is the answer, as the brief said. One correction beyond
that: `economics` inserted `UpkeepVaultV1` between `ProtocolParametersV1` and
`RationalCrossDomainV3`, and the list is alphabetical — an out-of-order line
makes every later insertion a conflict, so it was moved after `TsEmit`.

**`tools/gauntlet/journey/src/journey.rs`** (landing 2). Both sides write the
same `track_market` call on the same field: main had already fixed the field
name that `failure-arm`'s notes report as main's own red, and main carries a
two-line comment saying which of the two markets L4 retires on. Main's comment
kept, the branch's identical call taken.

**`claims_founding.rs` and the founding world** (landing 6, the real one).
`founder-bond` grew the founding world's fixture INLINE in the test file while
`claims-split-merge`, three landings earlier, EXTRACTED that world into
`programs/dclutch-claims-sbf/program-test/fractional-atomic/src/founding_world.rs`
and made it a library. A textual merge of the two is a thousand-line conflict
between the same code in two places, so it was rebuilt rather than chosen:
`founding_world.rs` gained the two bond hostiles, the derived `bond` field and
the escrow funding that pays rent PLUS bond, and `claims_founding.rs` is main's
extracted version with the branch's assertions and two campaigns re-applied on
top. The escrow's rent is `world.position_rent` rather than a second field
holding the same number, which is what the extraction was for.

**`marketDetail.ts` / `MarketActivity.tsx` / `marketDetail.test.ts`** (landing
6). `failure-arm` and `founder-bond` both add exports and both pass a different
argument to the same `outageDisclosureV1` call; the SDK signature already
accepts both, so the union loses neither sentence. One reconciliation beyond
the union: `failureEscrowAccountsV1` and `failureEscrowV1` derived the same
Position/admission pair from the same seeds in two places, so `failureEscrowV1`
calls `failureEscrowAccountsV1` now and the pair has one author.

**`tools/cohort/steps.tsv`.** Three families insert rows; `series` withdrew its
four and left the file byte-identical to main, which is the only family in the
wave that shrank the contested file. One conflict, at landing 6: main rewrote
the `retire` row wholesale and `founder-bond` appended one sentence to its
verifier — main's row verbatim with the sentence appended.

**`packages/dclutch-sdk/lib/generated/routeCensus.ts`.** Generated. Conflicted
at landing 9; main's taken and the whole surface regenerated once at the end
(§4), which is also what happened to every `docs/reference/*` page.

**`tools/frameguard/baseline.json`** was never touched: the file in this tree is
`tools/gates/frames-baseline.json`, several branches leave the ratchet red on
purpose and say so, and no capture was made (§5).

## 3. The family that was skipped, and exactly why

**`joint-clearing` (`8a44429f5`) was merged, resolved, and then aborted.**

Its two textual conflicts were resolved and are worth recording because the
resolution is the interesting part: `general-lifecycle` had split the coarse
`GeneralHotCandidateErrorV3::InvalidPlan` into nine clause enums — its
headline work — while `joint-clearing` renamed the V1 records to V2 and kept
the coarse code. Every one of the seven hunks in `hot_candidate_v3.rs` differed
in exactly that way, so the resolution was main's refusal clauses with
joint-clearing's types. Taking either side whole would have either lost the V2
rename or re-coarsened nine refusals, and AGENTS is explicit that a
`map_err(|_| Coarse)` converts a located defect into a search.

What could not be resolved is that **the family hands the merge lane an
unfinished migration in four program-test files it never compiled**:

* `VerifiedCandidateV2::encode_into` gained a leading `prices` argument;
* `CandidateHeaderV2` gained `live_order_count`;
* `GeneralOrderHeaderV2` gained `side`, `outcome_lo`, `outcome_hi` and
  `claims_per_lot`;
* `runtime_manifest_orders_for_row_v2` gained an `execution_lots` argument.

Three of the four call-site classes were patched here (both accelerator
fixtures' price tails, three sites in `lifecycle.rs`) before the remainder came
into view: `programs/dclutch-trading-sbf/program-test/bundle-builder/tests/general_dynamic_spans_v1.rs`,
`programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs` and
five more sites in `programs/dclutch-accelerator-sbf/program-test/tests/lifecycle.rs`
each need an `outcome_lo`/`outcome_hi`/`claims_per_lot` decision, and by the
family's own §3.1 the joint arm made the row shape SINGLE-OUTCOME while every
one of main's fixtures uses an all-outcomes vector. That is not a merge
resolution; it is authoring what those walks assert, in files the family did
not touch, and getting it wrong ships tests that measure something other than
their name — which is the failure this whole wave was convened to remove.

Three further facts make skipping the right call rather than the tired one:

1. **The family declares itself NOT READY** in its own notes' §1. It is the
   only one of the eleven that does.
2. **Its two on-chain writes have no frame to happen in.** `Action::Close`
   declares four child frames and none is a ProtocolPosition mutation, so the
   strand's burn is refused fail-closed; and the batch account is not in the
   65-account profile at all, so `GeneralBatchV2::clear` cannot be called by
   the settlement that describes it. Landing it would move a great deal of code
   and execute nothing.
3. **It hands on a second unfinished migration** (§1.1b of its notes): it made
   the Batch envelope Product-width in `local_state_v3.rs` and left five
   physical-account sites at the old 224 with a zero stride. Its own note says
   those sites' tests "pass today only because nothing cross-checks them against
   the envelope."

**Work done during the aborted attempt that a future lane should not redo, and
should not trust either.** Three arms for a `BasketAction::StrandResidual` were
written from `EconomicKernel.strandPost` — shape `(source, no destination)`, the
same three debits as a terminal redemption, and **the Hoard does not move and
the payout is zero** (`strand_conserves_hoard`, `strand_keeps_full_backing`).
They are NOT in the tree: `git merge --abort` discarded them. They are recorded
here because the derivation is the reusable part.

The refusal codes the family asks for — `ClaimsSbfError::Strand` and
`StrandUnseated` — were deliberately NOT allocated. Nothing in the merged tree
raises either, and a refusal code with no producer is the exact pattern the
wave's own families spent the night diagnosing.

`batch-spine` inherits an obligation from this: `DirectRfqV1.lean` names
`ClearingPriceV1` in prose as the persistence surface for the derived price,
and `ClearingPriceV1Abi.lean` is defined only on `build/joint-clearing`. That
reference is unreal until joint-clearing lands.

## 4. What was regenerated, and what that proved

**`tools/gate reference --converge`**, once, from a detached worktree at
`bebac8972` (the gate reads the working tree and refuses a dirty one). Three
passes: thirteen files moved, then three more — the reference and the client
mirrors close a cycle, which is why one pass is never enough — then nothing.
Committed as `9b2d22af8`, and `tools/gate reference --check --converge` at that
revision reports *"9b2d22af8708 is already the fixpoint"*.

**149 → 160 routes. 370 → 448 refusal codes.** `docs/reference/abi/scoringRuleV1.md`
is new. `refusalRegistryV1.ts`, `routeCensus.ts` and `marketPhaseAdmissionV1.ts`
are the emitters' own output.

This was one commit rather than ten on purpose: every family that added a code
or a route declined to commit these files, because a regenerated reference is a
large diff that conflicts badly and doing it per-branch puts the identical diff
on ten branches. It also could not have been done earlier — `product-shapes`
moved ten codes on landing.

**`tools/gates/emission-coverage.md`** was regenerated after each landing that
moved it, never hand-edited: 91 generated files at the start of the queue, 100
at the end, **guarded 100, unguarded 0 throughout**. An unguarded count that had
gone up would mean a generated file arrived with nothing checking it.

**`lake build`** over all 146 modules exits 0 — and it did not before
`bebac8972`. `RelayedVenueDecodingRulesV1.the_cut_separates_the_branches` left
an unsolved goal, and `product-shapes`, which authored 254 of that file's lines,
says plainly in its notes that no `lake` run ever happened in that worktree.
The goal `simp` left is an inequality across an `Int.ofNat` coercion and nothing
else; `omega` discharges it and the theorem's statement is untouched. This is
the check that family asked the merge to make before trusting a theorem name in
its files, and it found a red on the first run.

`ParentReferenceV1Abi.lean` — whose refusal-corpus theorem the family reports
was FALSE as committed, because a corpus row set a byte that was already zero —
now elaborates.

**Two `sorry`s survive**, in `ScoringRuleV1.lean`
(`exp2Neg_below_the_real_value`, `exp2Neg_near_the_real_value`). They are NOT
this wave's: both are present on main at `67ecd841b`, left by `1f755edc4`,
whose own subject records that the two `exp2Neg` statements were found FALSE.
Named here so no reader takes "the library builds" for "the library has no
holes."

**`tools/gate cheap` is nine of ten** at `09068cfa9`: selftest, census,
emission, citations, budgets, fmt, locks, commands and release all PASS.

* `fmt` was red on twenty files. Every convergence lane declined to format them
  for a reason that was right in isolation — *this lane does not format another
  lane's files* — and ten lanes each being right left a push gate nobody owned.
  Formatted with the pinned `tools/lane.sh fmt`, and the **baseline shrank by
  fourteen rows**: the wave's rewrites made those files formatted, so their
  baseline lines had gone false, which that gate treats as a failure of its own.
  120 → 106.
* `citations` was red on one register row, and it was main's: the System program
  address is cited by two evidence documents and the register named one. Fixed.

## 5. What is red and owed

**The frame ratchet, and it is the largest single debt.**
`tools/gates/frames-baseline.json` is untouched. Seven of the ten landed
families move an SBF link and every one of them says so in its own commit
message rather than in a file nobody greps:

| family | what moved |
|---|---|
| `deploy-lifecycle` | four functions lost a `rent: &Rent`; Core's capability route gained a decode. Resolution and Core links. |
| `failure-arm` | one `#[repr(u32)]` variant and a `const fn` reached from `authenticate_and_prepare`. Claims link. |
| `general-lifecycle` | five Trading functions, one accelerator (`candidate_cause`). |
| `claims-split-merge` | thirteen `claims_conservation_v1::*` symbols leave, eleven arrive; `signed_delta_v3` and `affine_batch_v2` each gain a frame. |
| `economics` | every `direct_close_maker_v1::*` row: the frame went 22 → 27 accounts and the route gained a CPI. |
| `founder-bond` | four new `#[inline(never)]` symbols in the Claims link. |
| `dealer` | four new Trading routes, one new accelerator arm. |
| `series` | `hot_v3::children::{execute,preflight}_child_routes_v3`, thirteen `hot_v3::series_expiry::*`, `emit_series_prepare_funding_artifacts_v5`. |
| `product-shapes` | all of `parents_v1` + a widened `FoundAccounts::parse` (Core); all of `derived_v1`/`derived_transport_v1` + a widened `process_instruction` and two `consume_*` helpers (Resolution). |
| `batch-spine` | the ordinary transition grew 96 bytes; the Direct hot link was measured against the shorter program. |

**It is NOT recaptured here, deliberately.** `tools/gate frames accept` requires
**two independent captures of one commit** on the canonical Linux builder, and
one capture records whatever the tree happened to say that run. Neither this
queue nor any convergence lane was permitted an SBF build. Two things to carry
into that capture: `execute_child_routes_v3` must stay under 4096 (it was 3392
before a by-value `ChildMarketAuthorityV3` parameter), and
`process_derived_settle_v1` must too (it already boxes the source records and
each parent for that reason).

**`tools/gate seam` is red, 21 findings, and NOT recaptured.** Two families
independently reached the same reading and it is right: `--write` rewrites the
whole register, so from a tree carrying other lanes' findings it adopts them as
ACCEPTED without the verdict `tools/seam-audit/EXCEPTIONS.md` requires, and it
drops `GONE` rows that belong to whoever repaired them. The 21 are: seven
`UNSET_GUARD_PRESENT` (positive inventory records, the cheapest to accept);
seven `DOMAIN_RAW_RESTATEMENT` and one `SEED_DOMAIN_UNASSERTED` (each needs a
verdict); and six `GONE` rows, of which three are the `claims_founding.rs` →
`founding_world.rs` extraction showing up as a move, and two are genuine
repairs — `economics` rewrote `direct_close_maker_v1`'s signer rule into a
per-index equality that exempts the closer by name instead of refusing every
signer, and the `TRANSACTION_LEVEL_SIGNER_CENSUS` hazard on both sides of it is
gone because it was fixed.

**The ELF campaigns nobody has run.** Every one of these compiles and none has
executed:

* `claims-split-merge`'s five, in
  `programs/dclutch-claims-sbf/program-test/fractional-atomic/tests/claims_conservation.rs`.
  `tools/gauntlet/claims-fractional-atomic/run-fractional-atomic.sh` is the
  command. Three defects in them were already caught by reading them against
  the route; the family says to expect more of that kind. Recorded in
  `tools/gauntlet/blocked.json` as `unwired` so the register does not read as
  though nobody ever wrote a campaign for that route.
* `dealer`'s: no real-ELF program test and no CU. Its child metas are read off
  two other programs' specs and are what that run will convict first.
* `founder-bond`'s two SBF tests: they compile and cannot run here; audited
  instead.
* `batch-spine`'s `tools/gate sbfcontracts`, `programs` and `suites` — the
  seven pinned identities it recomputed are exactly what those tests compare
  against.

**Producer-missing work, named rather than papered over.** Three families end at
the same shape and it is worth stating as one fact:

* `economics`: **nothing anywhere has executed the vault credit** — the one step
  that makes that family do something. Its two runbook rows (`upkeep-found`,
  `parameters-found`) were declined by this queue rather than written, because a
  runbook row's driver is a successor subcommand and there is none for either
  family; `tools/gate commands` establishes a row by reading the program's own
  `--help`, so the row would name a program that does not exist. Recorded in
  `blocked.json` instead: write the command, then the row, then the binding.
* `series`: the family's real blocker is a producer-missing input, and the
  Shadow certificate has the same shape — its only in-tree writer says of itself
  that it is not release evidence.
* `product-shapes`: no runbook step founds a child market, which is why its
  `blocked.json` row says `status-report` and may not claim a campaign.

**A census blind spot, found by this queue and owed to whoever owns the census
reader.** `custody/protocol_parameters_v1::process` and
`custody/upkeep_vault_v1::process` are live wires that the route census does not
enumerate at all. It reads an entry route off the inline `len == N && data[..k]
== MAGIC` shape in a program's dispatch, and those two dispatch through a
`selects(instruction_data)` helper instead. The six actions behind them ARE
enumerated, so the register looks complete. It surfaced by adding blocking
entries for the entry routes and having the census call them stale — *matches no
enumerated route* — which is the census telling the truth about itself.

**The deploy gate `batch-spine` creates, and it is a re-release obligation.**
That branch changes the emitted ordinary transition and therefore **every
artifact identity a live Direct market pins** — seven of them, because three new
scalar slots moved the common bank 68 → 71. Cohort-8's published RequestProfile
no longer validates and cohort-17's will not either. **Under decision 0012 an
upgrade is a re-found, so this is not a migration: it is a RE-RELEASE.** The
next cohort must redeploy the Direct set and re-found, and no market founded
before `b5f277e63` can execute an ordinary fill under the new client. Nothing in
the handoff said so; the family found it, and it is the reason to run a cohort
early in a cycle rather than late.

**Smaller, and each with an owner.**

* `tools/gauntlet/CU_BUDGETS.json` — no row moved for any new route. The gate
  refuses a campaign no bindings file names, so budgets follow bindings, which
  follow a run.
* `tools/gauntlet/ladder/bindings.json` has no rows for the refund
  continuation's labels, so the exhaust walk's census will report unbound
  transactions on its first run. That is the honest discovery state; the fold
  comes after the run, never before it.
* Two `abi:*`/`abi:*:verify` script pairs are owed in
  `packages/dclutch-sdk/package.json` for `EmitUpkeepVaultV1Ts.lean` and
  `EmitProtocolParametersV1Ts.lean`. Both emitters exist and neither is
  referenced. Landing the script without the generated file would put a red
  guard in the tree.
* `apps/dclutch-web/lib/generated/capabilitySurfaceV1.ts` is stale on main
  (`@dclutch/sdk/shadowDigestV3` on `/general`, added by `7f9d391a4` without
  regenerating the surface). One `npm run abi:capability-surface`.
* `tools/sbom/SBOM.md` is stale by the Solana 2.1.0 → 3.1.12 bump. Whoever
  bumped Solana owns it.
* `check-steps.py --prove-frozen` is red at cohort-14, and the generator refuses
  cohort-14 for `market 'A' has no field 'market.work_dir'`. Both measured at
  the pre-merge commit; pre-existing.
* `check-steps.py` validates a driver KIND and never whether a `bootstrap`
  invocation names a verb the successor dispatches. `series` shipped a `-v2`
  verb the tree only has at `-v1` and caught it by hand; this queue cross-checked
  the `dealer`'s four the same way. `tools/doc-commands` is the right home and
  its `--roots` do not include `steps.tsv`. A regex conjunct inside
  `check-steps.py` was tried and abandoned by that lane: 12 of 27 verbs dispatch
  through forms a regex misreads.

## 6. The workspace at `09068cfa9`

`cargo check --workspace --tests --offline` is **green except two crates, for
one manifest reason, and they are red on `main` at `67ecd841b` too**:
`dclutch-wallet-terminal-payout-wasm` (3 errors) and
`dclutch-wallet-terminal-input-wasm` (6), all nine
`error[E0433]: cannot find 'tests' in 'wire'`. The cause is one line:
`crates/dclutch-wallet-terminal-*-wasm/Cargo.toml` takes
`dclutch-operator = { …, default-features = false }` under a comment reading
"the shared fixture, not a second copy of one", while the fixture is
`#[cfg(any(test, feature = "test-fixtures"))] pub mod tests` and nothing enables
`test-fixtures`. The fix is `features = ["test-fixtures"]` on that
dev-dependency. It is nobody in this wave's — no build branch touches either
file — and it was the exact same nine errors before the first landing and after
the last. That control was re-measured at every landing.

Everything else in the workspace compiles, tests included.

* `tools/gate census` PASS: 160 routes, 448 refusal codes, 0 unclassified
  positions, 45 blocked, 0 stale blocking entries.
* `tools/gate reference --check --converge` at `9b2d22af8`: already the fixpoint.
* `tools/gate emission` PASS: 100 generated, 100 guarded, 0 unguarded.
* `lake build`: 146 modules, exit 0, two pre-existing `sorry`s named above.
* `check-steps.py`: cohorts 15, 16, 17 and 18 all green. Cohort-18 is 53 steps
  and emits 87 stage scripts; cohort-17 is 39 steps and 69 scripts, and those 69
  are **byte-identical before and after the whole queue** — the control that
  nothing already-run moved.
* `tools/gate cheap`: nine of ten, `seam` the tenth (§5).

## 7. Two mistakes this lane made, recorded because the fix is the interesting part

`git add -A` and then a directory-scoped `git add -A tools crates …` each swept
up two files that were sitting UNTRACKED in the working tree before the wave and
belong to no family in it:
`crates/dclutch-trading/src/registered_terminal_artifacts_v4.rs` (an orphan — no
`mod` line declares it, so nothing compiles it) and
`docs/evidence/NIGHT_FREEZE_2026_09_01.md` (an evidence document with a
deliberate provenance header). Backed out at `f99655ae7` and `e57a08832`, both
byte-identical on disk and untracked again, and then named in `.git/info/exclude`
— a local, uncommitted file, so it is a fact about this working copy and not a
claim on the repository. AGENTS.md forbids `add -A` for exactly this reason and
the second occurrence is what shows that a scoped `add -A` is the same hazard.

Publishing another lane's uncommitted work inside a merge commit makes the merge
commit a claim about work the merge did not do.

---

## 8. The eleventh landing — `recovery-capture`, by MERGE-QUEUE-2

A second queue opened after the first closed, to land the twelfth family the
first one never had. Sections 8, 9 and 10 are its record; sections 1–7 above
are the first queue's and are untouched, except where §8.3 corrects one
measurement of §6 that had gone stale by the first queue's own landing 6.

| # | family | branch head | merge | cut | conflicts |
|---|---|---|---|---|---|
| 11 | `recovery-capture` | `957fd0b35` | `ab29bbe97` | `d63e4696a` | **none** |
| — | (regeneration) | — | `15934ad5a` | `e9773f660` | — |

**No conflicts, and that is a fact about the lane rather than about the merge.**
`build/recovery-capture` rebased itself three times as the first queue advanced,
and the last rebase resolved all ten landed families by hand. The one commit
`main` took after that rebase — `673220e24`, the census recogniser — touches
three files (`tools/gates/README.md`, `tools/gauntlet/census/src/{enumerate,main}.rs`)
that this family does not. 61 files, +7,806/−680.

### 8.1 The refusal codes, and the ordering rule the family established

**Taken as they stand: `EnsembleMember = 0x8029`, `EnsembleQuorum = 0x802A`,
`EnsembleQuorumMet = 0x802B`**, contiguous behind `RelayedVenueKind = 0x8028`.

§1 of this document records the queue's plan for this family: renumber around
`deploy-lifecycle`'s `0x801E`. **That plan is impossible for any branch in
isolation, and the family proved it.** `pin_refusal_band!` asserts that the
discriminants are the *contiguous run from the band base*, so a hole is a
compile error — a branch cannot leave `0x801E` free and take `0x801F` before
the branch that fills `0x801E` has landed. **Land-then-renumber is the only
order that exists**, which is what the first queue did in practice at landing 9
without naming it as the general rule. Recorded here because it is the rule,
not the anecdote: a lane asked to "renumber around" an unlanded branch should
answer that it cannot, and rebase instead.

### 8.2 What was regenerated, and the three routes that were not this family's

`tools/gate reference --converge` from a clean worktree at `ab29bbe97`, to its
fixpoint in three passes (six files, then two, then nothing);
`tools/gate reference --check --converge` at `15934ad5a` reports *"15934ad5a8af
is already the fixpoint"*.

**162 → 165 routes; refusal codes unchanged at 451.** The family had already
regenerated the whole surface on its branch for its own three codes and two
routes, which is why 451 does not move here and why §7 of its notes asked only
that the reference be regenerated rather than hand-merged.

The three routes that move are `673220e24`'s, not this family's, and that
commit's own message says the regeneration is owed and is the convergence
owner's:

  * `custody/upkeep_vault_v1::process` (predicate `selects`, `DCLCUPQ1`)
  * `custody/protocol_parameters_v1::process` (predicate `selects`, `DCLTPRQ1`)
  * `accelerator/dealer::process` (predicate `dealer_family_selected`)

The first two are exactly the live wires §5 records as invisible to the route
census. **They are on the published surface now.** `custody` goes 4 → 6 entry
routes, `accelerator` 3 → 4; magic-selected 86 → 88, predicate-selected 26 → 29.
One honest row arrives with them: `protocol_parameters_v1::process` also lands
in `UNRESOLVED_PREDICATE_ARMS_V1`, because its magic does not resolve to eight
ASCII bytes for the reader. The route is enumerated and its magic is not
resolved, and the generated surface states both rather than choosing.

The route-count history of the whole wave, since three different numbers are
each correct for their own commit: **149** before the wave → **160** at
`09068cfa9` (ten families) → **163** at `673220e24` (no new code; the census
recogniser reading a guard's *body* instead of its name found three that had
always been there) → **165** at `ab29bbe97`. Refusal codes: **370 → 448 → 451**.

### 8.3 The workspace, and a correction to §6

`cargo check --workspace --tests --offline` at `15934ad5a`: **exit 0, zero
errors, 98 workspace members.** No exceptions.

**§6 above is stale and this corrects it.** It records
`dclutch-wallet-terminal-payout-wasm` and `dclutch-wallet-terminal-input-wasm`
as red for nine `E0433: cannot find 'tests' in 'wire'`, states that the fix is
`features = ["test-fixtures"]` on that dev-dependency, and says "It is nobody in
this wave's — no build branch touches either file … That control was re-measured
at every landing."

The first two clauses were true at `67ecd841b` and the third was not true after
landing 6. `3f9e3c810` (Lane `CONVERGE-founder-bond`) put exactly that
`features = ["test-fixtures"]` on **both** dev-dependencies, and says so in its
own message — "main's own red, outside this family and fixed so the family's
gate could run". It reached `main` inside `98eb0deb8`. The manifests at
`09068cfa9` already carry the feature. Whatever the queue's re-measurement was
reading after landing 6, it was not those two packages: they compile.

The lesson is not about the wasm crates. **A control that is stated once and
re-asserted at each step stops being a measurement.** The nine-error control was
carried forward by name through four more landings after the branch that
removed it had landed.

### 8.4 Four findings about `main` this family measured, carried here because they are not about it

1. **Two workspaces outside the root one were red on this family's new relay
   variants**: `tools/relayer` and `tools/gauntlet/relayed-vertical`, both of
   which match exhaustively over `RelayAccountNameV1`. `cargo check --workspace`
   does not reach either, and the `journey` gate is the only thing in the tree
   that compiles them. The family repaired both (`214830bd1`). **The structural
   fact is owed to whoever owns the gates**: a non-root workspace with an
   exhaustive match over a shared enum is a consumer that only one gate can see,
   and the family's own BUILD doc named neither.
2. **`relay::instruction::tests::an_unknown_action_refuses` was red and nobody
   had run it.** It put action byte `9` — which this family's `ReclaimMemberSeat`
   had taken — so it was decoding a well-formed action of the wrong width and
   reading `InvalidLength` as its refusal. It asks `RelayActionV1::decode` which
   bytes are actions now, and puts every byte that is not one. **A family that
   adds an action must re-run the wire tests, not only the ones it wrote.**
3. **`tools/gate seam` is red on `main` itself**, measured by that lane in a
   clean `git worktree` at `main` rather than asserted, and identical to its own
   branch's — the family adds nothing to it.
4. **`tools/gate clippy` is red on `main` itself**, in six packages outside
   `clippy-debt.tsv`, and **this family repairs one of them** —
   `crates/dclutch-source/src/relay/decode.rs:359`, a collapsible `if` that
   landed with the native venue. It is one line of `main`'s, and it made the
   whole `dclutch-source` package red; **a red package hides every package above
   it**, so nothing in this family's own crate could be measured while it stood.

Findings 3 and 4 were re-measured at `15934ad5a` by this queue:

* **`seam`: 28 findings** — 10 `NEW DOMAIN_RAW_RESTATEMENT`, 9
  `NEW UNSET_GUARD_PRESENT`, 1 `NEW PRIVILEGE_PIN_UNEXEMPTED`
  (`derived_transport_v1.rs::validate_frame`), 1 `NEW SEED_DOMAIN_UNASSERTED`
  (`PROTOCOL_PARAMETERS_RECEIPT_PDA_DOMAIN_V1`, 29 bytes with no `<= 32`
  assertion), and 7 `GONE`. The seven `GONE` include the three
  `claims_founding.rs` → `founding_world.rs` moves and the three
  `direct_close_maker_v1` signer-census repairs §5 names. **Still not
  recaptured**, for §5's reason: `--write` from a tree carrying other lanes'
  findings adopts them as ACCEPTED without a verdict. §5 recorded 21 at
  `09068cfa9`; it is 28 now, and the same lane measured 28 on a clean `main`
  worktree before this landing — so the growth is not this landing's.
* **`clippy`: 98 members, 52 clean, 9 red, 37 never reached.** Five red outside
  `clippy-debt.tsv`: `dclutch-claims`, `dclutch-market`,
  `dclutch-pre-market-funding-test-caller-sbf`,
  `dclutch-resolution-core-v3-operator`, `dclutch-route-census`.
  **`dclutch-source` is not among them**, which is finding 4 confirmed at HEAD:
  six became five and the one that fell was the one hiding the others. The
  37 never-reached are behind the remaining five.

### 8.5 The gates at `15934ad5a`

`tools/gate cheap` is **eight PASS, one FAILED, one NOT RUN**: selftest, census,
emission, citations, budgets, fmt, locks and release PASS; `seam` FAILED (§8.4);
`commands` **NOT RUN** — "a published command was not probed", because the
`dclutch` CLI is neither built under this worktree's `target/` nor on `PATH`.
That is a property of the isolated worktree this queue measured in, not of
`main`; the first queue ran `commands` green from the live tree.

* `tools/gate census` PASS: **165 routes, 451 refusal codes**, 0 unclassified
  positions, 47 blocked, 0 stale blocking entries.
* `tools/gate emission` PASS: **101 generated, 101 guarded, 0 unguarded.**
* `tools/seam-audit/baseline.json` was set-differenced against `main` rather
  than read as a diff, because a re-sorted JSON register makes a textual diff
  unreadable: **exactly two rows added**, both this family's own
  (`validate_relay_frame_with_tail_v1`; `ensemble_v1.rs`), **none removed, none
  changed**. Main's outstanding findings were not absorbed.

**Where this queue measured, and why it matters.** The live tree
`/Users/ember/dev/dclutch` was checked out on `build/joint-clearing` throughout,
by the lane that owns that family, with that lane's work in it. Every landing,
gate and cut above was therefore done from a private `git worktree` at `main`
with its own `CARGO_TARGET_DIR`, and the live tree was never touched. A reader
reproducing these numbers from the live tree will reproduce them; a reader
running `tools/cut.sh` from it will not publish `main` (§8.7).

### 8.6 What this landing owes

**The frame ratchet, unchanged and now owed by one more commit.** Two new
`#[inline(never)]` symbols enter the Resolution link — `process_ensemble_fold`
and `process_reclaim_member_seat` — and neither has a row in
`tools/gates/frames-baseline.json`. No SBF build was run by this queue either.
The §5 table gains an eleventh row:

| family | what moved |
|---|---|
| `recovery-capture` | `process_ensemble_fold` and `process_reclaim_member_seat` enter the Resolution link; `ENSEMBLE_FOLD_FRAME_PREFIX_V1` 25 → 29 and `RECLAIM_MEMBER_SEAT_FRAME_V1` 6 → 10. |

**The ensemble is complete as a route and unreachable on any chain, and now
says so.** Two producers are missing and each is its own piece of work, not a
missing test — both are `structural` rows in `tools/gauntlet/blocked.json`,
which the route census prints, and both are visible in `docs/reference/routes.md`:

1. **No founding writes an ensemble material.** `SourceMaterialV3::with_ensemble`
   exists, `EnsembleSpecV1::validate_foundable` states the odd-quorum conjunct,
   and Core's founding calls neither; every material on any chain reads
   `EnsembleSpecV1::SINGLE`, so the fold's first conjunct refuses.
2. **No route writes a fragment.** The family's R9 makes this exact rather than
   optimistic: `observed_fragments = 0` from every existing frame is correct
   *today* and has a named trigger — **the moment a member-capture route exists,
   `ensemble_seat_tail_v1` must be threaded onto the crank's and the failure
   walk's frames in the same change**, or `EnsembleQuorumMet` becomes a code
   whose raiser can never fire and the crank can steal the decision from a met
   quorum.

Never started, and named as such: the operator, the successor driver, the
`tools/cohort/steps.tsv` row for `found-ensemble`, and the TypeScript decoder
for the fold receipt.

**`batch-spine`'s obligation is discharged in part.** §3 records that
`DirectRfqV1.lean` names `ClearingPriceV1` in prose while `ClearingPriceV1Abi.lean`
is defined only on `build/joint-clearing`. That reference is still unreal; it
belongs to the twelfth family, not this one.

### 8.7 An incident: the publication host was carrying an unmerged branch

Found by this queue when its first cut was rejected non-fast-forward.

**The public repo's `dclutch/` tree was `build/joint-clearing`'s.** Public
commit `36b81ab6c` — "dclutch 75cede26c (+10)" — has `dclutch` tree
`cbacef750`, which is exactly `75cede26c^{tree}`, the tip of a branch that is
not on `main` and whose own family declares itself NOT READY (§3). `main`'s
`673220e24` was never cut at all.

**The mechanism is `tools/cut.sh` reading `HEAD`, and it is not a bug in the
script.** The script is explicit that it cuts from HEAD and refuses the working
tree, and it gates on the published tree being byte-identical to that HEAD —
which it was. Its unstated premise is the one AGENTS.md states elsewhere: the
live tree is on the canonical integration branch. The live tree was on a lane's
branch, so a correct script published a correct tree of the wrong commit.
**A cut is authorized for `main`; the gate that proves the tree cannot tell
which branch it came from.**

Repaired by this queue's own cuts: `d63e4696a` publishes `ab29bbe97`, restoring
`main` as the published content, and says so in its message. Nothing is lost —
the public repo carries content and not history, and `36b81ab6c` stays in the
public log naming the live commit it published. `refs/cut/live` in the
publication host had to be force-updated once locally, because it pointed at
`75cede26c` and `main` is not a descendant of it; that is a transport handle
inside the publication host, not a remote force-push.

**Owed**: `tools/cut.sh` has no assertion that `HEAD` is on `main`. One would
have refused `36b81ab6c` and cost nothing. It is left unwritten here because
this queue is not the owner of that script and a lane editing a shared script
mid-wave is its own hazard; it is named so the next owner writes it.

## 9. The twelfth family — `joint-clearing` — still not landed

**Not landed, not abandoned, and not this queue's to force.** At 02:37 EDT
`build/joint-clearing` is 12 commits ahead of `main` at `5e76533f3` and its lane
(JOINT-CLEARING-2) is still committing — the last four commits in ten minutes,
the most recent being *"converge: the two writes with no frame get a name, a
reason and a test each"*, which is the second of the three reasons §3 gives for
the abort. No `MERGE_NOTES_JOINT-CLEARING-2.md` exists yet.

This queue's instruction was to land it only if that lane had landed it or
declared it ready. It has done neither, so it is recorded and left alone. §3's
reasons stand until that lane says otherwise, and §3's own warning stands with
them: completing the V2 migration means **authoring what those program-test
walks assert**, in files the family did not touch, and that is the family's work
and not a conflict resolution.

Two things about `main` that the twelfth family's landing will meet:

* **The `CONVERGE-RECOVERY-CAPTURE` ledger entry is not on `main`.** It was
  written onto `build/joint-clearing` (`75cede26c`, "ledger: the twelfth family
  is ready, and a hole in a band is a compile error") rather than onto `main`,
  so `docs/ledger/2026-09-07.md` on `main` has no closing row for the family
  that just landed. It is deliberately **not copied here** — the entry has one
  author and one copy, and duplicating it would make the twelfth family's merge
  conflict on a paragraph that agrees with itself. Whoever lands
  `joint-clearing` brings it with them.
* `docs/reference/` moved twice since that branch's base, and is generated.
  Regenerate after landing; never hand-merge it.

## 10. The wave's final tally

**Eleven of twelve families are on `main`.** Ten by MERGE-QUEUE-1
(`31de5e2bd` … `b5f277e63`), the eleventh by MERGE-QUEUE-2 (`ab29bbe97`), and
`joint-clearing` outstanding.

| | before the wave | after |
|---|---|---|
| routes | 149 | **165** |
| refusal codes | 370 | **451** |
| generated files / guarded / unguarded | 91 / 91 / 0 | **101 / 101 / 0** |
| `fmt` baseline rows | 120 | **106** |
| Lean modules imported by the root | 135 | **145** |
| `cargo check --workspace --tests` | 9 errors, 2 crates | **exit 0, zero errors** |

Refusal codes moved by the wave, in landing order: `ClaimsSbfError::Overdraw`
kept `0x5012` and `FounderBondFrame` took `0x5013`; `ResolutionError::FundedRent`
kept `0x801E` and `product-shapes`' ten moved to `0x801F`–`0x8028`; and
`recovery-capture`'s three took `0x8029`–`0x802B`, the first free run behind
them. **No code that had ever reached a chain or a published reference moved**,
which is decision 0007's rule and the reason the order mattered.

The Lean row is a count, not a build. This queue did not run `lake build`:
`formal/` at `15934ad5a` is **byte-identical to `build/recovery-capture` at
`957fd0b35`**, where that family measured `lake build DClutchSemantics` green at
147 jobs with only `ScoringRuleV1`'s two declared `sorry`s. `main` contributed no
Lean change after the rebase, so the measurement transfers exactly; a reader who
wants a build rather than a transfer should run it.

**Owed, in the order a reader should care:**

1. **The frame ratchet**, red across eight of the eleven landed families, needing
   two independent SBF captures of one commit on the canonical Linux builder.
   The largest single debt, and unchanged.
2. **`batch-spine`'s re-release obligation** under decision 0012: it moves every
   artifact identity a live Direct market pins, so the next cohort redeploys the
   Direct set and re-founds.
3. **The seam register**, 28 findings wanting one triage pass by whoever owns the
   wave rather than twelve lanes each declining to adopt the others'.
4. **`clippy`**, five red packages outside the debt list, hiding 37 more.
5. **The ELF campaigns nobody has run** (§5), unchanged.
6. **The producer-missing work** (§5), now four families rather than three:
   `economics`' vault credit, `series`' certificate, `product-shapes`' child
   founding, and `recovery-capture`'s ensemble material and fragment.
7. **A branch guard in `tools/cut.sh`** (§8.7).
8. **The twelfth family** (§9).
