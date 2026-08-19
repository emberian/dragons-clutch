# Economics admission and self-sustainability checkpoint

Status: **MODEL-ONLY / PROPOSED**. The executable model is
[`research/economics-admission/`](../../research/economics-admission/). It is an
independent exact-integer falsifier, not a promoted Realm policy, consensus
implementation, cost measurement, liveness guarantee, or revenue forecast.

## 1. The actual promise

Dragon's Clutch needs three ledgers that never borrow from one another:

1. claimant solvency in Realm-collateral atoms;
2. mandatory-work liveness in lamports plus any separately promised service
   reward atoms;
3. optional venue income in collected fee atoms.

The first two are admission invariants. The third is maintainer upside. Later
volume, a token's exchange price, emissions, a buyback, a new subscriber, and a
treasury rescue all have coefficient zero in an admission equation.

Maintainer cashflow is also reported without a hidden exchange rate:

```text
direct_SOL_surplus = direct_SOL_service_revenue - measured_SOL_cost
```

Realm-collateral treasury atoms remain a separately denominated asset. Even an
arbitrarily large collateral treasury does not make `direct_SOL_surplus`
nonnegative unless somebody performs a separately authorized conversion outside
the admission invariant. This keeps business break-even honest without making
cross-asset purchasing power a protocol promise.

For the collateral plane, define:

```text
H = protected claim collateral
F = free user cash
R = reserved order consideration
E = reserved worst-case fee head-room
P = collected, unallocated fee pot
M = maker rebates claimable
X = batch-executor rewards claimable
T = treasury revenue claimable

C_accounted = H + F + R + E + P + M + X + T
```

Every modeled transition conserves `C_accounted`. Only `F -> H` can capitalize
claims. Order reservation is `F -> R + E`; fill is `R -> seller F` and
`E -> P`; cancellation is `R + E -> F`; allocation is `P -> M + X + T`.
No fee, keeper, rent, or treasury method can address `H`.

This aggregate model intentionally does not replace the production pooled-
custody equation or per-owner reservations. It states the cross-pool conservation
obligation the runtime must refine.

## 2. Admission equation

Let `J` be the complete finite set of mandatory jobs remaining immediately after
the proposed transition. Each job freezes three independent maxima:

```text
w_j = maximum work payout in lamports
s_j = maximum storage/rent principal in lamports
r_j = maximum supplemental service payout in reward-asset atoms

W_required = sum(j in J, w_j)
S_required = sum(j in J, s_j)
R_required = sum(j in J, r_j)
```

Admission requires present dedicated balances:

```text
work_lamports    >= W_required
storage_lamports >= S_required
service_atoms    >= R_required
```

If `R_required > 0`, the immutable service asset must be named. The model treats
`DREGG`, another SPL token, and any other label identically. Reward atoms cannot
be converted into lamports inside admission; therefore even an imagined infinite
reward-token price cannot repair one missing lamport, and a price collapse to
zero cannot make an already prepaid SOL obligation insolvent.

Market creation books observation, repair, finalization, and cleanup. A later
order, page, or feed extension that creates more unavoidable work must execute a
new admission transition and bring its own worst-case work/storage/service
deposit. A venue fee on that order is revenue, not its liveness reserve.

Completing job `j` may pay only amounts no greater than `(w_j,s_j,r_j)`. Removing
the job releases the unused maximum, so the same inequalities remain true for
all remaining jobs in every completion order. Locked storage principal returns
only to the storage pool after valid account closure; it is not ordinary work or
treasury income.

No finite maximum guarantees Solana inclusion under unbounded congestion or
censorship. The correct claim is: all admitted finite payments remain covered;
new admissions stop before measured costs approach the frozen ceiling; the
immutable failure transition remains funded.

## 3. Shared-feed capitalization and refund neutrality

For a frozen feed epoch with maximum SOL reserve `B` and `k` current subscribers,
the canonical net capital shares are:

```text
q, r = divmod(B, k)
share_i(k) = q + 1  if i < r
             q      otherwise
sum_i share_i(k) = B
max_i share_i(k) - min_i share_i(k) <= 1 atom
```

The first subscriber deposits `B` into the reserve. Subscriber `k+1` deposits
`share_k(k+1)`. That deposit is exactly the sum of every incumbent's reduction:

```text
deposit_(k+1) = sum_i (share_i(k) - share_i(k+1))
```

It reimburses incumbents; the active reserve remains `B`. No future subscriber
is assumed, so the first Market really can settle alone. Atom residuals favor
the deterministic earliest indexes by at most one atom; an onchain design must
freeze the ordering/index semantics before claiming O(1) joins.

On successful completion after actual accepted keeper spend `A <= B`:

```text
cost_i   = integer_share(A,k)[i]
refund_i = share_i(B,k) - cost_i
sum cost_i = A
sum refund_i = B - A
```

Thus joining and settlement mint no value, every subscriber's net capital equals
its cost plus refund, and arrival order changes any equal share by at most one
atom. On terminal data failure, subscribers receive no refund; `B-A` goes to the
predeclared source-wide neutral reserve/sink. The creator, resolver, current
claimants, maker, executor, and treasury cannot receive it. That neutral sink is
still a policy decision and needs an onchain semantic owner.

