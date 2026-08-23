# Bounded liquidity facility V2

Status: **SELECTED RESEARCH MECHANISM / EXACT EXECUTABLE MODEL / NO LIVE
AUTHORITY** (2026-08-23)

Executable model:
[`research/bounded-liquidity-facility`](../../research/bounded-liquidity-facility)

This document selects a sophisticated liquidity architecture without pretending
that modeled quotes are deployed liquidity. It changes no runtime account,
instruction, batch relation, mint authority, or release status.

## Decision

Dragon's Clutch should support two complementary, separately versioned
liquidity policy families:

1. the existing fully capitalized, finite schedule compiler for sponsors who
   want exact sizes, shapes, floors, ceilings, and expiry; and
2. a bounded quadratic issuance-and-repurchase facility for sponsors who want
   deterministic depth, path-independent native coefficient pricing, and an
   exact ex-ante loss cap.

Both submit through the call auction. Neither is an oracle, mint authority,
Hoard owner, privileged solver, or promise of continuous inclusion. The cost
facility's state transition is nonlinear and must be verified once over its
aggregate per-batch native flow; it must not be approximated by an arbitrary
ladder of client orders.

The facility selected here is fully collateralized from present sponsor
capital. It deliberately has no margin, loss mutualization, dynamic depth,
future-fee asset, treasury bailout, or right to Hoard principal. Its quote
curve is oracle-free. Its terminal payout is not: resolution still consumes the
Market's authenticated payout vector.

The exact model generalizes the usual uniform quadratic scoring rule with an
immutable rational initial simplex price `pi`. It therefore does not force
every product, Realm, or Series to begin at `1/n`. A sponsor may commit a prior
derived offchain from a named model or market thesis; consensus authenticates
only the committed integers, not the truth of that model.

## What this facility actually provides

The V2 facility is a bounded primary issuance and repurchase underwriter over
native Eggs:

- from zero externalized inventory it can sell any native Egg or exact native
  coefficient basket, creating complete sets as needed;
- after an Egg has been externalized through this facility, it can buy that Egg
  back, reducing its attributed inventory;
- coupled call-auction flow can sell some outcomes and buy back others in one
  atomic endpoint transition; and
- every payoff in the same native claim domain can reach the same inventory
  curve after canonical wrapper-to-Egg decomposition.

This issuance policy is not a universal two-sided dealer from an empty state.
It does not buy a long Egg position that it did not previously underwrite. That
is a real restriction, not marketing fine print. The separate exact
[`COVERED_SIGNED_DEALER_V1`](COVERED_SIGNED_DEALER_V1.md) research model now
supports genuine bids only when LPs explicitly contribute cash and existing
long Eggs and a sponsor separately capitalizes curve loss. No live adapter may
infer signed authority from this issuance state.

This underwriter role is still economically meaningful. It supplies initial
state-contingent claims without guessing which complete-set complements users
will want, gives issued claims a deterministic repurchase curve, shares depth
across every compatible shaped payoff, and caps the sponsor's information
subsidy exactly. User-to-user bids and independent inventory holders remain
necessary for a genuinely two-sided secondary market.

## Exact curve and integer algorithm

Let:

```text
n       active native Eggs, 2 <= n <= 16
pi_i    immutable initial price, pi_i >= 0, sum_i pi_i = 1
b       immutable depth in raw Egg atoms
q_i     nonnegative facility-attributed Eggs held outside the facility
Q       sum_i q_i
```

The rational convex potential is

```text
C(q) = dot(pi,q) + (n*sum_i(q_i^2) - Q^2)/(2*b*n).
```

The exact rational marginal price is

```text
p_i(q) = pi_i + (n*q_i-Q)/(b*n).
```

The policy represents `pi_i=a_i/A` with integer weights summing exactly to
`A`. Price numerators share denominator `A*b*n`:

```text
price_num_i = a_i*b*n + A*(n*q_i-Q).
```

Every numerator must remain nonnegative. They sum exactly to the denominator,
so admitted prices lie on the simplex without normalization or floating point.
For a uniform prior the domain is simply
`Q-n*min(q)<=b`. A skewed prior shifts capacity toward outcomes the sponsor is
more willing to underwrite.

Consensus takes one ceiling of the complete rational potential:

```text
C_hat(q) = ceil(
  [2*b*n*sum_i(a_i*q_i) + A*(n*sum_i(q_i^2)-Q^2)]
  / [2*b*n*A]
).
```

One batch transition from `q` to `q'` exchanges exactly
`C_hat(q')-C_hat(q)` collateral atoms. Endpoint differencing gives:

- exact path telescoping;
- exact round trips before separately assessed batch fees;
- no per-leg rounding accumulation;
- no solver-selected ordering advantage; and
- exact complete-set translation
  `C_hat(q+k*1)=C_hat(q)+k` for integer `k`.

The denominator for `pi` is capped at `10^9`; atom values and depth are capped
at `10^12`. Those are proved arithmetic bounds for this model, not universal
protocol limits.

## Solvency and physical backing

For the full integer payout simplex, external inventory has conservative
liability

```text
H(q) = max_i q_i.
```

The worst rational sponsor loss is the convex conjugate at terminal vertices:

```text
L_max = b/2 * max_j ||e_j-pi||^2.
```

For uniform `pi`, this reduces to `b*(n-1)/(2n)`. The policy requires present
sponsor capital

```text
K >= ceil(L_max).
```

Rounding `C` upward cannot increase sponsor loss. Every graded simplex payout
is a convex combination of terminal vertices, so the vertex bound covers all
admitted payout vectors. This is an actual worst-case bound, not a VaR estimate
or belief-dependent promise.

The model also realizes backing as assets. At every live endpoint:

```text
H                 collateral atoms in facility-attributed Hoard complete sets
r_i = H-q_i       facility-held complement Eggs
F = K+C_hat(q)-H  free facility cash outside Hoard
```

When `H` rises, collateral moves from `F` to Hoard and one of every Egg is
split. When `H` falls, one complete set is merged and collateral returns to
`F`. The exact receipt verifies componentwise Egg conservation. A live adapter
must refine that recipe into actual Token-2022 and pooled-ledger transitions;
the model itself cannot mint anything.

### Terminal proof

Let exact integer payout weights `w_i` sum to denominator `D`. External holders
receive

```text
E = sum_i(q_i*w_i)/D,
```

while the facility's retained complement receives `H-E`. The remaining `E`
stays in Hoard as backing for externally held claims. Facility terminal cash is

```text
F + H-E = K+C_hat(q)-E >= 0.
```

V2 conservatively requires each facility Egg coordinate and trade flow to be a
multiple of `D`, making every redemption exact. A future smaller-lot profile
should reuse one protocol-owned fractional-credit/carry ledger and prove its
terminal conservation; silently flooring is not an acceptable relaxation.

## LP loss surface and incentive truth

At payout distribution `w/D`, sponsor profit relative to deposited capital is

```text
P_and_L(q,w) = C_hat(q) - dot(q,w)/D.
```

For a uniform prior and a one-outcome inventory `q=t*e_j`, ignoring the less
than one-atom ceiling benefit:

```text
loss_if_j_wins(t)
  = (n-1)/n * (t - t^2/(2b)),  0 <= t <= b.
```

The marginal price of `j` rises from `1/n` to `1`; every other price falls to
zero; and maximum loss occurs exactly at the capacity boundary `t=b`. More
generally, a skewed prior makes states farthest from that prior more expensive
to capitalize.

This is bounded loss, not positive expected return. An informed trader can
move inventory toward the realized payout and extract some or all of the
sponsor's committed bound. The sponsor's credible motivations are therefore:

- paying a known maximum to obtain price discovery or product usability;
- dogfooding or bootstrapping a recurring Series;
- having a differentiated belief and accepting the scoring-rule exposure; or
- earning already-realized spread/fee revenue under a separate frozen policy.

Expected future fees are never subtracted from `K`, and they never fund close,
resolution, or redemption. The base curve has no hidden spread. If a future
policy adds a spread, it should preserve one endpoint potential per side or a
separately proved path-independent toll, disclose its round-trip surface, and
book only fees already collected. Calling sponsor capital “LP principal” does
not make this a yield product.

## Mechanisms evaluated

| Mechanism | Strong property | Structural problem | Decision |
| --- | --- | --- | --- |
| Existing finite schedule compiler | exact sponsor limits, native shaped rungs, ordinary Portfolio orders | only eight persistent rungs in its model; keeper/page availability; no endogenous curve | retain as a separate policy family |
| Generalized quadratic cost function | exact rational arithmetic, exact simplex prices, path independence, closed-form loss cap | finite nonnegative-price domain; nonlinear auction leg; sponsor funds information subsidy | select as V2 research facility |
| LMSR/log-sum-exp | strictly interior prices and global `b*log(n)` loss bound | exponentials/logarithms, approximation envelopes, consensus rounding and path independence are not yet proved | reject for live promotion; keep as research comparator |
| Constant-product reserve pool | familiar secondary-market interface | fragments liquidity by pair/wrapper, awkward simplex normalization, requires independent reserve/redemption semantics | do not make canonical |
| Oracle-targeted inventory manager | can track an external reference | imports latency/manipulation and turns a source into a trading authority | reject as core mechanism |
| Independent rebalancing auction | competitive price discovery for inventory reduction | another lifecycle, incentive pot, and inclusion dependency; duplicates the call auction | use the existing call auction instead |
| Virtual or protocol-owned “depth” | attractive UI number | no deliverable assets or present loss capital | forbid |

