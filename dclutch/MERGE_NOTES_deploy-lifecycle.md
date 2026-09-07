# MERGE NOTES — `deploy-lifecycle`

Branch `build/deploy-lifecycle`, measured at `9552f6104` (this document is the
commit above it), rebased onto `main` at `a43517586` (clean rebase, no
conflicts, as the BUILD doc predicted; the branch's five original commits kept,
seven convergence commits on top).

## 1. READY

The two compile errors are gone, and so is the half-migration that produced
them — including one half the doc did not know about, a producer/consumer split
this branch had opened across a program boundary (§7.1). One shared-file
collision needs a ruling from the merge lane before this lands: **three build
branches all claim refusal code `0x801E`** (§2).

| what | result |
| --- | --- |
| `cargo check -p dclutch-resolution-proof-sbf -p dclutch-core-sbf -p dclutch-market -p dclutch-registry -p dclutch-local-successor-bootstrap --offline --tests` | green, 0 errors. The two errors §7 of the BUILD doc quotes verbatim are both closed. |
| `cargo test -p dclutch-local-successor-bootstrap --offline --bin dclutch-local-successor-bootstrap` | **838 passed, 0 failed, 1 ignored.** One test in it was RED when this lane started and is not a test this lane wrote — see §5, item 2. |
| `cargo test -p dclutch-market --offline --test capability_manifest__funded_rent_v1` | 8 passed, 0 failed. |
| `cargo test -p dclutch-market --offline --lib capability_manifest::funding` | 26 passed, 0 failed. |
| `cargo test -p dclutch-registry --offline --lib` | 91 passed, 0 failed. |
| `tools/gate census` | **PASS.** 148 routes, **353** refusal codes, 0 unclassified positions — the new code is inside its band and the band is append-only. |
| `tools/gate fmt` | green for every file this branch touches. Two files remain red and both are main's: `crates/dclutch-operator/src/lib.rs` and `tools/local-validator/bootstrap/successor/src/market.rs`, unformatted in a sibling build worktree that has never touched either. |
| `tools/gate reference --check` | red, deliberately, and not committed here. See §2. |

Warnings: the only one in a file this branch touches is gone. The three dead
`rent: &Rent` parameters and the abandoned `prestate` decode that §7 of the
BUILD doc lists as its five warnings are all closed. What remains in these
crates is main's: one unused import at
`programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs:34`, and
seven in `programs/dclutch-trading-sbf/`.

## 2. SHARED FILES — for the merge lane

### 2.1 THE COLLISION: three branches claim `ResolutionError = 0x801E`

`programs/dclutch-resolution-proof-sbf/src/lib.rs` is touched by three build
branches and every one of them takes the next free code after
`SourceLadder = 0x801D`:

| branch | codes claimed | count |
| --- | --- | --- |
| `build/deploy-lifecycle` (this one) | `FundedRent = 0x801E` | 1 |
| `build/recovery-capture` | `EnsembleMember/Quorum/QuorumMet = 0x801E–0x8020` | 3 |
| `build/product-shapes` | `DerivedParentNotTerminal … RelayedVenueKind = 0x801E–0x8027` | 10+ |

Decision 0007 makes bands append-only and forbids renumbering a code — but a
code that has never reached a chain or a published reference is not yet a fact,
and all three of these are unlanded. **Renumber on landing order, not on merge
convenience**, and regenerate the mirrors once at the end. The cheapest order is
this branch's single code first (`0x801E`), then recovery-capture
(`0x801F`–`0x8021`), then product-shapes (`0x8022`+): one branch has to move one
number, not ten. Whatever the order, every branch's `pin_refusal_band!` array
must keep its variant — the macro is exhaustiveness-checked, so an omission is a
compile error rather than a silent gap.

### 2.2 `docs/reference/` and the SDK mirrors — regenerated, then REVERTED here

`tools/gate reference --converge` at this head produces nine files, and this
branch commits **none of them**. Two reasons, both about the merge lane's
binding constraint:

1. The code is going to be renumbered (§2.1), so every row generated now is
   wrong the moment a second branch lands.
2. `docs/reference/README.md`'s totals paragraph carries this branch's refusal
   count (`352 → 353`) and **main's** cohort-17 witness drift (`110 → 111`
   witnesses) in one adjacent hunk, and `routes.md` and `route-witnesses.md` are
   main's drift alone. Committing them here would put an identical 155-line diff
   on however many branches also regenerate, and generated files conflict badly.

After the renumber: `tools/gate reference --converge` once, commit all nine.
The rows this branch owes, so they can be checked rather than trusted:

| file | row |
| --- | --- |
| `docs/reference/refusals.md:429` | `\| 0x801E \| ResolutionError::FundedRent \| The rent a funding ledger was FUNDED at did not price its balance. \| -- \| programs/dclutch-resolution-proof-sbf/src/lib.rs:238 \|` |
| `docs/reference/abi/refusalRegistryV1.md:345` | the same code/name/meaning, band `resolution` |
| `packages/dclutch-sdk/lib/generated/refusalRegistryV1.ts` | the same, after the `0x801D` entry |
| `docs/reference/abi/routeCensus.md`, `packages/dclutch-sdk/lib/generated/routeCensus.ts` | `code: 32798` in the resolution arm |
| `docs/reference/programs.md:22` | resolution's code count `30 → 31` |
| `docs/reference/README.md`, `docs/reference/refusals.md` | `352 → 353` refusal codes |

(Contrast with `build/batch-spine`, where the one generated mirror this wave
owed WAS committed: there the value is stable and the file is separable. The
rule is the same in both — carry a generated mirror when it is stable and
separable, hand it on when it is neither.)

### 2.3 `tools/cohort/steps.tsv` — two rows, four other branches

Touched by `failure-arm`, `series`, `claims-split-merge` and `founder-bond`, so
untouched here. Two rows are owed:

- **Row 74 `funded-rent-recorded` (tier 16) needs an added expectation.** It
  reads the LEDGER's recorded rate only. Add: *and every close this cohort
  performs plans a refund equal to `(128 + width) × the rate the ledger records`,
  not to `getMinimumBalanceForRentExemption` at the time of the close.* That is
  the conjunct this branch moved, on all three sites.
- **A new row for the deployment-set binders, tier 16, after the upgrade rows.**
  `devnet-deployment-set-bind-upgrade-row-v1 --journal <set>.json --role <role>
  --execute`, expectation: *the row carries the receipt and dump digests its own
  receipt names, and `devnet-deployment-set-journal-v2 --audit` reads the journal
  it could not read before.* Cohort-16.1 ran a job-directory script here and
  cohorts 14–16 never reached the case at all (§5, item 3).

### 2.4 `tools/local-validator/bootstrap/successor/src/main.rs`

Two dispatch arms added after the `ALREADY_CURRENT_COMMAND_V1` arm (`:170-172`).
`series`, `claims-split-merge` and `founder-bond` also touch this file; the
arms are additive and adjacent to a distinctive anchor, so a conflict here is
mechanical.

### 2.5 Not owed, checked

- `tools/gauntlet/blocked.json` — no row (campaign blockers, not refusal codes).
- census TARGETS — `tools/gauntlet/census/src/main.rs:52` already lists
  `("dclutch-resolution-proof-sbf", "resolution")`; the census picks the new
  variant up on its own, and `tools/gate census` proves it did (353).
- `programs/dclutch-claims-sbf/src/lib.rs` — see the S18 ruling in §3.3.

### 2.6 `tools/gates/frames-baseline.json` — owed, needs an SBF build

Four functions lost a `rent: &Rent` parameter and one gained a decode
(`funded_rent_minimum` in Core's capability route), so the Resolution and Core
links' frames moved. No SBF build was run here (the convergence brief forbids
it). `tools/gate frames --at <commit> --capture` is owed by whoever next builds
these two links; until then the ratchet is red for this commit and the commit
message should say so.

