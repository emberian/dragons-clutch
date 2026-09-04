# Decision 0033: the founder bond is mandatory, at the size rule

Status: **PROVISIONAL — the BOND note's §8 question ruled by the orchestrator on
2026-09-04 under ember's standing goal, with the bond itself designed and unbuilt,
and reversible at the cost §7 states**. The question is
`docs/design/MECHANISM_FOUNDER_BOND_2026_09_04.md:456-473`; the design and its 34
sorry-free theorems landed at `86d38a203` and `9365be226` (`GOAL.md:4756-4763`).
Direction 5 of the mechanism agenda, decision 0031.

## 1. The question

Decision 0025 stops an oracle outage from **paying** the founder for choosing the
oracle. It leaves that choice **priced at zero**: after 0025 a founder whose
oracle goes quiet receives their share as a holder and nothing more, and also
loses nothing they had staked, because they staked nothing.

The BOND note is the design that prices it. At founding the founder posts a bond
in lamports, sized to the market's own terminal cost, returned in full on an
honest terminal and walked pro rata to the ordinary claims on an exhausted one,
in the same redemptions that pay decision 0025's escrow refund.

The size rule is the founding's own projection and **nothing in it is typed**:

    B = S + F + Λ
    S = rent(312)                              the certificate seat
    F = the compaction opener's advance, less what the first crank repays
    Λ = Σ over rungs of the rung's Bounty quote

Every term is rent arithmetic over widths the founding already holds, at **the
rate the founding recorded** rather than the sysvar of the moment (decision
0030). On cohort-15's numbers that is **4,031,465 lamports — 0.004031465 SOL,
1.75% of a market lane** — decided in Lean at all three rates the economics note
prices (`FOUNDER_BOND:99-124`).

What the note left to ember (`§8`): **is the bond mandatory at the size rule, or
a founder's choice the market page derives from the escrow's lamports?**

## 2. The ruling

**Mandatory, at the size rule.** There is no optional mode and no founder-chosen
size. A market that does not fund `rent + B` on the escrow account at founding is
not founded.

The reasoning is the note's own, and this record adds one line to it:

- **An optional bond creates a class of markets a stranger has to learn to
  avoid.** The page can disclose either — *"forfeits 0.0040 SOL"* or *"posted no
  bond"* is one more sentence from data `outageDisclosureV1` already reads — and
  *"a disclosure that has to be read is weaker than a shape that cannot be
  founded otherwise, which is the argument 0025 §2 already made for the escrow"*
  (`FOUNDER_BOND:467-473`).
- **The bond is returned to every honest founder.** Its cost to an honest market
  is a lock-up, not a fee, so mandatory does not tax the behaviour the venue
  wants.
- **And the line this record adds:** an optional bond would price the oracle
  choice at zero *for exactly the founders who most want it priced at zero*.
  Optionality is not neutral here — it is selected against by the party whose
  incentive 0025 identified. That is what makes this different from every other
  optional disclosure in the tree.

**The size rule is mandatory too, not just the bond.** A founder-chosen size
reintroduces the same selection through the back door and makes the page's
sentence a comparison rather than a fact.

## 3. Ember's amendment

None. This is the question the note put to ember and the orchestrator answered it
under ember's standing goal, as it answered decisions 0031, 0032 and 0034 the
same day. Ember's words authorising the agenda are quoted in decision 0031 §3.

The ruling sits downstream of an amendment ember **did** make: decision 0025 §3,
*"D2 — wants the failure/recovery pathways explained, robust."* The bond is the
"robust" half taken one step further than 0025 took it — 0025 makes the outage
pathway fair, and the bond makes choosing badly cost something.

## 4. The lanes

None is chartered by this record. BOND closed as a design lane at `86d38a203`;
**cohort-17** carries the build (`FOUNDER_BOND:444-455`): the founding frame, the
payout frame and the close route. Cohort-16 is carrying 0025's escrow seating and
0027's ladder and must not also carry this.

The bond's account **is the failure escrow's own** — the bond is the lamport side
of a fact that account already owns — so the build rides ESCROW's seam rather
than opening a new one, and `ResolutionCloseFund` is untouched at 252,368 CU.

## 5. The hostiles and laws that guard it

**The compartment is none of the nine and not a tenth.** L8 counts atoms and the
bond never holds one; its law is **L7**, as a watched account declared by lamport
at founding, at each exhausted redemption and at the close — the class cohort-15's
capture already exercised. Creating a tenth compartment is one of the five
economic choices C-11 reserves to ember by name, and this design does not.

**The hostiles, each with its refusing route and its theorem**
(`FOUNDER_BOND:337-354`):

| hostile | what refuses it | Lean |
| --- | --- | --- |
| a founding without the bond | the founding conjunct `rent + B ≤ lamports` on the escrow account — a new Claims discriminant `FounderBondUnderfunded`, **red at founding** | `a_founding_one_lamport_short_refuses` |
| a withdrawal mid-life | no route moves the escrow's lamports while `Phase = Open`; the escrow has no signer | `no_exit_while_the_market_is_open` |
| a payout exceeding the bond | the draw is bounded by what remains, for any quantity the aggregate admits | `no_draw_exceeds_what_remains` |
| a bond paid on an honest resolution | on kind `1` every redemption draws zero and the close returns everything | `an_honest_exit_pays_the_holders_nothing` |
| the escrow's own claims drawing on the failure arm | the failure coordinate's quantity is zero to the draw | `the_failure_coordinate_draws_nothing` |
| a close before the walk finishes | the exhausted-arm close refuses while `outstanding > 0` — `OrdinaryClaimsOutstanding` | `an_exhausting_walk_pays_the_bond_exactly` |
| `k` Positions to farm the rounding lamport | fewer than `k` lamports for `k × rent(Position)`; unprofitable by construction | `draw_within_one_lamport_of_the_exact_share` |

