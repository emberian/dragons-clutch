# Decision 0029: the product list — what gets built, what is refused, and why each refusal is load-bearing

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible
on request) — the nine items ruled by the orchestrator on 2026-09-04 under
ember's standing goal, answered by ember at 10:15 EDT with "build", and
reversible item by item at the costs §7 states; the tenth item — the
conditional layer's flagship child market — stays OPEN and is ember's**. It was
PROVISIONAL from the ruling until 15:50 EDT, when ember read the docket and
accepted it in conversation without amending it; the confirmation line below is
the whole of what was said. Docket item D7. Ember's amendment is at
`GOAL.md:4655-4656`. These are the queued product questions the tree had been
carrying, several of them for weeks; each was tabled with options and a cost,
and none was an engineering blocker — the ruling decides which lane exists. **A
tenth item — the conditional layer's flagship child market — arrived on
2026-09-04 from decision 0031 and is OPEN, waiting on ember; it is the addendum
at the end of this record.**

**Confirmed, 2026-09-04 15:50 EDT.** Ember, after reading the docket and the
mechanism cohort page:

> you aren't waiting on me for rulings are you? i was reading the docket and
> contemplating it, but overall find your takes reasonable

The orchestrator's reply: nothing was waiting on ember — the rulings were
provisional and already in force, and the lanes had been working under them
since they were made; *"overall find your takes reasonable"* is taken as
confirmation rather than as an invitation to re-argue them; and the one thing
still genuinely ember's is this record's tenth item — the flagship conditional
market's feature gate, its slot and its metric. So the nine items above are
CONFIRMED and no longer PROVISIONAL: accepted in conversation, unamended, and
reversible item by item at the costs §7 states. **The tenth item stays OPEN.**
It is the one thing ember's sentence leaves with ember by name, and the
addendum at the end of this record is unchanged by this confirmation.

## 1. The question

Nine product choices, each independently escalated by the lane that hit it, each
with the tree's own options and costs already written down. They are gathered
here because ruling them one at a time produced a docket nobody could hold in
mind, and because five of the nine are refusals whose value is that they stay
refused.

## 2. The rulings

| # | item | ruling | where the tree states the options |
| --- | --- | --- | --- |
| 1 | **The Series family: A or B** | **BUILD (A).** `crate::series` in Trading (28 files) gets its dispatch and shadow derivation and a C-row, rather than being cut | `GOAL.md:2655-2662` |
| 2 | **Basis ABI: is curvature out of scope permanently?** | **KEEP.** Curvature stays in scope; the basis kernel is not retired | `docs/design/BASIS_ABI_UNIFICATION_V1.md:536-543`; `docs/OMISSION_INDEX.md:53` |
| 3 | **May a Custody reservation carry the Dealer accelerator's candidate as authenticated first-party state?** | **YES**, under the same rule as decision 0022, since the candidate is sealed and PDA-signed | `docs/design/DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md:320-342` |
| 4 | **Width-2 spot band, or a stated proposition?** | **The bare width-2 band is REFUSED.** A proposition with a stated prior is admitted in its place | `docs/design/PACKET_LIMIT_2026_09_01.md:319-330` |
| 5 | **Claims split/merge as user acts** | **BUILD.** The outer route lands; `claims.conserve`/`DCLCNS01` stops being the tree's one orphan magic and `CustodyRequired 0x5006` stops being dead | `GOAL.md:1751-1753`, `:1391-1397` |
| 6 | **Materialize / Dematerialize: delete or drive** | **DELETE.** 1,444 lines of supply-moving codec with zero dependents; C-08's clause is already carried by Reconstitute/UnwrapStructured, so nothing is lost | `GOAL.md:1754-1758` |
| 7 | **Is a K = 2 structured product useful?** | **NO.** K = 3 is the product — and it is **packet-bounded**, see the correction below | `GOAL.md:1759-1760` |
| 8 | **The two binaries named `dclutch`** | **RENAME the TypeScript one.** Near-misses are lethal: `market show` against `markets show`, `--keypair` normal in one and refused by name in the other, env vars differing by one character | `GOAL.md:1764-1767`, `:1500-1502`; correction `:2171-2173` |
| 9 | **Provider breadth** | **ENOUGH this generation.** Pyth plus relayed; no third family. The generic-header refactor is authorized either way | `GOAL.md:1812-1830` |

