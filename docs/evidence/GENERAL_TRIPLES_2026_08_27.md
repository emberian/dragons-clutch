# The rung under the seven triples — and why they are one unit, not seven

Lane GEN-TRIPLES, 2026-08-27. Commits `b8d94e25`, `3affdadc`, `bfcbc5d5`.
Charter: author the seven collection and candidate artifact triples in
GEN-ESCROW's eleven-site dependency order, each landing with its caller.

This is not release evidence. **The seven triples are NOT authored**, and §1
gives the structural reason a lane cannot author them one at a time — which is
the finding this lane exists to have made, because it is what the charter's own
"EACH landing with its caller" turns out to require. §2–§4 are what landed
instead: the three things that had to be true before the first triple could be
written at all, each gated, each moving zero artifact identities.

---

## 1. The finding: the seven triples are ONE unit for release admission

GEN-ESCROW's evidence document §5 item 1 lists eleven sites "in dependency
order". That reads as an order a lane can walk one action at a time — author
`OpenBatch` at all eleven, exercise it in the campaign, then the next. **It is
not**, and the reason is in `release_v3.rs`, which is site 8 on that same list.

ADR-0010 §4 replaced the `== 7` rule with four NAMED PROFILES rather than a `>=`.
Their action counts are **7 and 14, and nothing else**:

| profile | entries | action coordinates |
|---|---|---|
| `SettlementOnly` | 7 | 7 |
| `SettlementWithActivation` | 8 | 7 + the activation descriptor |
| `Complete` | 14 | 14 |
| `CompleteWithActivation` | 15 | 14 + the activation descriptor |

Entry counts 9 through 13 are refused by `from_entry_count`. The one legal count
between 7 and 14 is 8, and that eighth entry is the activation descriptor —
selector `255`, which `Action::decode` refuses — **not an eighth action**.

So a release carrying one authored collection or candidate triple has no legal
set to be published in. It cannot be joined by
`authenticate_general_release_v3`, cannot be admitted, cannot be reached through
the accelerator, and cannot be executed by the campaign. That is precisely the
STRUCT-CAMP "an artifact builder with no caller outside its own crate is parked,
not landed" state — the distinction the charter says this cycle has already paid
for twice.

**Decided, not asserted**:
`release_v3::tests::there_is_no_profile_between_the_seven_settlement_triples_and_all_fourteen`.

### What follows

"EACH landing with its caller" is satisfiable only by **all seven at once**, or
by a fifth profile. ADR-0010 §4 already rules on the second: "Adding a fifth
profile is a visible edit, not a consequence of publishing a longer set." That
makes it an authority decision about what a General release may enumerate, not a
lane's to take — and the recommended answer is **no fifth profile**. The seven
actions are one economic half of the family (a batch opens, orders are admitted
and cancelled, the batch closes, a candidate is submitted and verified, orders
are released); a release that could publish three of them would be a release
whose collection half is partially reachable, which is a worse thing to be able
to deploy than one that refuses.

The scheduling consequence, stated plainly for whoever writes the next charter:
**the seven triples are a multi-lane unit that lands in one commit-series and one
regeneration.** Sizing them as "one triple per lane, seven lanes" produces seven
parked artifacts and zero census rows.

## 2. The General family had no Lean transition module, and the charter assumes one

The charter names the house method as "TransitionVM program Lean-authored per the
house method — TransitionVMV3.lean, byte-gates". `grep -rl TransitionVMV3` over
`formal/` returned five files and **every one of them was Direct's**.

General's seven transition programs were built imperatively in
`transition_artifacts_v3.rs`, which carried its own instruction counts as a Rust
`match`. That is exactly the gap `73f0793` closed for Direct. So the method the
seven NEW programs are supposed to follow had no rung to stand on: authoring
`OpenBatch`'s program "per the house method" meant first building the house.

