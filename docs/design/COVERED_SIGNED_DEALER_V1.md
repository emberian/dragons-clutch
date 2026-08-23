# Covered signed-inventory dealer V1

Status: **SELECTED RESEARCH EXTENSION / EXACT EXECUTABLE MODEL / NO LIVE
AUTHORITY** (2026-08-23)

Executable module:
[`research/bounded-liquidity-facility/src/signed_dealer.rs`](../../research/bounded-liquidity-facility/src/signed_dealer.rs)

This design extends the bounded quadratic facility into a genuinely two-sided
dealer without introducing leverage. LPs contribute existing, already-backed
native Eggs and cash before activation. The dealer may sell those custodied
Eggs or buy more with present cash. It never invents an Egg, borrows collateral,
uses future fees, or reaches into Hoard principal.

The Solana-free RelationV2 batch model now implements the canonical dealer join
and per-order cash allocation. No current SBF instruction, account codec,
custody route, candidate lifecycle, or release manifest implements the dealer.

## Decision and economic ownership

V1 selects a **covered pooled dealer with an irrevocable activation subsidy**:

- LP capital is a fixed exact unit basket of cash plus every native Egg amount;
- LP shares own the contributed basket and every terminal pool asset;
- a sponsor deposits separate cash `K` before funding;
- `K` remains sponsor-refundable if funding is cancelled before activation;
- activation irrevocably donates `K` to the LP pool; and
- the sponsor has no terminal residual or claim on LP capital.

This waterfall is intentionally unambiguous. At no-trade resolution, LPs receive
their basket payoff plus `K`. The subsidy is therefore real prepaid LP
compensation, not projected trading yield. It buys initial two-sided depth and
absorbs the curve's worst-case adverse-selection loss.

A refundable sponsor first-loss tranche would be a different instrument. It
would return the nonnegative residual to the sponsor and leave LPs with their
basket floor but no curve yield. V1 does not mix those semantics. A successor
may add an explicit waterfall enum only with separate share and terminal
allocation proofs.

## State and physical interpretation

Let:

```text
S       total immutable LP shares after activation
c_u     cash contributed per share
g_ui    existing Egg i atoms contributed per share
c0      S*c_u
g0_i    S*g_ui
K       irrevocable sponsor subsidy after activation
q_i     signed cumulative net Eggs sold by the dealer
```

The actual pool assets are

```text
cash(q) = c0 + K + C_hat(q)
Egg_i(q) = g0_i - q_i.
```

`q_i > 0` means the dealer sold some contributed Egg `i`; `q_i < 0`
means it bought additional existing Egg `i`. For every aggregate trade:

```text
old_Egg_i + bought_i = new_Egg_i + sold_i.
```

There is no Split/Merge leg. The contributed and purchased Eggs already exist
and remain backed by the global Market Hoard under the ordinary supply theorem.
LP cash and `K` are trading assets outside Hoard; neither becomes claim backing.

This is what makes the facility genuinely two-sided from its first admitted
auction. It can bid with contributed cash and offer contributed Eggs. Every
quoted atom is deliverable from authenticated custody.

## Signed exact quadratic curve

The immutable policy binds an exact rational prior `pi`, depth `b`, and signed
box

```text
-B_i <= q_i <= U_i,
```

where `B_i` is maximum net buying and `U_i` maximum net selling. The rational
potential and gradient are unchanged:

```text
C(q) = dot(pi,q) + (n*sum_i(q_i^2)-Q^2)/(2*b*n)
p_i(q) = pi_i + (n*q_i-Q)/(b*n)
Q = sum_i q_i.
```

Signed integer consensus uses the mathematical ceiling, including negative
values:

```text
C_hat(q) = ceil(C(q)).
```

Rust truncation toward zero equals ceiling only for a negative numerator over a
positive denominator; the executable helper treats positive and negative cases
separately. A transition charges the signed endpoint difference. Thus complete
paths telescope, round trips return exact pool cash before external fees, and
wrapper decomposition cannot change aggregate facility cash.

### Whole-box price proof

It is insufficient to check only the all-buy and all-sell diagonal corners.
For outcome `i`, its price is minimized at the mixed adverse corner

```text
q_i = -B_i
q_j = U_j, j != i.
```

