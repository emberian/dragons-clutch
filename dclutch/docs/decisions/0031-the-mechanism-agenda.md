# Decision 0031: the mechanism agenda — six directions explored as designs, and cohort-17 is the mechanism cohort

Status: **PROVISIONAL — the agenda, its design-first form and its order ruled by
the orchestrator on 2026-09-04 under ember's standing goal and under ember's own
instruction to explore all six, and reversible direction by direction at the
cost §7 states**. The six designs all exist and no program has moved; the entry
that opened the agenda is `GOAL.md:4670-4679`, and the closes are `:4689-4699`,
`:4715-4726`, `:4756-4780` and `:4786-4794`.

## 1. The question

On 2026-09-04 the orchestrator put six mechanism directions to ember, ranked by
property per unit of change (`GOAL.md:4671-4679`):

1. the frequent batch auction as the clearing spine of every family;
2. joint clearing of all outcomes with complete-set minting inside the batch;
3. the Dealer as a bounded-loss scoring-rule participant;
4. resolution by observed median over an ensemble of declared sources, the
   funded ladder as its fallback;
5. a founder bond paid to holders on exhaustion;
6. conditional and product markets as the combinatorial layer.

The question was not which one to build. Every one of them changes a rule the
tree already executes — the clearing rule, the resolution rule, or the founding
— and the tree had a *written design* for none of them. So the question was
whether to explore them at all, in what form, and in what order, given that
cohort-16 is in flight with the founding changes of decisions 0025 and 0027 and
must not carry a layout change under it.

## 2. The ruling

**All six, explored as designs first, and cohort-17 is where they land.**

1. **Each direction gets a note, a Lean module, a price and a build list**, and
   **no program moves under cohort-16**. A direction with a note and no Lean
   statement is a direction nobody has checked; a direction with Lean and no
   price is one nobody can schedule.
2. **Cohort-17 is the mechanism cohort.** Every one of the six names it, for the
   same reason: each is a record-layout or founding change, and cohort-16 is
   carrying 0025/0027 (`MECHANISM_JOINT_CLEARING:456-466`,
   `MECHANISM_ENSEMBLE_RESOLUTION:489-498`, `MECHANISM_FOUNDER_BOND:444-455`,
   `MECHANISM_CONDITIONAL_MARKETS:625-631`, `MECHANISM_BATCH_SPINE:567`,
   `MECHANISM_SCORING_DEALER:517`).
3. **The order inside cohort-17 is joint clearing, then the scoring Dealer, then
   the ensemble.** The clearing rule first because the Dealer's participation is
   defined against it (`SCORING_DEALER:517` — the schedule is placed as signed
   limits into JOINT-CLEARING's order record, and the joint optimality statement
   across the two modules is owed once the clearing note lands, `:544-547`); the
   Dealer second because it is the first participant the certified batch has to
   admit; the ensemble third because it is orthogonal to both and its lift is a
   measurement, not a dependency (`ENSEMBLE:408-412`). The batch spine, the bond
   and the conditional layer are **designed and not scheduled** — the spine
   because its commitment deletes routes and is ember's to make
   (`BATCH_SPINE:661-679`), the bond and the conditional layer because they ride
   whichever cohort takes their founding change.

**The six, as they now stand.**

| # | direction | note | Lean | commit | the headline price |
| --- | --- | --- | --- | --- | --- |
| 1 | the batch spine | `docs/design/MECHANISM_BATCH_SPINE_2026_09_04.md` | none new — the existing modules move (`§3.2`) | `2fbd73474` | 3.70 M CU per order at M=136; 6.4× a bilateral fill, 5.2× with the output page |
| 2 | joint clearing | `docs/design/MECHANISM_JOINT_CLEARING_2026_09_04.md` | `formal/dclutch-semantics/DClutchSemantics/JointClearingV1.lean`, 44 theorems, zero sorry | `554a29119` | ≈ 9.4 M CU per batch at N=2 → ≈ 395 M at N=258; no single transaction near the ceiling; K ≤ 60 |
| 3 | the scoring Dealer | `docs/design/MECHANISM_SCORING_DEALER_2026_09_04.md` | `.../ScoringRuleV1.lean` | `3bf1905a7`, `a16c06d33` | the participation check at 39k / 46k / 93k CU for K = 2/3/5, against the 131,790 selector-9 evaluation it replaces |
| 4 | the ensemble | `docs/design/MECHANISM_ENSEMBLE_RESOLUTION_2026_09_04.md` | `.../EnsembleResolutionV1.lean`, 768 lines, zero sorry | `ff4f3b142` | +0.0065 SOL prepay at k=3, +0.0130 at k=5, nearly all of it rent that returns |
| 5 | the founder bond | `docs/design/MECHANISM_FOUNDER_BOND_2026_09_04.md` | `.../FounderBondV1.lean`, 34 theorems, zero sorry | `86d38a203`, `9365be226` | 4,031,465 lamports on cohort-15's numbers — 0.004 SOL, 1.75% of a market lane, returned in full on an honest terminal |
| 6 | conditional markets | `docs/design/MECHANISM_CONDITIONAL_MARKETS_2026_09_04.md` | `.../ConditionalMarketV1.lean`, 50 theorems, zero sorry | `4b15cf69a` | K ≤ 60 (7×8 fits, 8×8 refused), heap ≈ 30 |

