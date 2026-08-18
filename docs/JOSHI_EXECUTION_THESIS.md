# JOSHI execution thesis

## 1. Why JOSHI would care

JOSHI should not trade Dragon's Clutch merely because both projects exist. The
venue is interesting to JOSHI if it exposes an executable state basis matching
what JOSHI already tries to perceive: timing, order-flow size and identity,
attention propagation, liquidity topology, lifecycle, PvP compression, reserve
geometry, and their path-dependent response.

Spot turns that field into one scalar position. A Clutch preserves more of it. For
one Template and horizon, JOSHI can emit a witnessed belief vector:

```text
r = [r_0, ..., r_(n-1)]
r_i >= 0
sum_i r_i = BELIEF_SCALE
```

The venue emits a separately witnessed clearing vector `p`. Neither vector is
truth: `r` is a model artifact with lineage; `p` is a market-clearing artifact
with order-set lineage. Their disagreement creates hypotheses.

## 2. Four native uses

### Distributional alpha

For exact payoff vector `a`, JOSHI's expected terminal value is:

```text
model_value(a) = dot(r,a) / BELIEF_SCALE
```

A buy is interesting only when model value exceeds executable cost, venue fee,
materialization/routing cost, expected settlement friction, uncertainty reserve,
and a capital-time hurdle. The model must calculate the whole branch, not compare
two displayed probabilities.

The opportunity may be shape-specific even when expected terminal price agrees:
JOSHI can disagree about tails, dispersion, drawdown, or relative performance
without having a simple directional view.

### State-contingent hedging

Map an existing spot, crackle, LP, or treasury position into a conservative wealth
vector `w_i` over the same partition. Choose an Egg portfolio `x` that improves a
frozen objective such as worst-state wealth, drawdown, expected utility, or CVaR:

```text
maximize_x  Objective_r(w + payoff(x) - all_in_cost(x))
subject to  collateral, liquidity, concentration, and authority limits
```

This is genuine risk transfer only when another participant bears the opposite
state exposure. Self-minting and self-crossing do not hedge anything.

### Coherent market making

JOSHI can quote an entire distribution rather than unrelated Egg prices. Its
inventory is a state-payoff vector, so quote shading should respond to marginal
wealth in each state, not merely token count. A useful diagnostic covariance at
belief `r` is:

```text
Sigma(r) = diag(r) - r*r^T
```

This captures the negative coupling among exhaustive outcomes. It is a modeling
input, not an onchain truth or a promise that quadratic risk is sufficient.
Complete-set split/merge gives the desk a risk-free inventory direction; only the
orthogonal state-contingent component deserves a risk charge.

### Market-as-sensor

The verified simplex and its changes become another JOSHI evidence stream:

- belief/market residual by state and horizon;
- order arrival, cancellation, fill, and solver-candidate response kernels;
- depth and elasticity along payoff directions;
- divergence between native coherent prices and external per-Egg venues;
- implied distribution changes around social/liquidity/topology events.

This stream is reflexive. JOSHI's own orders must be tagged and removable from any
analysis purporting to measure independent market information.

## 3. The strongest product wedge

The best first family for JOSHI is not arbitrary news. It is a repeated objective
token-native surface whose risks JOSHI already models:

- terminal token/quote price bins at several horizons;
- maximum-drawdown or sampled-extrema regimes;
- relative performance against SOL or a peer basket;
- pool migration/liquidity-state transitions;
- bounded combinations only after source and accumulator closure.

One eight-state terminal Template supports tail insurance, ranges, digitals, and a
full distribution. Repeated Instances create chronological calibration data and
make passive liquidity more plausible than one-off bespoke questions.

## 4. Preconditions for an executable JOSHI edge

JOSHI refuses a trade unless all are true:

1. Template, Instance, Realm, compiler, source, and Hatch semantics are known.
2. Belief artifact uses only evidence available at its decision cutoff.
3. Resolution evidence is independent of JOSHI's ability to move the observed
   source, or manipulation exposure is explicitly bounded and acceptable.
4. Executable depth covers the proposed portfolio at its atomic limit.
5. All-in cost includes fee, network, priority, rent opportunity, spread,
   slippage, materialization, settlement delay, and failure-state haircut.
6. Model uncertainty and calibration error leave positive residual edge.
7. Position and common-source exposure fit the whole desk, not just the Market.
8. Counterparty, self-trade, wash, and operator-conflict policies permit it.
9. Legal authority for the person, jurisdiction, instrument, venue, and role is
   separately established.
10. The actual execution mode is authorized: shadow, recommendation, user-signed,
    or a later separately approved automated policy.

## 5. Development ladder

### J0: compiler alignment

Export Clutch partitions and payoff vectors into JOSHI's exact artifact system.
Compute belief vectors and existing-book state wealth with no venue or transaction
authority.

### J1: historical/shadow valuation

Evaluate hypothetical payoff portfolios using out-of-sample chronological cuts.
Track calibration, abstention, failure states, capital-time, and complete
liquidation. No synthetic fill is called executable.

### J2: live read-only market sensor

Ingest verified Clutch state, candidates, final auctions, and external Egg quotes.
Produce no orders. Separate JOSHI-derived and independent flow.

### J3: user-signed bounded execution

JOSHI proposes exact portfolio intents with why, cutoff, alternatives, worst case,
all-in cost, and expiry. Ember signs each transaction. Hard refuse self-cross,
source influence, unknown deployment, or insufficient counterparty depth.

### J4: liquidity policy research

Shadow an inventory-aware full-simplex quoting policy. Promote only after
prospective calibration, adverse-selection, markout, inventory, settlement, and
legal/conflict gates. No automatic authority is implied.

## 6. Operator/deployer conflict

If Ember deploys or economically benefits from a Clutch instance while JOSHI
trades there, the system has several roles at once:

- protocol author/deployer;
- fee beneficiary or DREGG holder;
- principal trader or market maker;
- solver/keeper participant;
- possible participant in the underlying token market;
- analyst publishing a client or explanatory material.

Immutability removes some discretion but not these conflicts. Any serious path
requires public role/address disclosure, identical public instructions, no private
ordering privilege, self-trade prevention, source-manipulation exclusions,
surveillance, and a specialist legal analysis. JOSHI must not treat our own venue
fees, burns, or price impact as trading alpha.

## 7. Kill criteria

The JOSHI integration is not interesting if:

- viable Templates do not overlap risks we actually carry or model;
- no independent counterparty depth forms;
- all-in cost overwhelms measured distributional disagreement;
- oracle/failure manipulation dominates the payoff;
- the native auction gives no coherence/cost advantage over ordinary Eggs;
- beliefs fail prospective calibration or merely chase venue prices;
- legal access cannot be established; or
- operating/deployer conflicts cannot be separated honestly.

Even then, the partition algebra, verified kernel, static client, and conformance
suite can remain valuable public infrastructure.
