# Profile upgrade ruling — the Registry can move only if the profile can follow

Date: 2026-08-31. Adjudicator: PROFILE-RULE (Fable-class), chartered off
LINEAGE-FIX's discovery (commit `d6e43b11`). Status: RULED, subject to the
orchestrator's veto window; two items flagged for ember in §9. This ruling
gates cohort-9's cut: the Registry joined the redeploy set (GOAL.md, the
LINEAGE-WRITER blocker), and without this ruling's mechanism the upgrade
bricks founding, retirement, the series-permit refund, and provider
resolution with no in-tree repair.

Every claim below was read from the code at HEAD (`5616aaae`), not from the
report that chartered this. File:line citations are to that tree.

## 1. The invariant that must survive

Stated before any option is weighed, because every option is measured
against it and not against convenience:

> **I1 (noncyclic root).** Core's trust in the Registry and Rent programs
> must not derive from Registry-owned state. The profile is Core-owned,
> created under Core's own ProgramData upgrade authority
> (`programs/dclutch-core-sbf/src/infrastructure.rs:252-274`), with the
> deployed ELF hashed at first admission (`:472-522`). This is what breaks
> the cycle "the Registry attests the Registry."
>
> **I2 (no silent substitution).** No party may change which bytes serve as
> Registry or Rent under routes that already trusted a selection, except by
> an act at least as strong, as explicit, and as on-chain-visible as the
> founding act — and between the substitution and that act, every dependent
> route refuses by name. The slot pin implements the refusing
> (`crates/dclutch-registry-contract/src/immutable_registry.rs:439-459`);
> write-once implements "no new selection can ever be made," which
> OVER-satisfies I2, and the overshoot is exactly the undocumented
> carry-forward constraint.
>
> **I3 (first-admission honesty).** A claimed `elf_digest` becomes truth
> only by hashing the deployed bytes under the ceremony, once
> (`infrastructure.rs:467-471`: "must never be replaced by a pinned-digest
> fast path"). Every fast path afterward must derive from that admission.

What the profile protects, in one sentence: **markets that trusted an
infrastructure selection at founding can never have different bytes
substituted under them silently — not even by the deployer key.** The
deployer can move the bytes (Loader `Upgrade`); it cannot make anything
*accept* them. Note what the write-once does NOT protect: Core's upgrade
authority can already replace the reader itself (Core upgrades in place,
`ExactAuthority`, every cohort), so immutability against that party was
never a real guarantee — it was a speed bump that cost us the ability to
upgrade the Registry at all.

## 2. Ground truth — the mechanism, verified

- **Write-once, by vacancy.** `create_profile` demands the PDA be
  System-owned, zero-length, non-executable
  (`programs/dclutch-core-sbf/src/infrastructure.rs:532-538`); the sole
  write site is `:575-583`; the sole dispatcher entry is
  `lib.rs:351-355`. No re-initialize, succession, or close route exists
  anywhere in the tree. Once written, the PDA can never return to
  System-owned. **Escape "rewrite it": dead.**
- **Content pin, including the slot.** The profile stores
  `ArtifactReleaseIdV1` = sha256 of the raw `ArtifactReleaseV1` record
  bytes (`infrastructure.rs:326,349`), and that record's content includes
  `deployment_slot`, `upgrade_policy`, and the bound authority
  (decision 0016's layer 3). A recurring read (`AlreadyPinned`,
  `infrastructure.rs:307`) re-derives the record digest and refuses any
  record but the pinned one at `:177-179`. **Escape "present the new
  record": dead** — a new record has a new digest.
- **Slot pin under `ExactAuthority`.**
  `slot_pinned_release_elf_digest_v1` refuses observed-slot ≠ pinned-slot
  (`immutable_registry.rs:448-457`), naming a strictly-later slot
  `ReleaseSupersededByUpgrade` → `CoreSbfError::ReleaseSuperseded`
  (`infrastructure.rs:406-413`). Devnet mints `ExactAuthority` for every
  role whose ProgramData carries an authority
  (`tools/local-validator/bootstrap/successor/src/plan.rs:1417-1439`), and
  devnet keeps authorities live — that IS decision 0012's iteration
  substrate. So the deployed profile's registry pin sits on the
  slot-refusing arm. LINEAGE-FIX's premise verified.
- **The remedy the code promises is a lie for this one account.**
  `pinned_deployment_refusal`'s doc (`infrastructure.rs:400-405`) says a
  moved slot's "remedy is a re-release rather than an investigation
  (decision 0012)." Decision 0012 (§"the remedy",
  `docs/decisions/0012-devnet-iteration-substrate.md:180-199`) promises
  re-release → re-activate → re-found — a remedy that exists for the five
  cache-pinned roles and **does not exist for the profile**, because the
  profile is the one account with no re-pin route. 0012 widened what the
  profile's pinned records may say (`:222-227` admits `ExactAuthority` at
  the infrastructure sites) without giving the profile the succession the
  caches got. That is where this trap was armed.
