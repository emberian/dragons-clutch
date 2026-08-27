# The Disagreement Exhibit — implementation-ready design

Status: **DESIGN / APPROVED FOR BUILD.** Produced 2026-08-20 by the design
lane; every cited theorem, seam, and fixture was read, not recalled. The
implementation lane executes L1-L3 below; L4 (the filing clause) is the
orchestrator's, gated on L2 green. The claim-plane honesty bar is the
filing bar.

**The claim being witnessed** (IAC statement Position 3): "an automated
trader can state a precise belief, a density rather than a direction, and
the market prices disagreement between models instead of only between
moods."

**The exhibit in one paragraph.** Two named, textbook estimators are run on
one published synthetic price history. Each emits a density over the same
eight-knot degree-1 hat basis. A published deterministic book-former lowers
the *pair* of densities into a clearing book — per-knot quotes at each
model's own fair value, plus one "disagreement package" portfolio pair —
that trades exactly where and only where the densities differ. The book
places, freezes, clears, verifies, and settles end-to-end through the
general clearing plane in a local bank (the T2-8 walk, tags 47-59), with
conservation asserted to the atom and the pot provably empty. The cleared
integer price vector lies coordinatewise between the two beliefs, is
checkably the basis-moment vector of an explicit probability measure (the
degree-<=1 implied-measure theorem, instantiated by arithmetic), and
carries a recomputable zero duality gap under the frozen policy tuple. No
resolution ever occurs: the exhibit ends at the settled book, so neither
model is ever "right."

## 1. Quantity and knot grid

- Quantity: the 20-trading-day-ahead level of **SYN-1**, a synthetic price
  series published with the exhibit. No live data. SYN-1 is 120 daily steps:
  start 160.0, daily log-return N(0, 0.01) plus, with probability 0.10, a
  jump +/-0.06 — all constants published. The jumpiness is *declared
  design*: it is what makes a Gaussian estimator and an empirical estimator
  structurally disagree, and the page says so.
- Seed: `20260827` (the filing date, chosen by rule before generation).
  Realized terminal level: x0 = 156.48.
- Pre-registered horizon/grid rule (stated before looking at output):
  H = 20; grid = the Friday-clutch grid: knots [100, 120, 140, 160, 180,
  200, 220, 240], gap g = 20, degree 1, n = 8, D = 64, S = 10,000.
- Terms (bank fixture): degree-1 v3 TermsAccount — basis_degree 1,
  knot_count 8, knots as u128 cents (10,000..24,000), general spacing
  (uniform_log2_spacing = 0xFF, admitted at d=1), STAT-TERMINAL-01,
  EDGE-CLAMP-01, payout_map all PAYOUT_MAP_UNUSED, failure preset uniform
  refund 8/64 x 8, nonzero collateral_cap. **Must be a real degree-1 terms
  artifact** — degree-0 fallback would violate CURRENT_TRUTH section 7's
  no-impersonation rule.

## 2. The two models — genuinely different, non-theatrical

- **Model G ("the Gaussian")**: ML normal fit to SYN-1's daily log-returns
  (mu = -0.00019, sigma = 0.01477), projected 20 days as a lognormal over
  the terminal level, integrated against the clamped hats.
- **Model E ("the empiricist")**: the empirical distribution of all 101
  overlapping 20-day log-moves in the same history, applied to x0, averaged
  through the clamped hats.

Both are one screen of code with zero free hyperparameters beyond the
pre-registered rule. Honesty statement: each model is a named textbook
estimator; both ran once on the same published history; neither saw a
price or the other's output; nothing was adjusted after generation. The
disagreement is structural: jump variance inflates G's fitted sigma, so G
spreads mass into the shoulders while E keeps the realized peak.

Belief vectors (largest-remainder quantized to the price simplex,
per-mille of S):

```
knot:   100   120    140    160    180   200  220  240
v_G  = [  0,   98,  2961,  5696,  1213,  32,   0,   0]   (sum 10,000; forward $156.24)
v_E  = [  0,  127,  2662,  5945,  1266,   0,   0,   0]   (sum 10,000; forward $156.70)
delta = v_G - v_E = [0, -29, +299, -249, -53, +32, 0, 0]
```

Float honesty: estimation is float (disclosed); the script pins the two
quantized integer vectors as constants, checks regeneration reproduces
them (naming the libm boundary if a platform differs), and everything
downstream is exact integer/rational arithmetic matching the venue.

## 3. Portfolio construction

The **book-former** is a published deterministic function of the pair
(v_G, v_E) and nothing else. Stated honesty: the exhibit does not simulate
order-flow discovery between autonomous agents; it lowers a known
disagreement into the minimal book that expresses it.

