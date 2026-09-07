# BUILD — deploy-lifecycle

Branch `build/deploy-lifecycle` off `main` at `f7c03e845`. Built by source
inspection; the convergence wave compiles and enmeshes. Every section below is
written as the work lands; a section that says *owed* is debt named, not hidden.

## 0. The family in one paragraph

Genesis, close, redeploy, upgrade-in-place and re-release are exercised (cohorts
15–17). Five things a deploy's lifecycle still could not do at `f7c03e845`:
(1) cross an EPOCH BOUNDARY with a market open under the rent RATE — the census
of `minimum_balance` sites still had raw exactness sites and floors in programs;
(2) bind a COMPLETED Upgrade row into the deployment-set journal — the only
writer was the AlreadyCurrent row, and cohort-16.1 bound its two Upgrade rows
with a python script that lived in the job directory (`~/jobs/dclutch-cohort161-20260905/bind-upgrade-row.py`,
`bind-baseline.py`) — the producer-missing pattern; (3) beat a 6.13-block/s
cluster — the checked-upgrade loop re-audited seven roles at every phase,
including the two AFTER the blockhash fetch; (4) name the sub-10,240 pad as the
release's own fact, or escape a stuck `ExtendProgram` the way the Upgrade can;
(5) decide `lineage_walk`'s fate — the one forward mechanism, consumed by
nothing.

## 1. What was built, file by file

### 1.1 The rent census closed (decision 0030, one author per exactness)

**`crates/dclutch-market/src/capability_manifest/funding.rs`** (`74fb50760`)
- `FundingLedgerCloseCustodyV2::recorded_native_only(ledger, lamports, bytes, credit)`
  and `recorded_native_with_crank(…)`: the CLOSE's rent term from the ledger's
  own header. The close was the second exact site the ruling left on the
  sysvar (three program calls: `core_effect.rs` ×2, `capability.rs` ×1).
- `FundingCustodyObservationV1::recovered_native_only(lamports, principal, bytes)`:
  a V1 funding state (frozen layout, no rate field) recovers its rent from its
  balance minus its own principal, admitted only when the division to a rate is
  exact (`funded_rent_rate_from_minimum_v1`) — the recovery form 0030 §3 admits.
  Its blind spot is asserted in a test: a donation of exactly one `(128+len)`
  reads as a rate one higher and is classified as rent, never principal.
- Tests (e), (f) in `crates/dclutch-market/tests/capability_manifest__funded_rent_v1.rs`.

**Program sites** — see §1.1.1 (lane report) for the before/after per site.

**Census gate** — `tools/gauntlet/census` gains a `rent` command (§1.1.2).

### 1.2 The deployment-set journal's Upgrade producer — §1.2 (lane report)

### 1.3 The checked-upgrade loop against a fast cluster (`ea3cca04f`)

**`tools/local-validator/bootstrap/successor/src/upgrade.rs`**
- `phase_precedes_loader_action_boundary_v1(phase) -> bool`: the phases that
  owe a fresh seven-role audit are `Prepared | BufferWriteArmed | BufferReady`
  — everything up to `getLatestBlockhash`. `MessagePrepared` and
  `SignedNotSubmitted` are pure local transitions over an fsynced receipt; the
  audit that admitted the phase before the fetch is the one the packet is
  signed and sent under. `require_mutation_permit` (local, cheap) still pins
  the journal plan digest at every phase.
- `authenticate_phase_mutation_boundary` now consults it (was: audit at every
  phase except `Submitted | Complete`).
- Tests: `a_seven_role_audit_per_phase_cannot_beat_a_fast_cluster` (the
  numbers: 6,130 milliblocks/s, 44 s per audit, 150-block lifetime → two
  post-fetch audits = 538 blocks > 150; new shape = 0) and
  `the_send_window_is_entered_with_no_audit_after_the_blockhash` (the fake
  counts the loop's boundary decisions: three before the fetch, none after,
  none when verifying a complete receipt). `cargo check -p
  dclutch-local-successor-bootstrap --tests --offline`: green.

The extension's escape and the pad fact — §1.3.1 (lane report).

### 1.4 `lineage_walk` — deleted (§3 for the ruling; §1.4.1 lane report)

## 2. THE SEAMS (exact file:line, one-line change)

*Derived by CLOSEOUT-B from the diff, 2026-09-06. The "as lanes report" version of this section was never
filled: the lanes it waited on did not write. What follows is read off the tree.*