- **The constraint was encoded, but only as tooling.** The successor
  bootstrap hard-whitelists Registry and Rent to `CarryForward` — "can
  never enter an Upgrade/Extend mutation path"
  (`tools/local-validator/bootstrap/successor/src/upgrade.rs:2339-2351,
  2535-2539`). The whitelist exists; its REASON (the brick) was written
  nowhere, and no design doc owns it. §11's P-008 turns it into a
  documented constraint with an owner.
- **The lineage machinery does not cover this.** `DeclareSuccessor`
  spans exactly the five execution roles
  (`crates/dclutch-release-set-contract/src/lib.rs:580-586`); Registry and
  Rent "cannot move with a set"
  (`docs/design/RELEASE_LINEAGE_MIGRATION_V1.md:803-804`). The profile is
  the single account in the system whose address carries no release, no
  generation, and no succession pointer
  (`crates/dclutch-release-set-contract/src/protocol_infrastructure.rs:34`,
  one-seed PDA under the Core program id).

## 3. The consumer map

Every on-chain reader, verified by grep over `programs/*/src` and by
reading each site. Trading, Claims, Custody, Registry, Rent, Dealer,
Product-Runtime read it **nowhere**; `DeclareSuccessor` never reads it.

| # | Route | Site | What breaks on Registry upgrade |
|---|---|---|---|
| 1 | Found, full frame | `core-sbf/src/found.rs:311` → `infrastructure.rs:73` (profile = frame account 30, `frame.rs:164`) | every new founding refuses `ReleaseSuperseded` |
| 2 | Found, projected frame | `found.rs:289` → `infrastructure.rs:96` | same |
| 3 | `retire_v1::process_checkpoint_prepare` | `retire_v1.rs:458,472` | retirement checkpoint refuses |
| 4 | `retire_v1::process_checkpoint_suffix` | `retire_v1.rs:702,718` | same |
| 5 | `retire_v1::process` | `retire_v1.rs:1501,1514` | physical retirement refuses |
| 6 | Series-permit expiry refund | `series_permit_expiry.rs:207,216` | expired-permit refunds refuse — prefunded lamports stay locked |
| 7 | Resolution provider instruction | `resolution-proof-sbf/src/provider_instruction_v3.rs:519-575` | provider resolution refuses |
| 8 | Resolution provider transport | `provider_transport_v3.rs:435-475` | same |

Both resolution sites re-derive the profile PDA under a Core program
account bound to the market (`provider_instruction_v3.rs:497-527`), then
authenticate the pinned registry deployment exactly as Core does —
`authenticate_deployment` over a live observation with
`pinned_deployment_refusal` naming supersession. Not a weaker read.

