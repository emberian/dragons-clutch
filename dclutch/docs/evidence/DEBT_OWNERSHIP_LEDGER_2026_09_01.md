# Debt deletion/ownership ledger — lane S11, 2026-09-01

Status: census output and ruling register. Not release evidence.
Owner: lane S11 (`docs/LETTER_TO_CLAUDE_2026_09_01.md:827-867`), against
`docs/MASTER_COMPLETION_CONTRACT.md` row **C-00** and rows C-15, C-16.

**What this row terminates in:** `docs/evidence/C16_ENTRY_LIST_2026_09_01.md` —
the list a hostile reviewer is handed, by C-16's own six categories, with every
item in exactly one of two states and an owner for each. This file is the
working material; that one is the gate. Read that first.

C-00's hardest clause is *never-executed intended route*: the codebase declares
a capability, allocates it an identity, and nothing ever reaches it. Contract
vocabulary admits exactly two terminal states — implemented with evidence, or
ruled out by Ember with a date. This file does not create a third. Every row
below is triaged **(a)** reachable and now tested, **(b)** genuinely
unreachable with the reason convicted, or **(c)** needs an Ember ruling.

Reproduction for every count here:

```sh
cd tools/gauntlet/census && cargo build --release
./target/release/dclutch-route-census inventory --root ../../.. \
    --out /tmp/inventory.json --check-unique
```

---

## 1. Declared-but-unraisable refusal codes

**Denominator: 299** protocol-visible codes across 13 programs — every
`#[repr(u32)]` discriminant the census enumerates, which per `AGENTS.md` is the
declaration "these codes are protocol-visible". Cross-referenced against
**8,351** production raise sites (program/crate `src/`, excluding tests,
tooling, generated mirrors, doc comments, `ALL`-array and `ordinal()`
bookkeeping, and `as u32` code reads; `Self::V` counted inside `impl … for E`
blocks wherever they live).

*Measured twice. The first pass read 297 codes / 8,339 sites / 12 dead; the
re-run after `7883aef8` and `e447d28b` reads 299 / 8,351 / 11. The denominator
moves under a live swarm, so it is stated with the revision it was taken at
rather than as a constant.*

**Hit list: 11 codes with zero raise sites. All eleven are in Claims.**

| code | name | triage |
| --- | --- | --- |
| `0x5006` | `ClaimsSbfError::CustodyRequired` | **(c)** |
| `0x5100` | `LiabilityBasisSbfErrorV2::Instruction` | **(b)/(c)** |
| `0x5101` | `LiabilityBasisSbfErrorV2::Accounts` | **(b)/(c)** |
| `0x5103` | `LiabilityBasisSbfErrorV2::FinalizedRecord` | **(b)/(c)** |
| `0x5104` | `LiabilityBasisSbfErrorV2::ProductLink` | **(b)/(c)** |
| `0x5105` | `LiabilityBasisSbfErrorV2::Release` | **(b)/(c)** |
| `0x5106` | `LiabilityBasisSbfErrorV2::Candidate` | **(b)/(c)** |
| `0x5107` | `LiabilityBasisSbfErrorV2::CustodyRequest` | **(b)/(c)** |
| `0x5108` | `LiabilityBasisSbfErrorV2::CustodyCpi` | **(b)/(c)** |
| `0x5109` | `LiabilityBasisSbfErrorV2::Postcondition` | **(b)/(c)** |
| `0x510A` | `LiabilityBasisSbfErrorV2::Commit` | **(b)/(c)** |
| ~~`0x5644`~~ | ~~`FractionalClaimCheckCompactionSbfErrorV1::Phase`~~ | **CLOSED** — see §1.3 |

**No code in this list is (a).** None is a live guard that was merely untested;
each is dead because the thing that would raise it does not exist. Writing a
test for any of them would be writing a test that cannot go red, which is the
defect `AGENTS.md` names, not a fix for it. Saying so is the honest result.

### 1.1 `ClaimsSbfError::CustodyRequired = 0x5006` — CONFIRMED dead

The seed is confirmed. The only occurrences of the discriminant are its
declaration (`programs/dclutch-claims-sbf/src/lib.rs:182`), the `ALL` array
(`:272`), the `ordinal()` match (`:296`), six generated mirrors, and two doc
comments. Nothing raises it.

It is dead for a *named* reason, written down by the crate that would need it:

> "Neither the legacy generic route nor anything downstream of it builds a
> `CustodyRequestV1` … The program even declares a refusal for this —
> `ClaimsSbfError::CustodyRequired = 0x5006` … and raises it nowhere."
> — `crates/dclutch-claims-conservation-contract/src/lib.rs:29-34`

So this is not a removed guard. It is the marker of an unbuilt route, and
deleting it would erase the only in-program evidence that the route is owed.
It is the same item as §2.1 below.

### 1.2 `LiabilityBasisSbfErrorV2` — ten codes orphaned by a route banishment

`programs/dclutch-claims-sbf/src/liability_basis_v2.rs:1-8` states plainly:
"This module is now exactly what outlived the `DCLLBX02` route … **It owns no
route and dispatches nothing.**" The banishment was correct and its rationale is
excellent (`:10-27`). It was not finished at the refusal boundary.

Eleven discriminants survive. Exactly one — `ClaimsState = 0x5102` — is still
raised, by the eight surviving read/encode helpers
(`liability_basis_v2.rs:173,177,187,191,200,208,216,218`). The other ten can be
raised by nothing, and because the enum keeps `#[repr(u32)]`, the census still
enumerates them and every generated surface still publishes them
(`docs/reference/refusals.md:75+`, `packages/dclutch-sdk/lib/generated/refusalRegistryV1.ts`,
`apps/dclutch-web/lib/generated/refusalRegistryV1.ts`, six mirrors each).

`AGENTS.md` names this exact failure mode for non-Rust consumers; the missed
sweep here is one file further back, in the Rust enum itself.

**Why this cannot be a lane's unilateral cut.** The enum's own compile-time
assertions require `ALL` to be the contiguous run from the registered sub-band
offset. Dropping `Instruction = 0x5100` and `Accounts = 0x5101` while keeping
`ClaimsState = 0x5102` breaks the base assertion; keeping them and dropping the
tail leaves a hole. Shrinking this enum is a sub-band renumbering, and sub-bands
are documented in decision 0007. → **(c)**, §10 row R-2.

### 1.3 `FractionalClaimCheckCompactionSbfErrorV1::Phase = 0x5644` — CLOSED

**Status: DECLARED, CONVICTED, BUILT, OBSERVED — closed across three lanes in
one session.** It was declared and unraisable when this ledger opened; convicted
here as a guard declared and never written; built by the Structured lane at
`fractional_claim_check_v1.rs:1196-1209`; and then *observed firing on a real
ELF* by this lane's own fold (`939d0806`), through a hostile that rewrites the
Core Market to `Open` with its terminal receipt dropped, at its own derived
address, so the phase is the sole discriminator. `Custom(0x5644)`, in the
runtime's own log.

That is the whole method in one refusal code, and it is worth naming as a
sequence rather than as an outcome: a census found an absence, a second method
convicted it as a defect rather than an artefact, an owner built the guard, and
a third instrument watched it fire. **No single step of that is evidence; the
sequence is.**

**Status: fixed by S7 while this lane was running.** `fractional_claim_check_v1.rs`
now decodes `CoreState` (`:1196`) and raises `Phase` on both halves —
`CorePhaseGateV3::TerminalOrRetiring.admits(core.phase)` (`:1202-1203`) and
`core.terminal_receipt.is_none()` (`:1208-1209`) — behind a shared
`CorePhaseGateV3` the native and fractional routes now both read. The code left
the dead list between this file's two measurements. The finding as originally
convicted is kept below because the *class* is what matters, and because a
reader should be able to see what the register looked like before the repair.

---

This was the one finding in §1 that was a live defect rather than an artifact of
deletion, and it was the highest-value row in this file.

The native claim-check compaction route carries the guard:

```rust
// programs/dclutch-claims-sbf/src/claim_check_compaction_v1.rs:441-448
if !matches!(core.phase, Phase::Terminal | Phase::Retiring) {
    return Err(ClaimCheckCompactionSbfErrorV1::Phase.into());
}
// Checked even though the phase invariant implies it. A checked invariant
// is one an implementer cannot silently delete.
if core.terminal_receipt.is_none() {
    return Err(ClaimCheckCompactionSbfErrorV1::Phase.into());
}
```

