# Fee geometry — the study three generations owed

Status: **DESIGN STUDY, decision-ready.** Read-only on code; no byte, gate, or
rate changes with this document. It does three things: (1) recovers what
gen-1 actually decided about fees and why, what gen-2 built for it, and what
gen-3 ships instead; (2) surveys what a claim-based fully-collateralized
venue wants from fee geometry, against the field, judged against this tree's
actual structure; (3) recommends what v1-devnet ships, what the smoke can
demo, and the post-smoke implementation shape with its Lean obligations.

**This document is also the N-1 reconciliation** the aspiration ledger asked
for (`docs/evidence/ASPIRATION_LEDGER_2026_08_27.md:2120-2126`): it records, in §4.1, that
`fee_basis_points` is the deliberate V1 placeholder and the gen-1 composite
remains the selected target shape — the closure N-1 said was "cheap and
honest" and that silence was not.

Citations: `file:line` paths are this tree unless prefixed `dragons-clutch/`
(the compost repo at `~/dev/dragons-clutch`); `legacy-commit` hashes are that
repo's history.

---

## 1. What three generations did (recovered)

### 1.1 Gen-1 formally eliminated flat and selected a composite

Gen-1's objective, verbatim (`dragons-clutch/docs/FEE_GEOMETRY.md:17-21`):

> The venue should charge for transferring contingent risk, not for moving
> claimant principal, redeeming a correct claim, or carrying a risk-free
> complete set.

On that objective it defined the **state-contingent dispersion** of a
transferred payoff vector `a` at clearing prices `p` on scale `S`:

```
G_num(a,p) = sum_{i<j} p_i * p_j * |a_i - a_j|        G = G_num / S^2
```

— for a single outcome `k`, exactly `q * p_k * (S - p_k) / S^2`, the
`q·p(1-p)` uncertainty curve — and the price-free **quotient/range norm**
`R(a) = max_i a_i - min_i a_i`, which is the same functional the solvency
machinery locks as collateral. It then ran the decision as a formal
selection (`dragons-clutch/docs/decisions/REPORT_fee-base-selection_2026-08-20.md`),
with a bounded-exhaustive lab, and the verdict on flat is not a preference
but an elimination (REPORT §6):

