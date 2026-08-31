# Cohort-9 plan review — 2026-08-31 (C9-REVIEW, Fable)

Charter (ember): "really evaluate and exhaustively take apart the plan and
make sure it's as good as can be — at least for the creative/challenging
aspects." Reviewed before CLOSEMAKER / SPLINE-WIRE / ZEROBUMP build; the two
proceeding items (KAPPA-CAP, FRACCHECK-3) reviewed for interaction only —
**one binding interaction found (KAPPA-CAP, §4)**. Method: four ground-truth
investigation lanes over tree/Lean/jobs evidence, with the three heaviest
claims re-verified line-by-line by the reviewer (the begin-retiring gate vs
Lean ordering, `close_maker_replay_v2`'s full body, the admission cascade's
caller set). Where the charter's premise was wrong it is flagged
PREMISE-FALSE inline.

## Verdicts

| # | Item | Verdict |
|---|---|---|
| 1 | CloseMakerReplay end-to-end | **BUILD AMENDED** — 4 amendments; the sizing missed piece 0 (reachability) and the fee gate entirely |
| 2 | ZeroBump seal recovery | **BUILD AMENDED** — rider-only on item 1's Trading upgrade, probe-gated; never a standalone upgrade; burn rejected |
| 3 | Spline wire commit | **BUILD AMENDED** — §9 commit 9 (DCLTPGT1 + founding conjunct) must ride the wire commit; overflow arm (a) mandatory |
| 4 | KAPPA-CAP (proceeding) | ~~**BUILD AMENDED** — must carry RECORDS-MIGRATE row (b) + the Found-frame +6 per the 18:55 ruling; its CoreState widening rides THE cut~~ **— SUPERSEDED, see the correction in §4: the cap, the Found-frame +6 and the checks are all already at HEAD, row (b) is superseded, and KAPPA-CAP moves no wire, so it rides no cut.** Curvature verdict unchanged and accepted |
| 5 | FRACCHECK-3 (proceeding) | **BUILD AS PLANNED** — one ELF (claims-sbf), wire-additive; one pre-cut check (C0 weld confirmed landed) |

**One cut, not two.** Every item moves ELFs and the cut is the restrand
unit; splitting multiplies restrands for nothing. The only defensible second
cut is General's fourteen-cut, already gated on weeks-class runtime
dispatch — it must not hold cohort-9. Gate list in §7.

## 1. CloseMakerReplay — BUILD AMENDED

Wall-22 premises verified true: encoder-only (two-line refusal,
`programs/dclutch-trading-sbf/src/hot_v3.rs:3961-3963`; hard
`[CapabilityProgramSetEntryV2; 4]` table,
`crates/dclutch-direct-codec/src/program_set_v4.rs:471`, "exact four-entry"
validator :167); the five zero-count gates split 3 begin-retiring (native
`direct_begin_retiring_v1.rs:518`, operator :684/:865) + 2 physical-close
(`terminal_retirement_v1.rs:1136`, `native_close_bundle_v1.rs:409`), and
**all five decode the live root bytes — no cache survives a decrement**;
`selected_release_set` is setterless (comparisons only).

**Amendment 1 — piece 0, the missed reachability.** PREMISE-INCOMPLETE: the
spec'd close is unreachable under the chain's own gates. Lean orders
`beginRetiring` (phase-only, `DirectSuccessor.lean:413-416`) →
`closeMaker` (requires `.retiring`, :430-447) → `rootClosable`
(retiring ∧ count = 0, :472-475); the chain's three begin-retiring sites
demand count == 0 **before** Retiring, so a filled market can never reach
the phase where the close is legal. Built as sized, entry 11 is a second
dead instruction. Piece 0: relax the three begin-retiring count gates to
Lean's ordering, keeping both close-time gates. Same cut, no extra
redigest, but real design + conformance work. Semantics: makers wind down
*inside* Retiring — what the Lean comment already intends, and safe because
`consume_nonce_v2` refuses non-Open (`successor.rs:1146-1148`): Retiring
already stops trading.