Its fractional sibling declares the identical refusal —
`FractionalClaimCheckCompactionSbfErrorV1::Phase = 0x5644`, "The Core phase, or
the absence of a terminal receipt, refused"
(`programs/dclutch-claims-sbf/src/fractional_claim_check_v1.rs:110-111`) — and
**never reads `core.phase` or `core.terminal_receipt` at all.** There is no
`CoreState` decode anywhere in that module. The comment the native author wrote
to stop exactly this ("a checked invariant is one an implementer cannot silently
delete") describes what then happened to the fractional twin.

The route is not currently exploitable-by-inspection: the escrow it requires can
only be opened by `claim_check_compaction_v1::process_open_escrow`, which does
check the phase, and Core phase does not run backwards. That is an *implied*
invariant across two instructions separated by a 180-day deadline — which is
precisely the reasoning the native route refused to rely on.

The fix was a guard in `fractional_claim_check_v1.rs`, not a test, and the file
was another lane's working surface. Routed to S7; landed there. → §10 row R-1,
now closed.

### 1.4 Related: `ClaimsSbfError::BasisEvaluatorAbsent = 0x500C`

Not in the dead list — it is raised at
`programs/dclutch-claims-sbf/src/rational_terminal_v3.rs:239`. Recorded here
because its own doc declares it *unreachable* at runtime while
`ProductBasisV3::decode` refuses kind byte 3 (`lib.rs:213-219`), and it argues
why allocating it anyway is right. This is the model for how a knowingly
unreachable code should be documented, and none of the twelve above does it.

---

## 2. Declared-but-undispatched routes

**Denominator: 159 routes** (`docs/reference/routes.md`, generated). Status
split at HEAD: **66 witnessed, 27 blocked with a written reason
(`tools/gauntlet/blocked.json`), 65 `NEVER-EXECUTED, no stated reason`.** After
this lane's repair (§2.2, `96ddf38f`) the same inventory scores **69 / 35 / 55**;
the in-tree register still shows the old numbers because `docs/reference/` is
stale for an unrelated reason (§9).

Separately, **240 named 8-byte magic constants**, of which **50 are
request/instruction-shaped** by name; **32 of those 50 are not a dispatch
selector in the census inventory.** Most of the 32 are reached through a codec
constructor whose selector the census records under a different constant, so the
raw number is not a defect count. One is a true orphan:

### 2.1 `claims.conserve` / `DCLCNS01` — CONFIRMED never dispatched

`CLAIMS_CONSERVATION_REQUEST_MAGIC_V1 = *b"DCLCNS01"`
(`crates/dclutch-claims-conservation-contract/src/lib.rs:166`). It appears in
exactly one place in the tree — its own declaration. No program dispatches it,
no operator builds it, no client sends it.

The crate is a full workspace member (`Cargo.toml:10`), `no_std`, total, with
its own tests, and it is honest about its own state:

> "**The Claims-owned outer route that would call this does not exist.**
> Nothing on chain dispatches `CLAIMS_CONSERVATION_REQUEST_MAGIC_V1`; no
> operator builds it; no client can send it. Split and merge remain
> UNIMPLEMENTED as user acts."
> — `crates/dclutch-claims-conservation-contract/src/lib.rs:49-54`

The SDK publishes the capability with a matching wall rather than a claim
(`packages/dclutch-sdk/lib/capabilityModel.ts:219-222`), so there is **no
contradictory guide here** — the client surface is already truthful.

This crate also records two live semantic defects in the legacy path it
replaces: `execute_basket` mints a complete set against a Hoard that received no
collateral, and merge returns a payout in complete *sets* where a Custody
transfer moves *atoms* — invisible at `basis_scale == 1`, which is what every
in-tree fixture uses (`lib.rs:19-29`).

**Verdict: confirmed, both halves.** Split/merge as user acts is a designed,
semantically-owned, tested capability with no route, no dispatcher, and no
client. → **(c)**, §10 row R-3.

### 2.2 The route register over-reported NEVER-EXECUTED — REPAIRED, `96ddf38f`

**Corrected denominator: 55, not 65.** Ten of the sixty-five
`NEVER-EXECUTED, no stated reason` rows were false. Measured at the HEAD
inventory (159 routes, 299 refusal codes):

| | committed register | after `96ddf38f` |
| --- | --- | --- |
| witnessed | 66 | **69** |
| blocked, reason stated and owned | 27 | **35** |
| **NEVER-EXECUTED, no stated reason** | **65** | **55** |
| binding refs naming a route the code does not have | 12 | **0** |

The exact delta is ten rows, no others moved, and nothing regressed into
never-executed. Two distinct defects produced them.

**Defect A — renamed routes, so real folded evidence credited nothing (2 rows,
now witnessed).** A binding names a route id; when a route is renamed, the
binding keeps pointing at the old name and the campaign's evidence lands on
nothing, while the new name reads NEVER-EXECUTED.

- `core/persist_state#VerifyFundReady` → `core/resolution::process#VerifyFundReady`
  (`tools/gauntlet/journey/bindings.json`). Decisive because of its neighbours:
  the sibling `CreateFund` bindings in the *same campaign*, with the *same frame*
  (`core/process_instruction`, `resolution/process_verify#VerifyFundReady`,
  `resolution/core_effect::process_core_effect`), already name the current
  `core/resolution::process#CreateFund`, and the two actions sit one line apart
  in the enumerator (`resolution.rs:264` and `:265`). One ref kept a stale
  handler name; the journey campaign executed the route on a real validator.
- `trading/generic_market_founding_v1::process_generic_market_founding_v2` →
  `_v3`, two entries in `tools/gauntlet/tier1/bindings.json`, including "found
  the Market atomically: Lock, Found, Realize, Claims, Open (DCLTGMF3)". Only
  `_v3` exists (`generic_market_founding_v1.rs:187`, dispatched at
  `lib.rs:632`); `fdfbe0dd` did the rename and left the bindings behind. **Tier 1
  founds a Market every run and the register said that route had never
  executed.**

Also removed: nine refs to `claims/process_admit#Admit` and
`claims/process_close#Close`, which the enumerator no longer emits at all — it
now records only the parent `claims/protocol_position_v2::process`. Every entry
touched still carries that parent, asserted before each drop, so no coverage was
lost. (Worth knowing separately: the census can no longer distinguish Admit from
Close for that route. That is an enumerator granularity change, not a campaign
change.)

**Defect B — campaigns that pass and emit no evidence (8 rows, now blocked with
a stated reason and a named owner).** Three real-ELF campaigns exist, pass, and
are invisible: `fractional-atomic` (4 targets, 49 async cases, exact
discriminants at `fractional_compaction.rs:2544-2568`, committed `8fdcdc56`),
`user-position-admission` (builds a real `ProtocolPositionActionV2::Admit` at
`lifecycle.rs:376-386`), and `general-hot`. None takes the
`dclutch-program-test-evidence` dependency, so none calls `record()`, so none
emits a document `census observe` could corroborate — and a `bindings.json`
written without that would be an assertion, not evidence.

**So these were NOT bound.** They were recorded in `tools/gauntlet/blocked.json`
with the precedent that file already set for exactly this situation — the note
that unblocked `claims-affine-batch` read "a real-ELF ProgramTest for this route
already exists and passes; it simply does not emit census evidence yet". Each
entry names what exists, what is missing, and both owners: the lane that owns the
test file (S7 for Claims, the Trading lane for `user-position-admission`) for the
`record()` calls, and this lane for the runner and bindings once they emit. That
is the honest state — *driven, not yet corroborated* — and it is now stated
rather than silent.

### 2.3 What the register actually derives its status from

Found while repairing the above, and it changes how the number should be read.

**`docs/reference/routes.md` never consults the evidence ledger.**
`tools/genref/generate.mjs` builds its `routeEvidence` map purely by scanning
`tools/gauntlet/*/bindings.json` and `*-bindings.json` (`:122-176`), and
`routeStatus()` (`:207-222`) returns *witnessed* for any route a binding
mentions. `grep -n "ledger\|CENSUS\|observe" tools/genref/generate.mjs` returns
nothing. The strict corroboration — chain logs, signatures, refusal codes,
`Program <address> invoke` lines — lives in `census observe`, which produces
`CENSUS.md` on a *different* path.

Two consequences, both material to C-00 and C-16:

1. **A bindings file alone flips a row to witnessed.** Nothing in the
   `routes.md` pipeline can distinguish "bound and corroborated by folded chain
   evidence" from "bound by assertion". This is exactly why defect B above was
   not fixed by writing bindings.
2. **The register's 12 stale refs were visible but inert.** genref does render
   them, in a "Campaign records naming routes the code does not" table
   (`:498-509`, and `docs/reference/routes.md:251`). So the information was
   published for anyone who read that far — and for however long it stood, tier
   1's Market founding sat in that table while the route it drives sat in the
   NEVER-EXECUTED list on the same page. **A defect that is reported in one
   table and contradicted in another is not a reported defect.** Cross-checking
   those two tables against each other is one predicate, and nothing does it.

**C-00 and C-16 should close against 55, not 65 — and only once the two
pipelines are joined.** The register a stranger reads is currently an assertion
surface with an evidence surface beside it that it never consults.
→ §10 row R-4, partly discharged.

### 2.4 The gate checked refusal bands and never once checked a magic

`AGENTS.md` names `dclutch-route-census inventory --check-unique` as *the* gate.
Until `7bf75057` it checked refusal-code bands only. A refusal code and an
instruction magic are the same kind of object — a wire discriminant a program
dispatches on — and only one of them had a uniqueness rule.

The gate now sweeps every `const NAME: [u8; 8] = *b"…"` under `crates/` and
`programs/` and fails on **one magic value claimed by two or more distinct
constant names**. It goes red at HEAD on **ten collisions**, and the redness is
the finding:

| magic | claimed by | why it matters |
| --- | --- | --- |
| `DCLTDRS1` | `DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1` (`programs/dclutch-trading-sbf/src/dealer_scenario_checkpoint_v1.rs:88`), `DIRECT_REPLAY_SETUP_REQUEST_MAGIC_V1` (`crates/dclutch-direct-codec/src/replay_setup_v1.rs:13`) | **the sharp one — see below** |
| `DCLTRIX1` | `RECORD_INSTRUCTION_MAGIC_V1` (`crates/dclutch-record-contract/src/lib.rs:57`), `REGISTRY_INSTRUCTION_MAGIC_V1` (`crates/dclutch-registry-svm/src/lib.rs:39`), `RELAY_INSTRUCTION_MAGIC` (`crates/dclutch-relay-contract/src/instruction.rs:20`) | three programs' instruction magics are one value |
| `DCLTPRQ2` | `ADMISSION_REQUEST_MAGIC_V2`, `PAYOFF_REQUEST_MAGIC_V2` | two request families |
| `DCLTSA03` | `SHADOW_ACK_MAGIC_V3`, `TERMINAL_SETTLEMENT_RECEIPT_MAGIC_V3` | an ack and a receipt |
| `DCLTARF1` | `AGGREGATE_RETIREMENT_FINISH_MAGIC_V1`, `ARTIFACT_RELEASE_MAGIC_V1` | unrelated records |
| `DCLTDAC1` | `DEALER_SCENARIO_ACTIVATION_RECEIPT_MAGIC_V1`, `DIRECT_ACTIVATION_REQUEST_MAGIC_V1` | a receipt and a request |
| `DCLTDRR1` | `DEALER_SCENARIO_RESERVATION_RECEIPT_MAGIC_V1`, `DIRECT_BEGIN_RETIRING_RECEIPT_MAGIC_V1` | two receipts |
| `DCLTDCM1` | `DEALER_SCENARIO_CHECKPOINT_COMMIT_MAGIC_V1`, `DEALER_SCENARIO_CUSTODY_EFFECT_MANIFEST_MAGIC_V1` | within one family |
| `DCLLBM02`, `DCLLBP02` | the canonical `LIABILITY_BASIS_*_MAGIC_V2` plus fixture-local `CLAIMS_*_MAGIC_V2` / `*_MAGIC_V2` | test fixtures re-declaring a record magic under a local name |

**`DCLTDRS1` is a dispatch-safety question, not a naming one.** Both constants
are live top-level selectors of the *same* Trading ELF. Verified: the dealer arm
is `data == DEALER_SCENARIO_CHECKPOINT_RESERVE_MAGIC_V1` — exactly 8 bytes
(`dealer_scenario_checkpoint_v1.rs:216-218`); the Direct arm is
`input.len() == DIRECT_REPLAY_SETUP_REQUEST_BYTES_V1 && input[..8] == MAGIC`,
and that width is 120 (`replay_setup_v1.rs:17,269-272`). The dealer branch is
tried first (`src/lib.rs:546` before `:601`). So **a mis-sized Direct
replay-setup request does not refuse — it routes into the Dealer family**, and
any future widening of the bare dealer instruction collides silently. Not
exploitable today; a wrong-handler bug the moment either side's shape moves.
`DCLTPCA1` is the same bare-8-byte shape (`:1347-1348`), which is the shape
where this goes wrong most easily.

### 2.4.1 The adjudication: two are hazards, eight are naming smells

Ember's standing devnet-deploy authority (`AGENTS.md`, `6a68c9c1`) requires a
**full** redeploy with fresh identities, so a wire change costs nothing right
now — no durable state to migrate, and every mirror is generated. Re-lettering
was authorized on that basis and done ahead of the cohort.

The hazard is **same-ELF collision**, where one program dispatches on both
constants and only data length and branch order separate them. A magic shared
across *different* programs' ELFs cannot be mis-dispatched: the two constants
never meet a single dispatcher, so it is a naming smell, not a safety defect.
Adjudicated by tracing each constant to the `is_*` predicate that reads it and
then to the program `src/` that calls that predicate:

**Same-ELF dispatch hazards — 2 of 10.**

1. **`DCLTDRS1`, Trading — FIXED (`b64ecbb5`).** The dealer reserve arm is now
   `DCLTDRV1`; `DCLTDRS1` belongs solely to Direct's replay setup. Re-lettered
   rather than separated by length, because a length test is itself a guard
   whose two sides move together. Landed with a regression test,
   `the_reserve_arm_does_not_answer_to_directs_replay_setup_magic`, which
   asserts the two constants differ *and* that a bare Direct prefix selects
   nothing here. The suite proved the channel live before the fix: the existing
   `selectors_are_exact_…` test failed on `b"DCLTDRS1"` exactly as it should.
2. **`DCLTRIX1`, Registry — NOT re-lettered, and it is the sharper of the two.**
   See §2.4.2. It is a *deliberate, documented* action-space partition, so
   changing it is the owners' design decision, not a mechanical re-letter.

**Cross-ELF or non-dispatch — 8 of 10, gate stays red.** `DCLLBM02`, `DCLLBP02`
(test fixtures re-declaring a record magic under a local name — fix is to import
the canonical constant), `DCLTARF1`, `DCLTDAC1`, `DCLTDCM1`, `DCLTDRR1`,
`DCLTPRQ2`, `DCLTSA03`. None has two constants meeting one dispatcher; several
pair a request with a receipt, which travel on different channels entirely
(instruction data versus account data). Left red deliberately: they are real
ambiguity for anyone reading a wire dump, and narrowing the gate to the subset
already fixed would be the exact laundering this lane exists to prevent.

### 2.4.2 `DCLTRIX1` — a documented partition whose own rule is already broken

`crates/dclutch-registry-svm/src/lib.rs:103-106` states the design outright:

> "Action `0` is reused, because this magic is shared with the record family
> (`dclutch_record_contract::RECORD_INSTRUCTION_MAGIC_V1`), which owns every
> action from `2` upward; the Registry side of that split has exactly actions
> `0` and `1` to spend."

So one 8-byte discriminant, two request families, partitioned by an action byte
at offset 10 — an offset both families deliberately share. **Record's own
`RecordActionV1::Begin = 1` sits inside the half the document assigns to
Registry** (`crates/dclutch-record-contract/src/lib.rs:1734-1739`), where
Registry already spends `0 = ActivateRole` and `1 = Reauthenticate`.

What keeps that from mis-dispatching today is the second clause of the guard at
`programs/dclutch-registry-sbf/src/lib.rs:282-290`:

```rust
if instruction_data.get(..8) == Some(RECORD_INSTRUCTION_MAGIC_V1.as_slice())
    && (instruction_data.get(10).is_some_and(|action| *action >= 2)
        || instruction_data.len() != REGISTRY_INSTRUCTION_BYTES_V1)
```

and the arithmetic accident that `BEGIN_RECORD_BYTES_V1 == 176` while
`REGISTRY_INSTRUCTION_BYTES_V1 == 16`. Record's other actions are safe by the
first clause: `AppendPage = 2`, `Finalize = 3`, `Abort = 4`, and its two
16-byte unit requests are Finalize and Abort, both `>= 2`.

**So a Record `Begin` is separated from a Registry `Reauthenticate` — same
magic, same action byte `1` — by nothing but 176 ≠ 16, and no assertion anywhere
ties those two widths together.** Narrow the Begin request to 16 bytes, or give
Registry an action `2`, and valid traffic silently executes as the other
family's instruction. This is the project's own named defect class, "guards
whose two sides move together", one wire object over from where it was first
found.

The minimum honest repair is not a re-letter: it is a compile-time assertion
binding the two sides (`BEGIN_RECORD_BYTES_V1 != REGISTRY_INSTRUCTION_BYTES_V1`,
and that no `RecordActionV1` below `2` exists other than `Begin`). Moving
`Begin` to `5` removes the exception entirely and makes the documented rule
true. Either is a Registry+Record decision. → §10 row R-13.

**Not fixed by re-lettering, for the remaining eight.** Some are shipped
instruction discriminants, where a change is a wire event needing its own
decision record exactly as a refusal-band renumbering does; others are fixtures
that should import the canonical constant. → §10 row R-11.

**Same-name mirrors are counted, not failed** — three of them (`DCLTCF1A`,
`DCLTCF2A`, `DCLTPCA1`, each re-declared under its own name in a second
package). That split is stated in `magics.rs`'s module doc and pinned by a test
whose name says so, so nobody has to wonder whether the gate was narrowed to
whatever passed: a mirror is one fact with two authors (a convergence problem);
a collision is two facts with one wire encoding (a dispatch-safety problem).
Only the second routes a caller into the wrong handler.

**This red blocks two shared commands** — `tools/genref/generate.sh` and
`tools/gauntlet/run.sh` both pass `--check-unique`. That is the intended cost of
the finding, and it is stated here so it is not discovered as a mystery.

### 2.5 Client reachability, corrected — the register counted its own listing

The mirror image of §2.2. There the instrument under-reported executed routes;
here it over-reports reachable ones. Same root cause: **the register measures
mentions, not executions.**

Grepping a magic in `apps/dclutch-web` finds hits in
`apps/dclutch-web/lib/generated/routeCensus.ts` — *the published census listing
itself*, which mentions every route by construction and builds none of them.
Counting those as browser reachability inflates C-12.

Measured over the 270 distinct protocol magics, with the definition stated
because the number depends on it:

| what counts as "not a builder" | magics | routes over-counted |
| --- | --- | --- |
| only `routeCensus.ts` | 14 | **14** |
| the three pure generated tables | 14 | **14** |
| every `lib/generated/*` module | 18 | 15 |
| generated modules **and** `.test.` files | **26** | **20** |

So the S9 lane's 26 and this lane's 14 are the same measurement under different
definitions of *builder*, and both are right. **For C-12 the honest figure is
20**: a magic whose only browser mention is the census listing or a test has no
user-facing builder, and a test is evidence that the code can construct the
transaction, not that a person can. The 14 routes whose *only* browser mention
is the listing include `claims/founding_v5::process`,
`claims/protocol_position_v2::process`, `custody/projected::process` and
`core/found::project`.

### 2.6 User-inaccessible capabilities — C-16 forbids these

Of ≈57 top-level selectors, **19 are reachable from neither the CLI nor the
SDK** (S9's hand-resolved count; this lane verified the two sharpest, below,
and the tree's shape is consistent with the rest). C-16 requires that no
"user-inaccessible capability" remain, so each is a C-16 blocker, not a nicety.

- **`DCLTDFS1` — `DIRECT_FEE_SETTLEMENT_REQUEST_MAGIC_V1`.** C-04's
  permissionless third-party fee completion: a route **designed for a stranger
  to call**. Every mention in the tree is its own codec
  (`crates/dclutch-direct-codec/src/fee_settlement_v1.rs`), a campaign harness
  (`programs/dclutch-trading-sbf/program-test/tests/direct_hot_fee_pair.rs`,
  `…fee_second_transaction.rs`, `run-fee-pair.sh`), the local-validator
  bootstrap successor, and docs. **Nothing in `packages/dclutch-sdk`,
  `tools/dclutch-cli` or `apps/dclutch-web`.** A permissionless act only our own
  test harness can perform is not permissionless.
- **`DCLTPCA1` — `PROJECTED_CUSTODY_ABORT_MAGIC_V1`**, "Sole top-level
  projected-Custody founding-abort instruction"
  (`programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs:199`),
  dispatched at `src/lib.rs:696-697`. It is the way back out of collateral
  stranded by an expired projection. Client surfaces: none — the only exterior
  is `tools/local-validator/bootstrap/successor/src/source_abort_exterior.rs`,
  an operator bootstrap, not a shipped client. **A recovery act with no client
  is a fund-recovery path a user cannot walk.**

→ §10 row R-12.

---

## 3. Orphan `unfinished` / material TODO

The tree's idiom is not `TODO`. It is the five classification words of
`docs/OMISSION_INDEX.md` — `hard invariant`, `current safe profile`,
`likely scar`, `open research`, `unfinished` — in three tables (`O-001..O-018`,
`P-001..P-008`, `U-001..U-015`), plus one generated marker,
`NEVER-EXECUTED, no stated reason`, emitted by `tools/genref/generate.mjs:219`.

**Denominator: 825 distinct matching lines** across 20 marker patterns
(excluding `target/`, `.claude/worktrees/`, `node_modules/`, `Cargo.lock`,
`docs/board-archive-2026-08-27.md`, `docs/compost/`, `docs/recovered/`).

Classification: **~700 cosmetic, ~120 tracked, 11 orphan-material.**
`ASPIRATION_LEDGER.md:1283-1285`'s claim that the repo carries zero real
`TODO`/`FIXME`/`HACK`/`unimplemented!`/`todo!()` **verifies true at HEAD** — all
10 such hits are meta-references to the counting itself. All 52 `unreachable!()`
are `#[cfg(test)]` bounded-match arms; none is on an on-chain path.

**Orphan material — material, and no row owns it:**

| finding | `file:line` |
| --- | --- |
| the whole ClaimCheck escrow/compaction/redemption capability is on chain and never run; owned only by a design doc | `docs/reference/routes.md:29-32,36,37` |
| `trading/projected_custody_bootstrap_v1::process_controller_funding_{prepare,cleanup_step1,cleanup_step2}_v1` never driven | `docs/reference/routes.md:237,239,240` |
| `trading/outer::process_capability_lifecycle#else` — the outer's fallthrough arm has no campaign and no reason | `docs/reference/routes.md:226` |
| `registry/lineage_v1::process`, `registry/process_abort#4` — lineage publication and record abort unexecuted | `docs/reference/routes.md:158,159` |
| `core/authenticate_no_recovery_entries#None` never executed | `docs/reference/routes.md:61` |
| `/local` console renders a cluster `<select>` whose selection it then ignores; known-wrong shipped behavior dispositioned "out of scope" with no ruling and no date | `docs/design/OPERATOR_FORMS_V1.md:469` |
| SDK/web carry tracked mirrors of one surface reconciled only by a drift report — two authors for one fact, adjacent to O-005 | `packages/dclutch-sdk/README.md:7` |
| `tools/devnet-activity` `resume` is poll-only and never invokes a child: an interrupted devnet lifecycle can be observed but not resumed | `tools/devnet-activity/README.md:31` |
| Dealer accept-split has topology/frame evidence and no executor ("the executor is not implemented yet") | `docs/evidence/DEALER_ACCEPT_SPLIT_TOPOLOGY_2026_08_28.md:82` |
| founding wizard ships preview placeholder service identities; a real founding must name checked releases | `apps/dclutch-web/components/CreateMarketWizard.tsx:216,436` |

### 3.1 `OMISSION_INDEX.md` staleness

Twelve rows checked against current code. **Four are stale-open** — the code
moved and the status column did not:

- **U-013** — status column still reads "the physical Market/Claims layout slice
  is unfinished" while its own 2026-08-30 amendment *in the same cell* records
  the layout half SATISFIED and the enum half LANDED. `SplineDegree2To3` ships
  (`programs/dclutch-claims-sbf/src/terminal_certificate_v3.rs`). The row
  contradicts itself in its status field.
- **U-003** — status column claims "two named defects" while the body records
  defect (a) CLOSED 2026-08-27 and the publication closure LANDED.
- **U-006** — `docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md:2080`
  asserts "`AbortSourceAndClose` is still not implemented" while
  `docs/reference/routes.md` shows `custody/abort_source_and_close#AbortSourceAndClose`
  executed and refused in tier1.
- **U-014** — "unfinished expansion" understates: see §5.

**Genuinely still open, verified:** U-002, U-004, U-005, U-007, U-008, U-009
(no non-Pyth adapter crate exists at all), P-007, P-008.
**Verified-accurate closures:** P-005, P-006.

---

## 4. Contradictory current guides

**Denominator: 82 in-scope guide files** — 6 top-level, `docs/START_HERE_2026_09_01.md`,
11 `docs/guides/`, 3 `docs/operators/`, 6 `docs/reference/*.md`, 55 READMEs under
`tools|apps|packages|programs`. 22 audited claim-by-claim, all 82 swept for dead
paths and unparsed flags; **~240 checkable claims** extracted (~60 paths, ~90
subcommands/flags, ~25 magics/widths/enums, ~25 addresses, ~20 package names,
~20 script arg-parsers).

**The `dclutch` binary trap was checked and produced no finding.** Both binaries
were resolved — Rust `tools/dclutch-cli/src/main.rs:127-131` (`market`,
`capability`, `fractional-retirement-next`, `general`, `ticket`) and TypeScript
`packages/dclutch-cli/src/main.ts:196-223` plus its `FLAG_OPTIONS` at `:78-168`.
**Every `dclutch` command in every in-scope guide resolves against one of the
two.** No missing-command finding exists.

**Eight verified contradictions.** Each carries the guide line and the code fact
that refutes it; every one below was independently re-checked against source.

| # | guide | false claim | refuted by |
| --- | --- | --- | --- |
| 1 | `README.md:166`, `docs/guides/reader.md:67` | `tools/release/private-validator-lifecycle/run.py --through participant` presented as a runnable one-liner | `run.py:7190-7197` marks `--repo`, `--release-root`, `--validator`, `--solana`, `--work` all `required=True`; `:7214-7216` raises `Refusal("the founding/participant development probe requires exactly one named seed")` because `--seeds` defaults to 20. It aborts in argparse. The correct 9-flag form is at `docs/operators/found-a-market.md:117-126` |
| 2 | `docs/guides/client-developers.md:59` | "current 360-byte `DCLTCOR3` Markets" | `packages/dclutch-sdk/lib/generated/coreFound.ts:6` `CORE_STATE_BYTES = 368`; `marketDiscovery.ts:95` lists `generation(3, 360)` inside `HISTORICAL_CORE_GENERATIONS_V1`. A client filtering on 360 finds nothing. Same stale 360 in `marketDiscovery.ts:44`'s own doc comment |
| 3 | `ARCHITECTURE.md:5-6` | the 2026-08-27 supersession banner — explicitly "the only edit" made to un-stale the file — says "`DCLTCOR2` the one live Market representation" | `coreFound.ts:43` `CORE_STATE_MAGIC` is `DCLTCOR3`, `:4` `CORE_VERSION = 3`; `marketDiscovery.ts:95` lists `generation(2, 352)` as an incompatible historical generation. **The banner that fixes staleness is itself stale** |
| 4 | `tools/gauntlet/tier2/README.md:76,83` | "the shared `tools/gauntlet/programtest/evidence.rs` module, which every campaign includes by `#[path]`" — and therefore "no manifest or lockfile change" | no `tools/gauntlet/programtest/` in tree; no `evidence.rs` anywhere under `tools/gauntlet`. The real surface is the Cargo crate `tools/gauntlet/program-test-evidence/`, taken as a **path dependency** (`programs/dclutch-dealer-sbf/program-test/Cargo.toml:11`). The stated mechanism is exactly backwards, and against `AGENTS.md`'s "a commit carries the manifest and lock its code needs" |
| 5 | `tools/gauntlet/tier2/README.md:79` | "run the lane with `tools/gauntlet/run.sh --mode fast`" | `run.sh:84` — `case "$MODE" in census\|full) ;; *) … exit 2` |
| 6 | `tools/gauntlet/tier2/README.md:87` | "Add the manifest and test target to `CAMPAIGNS` in `campaign.sh`" | `tools/gauntlet/tier2/` contains only `README.md`; no `campaign.sh` exists under `tools/gauntlet/` (tier4 has `run-campaign.sh`); the token `CAMPAIGNS` appears nowhere in the tree but this line |
| 7 | `programs/dclutch-claims-sbf/README.md:53` | names `programs/dclutch-trading-sbf/src/dealer/physical.rs` as one of "the exact remaining producers" whose removal "permits deletion of the generic branch and its EconomicSlice dependency" | that file does not exist — `src/dealer/` holds `mod.rs`, `tests.rs` and `v3_*`/`v4_*` only. **A deletion-readiness claim wrong by one producer**, which is the kind that gets acted on |
| 8 | `tools/dclutch-cli/README.md:13` | installer URL pins `v0.1.0-devnet.2` | `tools/dclutch-cli/Cargo.toml:3` `version = "0.1.0-devnet.3"`; `docs/operators/author-a-ticket.md:22` expects the `dclutch 0.1.0-devnet.3` read-back. devnet.3 is the commit that shipped `ticket author` — so the pinned installer predates the commands the same README teaches at `:36-41` |

Findings 4-6 are one cluster: `tools/gauntlet/tier2/README.md` teaches three
things that do not exist. It is the only in-scope guide that is wrong about its
own subject throughout rather than in one line.

**Doc-vs-doc, no code constant decides either — flagged, not filed:**

- `docs/guides/operator.md:10` says "the first open market is not live yet";
  `README.md:14-17`, `docs/guides/reader.md:22-23`, `docs/guides/trencher.md:14-16`,
  `docs/guides/README.md:22` and `docs/reference/README.md:33` all say one market
  is open on devnet. `packages/dclutch-sdk/lib/deployments.ts:89-112` pins seven
  program ids and an activation cache but **no market address**, so the tree
  cannot settle it. Needs an RPC read or a ruling.
- `BROWSERTASK.md:43` tells the checker to confirm a "fee display (0.50% per
  side)" while `README.md:17` says that market is "zero-fee on both sides"; and
  `BROWSERTASK.md:40` tells the checker the landing page should explain "what
  Dragon's Clutch does" — per `README.md:188` and `AGENTS.md:4` that is the
  archived predecessor repository, not this project.

**Clean on audit, recorded so it is not re-swept:** `AGENTS.md` in full
(`crates/dclutch-refusal-registry`, `tools/lane.sh`, `tools/lane/README.md`,
`rustup run 1.97.1` against `rust-toolchain.toml`, `npm run abi:coverage`,
`run-postjoin-hostiles.sh`, `--check-unique` at `run.sh:421`);
`docs/START_HERE_2026_09_01.md` (every named file and line anchor resolves);
`docs/operators/found-a-market.md` and `author-a-ticket.md` (every flag,
subcommand, env var, magic, refusal string and route verified);
`docs/guides/operator.md`'s four founding magics and eight `CompartmentV1`
variants; `docs/guides/trader.md`'s genesis hash and nine `join` flags;
`packages/dclutch-cli/README.md`; `tools/dclutch-cli/README.md`'s 27-field
fractional route schema (exact match to `fractional.rs:188-217`).

Reference totals reconcile by hand: `docs/reference/programs.md:16-28`'s 13
entrypoint anchors all resolve, and `docs/reference/README.md:26-28` sums to
88 entry + 71 action = 159 routes and 297 refusal codes — the same denominators
as §1 and §2 at the revision those pages were generated. The code count has
since moved to 299; the reference is stale (§9), not wrong-by-authorship.

### 4.1 The generated reference over-promises the refusal surface

`docs/reference/refusals.md:9` tells a stranger "**Every error code the protocol
can return**, with its meaning", then carries all 297 — including, at the
revision it was generated, twelve that nothing could return (eleven today, §1). It is generated by `tools/genref/generate.mjs` from the
same inventory as the route register, so this is §2.2's instrument gap seen from
the other side: **the census knows what is declared and has no notion of what is
raisable.** One predicate fixes both.

---

## 5. AOT versus interpreted — the disposition (U-001, U-014)

**It is an equivalence obligation with a real and specific gap, not a deletion
candidate.** Both sides execute.

- `dclutch-direct-aot-contract` (V2) is depended on by `programs/dclutch-direct-aot-sbf`
  and three gauntlet harnesses; `direct-aot/process_instruction` is witnessed
  executed and refused in the route register.
- `dclutch-transition-vm` (interpreted) has 23 `Cargo.toml` dependents and is
  load-bearing across Series, General, Fractional and Rational lifecycles.
- `dclutch-direct-aot-v3-contract` has **two** dependents: the workspace member
  list and `tools/gauntlet/aot-cu/twin-v3`.

`docs/evidence/DIRECT_HOT_AOT_MEASUREMENT_2026-08-31.md:255-266` already closes
three of U-014's seven columns on real ELFs — refusal equivalence (32 seeds per
relation, identical acknowledgement bytes for V2, identical disposition and
output-bank digest for V3), CU comparison, and rent partially. Open: packet,
rollback-on-ELF, the equivalence **certificate**, and the Registry-bound
artifact/toolchain.