Five new Lean modules, all imported by the root (`b31b35a21`), with four stated
`sorry` in `ScoringRuleV1` and none anywhere else. The batch spine is the one
direction with no new module by construction: it is a migration of
`GeneralClearing` and its siblings, and the note says which ones move
(`BATCH_SPINE:495-511`).

## 3. Ember's words

Ember, to the orchestrator on 2026-09-04, on the six-direction agenda:

> Sounds beyond excellent, I think we need to explore all these directions to be
> honest :)

The tree's narrative carries the short form in the entry's own heading —
*"we need to explore all these directions"* (`GOAL.md:4670`) — and this record
carries the sentence.

Two obligations come out of it, and they are not the same one:

- **All of them.** The ranking in §1 is a ranking, not a filter. The lowest-ranked
  direction — conditional markets — is the one that turned out to prove the most
  (50 theorems, and *a conditional market IS the product market's row projection*
  as a theorem rather than an analogy). A ruling that had taken the top two would
  have missed it.
- **Explore, not build.** "Explore" is why §2 item 1 is design-first and why no
  program moved. A direction that arrives as a program arrives without the thing
  that makes it reversible: a written alternative, priced.

## 4. The lanes

Six design lanes, all opened and closed on 2026-09-04, none of them touching a
program:

- **Wave one** (`GOAL.md:4680-4682`): BATCH-SPINE, JOINT-CLEARING, SCORING-DEALER.
- **Wave two** (`GOAL.md:4751-4755`), started once the clearing rule was stated:
  ENSEMBLE, BOND, CONDITIONAL.

No implementation lane is chartered by this record. The three rulings the
joint-clearing note owed are decision 0032, the bond's mode is decision 0033, and
the ensemble's parameters are decision 0034; the conditional layer's flagship is
the addendum to decision 0029, and it waits on ember.

## 5. The hostiles and laws that guard it

**A design-first agenda has one characteristic failure — a note nobody can
falsify — and the guards against it are structural.**

- **The Lean modules are imported by the root** (`b31b35a21`, 145 jobs green), so
  a mechanism module that stops building is a red tree rather than an island. This
  is exactly the failure mode decision 0029 item 1 names for `crate::series`: a
  thing with no non-test consumer is invisible to every gate.
- **Each note names its hostiles by route and discriminant**, not as prose: the
  joint clearing's seven with the four new `RuntimeVerifyErrorV2` codes they need
  (`JOINT_CLEARING:198-212`), the ensemble's five (`ENSEMBLE:346-382`), the bond's
  eight with `FounderBondUnderfunded` red at founding
  (`FOUNDER_BOND:337-354`), the conditional layer's at `§4.4`.
- **Each note says which census law moves.** The joint clearing is L1 on the
  batch's Settlement compartment and L8's declared `HoardPrincipal` delta with
  complementary slackness as a new per-batch form of L4; the bond is L7 and no
  compartment at all, *because the bond never holds an atom*; the ensemble is
  L6/L7/L8 with a table naming where each lamport ends. The census
  (`tools/gauntlet/journey/src/ledger.rs:1004-1012`) stays the standing
  instrument and no note asks for a tenth compartment — which C-11 reserves to
  ember by name.
- **The sorries are stated.** Four in `ScoringRuleV1`, each named for its reason
  (the one-sided and two-sided approximation bounds), with `table_power_bound`
  landed at `a16c06d33` as the fact they compose from. A design whose gaps are
  named is falsifiable; one whose gaps are not is a claim.

**And the strongest evidence that design-first was the right form: the six notes
found six live defects in the tree we already have**, none of which a build lane
was looking for.

