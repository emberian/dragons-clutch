# Cryptoeconomic design

## 1. Three independent economic propositions

1. **Solvency:** claimant principal covers every allowed terminal payout.
2. **Liveness:** every mandatory future job is prepaid when the obligation is
   admitted.
3. **Revenue:** optional activity may pay maintainers and replenish future public
   infrastructure.

Revenue cannot prove either solvency or liveness. This distinction survives zero
volume, team disappearance, and collapse in either the chosen collateral's or an
optional reward token's external exchange value.

## 2. Protected pools

| Pool | Asset | Permitted payments | Forbidden payments |
|---|---|---|---|
| Market Hoard | Realm collateral | Complete merges and resolved redemptions | Keepers, rent, rebates, creator refunds, treasury |
| Order escrow | Realm collateral/Eggs | Matched consideration and cancellation refunds | Claim liability |
| Batch fee pot | Realm collateral | Maker rebate, capped clearer reward, protocol share | Hoard shortfall |
| Market liveness | SOL | Market-specific finalization and cleanup | Claims or treasury |
| Shared-feed reserve | SOL | Accepted observations and repairs | Claims or withdrawals while booked |
| Keeper endowment | Frozen reward asset, optionally DREGG | Supplemental accepted-job rewards | Claims |
| Rent bond | SOL | Account/mint/page creation and valid close refund | Ordinary work |
| Treasury | Disclosed fee assets/SOL | Protocol operations | Any guaranteed obligation |

A one-way treasury top-up into liveness is safe. Reverse movement out of booked
liveness is forbidden. The Hoard is never lent, staked, rehypothecated, or netted
against another Market.

## 3. Worst-case liveness accounting

For every unfinished job `j`, freeze the maximum unavoidable SOL payout and, if
the Realm uses one, the maximum supplemental reward-asset payout:

```text
B_SOL[j]
B_REWARD[j]
```

The invariant is:

```text
liveness_SOL.balance    >= sum(B_SOL[j]    for unfinished j)
liveness_REWARD.balance >= sum(B_REWARD[j] for unfinished j)
free(asset) = balance(asset) - booked(asset) >= 0
```

Market admission fails unless the invariant remains true after adding every
observation, repair, finalization, and cleanup obligation. Expected fees, future
top-ups, subscribers, token appreciation, and treasury charity count as zero.

No finite bounty guarantees inclusion under unbounded congestion or censorship.
The honest guarantee is conditional liveness while the frozen maximum remains
competitive, followed only by a deterministic exhaustion disposition for the
prepaid repair reserve—not a failure-selected payout—if the repair window
closes. Admission of new obligations stops when recent landing-cost quantiles
approach the supported maximum.

The candidate keeper schedule reserves the maximum but pays a reverse-Dutch
amount:

- initial bounty near `1.2 * measured P50 all-in cost`;
- deterministic steps as the deadline approaches;
- final bounty at least `2 * measured P99.9 cost`;
- only the first novel accepted transition earns;
- catch-up may earn the sum of novel work minus a frozen batching discount.

SOL pays unavoidable network/provider costs. A frozen reward asset—DREGG in the
house configuration—is supplemental service income. There is no automatic
critical-window collateral/reward-to-SOL swap and therefore no circular price-
oracle dependency.

## 4. Shared-feed capitalization

One feed bucket serves every Market with identical source/grid semantics. Work is
booked over the union of buckets, not the sum of market requests:

```text
shared work = |union(required buckets)|
naive work  = sum(|market buckets|)
```

For a feed epoch with maximum reserve `B`, the first subscriber capitalizes `B`.
When subscriber `k` joins, it deposits `B/k`; each of the previous `k-1`
subscribers accrues reimbursement `B/[k(k-1)]`. After the update all `k`
subscribers have equal net at-risk capital `B/k`, while the reserve still contains
`B`. A cumulative reimbursement index could make joining and later claiming O(1);
the executable model (`research/economics-admission/model.py`) instead recomputes
canonical shares in O(k) per join and says so. No O(1) index is implemented
anywhere.

Subscriptions become irrevocable once committed. Otherwise departure would
retroactively increase other Markets' frozen obligations. At successful epoch
completion, unused reserve may be divided by the frozen rule. After a data-failure
outcome, unused funds must not return to the creator, resolver, or current
claimants; doing so would pay an interested party for inducing failure. Residual
funds roll into the source-wide liveness reserve or a predeclared neutral sink.

## 5. Simplex-auction fee hypothesis

Kernel operations and ordinary token movement remain percentage-free:

- no split fee;
- no merge fee;
- no redemption fee;
- no Token-2022 transfer fee or transfer hook.

Those taxes corrupt the complete-set arbitrage bands, terminal payoff, or external
routing. The simplex venue instead charges filled state-contingent risk transfer
in Realm collateral. For a single Egg this remains:

```text
F = kappa * q * p * (1 - p)
kappa = 0.004                       # initial experimental policy
```

For an atomic portfolio `a` under scaled simplex prices `p_i`, the candidate fee
base generalizes to:

```text
G_num(a,p) = sum_{i<j} p_i * p_j * abs(a_i - a_j)
```

with exact scale and one final carry-aware division. This is invariant to adding a
risk-free complete set, symmetric under outcome relabeling, and reduces exactly to
`q*p*(1-p)` for one Egg. See [FEE_GEOMETRY.md](FEE_GEOMETRY.md).
[research/RISK_SUMMED_POSITIONS.md](research/RISK_SUMMED_POSITIONS.md) §3 pins
the base exactly: it is the *unique* positively 1-homogeneous functional that
charges each digital layer its own `q(1-q)` and adds over layer-cake
decompositions (Propositions 11-12), and it is **not** the model-free risk
norm — `G(a,p) <= R(a)/4` with the ratio `2p(1-p)` vanishing at extreme prices
(Proposition 10), and at boundary prices its kernel strictly exceeds the risk
quotient (Proposition 9), so risk transfer supported on zero-priced outcomes is
feeless.

**Selected shape, 2026-08-20.** The adopted V1 fee base *shape* is the additive
composite `kappa*G(a,p) + kappa'*R(a)` — the dispersion base above with a
price-free quotient-norm floor, which closes the zero-price channel by making
the kernel exactly `span(1)` at every admissible price vector
([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md) item 9, on
[decisions/REPORT_fee-base-selection_2026-08-20.md](decisions/REPORT_fee-base-selection_2026-08-20.md)).
**Both rates remain undecided**, every consensus byte stays `FeeBaseV1::None`
until the RevenuePolicy destination lands, and the selection is reversible until
a rate freezes. Flat-notional and per-leg were run against it and eliminated;
the price-free arm remains a control. The market-quality axes are ratified out
of V1 scope for this selection — see
[FEE_GEOMETRY.md](FEE_GEOMETRY.md) §6/§7.

All values use exact scaled integer arithmetic. At `p = 0.5`, the candidate fee
is exactly 20 basis points of cash consideration as a rational — at size only.
The terminal-ceil close charges a minimum of one atom per fee-bearing intent,
and the laboratory's own fee vector (`FEE-001`,
`research/economics/fixtures.py`) records a 1-atom fee on 1 atom of
consideration: 10,000 basis points on the smallest fill. Distribution envelope:

- 60% standing-maker rebate;
- at most 15% batch executor, capped by that batch's collected fees;
- at least 25% protocol treasury.

**Adopted V1 vector, 2026-08-20:** 60/0/40 with the executor share deferred and
the trivially-true `AllRestingMakers` standing-maker predicate
([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md) item 8, on
[decisions/REPORT_revenue-policy-v1_2026-08-20.md](decisions/REPORT_revenue-policy-v1_2026-08-20.md)
B4e). The envelope above becomes a structural `validate()` refusal rather than
prose. The vector constrains nothing until a fee-bearing Realm exists.

For the single-Egg midpoint example the envelope's shares correspond to
approximately 12, 3, and 5 basis points. The split is still design prose: no
Rust implements it. The only
Rust fee allocator in the tree (`research/liquidity-policy-model`) distributes
by LP capital-time weight — a different mechanism — and has no consumers; the
Python laboratory's `allocate_fee` is the split's sole executable form. Empty
batches and zero-fee fills pay no executor subsidy. Obvious same-authority
self-crosses are rejected. A Sybil controlling taker, maker, and executor
recovers at most 75% of fees paid, which leaves its collateral-domain net
non-positive, not strictly negative: exactly zero on zero-fee fills when carry
is dropped, and strictly negative only under the terminal-ceil close, which
charges at least one atom per fee-bearing intent. Network costs are an
additional loss in SOL, a separate asset with no conversion assumed. The
terminal-ceil close, not the split alone, is what makes wash cycling strictly
costly.

Taker fees round upward; rebates round downward; residual atoms follow a frozen
allocation. Order splitting must not erase fees. The fee schedule is immutable per
Market and selected only from audited policies under a protocol hard cap.

The curve and portfolio generalization are hypotheses, not revenue entitlements.
Test them against flat-notional and decomposed-leg controls over midpoint-
equivalent rates of 0, 5, 10, 20, 35, and 50 basis points. Choose the lowest rate
satisfying market-quality and positive-contribution floors. That instruction is
not executable in the tree today: the market-quality axes it reads have no
simulator, and the 2026-08-20 descope ratified them out of scope for V1 *base*
selection ([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md)
item 9). It stands as the standard for the **rate** decision, whose own record
is that the question returns there — measurable on a live venue rather than in a
simulator the tree does not have. Liveness reliability
is independently prepaid and therefore cannot justify raising the trading fee.

## 6. Maintainer break-even

For one Realm, let:

```text
W       = sum(state-contingent fee base) over filled trades, in collateral atoms
a       = treasury fraction of fee (initially 0.25)
x_floor = conservatively haircutted SOL per collateral atom
P_SOL   = optional SOL service-premium revenue
O_SOL   = measured protocol operating and maintenance cost
```

Then maintenance break-even is:

```text
a * kappa * W * x_floor + P_SOL >= O_SOL
```