**Amendment 2 — the fee-debt gate (the charter's question, answered).**
The fee stack is on main, not branch-only (`a0b1f4cb`/`a7d50d3a` are
ancestors of HEAD; the first public trade settled `fee_owed` 9,950→0
permissionlessly). `close_maker_replay_v2` **never reads `fee_owed`**
(`successor.rs:2479-2510`, verified line-by-line); the root carries only
`{phase, open_maker_root_count}` (:588-591), so the replay account is the
SOLE record of the receivable — close with debt outstanding erases it with
no residue, and tx2 becomes impossible afterward
(`fee_settlement_v1.rs:398` needs the replay's projection). Were close ever
legal in Open, close+recreate would launder the E5 lockout outright (the
Vacant creation arm mints `fee_owed: 0`, `successor.rs:1150-1181`).
Required: **close refuses `fee_owed != 0`.** This strands nothing — tx2 is
deliberately phase-free (`FEE_SECOND_TRANSACTION_V1.md:855-858`), so
settle-then-close is always available in Retiring. The Lean `MakerRoot` has
**no `feeOwed` field** — the close spec predates FEE-TX2. Order: amend Lean
first (add `feeOwed`, gate `closeMaker` on `feeOwed = 0`, re-prove
`maker_close_count_conserved` / `maker_close_refund_conserved` — both
currently real, non-vacuous — plus a fee-conservation theorem), then write
the chain code from the amended spec.

**Amendment 3 — rent: mostly already ruled.** Principal → the immutably
recorded `rent_owner` is IN the landed spec (`successor.rs:751/:1154`;
Lean `MakerClosePlan` + proved refund conservation :459-468) — the wall-10
chain-writes-the-refund-wallet pattern, not an open ruling. Open: only the
`unclassified_donation` slice. Recommend a capped permissionless-closer
reward carved from the donation slice alone (funded-crank shape; E3
correctly narrowed — the principal is the maker's own money, unlike the
orphan seal's). Ruling 1 in §8, record before code.

**Amendment 4 — permissionless, no signer.** Consistent with the family
(native begin-retiring refuses ANY signer, `direct_begin_retiring_v1.rs:91`;
Lean `closeMaker` takes no authority). With the count, fee, phase, and
rent-to-owner gates, a closer cannot harm the maker; maker-only close would
strand absent makers' replays — the exact stranding shape E3 rejected.

**Blast radius: as chartered, and the preservation worry dissolves.**
Markets 21/22 (including THE TRADED MARKET) are *already* permanently
unretirable under the four-entry set; the cut cannot rescue them (no
setter) and does not worsen them. No preservation lane: history lives on
chain + the archives (`/Users/ember/jobs/dclutch-fill2/retire/`, the 82-act
life table), and `RELEASE_LINEAGE_MIGRATION_V1`'s lineage record is keyed
by predecessor and **authorable retroactively** — a future migration cohort
can still rescue them. Cheap insurance rides the cut: record the
cohort-8→9 mapping durably (§7 gate 6).

## 2. ZeroBump — BUILD AMENDED (rider-only)

PREMISE-IMPRECISE: 7,628,160 lamports is the account's rent-exempt minimum
(6,960 × 1,096 exactly), not market stake. Not a writer bug: the bump byte
at offset 20 was reserved-zero in the prior layout
(`capability-seal-contract/src/lib.rs:66-69`); `encode` refuses bump 0
(:541-542) and derivation walks 255→1 (:186) — **the class is closed at
exactly one member (`6hDpsgAo…`), so deferral is free forever.**

The clean shaping EXISTS and write-once survives it: a `decode_defunct` arm
pinning every field canonical (length/magic/schema/profile/reserved/rows)
while REQUIRING bump == 0; address authority via caller-supplied bump
candidate + `create_program_address(body seeds, candidate) == seal.key`;
every other landed CloseSeal conjunct verbatim — closer shape, Trading-owned
968 B, registry-from-body, and the live-release refusal
(`hot_v3/seal.rs:344-352`), which is where the close's soundness actually
lives (P-006's own closure argument), not decode strictness. Disjointness
from every well-formed seal, present or future, is provable three
independent ways. The broad "any decode failure ⇒ closable" predicate is
REJECTED (the profile gate exists to stop exactly that, seal.rs:250-253).

Conditions: **(a) rider-only** — same Trading ELF as item 1. Any Trading
upgrade defuncts cohort-8's live seal and forces a seal re-write before the
refounded market trades; a standalone upgrade for 0.0076 SOL of rent is
never worth that. **(b) probe-gated** — free host-side pre-build probe
(`decode_defunct` + address reproduction + non-live release seed against
the fetched bytes); if it fails, the item is DOA and neglect is forced.
Burn-by-neglect is strictly dominated by defer-as-rider. Take P-007 (Lean
byte-identical seal-layout emission, wire-free) as the de-risking rider.
Bonus the cut yields free: cohort-8's seal becomes a *normal-path* stranded
seal — CloseSeal's first end-to-end devnet exercise needs no tolerant arm.

## 3. Spline wire — BUILD AMENDED

More is landed than the plan assumed: the refused-everywhere enum
(`f8701b6b`), the `SPLINE_EVALUATOR_RELEASED_V3 = false` const seam
(`spline_admission_v3.rs:61`), the full admission cascade (:100-131), the
953-line de Boor port + 9/9 differential reproducing all 19 Lean corpus
cases (`aac98afd`/`ffdc63f1`), corpus script clean. Doc staleness: §1.6.1's
"unguarded" claim was corrected in-tree; §6.1's CoreState figure is stale
(368 bytes, bump tail, 5 reserved).

**Amendment 1 — commit 9 is a safety conjunct of the wire commit, not
trailing work.** `admit_basis_selection_v3` has **no production caller**
(verified: doc-comment references only). Founding authenticates via
`authenticate_product_basis_v3` (found.rs:504, founding_v5.rs:134,
series_consume.rs:960, generic_founding_v1.rs:1043), which refuses tag 3 at
decode — that refusal IS the price gate today. The moment decode accepts
tag 3, the refusal must move into an admission call founding actually
makes, against a real DCLTPGT1 record — same commit-set, or a spline market
founds with no no-arbitrage gate. (A live mirror-disease instance: the
cascade is green in tests and called by nothing.) The atomic unit:
decode-accept + offset-18 reserved spend + schema-id bump (flipping
`a_record_whose_kind_byte_is_three_is_refused` intentionally) +
knot-ordering relaxation + the 13/10/3 sites + the off-chain twin +
DCLTPGT1 + founding conjunct + cumulative-floor blessed with
floor-plus-complement DELETED (the 76e2ca3f ruling — both still ship
today) + the seam flip LAST.

**Amendment 2 — the overflow envelope: fail-closed arithmetically, a trap
operationally.** Every op is checked → `ArithmeticOverflow` (no wrong
number possible), but the coordinate is scaled BEFORE clamping
(`spline_eval_v3.rs:167-172`) and the cascade checks no magnitude envelope,
so an admitted basis can first refuse at **settlement** — E5-class
principal stranding, wall-22's own lesson class. Required in the cut, arm
(a): saturate the coordinate against the knot range before scaling + an
admission-time envelope conjunct over the founding-fixed quantities.
SignedU256 then trails honestly. Shipping with neither: refused.

Can trail: TS/SDK decoders (needed before the first spline FOUNDING, not
the cut — clients throw on unknown tags, chain-sound), kernel deletion
hygiene. Gates it adds: hot-CU re-measure before the seam flips (HEAPRED:
the tier's figures are +35,127 CU high); the 22 hostiles + 4 byte guards
run in the cut campaign — no CI runs them.

## 4. KAPPA-CAP — the interaction, found and binding

Curvature does NOT break κ: the predicate `principal · d ≤ n · floor`
(`principal_capacity_v1.rs:1-56`) is shape-independent — premised on
capturing at most the whole Hoard, already worst-case over payoff geometry,
and maximal-steepness payoffs (categorical steps) already ship. **No
curvature-aware bound is needed before tag-3.**

The binding interaction is batching, twice over: (i) the cap is ruled onto
CoreState ("the cap is part of the wire break", 18:55 RECORDS-MIGRATE row
(a)); CoreState's bump tail has 5 reserved bytes, so the cap moves
STATE_BYTES and re-pins ~25 length-refusal sites — **if that widening and
the spline/selector cut land separately, every market restrands twice**;
(ii) the 18:55 ruling batched rows (a) and (b) —
`SourceCapacityProfileV1.floor_content_id` + the Found-frame +6 accounts
ride the same migration — so shipping (a) alone splits a ruled batch and
forces a second CoreState-adjacent break later. The lane branch is still
empty: **route this to the KAPPA lane before it writes.** Also standing:
a founding-only check is not a cap — it lands at founding AND split.

> **CORRECTION from the KAPPA lane, 2026-08-31 — §4's batching verdict does
> not bind, because all three batched items are already at HEAD.** The review
> reasoned from the 18:55 RECORDS-MIGRATE ruling rather than from the tree, and
> the tree moved first. Verified item by item:
>
> - **(i) the cap does not move `STATE_BYTES`.** `principalCapSets` is at offset
>   288 *inside* the existing 368 (`MarketCoreAbi.lean:62,121`, committed;
>   `state_schema_width` proven). §4 half-caught this — it flags §6.1's CoreState
>   figure as stale "(368 bytes, bump tail, 5 reserved)" — without noticing the
>   cap already sits inside those bytes. No widening, no re-pinned length sites,
>   no double restrand. Landed `ff008fea`.
> - **the Found-frame +6 is landed too** — `core-sbf/src/frame.rs:33-38`, the
>   three `(raw, staging)` pairs at frame indices 16-21, same commit.
> - **(ii) row (b) is superseded, not owed.** `floor_content_id` was ruled onto
>   `SourceCapacityProfileV1` to stop a caller picking among floors with
>   identical bindings; `SourceMaterialV3.principal_policy =
>   BoundedByFloor(selected_floor_id)` closes that at a content-addressed site
>   and refuses any other floor id, with `ExplicitlyUnbounded` requiring `None`.
>   It is also unbuildable as ruled: the profile's free tail is 16 bytes at
>   offset 96 against the 32 a `ContentId` needs.
>
> **Consequence:** KAPPA-CAP moves no wire, so it is independent of the
> spline/selector cut and splits no batch. What it actually shipped (`0815ca11`)
> is the missing half nobody had named as missing: the bound was enforced but the
> refusal had no NAME at any of the four sites. Curvature §4's first paragraph is
> accepted unchanged and gratefully — that question is closed.
>
> §4's last sentence stands and is worth keeping: a founding-only check is not a
> cap. It already lands at founding and at all three growth routes.

## 5. FRACCHECK-3 — BUILD AS PLANNED

Designed property: exactly one ELF changes (claims-sbf,
`CLAIM_CHECK_COMPACTION_V1.md` §0.7); wire-additive; rides any cut.
Complementary to item 1 — compaction operates in the Retiring phase that
item 1 finally makes reachable for filled markets. One check before
refounding: the C0 weld (§0.5) confirmed landed.

## 6. Missing items — adjudication

| Candidate | Wire-breaking? | Cohort-9? |
|---|---|---|
| Migration story | PREMISE-FALSE that none exists: `RELEASE_LINEAGE_MIGRATION_V1.md` is designed + chartered; impl is weeks-class | **Not in the cut; charter the implementation lane NOW** (it is the standing pre-mainnet blocker with a finished design) + gate 6's lineage mapping + doctrine-check the new close routes against its §5.3 |
| Funded FailNext | ELF-only route addition (`#[cfg(any())]` today), no vocabulary | NO — cuts are the restrand unit; deferral costs no extra restrand. Name the liveness gap in the cut doc for cohort-9-founded markets |
| RECORDS-MIGRATE row (b) + Found-frame +6 | YES | **YES — ruled to ride KAPPA's migration (18:55)** |
| Rest of the RECORDS-MIGRATE batch (FundingStateV1, seal stored-PDA→bumps, profile narrowing, manifest, derivable ids) | YES | Take what is sized, or explicitly re-rule the split (ruling 2, §8) |
| P-007 seal-layout Lean emission | NO (byte-identical) | YES — rider; de-risks item 2 |
| Walls 12/13/16 hardening | NO | YES as campaign tooling for this very cut (evidence author re-derives every address-bearing field from a finalized chain read; wall-12's RULING-AND-COMMAND record-first template as a lane.sh/driver affordance) — tooling, not a gate |
| AlreadyCurrent 5-site dedup | NO (host tools only) | Any time; not cut-coupled |
| Occupancy-walk sub-band table | NO | Design-first; not cut-coupled |
| General 68→131 re-publication | Publication only; nothing deployed | NO — the fourteen-cut, gated on runtime dispatch; must not hold cohort-9 |

## 7. Sequencing — one cut, and its gates

Order inside the cohort (minimizes rework): **(1)** Lean first — amended
`MakerRoot`/`closeMaker` + begin-retiring conformance (item 1's spec is the
only one currently wrong-shaped). **(2)** Parallel branches: piece 0 +
close + entry 11; ZeroBump rider arm; the spline atomic unit; KAPPA's
CoreState widening + row (b). **(3)** Converge; whole-tree build (the
red-umbrella rule); hostiles + byte guards. **(4)** The cut.

Gate list for the cut:

1. Lean green, no sorry: `feeOwed` on `MakerRoot`; `closeMaker` gated;
   both conserved theorems re-proved + a fee-conservation theorem;
   statements read for vacuity.
2. Red-proofs both ways: close refuses `fee_owed != 0`; physical close
   refuses count != 0; begin-retiring now ADMITS count > 0; ZeroBump arm
   refuses every well-formed seal (cohort-8's live seal as the control);
   spline founding refuses an absent/mismatched DCLTPGT1; the admission
   envelope conjunct refuses an out-of-envelope basis.
3. Cumulative-floor blessed, floor-plus-complement deleted; schema-id bump
   in the same commit-set as decode-accept;
   `a_record_whose_kind_byte_is_three_is_refused` flipped intentionally.
4. The 22 hostiles + 4 byte guards run and recorded in the campaign (CI
   does not run them).
5. CU floor gate re-measured post-HEAPRED (+35,127 correction), 32 seeds,
   M-61 reporting.
6. Pre-upgrade sweep: outstanding `fee_owed` settled or recorded;
   redeemable value drained from markets 21/22; life tables archived; the
   cohort-8→9 predecessor mapping recorded durably.
7. No founding lane mid-flight on the old wire (the night ruling's
   scheduling gate — steward's call at cut time).
8. FRACCHECK-3's C0 weld confirmed landed before refounding.
9. Post-cut acceptance: one market driven through the COMPLETE life
   **including retirement** — found → fill → fee settle → begin retiring
   (count > 0) → close replay(s) → zero count → physical close. The first
   retirement ever possible for a filled market is this cut's real
   deliverable. Plus: CloseSeal end-to-end on cohort-8's now-defunct seal
   (normal path), and the ZeroBump close fired on `6hDpsgAo…` (tolerant
   path).

## 8. Rulings needed

1. **The `unclassified_donation` slice on replay close** — payee + capped
   closer reward (recommend: closer keeps the donation slice, capped,
   funded-crank shape; principal untouched). Economic-destination class;
   record before code.
2. **Any split of the 18:55 RECORDS-MIGRATE batch** beyond rows (a)+(b)
   riding this cut needs an explicit re-ruling — the ruling batched them.
3. **The Retiring-semantics amendment** (makers wind down inside Retiring)
   is within cohort-9's granted authority but changes a phase's meaning —
   record it in the cut doc inside ember's veto window.