The policy checks all `n` such corners using exact integer numerators:

```text
a_i*b*n >= A*((n-1)*B_i + sum_{j != i} U_j),
pi_i = a_i/A.
```

Every price is linear in `q`, so those checks prove nonnegative prices over the
entire box. Numerators always sum to their common denominator; every admitted
instantaneous price is therefore an exact simplex vector.

## Three separate capitalization gates

### 1. Adverse-selection loss

For every signed `q` and simplex payout `w/D`:

```text
q dot w/D - C(q) <= b/2 * max_j ||e_j-pi||^2.
```

The curve-loss theorem requires

```text
K >= ceil(b/2 * max_j ||e_j-pi||^2)
```

This covers terminal curve loss. It is not bid cash capacity by itself.

### 2. Cash financing

Because every price is nonnegative throughout the box, `C` is coordinatewise
nondecreasing there. The exact minimum cash endpoint is the componentwise lower
corner `L_i=-B_i`. The actual sponsor deposit must make the minimum-share pool
satisfy

```text
c0 + K + C_hat(L) >= 0.
```

Accordingly, the API exposes two different minima: `minimum_sponsor_subsidy`
is only the first-loss theorem bound, while `minimum_sponsor_capital` is the
larger of that bound and the exact lower-corner financing need after minimum LP
cash. Initialization uses the latter. A sponsor may therefore contribute more
than the loss bound to finance bids; that additional present cash is still a
separate donation on activation, never LP principal or Hoard backing.

The distinction is load-bearing. With two uniform outcomes and `b=100`, the
loss subsidy is `25`, while buying `100` complete sets reaches
`q=(-100,-100)` and `C_hat(q)=-100` without changing prices. Zero LP cash plus
`K=25` cannot pay the trade and initialization refuses it. Either `75` present
LP cash plus `K=25`, or zero LP cash plus `K=100`, admits exact equality.

### 3. Egg custody and arithmetic

For the minimum activated share supply:

```text
g0_i >= U_i.
```

For the maximum share supply:

```text
g0_i + B_i <= 10^12.
```

Thus every point in the declared box has nonnegative, bounded physical Egg
custody. Cash, Egg, potential, and share arithmetic use checked integers. A
live adapter must still prove actual token balances after every transfer.

`10^12` is a single-source and per-coordinate bound, not an aggregate-pool
bound. Nonnegative simplex prices imply `|C_hat(q)| <= ||q||_infinity`, so live
cash is conservatively bounded by `3*10^12` from LP cash, sponsor cash, and the
curve term. Exact simplex redemption adds at most the largest Egg-custody
coordinate, giving a conservative terminal bound of `4*10^12`. The executable
state checks those distinct aggregate caps.

## Terminal solvency and LP yield

For authenticated integer payout weights `w_i >= 0`, `sum w_i=D`, the pool
redeems its actual Eggs and ends with

```text
T(q,w)
 = c0 + K + C_hat(q) + (g0-q) dot w/D
 = [c0 + g0 dot w/D]
   + [K + C_hat(q) - q dot w/D].
```

The first bracket is the exact terminal payoff of the LP-contributed basket.
The second is V1 pool yield `Y`. The global loss bound and upward potential
rounding prove

```text
Y >= 0
T >= contributed-basket terminal payoff
```

for every admitted state and every payout vector. This is a state-contingent
in-kind principal floor, not a promise about the Eggs' acquisition price,
fiat value, opportunity cost, or secondary-market mark.

Each per-share Egg coordinate is a multiple of the full payout denominator, so
one share's basket payoff is an integer under every payout. Trade flows use the
same conservative lot. Smaller lots require the protocol's named fractional
credit owner; they may not be introduced by flooring.

### Expenses are outside the theorem

The model charges no LP-borne maker fee, transfer tax, keeper reward, rent,
resolution expense, or token extension debit. One such atom can violate a tight
principal floor. A live profile must either:

- exclude those expenses from pool assets;
- charge them to traders under the separately conserved batch fee relation; or
- pre-fund a distinct finite expense compartment above `K` and LP capital.

Expected rebates or future volume never repair the guarantee.

## Share accounting without an oracle NAV

V1 never tries to price an arbitrary cash-and-Egg deposit into a scalar share.
The policy freezes one capital unit:

```text
one share <-> (c_u cash, g_u0 Eggs_0, ..., g_u(n-1) Eggs_(n-1)).
```

Before the funding deadline, a contributor may mint or burn only an integer
number of those units. This makes issuance order-independent and needs no
oracle, dealer mark, or selected outcome. A fixed array holds at most eight
unique LP owners; repeat contributions from one owner aggregate into its one
record.

After activation:

- share supply and owners freeze;
- no deposit, transfer, or withdrawal is admitted;
- a share remains locked through resolution; and
- terminal cash is allocated once over the exhaustive frozen share set.

These restrictions are conservative, but honest. An LP cannot withdraw a
pro-rata state-contingent basket mid-curve without also scaling `q`, depth,
subsidy, signed bounds, and every outstanding auction commitment. V1 refuses
that unsupported transition.

### Terminal integer allocation

For terminal pool `T`, position shares `s_h`, and total `S`, the model computes

```text
base_h = floor(T*s_h/S)
rem_h  = (T*s_h) mod S.
```

Remaining atoms go to descending remainder, then lexicographically smaller
immutable owner identity. Allocations sum exactly to `T` and may be claimed in
any order. Since the per-share baseline payoff `P` is integer and
`T=S*P+Y`, every holder receives at least `s_h*P`; Hamilton rounding applies
only to nonnegative yield.

The rule is deterministic, but identity splitting can influence at most the
bounded terminal yield dust. A live beneficial-owner aggregation rule would
need its own authenticated exhaustive-set semantics; V1 does not pretend a
wallet address proves beneficial ownership.

## Funding, queue, shutdown, and resolution

```text
Funding --sufficient + permissionless activation--> Trading
Funding --underfunded/stale cancellation----------> Cancelled
Trading --sponsor halt / queue quorum / timed close-> UnwindOnly
Trading or UnwindOnly --authenticated maturity----> Resolved
```

### Funding failure

The policy freezes minimum and maximum shares and a funding deadline. If
underfunded after that deadline, anyone may cancel. A sufficiently funded but
never activated pool becomes cancellable at trading close. In `Cancelled`, LPs
withdraw their exact unit baskets in any order and the sponsor separately
refunds `K`. No activation means no subsidy donation.

### Exit queue

An LP may irrevocably queue some or all shares. A checked share-weighted rational
threshold atomically moves the dealer to `UnwindOnly`. Queueing does not redeem
shares and does not promise a counterparty. It is a risk-stop vote, not LP
liquidity.

In `UnwindOnly`, every coordinate must move monotonically toward zero without
crossing it:

- positive `q_i` may only decrease toward zero by buying Egg `i`;
- negative `q_i` may only increase toward zero by selling Egg `i`; and
- zero exposure cannot reopen.

Failure to unwind is safe. At maturity, any caller with the authenticated payout
may resolve directly from either live phase. The pool redeems its actual Egg
custody through ordinary claim redemption and allocates all terminal cash. No
sponsor or LP signature is required for close or resolution. Caller inclusion,
source publication, and rent still require separately prepaid liveness.

## Aggregate call-auction receipts

The dealer is one nonlinear candidate leg, not an independent sequencer. For
each facility in one candidate, the solver must first net and aggregate every
native fill into one canonical `sell_to_users`/`buy_from_users` vector. The
verifier recomputes:

- policy, phase, slot, and exact pre-generation;
- signed endpoint and full box/domain admission;
- `C_hat(q')-C_hat(q)` total collateral;
- physical `g0-q'` Egg custody;
- exact pool cash; and
- componentwise Egg flow.

The endpoint determines total dealer cash, not a unique per-user allocation.
`clutch_batch::dealer_leg_v2` is the sole per-order allocation owner. Its frozen
`MinimumGrossHamiltonV1` rule selects the least feasible gross payer and
receiver totals, allocates payer cash by residual buyer capacity, satisfies
seller minima, and assigns forced excess payout by native Egg flow, with exact
Hamilton remainders and immutable-order-ID ties. The signed facility neither
accepts candidate-supplied allocation bytes nor reimplements that rule. It
binds an authenticated dealer verdict to the exact facility, policy, and
pre-generation and independently recomputes only the aggregate curve receipt.