`DClutchSemantics/GeneralTransitionV3.lean` + `EmitGeneralTransitionV3Rust.lean`
author all seven (commit `3affdadc`). The register space is typed as three
constructor lists whose order IS the wire index — 90 scalars, 6 item scalars, 40
identities — and `hot_candidate_v3.rs` remains the name authority, joined by a
test rather than by trust.

### The gate is byte-identity, both directions

| gate | what it refuses |
|---|---|
| `every_authored_program_is_byte_identical_to_the_lean_authored_one` | the imperative builder producing anything other than the emitted array, for all seven actions |
| `checked_in_general_transition_programs_are_exact_lean_output` | a Lean edit nobody regenerated |
| `the_lean_register_schema_is_the_one_the_rust_bank_declares` | either side renumbering the bank |
| `the_emitted_geometry_agrees_with_the_declared_instruction_counts` | a regenerated program disagreeing with the count that sizes its caller's workspace |

Neither of the first two is sufficient alone: without the second, an edited Lean
module leaves two agreeing Rust authorities and one silent Lean one.

**It passed on the first run.** The transcription is faithful, so nothing
regenerated and **no artifact digest moved** — this lane carries none of the
batched identity regeneration the charter anticipated.

### Decided in Lean, not argued

Well-formedness for all seven; the section counts the Rust `match` declares
(15/1, 17/1, 21/2, 21/4, 16/1, 21/4, 27/6); the seven encoded widths (416, 464,
584, 632, 440, 632, 824); **pairwise distinctness** — a shared prelude plus an
empty action half would give two artifacts one identity, and the digest is what
the descriptor and the capability seal name; that no prelude conjunct addresses
the item space, which is what lets one artifact serve N=1 and N=258; and that the
three actions binding a vault context are exactly `Collect`/`Distribute`/`Close`,
with `Materialize` excluded because it patches its compartments at runtime.

The seven unauthored actions get an explicit verdict rather than a default:
`no_unauthored_action_carries_an_action_conjunct` decides that `program` answers
them with the shared prelude alone, and the emitter never asks for one.
`general_transition_program_bytes_lean_v3` returns the empty slice, which
`ProgramV3::decode` refuses — the same fail-closed shape
`general_request_profile_bytes_v1` uses.

Zero `sorry`. The module is in `DClutchSemantics.lean`, so a plain `lake build`
typechecks it and runs its `native_decide` theorems. **`GeneralRequestProfilesV1`
is still NOT in that list** — which is why its own freshness test has to build it
explicitly, and is a gap worth closing.

## 3. The four records had no physical envelope

GEN-ESCROW's §5 names the register-bank growth as the item that is "NOT on that
list". Underneath it is one that was not on any list.