**The single load-bearing fact for the disposition** is at `:270-283` of that
same document:

> "`cargo build-sbf` therefore fails with 175 unresolved `FILL_SCALAR_*_V4` and
> `FILL_IDENTITY_*_V4` names. A host `cargo check` passes, which is why this has
> gone unnoticed: **the current Direct AOT translation has never been compiled
> for the target it exists to run on.**"

So the V3 AOT path is a second execution authority for the same semantics that
**cannot be built for Solana at all**. It is not dead code — it is worse than
dead code, because host `cargo check` is green and the gauntlet twin exercises
it, so every layer reports health for an artifact that could never ship. The
repair is known and was deliberately left uncommitted by the measuring lane
(protocol-crate change, measurement lane).

That makes U-014's disposition concrete rather than philosophical, and it is
**not** an Ember ruling: it is an engineering obligation with a named first
step. U-001's "explicit deletion/non-authoritative-AOT ruling for standalone
family artifacts" is the part that still needs Ember — §10 row R-6.

---

## 6. The refusal-granularity class

Not a bug list — a measured distribution. Across 297 codes and 8,339 production
raise sites (first pass; the re-run at HEAD reads 299 / 8,351 and moves no
share below by more than a tenth of a point):

| | |
| --- | --- |
| mean raise sites per code | 28 |
| codes with ≥ 10 raise sites | 116 |
| top 1 code's share of all raise sites | **25.0%** |
| top 5 codes | 39.2% |
| top 10 codes | 49.3% |
| top 50 codes | 80.3% |