The live adapter must authenticate every price and dealer quote precondition,
obtain the verdict from the checked dealer relation, reconcile its aggregate
receipt with the facility, and close all user, dealer, and fee transfers in one
atomic transition. The pure verdict is a projection, not an authentication
token. Its external fee amounts are upstream-quoted semantics, not proof of fee
funding, custody, recipients, or transfer conservation. Standalone offchain
quotes remain indicative until their generation and aggregate candidate execute
atomically. The protocol still says “best valid submitted candidate,” not
optimal clearing.

Mixed cross-outcome flow may contain gross cash in and cash out even though the
facility receipt stores only their canonical net. Candidate validation checks
that the pool can stage gross outputs after gross inputs and that the signed
difference equals the endpoint receipt. Gross facility flow must not be scored
as independent economic volume without the batch relation's anti-wash policy.

Same-outcome buys and sells must be netted before the facility leg. A fully
net-zero facility flow is omitted; user orders may cross each other elsewhere
in the candidate. Canonical wrapper orders first authenticate and decompose to
the exact native claim domain. Wrapper labels never create extra dealer
inventory or a second price.

## Hoard and custody boundary

This covered dealer is intentionally different from the issuance facility:

- LP Eggs are existing bearer claims whose backing is already in Market Hoard;
- depositing, buying, or reselling them does not move or duplicate that backing;
- `c0` and `K` are pool cash, never claimant principal;
- at resolution, the dealer burns/redeems only its actual `g0-q` Eggs through
  the ordinary resolved-claim route; and
- externally held Eggs remain globally backed and independently redeemable.

The pure model has no facility Hoard field. A live adapter must authenticate
native mints, Market/Terms/Instance, vault owners, exact balances, and the
existing Hoard/supply invariant. It must reload after every transfer, refuse
transfer-fee or opaque-balance profiles, and segregate unsolicited surplus.

## Adverse selection and market quality

Two-sidedness does not eliminate the cost-function maker's information loss.
Traders can buy underpriced Eggs and sell overpriced ones as the terminal event
becomes observable. `K` makes that extraction finite and prepays it; the queue
and close window bound how long the pool offers risk.

Economically, V1 separates three returns:

1. LP basket principal: the same terminal payoff those contributed assets would
   have produced outside the dealer;
2. prepaid subsidy: the donated portion of `K` remaining after curve loss; and
3. favorable dealer P&L: any additional nonnegative curve residual.

There is no claimed fee yield. LPs still bear custody, smart-contract,
opportunity-cost, and the state-contingent value of their original basket.
Sponsors intentionally pay to create depth. Whether that is desirable depends
on price-discovery and product-bootstrapping value, not an invented APR.

Common-set buying or selling moves `q` along the all-ones direction and leaves
relative prices unchanged, yet consumes finite cash, Egg custody, and box
capacity. With no Split/Merge inside this dealer, that is a known capital
inefficiency and possible capacity-grief vector. Candidate policy may omit
economically pointless flow, but the base state remains solvent.

One-ceiling pricing can also create zero/one-atom discrete effects near the
origin. It preserves telescoping and cannot worsen the subsidy loss bound. A
production profile should measure them at its actual collateral precision and
lot size.

## What the executable model proves

- exact signed potential and simplex prices with correct negative ceiling;
- all mixed adverse price corners for the declared signed box;
- exact lower-corner bid financing, separately from loss subsidy;
- physical cash and Egg custody for every admitted endpoint;
- exact unit-basket share issuance and pre-activation withdrawal;
- state-contingent LP basket floor under every payout;
- share-weighted irreversible queue and componentwise unwind;
- deterministic terminal Hamilton allocation and claim-order independence;
- failed/stale funding cancellation with separate sponsor/LP refunds; and
- checked fixed-capacity rollback under adversarial mutations.

It does not prove a live account codec, token custody, authentication of live
dealer/price projections, atomic application of derived user allocations,
source publication, transaction inclusion, fee/rent funding, CU/rent feasibility,
beneficial-owner identity, counted terminal account retirement, or formal
refinement. Those remain promotion gates, not implementation details. The pure
state deliberately retains an immutable resolved share/claim record even after
all claims are paid; a live close must join the repository's counted-retirement
owner rather than erase that evidence ad hoc.
