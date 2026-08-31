# Release-set lineage and market migration, v1

Status: **design, chartered by ember 2026-08-30 as the fix for liveness-census
R1 / queue Q1.** Not an ADR, not a release manifest, not deployment evidence.
It closes no `docs/OMISSION_INDEX.md` row by itself; it specifies the work that
would.

Every claim carries **verified-from-source** (read in this repository at HEAD,
with `file:line`) or **design** (a decision this document is making). Where a
fact is asserted without a `file:line`, it is a design choice, not a finding.

Audience: the lane that implements this. The document is written so that lane
makes no judgment calls — wire layouts, seed tuples, account frames, refusal
codes and conjuncts are specified, not gestured at. Where a genuine choice
remains, it is called out as **OPEN** with the decision rule attached.

---

## 0. What this document decides

1. A release set names its predecessor in **one record, keyed by the
   predecessor**, authored by the party that can cause the supersession — the
   role programs' upgrade authority (§4).
2. Migrating a market along that lineage is **permissionless, forward-only by
   structure, and admissible in every market phase** — including retirement,
   which is the whole point (§5).
3. A lineage hop **may not change any role's program id**. That single conjunct
   is what makes migration a one-field write instead of a re-derivation of the
   market (§6).
4. Migration **never reads the superseded activation cache**, so the brick
   cannot defend itself (§5.3).
5. Core becomes the **single author of a per-release-set census of live
   subjects**, which is simultaneously the funding size for migration and the
   predicate that makes activation-cache closure (B22/Q8c) safe (§8, §9).
6. Founding on a release set that already has a declared successor is
   **refused** — a market is never born needing a migration (§7.4).

It does **not** decide: whether to keep `ExactAuthority` on production
deployments (decision 0012 owns that), the Dealer family's own liveness gaps
(Q4), or anything about Q3's claim-check compaction beyond noting where the two
designs touch (§9.4).

---

## 1. The problem, stated exactly

### 1.1 The three facts that compose into a permanent strand

**Fact 1 — the activation cache is keyed by release-set content id.** The cache
PDA is derived under a domain plus the release-set id, so a new release set
mints a *new account*; the old one is never updated in place
(`programs/dclutch-registry-sbf/src/lib.rs:513`).

**Fact 2 — a market's release-set pin is write-once.**
`state.identity.selected_release_set` is written at founding and compared
everywhere else; there is no route in the tree that rewrites it.

**Fact 3 — the pin is enforced on the exit path too.** Core `Retire`
authenticates roles against the market's pinned cache on both legs
(`programs/dclutch-core-sbf/src/retire_v1.rs:488,732`), as do Claims market
closure and Trading `BeginRetiring`.

**The trigger.** Decision 0012 made the devnet substrate mutable and replaced
immutability with a slot pin: an `ExactAuthority` release whose ProgramData
slot has moved is refused as `ReleaseSupersededByUpgrade`
(`crates/dclutch-registry-contract/src/artifact.rs:273`), and deployments
retain `ExactAuthority` whenever an upgrade authority exists
(`tools/local-validator/bootstrap/successor/src/plan.rs:1422`).

**The composition.** One `Upgrade` instruction on one role program moves that
program's ProgramData slot. Every market pinned to a release set containing
that program now fails authentication on *every* route — including
`Retire`. The market cannot trade, cannot resolve, cannot retire, and cannot
re-point, because re-pointing is not a route that exists. Value is stranded
permanently, by design, on the ordinary act of shipping a fix.

### 1.2 Decision 0012 conceded exactly this, in one clause

The record is explicit about the protection it bought and honest about its
edge (`docs/decisions/0012-devnet-iteration-substrate.md`):

> What this does NOT protect, named plainly: the upgrade authority (ours) can
> still ship new bytes. The protection is that **every open market refuses the
> instant it happens** — the slot moved, so every cached authentication returns
> `DeploymentSlotMismatch` until a new release generation is published,
> activated, and **new markets founded on it**.

*New markets.* The clause that closes the sentence is the gap: 0012 provides a
recovery for the substrate and none for the markets already on it. Core's own
refusal doc-comment narrates it as a stall —

> Every open market on the superseded release generation refuses until a
> re-release re-authenticates the new deployment and re-pins its slot.
> (`programs/dclutch-core-sbf/src/lib.rs`, `ReleaseSuperseded = 0x3010`)

— but the routes make it permanent, because "re-pins its slot" describes a
*new* cache that the *old* markets have no way to point at.

### 1.3 Why this is the charter's problem and not an operational one

The protocol's one-sentence differentiator is *no liveness dependency on any
identified party*. R1 is the sharpest violation in the census: not a party who
must act and might not, but a party whose ordinary, expected, correct action
(shipping an upgrade) **destroys** the exits of every live market, with no
recovery available to anyone at any price.

---

## 2. The ruled constraints

**Ember, 2026-08-30 ~10:20 EDT, binding** (`WAVE.md`):

> Q1: no superseded-cache carve-out EVER (option b rejected outright); devnet
> stranding accepted meanwhile; the real fix is design (a) — release-set
> lineage/re-point migration — chartered as a proper design. Cohort-7 may
> proceed.

Three constraints follow, and this document is bound by all three.

**C1 — no superseded-cache carve-out, ever.** No route, however sympathetic
its purpose, may authenticate against a cache whose slot pin is broken. In
particular the census's own "smallest" option — exempting
`retire_v1`/`begin_retiring`/`CloseFund` from the slot pin so the exit path
reads the deployed ELF without the `>` check — is **rejected outright and is
not a fallback if this design proves expensive.** A carve-out would mean the
retirement path alone runs against unauthenticated code, which is precisely the
path where the money moves.

**C2 — the devnet strand is accepted today.** This design does not need to
land before the current devnet cut, and no lane should destabilise cohort-7 to
accelerate it. Devnet is recycled freely per the wave's standing ruling.

**C3 — a proper design first.** Direction (a) is chartered as a design, which
is why this document exists before any code.

A fourth constraint comes from the day's defect taxonomy rather than the
ruling, and it governs every structural choice below.

**C4 — one fact, one author.** `WAVE.md`, distilled doctrine 3: *"One fact, one
author — and a guard's other side is a different author."* The 12:30 ruling on
Series states the corollaries this design uses directly: *"(b) the rent-quote
generation pin is DERIVED at emit time from the publication context the release
set already binds — never supplied, never a second copy. (c) the refund
recipient is a RULE, never an identity: the beneficiary fixed at state
creation."*

`SEAM_AUDIT_2026_08_29.md` supplies the failure mode C4 exists to prevent —
nine always-refuses routes and one always-admits across six seams, **none of
which had a failing test**, every one of them a place where two sides
independently authored one identity and drifted. A migration route is a new
seam between Core (owns markets), Registry (owns release identity) and the
Loader (owns upgrade authority). It will be audited by that method, so it is
designed against it.

---

## 3. Ground truth: five facts that decide the design

These were established by reading the routes at HEAD. **Three of them
contradict the charter's premises**, and the design is built on the corrected
versions.

### 3.1 The release set is a Market PDA **seed**, not merely a stored pin

This is the fact that decides everything else.

`crates/dclutch-market-core-codec/src/physical.rs:646-661`:

```rust
    /// Return the sole ordered PDA seed projection, excluding the bump.
    #[must_use]
    pub fn as_slices(&self) -> [&[u8]; 9] {
        [
            crate::MARKET_CORE_STATE_PDA_DOMAIN_V2.as_slice(),
            &self.realm,
            &self.product_record,
            &self.product_id,
            &self.resolution_policy,
            &self.capability_manifest,
            &self.release_set,
            &self.registry_program,
            &self.generation,
        ]
    }
```

with the projection at `physical.rs:641`:

```rust
            release_set: identity.selected_release_set.to_bytes(),
```

The release set is **seed component 6 of 9**. The derived address is then
stored back into the state as `market_id` (`programs/dclutch-core-sbf/src/found.rs:393`)
and re-derived and re-compared at eight sites, including across the program
boundary — `found.rs:407-408`, `capability.rs:797-800`, `fixed_role.rs:353-354`,
`retirement_replay_handoff_v1.rs:105-106`, `generic_founding_v1.rs:1401-1402`,
`retire_v1.rs:1686-1691`, `series_open.rs:392`, and Claims'
`market_closure_v1.rs:581-585`.

**Therefore an in-place rewrite of `selected_release_set` does not migrate a
market — it destroys it.** The account's stored `market_id` would stop equalling
`find_program_address(seeds(identity))` and all eight checks would refuse. This
is the trap any lane approaching R1 from the census text alone will fall into,
because the census describes the field as a pin and not as a name.

It is worse than one account. Seven child PDA domains seed on the release set
as well, and one of them holds the money:

| child | seed tuple | file:line |
|---|---|---|
| **Custody vault (the Hoard — market collateral)** | `[CUSTODY_VAULT_PDA_DOMAIN_V1, market, release_set, context, compartment]` | `crates/dclutch-custody-contract/src/lib.rs:389-397` |
| Custody transfer authority | `[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, market, release_set]` | `crates/dclutch-custody-contract/src/lib.rs:256-262` |
| Custody replay cursor | `[CUSTODY_REPLAY_PDA_DOMAIN_V1, market, release_set, caller_role, context]` | `crates/dclutch-custody-contract/src/lib.rs:320-330` |
| Projected-custody caller | `[PROJECTED_CUSTODY_CALLER_PDA_DOMAIN_V1, release_set, market, parent_capability_root, context_digest, request_digest]` | `crates/dclutch-custody-contract/src/projected.rs:282-292` |
| Core-effect caller authority | `[CALLER_AUTHORITY_PDA_DOMAIN_V1, release_set, market, caller_role, context, role_request_digest]` | `crates/dclutch-release-set-contract/src/lib.rs:302-311` |
| Rational-representation V2 state | `[…, graph_digest, market, release_set]` | `crates/dclutch-rational-representation-v2-contract/src/seeds.rs:37-49` |
| Structured V2 state | includes `release_set` | `crates/dclutch-structured-v2-contract/src/seeds.rs:210,240` |