1. **General as built is ONE call auction per Market** — the selection is seeded
   by root alone and nothing writes it `Open` again (`BATCH_SPINE`, read not
   asserted; confirmed by PROGRAMS-16C's `OpenBatch → CloseBatch → OpenBatch` run).
2. **Early freeze was live** — `Freeze` carried no slot conjunct. PROGRAMS-16D
   landed the deadline Lean-first the same day (`9653ef363`), and closed a hole on
   the way: the evidence was caller-supplied, so any long-closed batch satisfied it.
3. **A net seller has no price floor** (`runtime_verify.rs:1242`): `Order` bounds
   only the debit side, so a Direct sell ticket's limit has nowhere to go.
4. **A candidate can omit an order** — the shipping verifier does not read
   unfilled orders, so completeness is unchecked.
5. **`initialize_certificate_at_kind` accepts an already-owned well-formed seat**
   (`programs/dclutch-resolution-proof-sbf/src/relay_transport_v1.rs:2184-2191`); write-once comes from the terminal write's
   all-zero conjunct, which any fragment route must keep.
6. **The price series is not a durable chain fact** — the price vector lives only
   in the candidate account `CloseCandidate` reclaims.

And one producer found: the ensemble's push-route fragment mode **is** the
recovery capture's owed producer, which RECOVERY-2 took up the same afternoon.

## 6. What was given up, named

**Cohort-16 gets nothing from any of this.** That is the design-first rule's exact
cost, and it was paid deliberately: six directions, roughly 3,000 lines of note and
4,000 lines of Lean, and not one atom moves differently on any chain until
cohort-17.

**Three of six are scheduled and three are not.** Ordering joint clearing, the
Dealer and the ensemble into cohort-17 leaves the spine, the bond and the
conditional layer designed-but-unscheduled, which is the state decision 0029's
§3 warns about under *"underdesigned"* run in reverse: a design with no cohort is
a design that ages against a moving tree. Each of the three carries a note saying
what its cohort boundary is, which is the mitigation and not a fix.

**The batch spine's commitment is deferred to ember and it is the largest one.**
*"Every transfer of claims between two parties is a verified General candidate —
including the bilateral one — and a resting order rests in a batch, never in a
public pool of bearer tickets"* (`BATCH_SPINE:664-670`) deletes 13 routes, the
registered Direct branch, the Dealer's seven-step checkpoint chain and the Series
shadow. It is not ruled here and this record does not rule it.

**A batch costs five to six times the compute of a bilateral fill per order**, so
the spine's frequency on Solana is tens of seconds. That is right for a forecast
series and wrong for a latency product — which the project refused on its first
day, so the cost is consistent with the product rather than a surprise, but it is
still a cost.

## 7. The cost of reversal

**The agenda as a whole:** the notes and the Lean stay whatever ember rules; a
reversal costs no deletion, because nothing shipped. This is the direct benefit of
the design-first form and the reason it is the ruling.

**Direction by direction, if ember rules one out:**

- **Batch spine.** Reversing costs nothing today and everything later: the routes
  it would delete keep accruing consumers, and the note's own disposition table
  (156 survive / 6 amended / 3 participant / 13 delete) is a snapshot that gets
  more expensive to re-derive each cohort.
- **Joint clearing.** Losing it keeps *"best valid submitted candidate"* as the
  strongest sentence the tree may say — `AGENTS.md` forbids *"optimal clearing"*
  without a checked certificate, and this is the certificate. It also leaves the
  seller with no price floor and a candidate free to omit an order.
- **The scoring Dealer.** Losing it keeps the Dealer's selector-9 evaluation at
  131,790 CU where the participation check is 39k–93k, and leaves *bounded loss*
  as an aspiration rather than `bounded_loss`.
- **The ensemble.** Losing it leaves resolution single-sourced, which is exactly
  the shape cohort-13's outage exploited — one Pyth receiver redeploy under every
  market's release pin.
- **The founder bond.** Losing it leaves the oracle choice priced at zero, which
  decision 0025 explicitly did not fix: 0025 stops an outage from *paying* the
  founder and says nothing about what it costs them.
- **Conditional markets.** Losing it costs the combinatorial layer and the one
  product the mainnet-state relay was built for; the theorem that a conditional
  market is a product market's row projection would have to be re-derived.

**Reversing the ORDER** is the cheapest reversal in this record and the one most
likely to be wanted: cohort-17's three are independent as builds, and only the
Dealer's dependency on the clearing rule's order record is real.

## Evidence pointers

`GOAL.md:4670-4699`, `:4714-4738`, `:4751-4780`, `:4786-4799`;
`docs/design/MECHANISM_BATCH_SPINE_2026_09_04.md` (esp. `§3.1`, `§3.2`, `§4`, `§5`);
`docs/design/MECHANISM_JOINT_CLEARING_2026_09_04.md` (esp. `§1.3`, `§1.5`, `§2`, `§3.3`, `§4.4`, `§5`);
`docs/design/MECHANISM_SCORING_DEALER_2026_09_04.md` (esp. `§1.2`, `§3`, `§6`, `§8`);
`docs/design/MECHANISM_ENSEMBLE_RESOLUTION_2026_09_04.md` (esp. `§4`, `§5`, `§6`, `§7.3`);
`docs/design/MECHANISM_FOUNDER_BOND_2026_09_04.md` (esp. `§1`, `§1.1`, `§5.1`, `§8`);
`docs/design/MECHANISM_CONDITIONAL_MARKETS_2026_09_04.md` (esp. `§1.3`, `§5`, `§8`, `§9`);
`formal/dclutch-semantics/DClutchSemantics/JointClearingV1.lean`,
`ScoringRuleV1.lean`, `EnsembleResolutionV1.lean`, `FounderBondV1.lean`,
`ConditionalMarketV1.lean`;
`tools/gauntlet/journey/src/ledger.rs:1004-1012`;
`docs/decisions/0024-sustainable-economics-and-a-governable-parameter-surface.md`,
`0025`, `0027`, `0029` items 1 and 7;
commits `2fbd73474`, `554a29119`, `3bf1905a7`, `a16c06d33`, `ff4f3b142`,
`86d38a203`, `9365be226`, `4b15cf69a`, `b31b35a21`.