Host-side readers (must learn any new shape, no chain risk):
`crates/dclutch-operator/src/infrastructure.rs:322`,
`crates/dclutch-release-tool/src/infrastructure.rs:323,354`,
`crates/dclutch-market-retirement-v1-operator/src/lib.rs:888`,
`crates/dclutch-product-runtime-v2-operator/src/found.rs`,
`crates/dclutch-provider-transport-v3-operator/src/lib.rs:371,716`, and
the bootstrap successor tooling.

What KEEPS working during a broken-profile window: all trading, claims,
custody flows, `begin_retiring`, `open_market`, series open/consume,
redemption — none read the profile. The breakage is founding + the three
retirement entries + the refund + provider resolution. This asymmetry is
why the window in §6's ordering is livable and why nobody noticed the
constraint for eight cohorts: the carried-forward Registry never moved a
slot, so the refusal arm never fired outside a test.

### Which consumers redeploy in cohort-9 anyway

Measured `dfb41be6..HEAD` (the cohort-8 candidate pin — `a7d50d3a..dfb41be6`
is two prose commits, no `programs/` or `crates/` change; `Cargo.toml`,
`rust-toolchain.toml`, and the lock's resolution are byte-stable since the
pin), with per-program source diffs and a transitive path-dep closure:

| Program | Source vs cohort-8 pin | Chartered in cohort-9? | Profile consumer? |
|---|---|---|---|
| Trading | MOVED, +1828/−71 (CloseMakerReplay `direct_close_maker_v1.rs`, claims composition, ZeroBump seal arm) | YES (charter item 1+2) | no |
| Claims | MOVED, +2076/−245 (fractional claim check, compaction, curvature) | YES (FRACCHECK-3 et al.) | no |
| Core | MOVED, +223/−20 (`fcd6aecc` curvature founding gate, Found frame 39-account form, 5 refusal codes) | YES | **yes — 6 routes** |
| Resolution | source-unmoved (const-assert sweep + doc comments only; the changed reader symbols it never calls) | **not chartered — this ruling adds it** (§6) | **yes — 2 routes** |
| Custody | source-unmoved (same two hygiene commits) | not chartered | no |
| Registry | MOVED vs pin (`d6e43b11`); vs DEPLOYED bytes, +2154/−40 — see below | YES (GOAL: "joins the redeploy set") | no |
| Rent | source-unmoved vs pin; vs DEPLOYED bytes +451/−24 pent up | not chartered | no |

Two facts the map surfaced that change the problem's shape:

- **The deployed Registry is DEPLOY-1's bytes** (slot 489,100,383;
  `tools/release/devnet_upgrade_dryplan/dryplan.py:37-45` lists registry
  and rent as `carry-forward` at their DEPLOY-1 slots). It predates the
  entire lineage machinery — `lineage_v1.rs` (447 lines), `record_v1.rs`
  (+698) are all newer. So the cohort-9 Registry upgrade ships
  `DeclareSuccessor` to the chain FOR THE FIRST TIME, with `d6e43b11`'s
  conjunct fix inside it: no hop of any cohort is declarable on today's
  chain, and gate 6 has no on-chain form without this upgrade.
- **Rent is silently accumulating the same debt.** HEAD's rent-sbf carries
  +451/−24 vs the deployed bytes (checkpoint aggregate retirement, capped
  sweep-crank share) that the same brick has been blocking. The mechanism
  ruled here unbricks Rent too; whether Rent's pent-up changes ship in
  cohort-9 or a later ceremony is the steward's cut-scope call, no longer
  a structural impossibility.

A digest caveat carried from the map: `BASIS_ABI_UNIFICATION_V1.md:900-907`
prices the basis wire change by reverse-dependency closure as "eight of
ten releasable programs take a new ELF digest," custody and resolution
included; the symbol-level analysis says both survive byte-identical. Only
building both trees and diffing ELFs settles it — the cut campaign must do
that before claiming any role `AlreadyCurrent` (the evidence kind TRADE-5
invented for exactly this at cohort-8).

## 4. The design space, adjudicated against §1