## 3. What this lane completed, and what it ruled

### 3.1 The compile break (S1, S2a, S2b) — closed, and one more than the doc found

`funded_rent_refusal` is written at
`programs/dclutch-resolution-proof-sbf/src/core_effect.rs:2703`, on the shape of
the three Core precedents, returning `ProgramError`.
`ResolutionError::FundedRent = 0x801E` is declared with the `///` the census
reads as its meaning, and appended to the `pin_refusal_band!` array.

Beyond the doc: **both** close constructors now refuse through
`funded_rent_refusal`, not just the new call. `commit_direct_close` already
called `recorded_native_only` and mapped every cause to `Funding`, so the code
the branch declared had no producer on the path it was declared for. It has one
now, in both close routes.

### 3.2 The half-migration (S4, S5, S6) — settled as one decision, all three sites

Decision 0030 names three exact-rent close sites. The branch migrated one and
left the second mid-edit. All three are migrated:

| site | before | after |
| --- | --- | --- |
| `core_effect.rs:565` `commit_direct_close` | already `recorded_native_only` | + `funded_rent_refusal` |
| `core_effect.rs:2369` `process_close` | `rent.minimum_balance(...)`, with an authenticated `prestate` decoded three lines above and thrown away | `recorded_native_only(prestate, …)`. The `Box`, the `?` and the comment are now paid for. |
| `programs/dclutch-core-sbf/src/capability.rs:548` | `rent.minimum_balance(pre_bytes.len())` | `authenticated.funded_rent_minimum(pre_bytes.len())`, read while the authenticated ledger still borrows the account data and carried past the `drop` as a lamport figure |

And three the doc did not name, found by the §7 census after these three
had landed:

| site | why it is the same defect |
| --- | --- |
| `programs/dclutch-core-sbf/src/resolution.rs:542` | the CONSUMER of a receipt field this branch had just moved to the recorded rate — see §7.1, item 1 |
| `programs/dclutch-resolution-proof-sbf/src/pre_market_funding_abort_v1.rs:134` | one sysvar read that is the custody conjunct, the refund and the receipt's `rent_refund_lamports` |
| `programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:1649` | the failure ledger's custody conjunct and the relay's planned refund |

`funded_rent_refusal` moved from `core_effect.rs` to `lib.rs`, beside the code it
names, once it had four callers in three modules.

The third takes the rent from `AuthenticatedFundingLedgerV2::funded_rent_minimum`
rather than through `FundingLedgerCloseCustodyV2::recorded_native_only`, because
`authenticated` cannot outlive the `drop(ledger_data)` that the function's own
shape requires. That is the same author — `recorded_native_only` is a two-line
wrapper over exactly this call — at the cost of one more `Rent` decode avoided
rather than one more ledger decode bought.

Eight dead `rent` parameters and bindings are gone with their arguments (S7, S8,
S9, plus `validate_ledgers_pre`, `series_consume`'s two, `authenticate_failure_funding`,
and the two bindings in `core-sbf/src/resolution.rs` and
`pre_market_funding_abort_v1.rs`). Core's capability route
still DECODES the Rent sysvar, so a frame whose rent account is unparseable
refuses exactly as it did; nothing prices off it. **Removing the account from
that frame is a wire change with a redeploy behind it and is named, not taken**,
and the same is now true of three more routes.

### 3.3 S10 — the producer-missing seam the doc diagnosed and reproduced

`FundingCustodyObservationV1::recovered_native_only` had zero production
callers. It has one:
`programs/dclutch-core-sbf/src/series_consume.rs:797`, which priced a **Pending
V1 funding state** at today's sysvar. A prepared series is consumed in a later
transaction and can be consumed in a later EPOCH; at the old code every state
the cluster had since re-rated refused by the rate difference, exactly the
cohort-15 shape. The site now recovers the rate from the state's own balance
and its own `remaining().native_lamports_total()`, refusing
`CoreSbfError::FundedRent` when no integral rate reproduces the remainder.