Quadratic selection is not a claim of universal optimality. It is the most
expressive mechanism in the current candidate set whose price normalization,
rounding, physical flow, and worst-case loss can all be implemented exactly
with bounded integers.

## Call-auction composition and solver incentives

The facility should be one optional nonlinear leg in the same coupled candidate
relation, not a second sequencing venue. For each candidate and each facility:

1. aggregate all proposed facility fills into one native sell vector and one
   native buyback vector;
2. reject both directions on the same native Egg as noncanonical after netting;
3. recompute the unique endpoint, price-domain and inventory-cap checks;
4. recompute the exact endpoint cash difference and Split/Merge recipe;
5. include those amounts in total Egg, collateral, fee, and reservation
   conservation; and
6. consume the exact facility generation atomically with every user leg.

The solver does not choose the facility price, update order, or rounding. It
does choose which compatible user orders and facility quantity to include.
Candidate scoring can reward surplus or volume under the existing frozen
relation, but the documentation must still say “best valid submitted candidate”
unless a checked optimality certificate exists.

Solver liveness remains a separately prepaid public-good problem. A facility
may create more matchable candidates and hence more realized fee opportunity,
but expected fees cannot substitute for the present reward needed to guarantee
submission. A Series that promises progression must reserve solver/close work
outside both `K` and Hoard principal.

### Rebalancing without a second oracle

Inventory changes only when authenticated orders clear. During ordinary
trading, mixed flows can rotate exposure: buy back one previously externalized
Egg while selling another. At the frozen close or an earlier sponsor halt, the
facility enters `BuybackOnly`; candidates may then reduce every `q_i` but never
increase one. The same endpoint curve specifies the repurchase cash. No keeper
receives authority to guess a “correct” outcome distribution or trade against a
private oracle.

Competing facilities should remain segregated. The auction can route against
several independently capitalized curves, but it may not net their loss bounds,
cash, retained Eggs, or generations. Competition, not mutable governance,
selects which priors and depths users consume.

## Wrapper and Series liquidity concentration

The facility trades the native Egg basis, not branded payoff wrappers. An
authenticated wrapper or exact coefficient order lowers to one vector over the
same immutable Terms and claim-domain digest. The call auction can combine that
vector with user and facility native flows. Consequently:

- a range, tent, capped-linear, or other exact native payoff can share the same
  facility depth;
- two wrapper labels with identical canonical native coefficients cannot create
  separate liquidity silos or obtain different endpoint prices;
- wrapper mint/burn is unnecessary for the facility unless the product promises
  a transferable atomic wrapper; and
- an approximation, different Terms, different maturity, or different claim
  domain cannot borrow this liquidity merely because a UI calls it similar.

This is a major advantage over pairwise pools: one native inventory surface can
serve many shaped intents. It does not make every coefficient portfolio equally
deep. A basket that drives an already-low price negative hits the facility's
capacity boundary, and a large common complete-set component still consumes
real Hoard throughput even though it does not change relative prices.

For recurring Series, each Instance receives its own immutable facility policy
binding exact Market/Terms/Instance identity. A Series blueprint may reuse the
same prior-generation algorithm and depth rule, but not the same live state.
The current product-compiler design correctly prepays every instance's
liquidity allocation. Capital reuse across nonoverlapping Instances is an
optional sponsor strategy, not unconditional Series liveness: later activation
cannot assume an earlier market will resolve and return capital unless that
entire progression is itself funded and enforced.

## Oracle-free quotes versus observable outcomes

`C_hat(q)` depends only on immutable policy and authenticated facility
inventory. No source value, midpoint, price feed, confidence band, or proposed
terminal category enters quoting. This prevents a source adapter from silently
becoming a trading oracle and makes quote replay deterministic.

It does not remove adverse selection. Traders may observe the underlying event
offchain before the protocol can seal or resolve it. A “sealed outcome” is an
immutable authenticated result, not secret information. Therefore each policy
must freeze an ordinary trading close early enough for its source/window
semantics. The adapter must prove the close relationship against immutable
Terms; a sponsor-chosen slot detached from the observation window is not enough.