### (a) Don't upgrade the Registry in cohort-9 — REFUSED as the plan, kept as the fallback

Deferral preserves I1-I3 trivially, and its cost is larger than the
charter framed it: the deployed Registry is DEPLOY-1's bytes (§3), which
predate the lineage machinery entirely — deferring the upgrade defers not
just `d6e43b11`'s fix but the existence of `DeclareSuccessor` on chain.
NO hop of any cohort is declarable today; gate 6's "predecessor mapping
recorded durably" degrades to an off-chain record; market22's walk shows
honest successor-undeclared at cohort-9 activation (degrades, not breaks —
`walk_lineage_to_head` ENDS at an undeclared successor,
`lineage_walk.rs:223-232`). And §10's verdict 2 says the 8→9 hop itself
has unmoved roles, so the debt is not historical: every cohort that
defers adds one more undeclarable hop to a walk that markets will
eventually need. The deeper cost: the carry-forward constraint survives as a
whitelist in a bootstrap tool, and the next person to touch it learns why
the hard way, again. Deferral is the fallback if the cut runs out of road,
not the plan.

### (c) Slot-tolerance in the authentication — REFUSED

The proposal: teach the pin an upgrade-aware reading — accept
observed-slot > pinned-slot when the observed upgrade authority equals the
bound authority. Measured against I2 and I3 it is unsound, and not
narrowly: the deployer key alone (no ceremony, no hash, no record) could
then put arbitrary bytes behind every profile-reading route, and
`slot_pinned_release_elf_digest_v1` would return the admitted digest as
"current" for bytes that no longer have that digest — the function's
contract ("observed-slot equality proves the admitted digest is the exact
current digest," `immutable_registry.rs:415-421`) becomes a lie precisely
when it matters. The two repairs both collapse: (i) re-hash the ELF per
read — refuses I3's economics (first admission hashes once because a
megabyte-scale hash per Found is the CU wall 0012 existed to remove), and
still leaves the profile pinning a record whose content (slot, digest) is
false; (ii) accept a successor record naming the new digest — that IS
succession, option (b), wearing the authentication's clothes. There is no
third repair inside the evidence model, because the model's whole design
is that a policy is a predicate the observation satisfies (0016 option C
rejection), and "some later slot under the same key" is a predicate that
arbitrary bytes satisfy. **The 0016 release-identity model does not
support slot tolerance without weakening exactly what `ExactAuthority`
refuses.**

### (d) Registry as a sixth execution role — NAMED as the end-state candidate, out of cohort-9