**S11 (`recorded_native_with_crank`, zero callers) is ruled KEEP, and the reason
is not sunk work.** Its non-recorded twin `native_with_crank` has zero
production callers too: the crank close route (`docs/design/FUNDED_CRANK_V1.md`)
is designed and unwired. Deleting only the recorded form would leave the
sysvar-reading constructor as the sole thing a future wiring lane finds, which
reproduces the defect decision 0030 exists to prevent. Cost of keeping: 14
lines. The correct deletion, if anyone wants one, is BOTH — after ruling that
the crank route is not coming.

**S18 (`ProtocolPositionSbfErrorV2::FundedRent = 0x514B`) is ruled NOT OWED
here.** `programs/dclutch-claims-sbf/` calls
`validate_recorded_native_custody` nowhere, so the variant would be a refusal
code with no producer — the exact pattern §0(2) of the BUILD doc diagnoses.
Claims *does* have raw exactness comparisons of its own
(`protocol_position_v2.rs:420`, `:618`; `founding_v5.rs:1682`) and those are the
work; the code follows the site, not the other way round. See §7.

### 3.4 R1 REVERSED — `lineage_walk` stays

R1 ruled it deleted on a census of zero consumers. **The census is wrong.**
`programs/dclutch-registry-sbf/tests/lineage_program_test.rs` calls
`walk_lineage_to_head`, `walk_lineage_to` and `LineageAt` to turn "two
`DeclareSuccessor` records landed" into "the chain they form is walkable across
BOTH hops, on a Registry that upgraded under it", and it is the named guard for
the live-measured trap in
`docs/evidence/RELEASE_SET_COHORT_LINEAGE_2026_08_31.md` — a walk that asks
`is_already_current` answers about the wrong endpoint.

R1 was not careless: the module's own head named three readers (the SDK, the
site, the lifecycle rent credit's close) that no census finds, and a reader who
trusted it would reach R1's answer. That paragraph is corrected, and the module
now carries its consumer, its census date and its cost of keeping. What survives
from R1 is the rest of its argument — `Core::MigrateMarket` never landed and
cannot under decision 0012 — and it is recorded there too.