`TradingSbfError::Content = 0x4003` carries **2,086** production raise sites
across 30 files — 780 of them in `programs/dclutch-trading-sbf/src/hot_v3.rs`
alone. Trading as a whole has 24 codes and 3,336 raise sites, 62.5% of them on
that one discriminant.

This is the mechanism behind the ledger `M-38` universal-donor problem and
behind the current Dealer wall: the letter records the honest Add refusing
Trading `Content` at 148,093 CU and a lane bisecting *by compute-unit
checkpoint* between `root-product` and `artifacts-strategy-effect`, because the
refusal code carries no information about which of 780 conjuncts fired. A test
naming `0x4003` is very nearly a test naming nothing.

Second-worst concentrations, for scale: `series-shadow` 65.7% on
`SeriesShadowSbfErrorV4::Runtime`, `dealer-accelerator` 72.7% on
`DealerAcceleratorSbfErrorV4::InvalidAcknowledgement`, `direct-aot` 50.0% on
`DirectAotSbfError::InvalidAck`. Claims, by contrast, spreads 1,489 raise sites
over 141 codes with a 15.2% maximum — Claims has already solved this, and the
sub-band convention it invented is the mechanism.

The individual fix at the Dealer site belongs to another lane. The class — a
granularity policy, and a census predicate that flags a code once its raise-site
count crosses a threshold — is §10 row R-5.