The architecturally uniform end state: the profile shrinks to identity
only (program ids never move — `DeclareSuccessor` conjunct 4 already makes
role-identity invariance a protocol law), and the Registry's CONTENT pin
moves into the activation cache like every other role, repaired per
release set by the existing lineage machinery. This would retire the
per-upgrade ceremony of (b) forever. It is refused for cohort-9 on size
and on an unresolved trust question: it rewrites the release-set contract
(five → six roles), the 1,288-byte cache layout and its 25 aliasing
pairs, `release_set_id` derivation (hashes the role ELF digests — every
existing id changes meaning), the lineage consent geometry LINEAGE-FIX
just stabilized, and every activation flow — weeks-class, across the most
safety-critical seams, in a cut already carrying CloseMakerReplay + spline
+ KAPPA. The trust question: a cache's registry-role entry is written by
Registry bytes, so the new Registry's first cache self-attests; the
consent that launders that (the declaration's authority signatures) roots
in the deployer key where the profile roots in Core's ceremony + hash —
whether that is a weakening of I1 or an acceptable restatement needs its
own adjudication. (b) is shaped so it does not foreclose (d): the V2
profile carries the same two bindings and nothing that would survive into
a role-based world.

### (b) Profile succession — RULED

A `ProtocolInfrastructureProfileV2` at a new one-seed PDA
(`b"dclutch:infrastructure:v2"` under the Core program id), write-once by
the same vacancy discipline, carrying the registry binding, the rent
binding, and the predecessor's two artifact-release ids (the succession
becomes content-walkable, like the lineage record's predecessor keying).
V1 is never touched — CloseSeal's precedent (P-006): extend write-once
state with new accounts, never mutate it. The redeployed consumers read
**V2 only** (§6). The creation ceremony is §5.

Why this survives §1 where nothing else does: I1 — V2 is Core-owned,
created under Core's ProgramData upgrade authority with first-admission
hashing, exactly V1's noncyclic root; I2 — between the Registry upgrade
and the ceremony, every consumer refuses by name (V1 readers on the slot
pin, V2 readers on vacancy), and the selection changes only by an act
strictly STRONGER than the founding act (§5 adds consent conjuncts V1
never had); I3 — the ceremony IS a first admission of the new deployment.
The cost, stated honestly: a Registry (or Rent) upgrade becomes a
**Core-release-class event, forever** — new profile version, new domain
string, Core and resolution-proof source changes, full ceremony. That
coupling is the documented replacement for the undocumented carry-forward.
It is cheap in practice (Core has shipped in every cohort; this is the
first infrastructure upgrade in nine) and it is honest: an infrastructure
substitution SHOULD cost a visible protocol release. If the cadence ever
makes it expensive, the exit is (d), not a weakening of (b).

## 5. The ceremony — `InitializeProtocolInfrastructureV2`

DeclareSuccessor's evidence geometry
(`programs/dclutch-registry-sbf/src/lineage_v1.rs`), applied to the
infrastructure pair, plus V1's own gates. Conjuncts, all refusing by
name:

1. **V1's whole gate, unchanged.** Core's live ProgramData upgrade
   authority signs (`authenticate_current_core_upgrade_authority`);
   first-admission FULL-ELF hash of both presented deployments against
   their finalized records (`ArtifactAdmissionV1::FirstAdmission` — the
   claimed digests are attacker-publishable until hashed); registry ≠
   rent, neither is Core; both releases satisfy
   `require_slot_pinned_release_v1`.
2. **Predecessor presence.** The V1 profile account is presented at its
   derived PDA, Core-owned, exact width, and hostile-decodes. Succession
   without a predecessor is `process_initialize`'s job; this route refuses
   a vacant V1.
3. **Identity invariance** (lineage conjunct 4). The V2 registry binding's
   program equals V1's registry program; likewise rent. A program-id move
   is a different, bigger act — refused here by name, always.
4. **Forward-only** (lineage conjunct 5). For a binding whose artifact id
   MOVED: the successor record's `deployment_slot` is strictly greater
   than the predecessor record's. For an unmoved binding (same artifact
   id — e.g. Rent when only the Registry upgrades): the binding is
   byte-identical to V1's, and no consent is demanded for it.
5. **Consent** (lineage conjunct 6). For each MOVED binding, the
   PREDECESSOR record's bound `upgrade_authority` signs. This is the party
   the Loader already required for the physical `Upgrade`; its signature
   here binds "the key that moved the bytes consents to the re-selection"
   on chain, mirroring the moved-role consent LINEAGE-FIX just
   red-proofed. (The key provably existed at upgrade time — the Loader
   demanded it — so this can never brick on a revoked authority; revoked
   means the ProgramData carries None, not that the key cannot sign.)
   Flagged to ember in §9 as the one open knob.
6. **No-fork vacancy** (lineage conjunct 7). The V2 PDA is System-owned,
   zero-data, zero-lamport-tolerant exactly as `create_profile` demands —
   write-once by the same discipline, one succession per domain, ever.
7. **Read-back belt** (lineage conjunct 8). Decode what was persisted;
   compare to the composed value.

Predecessor conjuncts (2, 3, 5) make the ceremony STRICTLY stronger than
V1's creation, which is what "creation gated by evidence as strong as the
original" demands from the AlreadyCurrent precedent's direction — the
chain's own records, not a receipt, are what conjuncts 4 and 5 read.

