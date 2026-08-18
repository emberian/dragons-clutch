# Economics laboratory implementation

Status: deterministic synthetic research scaffold. No rate, split, bounty, cap,
or fallback in this document is a promoted protocol constant.

## Purpose and boundary

The laboratory turns the cryptoeconomic claims in
[`ECONOMICS.md`](../ECONOMICS.md) into independently executable falsifiers. It is
host-only Python using standard-library integers and `Fraction`. It does not
import protocol code, access a wallet or RPC, query a provider, trade, submit,
deploy, or spend anything.

The code lives under [`research/economics/`](../../research/economics/):

- `model.py` defines exact reference transitions and accounting equations;
- `test_lab.py` performs bounded exhaustive and property-oriented checks;
- `run_lab.py` emits a stable, sorted JSON scenario report to stdout.

Run:

```sh
python3 -m unittest discover -s research/economics -p 'test_*.py'
python3 research/economics/run_lab.py > /tmp/dragons-clutch-economics.json
```

The second command writes only because the shell redirection explicitly requests
it; the runner itself has no filesystem output path.

## Implemented hypotheses and checks

### Protected-pool solvency

The categorical state model exhaustively walks bounded combinations of complete
splits, complete merges, direct external burns, resolution, winning redemption,
and losing-claim destruction. Every reachable state checks:

```text
Hoard >= maximum remaining allowed liability
```

The independent protected-pool model enumerates every permitted and forbidden
purpose. A Hoard debit for observation, repair, rent, maker rebate, protocol
revenue, or operations refuses without changing any pool. This is a host
falsifier for `P-SOLV-01` and `P-POOL-01`, not their proof.

### Prepaid liveness

Every unfinished job has frozen maximum SOL and reward-asset payouts. Admission
uses the sum of maxima, not expected payments. The lab checks all booking orders
for a small fixed job family, refuses undercapitalized combinations, releases
only the unused maximum after successful work, and rejects duplicate payment.

The reverse-Dutch model requires a monotone finite offer vector and books its last
element. It intentionally makes no unconditional-inclusion claim. Recorded fee
distributions and a defensible choice of P50/P99.9 multipliers remain future
provenance-bound measurements.

### Shared-feed capitalization and atom rounding

The prose equal-share rule uses `B/k`, but SOL and DREGG have indivisible atoms.
The laboratory names one deterministic rounding hypothesis:

```text
q, r = divmod(B, k)
first r subscribers carry q + 1 atoms; the rest carry q atoms
```

When subscriber `k` joins, its new capital share exactly funds the reductions in
all prior shares. Therefore the booked reserve remains `B`, subscriber shares
differ by at most one atom, and no future subscriber is assumed.

The reference model recomputes the full share vector so rounding is visible; it
is O(k) host analysis, not an implementation of the proposed O(1) cumulative
reimbursement index. E4 still has to specify and check an onchain constant-work
index, including its atom carry, claim, and final-residual rules.

On successful completion, actual keeper spend is allocated by the same rule and
the unused reserve is refunded. On terminal data failure, the model gives current
subscribers no refund and rolls the residual to a neutral source-wide liveness
reserve. This is a deliberate no-failure-reward hypothesis: the creator,
resolver, and current claimants do not receive the unspent amount. The identity
and governance of the neutral sink remain an open design decision.

Subscriptions are irrevocable in the model. Allowing departure would require a
new capitalization transition proving that no remaining subscriber inherits an
unfunded obligation.

### Failure and ambiguity attacks

For `n` equally funded outcomes, an attacker holding every nonwinning tail Egg
receives `(n-1)H/n` under equal fallback. With 16 outcomes and one percent total
tail acquisition cost, the synthetic scenario's net gain exceeds 90% of the
Hoard. This demonstrates that an equal vector hides rather than removes an
invalid-data trade.