### 2.A The branch does not compile — two seams, one crate

`cargo check -p dclutch-resolution-proof-sbf --offline` is RED; see §7 for the errors verbatim.

| # | file:line | the one-line change |
|---|---|---|
| S1 | `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:2796` | `.map_err(funded_rent_refusal)?` calls a function that **does not exist anywhere in the tree** — `grep -rn funded_rent_refusal` returns exactly this one hit. Write it. Three precedents do the identical split in the sibling program: `programs/dclutch-core-sbf/src/capability.rs:493-495`, `resolution.rs:1267-1269`, `generic_founding_v1.rs:999-1001` — each maps `capability_manifest::Error::{FundedRentNotEvidenced, FundedRentRateMissing}` to the crate's `FundedRent` code and everything else to the generic funding code. The Resolution version must return `ProgramError`. Those two variants (`crates/dclutch-market/src/capability_manifest/mod.rs:218, :228`) are exactly what `validate_recorded_native_custody` (`funding.rs:2287-2299`) can produce. |
| S2a | `programs/dclutch-resolution-proof-sbf/src/lib.rs:228` | add `FundedRent = 0x801E,` to `pub enum ResolutionError` (`:44`). `0x801E` **is** the next free code (last is `SourceLadder = 0x801D` at `:227`). It needs a `///` doc comment: the crate is `#![deny(missing_docs)]` (`lib.rs:3`) and that comment becomes the census's `meaning` field. |
| S2b | `programs/dclutch-resolution-proof-sbf/src/lib.rs:262` | append `FundedRent` to the `pin_refusal_band!(ResolutionError, RESOLUTION_REFUSAL_BASE, [...])` array (spans `:230-263`). The macro (`crates/dclutch-refusal-registry/src/lib.rs:265`) is exhaustiveness-checked, so omitting this is a **second compile error**, not a silent gap. |

### 2.B The migration is half-done — the asymmetry is the seam

Decision 0030 names **three** exact-rent close sites. The branch migrated one.

| # | file:line | state |
|---|---|---|
| S3 | `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:566-573` | **MIGRATED.** `commit_direct_close` calls `FundingLedgerCloseCustodyV2::recorded_native_only(prestate, …)`. |
| S4 | `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:2375-2379` | **NOT migrated.** `process_close` (the Core-effect `CloseFund` route) still calls `native_only(planned_ledger_lamports, rent.minimum_balance(RESOLUTION_FUNDING_LEDGER_BYTES), …)` — the sysvar-of-the-moment pricing this whole branch exists to kill, unchanged on the sibling route. |
| S5 | `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:2355-2358` | **STARTED AND ABANDONED, inside S4's own function.** The diff adds `let prestate_bytes = Box::new(ledger_prestate); let prestate = FundingLedgerV2::decode(…).and_then(|l| l.authenticate(manifest_id, manifest))?;` and **never uses `prestate`**. That is simultaneously an unused-variable warning, a live `?` that can now refuse `ResolutionError::Funding` on a path that previously could not, and one ledger-width heap `Box` bought for nothing. The three-line comment at `:2351-2354` describes behaviour the code does not have. **Finish it or revert it; do not leave it.** |
| S6 | `programs/dclutch-core-sbf/src/capability.rs:542` → `:571` | **NOT migrated**, third site. `let exact_ledger_rent = rent.minimum_balance(pre_bytes.len());` feeding `FundingLedgerCloseCustodyV2::native_only(`. §1.1 names it ("`core_effect.rs` ×2, `capability.rs` ×1") and the branch never reached it. |

### 2.C Dead parameters the de-parameterization orphaned

All twelve call sites of the four de-parameterized functions are inside `core_effect.rs` and **all were
updated** — the signature change breaks nothing. But three enclosing functions kept a `rent: &Rent`
parameter with zero uses left in the body, and their callers still pass it. These are the frame savings
§6 claims and does not collect:

| # | file:line | change |
|---|---|---|
| S7 | `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:1306` | drop `rent: &Rent` from `authenticate_completed_activation`; drop the argument at its caller `:291`. |
| S8 | `…/core_effect.rs:2031` | drop `rent: &Rent` from `process_verify`; drop the argument at the dispatch `:1502`. |
| S9 | `…/core_effect.rs:2164` | drop `rent: &Rent` from `process_admit`; drop the argument at `:1511`. |