Two legitimate product choices remain:

- close before outcome information becomes cheaply observable, limiting
  informed extraction but shortening exit liquidity; or
- intentionally remain open and treat the exact worst-case capital as a public
  information subsidy.

The UI should disclose which choice the policy makes. Facility prices must not
be fed back into resolution, source repair, collateral valuation, or claims of
event probability without an explicit reconstruction and manipulation model.

## Shutdown, redemption, and liveness

The exact lifecycle is:

```text
Trading --sponsor halt or permissionless timed close--> BuybackOnly
BuybackOnly --authenticated maturity payout-----------> Resolved
Trading --authenticated maturity payout---------------> Resolved
Resolved --sponsor terminal withdrawal----------------> Retired
BuybackOnly --fully flat, sponsor withdrawal----------> Retired
```

Safety does not require a successful buyback. At maturity, retained complement
Eggs redeem for `H-E`; exactly `E` remains as Hoard backing while external
holders independently redeem; sponsor cash remains nonnegative. Users are never
forced to sell to retire the facility. An unresolved nonflat state cannot
return sponsor capital early.

The conditional liveness theorem is deliberately narrow and executable: at
the close slot, one permissionless transition reaches `BuybackOnly`; at
maturity, one caller with the authenticated payout can take either live phase
directly to `Resolved`, using only already-backed retained Eggs. Neither step
needs sponsor authority, a buyback, future trade, or fee revenue. This proves a
finite progress path once a valid transaction is included; it does not prove
that the source publishes or a transaction lands.

Sponsor absence cannot be allowed to block timed close or claim resolution.
Sponsor authentication is needed only for discretionary early halt and sponsor
withdrawal. A live deployment still needs named, present balances for:

- timed close transaction submission;
- final auction/page closure;
- source/archive and resolution work;
- retained-Egg redemption and token-account closure; and
- rent reclamation or neutral-sink handling if the sponsor never withdraws.

Those balances are not `K`, are not Hoard principal, and are not projected fee
revenue. The pure model proves that progress is financially possible after an
authenticated payout; it does not prove a transaction will be included.

## Restrictions to lift next—and restrictions to keep

High-value successor work:

1. Promote the now-modeled covered signed dealer only after its existing-Egg
   custody, share roster, expense compartment, and aggregate candidate adapter
   gates close; do not add negative balances to this issuance facility.
2. Integrate the protocol's fractional-credit owner so lot size can be smaller
   than the payout denominator without order-dependent floors.
3. Define a proved realized-spread policy and compare sponsor expected utility,
   adverse-selection loss, and user execution against schedule rungs.
4. Add a bounded multi-facility candidate algorithm and prove permutation,
   aggregation, and cash-allocation invariance.
5. Model capacity choice from desired maximum loss: solve the exact inequality
   for `b` under a nonuniform prior and Series budget, then round conservatively.
6. Benchmark policy/state width, retained token-account topology, split/merge
   CU, and portfolio candidate growth before choosing live bounds.

Restrictions that should remain until a successor proof replaces them:

- immutable `pi`, `b`, capacity, claim domain, and trading schedule per policy;
- segregated sponsor solvency per facility;
- exact native coefficients and one rounded endpoint potential;
- no Hoard, fee-pot, rent, or liveness aliasing;
- authenticated resolution as the sole terminal payout owner; and
- refusal at the mathematical price boundary.

## Evidence and uncertainties

Established by the executable model and checked arguments:

- exact rational simplex prices over the admitted domain;
- exact generalized quadratic loss capitalization;
- integer path independence and complete-set translation;
- physical cash/Hoard/retained-Egg conservation;
- exact resolution for the full-denominator lot profile;
- rollback on all tested malformed transitions; and
- bounded fixed-capacity arithmetic at the selected maxima.

Not established:

- a live account codec or SBF instruction;
- compatibility with every current batch/Portfolio runtime branch;
- transaction inclusion or an economically sufficient solver reward;
- profitability to passive sponsors;
- live signed-dealer custody and candidate integration;
- smaller terminal lots;
- source-window-specific safe close times;
- rent/CU feasibility of retained token custody; or
- formal verification of the Rust or its runtime refinement.

Promotion is therefore gated by
[`MODEL_BOUNDARY.md`](../../research/bounded-liquidity-facility/MODEL_BOUNDARY.md).
Until those gates close, the correct claim is: an exact, executable, fully
capitalized liquidity-facility model exists—not that Dragon's Clutch currently
has live automated liquidity.