A stranger topping up the escrow is deliberately **not** refused: it enlarges the
holders' draw, and refusing a one-lamport transfer is the griefing vector 0024
item 5 rejected.

**The two new refusals must name their exact discriminants and be proved red
before green** (`AGENTS.md`, Refusal codes). `FounderBondUnderfunded` carries the
seat's check with it, so cohort-15 market 3's six-refusal discovery becomes one —
which is the `map_err` lesson applied at founding rather than after it.

**The rate is the founding's recorded rate, and that is decision 0030's law.**
Cohort-15 measured what happens otherwise: devnet moved 6,333 → 5,080 mid-life
and 491,176 lamports of one ledger's rent became an unclassified surplus.

## 6. What was given up, named

**Every founder pays a lock-up, including the honest ones.** 0.004 SOL on
cohort-15's numbers, returned in full — but locked from founding to terminal, on
a market whose whole lamport lane is 0.230228002 SOL. For a founder running many
small markets that is real working capital.

**A ladder makes the bond several times larger, and mandatory means there is no
way to opt out of that.** A rung's Bounty quote is a founding input floored under
0024 §3 by rent on the 376-byte funded receipt — `rent(376)` = 3,191,832 at
6,333 — so on cohort-15's other numbers a **one-rung market bonds 7,223,297 and a
two-rung market 10,415,129**. The interaction is the uncomfortable one and it must
be said: decision 0027 wants markets founded *with* recovery policies, and a
mandatory bond at this size rule **prices recovery policies out at the margin**
for a thin founder. `a_rung_never_lowers_the_bond` is the only thing the Lean
says about it; whether the ladder's own funding should be netted out of `Λ` is a
question this ruling leaves open and does not pretend to have answered.

**The founding gains a conjunct at 84.6–91.3% of the CU ceiling.** The atomic
founding runs at 1,184,132–1,278,747 CU, so even a conjunct is measured rather
than assumed: **provisional +500 CU**, lifted by the founding campaign's own
CU-budget row on the cohort that carries it.

**The bond does not repay the opener's first crank.** Decision 0024 item 3 stands
— crank-first, disclosed rather than discovered — and the bond deliberately does
not soften it, because a bond that pays the opener is a bond the founder can pay
themselves.

## 7. The cost of reversal

**Reversing to optional before it ships** costs one conjunct and the page's
sentence changes from a fact to a comparison. Cheap in code, and it gives back
exactly what §2 bought: the class of markets a stranger has to learn to avoid,
selected into by the founders 0025 identified.

**Reversing after it ships is a re-found**, symmetrically with landing it: what a
market's founding required is fixed at founding, so no deployed market can adopt
or shed the bond. This is the same shape as decision 0025's own reversal cost and
for the same reason.

**Reversing the SIZE RULE while keeping the bond** — a flat bond, a fraction of
the Hoard, a founder-chosen number — is the reversal most likely to be wanted and
the note argues against each: the terminal cost is the number the founder's own
choice actually puts at risk, and a fraction of the Hoard makes the bond a
function of trading rather than of the oracle (`FOUNDER_BOND:125-145`).

## Evidence pointers

`docs/design/MECHANISM_FOUNDER_BOND_2026_09_04.md:59-124`, `:125-145`, `:146-197`,
`:198-280`, `:281-297`, `:298-354`, `:355-393`, `:394-455`, `:456-473`;
`formal/dclutch-semantics/DClutchSemantics/FounderBondV1.lean` (whole);
`crates/dclutch-claims-svm/src/claim_check_conservation_v1.rs:126-183`;
`crates/dclutch-source-contract/src/source_recovery_policy_v2.rs:108-110`;
`formal/dclutch-semantics/DClutchSemantics/CapabilityManifestV1Abi.lean:66-118`;
`tools/gauntlet/CU_BUDGETS.md:9-10`;
`docs/evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md:1628-1631`,
`:1818-1837`, `:2232`;
`docs/evidence/COHORT14_SEALED_FOUNDED_FILLED_2026_09_03.md:1741-1750`;
`docs/decisions/0024-sustainable-economics-and-a-governable-parameter-surface.md`
items 3 and 5; `docs/decisions/0025-an-outage-refunds-rather-than-paying-the-founder.md` §2-§3;
`docs/decisions/0027-recovery-is-one-funded-ordered-ladder.md`;
`docs/decisions/0030-rent-is-fixed-when-an-account-is-funded.md`;
`docs/decisions/0031-the-mechanism-agenda.md`;
`docs/MASTER_COMPLETION_CONTRACT.md:96`;
`GOAL.md:4756-4763`; commits `86d38a203`, `9365be226`.