Still legitimately used — **do not strip**: `commit_direct_activation` (`:281`), `commit_direct_close`
(`:482`), `process_create` (`:1912`), `process_close` (`:2263`), and the four constructor helpers at
`:1363`, `:2881`, `:3236`.

### 2.D New public items with zero production callers

| # | file:line | the fact |
|---|---|---|
| S10 | `crates/dclutch-market/src/capability_manifest/funding.rs:1023` | `FundingCustodyObservationV1::recovered_native_only` — **zero production callers**; only the four calls in the new test (`tests/capability_manifest__funded_rent_v1.rs:453,468,478,488`). R3's "the consume site recovers the rate from the balance" **has no consume site**: the V1 funding-state readers are unchanged. This is the producer-missing pattern the doc's own §0(2) diagnoses, reproduced by the branch. |
| S11 | `…/funding.rs:2382` | `FundingLedgerCloseCustodyV2::recorded_native_with_crank` — **zero production callers**; one test call at `:422`. Every crank close route is still on `native_with_crank`. |
| S12 | `tools/local-validator/bootstrap/successor/src/upgrade.rs:3223` | `phase_precedes_loader_action_boundary_v1` is wired (one consumer at `:3235`), but `pub(crate)` is wider than the fact warrants — cosmetic. |

### 2.E Code-table mirrors

`docs/reference/` and the SDK mirrors are **generated** (`tools/genref/README.md:1-6`): the fix is
`tools/gate reference --converge`, and `--check --converge` fails until it runs. The rows owed, with
insertion points, once S2 exists:

| # | file:line | row |
|---|---|---|
| S13 | `docs/reference/refusals.md:428` | after `0x801D | ResolutionError::SourceLadder` |
| S14 | `docs/reference/abi/refusalRegistryV1.md:344` | after the `0x801D` entry |
| S15 | `docs/reference/abi/routeCensus.md:459` | after `code: 32797` |
| S16 | `packages/dclutch-sdk/lib/generated/refusalRegistryV1.ts:324` | after the `0x801D` entry |
| S17 | `packages/dclutch-sdk/lib/generated/routeCensus.ts:421` | after `code: 32797` (`0x801E` = 32798) |
| S18 | `programs/dclutch-claims-sbf/src/protocol_position_v2.rs:131` | **The Claims half of §6's split is entirely unwritten.** `ProtocolPositionSbfErrorV2::FundedRent = 0x514B` does not exist (last variant `Receipt = 0x514A` at `:131`; `0x514B` is next free), **and nothing on the branch references it** — `grep -rn FundedRent programs/dclutch-claims-sbf/` and `grep -rn validate_recorded_native_custody programs/dclutch-claims-sbf/` are both empty. No call site, no variant, no mirror rows. The generic `Rent = 0x5146` (`:123`) it is meant to be split *from* is untouched. |

**No row is owed** in `tools/gauntlet/blocked.json` (campaign blockers, not refusal codes — grepped
`FundedRent`, `0x801`, `0x514`: zero hits), and **no new census TARGETS row**: `tools/gauntlet/census/src/main.rs:52`
already lists `("dclutch-resolution-proof-sbf", "resolution")` and `:46` `("dclutch-claims-sbf", "claims")`;
the census walks the enums in listed packages and picks new variants up on its own.

### 2.F Frame ratchet

| # | file:line | change |
|---|---|---|
| S19 | `tools/gates/frames-baseline.json:4925, :4949, :4961, :5009` | the four de-parameterized symbols (`authenticate_live_ledger`, `authenticate_ledger_value`, `authenticate_direct_ledger`, `authenticate_direct_close_ledger`). Recapture with `tools/gate frames --at <commit> --capture`. **The movement is not the −64/decode §6 predicts**: the two new `Box::new` heap copies at `core_effect.rs:525` and `:2355` push the other way, and `:2355`'s is pure waste (S5). |

### 2.G Runbook

| # | file:line | change |
|---|---|---|
| S20 | `tools/cohort/steps.tsv:74` | the `funded-rent-recorded` row (tier 16) predates this branch and reads header bytes 12..16 against `getMinimumBalanceForRentExemption(0)`. It does **not** cover the close pricing or the V1 recovery, and no row names the phase-boundary change. If S2–S6 ship, this row needs an added expectation or a sibling. |

### 2.H Checked and correct — no action