**Cost of the reversal:** 268 lines in `crates/dclutch-registry/src/lineage_walk.rs`,
`Error::LineageWalkTooLong` (a variant no production path can produce; the
Registry `Error` enum carries no `#[repr]` and no band, so it is not a
protocol-visible code and decision 0007's withdrawal rule does not apply), and
`packages/dclutch-sdk/lib/releaseLineage.ts` (342 lines) plus its 418-line test.

That TS module is read by **nothing** — not `apps/`, not `packages/`, not even
re-exported from the SDK's `index.ts`; its only reader is its own test, and
`packages/dclutch-sdk/scripts/abi-coverage.baseline.json:34` carries its row. It
is kept this cycle deliberately: AGENTS' rule about hand-written browser mirrors
is that they become a wire's last authority *when their owner is deleted*, and
the owner is not being deleted. Its own head says it exists because no emitter
covers `DCLRLND1` yet. Delete it when that emitter lands, together with the
baseline row and a dated addendum to
`docs/evidence/RELEASE_SET_COHORT_LINEAGE_2026_08_31.md`, which cites it four
times.

### 3.5 R4 — half DISCHARGED, half ruled not owed

`crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs:5907` and
`:5993` already lower `lamports_per_byte` to 5,080 mid-test against a ledger
funded at 6,960, with an `assert_ne!` beside it as the positive control. That
landed with decision 0030 itself (`4137ec0d3`, 2026-09-04). **R4's rate-warping
sibling exists**; the closeout's "crates/dclutch-svm-harness/tests/ is unchanged
by the diff" is true of the diff and false as a statement about the tree.

The epoch half is ruled **not owed**. A `--slots-per-epoch 32` journey walk
would control for the hypothesis that an epoch boundary moves the rate — and
decision 0030 §1 records that the instrument already refuted that hypothesis:
the brief that reached ember said founding-time constant against a live sysvar,
and it is the opposite. Devnet's rate moved for a cluster-side reason, not an
epoch one. Building a control for a refuted mechanism buys documentation, not
evidence. If anyone wants it later it is one journey row; it should not sit in a
ledger as owed work.

### 3.6 R2 — the constant is still absent, and that is now stated rather than claimed

`EXTEND_PROGRAM_MINIMUM_ADDITIONAL_BYTES_V3` occurs exactly once in this
worktree: in the BUILD doc's own R2. Not written here — the extension escape is
a cohort-time operator affordance with no offline test, and this lane has no way
to measure the Loader refusal it would encode. `live_elf_padding_bytes` exists
on main and is untouched. R2 remains a design, and the merge lane should read it
as one.

## 4. Tests added, and what each proves

Six new tests, all in
`tools/local-validator/bootstrap/successor/src/upgrade.rs`, plus one corrected.

| test | proves |
| --- | --- |
| `a_completed_upgrade_row_binds_the_two_digests_the_audit_re_reads` | the wall AND the fix in one run. Its **positive control** is that the journal does not LOAD before the bind (`custody dump exists but the set journal pins no digest`), and its assertion is that the two digests the binder writes are byte-identical to the ones `MixedSetFixture` independently computed for every other test that reads a completed row, and that the journal then loads under the full closure. |
| `without_execute_the_row_binder_writes_nothing` | the preflight reports both digests and leaves the journal byte-identical. |
| `the_row_binder_refuses_a_bound_row_a_carry_forward_and_an_unfinished_upgrade` | four refusals by their own words: a row already carrying digests, a CarryForward row, a role the journal does not name, and an Upgrade with no receipt on disk. |
| `a_dump_that_disagrees_with_its_own_receipt_is_refused_and_writes_nothing` | the substituted-evidence case, naming the receipt's own digest in the refusal, and the journal byte-identical after. |
| `a_moved_baseline_is_re_pinned_and_the_journal_it_stranded_loads_again` | the same shape for the baseline binder, again with the stranding as its positive control: `load_set_journal_path` refuses `custody baseline` before the re-pin and admits after. |
| `a_baseline_is_never_re_pinned_under_a_completed_upgrade_or_a_pending_extension` | the two refusals that keep the binder from rewriting evidence. |
| `the_send_window_is_entered_with_no_audit_after_the_blockhash` (**corrected**) | see §5.3. |

The four tests the branch already carried (two in
`crates/dclutch-market/tests/capability_manifest__funded_rent_v1.rs`, two in
`upgrade.rs`) are unchanged except for that correction, and all pass.

**The gap that remains, named:** `ResolutionError::FundedRent` and
`CoreSbfError::FundedRent`-via-`series_consume` have no test asserting them,
because reaching either needs a program-test with a warped rate and this lane
ran no SBF build. The neighbouring Resolution codes at
`docs/reference/refusals.md:426-428` already carry an empty campaign column, so
the census will accept them silently. The shape to copy exists:
`resolution_core_v3_lifecycle.rs:5907`.

## 5. What a reviewer should distrust in `BUILD_deploy-lifecycle.md`

Its own §2.1 lists five false claims and is right about all five. Three more:

1. **§2.1 on R4** — "Neither the journey walk nor the rate-warping sibling was
   written." The rate-warping sibling **was** written, on 2026-09-04, by the
   decision this branch implements (§3.5). "Unchanged by the diff" is not the
   same statement as "not written", and the second is what a reader takes away.
2. **§5 on `the_send_window_is_entered_with_no_audit_after_the_blockhash`** —
   presented as proving `boundary_checks == 3`. It proves 4. The walk audits
   once **before the first receipt exists** (`core_effect`'s peer at
   `upgrade.rs`'s `existing == None` arm, unconditional and on main since before
   this branch), and the maker counted receipt phases. The test compiled under
   `cargo check --tests` and was never run, which is exactly the gap between
   §7's "green" and a green test. Corrected here, and strengthened: the fake now
   samples the counter at the blockhash fetch, so the test asserts *"every audit
   this walk owes is spent before the blockhash"* — measured — instead of listing
   the phases it believed ask.
3. **§1.2's premise, understated.** "The only writer was the AlreadyCurrent row"
   is true and not the whole wall. The measured fact is worse: the loader
   refuses a dump that exists while the journal pins no digest, and
   `devnet-upgrade-v1` writes both the receipt and the dump — so from the moment
   an Upgrade succeeded, **its own journal could not be read by anything in the
   tree, the audit included**. That is why the only author was a script that
   parsed the JSON directly, and it is why both new commands read the journal
   without its pinned-file closure and then prove the rewritten one loads.

Also worth knowing: the doc's §2.C, §2.B and §7 warning list are exactly the
compiler's, as it says. That part held perfectly.

**And one thing to distrust in the branch rather than the doc.** §1.1's own
migration opened a defect nothing in the tree could see: the activation
receipt's producer moved to the recorded rate and its consumer, in a different
program, did not (§7.1, item 1). No compiler spans that boundary, no test on
this branch links both programs, and the BUILD doc's §5 says so about its own
tests — "none of them touches the program, which is why nothing on this branch
would have caught either compile break". That sentence was more right than it
knew.

## 6. The rent census

See §7. This lane migrated the four sites decision 0030's discipline names in
the two programs it touches, and censused the rest rather than claiming it.

**The `tools/gauntlet/census rent` command §1.1 promised was not built**, and
should not be: the route census enumerates routes, magics and refusal codes from
the AST, and "is this rent figure compared against an account that already
exists?" is a different question with a different instrument.
`tools/seam-audit/` is its home — a standing gate over mechanical seam classes
with a triaged baseline and seven classes already — and a `FUNDED_RENT` class
there is one rule function plus baseline rows. Not written here: a static
classifier that misfires is worse than none, and the residue below is small
enough to name by hand.

## 7. Census: every `minimum_balance` / `is_exempt` under `programs/*/src`

Read-only, by a delegated agent, against this branch AFTER §3.2's and §3.3's
migrations. 142 raw hits; 24 excluded as `tests.rs` files and 21 as `#[cfg(test)]`
modules; **97 classified**. A further 201 hits live under `programs/*/tests/`
and `programs/*/program-test/**` and were not censused.

The classification rule, so it can be disagreed with: *"already exists" means
funded before this instruction began.* An account created by this instruction —
including by its own CPI, with a checked zero balance at entry — is CREATING
even where the syntax is `!=` against `.lamports()`.

| class | count | what a rate change does |
| --- | --- | --- |
| **A — CREATING** | 58 | nothing. Reading the sysvar is correct: the rate that funds is the rate that checks. |
| **B — FLOOR-OVER-EXISTING** | 27 | a rate RISE refuses. Decision 0030 §5 already names this class as owed and assigns it (`~112 is_exempt floor sites … by the RENT-FLOORS lane`); it is not this family's. |
| **C — EXACTNESS-OVER-EXISTING** | **7** | a rate change in EITHER direction refuses. |
| UNCLASSIFIED | 5 | three are prose in doc comments; two are named below. |

### 7.1 Class C — three closed here, four handed on

**Closed by this branch** (commit `9552f6104`):

1. `programs/dclutch-core-sbf/src/resolution.rs:542` — **a split this branch
   itself created.** `0e8940c8a` moved the activation receipt's PRODUCER
   (`core_effect.rs`) to `active.funded_rent_minimum(...)` and left its only
   CONSUMER on `rent.minimum_balance(...)`, with `receipt.ledger_rent_lamports
   != ledger_rent_lamports` between them. On `main` both sides read the sysvar
   and agreed. After that commit, Core's `Accept` refuses every activation
   acknowledgment made after the cluster re-rates — the cohort-15 wall, in the
   opposite direction, across a program boundary no compiler spans. **If nothing
   else in this document is read, read this: a producer/consumer migration that
   crosses a program boundary needs a census, not a compiler.**
2. `programs/dclutch-resolution-proof-sbf/src/pre_market_funding_abort_v1.rs:134`
   — the same figure is the custody conjunct, the `close_ledger` refund and the
   receipt's `rent_refund_lamports`, so the sysvar read moved three facts.
3. `programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:1649` — the
   failure ledger's custody conjunct and the relay's planned refund.

**Handed on, with the fix shape:**

4–5. `programs/dclutch-claims-sbf/src/protocol_position_v2.rs:618` and `:619` —
at Position CLOSE, today's `minimum_balance` must equal
`request.position_rent_principal` / the admission width's principal, and
`authenticate_admission` (`:1160-1161`) separately binds those to the
**admission record written at founding**. So today's sysvar must equal a
founding-time recorded figure, transitively. This is where
`ProtocolPositionSbfErrorV2::FundedRent = 0x514B` (S18) becomes owed: fix the
site, then split the code, in that order. **Whoever does this reverses the S18
ruling in §3.3 and should say so.**

6–7. `programs/dclutch-core-sbf/src/generic_founding_v1.rs:877` and `:879` — the
market PDA's and the founding permit's live lamports must equal today's minimum
exactly, and this route performs no top-up (its sibling `found.rs:869-871`
does). The censusing agent flagged this as its least-confident C: if the funding
is always an earlier instruction of the SAME transaction the rate exposure is
nil. Even then the exact equality is griefable by a one-lamport donation, which
this tree has already relaxed to a floor twice for that exact reason
(`rational_lifecycle_v2.rs:1317-1322`, `protocol_position_v2.rs:1084-1091`).
Settle it by finding the transfer that precedes `GenericFound`.

### 7.2 The two UNCLASSIFIED sites a reader should settle

- `programs/dclutch-trading-sbf/src/hot_v3/lifecycle.rs:32` — the quoted minimum
  goes into a **scalar register** and is revalidated only against itself. Whether
  it is spent as a funding amount or compared to a live balance is decided by the
  family's data-driven transition bytecode, which no static read resolves. If any
  shipped `StateLifecyclePolicyV5` compares it, this is a class-C defect hiding
  behind data. To settle: take one shipped policy's current-rent-quote
  declaration, read its `scalar_destination`, and trace that register.
- `programs/dclutch-core-sbf/src/open_market.rs:610` — literally class C
  (`vault.lamports() != rent.minimum_balance(...)`), classified A because
  `programs/dclutch-custody-sbf/src/lib.rs:1314-1316` requires
  `vault.lamports() == 0` before creating it in the same instruction's CPI. To
  settle: confirm `authenticate_vault_poststate` has no other caller. The same
  question and the same answer shape governs
  `programs/dclutch-resolution-proof-sbf/src/provider_transport_v3.rs:160`.

### 7.3 Two facts worth keeping from the B column

`programs/dclutch-trading-sbf/src/outer.rs:1961` floors by *arithmetic* rather
than comparison — `root_lamports.checked_sub(exact_root_rent)` underflows and
refuses when the live root holds less than today's minimum. A census that greps
for comparison operators will not see it, which is the third spelling decision
0030 §4 says the earlier pass found by hand.

`programs/dclutch-trading-sbf/src/hot_v3/lifecycle.rs:1042` is dual-use: it funds
the `Create` arm and floors an existing balance in the `Authenticate` arm
(`crates/dclutch-vm/src/account_profile/lifecycle_v3.rs:3717`). One site, two
classes, and only one of them is safe.