A naive re-point moves the market's address *and* every one of those, orphaning
the collateral in a vault nothing can name any more.

**The diagnosis this yields.** Decision 0012 made the deployment mutable. It
did not, and could not, make the *release-set id* mutable — because that id was
already welded into the names of the market and its children. So the real
defect behind R1 is not a missing route. It is that **a fact which decision
0012 turned into a moving one is committed inside an address that can never
move.** The fix is to stop asking one field to be both the market's name and
the market's current execution binding.

### 3.2 The capability manifest is orthogonal — the one degree of freedom

`capability_manifest` is seed component 5, and it is a plain content digest of
the manifest record's bytes with **no release-set input**
(`programs/dclutch-core-sbf/src/records.rs:56-67`, called for the manifest at
`found.rs:591-598` and `:656-663`). Seed components 5 and 6 are independent.

So the charter's phrasing — *"the manifest is sealed into the Market PDA
identity, so migration must NOT change identity"* — is right in its conclusion
and wrong in its reason. Migration must not change identity, but the manifest
is not what makes that so; the release set's own presence in the seeds is. And
because the manifest is independent, a successor release set can be adopted
without disturbing it at all.

### 3.3 Role programs are already re-derived every transaction

`CoreState` is 360 bytes and caches **no** role program ids, **no** capability
entries, and **no** frame specs (`crates/dclutch-market-core-codec/src/generated.rs:355-366`;
the commentary at `physical.rs:665-668` is explicit — *"None of these
child-owned facts is cached in the sparse Core state."*). Every role program is
re-derived per transaction from the activation cache
(`programs/dclutch-core-sbf/src/release.rs:127-131`).

The Lean model demands it, `formal/dclutch-semantics/DClutchSemantics/MarketCore.lean:594-596`:

```lean
/-! Retirement uses one coherent release-set observation.  Physical adapters
may authenticate its four required roles serially, but they cannot substitute
role-specific copies of the selected ReleaseSet. -/
```

**This is the design's good news.** Dispatch needs no migration whatsoever. If
the market can be made to name a different cache, everything downstream of the
cache follows for free.

### 3.4 Correction: `authenticate_role_semantic_release` does not exist

The charter names it as an existing on-chain refusal on `semantic_release_id`.
A tree-wide grep across all file types returns **zero hits**. What exists:

- `programs/dclutch-core-sbf/src/release.rs:349-355` compares a semantic release
  id — but both sides come from `release_projection` over the **same** cache
  account. It is a self-consistency check between two readings of one account,
  not a pin against an independent expectation.
- `programs/dclutch-resolution-proof-sbf/src/lib.rs:670` compares a semantic id
  against a constant — and sits inside `#[cfg(any())]` at `lib.rs:658`, so it
  never compiles.

Nothing in this design may cite either as an existing gate, and the
`semantic_release_id` field is used below only for what it verifiably is: a
216-byte-record field at offset 112 (`crates/dclutch-registry-contract/src/artifact.rs:36`)
that is **publisher-supplied rather than chain-observed** (§4.3).

### 3.5 Neither existing record has a spare byte, and the Registry has no spare action

- `ExecutionReleaseSetV1` is exactly 336 bytes — 16-byte header (magic
  `DCLTRLS1`, schema u16, profile u16, 4 reserved bytes) plus five 64-byte
  bindings (`crates/dclutch-release-set-contract/src/lib.rs:54-61`). The
  reserved run is `require_zero`-enforced on decode (`lib.rs:637`).
- `ArtifactReleaseV1` is exactly 216 bytes, fully packed to
  `UPGRADE_AUTHORITY_OFFSET = 184` + 32 (`artifact.rs:12,38`).
- The activation cache is exactly 1288 bytes = `48 + 5 × 248`, whose only spare
  header space is 4 `require_zero` bytes at `[12,16)`
  (`crates/dclutch-registry-contract/src/activation.rs:37-42`), and
  `require_cache_account` hard-compares `data_len() != 1288`
  (`crates/dclutch-registry-activation-auth-v1/src/lib.rs:201-210`).
- `RegistryInstructionV1` admits only actions `0` and `1`
  (`crates/dclutch-registry-svm/src/lib.rs:117-134`) because its magic
  `DCLTRIX1` is shared byte-for-byte with the record family, which owns every
  action ≥ 2 and is routed away first (`programs/dclutch-registry-sbf/src/lib.rs:165-174`).
  The doc at `registry-svm/src/lib.rs:101-108` says so outright: *"the Registry
  side of that split has exactly actions `0` and `1` to spend."*

**Consequences, both load-bearing.** The lineage fact cannot be a field of
either existing record without a schema-2 wire change that re-hashes every
release-set id in existence — so it must be its own account (§4.1). And a new
Registry instruction costs a new 8-byte magic plus a fourth sub-dispatcher
branch, exactly as the three existing sub-dispatchers do — not a new enum
variant (§4.4).

---

## 4. The design, part one: lineage

### 4.1 Where the predecessor link lives, and why

**Decision: a separate Registry-owned record, `ReleaseLineageV1`, whose PDA is
keyed by the PREDECESSOR release-set id.**

Three independent arguments converge on this shape, and the third is the one
that makes it inevitable.

1. **Neither existing record can hold it** (§3.5). A `predecessor` field inside
   `ExecutionReleaseSetV1` is a schema-2 wire change to the type that is *"the
   sole semantic owner of execution-role membership"*
   (`release-set-contract/src/lib.rs:587-593`) — and lineage is not membership.
2. **Lineage is a fact about a pair, so it cannot be a field of one member
   without that member restating the other.** C4 forbids exactly this.
3. **The predecessor's record is already published and immutable.** Release-set
   records are content-addressed and finalized
   (`programs/dclutch-registry-sbf/src/lib.rs:449-482`); a predecessor cannot
   later grow a pointer to a successor that did not exist when it was hashed.
   And the successor cannot be trusted to name its own predecessor, because the
   authority that must consent is the *predecessor's* (§4.3). The link therefore
   has no home inside either endpoint. It needs a third account.

**Keyed by the predecessor**, for two reasons that both pay:

- **Write-once-per-predecessor comes free.** A second `DeclareSuccessor(A→C)`
  finds the account already exists and refuses at account creation. There is no
  lineage fork, and no code is needed to forbid one.
- **The lookup direction is the one migration needs.** A migrating caller knows
  the market's current set `A` and nothing else; it derives `PDA(A)` and *reads*
  the successor out. The successor is never supplied on the wire, so there is
  nothing to supply wrongly — doctrine (b), derived at read time from the
  context the market already carries.

### 4.2 `ReleaseLineageV1` — exact layout

Domain seed, new:

```rust
/// First PDA seed for a release-set lineage record, keyed by PREDECESSOR.
///
/// Derived under the Registry program with exactly
/// `[RELEASE_LINEAGE_PDA_DOMAIN_V1, predecessor_release_set_id]`, in that
/// order, and no caller-selected seed.
pub const RELEASE_LINEAGE_PDA_DOMAIN_V1: &[u8; 26] = b"dclutch:release-lineage:v1";
```

Seeds: `[RELEASE_LINEAGE_PDA_DOMAIN_V1, predecessor.as_bytes()]` under the
Registry program id. Exactly the two-seed shape of the activation cache
(`registry-sbf/src/lib.rs:513-517`), so the derivation is already a known
pattern to every reader.

Account, **exactly 248 bytes**, magic `DCLTRLN1`:

| range | bytes | field |
|---|---|---|
| `[0,8)` | 8 | magic `DCLTRLN1` |
| `[8,10)` | 2 | schema version u16 LE = 1 |
| `[10,12)` | 2 | profile u16 LE = 1 |
| `[12,16)` | 4 | reserved, `require_zero` |
| `[16,48)` | 32 | `predecessor_release_set_id` (`ContentId`, nonzero) |
| `[48,80)` | 32 | `successor_release_set_id` (`ContentId`, nonzero) |
| `[80,85)` | 5 | `moved_roles` — one byte per role in canonical order, `1` if that role's artifact release changed across the hop, `0` if it did not |
| `[85,88)` | 3 | reserved, `require_zero` |
| `[88,248)` | 160 | `consenting_authority[5]` — the 32-byte upgrade authority that signed for each role, or 32 zero bytes for a role that did not move |

Fixed layout, no options, no variable-length anything, in the house style of
`ArtifactReleaseV1` (216) and the activation cache (1288).

Role order is the existing canonical one — Core, Claims, Trading, Resolution,
Custody — pinned at `crates/dclutch-registry-contract/src/activation.rs:521-533`.
Reuse `ExecutionRoleV1::role_index`; do not restate the order.

**Two fields a first draft would add, and why they are absent.** A `generation`
ordinal cannot be derived without the grandparent lineage record, whose address
is *not* derivable from `A` (the record is keyed by predecessor, so finding
`A`'s predecessor means already knowing it) — and two lineages may legitimately
converge on one successor, which makes the ordinal ambiguous anyway. A
`declared_at_slot` stamp would cost the Clock sysvar account. **Neither is read
by any conjunct in this design**, and a field no conjunct reads is a fact with
no author to hold responsible for it. Both are omitted. Forward-only ordering
is supplied by the data structure (§5.4), not by an ordinal.

`moved_roles` and `consenting_authority`, by contrast, *are* read: they are the
record of who consented to what, and they are what an auditor checks a
declaration against after the fact. They are derived at write time from the two
endpoints and the signer set, never supplied.

### 4.3 Who authors it: the upgrade authority of every role that moved

**The whole authorization requirement of this design reduces to one field.**

Consider what an attacker could put in a forged successor set `B'` that names
the same five role programs as `A`. Every other field of every
`ArtifactReleaseV1` in `B'` is forced by observation at activation time:
`activate_and_write_role` re-hashes the complete live ELF
(`programs/dclutch-registry-sbf/src/lib.rs:442`, whose doc at `:395-400` says
it *"must never be replaced by a cached-digest fast path"*), and
`authenticate_artifact_role` requires `release.program() == expected.program()`
(`lib.rs:328-363`). A release claiming `Immutable` over an authority-bearing
ProgramData is refused (decision 0012, *"What stays refused, deliberately"*).
So `programdata`, `elf_digest`, `deployment_slot`, `upgrade_policy` and
`upgrade_authority` are all pinned to the truth, and a `B'` that lies about any
of them **cannot be activated at all**.

The exception is `semantic_release_id`. It is source-derived and
publisher-supplied — the tool computes it as
`checked_semantic_release_id(role, source_revision)`
(`tools/local-validator/bootstrap/successor/src/upgrade.rs:3178`) — and nothing
on chain can check it against anything. It is also load-bearing downstream: a
capability seal key joins on it
(`crates/dclutch-capability-seal-contract/src/lib.rs:612`).

So: **the one thing a forged successor can choose freely is the one thing that
needs consent.** Hence the conjunct.

**Rule.** For each role `i`, let

```text
moved_i  ≡  A.binding(i).artifact_release != B.binding(i).artifact_release
```

`DeclareSuccessor(A→B)` requires, for every `i` with `moved_i`, a signature from
the key that `B`'s activated artifact for role `i` binds as its
`upgrade_authority` — read out of `B`'s activation cache at role slot `i`,
offset `+184` within the embedded 216-byte `ArtifactReleaseV1`
(`artifact.rs:38`). For every `i` with `!moved_i`, no signature is required and
no consent is being asked for, because that role's binding is byte-identical on
both sides and therefore makes no new claim.

An `Immutable` role has `upgrade_authority == None`. An immutable deployment
cannot be upgraded, so `moved_i && authority == None` is a contradiction and
refuses.

### 4.4 The supersession symmetry, and what it does and does not buy

**Theorem (informal).** The coalition of keys able to strand a market on set `A`
is exactly the coalition able to author `A`'s successor.

*Why.* Stranding a market on `A` requires moving some role program's
ProgramData slot past `A`'s pin, which under Loader V3 requires that program's
upgrade authority (`registry-contract/src/artifact.rs:262-282`; the doc there
records that the Loader also refuses an `Upgrade` in the ProgramData's own
recorded slot, and the `Close` a redeploy would need, so the same-slot corner is
closed by the loader itself). Authoring `A→B` requires the upgrade authority of
exactly the roles whose artifacts moved — which is exactly the set of roles that
were upgraded. So the act that creates the hazard and the act that authors the
remedy require the same signatures, and **a set that nobody can supersede is a
set for which nobody needs a successor.** An all-`Immutable` release set can
neither strand nor migrate, and both facts vanish together.

**What it buys.** No new trust. A market that migrates `A→B` ends up bound to
the same five role programs it was founded on — role identity invariance is
enforced at declaration (§4.5, conjunct 4) — and the code behind those programs
was always the upgrade authority's to change. Migration converts a permanent
brick into a recorded, authorized, one-step state transition and changes nothing
about who the market trusts.

**What it does not buy, named plainly.** Migration does not, and cannot, protect
a market from its role programs' upgrade authority. That exposure exists at HEAD,
decision 0012 states it (*"the upgrade authority (ours) can still ship new
bytes"*), and this design leaves it exactly where it is. What this design
removes is the brick, not the authority.

**The residual, also named plainly.** The authority can decline to declare a
successor, leaving markets stranded. This is not fixable on chain — no
instruction can compel a signature — and it is bounded by the same symmetry: an
authority that never upgrades never strands anyone. The mitigation is
procedural and belongs in the release tool: a deployment plan that upgrades a
role must emit the declaration in the same plan (§11 commit 6).

### 4.5 `Registry::DeclareSuccessor` — the route

New 8-byte magic `DCLRLND1` and a **fourth sub-dispatcher branch** in
`programs/dclutch-registry-sbf/src/lib.rs:process_instruction`, placed after the
existing three (hot continuation `DCLTHOT3`, record `DCLTRIX1`, registry
continuation `DCLRGCI1`) and before the typed `RegistryInstructionV1::decode`
tail. It must not be a new `RegistryInstructionV1` variant — there is no action
byte left (§3.5).

Wire, **16 bytes**, mirroring `REGISTRY_INSTRUCTION_BYTES_V1`:

| range | bytes | field |
|---|---|---|
| `[0,8)` | 8 | magic `DCLRLND1` |
| `[8,10)` | 2 | schema u16 LE = 1 |
| `[10,16)` | 6 | reserved, `require_zero` |

The instruction carries no arguments at all. Both endpoints, the moved-role
mask and every authority are **derived from the accounts** — there is nothing on
the wire to disagree with the chain.

Account frame, **exactly 11**, fixed order:

| # | account | owner | privileges | authenticated by |
|---|---|---|---|---|
| 0 | payer | System | signer, writable | signs; funds the lineage record |
| 1 | lineage record | System → Registry | writable, non-signer | address `== PDA([RELEASE_LINEAGE_PDA_DOMAIN_V1, A], registry)`; must be **pristine**: System-owned, zero lamports, zero data (the record-family vacancy pattern, `registry-sbf/src/lib.rs:449-482`) |
| 2 | **predecessor** activation cache | Registry | readonly | `require_cache_account`; `ActivatedExecutionReleaseSetViewV1::decode` succeeds. Yields `A` and `A`'s five bindings and slots |
| 3 | **successor** activation cache | Registry | readonly | same; yields `B`, `B`'s bindings, slots and upgrade authorities |
| 4..8 | `authority[0..5]`, one per role in canonical order | any | readonly; **signer iff `moved_i`** | if `moved_i`: `key == B.role(i).release().upgrade_authority()`, which must be `Some`. If `!moved_i`: `key == system_program::ID` and **not** a signer |
| 9 | System | native loader | readonly, executable | `authenticate_rent_and_system` (`registry-sbf/src/lib.rs:621-633`) |
| 10 | Rent sysvar | sysvar | readonly | same helper |

Neither endpoint is named on the wire. `A` and `B` are read out of accounts 2
and 3 as `execution_release_set_id()`, and the lineage PDA is then derived from
`A`. There is nothing supplied that could disagree with the chain.

**Why the predecessor's *cache* and not its record.** The cache is a
digest-consistent projection of the record — `require_consistent_completion`
(`registry-sbf/src/lib.rs:252-272`) refuses unless the decoded cache's
`release_set_projection()` equals the finalized record's, so reading the cache
is reading the record. It also costs two accounts instead of four (no raw +
staging pair), and it carries the `deployment_slot` values that the record does
not, which conjunct 5 needs.

**This is not a carve-out, and the distinction matters.** C1 forbids
*authenticating against* a superseded cache — admitting a role, and therefore a
privilege, from a cache whose slot pin is broken. Account 2 admits no role and
confers no privilege. It is read only as the source of `A`'s own bindings and
bound slots, for comparisons that can **only refuse**, never admit. A check
that cannot admit anything cannot be an exemption from a check.

**Conjuncts, in order (refusal codes in §8).**

1. Frame width is 11 and privileges are exactly as tabled.
2. Accounts 2 and 3 are decodable, Registry-owned activation caches at their own
   derived addresses; `A` and `B` are their `execution_release_set_id()`s.
   **Decoding proves all five roles are activated** — a partially written cache
   cannot decode (`activation.rs:309-316`) — so this one check is also the
   guarantee that a migrated market lands somewhere immediately operable.
3. **Not self-succession**: `A != B`.
4. **Role identity invariance**: for every `i`,
   `A.role(i).release().program() == B.role(i).release().program()`.
5. **Forward-only**: for every `i` with `moved_i`,
   `B.role(i).release().deployment_slot() > A.role(i).release().deployment_slot()`.
   Under Loader V3 a ProgramData slot only moves forward, and the Loader refuses
   an `Upgrade` within the ProgramData's own recorded slot and refuses the
   `Close` a redeploy would need — the reasoning recorded at
   `crates/dclutch-registry-contract/src/artifact.rs:262-282`. So "strictly
   greater slot" is exactly "was upgraded after", and a backward or sideways
   declaration cannot be built. An unmoved role needs no slot check: an
   identical artifact release id is an identical 216-byte record, slot included.
6. For every `i`: `moved_i` derived per §4.3, and account `4+i` satisfies the
   signer/identity rule tabled above.
7. Account 1 is pristine; create it at 248 bytes, rent-exempt, Registry-owned,
   via `invoke_signed` with `[RELEASE_LINEAGE_PDA_DOMAIN_V1, A, [bump]]` — the
   activation cache's creation shape (`registry-sbf/src/lib.rs:532-570`).
8. Write the record; post-check that it decodes and that its
   `predecessor`/`successor` equal the values just computed — the
   `require_consistent_completion` belt.

**A second declaration for the same predecessor cannot be made**, because
account 1 will not be pristine. That is the whole of the no-fork guarantee; no
code implements it.

**There is no rollback, and none is needed.** A botched successor `B` is
corrected by declaring `B→C` and letting markets walk `A→B→C`. Forward-only is
not a policy enforced against a temptation; after conjunct 5 it is the only
direction the data structure has.

---

## 5. The design, part two: the re-point route

### 5.1 The problem the route must solve, restated after §3.1

The market's release-set pin is its name. It cannot be rewritten. So the route
cannot rewrite it — and the design's whole job is to make that a non-problem.

**Decision: split the one field into the two facts it has been carrying.**

- `identity.selected_release_set` — **unchanged in every respect.** The set the
  market was founded on. Immutable, address-committing, at offset 208, still
  seed component 6 of 9. Nothing in this design writes it after founding.
- `active_release_set` — **new.** The set the market currently authenticates its
  roles against. Mutable, outside `MarketIdentity`, therefore structurally
  incapable of reaching the seed projection, which reads only from
  `MarketIdentity` (`physical.rs:614-644`).

Initialized equal at founding. Thereafter `active` walks forward along the
lineage and `selected` never moves.

**Why this placement is the design and not a workaround.** Two genuinely
different facts were sharing one field: *what this market is called* and *what
code it runs against*. Decision 0012 made the second one mutable and left it
welded to the first. Giving each fact its own field, with its own author —
founding writes `selected`, migration writes `active` — is C4 applied to the
exact place the defect came from.

**And it is what keeps every stored copy correct forever.** The market's
children store, compare against, and seed on the *founding* set: the capability
root header's `release_set` (`crates/dclutch-capability-program-contract/src/lib.rs:482`),
the lifecycle rent credit's `release_set`
(`crates/dclutch-rent-contract/src/lifecycle_v2.rs:167`), the Custody vault and
replay seeds, the position admission body, the Series template digest. Because
`selected` never changes, **not one of those copies ever goes stale, and not one
of those addresses ever moves.** An alternative that made the single field
mutable would have had to rewrite or re-derive every one of them; this one
rewrites nothing. That is the decisive argument, and §6 discharges it in full.

### 5.2 `CoreState` after the change

Appending at the tail shifts no existing offset — the Lean layout is strictly
sequential with no alignment padding
(`formal/dclutch-semantics/DClutchSemantics/AbiSchema.lean`, `specializeFrom`).
At HEAD `STATE_BYTES = 360` (`crates/dclutch-market-core-codec/src/generated.rs:3`),
proved by `theorem state_schema_width : stateBytes = 360 := by native_decide`
(`formal/dclutch-semantics/DClutchSemantics/MarketCoreAbi.lean:73`), with
`terminal_receipt` last at `[328,360)`.

| new field | offset | width |
|---|---|---|
| `active_release_set: Identity` | `STATE_ACTIVE_RELEASE_SET_OFFSET = 360` | 32 |

`STATE_BYTES: 360 → 392`; the theorem becomes `stateBytes = 392`.
`state_schema_unique`, `state_fields_disjoint` and `state_fields_bounded`
re-derive with no manual work.

The field is `Identity`, which rejects all-zero
(`generated.rs:92-103`), so "unset" is not representable and there is no
default-interpretation trap. Founding writes `active_release_set =
selected_release_set` in the same transition that writes the identity.

### 5.3 `Core::MigrateMarket` — the route

New 8-byte magic `DCLTMIG1`, dispatched by a new magic+length branch in
`programs/dclutch-core-sbf/src/lib.rs:process_instruction`, which is already a
chain of exactly such branches (`:201-256`).

Wire, **16 bytes**: magic `[0,8)`, schema u16 = 1 `[8,10)`, reserved `[10,16)`
`require_zero`. **No arguments.** The successor is not supplied; it is read out
of the lineage record that the market's own `active_release_set` addresses.

Account frame, **exactly 5**, fixed order, and **zero signers admitted** — the
route refuses any signer at all, the `begin_retiring.rs:57-58` shape:

| # | account | owner | privileges | authenticated by |
|---|---|---|---|---|
| 0 | market | Core | writable, non-signer | `data_len() == STATE_BYTES`; decodes; address `== find_program_address(MarketCoreStateSeedsV2::new(state.identity), program_id)` and `== state.identity.market_id` |
| 1 | lineage record | Registry | readonly, non-signer | owner `== state.identity.registry_program`; address `== PDA([RELEASE_LINEAGE_PDA_DOMAIN_V1, state.active_release_set], registry)`; decodes; `predecessor == state.active_release_set` |
| 2 | successor activation cache | Registry | readonly, non-signer | owner `== registry`; address `== PDA([ACTIVATION_PDA_DOMAIN_V1, lineage.successor], registry)`; `require_cache_account`; decodes; `execution_release_set_id() == lineage.successor` |
| 3 | migration bounty escrow | Core **or** System | writable, non-signer | address `== PDA([MIGRATION_BOUNTY_PDA_DOMAIN_V1, state.active_release_set], program_id)`. Either a valid Core-owned `MigrationBountyV1` (§5.5), or pristine System-owned with zero data — which *is* "no bounty was funded" |
| 4 | bounty beneficiary | any | writable, non-signer | caller-named; receives the hop fee if one is available |

**Four properties of this frame are deliberate and each is load-bearing.**

1. **It never touches the predecessor's activation cache.** If migration read
   the superseded cache, the brick would defend itself: the one route that
   repairs a stranded market would be unavailable exactly when the market is
   stranded. This is the single most important structural property of the
   design, and it is why account 2 is the *successor* cache and there is no
   predecessor cache in the frame at all.
2. **It never touches a Program or ProgramData account.** No deployment is
   observed, no ELF is hashed, no slot is compared. Migration is therefore
   available even when every role's deployment has moved — which is the state it
   exists to repair — and it is cheap.
3. **It has no phase gate.** Migration is admissible in every phase, including
   `Retiring`, and including after a terminal receipt exists. A lane will be
   tempted to add "only while Open"; that would re-strand the exit, which is the
   entire defect. §8 pins this with a hostile.
4. **It admits no signer.** There is no identified party anywhere in the route.
   Anyone may push any market forward.

**Conjuncts, in order (codes in §8).**

1. Frame width is 5, privileges exactly as tabled, no account is a signer.
2. Market authenticates: width, decode, PDA re-derivation, `market_id` equality.
3. Lineage record authenticates and `predecessor == state.active_release_set`.
4. Successor cache authenticates and its id equals `lineage.successor`.
5. Write `active_release_set = lineage.successor`. Nothing else in the account
   is touched.
6. Commit-last persistence postcheck: re-decode the written bytes and require
   that exactly the one field changed and every other byte is identical —
   Core's existing `Commit` discipline.
7. Pay the hop fee if account 3 is a funded escrow with a spendable balance
   (§5.5); otherwise pay nothing and **still succeed**.

### 5.4 Why forward-only, replay-safety and skip-resistance are free

The route holds no ordinal and makes no comparison of ordinals. All three
properties fall out of the addressing:

- **Forward-only.** The only successor the route can reach is
  `lineage(active).successor`. There is no wire field naming a destination, so
  there is no destination to name wrongly.
- **Replay-safe.** After the write, `active` is `B`. Re-submitting the identical
  transaction re-derives the lineage address from `B`, which is a *different
  account* than the one supplied, and refuses at conjunct 3. Idempotence is by
  state, not by a nonce.
- **No skipping.** Reaching `C` from `A` requires two transactions, each of
  which is a complete, independently valid hop. A market three generations
  behind migrates three times. This is a feature, not a cost: the lineage is a
  chain that must be *walked*, never a claim that can be *asserted*.
- **No going back.** `PDA(B)` names `B`'s successor, never `B`'s predecessor,
  and conjunct 5 of `DeclareSuccessor` (§4.5) refuses a declaration whose slots
  do not move forward. There is no account that could carry a backward hop.

### 5.5 Funding: `MigrationBountyV1`, and why migration never depends on it

The census's doctrine is unambiguous: a permissionless but unpaid verb is
*"permissible rather than live"* (`candidate_v1.rs:290-295`), and pattern P1's
fix template is a prepaid, purpose-bound work escrow. Migration should be
GREEN, not YELLOW.

**Who pays.** The party that creates the need: whoever upgrades. Sizing is
knowable off-chain (count the markets whose `active_release_set == A`), so no
on-chain census is required for it.

`MigrationBountyV1`, Core-owned, **88 bytes**, magic `DCLTMBT1`, PDA
`[MIGRATION_BOUNTY_PDA_DOMAIN_V1, predecessor_release_set_id]` under Core:

| range | bytes | field |
|---|---|---|
| `[0,8)` | 8 | magic `DCLTMBT1` |
| `[8,10)` | 2 | schema u16 = 1 |
| `[10,12)` | 2 | profile u16 = 1 |
| `[12,16)` | 4 | reserved, `require_zero` |
| `[16,48)` | 32 | `predecessor_release_set_id` |
| `[48,56)` | 8 | `bounty_per_hop` u64 LE |
| `[56,88)` | 32 | `refund_beneficiary`, fixed at creation |

`Core::FundMigration` is permissionless: anyone may create the escrow (fixing
`bounty_per_hop` and `refund_beneficiary` once, at creation — doctrine (c), the
Dealer-checkpoint precedent) and anyone may top it up afterwards. The payout at
a hop is `bounty_per_hop` to the caller-named beneficiary, taken only from the
balance above rent exemption.

**The floor is the point.** If the escrow does not exist, or is empty, the hop
**still succeeds and pays nothing**. Migration's availability is never
conditional on anyone having funded it. Stated in the census's own terms: the
migration act-point is **GREEN when funded and YELLOW when not**, and it is
never RED. Gating the exit on a bounty would have reintroduced exactly the
liveness dependency this design exists to delete.

The release tool makes funding a step of the deployment plan (§11 commit 7), so
the healthy path is funded by construction while the protocol depends on nothing.

**Residue, named.** Over-funding is the depositor's choice and stays claimable
by future hops forever — a lineage never expires, so the pool never becomes
purposeless. There is no closer, and adding one would need a count of markets
still on `A`, which §9 deliberately avoids needing.

---

## 6. What migrates, and what does not

### 6.1 Rule M1 — the one rule the implementing lane needs

> *(Amended by §14.4: the `selected` list below wrongly includes rent credits,
> and the rule needs a third category — a stored **founding** id consumed at an
> **active** site.)*
>
> **M1.** `active_release_set` is read at exactly one kind of site: deriving or
> authenticating the **Registry activation cache**, and joining the role
> admissions read from it. `selected_release_set` is read everywhere else — PDA
> seeds, child-request release-set coordinates, stored copies in capability
> roots and rent credits, and every consistency comparison among those.
>
> Founding is the one site where the distinction does not arise, because
> founding is the author of both and writes them equal.

M1 is mechanically checkable rather than a matter of taste. A site takes
`active` if and only if its value flows into `ACTIVATION_PDA_DOMAIN_V1`
derivation or into `authenticate_activated_role_v1`. Both are greppable, and
commit 1 of §11 is exactly the census that applies the test to every site
before any behaviour changes.

**Make the compiler enforce it.** Introduce two newtypes in the codec —
`SelectedReleaseSet(Identity)` and `ActiveReleaseSet(Identity)` — so a mix-up is
a type error rather than a silent 32-byte swap. This is the tree's own standing
rule, `WAVE.md` doctrine 2: *"the fixture is never the authority… address-vs-digest
gets a newtype."* Two release-set facts of identical width flowing through one
transaction is precisely the shape that rule exists for.

### 6.2 The claim, and its proof

**Claim.** A lineage hop changes exactly two addresses in the entire protocol,
and neither of them holds value.

**Proof.** Every derived address in the tree is a function of two things: the
seed bytes, and the program id it is derived under.

- *The seed bytes are unchanged.* Every release-set-bearing seed tuple in the
  tree draws its release set from the market's founding coordinate, and
  `selected_release_set` is not written by this design.
- *The deriving program ids are unchanged.* `DeclareSuccessor` conjunct 4
  (§4.5) refuses any hop that changes a role's program id, so the Core, Claims,
  Trading, Resolution and Custody program ids on both sides of a hop are equal.
  Rent and Registry are not execution roles at all — they come from the separate
  `ProtocolInfrastructureProfileV1` — so they cannot move with a set.

Therefore every child address is unchanged. ∎

The invariance conjunct now pays for itself twice: once as the authorization
boundary (§4.3, a hop never redirects trust to a different program) and once as
the reason migration is a single field write.

**The complete survival table**, verified seed tuple by seed tuple at HEAD:

| account | address depends on | survives a hop |
|---|---|---|
| Market state | `[…, selected, registry_program, generation]` under Core (`physical.rs:646-661`) | **yes** |
| **Custody vault — the Hoard, the collateral** | `[domain, market, selected, context, compartment]` under Custody (`custody-contract/src/lib.rs:44,348-353`) | **yes** |
| Custody transfer authority | `[domain, market, selected]` under Custody | **yes** |
| Custody replay cursor | `[domain, market, selected, caller_role, context]` under Custody | **yes** |
| Projected-custody caller | `[domain, selected, market, …]` under Custody | **yes** |
| Core-effect caller authority | `[domain, selected, market, role, context, digest]` under the role program | **yes** |
| Capability root | `[domain, market, generation, manifest, entry_index, kind, capability_release, config]` under Trading | **yes** (and its stored `release_set` body field stays equal to `selected`) |
| Capability funding (prepaid principal) | `[domain, market, generation, entry_index, config_id, release_id]` under Trading | **yes** |
| Lifecycle rent credit | `[domain, market, generation]` under Rent — not an execution role | **yes** (stored `release_set` stays equal to `selected`) |
| Claims aggregate | `[domain, market]` under Claims | **yes** |
| Protocol position / admission | `[domain, aggregate, owner]` under Claims | **yes** — and its body's `release_set`, `claims_program` and `trading_program` all still match |
| Direct maker root, registered intent | `[domain, market, generation, maker(, nonce)]` under Trading | **yes** |
| Series founding permit | `[domain, selected, market, ticket_context]` under Core | **yes** |
| Series ticket state | `[domain, root, ticket_record]` under Trading; template digest bakes `selected` | **yes** |
| Source resolution state, certificates | `[domain, market, generation]` / `[domain, source_state, kind, sequence]` under Resolution | **yes** |
| **Registry activation cache** | `[ACTIVATION_PDA_DOMAIN_V1, active]` under Registry | **moves — this is the entire point** |
| **Capability seal** | `[domain, descriptor_schema, descriptor_digest, action, trading_semantic_release, registry_program]` under Trading (`capability-seal-contract/src/lib.rs:406-425`) | **moves** — see §7.1 |

---

## 7. Compatibility: in-flight state across a hop

### 7.1 The one thing that must be rebuilt: capability seals

A seal's address commits `trading_semantic_release`, and an upgrade of the
Trading role is exactly a change of that value. So after a hop the seals for the
new Trading release do not exist.

Three facts make this a chore rather than a hazard: seals are
**market-independent** (nothing per-market is stranded), they are a **verdict
cache** rather than an authority, and re-minting is **permissionless**. The cost
is a one-time protocol-wide re-mint per Trading upgrade — which is already
incurred today by every upgrade, since today's answer to an upgrade is to
abandon the old markets entirely. Migration does not add this cost; it inherits
it.

**Required verification for the implementing lane** (commit 1): confirm that
seal re-minting has a live permissionless route at HEAD, and that a market whose
seals are absent refuses *softly* — falls back to the unsealed path — rather
than bricking. If it bricks, that is a second finding and belongs on the board,
not buried in this design.

**Answered, and it bricks — see §14.5.** Re-minting is permissionless as hoped;
the soft-degradation half of the premise is false.

### 7.2 Positions, replays, permits, records: untouched

By §6.2 every one of these keeps its address, and by M1 every stored copy of a
release set inside them keeps comparing equal, because they all hold `selected`
and `selected` never moves. Concretely:

- **Positions and admissions.** The admission body pins `release_set`,
  `claims_program` and `trading_program`, and Close refuses on any mismatch
  (`programs/dclutch-claims-sbf/src/protocol_position_v2.rs`). All three still
  match: the release set is `selected`, and the two program ids are pinned equal
  by invariance. **A hop strands no position and blocks no close.**
- **Custody replays and vaults.** Address and body both keyed on `selected`.
  Untouched.
- **Rent credit.** Address `[domain, market, generation]`; stored `release_set`
  equals `selected`. ~~Untouched~~ — **THIS ROW IS WRONG AS WRITTEN; see §14.1.**
  The stored id also seeds the *activation-cache* address on both branches of the
  close path, so a hop leaves the credit deriving a superseded cache and its
  close refuses forever. §14.3 carries the ruled resolution.
- **Relay and resolution records.** Carry no release set at all.

### 7.3 The worked case that looks hardest: a permit that outlives an upgrade

Stage-1 `Found` creates the Market and allocates a founding permit whose *own
address* commits the release set. The allocated permit is non-expiring — R5 /
`a16d1b0b` deleted the `current_slot > intent.expiry_slot()` conjunct from
`authenticate_permit`, so the guarantee is completion-only, forever. A permit
can therefore certainly outlive an upgrade.

Trace it: found on `A`; upgrade; `DeclareSuccessor(A→B)`; anyone migrates the
market, so `active = B`, `selected = A`; then stage-2 `Open` runs.

- The permit's address is `[domain, A, market, ticket_context]` under Core —
  unchanged, because the permit commits the *founding* set.
- `Open` compares `intent.release_set == request.release_set ==
  state.identity.selected_release_set`, all three of which are `A`. Unchanged.
- `Open` authenticates its roles against `active = B`. This is the only line
  that moved, and it is the line that makes the market openable at all.

So the Open request carries `A` while the role authentication uses `B`, and
both are correct. **This is exactly the case the two newtypes of §6.1 exist to
keep straight**, and it is the case the implementing lane should write first as
a hostile.

### 7.4 Founding on a superseded set: deliberately not refused on chain

A founder may still choose a set that already has a declared successor. The
market is born with `selected = active = A` and can be migrated forward
immediately by anyone.

Adding an on-chain refusal would cost the lineage record account on the founding
frame — already one of the largest in the tree — to prevent something that is
merely wasteful rather than unsafe. **The refusal belongs in the release tool
and the operator**, where it lands at the desk instead of the validator (the
`12d0deb5` precedent). Recorded here so a later lane does not read its absence
as an oversight.

---

## 8. Hostiles, with codes reserved

Bands are per decision 0007, whose authority is the crate
(`crates/dclutch-refusal-registry/src/lib.rs`), not the record. Verified at
HEAD: the Registry band `0x1xxx` is contiguous through
`RegistryError::ReleaseSuperseded = 0x100D` (`registry-sbf/src/lib.rs:104`),
first free `0x100E`; the Core band `0x3xxx` is contiguous through
`CoreSbfError::RecoveryWalkUnavailable = 0x3011` (`core-sbf/src/lib.rs:153`),
first free `0x3017`.

> **Renumbered 2026-08-31.** This design reserved `0x3012`–`0x3014`, but the
> cohort-9 spline wire landed first and its founding-time price-gate conjunct
> now owns `0x3012`–`0x3016` (`PriceGateRequired` through
> `PriceGateNonCanonical`). Nothing enforced the reservation — the census checks
> uniqueness among codes that *exist*, not among codes a design doc has spoken
> for — so this is the paperwork catching up with the code rather than a
> conflict. The three codes below move up by five; no other change.

**No eight-reader fan-out is needed.** The existing `ReleaseSuperseded` refusal
has eight banded discriminants because eight programs read the cache. Every
refusal below is raised by exactly one program — Registry for declaration,
Core for migration — because no other program participates in either route.

### 8.1 Codes to allocate

| code | name | program | the conjunct it is the other side of |
|---|---|---|---|
| `0x100E` | `ReleaseLineageAlreadyDeclared` | registry | §4.5 c7 — the lineage account is not pristine |
| `0x100F` | `ReleaseLineageRoleIdentityMoved` | registry | §4.5 c4 — some role's program id differs across the hop |
| `0x1010` | `ReleaseLineageSelfSuccession` | registry | §4.5 c3 — `A == B` |
| `0x1011` | `ReleaseLineageAuthorityMissing` | registry | §4.5 c6 — a moved role's authority slot is absent, not a signer, not the bound key, or the artifact is `Immutable` |
| `0x1012` | `ReleaseLineageNotForward` | registry | §4.5 c5 — a moved role's successor slot is not strictly greater |
| `0x3017` | `ReleaseLineageAbsent` | core | §5.3 c3 — no lineage record exists for the market's active set |
| `0x3018` | `ReleaseLineageMismatch` | core | §5.3 c3 — the record decoded but `predecessor != active_release_set` |
| `0x3019` | `ReleaseSuccessorNotActivated` | core | §5.3 c4 — the successor cache is absent, undecodable, wrongly owned, or names another set |

Reused rather than duplicated: `RegistryError::AccountFrame` `0x1001` and
`ActivationCache` `0x1005`; `CoreSbfError::AccountFrame` `0x3001`,
`Market` `0x3005` and `Commit` `0x300D`. Their existing meanings are exact here,
and a second code for the same fact would be a second author for it.

**Reserved, not allocated** — for Q8c's close route (§9): `0x1013`
`ActivationCacheStillLive`, `0x1014` `ActivationCacheNoSuccessor`.

**Two band assertions must move with the allocation**, or the build breaks
silently on the next append: `programs/dclutch-registry-sbf/src/lib.rs:110-118`
and `programs/dclutch-core-sbf/src/lib.rs:156-159` each name the *last* variant,
as does the mirror at `programs/dclutch-core-sbf/src/tests.rs:372`.

### 8.2 The hostile table

Every row ships as a test. Rows marked **admitted** are not defects — they are
places where a reviewer will expect a refusal and must be shown the argument
instead, which is why they are in the table rather than omitted from it.

| # | hostile | attack | verdict |
|---|---|---|---|
| H1 | **wrong-lineage** | migrate a market on `A` supplying a valid lineage record declared for some other predecessor `X` | refused `0x3018` — the record's address is `PDA(X)`, the route derives `PDA(A)` |
| H2 | **forged-lineage** | publish a lineage record for `A` naming an attacker-built successor, without holding any upgrade authority | refused `0x1011` |
| H3 | **skip-generation** | market on `A`, lineage `A→B→C`; migrate straight to `C` by supplying `C`'s activation cache | refused `0x3019` — the cache's id is `C`, the record at `PDA(A)` names `B` |
| H4 | **sideways-set** | declare `A→B'` where `B'` names a different Trading program id | refused `0x100F` |
| H5 | **backward-set** | declare `B→A`, or any successor whose deployment slots are older | refused `0x1012` |
| H6 | **replay-migration** | resubmit the identical successful migration transaction | refused `0x3018` — `active` is now `B`, so the supplied record at `PDA(A)` no longer matches |
| H7 | **lineage fork** | `DeclareSuccessor(A→B)` then `DeclareSuccessor(A→C)` | refused `0x100E` — the account is no longer pristine |
| H8 | **self-succession** | declare `A→A` | refused `0x1010` |
| H9 | **partial-cache successor** | declare `A→B` where `B`'s cache has only 3 of 5 roles activated | refused `0x1005` — a partial cache cannot `decode` (`activation.rs:309-316`) |
| H10 | **unmoved-role authority theft** | supply a signer in an unmoved role's slot to make it look consented | refused `0x1011` — an unmoved slot must be `system_program::ID` and must not be a signer |
| H11 | **immutable role claimed as moved** | declare a hop moving a role whose artifact is `Immutable` (no bound authority) | refused `0x1011` — `moved_i` with `upgrade_authority == None` is a contradiction |
| H12 | **phase gate** | migrate a market in `Retiring`, and again after a terminal receipt exists | **admitted, and must stay admitted.** The test asserts admission. Unbricking the exit is the design's purpose; a phase gate here would restore R1 |
| H13 | **migrate from a healthy set** | declare and migrate while `A`'s deployments have not moved at all | **admitted.** The invariant is "forward along an authored lineage to an activated set", never "away from a broken one" — which is what lets migration work without ever reading `A`'s cache |
| H14 | **hostile third-party migration** | a stranger migrates someone else's market without asking | **admitted.** By §4.4 the market ends up on the same five programs; the code behind them was always the upgrade authority's to change. No trust boundary moves, so there is nothing to consent to |
| H15 | **superseded-cache read** | any route in this design authenticates a role against `A`'s cache | **must not exist.** `MigrateMarket`'s frame contains no predecessor cache at all (§5.3); `DeclareSuccessor` reads one but admits no role from it. The test greps the two routes for `authenticate_activated_role_v1` over account `A` |
| H16 | **bounty as a gate** | drain or never fund the escrow, then migrate | **admitted and pays nothing.** The test asserts the hop still succeeds with a vacant escrow — the C1-adjacent trap where a funding mechanism quietly becomes a liveness dependency |
| H17 | **bounty double-spend** | migrate the same market twice to collect twice | refused by H6; and separately, a hop pays at most `bounty_per_hop` and only from the balance above rent exemption |
| H18 | **seed confusion** | build a Custody request carrying `active` instead of `selected` | refused by the existing vault-seed comparison; and made unrepresentable by the §6.1 newtypes |

---

## 9. B22 / Q8c: cache closure becomes derivable

Q8c's finding is that an unrefcounted `CloseActivation` is *"R1's brick,
weaponized"* — anyone funding one transaction could permanently disable
retirement for every market on a release set — and its prescribed order was
**Q1 ruling → refcount → close route**, with the refcount a Core-maintained
`markets_on_release_set == 0` that Registry would read.

**This design makes the refcount unnecessary.** Closability is derivable from
chain state that Registry can already see, using two conjuncts:

1. **A forward route exists**: a `ReleaseLineageV1` at `PDA([RELEASE_LINEAGE_PDA_DOMAIN_V1, A])`
   decodes with `predecessor == A`. Every subject still on `A` therefore has a
   migration available — and, critically, **that migration provably does not
   read `A`'s cache** (§5.3, property 1). Closing the cache cannot remove the
   route off it.
2. **The cache is already inert**: for a caller-named witness role `i`, the live
   ProgramData of `A.role(i).program` carries a `deployment_slot` strictly
   greater than `A.role(i).deployment_slot`. Under Loader V3 slots only move
   forward, so this is permanent — `A`'s cache will refuse every multi-role
   authentication for the rest of time.

Sketch frame for `Registry::CloseActivation`, **6 accounts** (Q8c owns the
build): cache `A` (writable), lineage record for `A` (readonly), witness role
Program and ProgramData (readonly), beneficiary (writable), and the caller's
fee payer. New codes `0x1013` / `0x1014` reserved in §8.1.

**Rent goes to the caller.** The cache carries no creation-fixed beneficiary
field and has no spare bytes to add one, so "the caller" is the only
well-defined recipient — and by the census's own P1 analysis it is also the
right one. That makes cache closure the tree's *second* caller-funded cleanup
verb after R6's record abort, which is a small but real move of a YELLOW row
toward GREEN.

**The residual, stated exactly.** *(§14.6 amends this: the bound below holds for
markets but fails for the lifecycle rent credit, which makes §14.3 an ordering
prerequisite for this section.)* A market still on `A` that has not yet
migrated loses `A`'s surviving *single-role* routes at the moment of closure.
It is not stranded: every multi-role route was already dead by conjunct 2, and
migration remains available and does not touch the cache. So the effect is a
delay bounded by one permissionless transaction, not a loss. Ordering the tool
to migrate before it closes reduces the residual to zero, and the §5.5 bounty is
what makes that ordering something a stranger will actually perform.

**Ordering, restated for the queue**: Q1 (this design) → Q8c. Unchanged from
Q8c's own conclusion; what changes is that the middle step it named — a
cross-program refcount, with a founding-frame change to maintain it — is not
needed at all.

---

## 10. Alternatives rejected, with reasons

Recorded so a later lane does not re-derive them, and so the ones that *look*
cheaper are shown to be more expensive.

1. **Exempt the exit path from the slot pin** (the census's "smallest" option;
   ember's option (b)). **Ruled out by C2, and wrong on the merits anyway**: the
   retirement path is the one where the money moves, so it is the last place to
   run against unauthenticated code. Not a fallback if this design proves
   expensive.
2. **Ship `Immutable` production releases.** Zero code, and it does close R1 —
   an immutable set can never be superseded. Decision 0012 already refused the
   premise for devnet on economics (~31.7 SOL of unrecyclable rent, days of
   faucet accumulation, against a substrate to be iterated "a million times").
   It remains correct for a final public substrate, and this design does not
   retire it — §4.4's symmetry says an all-`Immutable` set simply never needs a
   lineage.
3. **Let `Reauthenticate` re-pin the slot in place.** Verified read-only at HEAD
   (`require_readonly_frame` then `set_return_data`,
   `programs/dclutch-registry-sbf/src/lib.rs:275-302`). Making it a writer would
   mean a release-set id no longer identifies the code it was activated over —
   destroying the content-addressing the whole record layer rests on. It is C2's
   carve-out wearing a different hat: the market would silently begin
   authenticating against bytes nobody admitted.
4. **Pin `semantic_release_id` instead of the content id**, so upgrades within a
   semantic identity need no migration. This is blanket forward-trust: uncounted,
   unauthorized per market, and it deletes `ReleaseSupersededByUpgrade` as a
   signal. Worse, §4.3 establishes that `semantic_release_id` is the *only*
   field of `ArtifactReleaseV1` not forced by observation of the deployment — it
   is publisher-supplied. Pinning the one unverifiable field is the exact
   inversion of what the evidence supports.
5. **Rewrite `selected_release_set` in place.** §3.1: it is seed component 6 of
   9; the rewrite breaks eight PDA re-derivations and orphans the Hoard.
6. **Drop `release_set` from the Market and child seeds (V3 domains).** Coherent,
   and it would make the single field safely mutable. But it needs eight new PDA
   domains and a Lean physical-ABI change, it does not rescue existing markets
   either (their V2 addresses are already committed), and after a hop every
   *stored copy* of the release set — capability root header, rent credit,
   position admission — goes stale and needs rewriting or exempting. Strictly
   larger than the two-field split, with a new class of staleness the split does
   not have.
7. **A separate `MarketReleasePinV1` PDA instead of a state field**, absent
   meaning "founding set". Avoids the Lean change, but "an optional account
   whose absence means a default" is the ambiguity C4 exists to forbid, and it
   adds an account to the frame of every role-authenticating route across seven
   programs — against one tail append and one theorem.
8. **A Core-maintained refcount to gate cache closure.** Superseded by §9, which
   derives closability from a lineage record and a slot witness and needs no
   counter, no cross-program read, and no founding-frame change.
9. **Market relocation** — close the market and recreate it at the address the
   new set implies. This is the only option that rescues *already-existing*
   markets, and it is enormous: it moves the Hoard, every vault, every position,
   and every replay cursor. C2 makes it unnecessary — the devnet strand is
   accepted, and no market that matters exists yet.
10. **A `predecessor` field inside `ExecutionReleaseSetV1`.** No spare bytes
    (§3.5); a schema-2 wire change re-hashes every release-set id in existence;
    and lineage is not membership, which is the only thing that type is the
    semantic owner of.

---

## 11. Implementation plan, per commit

Sequenced so that every commit is independently reviewable and no commit both
changes a layout and changes a behaviour. Commit 1 is a census, in the tree's
own method: classify before you edit.

| # | commit | scope | control |
|---|---|---|---|
| 1 | **census: classify every release-set read site** | No code. Apply M1 (§6.1) to all release-set reads and publish the ACTIVE/FOUNDING verdict per site under `docs/evidence/`. The test is mechanical: a site is ACTIVE iff its value reaches `ACTIVATION_PDA_DOMAIN_V1` derivation or `authenticate_activated_role_v1`. **Also answers the two required verifications**: (a) §7.1, does seal absence degrade softly; (b) does any *persisted* checkpoint or continuation state store an activation-cache digest or release-set id, or are those per-transaction request fields only | the census is the artifact; reviewed before any behaviour moves |
| 2 | **registry-contract: `ReleaseLineageV1`** | The 248-byte type: layout constants, `to_bytes`/`decode`, header validation mirroring `validate_activation_header`, `require_zero` on both reserved runs. No program change | unit tests incl. decode hostiles (bad magic, bad schema, nonzero reserved, wrong width, zero endpoints) |
| 3 | **registry-sbf: `DeclareSuccessor`** | New magic `DCLRLND1`, fourth sub-dispatcher branch, the 11-account frame, conjuncts 1–8, codes `0x100E`–`0x1012`, band assertion update at `lib.rs:110-118` | hostiles H2, H4, H5, H7, H8, H9, H10, H11 — each refused at its own named conjunct with its pinned code; `cargo build-sbf` produces the ELF |
| 4 | **Lean: the `active_release_set` field** | `MarketCore.lean` — add the field to the state (**not** to `MarketIdentity`), its nonzero invariant, and its write in `found`; add the migrate transition to `Action`. `MarketCoreAbi.lean` — append the `StateField` variant + schema entry + `rustName` arm; `state_schema_width : stateBytes = 392`. Regenerate. Ships with the mechanical fixes to every exhaustive `Action` match, including `trading-sbf/src/outer.rs:209,228,548` | `lake build` green with zero `sorry`; regenerated `generated.rs` byte-compared against the emitter; the whole workspace builds |
| 5 | **codec: newtypes, and apply M1** | `SelectedReleaseSet` / `ActiveReleaseSet` newtypes; switch the join sites at `generated.rs:226-227`, `:281-282`, `:1217`, `:1219` and `physical.rs:725` to the active field via the emitter. `:804`/`:807` stay on `selected` — founding authors both | the type change is the control: a mix-up cannot compile. Existing Core tests stay green because at founding the two are equal |
| 6 | **core-sbf: `MigrateMarket`** | New magic `DCLTMIG1`, dispatcher branch, the 5-account zero-signer frame, conjuncts, codes `0x3017`–`0x3019`, band assertions at `lib.rs:156-159` and `tests.rs:372` | hostiles H1, H3, H6, H12, H13, H14, H15, H18, plus the §7.3 permit trace. H12 and H13 **assert admission** |
| 7 | **core-sbf: the migration bounty** | `MigrationBountyV1` (88 bytes), `FundMigration`, and the payout leg. Beneficiary and `bounty_per_hop` fixed at creation | hostiles H16 (vacant escrow still migrates) and H17 |
| 8 | **release tool: author the lineage** | Emit `DeclareSuccessor` as a required step of the deployment plan, derived in `authenticate_complete_upgrade_set_for_prepare` — the one function already holding both cohorts' revisions and every role's pre- and post-upgrade observation. Refuse to *plan* an in-place upgrade that cannot be followed by a declaration. Fund the bounty | plan-time hostiles; the refusal lands at the desk, not the validator |
| 9 | **mirrors and generated docs** | operator, SDK and TS mirrors; `abi:*:verify`; refusal registry regen (`tools/genref/generate.sh` → `docs/reference/refusals.md`); route census `--check-unique` | the existing verify gates |
| 10 | **the campaign that closes R1** | Real-SVM: found on `A`; upgrade one role; **assert the brick** by its pinned code; declare; migrate permissionlessly from a stranger's keypair; drive the market to full retirement on `B` and assert lamport conservation | this is the evidence that R1 is closed. Until it is green, R1 stays RED |

**Sizing.** *(§14.7 amends this: commit 5's real surface was measured at ~39
adapter sites across seven programs, not five join sites.)* Commits 2–3 ≈
600–900 lines; 4–5 are narrow in Lean and wide in
regenerated output; 6 ≈ 400–500; 7 ≈ 250; 9 is mechanical; 8 and 10 are the
judgment-bearing ones.

**Two sequencing constraints.**

- **Commit 8 touches `upgrade.rs`**, which is 14,011 lines and whose Reaffirm
  work is *approved and deferred to a window when it is quiet*
  (`WAVE.md:288-303`). Both changes bump the journal schema. **Land Reaffirm
  first, or land them together** — landing lineage first means two schema bumps
  through the same file.
- **Commits 1–7 do not touch `upgrade.rs` at all**, so the protocol half can
  proceed while that window is closed.

---

## 12. Cohort target

**The cohort after the current devnet cut.**

Commit 4 changes `STATE_BYTES`, and `CoreState::decode` refuses any other width,
so **every program that decodes a market ships together** — core, claims,
trading, custody, resolution, dealer and rent. There is no realloc path in
`dclutch-core-sbf`, so the widened state applies only to markets founded under
the new Core. That is the correct outcome and not a limitation to work around:
markets founded before this design cannot be rescued by it in any case, because
their addresses already commit their release set (§3.1).

Ember's ruling permits cohort-7 to proceed and accepts the devnet strand
meanwhile (C2), so nothing here should be accelerated into the current cut.

---

## 13. The charter's six questions, answered

1. **Lineage — how does a set name its predecessor, and who authors it?** A
   separate Registry-owned `ReleaseLineageV1` record keyed by the *predecessor*
   (§4.1–4.2). Neither endpoint can hold the field (§3.5, §4.1), and the pair-fact
   cannot live inside one member without restating the other. The author is the
   upgrade authority of every role whose artifact moved (§4.3) — which is exactly
   the coalition that could have caused the supersession (§4.4). Keying by
   predecessor makes no-fork structural and makes the successor derivable rather
   than supplied.
2. **The re-point route — who, under what proof?** Anyone. `Core::MigrateMarket`
   admits **no signer at all**, takes 5 accounts, names no destination on the
   wire, and reads the successor out of the lineage record the market's own
   pin addresses (§5.3). Forward-only, replay-safe and skip-resistant fall out
   of the addressing rather than from any comparison (§5.4).
3. **What migration means.** One field. `active_release_set` moves;
   `selected_release_set` — the market's name, seed component 6 of 9 — does not
   (§5.1). Role programs were never cached in state and are re-derived per
   transaction (§3.3), so dispatch follows for free. Exactly two addresses in the
   protocol change, and neither holds value (§6.2).
4. **Compat.** Positions, replays, permits, vaults, rent credits and records are
   untouched, because their addresses and their stored copies all key on the
   founding set (§7.2). Capability seals must be re-minted — market-independent,
   permissionless, a cost every upgrade already pays (§7.1). The permit that
   outlives an upgrade is traced in full at §7.3.
5. **Hostiles.** Eighteen rows at §8.2, each mapped to the conjunct that refuses
   it and the code it refuses with; five Registry and three Core codes allocated
   at §8.1. Five rows are **admitted on purpose** and their tests assert
   admission — the phase gate above all, because adding one would restore R1.
6. **B22 / Q8c.** Closability is derivable from a lineage record plus a
   deployment-slot witness, with no refcount, no cross-program read and no
   founding-frame change (§9) — because migration provably never reads the cache
   it would close.

**What this design does not fix, said plainly.** It does not protect a market
from its role programs' upgrade authority; that exposure is decision 0012's and
is unchanged. It cannot compel an authority to declare a successor, though §4.4
bounds that: an authority that never upgrades never strands anyone. And it does
not rescue any market founded before it lands.

---

## 14. Amendments from implementation

Commits 1-3 landed and found five things this document got wrong or left open.
Three are corrections to passages that are **wrong as written**; the doc says so
here rather than quietly diverging from itself.

### 14.1 §7.2's rent-credit row is wrong: the stored id is a cache seed

§7.2 lists the lifecycle rent credit as *"Untouched"* on the grounds that its
stored `release_set` equals `selected` and `selected` never moves. Half of that
row is right and the conclusion is wrong.

Right: the credit's own address really is `[domain, market, generation]`
(`programs/dclutch-rent-sbf/src/lib.rs:735-738`). The release set is **not** a
seed of the credit account. §6.2's address proof is unaffected.

Wrong: the stored id is not merely an equality pin. It is the **seed of the
activation-cache address**, at both branches of the close path.

- Stored at `crates/dclutch-rent-contract/src/lifecycle_v2.rs:169`, written at
  `:237-241`, read back at `:213`.
- Continuation branch: `programs/dclutch-rent-sbf/src/lib.rs:796-798` derives
  `find_program_address([ACTIVATION_PDA_DOMAIN_V1, state.release_set()])`.
- Direct branch: `lib.rs:758-766` passes `state.release_set()` into
  `authenticate_activated_role_v1`, which derives the same address at
  `crates/dclutch-registry-activation-auth-v1/src/lib.rs:188-194`.
- The code already says so in words, at `lib.rs:745-752`: *"the credit's own
  `release_set()` names the activation generation the address must be derived
  from."* The function is called `authenticate_current_core_v2` -- it demands the
  **current** Core out of a cache addressed by a **frozen** id.

After a hop the credit therefore derives the predecessor's cache and requires
the current Core to be admitted by it. That cache is, by the definition of
supersession, the one that no longer admits anything. The close refuses forever.

> **Migration as designed unbricks the market and leaves the account every other
> account's recovered rent drains into still bricked.**

### 14.2 Why a mirrored field on the credit cannot fix it

The tempting repair is to give the credit its own migrable `active_release_set`
and a route that updates it. It does not work, for two reasons.

**It creates a second author.** The market would hold one answer to "which
release set is active" and the credit another. Two authors for one fact is the
defect class this tree has already paid for roughly twenty always-refuses bugs;
the lineage record must stay the single author.

**Its update route has an unenforceable deadline.** The credit closes *after*
the market is gone: `LifecycleRetiredMarketObservationV2::validate`
(`crates/dclutch-rent-contract/src/lifecycle_v2.rs:150-157`, called at
`programs/dclutch-rent-sbf/src/lib.rs:618-630`) requires the market account to be
system-owned with `data_len == 0` **and** `lamports == 0`. Nothing on the close
path reads the market's data, and nothing can: there is no `try_borrow_data` on
`retired_market` anywhere in the file, which is structurally forced by the
zero-length requirement. So a mirror on the credit must be updated before the
market closes, nothing orders those two events, and once the market is gone the
authority that could repair a stale mirror no longer exists.

### 14.3 Ruled: the credit reads through the lineage instead

The credit's stored `release_set` stays exactly as it is -- it is a **founding
coordinate** and correctly records what the market was founded under. What
changes is how the close *consumes* it: rather than deriving a cache from it
directly, the close takes the lineage record for that id and walks forward to
the current set, then derives the cache from the endpoint.

This costs no new bytes, adds no state, introduces no second author, and has no
deadline -- the lineage records are permanent and the walk is available forever.
The forward-only conjunct of §4.5 makes the walk terminating. One consequential
detail for the implementing lane: the existing comparison at
`programs/dclutch-rent-sbf/src/lib.rs:820-824`, which requires the cache's own
`execution_release_set_id()` to equal the credit's stored id, must become a
comparison against the **endpoint of the walk** rather than against the stored
id, or it will re-impose the very refusal this removes.

### 14.4 M1 has no category for this site, and its example list is wrong

§6.1's rule M1 lists *"stored copies in capability roots and rent credits"*
among the sites that read `selected`. Its own mechanical test says a site is
ACTIVE iff its value flows into `ACTIVATION_PDA_DOMAIN_V1` derivation or into
`authenticate_activated_role_v1`. The credit's stored copy flows into **both**.
The test and the example list disagree, and the test is right.

The deeper gap is that M1 offers two categories where the tree has three. This
site is neither "an active id" nor "a selected id" but a **stored founding id
consumed at an active site** -- correct to persist, wrong to derive from. The
census found five accounts in exactly that shape
(`docs/evidence/RELEASE_SET_READ_SITE_CENSUS_2026_08_30.md` §6.1). Strike "and
rent credits" from M1's `selected` list and read §6.1 of the census as the third
category's membership roll.

### 14.5 §7.1's soft-degradation premise is false: seals refuse hard

§7.1 asked commit 1 to confirm that a market whose capability seals are absent
*"refuses softly -- falls back to the unsealed path -- rather than bricking"*,
and said that if it bricks, that is a finding. **It bricks.**
`process_hot_execution_v3` consults the seal unconditionally and
`authenticate_capability_seal_v3` refuses a vacant one; there is no unsealed
fallback reachable from that route (census §8). Re-minting is genuinely
permissionless and market-independent, so §7.1's cost analysis survives, but the
window between a Trading upgrade and the re-mint is a hard outage of the hot
route rather than a degradation, and §7.1 should say so.

### 14.6 §9's residual analysis does not hold for the rent credit

§9 argues that closing a superseded cache costs a delay *"bounded by one
permissionless transaction, not a loss"*, because migration remains available
and provably does not read that cache. That holds for markets. It fails for the
rent credit, which has no migration route at all and whose close **requires**
the predecessor's cache (14.1) -- close that cache and the credit's accumulated
rent becomes permanently unrecoverable, which is a loss, not a delay.

Under 14.3 the residual is repaired rather than merely bounded: a credit that
reads through to the live cache does not care that the old one is gone. **This
makes 14.3 an ordering prerequisite for §9** -- `Registry::CloseActivation` must
not ship before the credit stops deriving from a frozen id.

### 14.7 Commit 5 is mis-sized, and line citations decay

§11 scopes commit 5 as a newtype introduction plus five join sites. The census
measured the real surface at roughly **39 adapter sites across seven programs**,
plus the five persisted families of 14.4. The newtypes remain the right control;
the sizing does not.

A methodological note the next lane should not have to rediscover: this
document's line citations, and the census's own, decay within hours in a
twelve-lane tree. The census's rent-credit row was accurate when written and
drifted about sixty lines the same afternoon when an unrelated rent commit
landed. **Cite by symbol and function name; treat every line number as a hint.**

### 14.8 What the reader half needed, and the two ids that block the rest

Added by the MIGRATE lane, 2026-08-31, after commits 1-3 had been in the tree
through two further cuts. Four corrections and one finding.

**§5.2's widths are stale, and the pattern will repeat.** `STATE_BYTES` is
**368** at HEAD, not the 360 §5.2 records, and `state_schema_width` proves
`stateBytes = 368` (`formal/dclutch-semantics/DClutchSemantics/MarketCoreAbi.lean`).
`active_release_set` therefore lands at offset 368 and takes the state to 400,
not 392. More usefully: CoreState has now widened twice while this design sat,
so commit 4's `+32` must **join whatever batched widening the cut is already
making** rather than open a third restrand of its own. A lane picking this up
should confirm the current width from `generated.rs` and the Lean theorem
rather than from any prose, including this paragraph.

**The Registry codes are allocated; the Core codes are not.** §8.1's
`0x100E`-`0x1012` are live in `programs/dclutch-registry-sbf/src/lib.rs` and
mirrored into `docs/reference/refusals.md` and both TypeScript refusal
registries. `0x3017`-`0x3019` remain unallocated: the Core band is contiguous
through `PriceGateNonCanonical = 0x3016`, which the cohort-9 spline wire landed
after this section was written, so §8.1's Core rows have been renumbered up by
five and are accurate as they now read.

**Commits 1-3 left the record with no reader, and that was the real gap.**
`release_lineage_address_v1` and `is_lineage_account_v1` had zero production
callers, because a link is not a history: nothing in the tree could follow two
hops. `crates/dclutch-registry-contract/src/lineage_walk.rs` is now the single
authority for turning links into a chain, and
`packages/dclutch-sdk/lib/releaseLineage.ts` mirrors it for the SDK and the
site. The walk deliberately is not a fetcher — its three callers (an on-chain
route reading its own frame, a host tool reading RPC, a test reading a fixture)
cannot share one, but they must share the rule.

Two things fell out of writing it that this document should own. The **gap**
deserves its own refusal and gets one: a chain that ends before reaching the
world names the set that still owes a declaration, which is a repair
instruction rather than a complaint. And the **hop bound refuses only once a
further hop is offered** — a chain of exactly the bound arrives — which is not
what the first implementation did, and a test caught it.

**Retroactive authoring is admitted, and §4.2 is why.** The clause that omitted
`declared_at_slot` on the grounds that no conjunct would read it has a
consequence §4.2 did not draw: **lineage is retroactively authorable, honestly.**
A hop declared today for two cohorts that superseded each other weeks ago
encodes to exactly the bytes it would have encoded to at the time, so there is
no stamp to backdate and no contemporaneity to counterfeit. That is now asserted
in both languages rather than argued. It also dissolves the traded-market
worry: market22 does not need to survive a cut, it needs its history to be
followable across one, and a declaration authored after the fact does that.

**The finding, and it blocks the rest.** The full 32-byte release-set ids of
**cohort-7 and cohort-8 are not recorded anywhere in this repository** — they
exist only as eight-character truncations in `SESSION_STATE.md:14,665` and
`GOAL.md:280`. A truncation authors nothing: the lineage PDA seeds on all 32
bytes and both endpoints are derived from activation-cache accounts rather than
supplied. So the declarations that would make market22 followable cannot be
built from the tree as it stands, and this is a hard prerequisite rather than
an untidiness. Both ids are mechanically recoverable from chain — a market's own
bytes carry its founding set and prove it by its address, and superseded
activation caches are never deleted — and
`docs/evidence/RELEASE_SET_COHORT_LINEAGE_2026_08_31.md` records the mapping,
the three ids that ARE recorded, and the exact recovery route for the two that
are not. That file is also where cut gate 6's durable predecessor mapping
belongs.