## 4. Fee-base comparison

For exact simplex prices `p_i >= 0`, `sum p_i = S`, and one atomic nonnegative
payoff vector `a_i`, compare:

```text
N_flat(a,p) = sum_i p_i a_i / S

N_disp(a,p) = [sum_(i<j) p_i p_j |a_i-a_j|] / S^2
```

For one Egg of quantity `q` at price `p`:

```text
N_flat = q p / S
N_disp = q p (S-p) / S^2
```

The executed comparison calibrates a 20 bp flat control against `kappa=40 bp`
dispersion, so their exact fees coincide at `p=S/2`. These are comparison arms,
not proposed constants.

| Surface | Flat cash notional | Simplex dispersion |
| --- | --- | --- |
| `p=0` | zero | zero |
| `p=S` | charges a cash-equivalent sure claim | zero because no contingent risk moves |
| low-price tail, relative to cash paid | constant rate | approaches `kappa` (twice the calibrated midpoint flat rate) |
| high-price claim | constant rate | falls toward zero with remaining uncertainty |
| add a risk-free complete set | charge increases | exactly invariant |
| binary claim vs complement representation | asymmetric | symmetric |
| split a state into identical-payoff subcells | exact base invariant | exact base invariant |
| terminal-ceil every artificial leg separately | vulnerable to extra atom ceilings | vulnerable to extra atom ceilings |
| self-wash at positive fee | loses retained treasury share | loses retained treasury share |
| self-wash at a zero-fee boundary | no reward is created | no reward is created |

The shared lesson on partitioning is stricter than “choose dispersion”: compute
one exact fee for the atomic signed-intent payoff vector and carry its fraction
through that intent's lifetime. Per-Egg or per-page terminal rounding lets a
partition manufacture extra ceilings under either base. Resetting a floor carry
lets fragmentation erase fees.

### Recommended V1 default basis

Use **atomic simplex dispersion** as the default venue risk-transfer fee basis,
with a terminal-ceil persistent carry owned by one signed intent. Do not add a
percentage split, merge, transfer, or redemption fee. Do not use flat notional as
a hidden minimum: that would reintroduce the complete-set and complement
representation defects.

This recommendation is about the *basis*, not the coefficient or revenue split.
The coefficient, maker fraction, executor cap, and treasury fraction remain
unresolved until measured route elasticity, landing cost, market quality, and
signed-UI comprehension exist. The simple flat control remains the fallback if
dispersion fails a promotion gate.

Zero-price and certain-price orders do not fund an executor through the fee pot.
That is deliberate: `fee=0` implies maker rebate `=0` and executor reward `=0`,
so wash volume cannot mint a subsidy. Whoever submits optional zero-fee work pays
its transaction cost. If the protocol promises that clearing or settlement is
mandatory, its worst-case lamports must instead be booked by market/order
admission—not hoped for from fees.

## 5. Fee allocation and wash bound

For a collected pot `P`, proposed integer share selectors `m/d` and `x/d`, and a
per-batch executor cap `K`:

```text
maker    = floor(P*m/d)
executor = min(floor(P*x/d), K)
treasury = P - maker - executor
```

Every atom in these destinations came from a named payer's reserved fee
head-room. If one Sybil controls buyer, seller, maker, and executor, its maximum
collateral recovery is `maker+executor`; its wash loss is at least `treasury`,
plus separately denominated network lamports. The model never converts those
lamports through a token-price oracle. If `P=0`, all three destinations are zero.
No emission, point, creator-volume reward, fee-share staking reward, or buyback is
part of this model; adding one invalidates the wash result and requires a new
analysis.

## 6. Falsifiers and promotion boundary

The default-basis recommendation is falsified, not defended, by any of:

1. an economically equivalent signed intent that settles the same net payoff but
   pays less dispersion fee other than one documented rounding atom;
2. failure of complete-set translation, relabeling, complement symmetry,
   homogeneity, subadditivity, or identical-payoff partition refinement in the
   frozen integer implementation/proofs;
3. a cheap way to reset or abandon fee carry below the terminal ceiling;
4. positive wash return after every maker/executor/reward path, including any
   later incentive system;
5. width overflow at frozen outcome, price, payoff, and lot bounds;
6. worse route leakage or user cost than the lowest flat control that satisfies
   measured contribution and market-quality floors;
7. a signing explanation users cannot distinguish from notional tax;
8. any mandatory transition whose liveness depends on that fee being collected.

The current tests exercise protected-pool conservation, underfunding on each
asset axis, reward-price independence, job-order independence, shared-feed join
and refund identities for caps `0..32` and subscribers `1..16`, fee boundary and
tail behavior, complete-set and complement behavior, exact partition refinement,
per-leg rounding risk, persistent-carry fragmentation, exhaustive binary-price
fee head-room, allocation conservation, and Sybil wash signs. Passing them
establishes only that this offline model has not hit those bounded falsifiers.

Before promotion, the same equations need one semantic owner in account layout,
hostile-byte parsing, production transition tests, Verus/Rocq arithmetic checks,
real SBF/Token-2022 execution, measured cost inputs, and a clean evidence
manifest. None of this document changes the current release STOPs.