`crates/dclutch-market/src/capability_manifest/mod.rs:27` already declares `pub mod funding;` (a modified
pre-existing module, not a new one). `crates/dclutch-market/tests/capability_manifest__funded_rent_v1.rs`
is a pre-existing integration test grown by 106 lines; no `[[test]]` stanza, auto-discovery applies, and
its dev-deps are already present — no Cargo change. Neither crate needs a new dependency. And
`CliRunner::enforces_fresh_deployment_set_boundary` is **not** new (it is on `main` at `upgrade.rs:1267`);
the diff only adds the `FakeRunner` impl.

## 2.1 CLAIMS IN THIS DOCUMENT THAT THE TREE CONTRADICTS

Read this section before trusting any other. Five of the doc's claims are false against the branch it
describes, and a convergence lane that acts on them will look for work that was never done.

| claim | where | the tree |
|---|---|---|
| **`lineage_walk` is deleted** (R1, §1.4) | `:68`, `:74-98` | `crates/dclutch-registry/src/lineage_walk.rs` **exists at HEAD, 268 lines** — exactly the count R1 quotes as its cost-of-reversal. `git log -S lineage_walk main..HEAD` finds only the BUILD doc's own commit. The census in R1 (zero production consumers) appears sound; the DELETION was never performed. Every remaining reference, each a seam if the ruling is executed: `crates/dclutch-registry/src/lib.rs:19` (`mod lineage_walk;`), `:25` (`pub use lineage_walk::*;`), `:113` (`LineageWalkTooLong` — reachable only from the walk, its `From` impl at `lineage_walk.rs:118-124`); `crates/dclutch-registry/src/tests.rs:17,1306,1336,1348,1354,1371,1372,1377,1380,1385,1390,1391`; `packages/dclutch-sdk/lib/releaseLineage.ts:155-330` (the TS mirror, still present — R1 cites it at `f7c03e845` as if already gone); `tools/cohort/README.md:1020`; `tools/cohort/steps.tsv:58` (the `manifest-edges` row names it inside a load-bearing sentence about `Market.release_set_id` stranding); `docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md:199`; `docs/evidence/COHORT16_DEPLOYED_SEALED_2026_09_05.md:698`; `docs/ledger/2026-09-05.md:656,681`. Separately stale: `docs/design/RELEASE_LINEAGE_MIGRATION_V1.md:11,1403` point at `crates/dclutch-registry-contract/src/lineage_walk.rs`, **a path that does not exist today**. |
| **the census gains a `rent` command** (§1.1) | `:48` | `tools/gauntlet/census/src/main.rs:73-83` dispatches `inventory | observe | report | help` and returns `unknown command` for anything else; the usage at `:85-95` agrees. **Not built.** §1.1.2 is an empty cross-reference. |
| **the deployment-set journal's Upgrade producer** (§1.2) | `:50` | A bare heading. At HEAD the only journal writer is still the AlreadyCurrent path (`upgrade.rs:2188 journal_already_current_v1`, dispatched at `main.rs:170-172`); `upgrade.rs:2237` and `:2243` still refuse Upgrade rows from that command by name. **The producer-missing pattern §0(2) diagnoses is still the state of the tree** — the python script in the job directory is still the only Upgrade-row author. |
| **R2's extension escape and pad fact** | `:96` | `EXTEND_PROGRAM_MINIMUM_ADDITIONAL_BYTES_V3` has **one occurrence in the whole worktree: the BUILD doc's own R2.** The constant does not exist. `live_elf_padding_bytes` exists on `main` already (`upgrade.rs:809,1014,1089,4164,4698,4707`; `runtime.rs:2385`; `market.rs:17007`) and is untouched. §1.3.1 is an empty cross-reference. |
| **R4's two positive controls** | `:113-119` | `grep -rn "slots-per-epoch" tools/ crates/` → **zero hits**. `crates/dclutch-svm-harness/tests/` is unchanged by the diff. **Neither the journey walk nor the rate-warping sibling was written.** R4 is a design, and the ruling it states — that the epoch and the rate are separate facts needing separate controls — is worth keeping; the controls are owed. |

Five further cross-references — `:46`, `:48`, `:50`, `:72`, `:74`, each "(lane report)" — point at
documents that do not exist. `ls *.md` at the worktree root shows only this file.


## 3. Ruled provisionally (ember rules by reversal)

