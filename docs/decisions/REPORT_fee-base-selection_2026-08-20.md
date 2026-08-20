# Decision report — `fee-base-selection` (register B1, folding B2/B3)

Status: **ANALYSIS FOR EMBER / DECIDES NOTHING.** Standalone report 1 of the
2026-08-20 decision-register fan-out
(`docs/decisions/DECISION_REGISTER_2026-08-20.md` B1, with B2's bounds and
B3's axes-descope folded in as that entry directs). Claim vocabulary of
`CURRENT_TRUTH.md` §1 governs. Every number below is either cited from the
tree or produced by this report's own run of the economics lab; the lab run
and the one lab extension (the register's "not modeled anywhere yet" option
5) are committed alongside this file. Nothing here changes a byte of
consensus code, relaxes a gate, or promotes a claim.

---

## 1. The decision, in one sentence

**Select the fee base — the functional of `(payoff vector, clearing prices)`
whose scaled value a fee-bearing epoch would charge on filled risk
transfer — from: flat cash-notional, per-Egg leg, atomic simplex dispersion
`G(a,p)`, price-free quotient-norm `kappa'·R(a)`, a composite of dispersion
with a quotient-norm floor, or zero-fee-forever.**

What selecting freezes:

- the *shape* of the charge: which `FeeBaseV1` variant gets authored, what
  `docs/FEE_GEOMETRY.md` presents as the base rather than an arm, which
  zero-price disposition becomes the frozen §10.5 regression fixture of
  `docs/design/REVENUE_POLICY_V1.md`, and which bounds B2 must then freeze.

What selecting does **not** freeze:

- **any rate.** No numerator is proposed anywhere in-tree (register B1); the
  rate is a strictly-after decision, and every rate in this report is a lab
  comparison calibration, not a proposal.
- **any byte.** All five `max_fee_atoms == 0` gates stay closed; both pinned
  policy consts keep `FeeBaseV1::None`; the candidate ABI keeps no fee
  field. Relaxation is owned by the B4 destination decisions plus the §10
  falsifiers, in that order (findings §6: "decide the destination before the
  base" — see §5 below for how this report respects that sequencing).
- **promotion.** The selected base remains an unpromoted design object until
  the rewritten promotion gate (§5.3) closes.

## 2. Context: where fees stand

Everything is zero, on purpose, at every plane:

- **Program plane.** The signed fee envelope is forced to zero at exactly
  five gates: `validate_direct_v4_place`
  (`programs/clutch-sbf/program/src/instructions/orders_batch.rs:910`),
  `validate_submission_reservation`
  (`orders_batch/settlement.rs:435`), `prepare_direct_full_slice`
  (`orders_batch/settlement.rs:574`, whose comment states the whole blocker:
  *"Fees need a frozen fee base and a named recipient"*),
  `execute_settlement` (`direct_selection.rs:908-909`), and
  `validate_order_reservation` (`direct_selection.rs:1759`). (Line numbers
  re-verified for this report; the register's cites have drifted a few
  lines.) The layout is already fee-*capable* — `max_fee_atoms` rides
  `Intent::PlaceOrder`, `ReservationPlan` computes fee-inclusive envelopes,
  the codec round-trips it (findings §5) — there is simply nothing to pay a
  fee *to*.
- **Destination plane.** `RevenuePolicy` is named as an architectural
  boundary in four documents and is zero lines of code
  (`docs/ECONOMICS.md:206-208`); the V1 design exists as
  PROPOSED/DESIGN-ONLY (`docs/design/REVENUE_POLICY_V1.md`) with its six
  sub-decisions (B4a–f) undecided. ResolutionWork's five charge fields are
  hardcoded zero with the reason in the source: *"Every protocol charge is
  zero because V1 has no authenticated fee sink."*
- **Policy plane.** `DIRECT_POLICY_V1` and the PROPOSED
  `GENERAL_CLEARING_POLICY_V1` both pin `FeeBaseV1::None` at 0 bps, the
  latter with an explicit non-preemption note: the pin "deliberately does
  **not** preempt the queued fee-base fork"
  (`research/batch-policy-identity/src/general_clearing_v1.rs:24-26`).
- **Gate plane.** `FEE_GEOMETRY.md` §7's first promotion criterion demands
  that "Verus and Rocq close translation, homogeneity, complete-set
  invariance, bounded arithmetic, carry conservation, and
  partition-refinement invariance." Rocq contains zero theorems
  (`FEE_GEOMETRY.md:229-230`; register H1: Verus covers ~1.5 of 11 named
  properties, Lean carries 184+ theorems). The criterion is
  unsatisfiable-as-written against a dead architecture, not pending. Four of
  §6's eight measurement axes (depth, participation, fill rate, route
  leakage) require a market-quality simulator that exists nowhere in the
  tree (findings §3). The gate cannot close as written no matter which base
  is picked.
- **Economics plane.** `ECONOMICS.md` §6's break-even inequality returns
  unbounded required volume at every currently-true configuration: no
  volume covers any cost.

One measured fact recorded but read by no policy, noted here because a fee
design should know it: cancellation now costs **more** CU than placement —
282,868 vs 185,807 (`GOAL.md:327-329`, the sealed liveness reseal). It is a
lamports/liveness-plane fact (B4c territory), not a collateral-atom fee-base
input, and none of the candidate bases below touches cancellation.

## 3. The candidates, on the tree's own evidence

Lab verification for this report: `python3 -m unittest discover -s
research/economics-admission -p 'test_*.py'` — **40 tests, OK** (33 before
this report's extension), and `run_lab.py` re-run. Comparison calibrations
throughout (midpoint-equivalent, `test_model.py`): flat 20 bp, dispersion
and per-Egg kappa 40 bp, quotient-norm kappa' 10 bp of range.

### 3.1 The measured grid

One 10,000-atom binary claim at price scale `S = 100`, terminal-ceil atoms
charged, from this report's `run_lab.py` run:

| clearing price | flat cash | dispersion `G` | per-Egg | quotient `R` | composite `G`+floor |
|---:|---:|---:|---:|---:|---:|
| 0 | 0 | 0 | 0 | 10 | 10 |
| 1 | 1 | 1 | 1 | 10 | 11 |
| 10 | 2 | 4 | 4 | 10 | 14 |
| 50 | 10 | 10 | 10 | 10 | 20 |
| 90 | 18 | 4 | 4 | 10 | 14 |
| 99 | 20 | 1 | 1 | 10 | 11 |
| 100 | 20 | 0 | 0 | 10 | 10 |

(The composite column is at the *uncalibrated* lab rates 40 bp + 10 bp-of-
range, so its midpoint is 20 atoms where the calibrated pure arms meet at
10; midpoint parity is trivially recoverable by rate choice — e.g. 2/1000 +
1/2000 gives 5+5 — and rates are strictly-after decisions.)

### 3.2 The zero-price laundering channel, disposition per base

Proposition 9 (`docs/research/RISK_SUMMED_POSITIONS.md:522-538`): at
boundary prices the dispersion kernel is `span(1) ⊕ R^{Z(p)}`, strictly
larger than the risk quotient, so risk transfer supported on zero-priced
outcomes is feeless however large its model-free range.

The executable falsifier, re-run for this report — payoffs
`(10^30, 0, 0)`, prices `(0, 0, 100)`, model-free range `10^30`:

| base | terminal-ceil atoms charged |
|---|---:|
| flat cash-notional | **0** |
| simplex dispersion | **0** |
| per-Egg leg | **0** |
| quotient-norm (1/1000) | **10^27** |
| composite floor (1/1000 floor) | **10^27** |

Three of four pure arms charge exactly zero — the channel is a property of
**every consideration-proportional base**, not of dispersion (`GOAL.md:
300-307`; a zero-priced leg has zero consideration, so flat cash and
per-Egg fall into the same kernel). Fragmentation plus the terminal-ceil
close cannot rescue any of the three: the carry never becomes nonzero
(`test_terminal_ceil_cannot_rescue_the_dispersion_hole`). Only the
price-free arms charge it, at exactly `kappa'·R` at every tested scale up
to `10^30`.

**The channel is live at the byte plane, not hypothetical.** The relation's
`validate_prices` (`crates/clutch-batch/src/relation_v1.rs:903-928`)
refuses only `price > price_scale` and a broken simplex sum: **a clearing
price of exactly 0 is an admissible candidate coordinate, and no tick floor
exists.** And a tick floor would not be a fix: computed with the lab's own
`fee_quote` at the relation's real `PRICE_SCALE = 10,000`
(`relation_v1.rs:100`), a one-tick-priced transfer of range `10^8` is
charged 40 atoms by 40 bp dispersion (exact:
`3999600000000/100000000000`) against 100,000 atoms by the 10 bp
quotient floor — a **2,500x** leak. Per unit of range the one-tick
dispersion charge is `kappa·(S-1)/S^2 ≈ 4·10^-7`, i.e. **0.004 bp of
range**: a tick floor bounds the hole at essentially free.

Why the channel matters even though the outcome is "worthless": the
venue's own solvency machinery prices that same transfer at its full
model-free range — the writer locks `R(a)` collateral (RISK_SUMMED §1.3).
A transfer the collateral plane treats as maximal risk moved is one the fee
plane treats as nothing. Wash-resistance claims become conditional
("strictly costly, except through the documented feeless channel"), and
clearing prices are proposer-chosen coordinates within order-limit
constraints — thin books can be steered toward the boundary.

### 3.3 Arithmetic invariants and kernel behavior

All lab-verified (bounded exhaustive), all consistent with the proofs:

| property | flat | per-Egg | `G` | `R` | composite |
|---|---|---|---|---|---|
| vanishes on `span(1)` (complete sets free) | **no — taxes them** | **no — taxes them** | yes | yes | yes |
| kernel at interior prices | supported-on-`Z(p)` vectors | supported-on-`{0,S}`-priced vectors | `span(1)` | `span(1)` | `span(1)` |
| kernel at boundary prices | enlarged | enlarged | **`span(1) ⊕ R^{Z(p)}`** (Prop 9) | `span(1)` | **`span(1)` — exactly, verified exhaustively** |
| complement symmetry (YES vs NO) | no | no | yes | yes | yes |
| identical-payoff refinement invariance | yes | **no — binning changes the fee** | yes | yes | yes (verified) |
| subadditivity (netting discount) | n/a (not a seminorm on `Q`) | no discount (`G ≤` per-Egg always, strict on complete sets) | yes | yes | yes (verified) |
| fee/consideration on cheap claims | constant | `→ kappa` | bounded by `kappa` | **unbounded (`kappa'/p̂`)** | `kappa(1-p̂) + kappa'/p̂` — floor term unbounded |

RISK_SUMMED §3.1's necessary condition — a principled base must factor
through the risk quotient `Q` as a seminorm — is *failed outright* by flat
and per-Egg (they charge risk-free complete sets; the lab shows flat's
charge strictly increasing under `a → a + c·1` and per-Egg charging
`(7,7,7)` at interior prices). Among the survivors, one crisp new fact from
this report's lab extension: **the composite is the only candidate that is
a genuine *norm* on `Q` at every admissible price vector** — `G`
degenerates at the boundary (Prop 9), `R` is a norm but ignores prices;
the composite's kernel is exactly `span(1)`, boundary included
(`test_composite_kernel_is_exactly_the_diagonal_at_every_price`).

### 3.4 Characterization strength

- **`G` is characterized, twice** (Props 11-12): the unique positively
  1-homogeneous functional reducing to `q(1-q)` on digitals and additive
  over layer-cake decompositions; and unique in the pairwise family under
  relabeling symmetry + homogeneity. What is not derivable is the binary
  calibration itself.
- **`R` is the unique price-free envelope member** (Prop 10:
  `sup_p G = R/4`, `||[a]||_Q = 2·sup_p Gamma_p(a)`).
- **Flat and per-Egg have no characterization anywhere in-tree** and fail
  the §3.1 necessary condition besides.
- **The composite does not destroy the characterization machinery — it
  re-parameterizes it.** This report made that executable
  (`test_composite_layer_cake_additivity_with_shifted_binary_calibration`,
  bounded exhaustive with independent layer recomputation): `R` is itself
  layer-cake additive (each digital layer contributes its height), so
  `kappa·G + kappa'·R` is exactly **the unique layer-additive
  1-homogeneous extension of the shifted binary curve
  `kappa·q(1-q) + kappa'`** — Prop 11's uniqueness argument verbatim, with
  the one non-derivable input (the binary calibration) moved from
  `q(1-q)` to `q(1-q) + const`. What *is* lost: membership in Prop 12's
  pairwise family (`R` is not a pairwise functional), and the pure "digitals
  are charged their own uncertainty and nothing else" story — the floor
  charges a near-certain digital `kappa'` per unit of range. That is not
  collateral damage; it is the entire point of a floor.
- A **max-form** hybrid `max(kappa·G, kappa'·R)` was considered and is
  rejected on the same axis: it keeps the seminorm axioms and the
  `span(1)` kernel but destroys layer additivity (max of layer-additive
  functionals is not layer-additive), so Prop 11's machinery — and with it
  the clean fragmentation/carry story — does not survive. The additive form
  is the principled composite.

### 3.5 Implementation state

| arm | state |
|---|---|
| zero-fee | the live byte truth everywhere |
| flat cash | **complete** `FeeBaseV1::FlatNotional` arm in the relation with carry, conservation, exactness checks (`clutch-batch/relation_v1.rs:217-225`) — unreachable from the program |
| dispersion | `dispersion_fee_step` implements the §4 equation in checked `u128` (`programs/solana-layout/src/portfolio_settlement.rs:388`) — orphaned inside its own module; `prepare_full_pair` never calls it; callers are its own tests. Not in the relation. |
| per-Egg | Python lab only (arm 3, landed 2026-08-19) |
| quotient-norm | Python lab only (arm 6, landed 2026-08-19) |
| composite | Python lab only — `composite_floor_quote`, added by this report's commit as one exact rational over one common denominator with one carry (the single-`FeeBaseV1`-variant runtime shape; two parallel pipelines would pay up to one extra terminal atom per intent) |

Shared infrastructure, all built and idle: `IntentFeeCarry`
(`clutch-liveness:1128-1245`) already implements the signed-intent-domain
terminal-ceil carry with fragmentation-invariance tests and has zero
consumers; the fee-capable layout plumbing of §2. B2's ordering violation
stands for every arm: five bounds (max coefficient, price scale, lot count,
kappa, intermediate width) were to be frozen before implementation and none
is — `dispersion_fee_step`'s domain is "whatever does not overflow", not an
audited envelope.

### 3.6 Small-size behavior: the 1-atom / 10,000 bp finding

The terminal-ceil close charges a minimum of one atom per fee-bearing
intent. The laboratory's own fee vector `FEE-001`
(`research/economics/fixtures.py:885-900`) records the boundary case: a
dust fill whose exact per-side fee is `40000/10^7` of an atom — floor 0,
carry, then **one atom at intent close on one atom of consideration:
10,000 basis points on the smallest fill** (`FEE_GEOMETRY.md:157-162`).
This is a property of the terminal-ceil close, **not of any base** — every
arm inherits it (the quotient arm at 1/1000 likewise ceils any range
≤ 1,000 to one atom, up to 100% of a 1-atom range). It does not
discriminate between candidates; it is the price of making fragmentation
and dust-cycling strictly costly (wash negativity holds *only* under
terminal-ceil, findings §4.1), and it belongs in the signing-UI language
whatever base wins.

## 4. What cannot be measured, and the descope question

Four of `FEE_GEOMETRY.md` §6's eight axes — depth, participation, fill
rate, route leakage — require an order-flow generator, an elasticity model,
and a counterparty model. None exists in the tree (findings §3); building
one is a research program, not a lane (register B3). Two §7 criteria
("user costs no worse than the lowest sustainable control on primary payoff
families", "positive contribution under conservative route elasticity")
depend on exactly those axes, and `ECONOMICS.md` §5's "choose the lowest
rate satisfying market-quality floors" instruction is unexecutable without
them.

So the honest form of any V1 selection is the one findings §6 recommends
and REVENUE_POLICY_V1 already assumes without ratification
(`:451-455`): **declare the market-quality axes out of scope for V1
fee-base selection, and have the document say the base was chosen on
arithmetic invariants and laundering resistance alone.** This report is
written on exactly that evidence and is therefore *conditional on ember
ratifying the B3 descope*; the alternative (fund the simulator) converts B1
from decidable-now into a multi-month research dependency. What the descope
costs, stated plainly: nothing in this report can say whether any nonzero
fee at any rate loses more volume than it earns. That question returns with
the rate decision, where it belongs — measurable on a live venue, not in a
simulator we don't have.

## 5. Interactions

1. **RevenuePolicy sequencing (B4 before B1's bytes, not before B1's
   selection).** Findings §6 orders the *machinery*: destination →
   carry → policy sibling → gates. The register marks B1 rank 2 "behind
   the destination decisions B4". Selecting the base *shape* now violates
   nothing — it relaxes no gate and builds no machinery — and it
   unblocks work the destination decisions do not touch: B2's bounds have
   an arm to bind to, `FEE_GEOMETRY.md` stops presenting four arms as
   co-equal, and the §10.5 zero-price fixture becomes concrete. The
   destination design is explicitly base-agnostic within §8.3's four
   requirements (exact rational quote with admission-frozen denominator;
   admission-computable worst-case bound; checked `u128`; documented
   zero-price disposition). The composite meets all four: its denominator
   is `kappa_den·S²·kappa'_den`, its worst-case bound is
   `kappa·R/4 + kappa'·R` by Prop 10, its width analysis is B2's job, and
   its zero-price disposition is *charged, at the floor* — proved by
   fixture rather than documented as accepted.
2. **`GENERAL_CLEARING_POLICY_V1`'s `FeeBaseV1::None` pin (A1).**
   Untouched in every branch. The pin is deliberately non-preempting; a
   fee-bearing profile is a *sibling const with a new digest* (§8.1), never
   an amendment of the frozen one, and zero-fee routes keep their gates
   permanently ("a zero-fee route's gate is a feature, not debt",
   REVENUE_POLICY §9). A1 can freeze before, after, or without this
   decision. Ditto D1/D2 promotion: fee truth is orthogonal to the walk
   plane's evidence status.
3. **The promotion-gate rewrite (rides H1).** §7 must be rewritten
   regardless of arm: (a) replace the Verus+Rocq closure demand with the
   substrate that actually carries proofs here (per the ADR-0003
   supersession analysis — Lean-primary, with the house rule that
   constraint-level objects are Lean-authored; the six named properties
   keep their *content*, only the prover changes); (b) delete the four
   unmeasurable axes per the B3 descope and say so; (c) keep the
   adversarial-encoding, comprehensibility, and no-liveness-dependence
   criteria, which are satisfiable and load-bearing. Until this rewrite,
   *no* base can ever be promoted, including zero-fee's successor — the
   gate is a permanent contradiction sitting in a canonical doc.
4. **B2 bounds.** Freeze after the arm is picked (its stated ordering).
   Under the composite the five bounds gain a sixth: the floor rate. Under
   any single-rate arm they stay five.
5. **C5's `FeeCarryAccount` standing blocker** retires only via this
   cluster, in B4's dependency order; the selection here changes which
   carry denominator that account freezes, nothing about when.

## 6. Recommendation

**Select the additive composite base: `kappa·G(a,p) + kappa'·R(a)` —
atomic simplex dispersion with a price-free quotient-norm floor — as the
V1 fee base, jointly with ratifying the B3 descope and commissioning the §7
rewrite; keep every byte at `FeeBaseV1::None` until the B4 destination
lands and the §10 falsifiers exist.**

The case, from the tree's own evidence:

- Flat cash and per-Egg are eliminated, not merely outscored: both tax
  risk-free complete sets (the venue's own §1 objective forbids this),
  both are complement-asymmetric, per-Egg is refinement-sensitive (a
  binning manipulation surface) and never cheaper than `G` anyway — and
  **both still share the zero-price hole**. The benchmark the dispersion
  base was built to beat has now been run against it and lost on every
  measured axis while retaining the sole defect it was supposed to lack.
- Pure `G` is the best interior-price base and is uniquely characterized —
  and carries a proved, in-relation-live, tick-floor-unfixable evasion
  channel (§3.2: price 0 is an admissible clearing coordinate; a tick
  floor bounds the leak at 0.004 bp of range). Selecting it means
  documenting a feeless channel as accepted policy in the threat list of
  the fee's own canonical doc.
- Pure `R` prices the channel but abandons the market's information
  entirely: constant absolute fee per unit of range at every price,
  fee/consideration unbounded on cheap claims as its *entire* incidence,
  and it overcharges consensus-priced flow — the §3.4 fork chosen at full
  scale.
- The composite takes `G`'s interior incidence and `R`'s kernel. This
  report modeled it (the register's option 5, previously "not modeled
  anywhere yet") and measured: kernel exactly `span(1)` at every price
  including the boundary — the only candidate that is a norm on the risk
  quotient unconditionally; the laundering fixture charged at exactly the
  floor (`10^27` atoms on the `10^30` row) with fragmentation-proof carry;
  every seminorm axiom preserved, exhaustively; and the Prop-11
  characterization machinery intact with the binary calibration shifted to
  `kappa·q(1-q) + kappa'` — the composite is the unique layer-additive
  extension of its own binary curve, so the "characterized, not
  constructed" property survives re-parameterized. The runtime shape is
  one `FeeBaseV1` variant with two rate fields, one common-denominator
  rational, one carry — `IntentFeeCarry` works unchanged.
- The economics of the floor answer the §3.4 fork coherently rather than
  picking a side: *interior* risk is charged under the market's measure
  (insurance is cheap when the market deems it unlikely), while the floor
  charges every transfer a small constant per unit of **exactly the
  quantity the venue's solvency machinery locks against it**. The fee
  plane and the collateral plane stop disagreeing about whether a
  boundary-priced transfer is "nothing" or "maximal".

The rate pair stays open, strictly-after, per the register. The lab
calibrations (40 bp + 10 bp-of-range) are comparison arms; the single
observation worth carrying to the rate decision is that a floor
deliberately sized well below the dispersion midpoint (the tree's §3.4
tail-liquidity note pushes the same direction) keeps consensus flow nearly
free while still charging the laundering channel `kappa'·R` — 2,500x the
tick-floor leak at the tested rates.

### The strongest counterargument, stated fairly

**Select pure `G` and accept the documented channel** — the route
REVENUE_POLICY §10.5 explicitly provides. The grounds are real:

1. *The hole may not be worth a second rate.* A transfer supported
   entirely on outcomes the market prices at zero moves risk **the
   market's own measure says is worthless**; charging it is exactly the
   philosophical concession the dispersion base exists to refuse. The
   solvency plane already locks `R(a)` against the writer regardless, so
   the channel endangers revenue and wash-neutrality claims — never
   solvency. A venue may reasonably say: feeless movement of
   market-worthless risk is not laundering, it is the definition working.
2. *Simplicity is a promotion criterion.* §7 demands the explanation
   "remain comprehensible in the signing UI". One base, one rate, one
   sentence ("you pay for uncertainty moved") is materially easier than a
   two-term fee whose floor needs the quotient norm explained. The
   composite doubles B2's frozen-rate surface and adds a two-rate digest
   encoding.
3. *The composite reintroduces `R`'s overcharge at the floor scale.* The
   floor's fee/consideration ratio is `kappa'/p̂`, unbounded as `p̂ → 0`;
   at the lab rates a one-tick-priced buyer pays roughly 10x its
   consideration in fee. Lottery-ticket flow — the one place §3.4 says `R`
   is worst — is exactly where the floor binds, and §2.4 already shows the
   tail's writers are the most capital-taxed participants; the floor taxes
   the tail's buyers too.

The honest weighing: counterargument (1) is a defensible value call that
ember alone can make — it is the one place this report's recommendation
rests on judgment rather than measurement. Against it stand two measured
facts: the channel is *live at the byte plane with no tick floor even
available as a mitigation* (§3.2), and accepting it makes every wash-cost
and Sybil-negativity claim in `ECONOMICS.md` conditional on an exception
documented three documents away. (2) and (3) are rate-scale objections to
a base-shape decision: a small floor keeps the second sentence short
("plus at most `kappa'` per unit of maximum payoff") and the overcharge
small, and the rate decision is explicitly still open. If ember takes the
counterargument, the fallback ranking on this evidence is `G` alone with
the channel documented — never flat, per-Egg, or bare `R`.

## 7. Execution cost of each option

Common to every nonzero arm (the B4/§8 machinery, costed there, listed for
scale): authenticated destination, carry account family, fee-bearing
policy sibling const + domain-validator admission, **candidate ABI
version** carrying the fee/rebate vector (the compact candidate has no fee
field at all), five gate relaxations per §9, eight §10 falsifiers, B2
bounds frozen and the implementation re-verified against them, §7
rewritten (H1 rider), FEE_GEOMETRY/ECONOMICS updated from "arms" to "the
base".

Marginal cost per option, on top of or instead of that:

- **Zero-fee-forever (option 6).** Code: none. Docs: FEE_GEOMETRY §6/§7
  retired or re-scoped honestly; ECONOMICS §6 stays `unbounded`. Cost:
  the fee-capable layout, `IntentFeeCarry`, the FlatNotional arm, and the
  whole lab become permanent dead weight; C5's `FeeCarryAccount` blocker
  and this fork reappear in every future review; the break-even story is
  "the venue is a public good" — a legitimate Track-A/B posture, but it
  should be *declared*, not defaulted into.
- **Flat cash.** Cheapest relation-side (the variant is complete; digest
  and `validate()` done; only unreachable). But it requires *reversing*
  two canonical commitments in prose (`FEE_GEOMETRY.md` §1's objective;
  `ECONOMICS.md` §5's kernel-operations-free list is only safe because the
  base ignores complete sets) — and it still needs the entire common
  machinery while keeping the laundering hole. Cheap code, expensive
  truth.
- **Per-Egg.** Everything `G` costs (it is not in the relation either)
  plus surrendering refinement invariance. Dominated on every measured
  axis; exists to be the control. No serious execution path.
- **Pure `G`.** New `FeeBaseV1::Dispersion { bps }` relation arm (the
  pairwise loop, `S²` denominator, carry) + tests; retire or subsume the
  orphaned `dispersion_fee_step`; B2's five bounds; **plus the
  §10.5-route-one documentation act**: the accepted feeless channel
  written into FEE_GEOMETRY §5's threat list as policy and frozen as a
  regression fixture proving the zero charge.
- **Composite (recommended).** The pure-`G` list, with the arm carrying
  two rate fields and one extra `max/min` scan (the `R` term is two
  comparisons per outcome — negligible next to the pairwise loop), digest
  encoding for the rate pair, B2 bounds + one (the floor rate), and the
  §10.5 fixture proving the *charge* instead of documenting the hole. Lab
  modeling is already done (this commit: `composite_floor_quote`, 7 tests,
  the `run_lab.py` composite rows). The marginal cost over pure `G` is
  small and buys the kernel; the marginal cost over doing nothing is the
  same B4-dominated mountain every nonzero arm pays.
- **Pure `R`.** Simplest possible arithmetic (`max - min`, denominator 1,
  no `S²`); no hole; but reverses the in-tree default recommendation
  (`docs/implementation/ECONOMICS_ADMISSION.md` §4 names atomic dispersion
  the recommended V1 default) and adopts the unbounded fee/consideration
  incidence at full scale — a doc-and-philosophy reversal larger than its
  code savings.

---

*Lab changes committed with this report:*
`research/economics-admission/model.py` (+`FeeBasis.DISPERSION_RANGE_FLOOR`,
+`composite_floor_quote`), `test_model.py` (+`CompositeFloorTests`, 7
tests; suite 33 → 40 OK), `run_lab.py` (composite rows in the fee grid and
the zero-price laundering fixture). Verified with the lab's own unittest
module only. No consensus code, no policy const, no gate, and no canonical
doc was modified.
