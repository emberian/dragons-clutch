# Clutch-aware simplex auction

## 1. Why a special venue exists

A generic spot orderbook can trade each materialized Egg independently. Manifest
already provides an efficient, permissionless Token-2022-compatible implementation
and should be an early venue adapter.

The native Dragon venue is justified only by coupling the exhaustive outcomes:

- one coherent price vector rather than unrelated books;
- automatic complete-set creation and destruction;
- atomic payoff-vector intents;
- internal balances without Token CPIs per fill;
- shared collateral and exact fee/rebate semantics.

It is a frequent batch auction over a probability simplex, not a faster generic
CLOB.

## 2. Price certificate

For `n` outcomes and integer `PRICE_SCALE`, a candidate carries:

```text
p_i in [0, PRICE_SCALE]
sum_i p_i = PRICE_SCALE
```

Every ordinary order is evaluated at exactly this vector. The simplex equation
means one complete Clutch clears at one collateral unit before explicit venue fees.

The vector is a candidate until every frozen page has been verified. No solver,
client, or offchain service has authority to set it unilaterally.

## 3. Order families

### Single-Egg limit order

```text
outcome
side
quantity
limit_price
minimum_fill / all_or_partial
owner Position
expiry Epoch
```

At candidate `p`, eligibility and price improvement are locally checkable.

### Proportional portfolio intent

```text
basket: [u64; MAX_OUTCOMES]
side: buy or sell
lots
maximum buy cost or minimum sell proceeds per lot
minimum fill / all_or-partial
```

One lot transfers the exact basket proportions. Its candidate value is one checked
dot product with a single final rounding. Settlement is atomic across every leg.

### Complete-set conversion

Split and merge are virtual venue operations at unit collateral value. They need no
standing order and provide the coupling that keeps the price vector coherent.

## 4. Tractability boundary

Unrestricted all-or-none combinatorial basket winner determination can become an
integer optimization problem and is not assumed cheap or uniquely solvable. V1
does not hide that behind “the solver.”

The tractable core is:

- divisible single-Egg orders;
- proportional basket lots with bounded outcome count and basket-order count;
- a public, exactly checkable candidate score;
- permissionless competition among valid candidates;
- no claim that the selected candidate is a mathematically global optimum unless
  a closed optimality certificate is later implemented.

Potential later work includes a totally-unimodular restricted order language,
linear-program primal/dual certificates, or succinct proofs. These are research
tracks, not assumptions in V1.

## 5. Candidate construction

Anyone may compute a candidate offchain, including the static browser for small
books. A candidate contains:

- simplex price vector;
- fill quantity for each included order/page aggregate;
- complete-set split and merge quantities;
- per-page asset deltas and settlement pots;
- maker/taker/clearing fee allocation;
- exact public score and canonical digest;
- solver bond and reward destination.

Heavy search may be offchain; correctness and asset conservation are onchain.
There is no required solver service.

## 6. Verification

A paginated verifier scans every frozen order exactly once and checks:

1. order identity, reservation, Epoch, status, and canonical bytes;
2. fill bounds, minimum-fill/all-or-partial rule, and expiry;
3. single-Egg limit or exact basket dot-product limit;
4. per-outcome debits and credits;
5. complete-set split/merge conservation;
6. collateral conservation;
7. fee cap, payer, and allocation;
8. page-local settlement-pot totals;
9. candidate score recomputation;
10. no order appears twice or is omitted from the committed page closure.

For every outcome `i`:

```text
opening reserved Eggs_i + split_quantity
  = closing/unfilled Eggs_i + filled credits_i + merge_quantity
```

Collateral conservation includes reserved cash, trade transfers, complete-set
split deposits, merge withdrawals, and fees. Hoard principal participates only in
the exact split/merge transition, never as trading liquidity.

The leading portfolio fee base is the exact state-contingent dispersion in
[FEE_GEOMETRY.md](FEE_GEOMETRY.md). Candidate verification recomputes it from the
same canonical payoff vector and simplex prices used for limits; a solver cannot
declare a cheaper economic shape than it settles.

## 7. Candidate selection

During a bounded proposal window, any valid candidate may replace the current best
if its score is lexicographically superior. A candidate never blocks later
candidates merely by arriving first.

Initial score proposal:

1. maximize executed risk mass under a frozen definition;
2. maximize limit-price surplus;
3. maximize number of participating distinct orders or minimize concentration;
4. minimize complete-set churn and compute/storage burden;
5. canonical digest as final deterministic tie-break.

Every component is an exact integer recomputed onchain. The final choice is the
best valid submitted candidate, not an unproved global optimum. The UI says this
plainly.

An invalid candidate loses a bounded bond that compensates verification work. A
valid losing candidate recovers its bond. Candidate-copying affects solver rent,
not correctness; reward rules should avoid paying merely for revealing somebody
else's public solution first.

## 8. Allocation and settlement

After the proposal window:

1. finalize the best candidate identity;
2. complete page verification and allocation;
3. keep outputs inactive until every page closes;
4. atomically flip Epoch to Final;
5. let users settle page-local results lazily and idempotently into Positions.

Before Final, balances belong to order reservations. After Final, exact page pots
own the settlement obligations. The proof must show no state interprets the same
asset under both ownership phases.

## 9. Complete-set price coherence

The simplex constraint eliminates an internal clearing vector with sum different
from one, but fees create visible execution bands. The venue should report:

- raw simplex prices;
- all-in buy/sell prices after fee;
- complete-set mint/merge quantities;
- external materialized-market deviations;
- whether an arbitrage is executable after all venue and network costs.

The fee is not hidden in the probability vector.

## 10. Manifest and other venue adapters

The first external adapter should materialize selected Eggs into a generic
Token-2022 venue such as Manifest. The adapter must expose:

- exact Market/outcome identity;
- materialization/dematerialization path;
- external fee, lock, route, and liquidity status;
- no claim that external prices satisfy the simplex;
- optional complete-set arbitrage transaction planning without signing authority.

Building a generic orderbook is explicitly outside the native venue's charter.

## 11. Research questions

- Can the single-Egg clearing search be reduced to a separable convex allocation
  with an exact small dual certificate?
- Which restricted basket language yields total unimodularity and integral fills?
- Can one-pass aggregate curves verify maximal eligible fill at a candidate vector?
- What score most resists wash volume and fragmentation while remaining exact?
- How should price-time priority interact with frequent batches and proportional
  allocation?
- Can a future proof compress O(m) page verification without introducing a
  privileged prover?
