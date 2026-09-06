# Decision 0025: an oracle outage refunds the holders, it does not pay the founder

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible
on request) — ruled by the orchestrator on 2026-09-04 under ember's standing
goal, amended by ember at 10:15 EDT to require that the pathways be explained
and robust, and reversible at the cost §7 states**. It was PROVISIONAL from the
ruling until 15:50 EDT, when ember read the docket and accepted it in
conversation without amending it; the confirmation line below is the whole of
what was said. Docket item D2. Ember's amendment is at `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4653-4654`.
**The payout arm landed the same morning at `f9d40b615` (lane ESCROW),
hostile-first, and §5 records what it turns on; the founding change that seats
the failure coordinate in an escrow Position is owed and rides cohort-16.**
**Amended again at 12:30 EDT by the orchestrator's merge ruling — for a
refunding market `MergeCompleteSet` is redefined over the ORDINARY coordinates
— recorded in the amendment section at the end of this record; ESCROW-2 stated
it in Lean at `e37116b03` and ember may reverse it to the immobile-coordinate
shape.**
**Amended a third time on 2026-09-05 (lane PROGRAMS-17B): the escrow shipped
with no way to DISCHARGE it, so no market founded under this record can retire
— the failure column is unpayable under every certificate and its holder has no
key. The addendum at the end of this record states the two shapes and rules
PROVISIONALLY for A.**

**Confirmed, 2026-09-04 15:50 EDT.** Ember, after reading the docket and the
mechanism cohort page:

> you aren't waiting on me for rulings are you? i was reading the docket and
> contemplating it, but overall find your takes reasonable