Compatible-outcome settlement removes payouts for outcomes excluded by monotone
authenticated evidence, but narrowing from `n` compatible outcomes to two raises
one remaining tail's individual fallback weight from `1/n` to `1/2`. The lab
therefore does not assert that compatibility is universally incentive-reducing;
it reports both basket and concentrated-tail effects.

Common-mode exposure is computed before applying the experimental admission arm:

```text
A[f,k] = sum(market_cap[m] * maximum_payout_change[m,f,k])
A[f,k] <= 0.1 * manipulation_cost_lower_bound[f,k]
```

The ten-percent coefficient is a hypothesis. If there is no defensible numeric
manipulation/censorship bound, the lab cannot manufacture one.

### Fee curve, carry, and allocation

The candidate single-Egg curve is exercised at exact simplex prices:

```text
F = kappa * q * p * (1-p)
kappa = 0.004                  # experimental arm
```

At `p=1/2`, the exact gross burden is 20 basis points of cash consideration. The
runner reports the full curve and the tentative 60% maker, at-most-15% executor,
at-least-25% treasury allocation. Allocation floors maker and executor shares and
assigns all integer remainder to treasury, conserving every atom.

Two rounding policies are contrasted:

1. persistent fractional carry with one final floor is invariant to proportional
   order fragmentation inside the same carry domain;
2. stateless upward rounding cannot make splitting cheaper but can overcharge
   dust.

Resetting a floor carry across fragments is included as a known attack and makes
many small fees disappear. The carry's actual ownership domain—Position, signed
intent, Epoch, or another identity—remains a P2 decision.

The simplex-dispersion reference calculation also checks complete-set translation,
outcome relabeling, and identical-payoff partition refinement on bounded examples.

A self-washer controlling taker, standing maker, and executor can recover at most
the maker and executor allocations. With the experimental split it loses the
treasury residual, at least 25% of collected fee, plus network costs. The result
depends on there being no external emission, point, creator-volume rebate, or
unrelated subsidy.

### Price collapse and maintainer break-even

The price-collapse table holds DREGG Hoard atoms, DREGG liabilities, and prepaid
SOL constant while the external SOL/DREGG value falls through zero. At zero:

- atom-denominated solvency is unchanged;
- prepaid SOL liveness is unchanged;
- DREGG-denominated fee and supplemental keeper income have zero SOL value.

This is why SOL work cannot be booked against expected DREGG conversion.

Break-even sensitivity uses:

```text
a * kappa * W * x_floor + P_SOL >= O_SOL
```

where `W` is state-contingent fee base, `a` treasury share, `x_floor` a haircutted
SOL value per collateral atom, `P_SOL` measured service-premium revenue, and
`O_SOL` measured maintainer cost. The lab reports `unbounded` required volume when
`a*kappa*x_floor` is zero and service premia do not already cover cost. This
business equation never enters market admission.

## Current stop and promotion rules

- Do not promote `kappa=0.004`, 60/15/25 allocation, ten-percent source exposure,
  bounty multipliers, or the no-failure-refund sink from these synthetic checks.
- Stop a fee policy if resetting its required carry domain is cheap or if an
  economically equivalent encoding evades the fee.
- Stop a shared-feed policy if integer reimbursement can undercapitalize `B`, if
  subscribers can withdraw without replacement funding, or if the neutral sink
  gives an interested party a material failure payoff.
- Stop new liveness admissions when measured landing costs approach the frozen
  maximum. Existing markets continue through their prepaid repair/failure rule.
- Reject an external manipulation-cost cap when its lower bound is not
  reproducible and source-specific.
- Treat every passing check as evidence against modeled bugs only. It proves
  neither Solana behavior, authenticated source truth, keeper participation,
  market liquidity, nor regulatory/deployment readiness.

Future recorded inputs must use the result-directory and provenance contracts in
[`BENCHMARK_PLAN.md`](../BENCHMARK_PLAN.md) and
[`PROVENANCE.md`](../PROVENANCE.md).