---

## 7. The `73ffb010` family sweep

`73ffb010` added `validate_plan_permissions` to the **shared**
`crates/dclutch-account-profile-contract/src/lifecycle_v3.rs:3568-3602`, called
from `StateLifecyclePolicyV3::validate_account_profile` at `:1332`. It now
requires, for every family:

- `Create` / `AuthenticateOrCreate` — the recipe account holds
  `CREDIT_LAMPORTS | WRITE_DATA`; `plan.payer` is `Some` and holds
  `DEBIT_LAMPORTS`; `plan.rent_credit` is `Some`;
- `Close` — the recipe account holds `DEBIT_LAMPORTS | WRITE_DATA`;
  `plan.rent_credit` is `Some` and holds `CREDIT_LAMPORTS`.

**The sweep, which had not been done.** Producers of `Create` /
`AuthenticateOrCreate` / `Close` plans, tree-wide:

| family | producer | permission grants | verdict |
| --- | --- | --- | --- |
| Direct (registered v4) | `crates/dclutch-direct-codec/src/registered_state_artifacts_v4.rs:369-378,481` | created `(debit,credit,write)=(F,T,T)` `:512-515`; payer `(T,F,F)` `:530-533`; rent credit `(F,T,F)` `:546-549` | **compliant** |
| Direct (ordinary v3) | `crates/dclutch-direct-codec/src/state_artifacts_v3.rs` | same triple at `ordinary_account_artifacts_v3.rs:284,299,312` | **compliant** |
| General | `crates/dclutch-general-adapter-contract/src/state_artifacts_v3.rs` | repaired by `73ffb010` itself | **compliant, by the commit** |
| Series | `programs/dclutch-trading-sbf/src/series/lifecycle_policy_v5.rs:344`, `funding_artifacts_v5.rs` | named coordinates, not pattern-matched: `root` `(T,T,T)` at `funding_artifacts_v5.rs:201` satisfies Close's `DEBIT\|WRITE`; `rent_credit` `(F,T,F)` at `:239` satisfies Close's `CREDIT` | **compliant**; cost is the `TicketAuthorship` pin's payer and RentCredit arms, now defense-in-depth only |
| Trading `hot_v3` | *not a producer* — `hot_v3.rs:7619,8363` only read `selected.operation()` | n/a | **not affected** |
| Dealer, Fractional, Bearer, Rational | declare no `Create`/`Close` lifecycle plan at all | n/a | **not affected** |

