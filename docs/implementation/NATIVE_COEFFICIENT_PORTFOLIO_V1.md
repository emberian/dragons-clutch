# Native coefficient portfolios: audited V1 seam

Status: **PURE SEMANTIC SEAM; LIVE PROMOTION STOP** (2026-08-19).

This note records the implemented boundary in
`programs/solana-layout/src/portfolio_settlement.rs`. It changes no Intent,
account codec, PDA, dispatch branch, candidate-selection authority, or live
settlement path.

## 1. Two layers that must stay distinct

The Market first defines a native payout basis. Immutable Terms bind the source
and window, statistic, knots, degree `0..=3`, edge and ambiguity policies,
evaluator version, payout denominator, and native Egg order. Resolution produces
one exact nonnegative B-spline weight vector `w` whose active entries sum to the
denominator `D`.

A coefficient portfolio is then exact algebra over those native Eggs:

```text
native payout basis:       E_0 ... E_(n-1), resolved by w / D
coefficient claim:         a_0 E_0 + ... + a_(n-1) E_(n-1)
terminal payout:           dot(a, w) / D
```

This composition does not pick a categorical cell. It keeps the same Market,
Terms digest, B-spline degree, denominator, and outcome order all the way through
identity, reservation, transfer, and redemption of the component Eggs.

Categorical compatibility lowering is different. Sampling an analytic curve
onto a degree-zero terminal partition creates a different basis under different
Terms. It must not reuse the native degree-one, degree-two, or degree-three
claim identity, even if a UI gives both products the same label. The new claim
digest makes that substitution red by binding the complete Terms digest and the
degree again at the composition boundary.

## 2. Which shapes are coefficient programs

The following product shapes need no new settlement primitive once a compiler
has emitted a bounded exact or explicitly certified coefficient vector over the
native Eggs:

- ranges, shoulders, and one-sided tails;
- triangular and trapezoidal payoffs;
- capped calls, capped puts, and call/put spreads;
- exact sampled tables and piecewise polynomial coefficient tables;
- rationally enclosed Gaussian-like curves;
- histograms and distributional belief portfolios;
- LP-style outcome ranges and multi-band exposures; and
- any nonnegative sum of admitted coefficient programs whose products fit the
  fixed integer bounds.

These are legitimate native-basis compositions. A curve compiler may be exact
for one shape and an approximation for another; approximation metadata is not
consensus identity, but it must disclose its basis, error norm, bound, and
sampling domain. None of this permits replacing the Market's resolved B-spline
weights with a terminal-category selector.

Negative signed deltas are valid trade directions only when the exact native
Egg vector is already funded. They are not standalone bearer assets. V1 does
not introduce a portfolio mint, Token-2022 wrapper, NFT, callback, nested claim,
or separate claim supply.

## 3. What was already live

The live order-input half was already present and remains its own semantic
owner:

- `PlaceOrder` accepts `OrderSlot::Portfolio` on the signed wire;
- `PortfolioRecord` stores a fixed-width, canonically padded nonnegative
  coefficient vector and an integer lot count;
- `ReservationPlan::for_order` reserves a buy's exact cash limit plus signed fee
  cap, or a sell's exact products `lots * coefficients[i]`;
- placement moves those exact sell-side Eggs out of the Position and into the
  Reservation ownership phase; and
- cancellation returns only the same active Reservation's unused assets and
  refuses replay.

This work did not duplicate or replace those rules.

## 4. New pure semantics

### Canonical identity

`NativePortfolioClaimV1` binds:

```text
market
terms digest
basis degree
payout denominator
outcome count and order
primitive coefficient vector
```

The active requested vector is divided by its nonzero gcd. The removed scale is
multiplied into filled primitive units. Therefore `(2,4)` for three lots and
`(1,2)` for six lots name the same native claim, transfer the same vector
`(6,12)`, and settle for the same consideration. Padding is fixed at sixteen
entries and cannot influence the digest.

### Funding and worst-state bound

`PortfolioFundingV1` independently recomputes both the existing ReservationPlan
and the canonical vector. For a full sell it requires:

```text
reserved_internal[i] = lots * requested_coefficients[i]
                     = primitive_units * primitive_coefficients[i]
```