1. **Per-knot fair-value quotes** (single-Egg orders, size z = 500): at
   each knot where the values differ, the higher-value model posts a buy at
   its own value tick and the lower-value model posts a sell at its own
   value tick (a zero value quotes a 0-limit sell). Knots where values
   agree get no orders. Each model single-sided per outcome (frozen
   self_cross: RefuseOverlap satisfied).
2. **The disagreement package**: coefficients c+ = delta+ =
   [0,0,299,0,0,32,0,0] — one lot is the per-mille excess of G's belief
   over E's. G posts the portfolio buy, E the portfolio sell, identical
   coefficients and lots (the T2-8 portfolio-full-pair shape). Limits
   conservatively inward: G buys at floor(dot(c+,v_G)/S) = 88, E sells at
   ceil(dot(c+,v_E)/S) = 80. The trade window (80, 88) atoms/lot is open
   because and only because the models disagree: dot(c+,v_G) - dot(c+,v_E)
   = ||delta+||^2 > 0 — the room to trade IS the squared disagreement.
   Lots L = 50 by the published divisibility rule. (A mirrored delta- pack
   is a named optional extension, dropped to keep settlement exact.)
3. **One uncrossed quote** for refusal texture: E posts a second, lower buy
   at knot 120 (limit 98) that ends ineligible — its reservation stands
   ACTIVE post-clear, exhibiting the terminal-release boundary as T2-8's
   fixture does.
4. **Candidate prices**: p_i strictly inside the open window (min, max) of
   the two values at each traded knot — the cleared vector lies between
   the beliefs by construction — subject to: simplex sum exactly S; every
   settled consideration an exact integer of atoms (the entitlement seam
   refuses nonzero rounding pots and virtual legs — entitlement.rs:337-342
   — so the book MUST clear exactly, and the page discloses that this
   constraint shaped the book). Selection inside the band: maximize
   settlement divisibility, then nearest-to-midpoint, then lowest index.

**The pre-registered instance that closes** (verified by direct
computation in the design pass):

```
p = [0, 120, 2840, 5780, 1240, 20, 0, 0]        (sum 10,000; every p_i strictly in-band)
singles (z=500):  120: E buys @127 / G sells @98   -> 6 atoms
                  140: G buys @2961 / E sells @2662 -> 142 atoms
                  160: E buys @5945 / G sells @5696 -> 289 atoms
                  180: E buys @1266 / G sells @1213 -> 62 atoms
                  200: G buys @32  / E sells @0     -> 1 atom
package: dot(c+,p) = 849,800 in (800,000, 880,000); 50 lots -> 4,249 atoms exactly
         legs: 14,950 Eggs @140, 1,600 @200, paired G-buy vs E-sell
```

Budgets (genesis: cash 10,000 atoms and 16,000 Eggs per outcome per model,
injected — stated): G encumbers 4,551 atoms at placement (sup-norm story);
price-improvement refunds 151+7+1 return at settlement. E encumbers 15,450
Eggs @140 and 2,100 @200 plus ~374 atoms, plus 5 atoms standing on the
uncrossed low-ball until terminal release.

## 4. The walk and the licensing theorems

Plane: general clearing end-to-end in the bank — tags 47-59, the exact
T2-8 route through Settle x6 with conservation asserts; 7 pairing slices
(5 single crossings + 2 package legs), all homogeneous. Claim plane:
SBF-EXECUTED (bank), UNPROMOTED, fees zero, policy GENERAL_CLEARING_POLICY_V1
(frozen 2026-08-20).

The read "cleared book as synthesized belief" is licensed in three layers,
each at its true plane:
1. **PoU -> forced normalization** (sum p = S): PROVED-MODEL (Lean,
   DragonsClutch.BSpline); the simplex consequence is Theorem 3.1 of
   docs/research/DUAL_IS_THE_MEASURE.md (paper proof).
2. **Measure-hood** (Theorem 7.1): at degree <= 1 every V1-valid price
   vector is the basis-moment vector of Q* = sum (p_i/S) delta_{t_i};
   density p_i/(S g); implied forward $156.40, between $156.24 and
   $156.70. General theorem: paper proof. The exhibit's INSTANCE is
   elementary (cardinal hats: E_{Q*}[N_j] = p_j/S exactly) and the check
   script verifies it executably. Badge the general claim "paper proof",
   the instance "checked by script".
3. **Certificate** (Theorem 5.1): under the frozen tuple every accepted
   candidate is a zero-duality-gap LP optimum with p as optimal dual.
   Paper proof; the exhibit's gap is recomputable term-by-term (all
   strict orders fully filled, ineligible at zero, no marginals -> gap 0).

**What does not carry over, said plainly on the page**: Q* is A
representing measure, not THE measure; within each per-knot no-trade band
the published point is the submitter's selection, not market revelation
(show the band as a shaded region + state the rule); the Lean CertF
instantiation at this matrix is open, so nothing here is "verified"; bank
evidence, unpromoted; at degree >= 2 the measure reading is refuted
(section 7.4) — the exhibit lives strictly on the proved rungs.