**Result: no second casualty.** The reach of `73ffb010` is exactly as wide as
`GOAL.md:1357-1359` feared, and the only family it cost anything is the one
already recorded. Every other plan-producing family granted the three
permissions before the check existed. The debt this leaves is not a broken
family; it is that `TicketAuthorship`'s payer and RentCredit arms are now
unreachable defense-in-depth — a §1-shaped item that should be documented in the
enum the way `BasisEvaluatorAbsent` documents itself (§1.4), so a future census
does not re-discover it as an orphan. → §10 row R-7.

---

## 8. C-15 — the privacy/FHE/MPC/energy horizon

> **SUPERSEDED 2026-09-01 12:47 — HISTORICAL FRAMING.** Ember ruled: the
> privacy/FHE/MPC/energy objective is **ruled OUT** of this Clutch
> (`GOAL.md:2071`, commit `5a371810`; record
> [`decisions/0018-privacy-horizon-not-this-clutch.md`](../decisions/0018-privacy-horizon-not-this-clutch.md)).
> This section describes the question as it stood before that, and is retained
> as the evidence the ruling was made on. **It is not an open charter, and
> nothing here may be reported as deferred or future work** (`GOAL.md:2093`).
> One thing it must NOT be read as licensing: closing `O-019`. The ruling makes
> that row load-bearing — see `docs/OMISSION_INDEX.md:59`.

Framed for a one-line ruling, as the letter requires. **I do not rule on this.**

**What exists in code: nothing.** `ASPIRATION_LEDGER.md:162` records the
verification — `dark`, `FHE`, `shielded`, `Shielded`, `DrEX`, `zkML` have zero
hits outside prose — and it still verifies. There is no crate, module, kernel or
type serving this ambition.

**What ruling exists, and why it is not the ruling C-15 wants.** There is a
dated Ember ruling, `ASPIRATION_LEDGER.md:3-7` and `docs/INTENT.md:109`:

> "dark-FHE is NOT a near/medium-term ambition for dragons-clutch — its Tier-0
> rows are DROPPED-BY-DECISION for this horizon." — ember, 2026-08-27

That is a **horizon** ruling. `docs/MASTER_COMPLETION_CONTRACT.md:175` already
says so in its own open-decisions table: *"Privacy horizon | Whether the
accepted final public project includes the original FHE/MPC/energy objective |
open; **do not infer from the old horizon park**"*. C-15 exists precisely to
stop the 2026-08-27 ruling being read as a permanent scope cut it does not
claim to be.

**What is lost by cutting it.** Two things, and only the second is structural.
(i) The original motivating use case, which is not crypto: *"i originally had
wanted our dark fhe technology specifically so energy providers could settle an
efficient plan without revealing details about their operational…"* — ember,
`docs/INTENT.md:122-125`. (ii) The reason a piece of the current architecture
has its shape: *"Because our batch relation is small and specialized, it is a
much better future FHE/MPC/vFHE target than an arbitrary encrypted exchange
computer"* (`docs/INTENT.md:114-116`). `docs/INTENT.md:118-120` states the
consequence: **the batch relation is small and specialized on purpose, and if it
is ever "simplified" by someone who does not know why, a door closes
permanently.**

That gives the ruling a shape independent of whether FHE is ever built:

**Even a ruled-out C-15 must preserve the batch relation's specialization as a
stated invariant with its reason attached**, or the option is lost silently by
someone optimizing a relation that looks gratuitously narrow. Today nothing in
`docs/OMISSION_INDEX.md`'s `O-*` invariant table records it.

**What it would cost to keep.** The letter's retained charter starts with a
fixed-topology leakage/failure plan covering eight items. Foundation present in
the tree today: **none of the eight.** There is no commitment/note ledger, no
selective-decryption or key-rotation ontology, no encrypted owner allocation,
no non-equivocation/inclusion argument. A retained C-15 is a from-zero capability
charter, not an extension.

→ §10 rows R-8 and R-9.

---

## 9. What this lane did NOT do

Named as debt, not disclaimed.

- **No test was landed.** §1 explains why: none of the eleven dead codes is
  reachable, so every test I could write would be one that cannot go red. The
  one that *should* become reachable, `0x5006`, needs the route of §2.1 built
  first. `0x5644`, the other candidate, was fixed by S7 at source.
- **The remaining 55 NEVER-EXECUTED rows are not individually adjudicated.** Ten
  are now disposed of (§2.2); the other 55 are not proven true. They are,
  however, no longer being judged by an instrument with 12 dangling references
  in it, so hand-auditing them is now a reasonable next lane rather than theatre.
- **`routes.md` still cannot tell *bound* from *corroborated*** (§2.3). I
  repaired the references it resolves; I did not join it to the evidence ledger,
  which is the deeper fix and is R-4's remainder.
- **Three campaigns still emit no evidence.** `fractional-atomic`,
  `user-position-admission` and `general-hot` are recorded in `blocked.json` with
  owners, not bound. Writing bindings for them would have made `routes.md` say
  *witnessed* for transactions `census observe` could never corroborate — a green
  I declined to manufacture.
- **`general-hot`'s specific routes are unidentified.** It drives a General
  `OpenBatch` through real Trading and accelerator ELFs, but I did not establish
  which census route ids that lands on, so its `blocked.json` coverage names the
  campaign without claiming rows. Better than a wrong entry.
- **The 19 user-inaccessible selectors are not individually enumerated here.**
  I verified the two sharpest (§2.6) against the tree; the other 17 are S9's
  hand-resolved count, carried forward as theirs, not re-derived.
- **Eight of the ten magic collisions are adjudicated but unfixed.** I traced all
  ten to dispatch (§2.4.1), fixed the one same-ELF hazard that was a mechanical
  re-letter, and deliberately did not touch `DCLTRIX1` — it is a designed
  partition, and redesigning another lane's wire split is not a census lane's
  call. The remaining eight are cross-ELF naming smells; the gate stays red on
  them on purpose.
- **The 32 request-shaped magics of §2 are not individually traced** past
  `DCLCNS01`. Most are reached through codec constructors; the count is an upper
  bound on the orphan population, not the population.
- **Six of §4's eight contradictions are unfixed**, because their files belong to
  other lanes. They are routed in §10.1, not repaired here.
- **`docs/reference/` is stale at HEAD and I did not regenerate it.** Another
  lane's refusal split moved the inventory 297 → 299 codes, so
  `tools/genref/generate.sh` now rewrites eight files, only one of them mine.
  Whoever owns the next convergence should regenerate and commit; until then the
  in-tree register still reads 65, and this file's 55 is the measured truth.
  Hand checks reconcile exactly: 88 entry + 71 action = 159 routes, 13
  entrypoint anchors resolve.
- **The devnet-market liveness contradiction (§4, doc-vs-doc) is unresolved.**
  It needs an RPC read, which this lane had no task for.

---

## 10. The Ember ruling register

One line each. These are the rows this lane cannot close by engineering.