This is a business measurement, not an admission invariant. In every
configuration currently true in the tree, the fee is forced to zero, so the
inequality returns unbounded required volume: no volume covers any cost. The
"$2,000 of volume per dollar of cost" figure is arithmetic on an assumed
five-basis-point net take, not a measurement of anything. If volume does not
arrive, the team is not funded—but already accepted Markets still settle from
prepaid resources.

No emissions, points, wash rebates, or fee-share staking is required to
manufacture activity. A DREGG Realm may create organic DREGG demand, and a Realm
may nominate DREGG as an optional keeper reward, but Eggcrate never privileges
that mint and no other Realm must touch it.

Fee destinations are an immutable deployment/Realm `RevenuePolicy`, not Eggcrate
solvency law. `RevenuePolicy` is currently prose: it is named as an architectural
boundary in four documents and implemented in zero lines of code. Every
destination must be separately disclosed, conflict-reviewed, and unable to spend
Hoard principal or booked liveness funds. See
[DEPLOYMENT_REVENUE_BOUNDARY.md](DEPLOYMENT_REVENUE_BOUNDARY.md).

**Destination decisions adopted 2026-08-20**
([decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md) items 6 and
8, on
[decisions/REPORT_revenue-policy-v1_2026-08-20.md](decisions/REPORT_revenue-policy-v1_2026-08-20.md);
design at [design/REVENUE_POLICY_V1.md](design/REVENUE_POLICY_V1.md)):

- **Plane L — the five `ResolutionWork` charges are permanently zero as frozen
  policy, not as a placeholder** (item 6, B4c). No vault is built and no record
  is built for the lamport plane; the `lamport_sink` member stays reserved. The
  reason of record is the protected-pools row plus the anti-liveness argument —
  *not* the source comment's "V1 has no authenticated fee sink", which stops
  being the true rationale the moment any sink exists. This is the weak form of
  permanence: a V2 cost schedule is a sibling const with its own digest and may
  reintroduce charges for **new** Works, breaking no in-flight promise, because
  Begin freezes the schedule digest per Work.
- **Plane C** — fee atoms are credited to a treasury `PositionAccount`, with the
  mid-epoch-close grief rider joining the hostile walk (item 8, B4b); sequencing
  is Plane L before Plane C per B4c (item 8, B4d); the B4a custody requirements
  are adopted with the **treasury pubkey deferred to the first fee-bearing
  Realm** and reserved to ember.

None of this makes any charge nonzero. Every `max_fee_atoms == 0` gate stays
closed, and the break-even inequality above keeps returning unbounded until a
rate is decided and a destination is built.

## 7. Data-failure incentives

An equal failure payout is not neutral. When one outcome is nearly certain, cheap
tail Eggs may gain sharply if somebody can force equalization. A dedicated
`INVALID_DATA` Egg merely makes the incentive directly tradeable.

The preferred direction is:

1. preserve every monotone authenticated observation;
2. allow a long permissionless repair window with rising bounty;
3. resolve only if the frozen authenticated evidence relation selects one
   payout vector; otherwise retain claims in recoverable dormancy;
4. make resolver compensation independent of the selected payout;
5. cap common-mode exposure to any feed/bucket/source.

For bucket `(f,k)`, publish maximum affected collateral:

```text
A[f,k] = sum(market_cap[m] * maximum_payout_change[m,f,k])
```

If a defensible manipulation-cost lower bound `M[f,k]` exists, a conservative
initial admission cap is `A[f,k] <= 0.1 * M[f,k]`. When censorship or publisher
failure has no defensible numeric bound, expose a security tier and hard notional
cap rather than inventing one.

The R4 research profile now selects `EvidenceOnlyRecoveryV1`: a data gap never
selects a numeric payout. After finite independently prepaid repair, the market
remains dormant but recoverable by later valid evidence; complete-set merge and
claims persist. The equal-sum argument, rejected alternatives, terminal burn,
and fractional-credit STOP are recorded in
[`implementation/FAILURE_PAYOUT_DECISION_V1.md`](implementation/FAILURE_PAYOUT_DECISION_V1.md).
This is a model-only policy decision. Versioned ABI, source-specific evidence,
measured repair paths, Token-2022 lot encoding, and terminal burn integration
remain release gates.

## 8. Thin-market behavior

A thin Market may stop accepting native-auction orders. It must not stop
observing or settle to a numerical fallback because volume disappeared. In the
proposed `EvidenceOnlyRecoveryV1` profile, a missing required bucket moves the
market to `DEGRADED_RECOVERABLE`, stops new order exposure, displays the
evidence-only recovery rule, and runs only its finite independently prepaid
repair schedule. Ordinary external Token-2022 transfers remain possible because
no freeze authority exists.

Only a future implemented profile may retire a Market after every external and
internal liability, claimant credit, booked work, and declared terminal
dependency is zero. R4 shows arbitrary raw bearer units cannot generally reach
that state with a tombstone alone: nonzero claimant credits need persistent
segregated backing. Abandoned claimants are not confiscated to recover rent.