`GeneralBatchV1`, `GeneralOrderV1`, `GeneralCandidateV1` and the verifier cursor
are pure contract records with **no Trading-owned envelope**. Nothing said what
their canonical PDA bump was, who owned their rent, or how a vacant successor is
created. A record with no envelope cannot be the primary state of a lifecycle
plan — and a lifecycle plan is what an AccountProfile's `LifecycleBound` rule and
an EffectProgram's state patches are written against. So site 3 of the eleven
(`state_artifacts_v3`'s lifecycle counts) had nothing to count.

`GeneralLocalStateKindV3` grows from two kinds to six (commit `b8d94e25`). Each
new body hostile-decodes through its own semantic owner — the envelope asserts no
field of a record it does not own — and each is sized by that owner's own width
function, so the fixed-width pair (`Batch`, `Candidate`) and the Product-width
pair (`Order`, `Verifier`) are separated by `is_fixed_width` rather than by a
caller remembering which is which.

Witnesses: the six tags partition the tag space and 0/7/8/255 do not decode;
every kind sizes at N=1 and N=258 with the fixed/variable split checked rather
than asserted; the two collection records round-trip at three widths; and **a
body that is not the kind its header names refuses in both directions** with the
output preserved — the kind byte is what the AccountProfile rule and the
transition conjunct both read, so a disagreement there would let an artifact
authored for one record authenticate another.

Honest scope: `Candidate` and `Verifier` bodies are not round-tripped, and the
test says why. A candidate's identity is its own masked digest and a verifier
cursor is written by the first row verification, so neither has a pure
constructor.

## 4. The one blocker-class question in front of `OpenBatch` is answered: yes

`OpenBatch` and `CloseBatch` advance `GeneralRootV2` — `revision`,
`next_batch_sequence`, `open_batches`. **That has never happened on a chain.**
Every caller of `open_batch`/`close_batch`/`begin_retiring` in the tree is
host-side or a test, so a General root's revision has been frozen at its
activation value since the day it was created. Whether a Trading effect may write
the capability root at all was therefore an open question that would have blocked
those two triples' design.

It may, on every leg:

- the composite root is a **Trading-owned PDA** — `outer.rs` allocates it and
  assigns it to the family program, and Core's activation post-condition requires
  exactly that owner;
- Trading guards root writes **by offset, not by owner**:
  `require_root_write_is_state_only` refuses only offsets below
  `CAPABILITY_ROOT_HEADER_BYTES_V1`, which is precisely where the `GeneralRootV2`
  tail begins — its own test pins the first state byte as accepted;
- coordinate 0 is deliberately exempt from the read-only clamp Trading applies to
  common coordinates 1..=4;
- and **Direct and Series already do it** — Direct's ordinary and registered
  effect programs write the root's open-maker count through this exact shape.

General's coordinate-0 rule already declares the account writable; what it
withholds is the effect grant. The change those two triples need is one argument
at `common_rule`'s coordinate-0 arm — `no_effects()` becomes
`AccountEffectPermissionsV2::new(false, false, true)` — **action-selected** for
the two actions that advance the root and no others; granting it to an action
that does not write the root would widen what a release may do for nothing in
return. Recorded at that site, in commit `bfcbc5d5`, so the next lane reads it
where it will edit.

## 5. Measurements: the campaign, and the one cost this lane paid

`tools/gauntlet/general/run-general.sh`, end to end.
**19/19 (3 + 2 + 10 + 4), zero frame diagnostics, 8/8 witnesses, 18 observations
admitted under the ledger lock.**

**Accounts, legacy packet extent and scratch pages are IDENTICAL to GEN-ESCROW's
baseline in all 23 rows.** CU moves by a small constant per action, identical at
N=1 and N=258, so no slope moved:

| action | ΔCU | N=1 | N=258 |
|---|---:|---|---|
| Consider | +16 | 36,097 → 36,113 | 74,861 → 74,877 |
| Freeze | +16 | 32,643 → 32,659 | 65,054 → 65,070 |
| InitializeSettlement | +1 | 61,320 → 61,321 | 164,453 → 164,454 |
| Collect | +12 (×3) | 56,979/58,161/58,184 → 56,991/58,173/58,196 | 146,935/147,362/148,156 → 146,947/147,374/148,168 |
| Materialize | +12 | 53,159 → 53,171 | 141,390 → 141,402 |
| Distribute | +12 (×3) | 56,930/58,135/58,166 → 56,942/58,147/58,178 | 144,573/145,794/146,596 → 144,585/145,806/146,608 |
| Close | +12 | 61,322 → 61,334 | 155,774 → 155,786 |

The binding action stays `InitializeSettlement`: 164,453 → 164,454 of 1,400,000,
**11.75% of the compute ceiling to two decimals, unchanged**. Compute is not the
wall; the packet still is, and six of seven N=258 actions remain over the
1,232-byte legacy maximum, unmoved.

**Attribution, and its limit.** The only thing in this lane that the accelerator
ELF executes is §3's enlarged `GeneralLocalStateKindV3::decode` and
`general_local_state_len_v3`: §2 moved zero bytes (that is what the byte gate
says) and its emitted arrays have no caller in the ELF, and §1 and §4 are a test
and a comment. So the four new state kinds are the cost, and it is between +1 and
+16 CU per action. **I attributed that by elimination, not by phase measurement**
— the per-action variation (+1 for `InitializeSettlement`, +16 for the two
selection actions, +12 for the rest) is not explained here, and it is small
enough that measuring it would cost more than it is worth to know.

One reading note for whoever diffs against GEN-ESCROW's table: the multi-row
`Collect` and `Distribute` cells there are **sorted**, and stdout emission order
is not. Matched by sorted value every row is exactly +12; matched by emission
order two of them appear to move by ±800, which is an artefact of the comparison
and not of the run.

Also green: adapter 185 lib + 6 + 2 + 1 + 1 (was 176 + 6 + 2 + 1);
`dclutch-operator` general 22/22; `cargo check --workspace --all-targets` clean
(two pre-existing warnings, neither this lane's:
`core-sbf/src/resolution.rs:1317` dead `child_account_count`, and an unused
import in `operator/src/direct_inline_v3.rs:19`); full `lake build` green,
91 jobs, zero `sorry`.

## 6. What the next lane inherits — the corrected work list

GEN-ESCROW's eleven sites are still the right list. What this lane changes about
it:

1. **It is one lane's charter for all seven actions, not seven charters.** §1.
   Budget it as a multi-lane unit landing in one commit-series and one
   regeneration, or take the fifth-profile decision to ember first.
2. **Site 6 (`transition_artifacts_v3`) now means "add a branch to
   `GeneralTransitionV3.lean`"**, not "write a program in Rust". §2. The
   remaining step there is to delete the imperative builder in favour of the
   emitted arrays; the byte gate is what makes that safe to do, and it is
   deliberately left undone because deleting it while the seven new programs do
   not exist would remove the only thing the new ones can be gated against.
3. **The register-bank growth is still owed and is still one batched
   regeneration.** Nothing in this lane moved an identity, so the batch is
   exactly GEN-ESCROW's `Collect` effect digest, its `Collect`/`Distribute`/
   `Close` transition digests, and whatever the growth adds. Sized from the four
   record shapes: roughly 60 new scalars and 10 new identities, and **+2 to the
   item scalar stride** for `PlaceOrder`'s per-outcome `receive_per_lot` /
   `deliver_per_lot`, which is 6 → 8 and costs 2·N·8 bytes of bank at every
   width. That last one is the only part with a real runtime price and it is the
   one worth a second opinion.
4. **`OpenBatch`/`CloseBatch` need one profile argument, not a redesign.** §4.
5. **The current slot has no register.** `GeneralBatchV1::open` and `admit` both
   take `current_slot`, and General's AccountProfile declares
   `TrustedEnvironmentV2::None`. The collection half needs
   `TrustedEnvironmentV2::CurrentSlot { destination }` and a scalar to receive
   it; the mechanism exists and General has never used it.
6. **`GeneralRequestProfilesV1.lean` is not imported by `DClutchSemantics.lean`**,
   so `lake build` does not typecheck it or run its `native_decide` theorems.
   Adding the seven request profiles there without fixing that leaves seven new
   decided theorems that the default build never decides.

Unchanged from GEN-ESCROW §5 and still owed: the lamport mover behind the
triples, the canonical-width census row (ALT/v0), General's absent
`CU_BUDGETS.json` rows, `ExpireSettlement`, the claim escrow's Position
lifecycle, and `GeneralClearing.lean`'s collection and candidate halves.

## 7. Reproduction

```sh
cargo test -p dclutch-general-adapter-contract
#   lib 185 · unauthored_actions_v1 6 · root_lifecycle_projection_v3 2
#   request_profiles_generator_fresh 1 · transition_programs_generator_fresh 1
(cd formal/dclutch-semantics && lake build)
tools/gauntlet/general/run-general.sh
```

The two generator-freshness tests shell out to `lake`, so a checkout without the
Lean toolchain fails them by name rather than skipping them.