Holding the exact component Eggs fully funds the claim; settlement does not
create a new liability. Over all nonnegative simplex weights summing to `D`, the
conservative maximum payout of the transferred vector is exactly its largest
coefficient. The native B-spline image may be a strict subset of the full
simplex, so the implementation names this
`simplex_worst_case_payout_atoms` rather than claiming a tighter reachable-market
maximum.

### Full paired consumption

The isolated preflight admits only one complete direct pair:

- opposite sides and distinct Position owners;
- full Portfolio orders with the same canonical claim and primitive units;
- one exact frozen simplex vector;
- one dot product with no component-wise division;
- both order limits satisfied on the same numerator scale;
- exact divisibility by `price_scale` (no unnamed rounding);
- zero signed fee envelopes until fee carry has an authenticated state owner;
- exact ACTIVE reservations, owner/generation/market/epoch/Terms/grid/policy
  bindings, and open Positions; and
- no overflow in any debit, credit, or Egg-vector addition.

Every post-state is computed and validated before the first caller-visible
mutation. On success, the buyer receives the whole native Egg vector and pays
exact collateral, the seller receives the same collateral, both reservations
become empty `CONSUMED`, and the proposed entitlement becomes `CONSUMED`. A
second call refuses before mutation.

This is representation-invariant: proportional coefficient/lot encodings
canonicalize to the same claim and primitive fill before value or transfer is
computed.

### Fee/carry arithmetic

The module includes the exact experimental simplex-dispersion arithmetic from
`docs/FEE_GEOMETRY.md` as an offline comparison seam:

```text
G_num(a,p) = sum_(i<j) p_i p_j |a_i-a_j|
fee_num     = kappa_num * G_num + prior_carry
fee         = floor(fee_num / (kappa_den * S^2))
next_carry  = fee_num mod (kappa_den * S^2)
```

It consumes the actual transferred payoff vector, not its display
decomposition. Tests pin invariance under a complete-set shift, proportional
fragmentation with persistent carry, and refinement of one price cell into two
identical-payoff subcells. This remains an experimental policy, not a live fee.
The current pure paired settlement accepts zero fees only because Position and
Reservation accounts do not own a persistent per-Position/per-policy carry.

## 5. Why the seam is not live authority

`PortfolioEntitlementV1` is content, not an account. Its digest binds a proposed
Market, Epoch, candidate, Terms, price grid, policy, canonical claim, both order
ids, the full simplex vector, primitive units, and exact consideration. No
current transition is authorized to create it.

Digest consistency cannot answer who selected the candidate, whether every
relation order was reserved, whether this is the only receipt for these fills,
or whether another transaction already owns the same assets. Consequently the
new module is not imported by `clutch-sbf`, and no live instruction can reach
its apply helper.

Promotion requires, in dependency order:

1. live candidate verification and best-valid-submitted selection;
2. a complete frozen-order-to-reservation-set commitment;
3. a stable vector receipt codec binding the canonical claim, primitive units,
   prices, and exact value;
4. program-only receipt initialization during candidate finalization;
5. exact frozen-page provenance for both Portfolio records;
6. an immutable, decoded policy preimage owning fee and rounding choices;
7. a persistent per-Position/per-policy fee-carry account before nonzero fees;
   and
8. terminal closure proving every Reservation and receipt is consumed or
   refunded exactly once.

The existing single-Egg `SettlementReceiptAccount` names one outcome and one
quantity. Reusing it for a vector would lose the atomic claim identity and is
forbidden. `SubmitDirectPage` likewise constructs only the narrow two-order
single-Egg proposal and cannot be treated as portfolio selection authority.

## 6. Evidence

`cargo test --offline` in `programs/solana-layout` passes 147 unit tests and two
doctests. The new hostile suite covers:

- proportional encodings and degree/Terms identity separation;
- native identity admission for every basis degree `0..=3`;
- exact reservation vectors and the conservative simplex maximum;
- fee complete-set, fragmentation/carry, and partition-refinement invariance;
- exact full-pair cash and native Egg movement;
- entitlement and both Reservation one-time consumption;
- replay refusal;
- identity binding of every economic coordinate; and
- six validate-before-mutate hostile cases, including claim substitution,
  price mutation, coefficient mutation, underfunding, and unavailable fee
  authority.

This is executable model evidence, not a deployed runtime result or a formal
proof.