**A correction this record carries, because the docket got it wrong.** The
docket's D7 line said *"K=3 with the packet wall gone"*. That is false and the
STRUCTURED lane proved it the same day: **only K = 3 exists on the shipping
route, the first K that does not fit is 4, and the wall is the PACKET on common
Hot — 1,269 bytes against 1,232, over by 37** (`GOAL.md:4084-4086`). The wall is
not the RequestProfile (which admits 6), not the Claims-direct frame, and not the
1.4 M CU ceiling (max 770,422 at K=3, unreachable by construction on this
route). `STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2 = 3` is now an asserted ceiling
rather than a placeholder, and the full-width Hot frame carries a packet
equality where it previously had no packet assertion at all. So item 7's ruling
is *K = 3 is the product, packet-bounded* — and an amendment to decision 0011 §3b
is owed for the same reason.

## 3. Ember's amendment

Recorded at `GOAL.md:4655-4656`:

> D7 — build; wants to understand what is refused, underdesigned, and how the
> product becomes the coherently extrapolated vision of itself

Ember's words to the orchestrator were **"build some of these"**, with the
emphasis on understanding the refusals and the underdesign rather than on
ratifying a list. So the amendment adds an obligation the nine rulings did not
carry: **the refusals must be stated as product, not as absences**, and the
underdesigned parts must be named in the order the extrapolation needs them.
Both are written out below because that is what ember asked for, and because a
refusal that lives only as a missing feature is a refusal that gets "fixed" by
whoever arrives next.

### What is refused, and why each refusal is load-bearing

From the explainers page §3, written for ember the same morning:

- **No leverage, no liquidation, no insurance fund.** *"Every claim is backed
  before it exists; the Hoard principal never funds another class, which is the
  law the census proves on every crossing. This is the boundary drawn against
  the competitor's whitepaper: subsume its expressiveness, drop its machinery.
  It is also what makes 'nothing to liquidate' a true sentence on the front
  door."* (`docs/INTENT.md` §4; `tools/gauntlet/journey/src/ledger.rs` L1–L8.)
- **No AMM, no order book, no quote surface.** *"The Dealer is a dealer: it
  makes markets from its own capital under sealed rules and is refused any pool
  or quote behaviour. The kernel may one day admit a formally stated convex
  maker as a venue; dClutch itself is not one."*
- **No token, no staking, no buyback.** *"Refused as economics and as surface:
  do not manufacture a disclaimer for a thing nobody accused you of."*
- **No trusted index or relayer between the chain and the reader.** *"The
  browser derives everything and is a second author; it is how the two-scale
  defect was found."*
- **The three small refusals from the docket.** *"A width-2 spot band is refused
  in favour of a stated proposition with a prior, because a band is an answer
  with the question deleted. K equals 2 is refused because a two-outcome
  Structured claim is a Direct claim wearing a shard. Materialize is deleted
  because 1,444 lines with zero dependents is a promise nobody kept."*

The last three are items 4, 7 and 6 of §2. The first four are older and are not
ruled here; they are recorded because ember asked to see the refusals as one
set, and because three of the nine rulings only make sense against them.

### What is underdesigned, in the order the extrapolation needs it

1. **Parameters that can change.** Fees, the closer's carve, the crank reward
   and the protocol take live as constants — decision 0024's amendment, and the
   reason it exists.
2. **The failure ladder and the escrow** — decisions 0027 and 0025. *"Without
   them the honest path is the only robust one and the product's promise stops
   at the first outage."*
3. **Retirement.** No market has ever completed retirement on any chain. *"A
   product whose markets cannot close leaks rent forever."*
4. **General with more than one action.** *"A batch auction that can only open
   is not a family."*
5. **Series and the mainnet-state relay** — item 1 above. Recurring questions
   about the state of mainnet itself are the product's most distinctive shape
   **and the last rung of every recovery ladder** (decision 0027), which is the
   second and stronger argument for ruling A.
6. **Split and merge as user acts** — item 5 — *"so a holder can reshape a
   position without a counterparty"*, and the two binaries named `dclutch` made
   one (item 8).
7. **Reproducible bytes across hosts.** Nine of ten roles differ between our two
   machines because a prebuilt toolchain embeds its own build path; the REPRO
   lane owns it and the `supported_builders` definition (decision 0026 §4).

## 4. The lanes implementing it

