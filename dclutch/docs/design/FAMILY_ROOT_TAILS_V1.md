# Rational and Structured capability-root tails, V1

2026-08-30, ROOTTAILS lane. `DECISION_PACKET` §4.

Rational and Structured are the two families the reviewed activation template
(`docs/evidence/CAPABILITY_ACTIVATION_TEMPLATE_2026_08_30.md` §2) records as
"blocked one layer deeper": they are not missing an activation artifact, they
are missing the thing an activation creates. This document authors that thing —
each family's initial capability-root tail — as a permanent ABI, and states the
plan by which those bytes are emitted rather than hand-written.

It decides bytes. It does not publish a release, and it does not implement; §11
states why and what implementing would cost.

## 1. What "no root tail layout" means, measured

`root_state_bytes` is a free caller parameter in both families, and the value
every caller passes is a literal with no author.

| | Rational | Structured |
|---|---|---|
| Family crate | `dclutch-rational-lifecycle-hot-v3` | `dclutch-bearer-v2-operator` (the shared open-capability layer), compiled by `dclutch-operator::structured_selected_release_v1` |
| Actions | 4, `RATIONAL_LIFECYCLE_SELECTED_ACTIONS_V6` | 5, `OPEN_CAPABILITY_SELECTED_ACTIONS_V1` |
| `root_state_bytes` today | `64` | `8` |
| Sole non-test site | `tools/local-validator/bootstrap/successor/src/rational_market.rs:201`, inside `demo_rational_market_input`, whose own header labels the surrounding values **"LAB FACTS, labeled"** | `tools/local-validator/bootstrap/successor/src/structured_market.rs:209`, same lab function shape |
| Paired `root_schema` | `lab("root-schema")` — SHA-256 of the plan's release-set id | `lab("root-schema")`; elsewhere `identity(0x11)`, `id(11)`, `[0x42; 32]` |
| Type / decoder / accessor | none. `RationalRoot` has zero hits tree-wide | none for the capability tail |
| Magic, version, phase, counter | none, none, none, none | none, none, none, none |

Neither literal is constrained by anything except `CapabilityProgramV4`'s own
validity check — `root_state_bytes != 0` and `<= CAPABILITY_ROOT_STATE_MAX_BYTES_V1`
(4096). Any nonzero value under 4096 passes. `64` and `8` are placeholders
meaning "some bytes" and "a u64's worth"; they are not measurements, and nothing
on chain depends on either.

**Two traps worth naming before the layout.**