## 5. The reported numbers

| row | value |
|---|---|
| beliefs | v_G, v_E; forwards $156.24 / $156.70 |
| cleared | p = [0,120,2840,5780,1240,20,0,0]; density p_i/(S g); forward $156.40 |
| the band | per-knot (min,max) of the two values; every integral in-band candidate verifies |
| trades | 500 Eggs at each of five knots + the 50-lot package (14,950 @140, 1,600 @200) |
| what each paid | G net -4,035 atoms (paid 4,392, received 357); E net +4,035; fees 0 |
| holdings after | G: +15,450 @140, +2,100 @200, -500 each @120/160/180; E opposite; byte-compared to the verified summary |
| self-assessed edges | by G's light +196.1; by E's light +289.1; both think they won; sum = (trade)(v_G - v_E)/S = 485.2 atoms — **the disagreement traded is the total perceived gain** |
| certified surplus | 433.1 atoms (4,331,000 price units), the LP objective, gap 0 |
| conservation | cash constant, per-outcome Eggs constant, pot [0;16], rounding pot 0, release identity closed |

## 6. Artifacts (build order)

**L1 — docs/site-plan/disagreement_check.py** (friday_clutch_check.py
style): series regeneration from seed -> pinned v_G/v_E reproduction
(float boundary named) -> book-former (windows, divisibility, p) ->
relation arithmetic in exact integers: simplex, per-order eligibility
trichotomy (each strict/ineligible inequality asserted), conservation per
outcome, exact considerations, zero rounding residue, zero gap
term-by-term, measure instance E_{Q*}[N_j] = p_j/S, forwards, both
self-assessed edges and the dot identity, H = L + P + S ledger frames.
Every printed number appears verbatim on the site.

**L2 — programs/clutch-sbf/svm-tests/tests/disagreement_exhibit.rs**:
clone the entitled_clearing.rs fixture with OUTCOMES = 8, degree-1 v3
terms (section 1), a 10-tick grid {0, 32, 98, 127, 1213, 1266, 2662,
2961, 5696, 5945} (<= MAX_GRID_TICKS = 64; single-Egg limits
tick-verified at placement, portfolio limits not), genesis positions
(cash 10,000 / 16,000 Eggs per outcome x 2 owners), the 13-order book,
the pinned candidate, the full walk, and the conservation battery
(byte-compare positions, totals, consumed-reservation archives, the
ACTIVE low-ball, pot empty). **T0 first**: confirm the general plane
admits a degree-1 terms market (expected yes — the relation sees the
basis only through PoU); if any seam refuses, RECORD the refusal and
STOP — do not fall back to degree-0 terms. Also check: zero coefficients
inside a portfolio's active prefix (active_len 6, nonzeros at 2 and 5)
are admitted.

**L3 — site/disagreement.html** ("The Disagreement — two machines, one
price"), nav after For the Machines, shape.html's structure: kicker; the
two-model setup with the honesty box (named estimators, pre-registered
rule, seed rule, jumpy-generator disclosure, book-former disclosure);
the SVG: three overlaid grid-resolution densities (G thin, E thin,
cleared bold), the per-knot disagreement band shaded, three forwards
ticked; the numbers table; lane asides (degen: "both sides think they
won — that's what a priced disagreement is"; scholar: Q*, band-selection
honesty, gap; builder: the walk's tags and the test name); badges:
PROVED-MODEL (PoU/Lean), paper proof (DUAL_IS_THE_MEASURE 7.2/5.1),
SBF-EXECUTED bank UNPROMOTED (walk), "checked by
docs/site-plan/disagreement_check.py"; the falsifier box (run the
script; move any p_i out of band and the named inequality flips; run the
bank test). Plus one evidence.html row/anchor and a machines.html link.

**L4 — the statement clause** (orchestrator's, gated on L2 green; plane
tracks the strongest executed artifact, never the plan).

## 7. Failure modes, named and closed

1. Accuracy theater — no resolution step; no realized outcome shown;
   "neither model is right; the exhibit ends at the settled book."
2. Value theater — synthetic quantity, laboratory bank, fees zero,
   unpromoted, scope-note link.
3. Theorem inflation — "a measure, exhibited," never "the market's true
   belief"; band honesty shown; paper-proof badges never upgraded;
   "verified" never used; degree >= 2 boundary restated.
4. Model theater — estimators named and pinned; seed/horizon by
   pre-registered rule; generator jumpiness disclosed as the designed
   source of disagreement; book-former disclosed as deterministic exhibit
   coordination, not agent discovery.
5. Constraint laundering — the exact-only entitlement seam shaped the
   book; say so.
6. Plane creep — genesis-injected positions and the unpromoted walk are
   labeled; no Endow/Split executes; funding framing in prose and script.