**SERIES** carries item 1 (`GOAL.md:4657-4658`). Item 3 is the DEALER family's,
unblocking the two-transaction Remove. Items 5, 6 and 8 are Claims-route, cut
and CLI work respectively, each small and none blocking. Items 2, 4, 7 and 9 are
rulings that create no lane: they hold a position rather than schedule work —
which is the point of writing them down.

## 5. The hostiles and laws that guard them

- **Item 1** is guarded by the thing that made it a question: an island with no
  non-test consumer and no dispatch is invisible to every gate. Building the
  dispatch puts Series inside the route census, the never-executed count and the
  frame ratchet, where a regression is loud. The shadow program's compiler
  release-id preimage — a certificate field compared by no validator today — has
  to gain a comparer in the same lane or it stays exactly the defect that made
  the island possible.
- **Item 2** is guarded by 221 sorry-free theorems across 5,181 lines of Lean and
  by the no-arbitrage price gate, which is the only consumer that would notice
  the kernel's absence.
- **Item 3** rides decision 0022's rule and its hostiles: the callee verifies the
  signer's derivation, and *which program holds a role is still checked, because
  any program can sign a PDA under itself*.
- **Item 4** is guarded at authoring: the partition gate and the wizard refuse
  the bare band, so the refusal is a refusal in code rather than a style note.
- **Item 6** is guarded by the census: deleting the codec must move the route and
  refusal counts, and a deletion that moves neither did not happen.
- **Item 7** is guarded by the packet equality on the full-width Hot frame — the
  assertion the frame did not have until the STRUCTURED lane added it — and by
  `STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2 = 3` as an asserted ceiling.
- **Item 8** is guarded by the near-miss list itself: the rename is only safe if
  the overlapping verbs and flags are enumerated first, because the failure mode
  is a user running the right command against the wrong binary.
- **The four older refusals** are guarded by L1–L8 on every crossing (leverage),
  by the Dealer's accepted-transition contract (AMM), by their own absence
  (token), and by the browser being a second author (index) — the property that
  found the two-scale defect.

## 6. What was given up, named

**Item 6 deletes working code.** 1,444 lines with a complete codec, deleted
because nothing depends on it. The argument is that *"unexercised supply-moving
code with a 1,444-line codec and zero dependents is a risk surface"*, not that
it is wrong.

**Item 9 declines a third provider family**, so the tree's oracle diversity is
Pyth plus relayed for this generation — and cohort-13's outage was Pyth
redeploying their devnet receiver under every market's release pin. Decision
0027's ladder, with the relayed family as its last rung, is what carries the
risk item 9 accepts.

**Item 4 narrows what an author may ask.** Some questions a user would naturally
pose as a width-2 band now have to be posed as a proposition with a stated
prior, which is more work at authoring and a better question at resolution.

**Item 7 caps the shipping product at three outcomes**, and the cap is a packet
arithmetic fact rather than a preference — 1,269 against 1,232.

## 7. The cost of reversal, item by item

- **1, Series cut instead:** about 30 files and 3,508 lines of program plus its
  Lean and registers, and — after decision 0027 — the last rung of every
  recovery ladder. Reversing *back* to a cut after building the dispatch costs
  more than the cut would have today.
- **2, curvature cut:** 221 sorry-free theorems across 5,181 lines of Lean, the
  only degree-2/3 implementation in the project, and the entire no-arbitrage
  price gate. *"It is the right answer* only *if the ruling is that curvature is
  out of scope permanently."*
- **3, NO instead:** *"every split pays the 538,821-CU accelerator leg twice and
  none of them fit."* The two-transaction Dealer Remove ceases to exist as a
  shape; *"Ember may reverse either way; the numbers are the same numbers."*
- **4, admit the bare band:** the partition gate and the wizard both gain a shape
  whose question has been deleted, and the authoring surface has to carry both
  forms forever.
- **5, do not build split/merge:** `claims.conserve`/`DCLCNS01` stays the tree's
  one orphan magic and `CustodyRequired 0x5006` stays a dead refusal — both of
  which every census has to keep explaining.
- **6, un-delete Materialize:** re-writing 1,444 lines and then finding it the
  drivers it never had.
- **7, K = 2 as a product:** a Direct claim wearing a shard, with a second
  authoring path and a second set of fixtures for a shape Direct already serves.
- **8, do not rename:** the near-misses stay lethal. Renaming *after* release is
  the expensive direction, which is why it is ruled now.