**Contract shape**: V2 record = V1's 144-byte layout (V1's magic is
`DCLTINF1`; V2 takes the successor magic and schema 2) + 64 bytes
predecessor artifact ids + reserved tail; still a single account, still
one hostile decoder in `dclutch-release-set-contract`. **The layout is
Lean-owned**: V1's constants are `@generated` by
`formal/dclutch-semantics/EmitProtocolInfrastructureProfileAbiRust.lean`
into `generated_protocol_infrastructure.rs` behind a `check-generated.sh`
pin — V2 is authored in that emitter FIRST and regenerated, never
hand-authored beside it (P-007's doctrine, and the regenerate-before-
genref ordering the curvature landing just re-taught). The read side of
every consumer changes by exactly: the PDA domain string, the expected
width, and the decoder — the frame SHAPE is untouched (profile stays one
account at Found index 30, `frame.rs:164`; the artifact records at 31-36
become the successor records).

## 6. Reader semantics and the cut ordering this forces

**V2-only in redeployed consumers. No fallback.** A try-V2-then-V1 read
was considered and refused: it creates two live authentication paths (an
O-005 "parallel authority path" smell), its only benefit is founding
during the mid-cut window that gate 7 forbids anyway, and its failure mode
(V2 creation forgotten, V1 silently still ruling) is exactly the silent
divergence this codebase spends itself refusing. New code reads V2;
vacancy refuses; the ceremony is what un-refuses it. V1 becomes a sealed
historical record — still on chain, still content-walkable from V2's
predecessor ids, never again an authority.

Forced ordering inside the cut (extends §7 of
`COHORT9_PLAN_REVIEW_2026_08_31.md`):

1. Pre-upgrade sweep (gate 6 first half) — settle `fee_owed`, drain
   redeemable value from markets 21/22, archive life tables. Any market
   still needing PROVIDER resolution must resolve **before** step 3 or
   wait for step 5: the provider routes are consumers.
2. Publish the new finalized `ArtifactReleaseV1` records for the new
   Registry (and Rent, if it moves) deployments.