`STRUCTURED_ROOT_BYTES_V2 = 128` and `StructuredRootV2` exist, are Lean-emitted,
and are **not** this. That type is the Structured family's own node account at
the PDA `[b"dclutch:structured-root:v2", terms_id]`; the capability root is
Trading-owned at `CapabilityRootSeedsV1`. Reusing `StructuredRootV2` as the
activation tail would import exactly the three fields that make Fractional's
activation impossible (`dclutch-capability-activation-codec` header, "Fractional's
root cannot be composed at all"): a PDA `bump` at offset 10, derived after the
effect runs; a `terms` digest at 16 over Market-carrying bytes; a
`rent_beneficiary` at 80 with no seam author. `STRUCTURED_ROOT_SCHEMA_ID_V2` is
likewise never wired to any descriptor's `root_schema` — its only references are
its own definition, a re-export and the generator freshness test. The capability
tail must be a new, smaller type.

Rational has no such near-miss: it has nothing at all.

## 2. The question, and both families' answer

The template's gate (`CAPABILITY_ACTIVATION_TEMPLATE_2026_08_30.md` §3):

> **Is every byte of the initial root tail either a constant the family publishes,
> or a seam-seeded register the outer fills in before any artifact runs?** There
> is no third source: the effect kernel has no arithmetic and the activation
> frame holds only the root and the funding ledgers.

**Both families answer yes, with the strongest possible form of yes: every byte
is a published constant and neither family declares a single seam field.**
Direct is the precedent (24 bytes, all constant); General needed three
(`GENERAL_ACTIVATION_TAIL_FIELDS_V1`); Fractional could not answer. §5 gives the
rule that puts these two on Direct's side of that line rather than General's.

## 3. The tails

Both are 16 bytes: two aligned eight-byte words, both nonzero, both composed as
transition constants. Offsets are tail-relative; a physical reader adds
`CAPABILITY_ROOT_HEADER_BYTES_V1` (232) exactly once, as
`GENERAL_ROOT_LIFECYCLE_OFFSET_V2`'s doc comment prescribes.

### 3a. `RationalCapabilityRootV1` — 16 bytes

| Field | Offset | Width | Source | Why |
|---|---|---|---|---|
| `magic` | 0 | 8 | published constant `RATIONAL_CAPABILITY_ROOT_MAGIC_V1` = `b"DCRNCRT1"`, loaded into transition scalar 9 | Domain separation is the only thing standing between a Rational root and any other family's bytes; the outer writes the tail without decoding it, so the tail's first word is the sole self-identification a later decoder has. Deliberately unlike `DCSTRT02` and `DCLTCRT1` so a Structured node root and the shared capability *header* can never be mistaken for it. |
| `version` | 8 | 2 (u16 LE) | part of the header word, transition scalar 10 | `RATIONAL_CAPABILITY_ROOT_ABI_VERSION_V1 = 1`. A tail with no version can never be evolved: the width is frozen in a published descriptor, so the only migration lever left is a discriminated decode. |
| `phase` | 10 | 1 (u8) | same word | `RationalCapabilityRootPhaseV1::Active = 1`. §6 states exactly what it may hold today and what must exist before a second value is minted. Offset 10 is where Direct, General and Structured's runtime projection all put theirs. |
| `reserved` | 11 | 5 | same word, canonical zero | Fills the header word to eight bytes so `version`, `phase` and the padding are **one** aligned scalar write and **one** `ProjectDataU64` read. The decoder refuses any nonzero byte here, which is what makes the word's value pinnable as a constant (`GENERAL_ROOT_ACTIVE_HEADER_WORD_V2` is the same trick). |

Composed header word: `0x0000_0000_0001_0001`.

### 3b. `StructuredCapabilityRootV1` — 16 bytes

| Field | Offset | Width | Source | Why |
|---|---|---|---|---|
| `magic` | 0 | 8 | published constant `STRUCTURED_CAPABILITY_ROOT_MAGIC_V1` = `b"DCSTCRT1"`, transition scalar 9 | As above, and here the separation is load-bearing twice over: `STRUCTURED_ROOT_MAGIC_V2 = "DCSTRT02"` already names a different 128-byte Structured account. Two Structured "roots" that share a magic would be one bug away from decoding each other. |
| `version` | 8 | 2 (u16 LE) | header word, transition scalar 10 | `STRUCTURED_CAPABILITY_ROOT_ABI_VERSION_V1 = 1`. Numbered independently of `STRUCTURED_SCHEMA_VERSION_V2 = 2`, because this tail is not that ABI and tying them would make a bump in either a bump in both. |
| `phase` | 10 | 1 (u8) | header word | `StructuredCapabilityRootPhaseV1::Active = 1`. Note the rebase: `StructuredPhaseV2 { Open = 0, Terminal = 1, Retired = 2 }` exists at *projection* offset 10 and is a runtime observation of Token and Market facts, not persisted root state. Reusing its zero-based discriminants here would make an all-zero tail a valid state, which §6 rejects. |
| `reserved` | 11 | 5 | header word, canonical zero | As above. |

Composed header word: `0x0000_0000_0001_0001` — the same number, under a
different magic. That is the intended outcome of two families answering the same
question the same way, not a shared type; §7 keeps them separate.

### 3c. The four families side by side

| Family | Tail | Magic | Restates header facts | Seam fields | Counters |
|---|---|---|---|---|---|
| Direct | 24 B `DirectRootStateV1` | `DCLTDRT1` | no | 0 | `open_maker_root_count` @16 |
| General | 128 B `GeneralRootV2` | `DCGROT02` | market, config id, generation | 3 | `revision` @88, `next_batch_sequence` @96, `open_batches` @104 |
| **Rational** | **16 B** | `DCRNCRT1` | **no** | **0** | **none** |
| **Structured** | **16 B** | `DCSTCRT1` | **no** | **0** | **none** |
| Fractional | — | — | — | impossible | — |

## 4. Where each byte comes from, in the seam's own terms

The activation frame's register banks are `ACTIVATION_COMMON_SCALARS_V2 = 8` and
`ACTIVATION_COMMON_IDENTITIES_V2 = 12`; the parked rent quote lands at scalar 8
(`ACTIVATION_RENT_QUOTE_SCALAR_V1`), and family constants start at scalar 9
(`ACTIVATION_FIRST_CONSTANT_SCALAR_V1`).

Each family's composition, identically:

| Composed write | Register | Value |
|---|---|---|
| tail `[0,8)` | scalar 9 | the family magic word |
| tail `[8,16)` | scalar 10 | `0x0000_0000_0001_0001` |

Resulting geometry: 11 scalars (9 + 2 constants), 12 identities (the common
identity bank, declared but unread — the same shape Direct's
`ACTIVATION_IDENTITY_COUNT` uses). Both are far inside
`ACTIVATION_MAX_RUNTIME_SCALARS_V2 = 96` and
`ACTIVATION_MAX_RUNTIME_IDENTITIES_V2 = 32`.

Every refusal in `dclutch-capability-activation-codec`'s constructor gate is
either satisfied or vacuous for this shape, and that is the point of choosing it:

- `TailFieldRegisterOutOfBank`, `TailFieldOverwritesConstant`, `TailFieldGeometry`
  — **vacuous**: `seam_fields` is empty, so the whole class of "a field reading a
  family register composes a silent zero into the root" cannot arise.
- `TailAlignment` — satisfied: both nonzero words are eight-byte aligned by
  construction.
- `RootWidth` — satisfied: 16 equals the declared `root_state_bytes` and is well
  under `ACTIVATION_MAX_ROLE_REQUEST_BYTES_V2 = 2048`.
- `RegisterGeometry` — satisfied, as above.
- `ProjectedTailMismatch` / `ProjectedRentMismatch` — the brick gate still runs
  the real effect kernel over the built effect; it just has less to disagree
  about.
- `outer.rs`'s own last refusal, "an activation that projects no family state at
  all creates a root whose tail no family can decode", is satisfied because the
  magic word is nonzero. A tail of eight zero bytes — which is what Structured's
  current `root_state_bytes: 8` would compose if anyone tried — is refused there
  by construction.

General's completeness check, `RuntimeVaryingByteNotDeclared`, has no analogue
here and needs none: it exists to catch a byte that varies with `(market,
config, generation)` and sits outside a declared field. Nothing in either tail
varies with anything.

## 5. Why not General's shape — the rule

The obvious move is to copy General: market at 16, config id at 48, generation at
80, three seam fields. It is the wrong move, and the rule that says so is worth
stating permanently because it will come up for the next family too.

**A tail restates a header fact only when a family decoder holds the tail without
the header.**

- `CapabilityRootHeaderV1` is 232 bytes and already owns the release set (16),
  the Market (48), the generation (80) and the full 144-byte
  `CapabilityExecutionSelectionV1` (88), which carries manifest, entry index,
  kind, capability release and config. All seven non-domain seeds of
  `CapabilityRootSeedsV1` are projected from it. Nothing in the tail is needed to
  derive the account's own address.
- General restates three of those anyway, for a real reason:
  `GeneralRootV2::require_hot_context` binds root↔config↔market from the decoded
  *tail*, without trusting the caller and without decoding the header.
- **Neither Rational nor Structured has that reason, because neither decodes its
  tail at all.** Rational observes the root exactly once, as
  `hash(state.root_data)` over the whole composite account, fed to
  `HotExecutionEnvelopeV3` at offset 88 — an opaque digest that carries no offset
  knowledge (`RationalLifecycleHotStateV3::root_data`, and identically
  `hot_transaction_v3.rs` for the open layer). No Rational or open request
  profile reads the root. No Rational or open account-profile *operation*
  projects out of coordinate 0 — the effects declare exactly one route with role
  `FixedRole::Claims`, pinned by `open_selected_v3.rs:418`,
  `open_structured_v3.rs:524` and `hot_effect_v3.rs:279`.
- Direct — the family with by far the most on-chain root traffic — restates
  nothing either. General is the exception with a named cause, not the pattern.

The cost of copying General anyway would be 96 bytes of duplicated rent per root
for facts already in the account, three seam fields where zero are needed, and a
second author for three header-owned facts. This project treats a second author
as the cardinal defect (`P-007`; `general_root_creation_tail_v2`'s own "These are
not a second authority"); a tail that restates the header without needing to is
that defect, volunteered.

## 6. The phase byte, and the honest thing to say about it

A discriminant nothing can write is a hazard if it is shipped as a design. The
tree has ruled on the general case twice, and both rulings are in the very
modules these families use: `open_lifecycle_policy_v5` and
`lifecycle_policy_v5` both chose an **empty** policy over a decorative one,
because "an unreachable plan is worse than no plan: it reads as a design to
anyone auditing the release, and it is not one."

So the phase byte ships under three constraints.

**It costs nothing.** The header word is one aligned `u64` whether byte 10 is a
phase or a sixth reserved byte. There is no width, rent or CU difference.

**It is readable today, by an existing mechanism.** General's hot account rules
already project a root lifecycle byte with
`AccountOperationInputV2::ProjectDataU8` at
`CAPABILITY_ROOT_HEADER_BYTES_V1 + GENERAL_ROOT_LIFECYCLE_OFFSET_V2`, over the
comment "without this projection a `Retiring` or `Retired` General capability
executes hot actions exactly like a live one." Direct's native-close account
profile does the `ProjectDataU64` equivalent at the magic word, the header word
and the counter. Both mechanisms work on a Rational or Structured root the day
the tail exists. The reader half is not speculative.

**Exactly one value is admissible, and the decoder refuses the rest.**
`Active = 1` only. `0` is invalid, so an all-zero region is never a state — this
is `GeneralLifecycleV2`'s convention (`Active = 1, Retiring = 2, Retired = 3`)
rather than `DirectRootPhaseV1`'s older zero-based `Open`, and it is the better
one for a tail that is otherwise constant. `2` and `3` are **reserved in this
document as `Retiring` and `Retired` and refused by the decoder**, so that
admitting one later is a decoder relaxation plus a route, not an ABI change, and
so that no release ever publishes a state machine it cannot drive.

What must exist before a second value is minted, named rather than buried:

1. **A writer.** Today `split_root_account_mut_v1` has no production caller;
   nothing outside activation writes any family tail through the neutral seam.
   Direct's transitions are family-specific top-level Trading instructions
   (`direct_begin_retiring_v1`), and the neutral close route
   (`require_close_selection`) reads only the header. Either shape would serve;
   neither exists for these two. Note that *permission* is not the obstacle:
   Rational's account profile already marks coordinate 0 writable
   (`account_profile.rs`'s `let writable = index == 0 || …`), and so does the
   open layer's hot transaction. The lifecycle policy's sentence "Rational's
   account profiles never reference the root coordinate at all" is stale as
   written; what is true is that no rule *operation* projects out of it and no
   lifecycle plan names it.
2. **A reader on the action path**, which is the `ProjectDataU8` above, plus the
   request-profile predicate that makes the projected scalar refuse a
   non-`Active` root.

Both families' lifecycle-policy headers already flag the adjacent open question —
whether the optimistic-concurrency digest should be joined by a root-covering
`Authenticate` plan, on the Series precedent — and name their own module as
where that plan would go. This document does not settle it, and does not need
to: the tail is the same either way.

## 7. What stops being a caller parameter

The layout is only half the ABI. The other half is that two descriptor
coordinates stop being free.

| Coordinate | Today | Becomes |
|---|---|---|
| `root_state_bytes` | `RationalSelectedReleaseInputV1::root_state_bytes`, `StructuredSelectedReleaseInputV1::root_state_bytes` — caller-supplied `u32` | the tail type's own width, 16, at every call site |
| `root_schema` | caller-supplied `[u8; 32]`, in practice a lab hash | `RATIONAL_CAPABILITY_ROOT_SCHEMA_ID_V1` / `STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V1`, published beside their preimages |

Proposed preimages, in the tree's house form, using the descriptive suffix
Structured's kernel already uses so the commitment is on the wire and a widened
tail cannot silently inherit the identity:

```
b"dclutch/schema/rational-capability-root-v1|bytes16|magic-version-phase|no-family-state"
b"dclutch/schema/structured-capability-root-v1|bytes16|magic-version-phase|no-family-state"
```

This is the same removal `structured_selected_release_v1`'s own header argues
for, in its own words, about kind and capacity: "A release compiler that took
them as parameters would be offering a caller the chance to publish a Structured
capability under someone else's kind, which is exactly the hole that let a
placeholder `identity(0x10)` pass validation for months." `root_schema` is the
last placeholder of that species in either family.

Two families, two schema ids, two magics, two Lean namespaces, two decoders —
even though the layouts are byte-identical. A shared type would let one family's
root satisfy the other's decoder, and the activation outer owns no family
decoder by design, so nothing downstream would catch it.

## 8. The activation bundle, coordinate by coordinate

Both families call `build_activation_bundle_v1(ActivationBundleInputV1)` once.
Inherited coordinates are read off any one of the family's own action
descriptors and restated nowhere.

| Input field | Rational | Structured |
|---|---|---|
| `kind` | `RATIONAL_LIFECYCLE_CAPABILITY_KIND_ID_V1` | `STRUCTURED_CAPABILITY_KIND_ID_V2` |
| `config_schema` | inherited from an action descriptor | inherited |
| `root_schema` | `RATIONAL_CAPABILITY_ROOT_SCHEMA_ID_V1` (§7) | `STRUCTURED_CAPABILITY_ROOT_SCHEMA_ID_V1` (§7) |
| `derivation_policy` | inherited: SHA-256 of the empty `encode_rational_lifecycle_policy_v5()` output, pinned at `release_v1.rs`'s `derivation_policy() != lifecycle_id` | inherited: SHA-256 of the empty `encode_open_capability_lifecycle_policy_v5()` output |
| `capacity_profile` | inherited | `STRUCTURED_CAPACITY_PROFILE_ID_V2` |
| `root_state_bytes` | 16 | 16 |
| `constant_root_tail` | `rational_capability_root_creation_tail_v1()`, derived from the type — never a literal | `structured_capability_root_creation_tail_v1()` |
| `seam_fields` | `&[]` | `&[]` |
| `request_schema` | new, `b"dclutch/schema/rational-activation-request-v1"` | new, `b"dclutch/schema/structured-activation-request-v1"` |
| `funding_ledger_slot_count` | from the founding | from the founding |

The activation **selector request** follows Direct's and General's 16-byte shape:
an 8-byte magic (`DCRNACT1` / `DCSTACT1`), a `u16` version at 8, and the
selector byte at 10. Offset 10 and `SelectorWidthV2::U8` are already both
families' set geometry — Rational's from `RationalLifecycleHotLayoutV3::ACTION`,
Structured's from `REQUEST_ACTION_OFFSET`. `255` is the activation selector for
both: it is `GENERAL_ACTIVATION_SELECTOR_V4`'s value, and no action tag in either
family (Rational 0–3, open 0–4) can produce it.

The `request_schema` must differ from every action request schema. That is the
one coordinate activation does not inherit, and the reason is in
`ActivationBundleInputV1`'s own doc: reusing an action request schema would let
an ordinary action request select the activation descriptor.

## 9. The Lean emission plan

Hand-authoring a persisted byte layout is `P-007`, and `docs/OMISSION_INDEX.md`
records what it costs: a layout with "three independent authors and no
byte-identity gate". These layouts are emitted.

### 9a. Structured — extend an existing emitter and its existing guard

`formal/dclutch-semantics/DClutchSemantics/StructuredV2Abi.lean` already carries
the family's whole ABI, so the capability tail joins it. `rootSchema` is taken by
the 128-byte node root, so the new names are `capabilityRootSchema`,
`capabilityRootLayout`, `capabilityRootBytes`, `capabilityRootMagic`,
`capabilityRootSchemaPreimage`, `capabilityRootSchemaId`, and a
`CapabilityRootField` inductive.

```
def capabilityRootSchema : List (FieldSpec CapabilityRootField) := [
  ⟨.magic, .bytes 8⟩, ⟨.version, .u16⟩, ⟨.phase, .u8⟩, ⟨.reserved, .reserved 5⟩
]
```

with the four theorem families the module already proves for every other schema:
`capabilityRootBytes_is_16` (`by decide`), a `WellFormed` proof, a
`specializeFrom_pairwise` disjointness proof, and a `specializeFrom_bounded`
containment proof. `EmitStructuredV2AbiRust.lean` gains the offset loop and the
magic/preimage/id emissions; the output file
`crates/dclutch-structured-v2-kernel/src/generated_abi.rs` grows, and the guard
`crates/dclutch-structured-v2-kernel/check-generated.sh` needs its pinned line
count moved off `95`. The second guard,
`crates/dclutch-structured-v2-kernel/tests/lean_generator_fresh.rs`, needs no
change — it compares bytes, not lines.

### 9b. Rational — a new emitter, and a new guard, because it has none

Rational's existing emitter, `EmitRationalRepresentationV2PhysicalAbiRust.lean`,
writes into `dclutch-rational-representation-v2-request-contract`. The tail
belongs beside the capability kind id, in
`dclutch-rational-representation-v2-lifecycle-contract` — which does not depend
on the request contract and today has no generated file at all. So Rational needs
the whole shape, not a line:

1. `formal/dclutch-semantics/DClutchSemantics/RationalCapabilityRootAbi.lean` —
   the `FieldSpec` table above, the magic, the schema preimage and id, and the
   same four theorems.
2. `formal/dclutch-semantics/EmitRationalCapabilityRootAbiRust.lean`.
3. A `[[lean_exe]]` root in `formal/dclutch-semantics/lakefile.toml`.
4. `crates/dclutch-rational-representation-v2-lifecycle-contract/src/generated.rs`.
5. A guard. Either shape qualifies under
   `tools/emission-guard/COVERAGE.md`'s definition; the cheaper is a
   `tests/generator_fresh.rs` in the crate, modelled line for line on the
   request contract's, which needs no `rustfmt` and no pinned line count.
6. `tools/emission-guard/emission_guard.py --write`, then `--verify`. The census
   is itself byte-gated, so a new emission that arrives unguarded reds the gate
   until someone decides on purpose which it is going to be. Landing this
   unguarded would move the tree from 59 guarded / 15 unguarded to 59 / 16 and
   would be exactly the drift the census exists to make visible.

### 9c. What the Rust side may then hold

Only what an emitter cannot: the decoder, the phase enum, the creation oracle,
and the published composite-offset helper. Specifically, per family — a
`…CapabilityRootLayoutV1` of `pub const` offsets that read straight through to
`generated::*`, a `…CapabilityRootPhaseV1`, a `…CapabilityRootV1` with
`decode` / `encode` / `new`, and a
`…_capability_root_creation_tail_v1()` oracle in the style of
`general_root_creation_tail_v2` — required to be byte-identical to
`…CapabilityRootV1::active().encode()`, so a layout change that missed the
activation artifact fails at the oracle rather than on a deployed root.

## 10. What changes downstream, and what it costs

The tail is the cheap half. Adding the activation entry is the expensive half,
and the expense is the same for both families and is not the entry.

**The publication widens, and the release identity moves with it.** Rational's
`RationalSelectedPublicationV1` gains one identity: 12 → 13, and
`RATIONAL_SELECTED_PUBLICATION_BYTES_V1` 420 → 452. Structured's gains one:
13 → 14, 452 → 484. The publication digest is the `release_id` a founded
Market's capability manifest names, and the manifest digest is a **seed of the
Market PDA**. So this changes every Rational and Structured Market address.

That is a migration, not a patch — and it is free exactly once. Today both
families exist only in the local-validator lab (`demo_rational_market_input`,
`demo_structured_market_input`), whose root schema and width are labelled lab
facts. **The window in which this costs nothing is open now and closes the first
time a Rational or Structured market is founded anywhere that matters.** That,
more than anything in §3, is the argument for doing it in this order.

**The set builders relax two assertions, in different places.**

- Rational: `selected_set_v6.rs`'s entry-count check and `release_v1.rs`'s
  authenticator loop, which today requires *every* entry to decode as a
  `CapabilityProgramV4` with a Rational-action request schema. It must be split
  into "four action entries plus one activation coordinate", exactly as
  `release_v3.rs` does for General. Three more publication records —
  `activation-account-profile`, `activation-effect`, `activation-descriptor`,
  and three not four, because `CapabilityProgramV1` carries the activation
  transition inside the descriptor.
- Structured: the same split against `open_release_v1.rs`'s blanket-V4 stamp and
  its entry-count check, plus `open_capability_set_v3.rs`'s expected-entry array
  and `structured_selected_release_v1.rs`'s publication count. **The set builder
  is shared**: `open_release_v1.rs`'s header says one builder serves Bearer and
  Structured. Today `structured_selected_release_v1` is its only release-compiler
  consumer, so a `GeneralReleaseProfileV1`-shaped profile parameter costs
  nothing now and costs a Bearer migration later. Take the profile.

Neither family may relax the stamp to "accept V4 anywhere". The template rules
that out on two independent grounds, the second of which is that a V4 descriptor
could not activate anything even if it decoded.

**Sizes, as estimates with numbers.** Per family: one Lean module and emitter
(Rational ~250 lines plus a lakefile root and a guard test; Structured ~120 lines
into an existing module, plus a pinned line count); one root type with layout,
phase and creation oracle (~200 lines and its tests); one
`activation_bundle_v1` module modelled on
`dclutch-general-adapter-contract`'s 818 lines and
`dclutch-direct-codec`'s 893, but smaller — call it ~600 — because zero seam
fields removes the field-declaration surface and the whole
`RuntimeVaryingByteNotDeclared` completeness check; ~250 lines across three files
for the set entry, the authenticator split and the publication; and the
`root_state_bytes` / `root_schema` parameter removal at roughly a dozen sites,
most of them tests. Then the template's step 6 — publish through the family's
release closure and prove the activation on program-test before any cluster —
which is a separate lane and the only one that can claim a root was created.

## 11. This lane did not implement, and why

The charter allowed implementation if the design were so constrained it wrote
itself. It is nearly that constrained, and implementing it here would still be
wrong, for one specific reason: **the only thing this lane could write by hand is
the layout, and writing the layout by hand is the defect the design exists to
prevent.** A `generated.rs` committed without running `lake env lean --run` is
`P-007` with a fresher timestamp, and an honest run of the Rational emitter needs
a new lake root, a `lake build` and the emission-guard census regenerated —
neither budgeted here nor safe to do in a working tree several lanes are sharing.

The second reason is smaller but real: §10's publication widening moves every
Market address for both families. That is a release-owning decision, and the
right lane to make it is the one that also runs the founding.

What a follow-on lane inherits is complete: two byte tables, two schema
preimages, two magics, an empty `seam_fields` in both, the exact
`ActivationBundleInputV1` for each, the emitter and guard shape per family, and
the four assertion sites that must relax.

### 11a. Re-verified STILL TRUE at HEAD, 2026-08-31 (LEDGER-TRUE)

A readiness-wave report claimed the family root-tail ABIs had landed. **They
have not, and this section needs no correction.** Recorded here because the
claim will be made again, and because "ROOTTAILS landed" is true of the *design*
and false of the ABI — the word attaches to this document, not to any bytes.

Measured at HEAD of `main`:

- **No code exists for either symbol.** `RationalCapabilityRoot`,
  `StructuredCapabilityRoot`, and the magics `DCRNCRT1` / `DCSTCRT1` have
  **zero hits tree-wide under `--type rust`**. They occur only in this file and
  in `docs/ledger/SESSION_STATE_2026-08-31.md:579`. There is no struct, no offset constant, no size
  constant, no decoder — nothing to wire in, so the question of whether it is
  wired in does not arise.
- **Both commits are doc-only.** `ec530892` (2026-08-30 22:56, *"design: the
  two capability-root tails Rational and Structured never had"*) touches this
  file and nothing else, +474 lines; `97d0f435` (22:57) touches
  `docs/ledger/SESSION_STATE_2026-08-31.md` and nothing else. Both are ancestors of `main`. A pickaxe
  on `DCRNCRT1` across all refs returns `ec530892` alone.
- **The §9b negative control did not move.** §9b step 6 predicted that landing
  the Rational emitter would take the emission census from 59 guarded / 15
  unguarded to 59 / 16. `tools/emission-guard/COVERAGE.md:6` still reads *"74
  generated files from 72 emitters. 59 guarded (57 emitters), 15 unguarded (15
  emitters)."* That file is byte-gated, so it could not have stayed still
  through a landing. Neither emitter file
  (`EmitRationalCapabilityRootAbiRust.lean`, the `RationalCapabilityRootAbi`
  module) exists, and `dclutch-rational-representation-v2-lifecycle-contract`
  has no `generated.rs`.
- **The pre-state §1 measured is intact.** `root_state_bytes` is still an
  unauthored literal at all four sites — `64` at
  `tools/local-validator/bootstrap/successor/src/rational_market.rs:201,230`
  and `8` at `.../structured_market.rs:209,237` — still inside the "LAB FACTS,
  labeled" demo functions, still constrained by nothing but
  `CapabilityProgramV4`'s `!= 0 && <= 4096`.

**What the report probably saw.** Two adjacent things did land on 2026-08-30
and are easy to mistake for this: **General's** activation bundle and
publication closure (WALL22 / GENPUB), which are real code at
`crates/dclutch-general-adapter-contract/src/activation_bundle_v1.rs` — and
only Direct and General have an `activation_bundle_v1.rs` at all. The evidence
record for that very work says the opposite of the wave's claim in plain text:
Rational and Structured *"still have no capability root tail; Fractional's
remains impossible"* (`docs/evidence/GENERAL_PUBLICATION_CLOSURE_2026_08_30.md:248`).

## 12. What this does not settle

Nothing here needs a ruling; the packet already ruled the process. Three things
are deliberately left open, and each is left where its own module already flags
it.

- **The root-covering `Authenticate` plan.** Both lifecycle policies name the
  Series precedent and their own module as where it would go. The tail is
  unchanged either way.
- **Whether the optimistic-concurrency digest should ever be more than an
  identity binding.** `hash(root_data)` today serializes nothing, because nothing
  mutates the account after activation — for either family, this was true before
  this document and remains true after it. A revision counter would fix it and
  is deliberately *not* in the tail, because a counter with no writer is the
  decorative artifact §6 refuses. When a writer exists, the counter arrives with
  it, in a V2 tail for new roots.
- **Bearer.** It shares Structured's set builder and has no release compiler yet.
  The profile parameter in §10 is what keeps that true for free.