> Flat cash and per-Egg are eliminated, not merely outscored: both tax
> risk-free complete sets (the venue's own §1 objective forbids this), both
> are complement-asymmetric, per-Egg is refinement-sensitive (a binning
> manipulation surface) and never cheaper than `G` anyway — and **both still
> share the zero-price hole**.

The zero-price hole is the reason a *composite* was selected rather than
pure `G` (REPORT §3.2): at boundary prices the dispersion kernel enlarges
(`span(1) ⊕ R^{Z(p)}`, RISK_SUMMED Prop. 9), so a transfer supported on
zero-priced outcomes is feeless however large its model-free range. The
executable falsifier — payoffs `(10^30, 0, 0)`, prices `(0, 0, 100)` —
charged **zero** under flat, per-Egg, and pure dispersion, and `10^27` atoms
under the range floor; the channel was live at the byte plane (a clearing
price of exactly 0 was an admissible candidate coordinate in
`clutch-batch/relation_v1.rs`, and a tick floor bounds the leak at 0.004 bp
of range — not a fix). The report's sharpest line on why it matters:

> A transfer the collateral plane treats as maximal risk moved is one the
> fee plane treats as nothing.

So ember adopted, by delegation on the weakest-choice principle
(`dragons-clutch/archive/gen1/docs/decisions/ADOPTED_2026-08-20.md:52-56`):

> **Fee base: the composite `kappa*G + kappa'*R` SHAPE is selected**; both
> rates remain undecided; every byte stays FeeBaseV1::None until the
> destination lands; the market-quality descope is ratified and the
> FEE_GEOMETRY promotion criteria are rewritten per ADR-0005
> (REPORT_fee-base-selection). Reversible until a rate freezes.

What made the composite the principled pick rather than a patch (REPORT
§3.3-§3.4): it is **the only candidate that is a genuine norm on the risk
quotient at every admissible price vector, boundary included** (kernel
exactly `span(1)`, verified exhaustively); `G` is *characterized* — the
unique 1-homogeneous, layer-cake-additive extension of the binary `q(1-q)`
curve (Props. 11-12) — and the composite re-parameterizes rather than
destroys that characterization (unique layer-additive extension of the
shifted curve `kappa·q(1-q) + kappa'`); a max-form hybrid was considered and
rejected because it destroys layer additivity and with it the clean
fragmentation/carry story.

What gen-1 deliberately left open: **both rates** (the 40 bp + 10 bp lab
numbers are comparison arms, and its own 20 bp arm is called "an experiment,
not a natural constant"); the five-then-six **bound freezes** (register B2 —
ordering already violated then, named as owed); the **market-quality
descope**, ratified with the cost stated plainly (REPORT §4): *nothing in
the selection can say whether any nonzero fee at any rate loses more volume
than it earns — that question returns with the rate decision, measurable on
a live venue rather than in a simulator the tree does not have*; and the
**treasury**, made structurally undecidable
(`REVENUE-TREASURY-UNSET-SENTINEL1`,
`dragons-clutch/research/batch-policy-identity/src/revenue_policy_v1.rs:58-63`)
— gen-1 built a protocol that could not take a fee until ember answered
(ledger M-26). Its strongest self-counterargument (REPORT §6, "stated
fairly") is preserved and still worth reading: pure `G` with the documented
channel is a defensible value call — "feeless movement of market-worthless
risk is not laundering, it is the definition working" — and the report
answers it with the two measured facts (channel live at the byte plane, no
tick floor available; wash-cost claims become conditional).

### 1.2 Gen-2 built the machinery

Gen-2 (2026-08-23→24, ~2,700 near-message-less commits) implemented the
selection. What survives in the compost tree and its history:

- **`SelectedCompositeFeeV1`**
  (`dragons-clutch/crates/clutch-fee-runtime-contract/src/selected.rs:63-80`):
  one immutable, nonzero composite selection per selected candidate —
  `dispersion_bps: u32`, `floor_range_bps: u32`, `carry_denominator: u128`,
  bound to a revenue policy, a treasury owner, and a treasury Position. Its
  assessment object names the single rounding event per transition
  (`AssessmentBoundaryV1::FragmentFloor` / `TerminalCeil`, selected.rs:14-21)
  — exact floor plus persisted carry on fragments, exact ceiling closing
  carry to zero at the terminal owner event.
- **`RevenuePolicyV2`** (legacy-commit `c7c1b8ba`, 483 lines,
  `research/batch-policy-identity/src/revenue_policy_v2.rs`): the registered
  immutable 80-byte policy record carrying **both composite rates**
  (dev calibration 40 bp dispersion + 10 bp range floor), the
  60/0/40 maker/executor/treasury split, a maker-weight authority that only
  admits weights *certified by a private settlement traversal* ("no public
  row may supply or override a weight"), and the doctrinal detail that **a
  zero-fee policy is refused as a record — "absence, not a zero-valued
  record, is the zero-fee representation."** Recalibration is a *new
  registered record*, never a mutation: legacy-commit `eccd388a` ("Keep
  revenue policy V2 open to registered calibrations") removed the 25%
  treasury envelope floor precisely so a future registered calibration (a
  90/10 split in its test) validates as a sibling.
- **Allocation machinery**
  (`dragons-clutch/crates/clutch-fee-runtime-contract/src/allocation.rs:143-148,
  332-334`): owner-level fee debits allocated across signed intent envelopes
  by canonical prefix (account-meta-reorder-proof), and recipient splits by
  Hamilton largest remainder over verified standing-maker weights.
- **Registry coordinates**, allocated and parked
  (`dragons-clutch/programs/solana-layout/src/registry.rs:254-265,
  1255-1285`): `general-v2-selected-fee-record-v1-account` (0x82),
  `general-v2-owner-fee-carry-v1-account` (0x83), owner-fee-finalization v2,
  payer-allocation (0x84) — all `ReservedDisabled`, all doc-commented.

So what could RevenuePolicyV2 express that a flat `fee_basis_points` cannot?
**A two-rate uncertainty-shaped charge per candidate; owner-level (not
per-leg) assessment with exact carry across fragmentation and a terminal-ceil
close; an authenticated destination with maker rebates earned by certified
standing, not self-report; and recalibration as registered succession rather
than mutation.** Every one of those is zero in the successor (ledger
GITSCAN-2 §D.1 item 5: "Verified: `RevenuePolicy`, `recipient allocation`,
`fee manifest` — zero").

### 1.3 Gen-3 ships the eliminated arm — but on a different plane

The successor ships flat `fee_basis_points` in **67 places** across
`crates/` and `programs/` (count re-verified for this study). The mechanism
is genuinely good — better than gen-1's flat arm ever was:

- **Per-market immutable config.** `DirectExecutionConfigV1 { price_scale,
  fee_basis_points: u16, fee_recipient }`, content-selected, hostile-decoded
  only after descriptor-to-record selection, `fee_basis_points ≤ 10_000`
  enforced at construction (`crates/dclutch-direct-codec/src/successor.rs:327-352`;
  the Lean side pins the same bound: `.scalarLe (s .policyFeeBps)
  (s .feeDenominator)`, `formal/dclutch-semantics/DClutchSemantics/DirectOrdinaryV3.lean:509`,
  with `feeDenominator = 10000`, `Direct.lean:20`).
- **Consent geometry.** Both makers sign the exact rate: the transition
  refuses `sellerFeeBps ≠ policyFeeBps` and `buyerFeeBps ≠ policyFeeBps`
  (DirectOrdinaryV3.lean:507-509), and the campaign exercised the refusals
  ("a venue fee rate the makers did not sign", "a seller fee rate the policy
  did not set", `docs/evidence/DIRECT_FAMILY_CAMPAIGN_2026_08_27.md:112-113`).
- **Fees as differences of floors of cumulative gross.** Registered resting
  orders persist cumulative gross and cumulative fee; each fill recomputes
  both sides' fee from the record's own cumulative gross and **refuses a
  disagreement**, then charges exactly
  `floor(cum_after·bps/10^4) − floor(cum_before·bps/10^4)`
  (`DirectRegisteredFillV4.lean:653-706`;
  `crates/dclutch-direct-aot-v3-contract/src/registered.rs:334-352`). The
  algebraic heart is proved in general form: `cumulative_fee_telescopes`
  (`DirectProofs.lean:145-153`) takes an **arbitrary monotone**
  `feeAt : Nat → Nat` and shows matcher fragmentation cannot change a
  registered order's final fee. A **zero combined fee is a no-transfer path,
  not a refusal** — inherited doctrine, spelled out with its reason
  (`DirectRegisteredFillV4.lean:808-821`): a per-fill delta of zero is
  routine under cumulative floors, and refusing it would refuse ordinary
  small fills at realistic rates.
- **Dealer has its own leg**: policy `feeNumerator/feeDenominator` with
  `feeDue = ceilDiv(base·num, den)` charged as incremental differences of
  ceilings, fragmentation-independence proved
  (`DealerLiquidity.lean:192-206`), fee custody distinct from liveness
  custody, sell fees sourced from principal (commit `a4d6fb56`).
- **General — the batch-auction family — currently charges nothing at
  all.** No fee field exists in its config or codec; the only mention is a
  disclaimer ("never collateral or future fees",
  `crates/dclutch-general-config-contract/src/v3.rs:89`).
- **Claims, custody, redemption charge nothing** — zero fee terms in the
  Claims/Economic kernels (`ClaimsRepresentation.lean`,
  `EconomicKernel.lean`), which is gen-1's objective holding: complete-set
  split/merge and redemption of a correct claim are free.

The ledger's N-1 charge stands as history: this is the arm item 9
eliminated, running with no decision record, with the promotion gate never
closed and B2's bounds never frozen. §4.1 answers it. But the study has to
add what N-1 could not see, because it changes the verdict's *scope*:

**Gen-1 eliminated flat on the batch plane. Gen-3 runs flat on the bilateral
plane, where two of flat's three formal defects are structurally vacuous.**
Specifically:

1. *Complete-set taxation* — gen-1's headline defect — cannot occur on
   Direct fills: a fill transfers claims on a single signed outcome against
   collateral (one Claims leg, epilogue-checked to a single tail coordinate,
   DirectOrdinaryV3.lean:544-563); a complete set never passes through a
   fill. Split/merge live in the fee-free Claims kernel.
2. *Refinement sensitivity* (the per-Egg binning surface) likewise has no
   purchase: nothing about a Direct fill re-bins a payoff vector.
3. What **survives** is *complement asymmetry*: acquiring exposure `q·e_k`
   by buying outcome `k` at price `p` costs fees on gross `q·p/S`, while the
   identical economic risk acquired by buying the complement basket costs
   fees on `q·(S−p)/S` — a `(S−p)/p` ratio, unbounded on cheap claims. Flat
   charges the *label*, not the risk. `G` charges `q·p(S−p)/S^2` both ways.
4. The *zero-price channel* also survives ("a fill at an execution price of
   zero quotes nothing, charges nothing, and still transfers Claims" —
   witnessed, DirectRegisteredFillV4.lean:805-807) — but on the bilateral
   plane it is economically a consensual free transfer between two signers,
   and the Claims→Custody→Token-2022 direction (WAVE.md cycle 2) makes free
   transfer available anyway. **The channel's true habitat in this tree is
   the General batch relation, where clearing prices are computed rather
   than bilaterally signed — and General currently has no fee for it to
   leak from.** When General grows a fee, gen-1's threat model applies there
   natively, floor included.

---

## 2. What this venue wants from fees, against the field

### 2.1 The structural facts that judge every candidate

- **Complete sets mint and redeem at par, free.** There is no swap pool at
  the core; fee geometry lives on **fills** (Direct bilateral, General
  batch, Dealer quoted) and could live on **redemptions** (it must not —
  §3, G7). Any intuition imported from AMM fee design must survive this
  translation or be discarded.
- **The cumulative-floor discipline is the load-bearing constraint — and
  the extension point.** Everything charged must be a difference of a
  monotone integer function of persisted cumulative state, recomputable and
  refusable by the transition, with `cumulative_fee_telescopes` already
  proved for *any* monotone `feeAt`. A geometry that needs floating point,
  or per-fill rounding that does not telescope, is dead on arrival.
- **Fees are signed.** The maker signs the exact policy rate today; any
  richer geometry means the maker signs the geometry (the config is already
  content-selected by digest) plus an **admission-computable worst-case
  bound** — the same requirement gen-1's destination design imposed
  (REPORT §5.1: exact rational over an admission-frozen denominator,
  worst-case bound computable at admission).
- **The κ/Mango lens.** The tree already prices observable-manipulation for
  solvency: `total_principal ≤ κ · manipulation_cost_lower_bound`
  (`docs/research/CHAIN_STATE_SOURCES_2026_08.md:1049`), the predicate shape
  the Mango exploit violated. A fee that adapts to an observable creates a
  *second* incentive to move that observable. The discipline: adaptive
  inputs must be either **consensus-unmovable** (the clock), **already
  priced by κ** (the Source), or **the fee base itself** (cumulative gross)
  — and the marginal fee saved by moving an observable must be bounded below
  the cost of moving it.
- **The hot path is a budget.** W2 fought Hot from 2,949,172 down to
  831,953 CU against a 1.4M ceiling (WAVE.md, cycle-1 results). A fee
  geometry that reads and authenticates a new account on the fill path
  spends exactly the budget that was just recovered.

### 2.2 The field, judged against those facts

| venue | geometry | what it adapts to | verdict here |
|---|---|---|---|
| **Uniswap v4** dynamic-fee hooks | pool declares its fee dynamic; hook code chooses the fee per swap | anything the hook reads (volatility, inventory, oracles) | The expressiveness end-state — and the anti-model. Geometry-as-code moves the whole trust question into unverified hook logic. This tree's equivalent is a **versioned geometry enum in the content-selected config, specified in Lean** — never code. |
| **Curve** | imbalance-scaled fees on unbalanced liquidity ops | deviation from pool balance | The transferable idea is *charge flow that moves the system from balance, spare flow that restores it*. But the core venue has no pool to imbalance; only Dealer inventory qualifies (G4). |
| **LMSR / LS-LMSR** (Othman-Sandholm-Pennock-Reeves) | liquidity-sensitive spread, `b(q) = α·Σq`; revenue is vig inside the price rule | volume | Adaptivity can live in the *liquidity provider's curve* instead of the protocol fee. **Dealer already does this**: candidates sign their own `OutcomeCurve`s per outcome (DealerLiquidity.lean:150-160). Dealer competition is the adaptive spread; the protocol fee should not double-adapt on top. |
| **Polymarket** | zero trading fees | — | Proof zero-fee is a real growth posture. Gen-1 already ruled how it must be held: *declared*, never defaulted into (REPORT §7, zero-fee-forever arm). |
| **Kalshi** | taker fee `= ceil(0.07 · C · P · (1−P))` per contract, round-up, on general markets (lower schedules on designated series); maker side free/near-free | the traded price's own uncertainty | **The binary special case of gen-1's `G`, running in production**, ceiling rounding and all (the terminal-ceil analogue). Field validation that an uncertainty-shaped fee is shippable and explainable — "you pay for uncertainty moved" survives a retail signing UI. |
| **dYdX** | maker/taker bps tiered by 30-day account volume, maker rebates at top tiers | trader identity + volume | Identity-linked tiers are Sybil-food in an account-free venue. The salvageable form is **per-record concave tiering on cumulative gross** (G6): concave `feeAt` with `feeAt 0 = 0` is subadditive, so splitting across records or orders never reduces total fee — Sybil-resistance by the same subadditivity gen-1 prized in `G`. |

### 2.3 The desiderata, stated once

(a) complete sets and redemptions free — the objective, held by all three
generations; (b) exact-integer, monotone-cumulative, telescoping — the
discipline; (c) signable, with an admission-computable worst-case bound —
consent; (d) charge the risk, not the label — complement symmetry where the
plane allows it; (e) every adaptive input priced under the κ lens; (f) one
sentence in the signing UI, two for a composite ("plus at most `kappa'` per
unit of maximum payoff" — gen-1's own second sentence).

---

## 3. The candidate geometries

Every candidate below is stated in the only form that can ship: a per-fill
integer increment accrued into a monotone cumulative numerator `N`, fees
charged as `floor(N_after/D) − floor(N_before/D)` against a frozen
denominator `D`. This is exactly the shape `cumulative_fee_telescopes`
already covers; the carry is implicit in `N − paid·D`. Gen-1's
`IntentFeeCarry` and gen-2's `FragmentFloor`/`TerminalCeil` are the same
object one abstraction earlier.

### G0 — flat cumulative-floor bps (incumbent)

- **Adapts to:** nothing.
- **Exact form:** `N = cum_gross · bps`, `D = 10^4`. Shipped and proved
  (DirectProofs.lean:154-179; registered.rs:334-352).
- **Manipulation surface:** price-mislabel to the boundary makes the fee
  vanish (moot on Direct — free transfer exists anyway; real on General);
  complement asymmetry `(S−p)/p` (§1.3.3) — the identical risk pays up to
  the price-ratio more depending on which label carries it; wash trades cost
  `2·bps·gross` at interior prices, zero at the boundary.
- **Wire cost:** 2 bytes, paid.
- **Verdict:** the correct v1 placeholder (§4.1) and formally the eliminated
  arm; its surviving defects on the bilateral plane are complement asymmetry
  and nothing else of gen-1's list.

### G1 — dispersion-rate fills (`G` on the fill plane)

- **Adapts to:** the market's own uncertainty at the traded price — the
  observable is the *signed execution price already in the frame*; nothing
  new is read.
- **Exact form:** per-fill increment `q_i · p_i · (S − p_i) · κ_num`
  accrued into `N`; `D = κ_den · S^2`. With `S = 10^4` and `u64` fills the
  increment fits `u128` with wide margin; the cumulative width is a bound to
  freeze (B2's discipline, §4.3.5). Monotone by construction ⇒ the
  telescoping lemma applies verbatim.
- **Manipulation surface:** mislabeling the price toward a boundary cuts the
  fee — but the price is bilaterally signed and bracketed by both limits,
  and the boundary channel is exactly flat's. No new observable.
- **Wire cost:** `κ` replaces `bps` in the config; **one** new persisted
  cumulative scalar per record (`N` beside cumulative gross).
- **Note:** Kalshi runs this candidate's binary case at `κ = 0.07` today.

### G2 — the gen-1 composite on fills (`κ·G + κ'·R`)

- **Adapts to:** uncertainty, with a price-free floor per claim moved.
- **Exact form:** the single-outcome leg has `R = q`, so the floor term
  accrues on **cumulative filled quantity — a scalar the record already
  persists** (`sellerFilled`/`buyerFilled`). One common denominator
  `κ_den·S^2·κ'_den`, one floor, one carry — the single-variant shape the
  gen-1 report demanded (REPORT §3.5: "two parallel pipelines would pay up
  to one extra terminal atom per intent"). Net new state over G1: zero; net
  new config: the second rate.
- **Manipulation surface:** closes the boundary-free channel on fills —
  symbolically on Direct (free Custody transfer exists), **really on
  General**, where clearing prices are computed and gen-1's falsifier is
  native. Reintroduces `R`'s incidence at floor scale: fee/consideration
  `κ'/p̂`, unbounded on cheap claims — the report's own counterargument (3),
  answered there by rate scale (a small floor keeps the second sentence
  short) and still true.
- **Wire cost:** two-rate config variant; on General, the whole fee
  apparatus (which does not exist yet) — but gen-2's vocabulary and the
  registry coordinates (0x82-0x84) are already drawn for exactly this.
- **Verdict:** the revival target — **General first** (native threat model,
  greenfield, batch = where the payoff-vector `G_num` is even computable),
  Direct second and only for complement symmetry.

### G3 — time-to-resolution ramp (bracketed)

- **Adapts to:** slots-until-resolution-window — the adverse-selection
  clock. Information arrival concentrates near the Pyth print; a bracketed
  rate (higher near resolution, or lower early to seed liquidity — a rate
  question, not a geometry one) is the standard insurance-premium shape.
- **Exact form:** the naive "rate table times cumulative gross" breaks the
  discipline (a rate change would re-price *past* gross). The correct form
  is the accrual: `N += gross_i · rate_bracket(slot_i)`; `D = 10^4`.
  Monotone ⇒ telescopes. The bracket function is a small frozen table in the
  config (e.g. 4 × `(slot_offset: u64, bps: u16)`), signed as part of the
  config digest.
- **Manipulation surface:** none on the observable — the clock is consensus
  and unmovable; the market's resolution window is founding-frozen. Timing
  games at bracket edges are schedule effects, visible in the signed table.
  **The cleanest adaptive axis that exists.**
- **Wire cost:** ~40 bytes of config; zero new accounts; zero new reads on
  the hot path (the slot is already in the frame —
  DirectOrdinaryV3.lean:480-483 bounds validity windows against it).

### G4 — imbalance / inventory-sensitive (Curve / LS-LMSR lens)

- **Adapts to:** Dealer inventory utilization between the candidate's own
  `minimumInventory`/`maximumInventory` bounds.
- **Exact form:** banded multiplier on `feeNumerator` accrued per quote
  (same `N` trick; Dealer's ceiling discipline needs the accrual stated on
  the numerator to stay monotone).
- **Manipulation surface:** inventory is market-endogenous — trade to shift
  the band, trade back; the defense is the κ-style inequality (band spread
  strictly below round-trip fee cost), which is a real analysis obligation.
- **Verdict:** **parked.** The Dealer's signed curves are already the
  liquidity-sensitive spread — LS-LMSR's lesson delegated to dealer
  competition. Double-adapting at the protocol layer adds analysis burden
  for margin the dealer can price itself. Reopen only on evidence dealer
  competition stays thin.

### G5 — source-volatility-adaptive (Pyth confidence)

- **Adapts to:** the resolving Source's published confidence / realized
  variance.
- **Exact form:** bracketed multiplier from integer conf thresholds —
  expressible.
- **Manipulation surface:** **the Mango lens, verbatim.** The fee would read
  the same observable that resolves the market; whoever can move the Source
  moves both the resolution and the fee. κ already caps principal against
  manipulation cost; a fee reading the same observable adds a second,
  correlated incentive channel — and it pays **hot-path CU** to do it: a
  fill-time authenticated read of the Source on the path W2 just fought
  under the 1.4M ceiling.
- **Verdict:** **refused** for any near horizon. Price discrimination by
  volatility is the Dealer curve's job; the protocol buys manipulation
  surface plus CU for margin it does not need.

### G6 — volume-tiered concave (`dYdX` lens, per-record)

- **Adapts to:** the record's own cumulative gross — the fee base itself;
  no new observable exists at all.
- **Exact form:** piecewise-linear concave monotone `feeAt` (knots frozen in
  the config). This is the only candidate that is *literally just a
  different `feeAt`* — the discipline is native, the telescoping lemma
  already covers it, and only per-segment monotonicity is a new proof.
- **Manipulation surface:** none new. Concavity with `feeAt 0 = 0` gives
  subadditivity: splitting volume across orders, records, or Sybils never
  reduces the total fee — the tier is self-enforcing.
- **Wire cost:** a knot table in the config; zero new accounts.
- **Verdict:** the cheapest real sophistication; a values question rides it
  (concave tiers reward size — ember's call at rate time, not geometry
  time).

### G7 — redemption-side fees

**Refused, permanently, on the objective.** Gen-1: the venue does not charge
for "redeeming a correct claim" (`dragons-clutch/docs/FEE_GEOMETRY.md:17-21`).
A redemption fee is a tax on trusting the venue's own resolution — it decays
exactly the property a fully-collateralized claim venue sells. The field
does do it (Kalshi settlement fees on some series); this venue should wear
its refusal as a product statement. The Claims/Economic kernels carrying
zero fee terms is a feature to *keep*, not an omission.

### The geometry envelope (what all survivors share)

Every admissible candidate is: **a per-fill exact-integer increment
`g(q_i, p_i, slot_i)` accrued into a monotone numerator, charged as
differences of floors against one frozen denominator, signed by digest, with
an admission-computable worst-case bound.** G0, G1, G2, G3, and G6 are five
instances of one kernel. That kernel — not any single geometry — is the
correct thing to build and prove once (§4.3.1).

---

## 4. The recommendation

### 4.1 v1-devnet: flat stands — and this is its decision record (N-1 closed)

**Confirm flat.** `fee_basis_points` is the deliberate V1 placeholder;
**ADOPTED_2026-08-20 item 9's composite remains the selected target
shape**; nothing here reverses item 9, and nothing before the composite's
own gates close may promote flat beyond placeholder status. The grounds,
honestly:

1. On the bilateral Direct plane, flat's two disqualifying gen-1 defects
   (complete-set taxation, refinement sensitivity) are structurally vacuous
   (§1.3), the boundary channel is economically a free transfer that exists
   anyway, and the one real surviving defect — complement asymmetry — is
   acceptable for a devnet venue whose fee is an exercised-refusal
   demonstration, not a revenue system.
2. The mechanism under the flat rate — signed rates, cumulative floors,
   telescoping proofs, re-proof of persisted records, no-transfer zero path
   — is *better* fee infrastructure than any generation had, and it is
   exactly the substrate the composite needs (§3, G2: one new scalar).
3. Rates stay experiments: the campaign's 25 bp fixture inherits gen-1's own
   label for its 20 bp arm — "an experiment, not a natural constant." No
   rate is frozen by this document, so item 9's "reversible until a rate
   freezes" clause remains live.

The N-15 precondition transfers with this record: **the composite's
characterization is formalized in Lean before any rate freezes**
(`dragons-clutch/docs/design/NEXT_WAVE_ROADMAP_2026-08-20.md:98`), owned by
§4.3 below.

### 4.2 The smoke: demo config diversity, not new geometry

The cheap demonstration is the one already built: **per-market signed fee
configs at distinct rates** across the smoke's markets (e.g. the Pyth range
market at 25 bp, the mainnet-observer market at 0, the abandoned market at
100), with the conservation ledger publicly showing venue take equal to the
sum of floor-differences over each market's life — the journey already
counts fees per stage (commit `e9072f71`). That makes the true story public
("the fee is a signed, immutable, per-market config; a rate the makers did
not sign is refused") at zero new code. One optional slide of narrative:
Kalshi's production taker fee is `ceil(0.07·C·P·(1−P))` — the binary case of
the composite this venue selected on 2026-08-20; the successor's target is
the general-payoff version of the same law.

### 4.3 Post-smoke implementation shape (the FEE-GEO lane)

In dependency order:

1. **The geometry kernel, once.** Generalize `cumulativeFee` to the
   envelope: `feeAt(N) = N / D` over an accrued numerator with a per-fill
   increment function. Lean obligations:
   - `accrual_monotone` — appending a fill never decreases `N`;
   - `fee_telescopes` — **already proved for arbitrary monotone `feeAt`**
     (DirectProofs.lean:145); instantiate, don't re-prove;
   - `fee_bounded` — `feeAt(N) ≤ worst_case(config, intent)`, the
     admission-computable bound the signed intent carries;
   - `conservation` — the existing per-family fee-leg conservation restated
     over the kernel (`admitted_collateral_conserved` shape,
     DirectProofs.lean:128-133).
2. **General grows the composite.** The batch relation is where payoff
   vectors and computed clearing prices exist, where gen-1's falsifier is
   native, and where there is no incumbent fee to migrate. Revive gen-2's
   vocabulary: a registered immutable policy record carrying
   `(dispersion_bps, floor_range_bps)`, the treasury Position, and the
   split; the compost registry's 0x82-0x84 coordinates are the naming
   precedent. Composite-specific Lean obligations, porting RISK_SUMMED §3's
   statements per the promotion gate: `complete_set_invariance`
   (`G(a + c·1, p) = G(a, p)`), `complement_symmetry`, `homogeneity`,
   `partition_refinement_invariance`, `kernel_is_diagonal` (the composite's
   `span(1)` kernel at every admissible price — the fact that made it the
   selection), and the **zero-price fixture proving a charge at the floor**
   (gen-1's frozen-regression demand, REPORT §5's §10.5 route two).
3. **Direct keeps flat**; add G3's time-bracket table as the one adaptive
   axis worth its bytes if the smoke shows liquidity clustering at the
   close, and G2 as a config variant only if complement symmetry is wanted
   as a product statement.
4. **Destination:** the RevenuePolicyV2 revival is the M-26 answer's
   vehicle — treasury Position custody, maker rebates by certified
   standing, recalibration by registered sibling. The treasury pubkey stays
   **reserved to ember**, exactly as gen-1 built it to be.
5. **Freeze the bounds before implementing** — the ordering both prior
   generations violated and named: numerator width, price scale, lot count,
   both rates' domains, and the bracket-table shape, frozen as consts the
   Lean bounds reference.
6. **Standing refusals** (this document's negative space): source-adaptive
   fees (G5), redemption fees (G7), geometry-as-code (the v4-hook model),
   floats anywhere, and mutation of any fee record — calibration is
   registered succession (gen-2's rule, kept).

The rate pair remains strictly-after and ember's alone. The market-quality
descope means no simulator will answer the volume-elasticity question; the
smoke's live venue is the first instrument that can (gen-1: "measurable on a
live venue rather than in a simulator the tree does not have").

---

*Sources read for this study: gen-1's `FEE_GEOMETRY.md`,
`REPORT_fee-base-selection_2026-08-20.md`, `ADOPTED_2026-08-20.md`,
`RISK_SUMMED_POSITIONS.md` §3.4, `revenue_policy_v1.rs` (all
`dragons-clutch/`); gen-2's `clutch-fee-runtime-contract`,
`revenue_policy_v2.rs` (legacy-commits `c7c1b8ba`, `eccd388a`),
`registry.rs`; gen-3's `DirectProofs.lean`, `DirectOrdinaryV3.lean`,
`DirectRegisteredFillV4.lean`, `DealerLiquidity.lean`, `intent_v2.rs`,
`successor.rs`, `registered.rs`, `DIRECT_FAMILY_CAMPAIGN_2026_08_27.md`;
the aspiration ledger's GITSCAN-2 rows N-1, N-15, M-26, §D.1 item 5;
`CHAIN_STATE_SOURCES_2026_08.md` §6.5.*