3. Upgrade the programs: the five-role set per the charter, and the
   Registry (`d6e43b11`'s ELF). From this instant, routes 1-8 refuse by
   name. **The window is structural** — first admission can only hash
   deployed bytes, so the ceremony cannot precede the upgrade. Script
   steps 3-5 as one batch; the window is minutes on devnet.
4. **The ceremony** (§5): create V2. Routes 1-6 un-refuse (Core reads
   V2); routes 7-8 un-refuse because resolution-proof redeployed with the
   V2 read in step 3. **Resolution-proof joins the cut by this ruling**:
   it is source-unmoved at HEAD, but its two provider routes are the
   resolve path of every market — leaving them reading V1 makes every
   cohort-9 market unresolvable, which is not a window, it is a broken
   protocol. The only source change it takes is the V2 read.
   Consequence, stated because ember asked for this class of fact to be
   stated rather than discovered: **Resolution flips from unmoved to
   MOVED in the 8→9 declaration** (its consent slot becomes the
   authority-signed arm), and Custody becomes the hop's likely sole
   unmoved role — the role that exercises `d6e43b11`'s
   System-Program-exempt arm on the first real declaration.
5. Declarations: `DeclareSuccessor` for 7→8 (the unmoved-resolution hop
   the fixed conjuncts now admit) and for 8→9 — gate 6's durable record,
   on chain, walkable.
6. Activate cohort-9's release set; hop the open markets; refound; gate
   9's acceptance life (found → … → physical close) — which exercises
   routes 1-5 against V2 as a matter of course.

Gate 9 is the reason this ruling gates the cut: after step 3 there is no
founding and no retirement until step 4 exists, so **the cut cannot pass
its own acceptance gate without the succession mechanism landed**.

## 7. Sizing, honestly

| Piece | Where | Size |
|---|---|---|
| V2 contract type + domain + hostile decode + roundtrip/refusal tests | `crates/dclutch-release-set-contract` | ~half day |
| Ceremony route + dispatcher arm + conjuncts 2-7 | `core-sbf/src/infrastructure.rs`, `lib.rs`, `frame.rs` (Initialize frame gains V1-profile + predecessor-record + consent-signer slots) | ~1 day |
| Reader flip to V2 (6 Core sites via the one `authenticate_profile`; 2 resolution-proof sites) | `infrastructure.rs:122-181`, `provider_instruction_v3.rs`, `provider_transport_v3.rs` | ~half day |
| Host tooling: ceremony subcommand (keypair-env deployer + Core authority, `--i-mean-devnet` class, simulation key-free per LINEAGE-WRITER's pattern) + the 6 host readers | operator/release-tool/retirement/product-runtime/provider-transport/bootstrap | ~1 day |
| Real-SVM campaign (§8) | `dclutch-svm-harness` + program-tests | ~1 day |
| Docs: this ruling, P-008, 0012 correction note, `CarryForward` whitelist re-annotation | docs + `upgrade.rs` comments | ~half day |

**~3-4 lane-days of strong-lane work, on the cut's critical path before
gate 9.** The map settled the former unknown: resolution-proof was NOT
otherwise redeploying (source-unmoved at HEAD), so this ruling grows the
cut by one resolution-proof build+deploy whose only source change is the
V2 read — small in code, but it flips Resolution to a moved role in the
8→9 declaration (consent-signed arm). The declaration tooling handles
both arms; the cut doc must record, from measured ELF digests, which arm
each role used. Ceremony CU note: `process_initialize` already performs
two first-admission full-ELF hashes in one transaction on devnet, so the
V2 ceremony fits by the same precedent; if the budget presses, the
builder may take an UNMOVED binding via `AlreadyPinned` against the V1
profile authenticated in the same frame — admission-sound because V1's
first admission hashed those exact bytes and its pin still holds for an
unmoved deployment.

## 8. Red-proof obligations on the builder (all both-ways, real ELFs)

1. **The brick, reproduced first.** On a V1-only world: upgrade the
   Registry ELF; prove routes 1-8 refuse (`ReleaseSuperseded` /
   `ResolutionRelease` by name); prove the two escapes dead (new record →
   `Infrastructure` at the content pin; re-initialize →
   `Infrastructure` at the vacancy check). This is the measurement that
   makes everything after it a fix rather than a feature.
2. **The ceremony admits exactly the ceremony.** The full §5 conjunct
   set green once; then one hostile per conjunct: unsigned Core
   authority; absent V1; registry program moved (conjunct 3, by name);
   successor slot ≤ predecessor slot (conjunct 4); moved binding without
   the predecessor authority's signature (conjunct 5); occupied V2 PDA
   (conjunct 6, the no-fork); a doctored persisted image (conjunct 7).
   No vacuous hostiles: each must fail for ITS reason (assert the code),
   per LINEAGE-WRITER's own lesson (its campaign caught two hostiles that
   landed what they meant to refuse).
3. **V2 un-refuses the world.** Post-ceremony: found, all three retire
   entries, the permit refund, and both provider routes land against V2;
   the V1 account is byte-identical before/after everything above
   (write-once preserved — the CloseSeal bar).
4. **Mutation floor.** Kill at least: conjunct-3 equality dropped;
   conjunct-4 inequality inverted; conjunct-5 signer bit unchecked;
   conjunct-6 vacancy check dropped. LINEAGE-WRITER named skipped
   mutation testing as debt and LINEAGE-FIX paid it; this route starts
   paid.
5. **The declaration rides.** The 7→8 declaration (unmoved arm) and the
   8→9 declaration land on the upgraded Registry in the same campaign,
   and `walk_lineage_to(destination)` — never `is_already_current`, per
   the live-measured trap in `RELEASE_SET_COHORT_LINEAGE_2026_08_31.md`
   — follows both hops.

## 9. What needs ember (flag, not blocker — full-autonomy directive in force)

1. **Conjunct 5's signer set.** The ceremony as ruled requires BOTH
   Core's upgrade authority AND the predecessor registry release's bound
   authority. On today's devnet these are the same key, so the cut feels
   nothing; at mainnet key separation this is the difference between
   "the infrastructure operator can rotate the Registry under a willing
   Core" and "both estates must consent." Ruled to require both (it is
   free now and strictly stronger); ember may want to weaken to
   Core-only or strengthen further (e.g. a time-lock) before mainnet.
