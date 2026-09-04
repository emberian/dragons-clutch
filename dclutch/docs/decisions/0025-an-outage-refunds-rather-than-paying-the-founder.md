# Decision 0025: an oracle outage refunds the holders, it does not pay the founder

Status: **PROVISIONAL — ruled by the orchestrator on 2026-09-04 under ember's
standing goal, amended by ember at 10:15 EDT to require that the pathways be
explained and robust, and reversible at the cost §7 states**. Docket item D2.
Ember's amendment is at `GOAL.md:4653-4654`. **The payout arm landed the same
morning at `f9d40b615` (lane ESCROW), hostile-first, and §5 records what it
turns on; the founding change that seats the failure coordinate in an escrow
Position is owed and rides cohort-16.**

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
(`COHORT13:1443-1445`, `:1723-1755`, `:1890`; `GOAL.md:3509-3510`). The evidence
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

Recorded at `GOAL.md:4653-4654`:

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

**ESCROW** (`GOAL.md:4657-4658`), paired with **RECOVERY** (decision 0027). The
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
would not (`GOAL.md:1443-1450`). Cohort-16 carries it.

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
`GOAL.md:3507-3514`, `:4653-4658`; `tools/gauntlet/journey/src/ledger.rs:1004-1012`.