- **9, add Switchboard:** about 13,000 lines by the tree's own precedent, gated
  on economics *"currently recorded as reported secondhand and unverified"*.

## Addendum, 2026-09-04 13:20 EDT: a tenth item, and it waits on ember

The nine items above are ruled. A tenth product question arrived the same day
from the mechanism agenda (decision 0031) and is **not** ruled here, because the
part of it nobody in this tree can supply is the part ember has to choose.

**The item: the conditional layer's flagship child market.** The CONDITIONAL
design (`docs/design/MECHANISM_CONDITIONAL_MARKETS_2026_09_04.md` §8, commit
`4b15cf69a`) proposes

> **"If feature `X` activates by slot `S`, does mainnet's slot time move?"**

as the first conditional market — a decision market on mainnet's own parameters,
built as the mainnet-state relay's product, with **both parents read through the
relay's four-account set and no venue decoding at all**.

- **Parent `A`, the decision.** The feature-gate account of `X` on mainnet,
  relay-attested; cuts `[S + 1]`, giving *activated by `S`* and *not activated by
  `S`* plus failure. **Both branches are observed**, which a decision market needs
  and which the graduation product's one-cell shape does not give.
- **Parent `B`, the metric.** Mean slot duration over a window after `S`, from the
  mainnet `Clock` sysvar — already account 4 of the relay's set — attested at two
  slots; cuts at the founder's thresholds (e.g. `[390, 410]` ms).
- **The child.** `A × B` with `A` major: `2 × 3 = 6` cells, width 7, whose two
  rows are `P(slot time | activated)` and `P(slot time | not activated)` read off
  **one price vector**, with the futarchy comparison being their difference. Or
  the conditional `B | A = activated`, width 5, which settles the moment the
  feature is seen *not* to have activated.

**What ember decides, and why the tree cannot:**

1. **Which feature, and which slot `S`** — a real calendar, and no lane can pick it.
2. **The metric.** Slot time needs no venue and demonstrates the mechanism
   totally; SOL/USD through a T-1 threshold (*"if `X` activates, does SOL/USD land
   above `P`?"*) is the classic futarchy shape with the deeper trader interest.
3. **The disclosure line** for a decision made by validators who do not read this
   market (the note's §5(c)).

**The honest sentence about it**, which is the note's own and belongs in a product
record rather than a design one: *its economic interest is modest and its mechanism
interest is total* — two relayed parents, one derived child, every settlement arm
exercised on devnet, and the first conditional read that is a chain fact.

**Two further rulings the same note owes**, neither of them ember-only and neither
ruled here: `AttestedUnobservable` — whether a child exhausts early on a parent's
failure certificate or walks its deadline — and **whether the founder bond applies
to a founder who chose parents rather than an oracle**. The second is a live
interaction with decision 0033, which makes the bond mandatory at a size rule
derived from *the terminal's* cost: a child market's terminal reads two
certificates and observes nothing, so the rule's terms are computable for it, but
whether the bond's *purpose* — pricing an oracle choice — survives a founder who
made no oracle choice is exactly the question, and 0033 does not answer it.

**Status of this item: OPEN.** It is a product question with an owner (ember) and
a design already written, which is the state §3 of this record calls
*underdesigned* being repaired rather than the state it warns about.

## Evidence pointers

`GOAL.md:1391-1397`, `:1500-1502`, `:1751-1767`, `:1812-1830`, `:2171-2174`,
`:2655-2662`, `:4084-4086`, `:4655-4656`;
`docs/design/BASIS_ABI_UNIFICATION_V1.md:536-543`; `docs/OMISSION_INDEX.md:53`;
`docs/design/DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md:320-342`;
`docs/design/PACKET_LIMIT_2026_09_01.md:319-330`;
`docs/decisions/0011-structured-v2-physical-route.md` §3b (amendment owed);
`docs/decisions/0022-pda-signed-caller-facts.md`;
`docs/INTENT.md` §4; `tools/gauntlet/journey/src/ledger.rs:1004-1012`;
`docs/design/MECHANISM_CONDITIONAL_MARKETS_2026_09_04.md` §8, §9;
`docs/decisions/0031-the-mechanism-agenda.md`;
`docs/decisions/0033-the-founder-bond-is-mandatory.md`;
`GOAL.md:4786-4794`; commit `4b15cf69a`;
`docs/evidence/C16_ENTRY_LIST_2026_09_01.md:418`.