2. **Rent's pent-up +451/−24.** The mechanism unbricks Rent; shipping its
   accumulated changes (checkpoint aggregate retirement, capped
   sweep-crank share) in cohort-9's ceremony versus a later one is a
   cut-scope call the orchestrator can make, but it is new devnet surface
   ember has not seen listed before. Ruled here: NOT in cohort-9 (the cut
   is already carrying its maximum; Rent's binding stays byte-identical
   in V2), listed so the deferral is a decision rather than an accident.

## 10. Verdicts

1. **LINEAGE-FIX's discovery: CONFIRMED in every particular.** Write-once
   by vacancy with no second write route (§2); content pin including the
   slot with both escapes structurally dead (§2); the exact consumer set
   is the eight routes of §3 and nothing else — Trading, Claims, Custody,
   Registry, Rent, Dealer, Product-Runtime programs and `DeclareSuccessor`
   read the profile nowhere.
2. **LINEAGE-FIX's flagged inference: VERIFIED, and it bites cohort-9
   directly.** At HEAD vs the cohort-8 pin, Custody and Resolution are
   source-unmoved — the 8→9 hop has unmoved roles, so `d6e43b11`'s
   conjunct fix is needed for cohort-9's OWN declaration, not only the
   historical 7→8 (whose unmoved resolution role it was found on). Under
   this ruling Resolution flips to moved (V2 read, §6), leaving Custody
   as the expected unmoved role; final arms come from measured ELF
   digests at the cut (the basis-closure pricing disagrees with the
   symbol-level analysis, and only an ELF diff settles it).
3. **The undocumented constraint, named.** "The Registry is carried
   forward every cohort" was load-bearing because the profile made any
   Registry (or Rent) upgrade a protocol-wide brick with no repair. It
   was encoded only as a tooling whitelist
   (`upgrade.rs:2339-2351,2535-2539`) and as decision 0012's unexamined
   sixth admission site. It is now P-008, with a lifting mechanism.
4. **The ruling: profile succession (option b), in the cohort-9 cut,**
   with the §5 ceremony, V2-only reads, the §6 ordering, and §8's
   red-proof obligations binding on the builder. Slot-tolerance (c) is
   refused as unsound against I2/I3; deferral (a) is the fallback, not
   the plan; registry-as-a-role (d) is the named end-state candidate
   awaiting its own adjudication.
5. **The cut cannot pass gate 9 without this mechanism** — after the
   Registry upgrade there is no founding and no retirement until the
   ceremony exists (§6). The Registry upgrade and the profile succession
   are one unit of work, and the cut doc should treat them as one gate.

## 11. P-008 — the row this discovery deserves

Appended to `docs/OMISSION_INDEX.md` (see that file): the write-once
infrastructure profile made the Registry and Rent structurally
non-upgradable while every consumer's refusal message promised a
re-release remedy that did not exist for it; the carry-forward that
masked this for eight cohorts was load-bearing and encoded only as a
tooling whitelist. Classification: **likely scar, lifted by this
ruling's V2 succession**; closure = the ceremony route on chain with §8's
evidence, and decision 0012's remedy paragraph corrected to name the
profile's succession as the sixth admission site's remedy.