The orchestrator's reply: nothing was waiting on ember — the rulings were
provisional and already in force, and the lanes had been working under them
since they were made; *"overall find your takes reasonable"* is taken as
confirmation rather than as an invitation to re-argue them; and the one thing
still genuinely ember's is the flagship conditional market's feature gate, its
slot and its metric (decision 0029's tenth item). So the status above is
CONFIRMED and no longer PROVISIONAL: accepted in conversation, unamended, and
reversible on request at the cost §7 states.

## 1. The question

Under an oracle outage a market resolves to its pre-disclosed failure outcome.
On cohort-13 the founder held **every** failure claim and the two strangers who
traded held none, so the outage paid the founder five hundred million atoms and
paid the strangers nothing
(`docs/evidence/COHORT13_SEALED_FOUNDED_2026_09_02.md:1331-1357`). Is a failure
outcome wholly owned by the founder the shape this venue sells to strangers?

**The mechanism is two lines, and neither is a defect.** Founding mints one
equal complete-set quantity into exactly one Position, the founder's
(`crates/dclutch-claims-svm/src/founding_v5.rs:3-6`, `:172-173`), and the kernel
forces every coordinate equal to coordinate 0
(`crates/dclutch-economic-slice-kernel/src/lib.rs:621-633`). The failure
selector is coordinate `region_count` of that one vector
(`crates/dclutch-product-runtime-v2/src/lib.rs:210-213`). There is no
per-selector distribution step, and nobody trades for a failure claim. So one
kernel invariant plus one PDA seed puts the entire failure supply in the
founder's hands.

**It executed, and the census held to the atom through it.** Phase `0 → 4`
`FailureCommitted`, `selector = 3`, the payout ran at 353,233 CU
(`COHORT13:1443-1445`, `:1723-1755`, `:1890`; `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:3509-3510`). The evidence
document's own reading (`COHORT13:1476-1480`): *"The protocol did what it says
it does. Whether a venue should* sell *that shape to strangers is a product
question this cohort has now made concrete rather than hypothetical … the honest
reading is that a failure outcome wholly owned by the founder converts an oracle
outage into founder revenue."*

**Cohort-14 refused to do it a second time.** *"Kind 4 is cohort-13's outcome
and shipping it twice would make an oracle outage into founder revenue a second
time"*; *"The failure walk was NOT run. … This lane stops at the wall rather
than walking through the only door left open"*
(`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:609-613`,
`:864-872`). So a live market sits unresolved on this question.

The three facts that combine badly: the founder chooses the oracle, the window
and whether there is a recovery policy; the founder holds every failure claim;
so an outage is a pre-disclosed outcome wholly owned by the party with the most
influence over whether it happens. Nothing was hidden. It is still the one path
where a market pays someone for doing nothing.

## 2. The ruling

**An outage refunds.**

1. **Disclosure first, at no protocol cost.** The tree's own options A and B
   (`docs/design/SPONSORED_WINDOW_ADMISSION_2026_09_02.md:305-313`): the app
   discloses who owns the failure position, and the market's terms state the
   payoff asymmetry — *"the payoff asymmetry exists and should be stated
   wherever a market's terms are shown, rather than discovered at resolution."*
   Derived from the deployed founding's shape, which today still says the
   founder. This lands before the founding change and independently of it.
2. **The failure coordinate is founded into an escrow, not into the founder's
   Position.** On failure the escrow returns collateral **pro rata to whoever
   holds ordinary claims at resolution**. The founder receives exactly their
   share as a holder and nothing for having chosen the oracle.

Option C — found markets *with* a recovery policy — is not an alternative to
this but its companion, and it is decision 0027; the ladder and the escrow are
the same failure pathway seen from two ends, which is why ember asked for them
together.

Changing *who is issued the failure coordinate at founding* is proposed nowhere
in the tree. The design note offers only disclosure and explicitly withdraws the
mechanical repair (`SPONSORED_WINDOW_ADMISSION:271-313`, *"Two consequences
worth carrying, **neither of them a code change here**"*). This ruling adds the
option the tree did not have.

## 3. Ember's amendment

Recorded at `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4653-4654`:

> D2 — wants the failure/recovery pathways explained, robust

Two obligations, and they are not the same one:

- **Explained.** The pathway is not finished when it is correct; it is finished
  when a stranger can read what happens to their money under an outage before
  they trade. That is the disclosure half, and it is why item 1 lands first
  rather than as a follow-up to item 2.
- **Robust.** Not merely fair. The escrow is only robust if the ladder in front
  of it works (decision 0027) and if the crank that advances it is
  permissionless and cheap enough that a stranger with a stake turns it — paid
  from the attempt's own funding, at the funded-crank floor decision 0024
  carves. A refund reached only by a path nobody will walk is not a pathway.

## 4. The lane implementing it

**ESCROW** (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4657-4658`), paired with **RECOVERY** (decision 0027). The
conservation law is stated in Lean before any Rust is written. The disclosure
half is a web and terms surface (`apps/dclutch-web`), not a program change, and
is not blocked on the founding change.

## 5. The hostiles and laws that will guard it

The properties the lane states in Lean first, each of which is a hostile as much
as a theorem:

- **a founder with no ordinary claims receives zero on an outage** — the direct
  negation of cohort-13's measured table;
- **a stranger holding half the ordinary claims receives half the escrow**;
- **a payout that would exceed the escrow refuses by name**;
- **the honest path is byte-identical to today's**, which the Direct fill
  campaign and the census laws already hold.

The census is the standing instrument and it is already sharp enough: L1–L8 held
to the atom across cohort-13's failure payout, so a new conservation for the
escrow is checked by the same ledger rather than by a new one
(`tools/gauntlet/journey/src/ledger.rs:1004-1012`). The certificate's kind byte
is the coarse witness cohort-14 used to refuse the second failure walk
(`COHORT14:609-613`) and stays the outer check.

### What landed at `f9d40b615`, and what it turns on

The economics land in the **payoff vector**, and the lane found that no new
conservation machinery is needed: the terminal route already gates every payout
vector against the record's payout scale (`validate_partition`,
`product_basis_terminal_v3.rs:391`) and already refuses an underfunded one
(`Insolvent`, `:443`). A refund arm needs *a vector that sums to the same
scale*.

So a categorical basis now carries exactly **two admissible payout scales**, and
the scale is what says who an outage pays:

- **`1`, LEGACY.** The winner's claims pay one atom each, and when the winner is
  the failure coordinate that pays whoever minted the failure claims. This is
  cohort-13's shape and it is **unchanged, deliberately**: *"rewriting a deployed
  market's terms underneath the people who traded on it is not a repair."*
- **`basis_width - 1`, REFUNDING.** The winner pays its ordinary-region count
  per claim on the honest walk; an outage instead pays **one atom to every
  ordinary claim** and nothing to the failure coordinate. Both vectors sum to
  the same scale, so both arms pass the gate that was already there.

**Width 3 is the floor, and the floor is mathematical rather than a profile:**
at width 2 the two scales are the same number and the record could not say which
shape it carries — *"a disclosure that cannot be derived is a disclosure that
gets typed by hand."* That is §2 item 1's disclosure made derivable rather than
written by hand.

`categorical_refunds_on_failure_v3` (`runtime_v3.rs`) is the **sole author** of
the rule; `ProductBasisV3::refunds_on_failure` is that function applied to a
record's fields, and the two settlement call sites hold an authenticated mirror
joined to the record (`product_basis_terminal_v3.rs:534`) and call the same
function. `native_categorical_v1.rs` stopped restating *"scale must be 1"* — it
had been a second, quieter author that would have refused every refunding market
with a cross-record error naming nothing.

Hostiles: every assertion but one was **red before the commit**, and
`(CategoricalQ1, Failure)` had been reaching the evaluator's wildcard. Five more
in the codec name exact discriminants at the layer that has them, including that
a refused evaluation leaves the caller's output buffer untouched and that a
hand-built record with an inadmissible scale is refused by the **decoder**, not
only by the encoder. 272 tests green across the five touched crates.

**The founding already carries the refunding scale, and this was not designed
for -- it was found.** Core founding does not take a collateral scale from a
caller: it DERIVES it from the Product, `programs/dclutch-core-sbf/src/generic_founding_v1.rs:1104`
--- `basis_scale: product.payout_scale` --- and Claims founding then binds the
permit's scale to the request's (`programs/dclutch-claims-sbf/src/founding_v5.rs:1247`).
So a market founded on a refunding record funds its Hoard at
`quantity * ordinary_region_count` with no founding code change at all, the
honest walk pays that same scale per winning claim, and the failure walk pays
one atom to each of `ordinary_region_count` columns --- the same total, per
complete set, either way. The payout half of §2 therefore needs a founded
RECORD, not a founded PROGRAM, which is why it is a runbook row
(`tools/cohort/steps.tsv`, key `refund-scale`, `since=16`) rather than a wire
version. Only the escrow SEATING in §2 item 2 needs founding to change.

**What is owed, named by the lane rather than left to be found:** the founding
change that seats the failure coordinate in the escrow Position. *"Until it
lands, a refunding market's failure column is still minted to the founder —
worth zero, which is worse than worthless because it is SELLABLE."* And after
that, a market that shows a refund on a **forced** outage on a real chain; until
that exists the ruling is a design, and the record says so.

## 6. What was given up, named

**This is a founding change.** Founding mints the complete set; moving the
failure coordinate to an escrow changes what founding does, so no market already
on a chain can adopt it. Every existing market keeps the old shape until it is
re-founded, which the disposability regime permits on devnet and which mainnet
would not (`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:1443-1450`). Cohort-16 carries it.

**The founder loses a hedge they may have priced in.** A founder who held the
failure coordinate held insurance against their own oracle going quiet. After
this they hold only their share, and the collateral that used to be their
downside protection becomes the holders'. That is the point, and it is a real
transfer, not a free correction.

**The escrow is a new account with new conservation.** It is the first place in
the tree where collateral is held against a *distribution over holders at
resolution* rather than against a fixed coordinate, and pro-rata division is a
rounding boundary that must be named (`AGENTS.md`, *"exact scaled integers with
one named rounding boundary"*).

The rounding boundary is now named, and the lane's answer is that **there is no
remainder, because a remainder is refused at founding rather than housed**. A
floored atom has nowhere declared to go: the census names nine compartments
(`tools/gauntlet/journey/src/ledger.rs:270-280`) and none of them is an upkeep
vault; `unclassified` is a class L8 holds to a declared delta rather than a
bucket to hide in; and creating a tenth is one of the five economic choices
C-11 reserves to ember by name (`docs/MASTER_COMPLETION_CONTRACT.md:96`). So
the admission is a founding-time divisibility condition on the founder's own
basis scale, and under it `an_admitted_failure_walk_leaves_no_remainder`
(`EconomicKernel.lean`) holds for **every** partition of the ordinary claims
with no divisibility hypothesis surviving in it. As built, the refunding scale
`basis_width - 1` satisfies it by construction: one ordinary claim redeems for
exactly one atom, and the division disappears.

**The escrow forecloses complete-set MERGE, and that is not an implementation
choice.** `MergeCompleteSet` burns one claim at *every* coordinate from *one*
Position (`economic-slice-kernel/src/lib.rs` `validate_basket_quantities`
forces every coordinate equal, and `basket_candidate` debits `source_native` at
each index), so a holder who does not hold the failure coordinate can never
merge a complete set back into collateral. Move that coordinate anywhere the
founder is not — an escrow Position, a Position nobody owns, or no Position at
all — and merge stops working for that market. It is reachable and used: it is
a General child effect (`dclutch-general-adapter-contract/src/escrow_v1.rs:194`,
`plan.rs:889`) and a routed Claims action (`claims-svm/src/lib.rs:215`, `:599`).

So the escrow as §2 item 2 words it needs a companion: either `MergeCompleteSet`
is redefined for a refunding market as **the ordinary coordinates only**, with
the escrow's failure claims burned alongside by the program rather than by the
merging holder, or the failure coordinate stays in the founder's Position and is
made **immobile** instead of relocated. The second is much smaller and delivers
the same protection the escrow was for — the failure coordinate under a
refunding basis is worth zero, and worth-zero-but-sellable is the actual hazard
— by refusing, at the signed-delta waist (`claims-svm/src/signed_delta_v3.rs`,
which every split, transfer, merge and redeem passes through), any Position
delta at the failure coordinate whose direction is not the aggregate supply
delta's. A transfer moves no supply, so under that rule it may move no failure
claim; a mint or a merge moves supply, so it may. Either way the founder is paid
nothing for choosing the oracle, which is what the ruling is for. **This is a
fact the ruling did not have, and the choice between the two shapes is ember's
or the orchestrator's, not the lane's.**

## 7. The cost of reversal

**Reversing after it ships is a re-found**, symmetrically with landing it: the
issuance shape is fixed at founding.

**Reversing before it ships** returns the product to the state cohort-14
declined to ship a second time — an oracle outage that is, in decision 4 of the
design note's words, *"converted, **exactly and by design**, into revenue for
whoever minted the failure claims"* (`SPONSORED_WINDOW_ADMISSION:39-41`,
`:278-279`) — and leaves cohort-14's market parked, because the only reachable
terminal is the one that lane refused to walk. Disclosure alone (A and B) closes
the *honesty* of it and none of the economics: the buyer still bears the whole
loss, and the founder still receives the pool.

The root cause of cohort-13's outage was Pyth redeploying their devnet receiver
under every market's release pin, which no founder caused. The point stands
regardless of cause: the shape is what a stranger would rightly call rigged,
whoever triggered it.

## Amendment, 2026-09-04 12:30 EDT: merge is redefined over the ordinary coordinates

**PROVISIONAL, ruled by the orchestrator under ember's standing goal**, recorded
at `docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:4782-4785`, and reversible by ember to the second shape at the cost
this section states.

**The question this answers is the one §6 left open.** The ESCROW lane stopped
before seating the escrow because it found that the escrow **forecloses
`MergeCompleteSet`**: the kernel burns one claim at *every* coordinate from *one*
Position, so a holder who does not hold the failure coordinate can never merge a
complete set back into collateral. §6 named two repairs and said the choice was
*"ember's or the orchestrator's, not the lane's"* -- redefine merge for a
refunding market over the **ordinary coordinates**, with the escrow's failure
claims burned alongside by the program; or leave the failure coordinate in the
founder's Position and make it **immobile** at the signed-delta waist.

**The ruling: the first. Merge is redefined over the ordinary coordinates for a
refunding market.**

Why, in one line: §2 item 2 of this record already ruled that the failure
coordinate is founded into an escrow, and the immobile-coordinate shape is not an
implementation of that ruling -- it is a different ruling wearing the same
protection. Under it the founder still *holds* the whole failure supply and is
stopped from selling it by a conjunct that every split, transfer, merge and redeem
must keep passing; under the escrow the founder never holds it, and the protection
is a fact about who owns what rather than a refusal that has to stay correct
forever. The smaller change is not the safer one here.

**What the ESCROW-2 lane landed under it** (`e37116b03`, `EconomicKernel.lean`,
twenty-one new declarations, zero `sorry`, and `#print axioms` reporting only
`propext` and `Quot.sound`):

- **The foreclosure is proved, not asserted.**
  `a_holder_without_the_failure_coordinate_cannot_merge` shows `commandAccepts`
  returns false and `execute?` refuses `notAdmissible` for a holder whose failure
  coordinate is zero -- which is every holder on a market whose failure column is
  seated in an escrow -- instantiated against a **concrete** founded,
  cohort-13-shaped state, so it is a reachable refusal and not a hypothesis nobody
  satisfies.
- **The law is four combinators and one observation.** `addBelow`/`addFrom` and
  `subBelow`/`subFrom` split a coordinate vector at the ordinary boundary, and
  `addFrom_addBelow_eq_addEvery` says the two Positions of a refunding market hold,
  between them, **exactly one categorical complete set**.
- `refunding_merge_is_a_complete_set_merge_in_the_aggregate`: Hoard, supply and
  the native partition move exactly as the categorical merge moves them, so every
  conservation already proved still governs a refunding market and **the census
  reads it with no new compartment**.
- `the_refunding_merge_undoes_the_refunding_split` with **no hypothesis at all**,
  and its converse under exactly the merge's own admission.
- `the_refunding_actions_keep_the_escrow_seated` -- the induction, so the escrow
  holds the whole failure supply for a market's entire open life rather than only
  at founding; and
  `the_seated_escrow_stands_against_exactly_the_ordinary_supply`, which is what
  makes the pro-rata rate **a constant of the Market header** rather than a
  division.
- Founding is not a separate law: `refundingSplitPost` is the renamed and
  generalised `escrowedFoundingPost`, because **founding IS the refunding split
  run against a vacant pre-state**, and a law with two spellings eventually
  disagrees with itself.

**Still owed:** the Rust. The founding change that seats the failure coordinate,
the merge route that burns across two Positions, and a market that shows a refund
on a forced outage on a real chain. Until those exist this half of decision 0025
is a design with a proof, and §5's closing sentence still governs.

**The cost of reversing to the immobile-coordinate shape.** It is the smaller
change and it stays available: the founding is untouched, the failure supply stays
in the founder's Position, and the protection becomes a direction conjunct at
`claims-svm/src/signed_delta_v3.rs` -- any Position delta at the failure
coordinate whose direction is not the aggregate supply delta's is refused, so a
transfer (which moves no supply) may move no failure claim, while a mint or merge
may. What it costs is that the founder holds a worthless-but-held position for the
market's life, that every future route through the signed-delta waist inherits an
invariant it must not break, and that the Lean above would be replaced rather than
extended. Reversing **after** a market is founded is a re-found either way, because
the seating is a founding fact.

## Evidence pointers

`docs/evidence/COHORT13_SEALED_FOUNDED_2026_09_02.md:1320-1357`, `:1443-1445`,
`:1465-1500`, `:1723-1755`, `:1890`;
`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:609-613`,
`:864-872`, `:1405-1406`;
`docs/design/SPONSORED_WINDOW_ADMISSION_2026_09_02.md:20-41`, `:271-313`;
`crates/dclutch-claims-svm/src/founding_v5.rs:3-6`, `:141-146`, `:172-173`;
`crates/dclutch-economic-slice-kernel/src/lib.rs:604`, `:621-633`, `:771-775`;
`crates/dclutch-product-runtime-v2/src/lib.rs:204-213`;
`crates/dclutch-source-contract/src/source_resolution_v2.rs:449-451`, `:525`;
`programs/dclutch-resolution-proof-sbf/src/funded.rs:4-5`, `:348`;
`docs/ledger/GOAL_2026-08-31_to_2026-09-04.md:3507-3514`, `:4653-4658`, `:4739-4750`, `:4782-4785`;
`formal/dclutch-semantics/DClutchSemantics/EconomicKernel.lean` (the refunding
split and merge, `e37116b03`);
`crates/dclutch-claims-svm/src/signed_delta_v3.rs`;
`docs/decisions/0033-the-founder-bond-is-mandatory.md` (the bond that prices the
oracle choice this record leaves at zero); `tools/gauntlet/journey/src/ledger.rs:1004-1012`.

## Addendum, 2026-09-05 late evening: the escrow was seated with no discharge, and no refunding market can retire

**PROVISIONAL, ruled by lane PROGRAMS-17B under the precedent §6 set, and
reversible by ember or the orchestrator to shape B at the cost this section
states.** The fact came from cohort-16.1's first walk to Terminal
(`docs/evidence/COHORT161_UPGRADED_SEALED_2026_09_05.md`, the PROGRAMS-17B
addendum), which convicted the founding and had to withdraw the conviction: the
founding is correct and the escrow is seated exactly as §2 item 2 requires.

**The fact this ruling did not have.** Seating the failure coordinate in an
escrow changes who holds it for the market's whole life -- including after
Terminal. Closure refuses on any nonzero aggregate coordinate
(`programs/dclutch-claims-sbf/src/market_closure_v1.rs:656-668`,
`ClaimsMarketClosureSbfErrorV1::Liability` = `Custom(0x5503)`), and the operator
hoists the same conjunct to the `BeginRetiring` preflight
(`crates/dclutch-operator/src/wallet_terminal_input.rs:456`). On a refunding
market the failure coordinate's supply can never reach zero:

* it is unpayable under EVERY certificate — `evaluate_categorical` refuses
  `FailureCoordinateNotPayable` at that selector and `evaluate_categorical_failure`
  pays "one collateral atom to every ordinary claim, nothing to the failure
  coordinate" (`crates/dclutch-product/src/payoff/runtime_v3.rs:972`, `:990`),
  so neither an honest walk nor an outage drains it;
* its holder cannot sign — terminal settlement under `CallerRole::Claims`
  binds coordinate 0's signature to the Position's owner
  (`programs/dclutch-claims-sbf/src/terminal_settlement_v3.rs:635`), and the
  program states at `:620` that "a Claims capability owner" is "a
  program-derived address with no key", which is precisely what the escrow's
  owner is; and
* `MergeRefundingCompleteSet` — the one route that moves the escrow — returns
  collateral from a Hoard that Terminal has already drained.

The program's `payout == 0` arm exists and is complete
(`terminal_settlement_v3.rs:506`, `:939`). What does not exist is anyone who can
authorize it for this Position. **Every market founded under this decision is
unretirable, on any chain, whatever the founding does**, and that is a founding
fact in the same way the seating is: it cannot be repaired by re-founding under
the same programs.

This is the same shape as the merge foreclosure the ESCROW lane found before it:
the escrow relocates a coordinate, and every route that assumed the coordinate
was in a wallet has to be asked again. Merge was asked. Retirement was not.

### The two shapes

**A. Closure admits the seated escrow as the terminal residue.**
`market_closure_v1`'s conjunct becomes, for a refunding market: supply is zero at
every ORDINARY coordinate, and the failure coordinate's supply is exactly the
derived escrow's balance at that coordinate. Retirement closes the escrow
Position and its admission alongside the aggregate, and their rent reaches the
declared beneficiary, which L6 already watches. The escrow's balance is not a
free number the check has to trust: the Lean already proves
`the_refunding_actions_keep_the_escrow_seated` and
`the_seated_escrow_stands_against_exactly_the_ordinary_supply`, so at Terminal
it is a function of the Market's own header.

**B. A permissionless escrow settlement.** A crank entitled the way
`ClaimCheckCrank` is — the market is Terminal, the recipient is derived —
settles the escrow's failure column through the existing `payout == 0` arm,
after which the universal zero-supply rule stands untouched.

### The ruling: A

**B adds a second exemption to the one conjunct that stands between a keyless
PDA and every capability-owned Position in the system.** `terminal_settlement_v3`
has exactly one today, the compaction crank, and it replaced the owner's
signature with a persisted owner-kind tag and an elapsed deadline rather than
dropping it. A second exemption for the escrow would have to be at least as
careful, forever, and it buys a route whose entire correct behaviour is to move
zero atoms. §2 item 2 preferred "a fact about who owns what" over "a refusal
that has to stay correct forever"; the same instinct rules here, in the opposite
direction from the merge amendment, and for the same reason.

**What A actually has to do, because half of it is not obvious.** Admitting the
residue is not enough: `protocol_position_v2.rs:608` refuses to close a Position
with any nonzero balance, so a closure that merely tolerated the escrow's column
would strand the escrow's Position, its admission and their rent, and L6 would
be right to say so. So A is the closure BURNING the escrow's failure column --
writing the escrow's vector and the aggregate's supply at that coordinate to
zero in one program act, with the escrow in the frame -- and then closing both
escrow accounts alongside the aggregate. That is the same mechanism the merge
amendment above already ruled for, "the escrow's failure claims burned alongside
by the program rather than by the merging holder", moved to the one boundary
where nobody is merging. A repair that only relaxes the supply check is not this
ruling; it is a rent leak wearing it.

**What A costs, named.** Retirement learns the refunding shape — one derivation
it can already make from the aggregate. The universal sentence "every claim's
supply is zero" becomes "every claim's supply is discharged", and a reader has
to be told that an unpayable column held by nobody IS discharged. The
`authenticate_zero_claims_v1` host mirror moves with it, and so does its message,
which today instructs an operator to "produce and execute wallet terminal
payouts first" for a coordinate no wallet can hold — an instruction no party can
follow. And it is a Claims program change, so the ELF moves and cohort-17
carries it as a re-release plus a re-found under decision 0012.

**What reversing to B costs.** The same re-found, plus the exemption above, plus
a crank that somebody must run before any refunding market can retire.

**Repaired alongside this record.** The `K+1 partition` rule in the Direct
terminal certification demanded the seller hold a nonzero row at every outcome,
which a refunding seller structurally cannot, so it refused every refunding fill
it was asked to certify and left cohort-16.1's landed fill uncertified. The
schedule is now joined against the two Positions' own nonzero coordinates
(`tools/local-validator/bootstrap/successor/src/direct_trade.rs`,
`a_refunding_seller_without_the_failure_coordinate_certifies`), which is the
invariant the count stood in for and is strictly stronger than it.