| id | ruling needed | owner if ruled "build it" |
| --- | --- | --- |
| ~~**R-1**~~ | **CLOSED by S7.** The guard landed at `fractional_claim_check_v1.rs:1196-1209` behind a shared `CorePhaseGateV3` both routes now read; `0x5644` left the dead list between this file's two measurements. | done |
| ~~**R-2**~~ | **CLOSED (`32fc79d5`) — it was never a ruling, it was a fact nobody had established.** The question "gone or reserved?" is answerable from the tree: `docs/ASPIRATION_LEDGER.md` D-6 records `DCLLBX02` as *"ANSWERED AND EXECUTED: deleted"*, dead on both ends, `WAVE.md:569` struck, and `docs/design/BASIS_ABI_UNIFICATION_V1.md:185` finds *"Producers of `DCLLBX02`: **none.**"* Gone. So the ten codes go with it, and the append-only objection does not apply: the route had no producer in the tree and no `DCLTLNK2` record was ever finalized on chain, so **not one of the ten was ever raised anywhere** and no historical log can resolve against them. `0x5101..=0x510A` withdrawn, never reused; `ClaimsState` renumbered to the sub-band base. | done |
| **R-3** | Claims split/merge as user acts (`claims.conserve`, `DCLCNS01`, `0x5006`) — a designed, tested, semantically-owned capability with no route, no dispatcher, no client. **Build the outer route, or rule it out with a date.** | Claims |
| **R-4** | **PARTLY DISCHARGED, `96ddf38f`.** Register repaired 65 → **55** and 12 stale binding refs → 0 (§2.2). What remains is not a ruling: (i) three passing campaigns must emit `record()` evidence before they can be bound — S7 for `fractional-atomic`, the Trading lane for `user-position-admission`, plus `general-hot`; (ii) `routes.md` must consult the evidence ledger, or cross-check its own two tables, so *bound* stops reading identically to *corroborated* (§2.3). | S11/gauntlet holds (ii); S7 and Trading hold (i) |
| **R-5** | **Withdrawn as a ruling — it is engineering with a working precedent.** Claims already solved this class inside this repo: 1,489 raise sites over 141 codes, 15.2% maximum, via the decision-0007 sub-band convention, against Trading's 2,086 on one code at 25.0%. A policy whose template is already in the tree and already load-bearing does not need Ember to choose it; it needs a lane to apply it and a census predicate to hold it. Re-routed. | protocol-wide lane + decision 0007 |
| **R-6** | U-001's "explicit deletion/non-authoritative-AOT ruling for standalone family artifacts" — Ember owns whether standalone family AOT artifacts are authoritative. Separately, V3 AOT has never compiled for `target_os = "solana"` and needs an owner. | Direct / AOT |
| **R-7** | `TicketAuthorship`'s payer and RentCredit arms are unreachable defense-in-depth after `73ffb010`. Document in place, or fold? *(small; listed so it is not re-discovered)* | Series |
| ~~**R-8**~~ | **CLOSED — RULED OUT 2026-09-01 by ember**, the second branch of this row's own disjunction (`GOAL.md:2071`, commit `5a371810`; record [`docs/decisions/0018-privacy-horizon-not-this-clutch.md`](../decisions/0018-privacy-horizon-not-this-clutch.md)). Verbatim: *"privacy/FHE is a 'not yet' for sure for sure, that would be a much later version of Clutch, solana isn't ready for that kinda awesomeness onchain yet (we'd want to use minidregg, which isn't ready yet)."* The dated ruling exists and the contradictory claims are removed in the same commit as the record. §8 of this file (`:772-829`, which now carries the same marker) is therefore **historical framing**, superseded: it describes the state of the question before 2026-09-01 12:47 and must not be read as an open charter. `O-019` is NOT closed by this — the ruling makes it load-bearing (`docs/OMISSION_INDEX.md:59`). | done |
| ~~**R-9**~~ | **DONE (`eaa4a1fa`) — it was transcription, not a ruling.** `docs/OMISSION_INDEX.md` **O-019**: widening the batch relation toward a general encrypted-exchange computer is now a named `hard invariant, narrowly stated`, carrying Ember's own reason verbatim and `INTENT.md:118-120`'s consequence — a door that closes permanently. Explicitly independent of C-15: if the ambition is retained it is a prerequisite, and if it is ruled out the row is what stops the option being lost on the way. | done |
| **R-11** | **Eight magic collisions remain, gate red (`7bf75057`).** Ten found; `DCLTDRS1` fixed at `b64ecbb5`, `DCLTRIX1` promoted to R-13. The other eight are cross-ELF or non-dispatch — real wire-reading ambiguity, no mis-dispatch path. Adjudicate each: fixtures import the canonical constant; genuine sharers renumber under a decision record while the deploy window makes it free. **Do not re-letter to make the gate green.** | Claims fixtures, Dealer/Direct codecs, Registry/Record/Relay |
| ~~**R-13**~~ | **CLOSED (`a19d93b1`), both halves, before the cohort deploys.** `RecordActionV1::Begin` moved `1` → `5`, so the documented partition is true by construction; `RECORD_FIRST_ACTION_V1` and `REGISTRY_ACTION_CEILING_V1` are published and bound disjoint by `const _: () = assert!` in `programs/dclutch-registry-sbf/src/lib.rs`, alongside a second assertion pinning the two widths distinct so the length clause cannot quietly become load-bearing again. The dispatcher's bare literal `2` now derives from the constant, and `record_v1.rs`'s `Some(1)` arm is `Some(5)`. `crates/dclutch-registry-svm/src/tests.rs` proves the ceiling is what `decode` actually admits, over all 256 action bytes. **Proved red first:** setting the ceiling to `2` fails the build with *"the Registry and record action ranges overlap"*. | done |
| **R-12** | **19 of ≈57 top-level selectors are reachable from neither CLI nor SDK — C-16 forbids exactly this.** Two are sharp enough to name: `DCLTDFS1`, C-04's permissionless third-party fee completion, *designed for a stranger* and callable only from our own campaign harness; and `DCLTPCA1`, the sole way out of collateral stranded by an expired projection, with no client at all. Build the client surfaces, or rule the capabilities out with a date. | Direct (C-04), Trading (recovery), SDK/CLI |
| **R-10** | **Re-routed — answerable, not rulable.** Six guides disagree on whether the first market is open on devnet and `deployments.ts` pins no market address. S1/S10 is standing up a fresh cohort under Ember's disposability ruling, so a measurement settles it and no ruling is owed. | S1/S10 |

---

## 10.2 How the gate goes green without laundering

The magic gate's red blocked `tools/genref/generate.sh`, `tools/gauntlet/run.sh`
and `npm run abi:route-census` across four lanes — including a resolution lane
that could not publish two provably-unique new magics for a reason that had
nothing to do with them. Narrowing the gate to the subset already fixed would
have been laundering. Recording each adjudicated fact where the gate reads it is
not, and the tree already had the pattern: a register entry with a written
verdict, so the instrument stays live and fires on anything new.

`tools/gauntlet/magic-collisions.json` (`273eba16`) holds nine adjudicated
collisions. Three rules make it a register of facts rather than a mute switch,
and each is pinned by a test that fails if the rule is removed:

1. **The verdict is required and must be argued** — 80 characters minimum, and
   an entry that fails it is itself a gate problem *and* leaves its collision
   reported. `an_exemption_with_no_argued_verdict_is_refused_not_honoured`
   checks `""`, whitespace, `"safe"` and `"n/a -- checked, it is fine"`.
   An exemption that records nothing can be added by anyone in a hurry; one that
   must be argued cannot.
2. **An exemption pins a set, not a value.** It lists the exact constant names
   observed; a third claimant makes the set stop matching and the collision
   fires again on its own terms rather than inheriting the old verdict
   (`a_new_claimant_breaks_the_exemption_rather_than_inheriting_it`).
3. **A stale entry is a problem.** An exemption whose collision no longer exists
   must be deleted, exactly as `blocked.json` requires
   (`an_exemption_whose_collision_is_gone_is_reported_stale`). This is what
   would have caught a `DCLTDRS1` entry surviving its own re-lettering.

`DCLTDRS1` is not in the file — it was re-lettered. `DCLTRIX1` is, and only
because R-13 replaced its prose partition with compile-time assertions; its
verdict says so and says to delete the entry if those assertions ever go. The
same-name mirror split is untouched, still pinned by its named test.

## 10.2b `HoardPrincipal -> FeeVault` is shape-admissible on the wire

Contributed by the conservation lane, recorded here because it is this ledger's
class in the economic dimension: **a rule that exists everywhere except where it
is enforceable.**

C-10 forbids the movement. Nothing in `dclutch-custody-contract` refuses it,
because **every compartment rule lives in a *calling* program and the contract
itself never enforced one.** Deliberate, and undocumented and unmeasured until
2026-09-01. Nothing pins the pair; both FeeVault-funding sites take
`TradingPrincipal`, so what exists is admissibility on the wire, not a live
leak.

The sweep that found it is the atom half of C-16's *unowned economic flow*: all
81 ordered compartment pairs through `CustodyRequestV1::validate`, plus a census
of every compartment-setting site — 54 source-side, 49 destination-side, **every
one owned** by a pinned literal, a closed match with a catch-all `Err`, a
direction accessor with literal arms, an in-contract pass-through, or the wire
decoder, where the owner is the authenticated calling program.

**Its qualifier travels with the number, and this ledger is where it must not be
lost: *swept clean at the construction sites, correctness not asserted.*** A
site pinning a wrong-but-literal pair reads as **owned** and still violates
C-10. Ownership and correctness are different questions; the instrument answers
the first. That distinction is the same one §10.3 draws for refusal codes — an
instrument reports a property, never a verdict — and it is why "every
compartment set has a named owner" must never be quoted as "every compartment
set is right".

The lamport half — rent beneficiaries, funding releases, closes, crank rewards
— is a separate unit and is in flight.

## 10.3 A dead refusal code has at least three causes

This session produced one of each, which is the argument for never treating the
census's output as a verdict on its own:

| cause | instance | disposition |
| --- | --- | --- |
| the route was never built | `0x5006` `CustodyRequired` — `claims.conserve` has semantics, a tested contract and no outer route | still dead, and now the **only** dead code in the protocol; R-3, being answered by implementation at `1d1c2453` |
| the guard was removed | the ten `LiabilityBasisSbfErrorV2` codes — route deleted, taxonomy left behind | withdrawn with the route (R-2, `32fc79d5`) |
| the guard was present under another name | `0x5644` `Phase` — I convicted it as missing; it was reachable and S7 landed the real guard | my accusation was wrong, and the census could not have told me |

**Denominator now: 297 protocol-visible codes, 1 with zero raise sites** — down
from 12 at this file's first measurement. The survivor's alias-blind control
still agrees: `CustodyRequired` occurs only at its declaration, its `ALL` entry
and its `ordinal()` arm, and Claims has no renamed imports of the enum.

## 10.4 The class at its worst, recorded

`crates/dclutch-svm-harness`'s `resolution_successor` campaign is the sharpest
instance of §1's class anyone has found: `primary_instruction` and
`funded_caller_instruction` were replaced by `panic!` stubs at 2026-08-26 00:19
(`d1325c7f`), and the `#[ignore]` landed **47 minutes later** (`583e5bfa`).
The campaign body was unreachable for six days **while the README quoted its
ten-row compute table as evidence** — two attributes and six unset environment
variables standing exactly where the execution used to be. Found and remediated
by the resolution lane; the README now states it at `:97-109`, and `WAVE.md:2329-2342`
carries the accounting. Recorded here because it is the purest form of the
defect this lane exists to find: **not a missing test, but a present one that
had been hollowed out while its own documentation went on citing it.**

---

## 11. Method and controls

**§1's raise-site cross-reference.** Every `Enum::Variant` occurrence in the
tree, gathered by one `rg -f` pass over word-boundary-anchored patterns, then
bucketed by the file it lands in and the line it sits on: `mirror` (non-`.rs`),
`doc` (comment line), `test` (`/tests/`, `tests.rs`, `program-test`,
`/benches/`), `tooling` (`tools/`, `apps/`, `packages/`, `fixtures/`,
`formal/`), `coderead` (`as u32` without `ProgramError::Custom`), `book` (inside
the enum's own `ALL` array, `ordinal()` body, or a `const _: () = assert!`
span, found by brace balance), and everything else `raise`. `Self::Variant` is
resolved inside the declaring file **and** inside any `impl … for <Enum>` block
elsewhere — without that last step the count is wrong by five, because
`RegistryError::ReleaseSuperseded`, `RentSbfError::…`, `CustodySbfError::…`,
`DealerSbfError::…` and `TradingSbfError::ReleaseSuperseded` are all raised
through `From` impls that say `Self::`.

Two measurement bugs were found and fixed while building this, both of which
would have inflated the dead list:

- **Prefix shadowing.** `rg -o -F` with an alternation of 297 fixed strings
  matches the *shortest* alternative at a position, so `RegistryError::Release`
  swallowed every `RegistryError::ReleaseSuperseded`. Anchoring each pattern
  with a trailing `\b` fixes it. Uncorrected, the dead list read 30 instead of 12.
- **Leftmost-match truncation.** A generic `\w+::\w+` scan matches
  `crate::TradingSbfError` in `crate::TradingSbfError::HeapFrame` and never sees
  the variant, so `0x4008` looked dead while
  `programs/dclutch-trading-sbf/src/entrypoint_adapter.rs:745` raises it.

**The control, stated separately.** The classifier is a heuristic, so the
twelve survivors were re-checked by a method that shares none of its
assumptions: a plain `rg` for the **bare variant name** across
`programs/dclutch-claims-sbf/src/`, plus a scan for renamed imports
(`… as Err`, `… as E`, `ClaimsSbfError as`, and the two other enums). Results:
no renamed imports anywhere in Claims; `CustodyRequired` occurs in no file but
`lib.rs`; `Phase` occurs in `fractional_claim_check_v1.rs` only at its
declaration, its `ALL` entry and its `ordinal()` arm; and each of the ten
`LiabilityBasisSbfErrorV2` variants occurs in zero files outside
`liability_basis_v2.rs`. The alias-blind control agrees with the classifier on
all twelve.

**§1.3's missing guard, controlled.** `rg` for `CoreState`,
`MarketCoreStateSeeds`, `core.phase`, `Phase::` and `terminal_receipt` across
`programs/dclutch-claims-sbf/src/fractional_claim_check_v1.rs` returns nothing.
The module does not read Core state at all, so the guard is absent rather than
relocated.

**A third measurement bug, in this file's own §2.2.** The first draft said the
tree had "13 binding families" and listed `protocol-position` and `sparse-chain`
among the unbound. Both wrong, from one cause: the scan looked for the exact
filename `bindings.json` and genref accepts `*-bindings.json` too. There are
**15** binding files across 15 campaigns, and `protocol-position` and
`sparse-chain` are bound — under `claims-custody/claims-bindings.json`, campaign
`claims-family-programtest`, which is why the register already showed their
routes executed. Corrected above. The lesson is the same one that produced the
first two bugs: **a scan that names its own pattern is only as complete as the
pattern**, and the fix each time was to read the consumer's own matching rule
rather than infer it.

**§2.2's repair, controlled three ways.** The claim "ten rows were false" is a
claim about an absence turning into a presence, so it needs a live channel:

1. **The channel is proved live.** `claims/founding_v5::process` — a route known
   to execute in tier 1 — reads `WITNESSED [('tier1', 'executed')]` through the
   same replicated `routeStatus()` used for every other row. A "not witnessed"
   reading is therefore meaningful rather than a dead lookup.
2. **The delta is isolated.** The same inventory was scored twice, once against
   `git show HEAD:tools/gauntlet/blocked.json` and once against the edited file:
   witnessed unchanged at 67, blocked 27 → 35, never 65 → 57, i.e. exactly the
   eight entries added and nothing else. The two bindings repairs were then
   measured separately (67 → 69 witnessed, 57 → 55 never).
3. **Nothing regressed.** The never-executed set after the repair was diffed
   against the set parsed straight out of the committed `docs/reference/routes.md`
   — a source that shares no code with the replication. Ten rows left, and the
   newly-never-executed set is empty.

The status logic was replicated from `generate.mjs` rather than invoked, because
running `tools/genref/generate.sh` writes eight files into `docs/reference/` and
the tree is currently stale there for an unrelated reason (another lane's refusal
split moved the inventory from 297 codes to 299). Regenerating would have put
that lane's change in this lane's commit. The replication is exact on the two
rules that matter: `bindings.json` **or** `*-bindings.json`, and most-specific
blocked rule wins with a trailing `*` as prefix glob.

**§7's family sweep, controlled.** Static reading of permission triples is not
evidence on its own, so: `cargo test -p dclutch-direct-codec --lib
one_payer_signs_a_registered_creation_and_the_record_payer_aliases_it` passes at
HEAD (1 passed, 195 filtered out) — Direct's registered creation profile joins
`validate_plan_permissions` for real. Series was verified by coordinate rather
than by test, because compiling `dclutch-trading-sbf`'s host suite is not a cost
this claim justifies on a shared machine; the coordinates are named in §7 and
the grants are exact.

No unfiltered `-p <crate>` suite was run for this file.

### 10.1 Routed, not ruled — engineering with a named owner

Nothing here needs Ember; each needs the lane that owns the file.

| finding | owner | one-line fix |
| --- | --- | --- |
| §4 #1 — `README.md:166` and `docs/guides/reader.md:67` teach a command that aborts in argparse | docs / onboarding | replace with the 9-flag form already written at `docs/operators/found-a-market.md:117-126` |
| §4 #2 — `client-developers.md:59` teaches a 360-byte `DCLTCOR3` filter | SDK / client docs | 368; 360 is `HISTORICAL_CORE_GENERATIONS_V1` |
| §4 #3 — `ARCHITECTURE.md`'s supersession banner names `DCLTCOR2` as current | architecture | `DCLTCOR3`, `CORE_VERSION = 3` |
| §4 #4-6 — `tools/gauntlet/tier2/README.md` is wrong about its module, its mode flag and its campaign registry | gauntlet | rewrite against `program-test-evidence` as a path dependency, `--mode census\|full`, and the real runner layout |
| §4 #7 — `programs/dclutch-claims-sbf/README.md:53` names a producer file that does not exist | Claims | re-verify the producer list before anyone acts on the deletion-readiness claim |
| §4 #8 — `tools/dclutch-cli/README.md:13` installer pins a version predating the commands it teaches | CLI | repin to `0.1.0-devnet.3` |
| §4.1 — `docs/reference/refusals.md` promises every listed code is returnable; eleven are not | genref / census | one raisability predicate in the census, which also gives §2.3 the notion it lacks |
| §2.2 — three campaigns pass and emit no census evidence | S7 (`fractional-atomic`), Trading (`user-position-admission`), plus `general-hot` | add the `dclutch-program-test-evidence` dependency and one `record()` per submitted transaction; the runner and bindings then follow here |
| ~~§1.3 — `0x5644` guard~~ | ~~Claims / fractional~~ | **done** — `fractional_claim_check_v1.rs:1196-1209` |