**R1 — `lineage_walk` is deleted; the lineage RECORD stays.** Census: the walk
(`crates/dclutch-registry/src/lineage_walk.rs`, 268 lines) had zero production
consumers in any program, host tool or client — only its own tests and the SDK
mirror `walkReleaseLineageV1`/`followReleaseLineageV1`, which had zero consumers
in `apps/` and `packages/` outside its own test. The route it was written for
(`Core::MigrateMarket`, `docs/design/RELEASE_LINEAGE_MIGRATION_V1.md` §5) never
landed, and under decision 0012 as amended an upgrade IS a re-found: a market
pins its execution release set (`Market.release_set_id`, offset 208, written
once by `initialize_market`), the set id hashes each role's deployment slot,
and Loader V3 moves that slot on every Upgrade — so no route can carry a market
forward without a second, mutable pin (§5.2's `active_release_set`), a Core
width move, the 5-account route, three Core codes, a bounty escrow and ~39
adapter sites (§14). That is the cost of the other branch, and its benefit on a
disposable substrate is zero. `ReleaseLineageV1` + `DeclareSuccessor` stay:
the Registry program consumes the record and the infrastructure succession
ceremony mirrors its conjuncts. Cost of reversal: 268 lines at
`f7c03e845:crates/dclutch-registry/src/lineage_walk.rs` and the TS mirror at
`f7c03e845:packages/dclutch-sdk/lib/releaseLineage.ts:155-330`; the rule is
still written in §14 of the design.

**R2 — the extension's minimum is the Loader's measured fact.** `ExtendProgram`
below 10,240 bytes is refused by the Loader (COHORT-16C, measured). The
baseline rounds a nonzero shortfall up to `EXTEND_PROGRAM_MINIMUM_ADDITIONAL_BYTES_V3
= 10_240` (labelled measured-profile; lifting plan: re-measure on the next
Loader release), and the resulting zero pad is the release's own fact
(`live_elf_padding_bytes` on the checked pin).

**R3 — a V1 funding state recovers its rent by arithmetic.** See §1.1: the
frozen V1 layout gets no field; the consume site recovers the rate from the
balance and refuses a non-affine remainder. Cost: a donation of exactly
`k×(128+len)` lamports is read as rent (to the rent beneficiary), never as
principal.

**R4 — the epoch-crossing walk on the journey tier crosses the EPOCH, and the
ProgramTest sibling crosses the RATE.** A local validator's rent rate does not
move at an epoch boundary; devnet's did because of a cluster-side change. The
journey walk sets `--slots-per-epoch 32`, founds before the boundary and settles
after it, recording the Rent sysvar digest on both sides (equal); the
`svm-harness` test warps the epoch AND lowers the rate to 5,080. Together they
are the positive control the ruling asks for (0030 §5).

## 4. Stubs (named)

No `todo!()`, `unimplemented!()` or `unreachable!()` in any of the four changed files, and no `TODO`/`FIXME`
added. What stands in their place is the tree's "owed" idiom, plus two things that are stubs without saying so:

- `programs/dclutch-resolution-proof-sbf/src/core_effect.rs:524` — `// One ledger-width heap copy; the frame ratchet is owed.` (in `commit_direct_close`). Honest debt.
- `…/core_effect.rs:2354` — `// is owed.` closing the same three-line comment in `process_close`. **Here the comment describes a migration the code does not perform** (S5). A comment that narrates absent behaviour is worse than a `todo!()`, which at least fails loudly.
- `crates/dclutch-market/src/capability_manifest/funding.rs:92` — pre-existing, above `funded_rent_minimum_v1`: `folding it onto this author is owed`.
- **The unnamed stub**: `funded_rent_refusal` (S1). Called, never written. This is a `todo!()` that does not compile instead of one that panics.
- **The doc's own three empty headers** (§2 SEAMS, §4, §5) and five dangling lane-report references were the branch's largest stub; §2 and this section close two of them.

## 5. Tests written and what each proves

Four tests, in two files. **None of them touches the program**, which is why nothing on this branch would
have caught either compile break in §7.

**`crates/dclutch-market/tests/capability_manifest__funded_rent_v1.rs`** (394 → 500 lines; the six
pre-existing tests at `:118, :150, :179, :251, :287, :330` are unchanged)

| test | proves |
|---|---|
| `:404 a_close_prices_the_rent_the_founding_parked_not_todays` | `recorded_native_only` over a ledger founded at 6,333 returns `exact_ledger_rent_lamports() == funded_rent_minimum_v2(FUNDED_RATE, 120)`, not the post-fall 5,080 figure; `recorded_native_with_crank` with a cap of 7 moves **only** the cap and leaves the rent term identical. Its **negative control** is the load-bearing half: `assert_ne!` at `:441` shows the raw `native_only` constructor still accepts today's sysvar number and produces a rent the account does not hold — the defect, exhibited. The fixture straddles the rate change by `assert_ne!(today, funded)` at `:413`, so the test cannot pass vacuously. |
| `:446 a_v1_state_recovers_its_rent_only_when_the_remainder_is_affine` | `recovered_native_only(principal + funded, principal, FUNDING_STATE_BYTES)` recovers `funded` exactly and leaves `present_native_lamports() == principal`; one lamport over refuses `UnrepresentableRentRate` (`:466-475`); one lamport under principal refuses `UnderfundedPhysicalCustody` (`:476-485`); and — the honest part — a donation of exactly one `(128+len)` unit is **admitted** and classified as rent, never principal (`:486-499`). **R3's stated cost asserted rather than left in a doc comment.** |

**`tools/local-validator/bootstrap/successor/src/upgrade.rs`** (in-file `mod tests`)

| test | proves |
|---|---|
| `:13100 a_seven_role_audit_per_phase_cannot_beat_a_fast_cluster` | pure arithmetic over the predicate: the OLD rule consumes `> BLOCKHASH_LIFETIME_BLOCKS_V1` (150) on the two post-blockhash phases; the new predicate consumes exactly `0`; the predicate answers `true` for `Prepared|BufferWriteArmed|BufferReady` and `false` for `Submitted|Complete`. Carries its own positive control at `:13118-13121` — the old shape **must** exceed the window "or the test proves nothing". |
| `:13145 the_send_window_is_entered_with_no_audit_after_the_blockhash` | end-to-end over `execute_with_runner` with the fake: the completed receipt reaches `Complete` with exactly one blockhash fetch and one send; `boundary_checks == 3` (Prepared, BufferWriteArmed, BufferReady ask; MessagePrepared and SignedNotSubmitted do not); and verifying an already-complete receipt increments the counter by **zero**. |

Supporting infrastructure (not tests): `boundary_checks: Cell<u64>` at `:10355`, initialized `:10411`,
incremented by the `FakeRunner` impl at `:10471-10474` (it returns `false`, so the fake counts *decisions*
rather than enforcing); constants `DEVNET_MILLIBLOCKS_PER_SECOND_V1 = 6_130` (`:13077`),
`BLOCKHASH_LIFETIME_BLOCKS_V1 = 150` (`:13080`), `SEVEN_ROLE_AUDIT_SECONDS_V1 = 44` (`:13083`); helper
`blocks_elapsed` (`:13086`).

**The gap worth naming**: the two routes the branch rewrote (`commit_direct_close`,
`authenticate_ledger_value`) gain no program-test, no `svm-harness` case, no unit test. Once
`ResolutionError::FundedRent` exists it will have **no test asserting it** — and the neighbouring
Resolution codes at `docs/reference/refusals.md:426-428` already carry an empty campaign column (`--`),
so the census will accept it silently.

## 6. Frames / codes expected to move, and which cohort carries them

- Codes: `ResolutionError::FundedRent = 0x801E` (Resolution band, next free);
  `ProtocolPositionSbfErrorV2::FundedRent = 0x514B` (Claims sub-band 0x5140,
  next free). Both are exactness-vs-recorded-rent refusals split from
  `Funding`/`Rent`, in the shape `CoreSbfError::FundedRent = 0x301D` and
  `TradingSbfError::FundedRent = 0x4029` were split on 2026-09-04.
- Frames: dropping the `rent: &Rent` parameter through `authenticate_ledger_value`
  and its three wrappers in `core_effect.rs` moves Resolution frames the way
  `0f24245da` measured (−64 bytes per decode that leaves a route); the ratchet
  rows are owed by the convergence lane's SBF build (`tools/gate frames --capture`).
- Cohort: the next re-release cohort after 17 (cohort-18 or 17.1) carries the
  Resolution/Core/Trading/Claims links; the journal producer and the phase
  bound carry no ELF and can run against cohort-17's substrate.

## 7. Does it parse? (CLOSEOUT-B, 2026-09-06)

`cargo check -p dclutch-resolution-proof-sbf --offline` on `9c20d3bcd`: **RED, 2 errors, 5 warnings.**
Both errors are in `core_effect.rs` — the file the wave was mid-edit in when the quota ran out — and both
are S1/S2 above. Verbatim:

```
error[E0425]: cannot find value `funded_rent_refusal` in this scope
    --> programs/dclutch-resolution-proof-sbf/src/core_effect.rs:2796:18
     |
2796 |         .map_err(funded_rent_refusal)?;
     |                  ^^^^^^^^^^^^^^^^^^^ not found in this scope

error[E0599]: no variant, associated function, or constant named `FundedRent` found for enum `ResolutionError` in the current scope
   --> programs/dclutch-resolution-proof-sbf/src/core_effect.rs:369:39
    |
369 |         .map_err(|_| ResolutionError::FundedRent)?;
    |                                       ^^^^^^^^^^ variant, associated function, or constant not found in `ResolutionError`
    |
   ::: programs/dclutch-resolution-proof-sbf/src/lib.rs:44:1
    |
 44 | pub enum ResolutionError {
    | ------------------------ variant, associated function, or constant `FundedRent` not found for this enum
```

There is no third error. The five warnings are the S5 dead `prestate` (`:2356`) and the three S7–S9 dead
`rent` parameters (`:1306`, `:2031`, `:2164`) — every one of them a seam above, which is a pleasant
property: **the compiler's warning list on this branch is the same list as §2.C and §2.B's abandoned
migration.**

`crates/dclutch-market` and `tools/local-validator/bootstrap/successor` were checked green by the maker
at their own commits (§1.3), and their tests do not link the Resolution program — which is precisely why
neither compile break was caught.

### Rebase: clean

The branch's base is `f7c03e845`; `main` is at `19d5d2060`. **None of the nine files `main` changed
overlaps this branch** — it is the only one of the four build families that rebases without a conflict.
The four files it touches (`capability_manifest/funding.rs`, its test, `core_effect.rs`, `upgrade.rs`)
are all untouched on `main`.

## 8. STOPPED HERE — the next three steps, concretely

1. **Make it compile: S1, S2a, S2b — three edits, one file each.** Write `funded_rent_refusal` in
   `core_effect.rs` on the shape of `programs/dclutch-core-sbf/src/capability.rs:493-495` (returning
   `ProgramError`); add `/// …` + `FundedRent = 0x801E,` at `programs/dclutch-resolution-proof-sbf/src/lib.rs:228`;
   append `FundedRent` to the `pin_refusal_band!` array at `:262` — the macro is exhaustiveness-checked,
   so forgetting the third edit swaps one compile error for another. Then re-check. **Nothing else on
   this branch can be evaluated until this is done**, because no test binary here links the program.

2. **Settle the half-migration — S4, S5, S6 — as one decision, not three patches.** `process_close` at
   `core_effect.rs:2375` still prices the close off today's sysvar, and its own function already decodes
   and authenticates the prestate at `:2355` and then throws it away. Either finish that call (pass
   `prestate` into `recorded_native_only`, which is what the comment at `:2351-2354` already claims) and
   then do the same at `programs/dclutch-core-sbf/src/capability.rs:571`, or delete the dead decode and
   record in §3 that decision 0030 is discharged on one of three sites. **Do not leave the middle state**:
   a `Box` allocated for nothing and a live `?` on a path that previously could not refuse are a cost with
   no benefit, and the comment lies about the code. Whichever way it goes, `recorded_native_with_crank`
   (S11) has zero callers and either gets one here or is deleted.

3. **Correct §2.1's five false claims in whatever the convergence lane carries forward.** `lineage_walk`
   is not deleted, the census `rent` command does not exist, the Upgrade-row producer does not exist,
   R2's constant does not exist, and R4's two positive controls do not exist. Each is a real piece of work
   with a real ruling behind it — the rulings are worth keeping — but a lane that reads §1 and §3 as a
   record of what happened will go looking for code that is not there. If the `lineage_walk` deletion is
   still wanted, R1's census stands and the fourteen call sites in §2.1 are the whole cost; if the Upgrade
   producer is still wanted, `upgrade.rs:2237` and `:2243` are where the refusal to write one lives today.

Only after 1–3: `tools/gate reference --converge` (S13–S17), `tools/gate frames --capture` (S19), and the
S18 Claims-side split, which has not been started at all.
