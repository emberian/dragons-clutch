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

## Addendum 2026-08-18: kernel alignment, payout candidates, fee payer debit

Status unchanged: deterministic synthetic research scaffold. Everything below is
MODEL (a property of the host-only code, checkable by the cited experiment) or
PROPOSED (a candidate awaiting decision). No candidate, carry domain, `kappa`,
allocation split, executor cap, or lot magnitude is promoted by this addendum.

This addendum implements section 3 and the lab-scoped rows of the section 5
matrix of
[`POLICY_ANALYSIS_LOTS_FEES.md`](POLICY_ANALYSIS_LOTS_FEES.md). It closes the
three lab-side mismatches named in [`ADVERSARIAL_REVIEW_V0.md`](ADVERSARIAL_REVIEW_V0.md)
section P1-E: the lab admitted payout vectors the kernel refuses, the lab had no
fractional payout semantics at all, and no fee in the lab debited a payer.

### Same admitted market set (section 3.1)

`maximum_liability` now refuses any payout vector whose weights do not sum to
**exactly** one collateral unit, matching the kernel's `sum(w_i) == D`. The
previous `sum <= 1` behaviour is gone rather than kept as an arm: the policy
analysis section 4 table assigns this mismatch to the lab under every candidate,
so there is no contrast arm to preserve. Refusal tests cover both sub-unit and
super-unit sums.

`model.py` gained an integer mirror of the kernel's shape rules —
`IntegerPayoutVector`, `IntegerPayoutSet`, `WeightedBook` — using plain integers
and never `Fraction`. It reproduces the kernel's refusal *ordering* as well as
its refusals: zero denominator, weight above the denominator, weights not summing
to `D`, nonzero weight padding, nonzero vector padding, mixed denominators, and
the `MIN_OUTCOMES`/`MAX_OUTCOMES`/`MAX_PAYOUTS` bounds. Refusals carry a shared
`error_class` string so the two languages can be compared without sharing types.

### Payout candidate arms (section 3.2)

`WeightedBook` walks four arms:

- `kernel_baseline` — the landed kernel; the arm under which the P1-A trap is
  reachable. It is kept as a labelled contrast arm, not as a recommendation.
- `one_hot` — PROPOSED candidate (a1): non-one-hot sets are refused at admission
  with `invalid_payout_weights`.
- `lots` — PROPOSED candidate (b1): `split`/`merge` gate on
  `L_split = lcm_i L_i`, `materialize`/`dematerialize` gate on
  `L_i = D / gcd(D, {v_i != 0})`, bearer transfers stay ungated.
- `credit` — PROPOSED candidate (c): redemption pays `floor(q*v_i/D)` and accrues
  the exact `1/D` remainder to a per-position credit, with the solvency invariant
  carrying `credit_num_total`.

The terminal complete-set redemption of section 1.5 exists in every arm.
`enumerate_weighted_traces` is the companion to `enumerate_solvency_traces`: it
walks one arm and reports refusal classes, sub-lot residency by phase, stranded
value, and exit-dead states — exit-liveness, not solvency alone.

### Payer-debit fee accounting and carry policy (section 3.3)

`run_fee_schedule` settles fills with explicit legs: `buyer cash debit = C + f_b`,
`seller cash credit = C - f_s`, `fee pot delta = f_b + f_s`. Every atom in the
pot came from a named payer's cash; the Hoard is never a fee source; the identity
`sum(buyer debits) - sum(seller credits) = fee pot delta` is a reported flag, not
prose. Consideration is computed by `exact_consideration`, which *refuses*
off-grid `(q, p)` pairs instead of silently flooring.

Carry policy is now two orthogonal choices: the domain (`position` / `intent` /
`epoch`) and the close policy (`terminal_ceil` / `dropped_carry`).
`fee_fragmentation_result` keeps its three original arms and adds
`terminal_ceil_total`, `dropped_carry_total` and `exact_ceil_total`.
`allocate_fee` gained the per-batch `executor_cap`; the uncapped default is
unchanged. `dispersion_numerator` is now wired into the debit path as the fee
base of record instead of existing beside it.

### Executed matrix rows

All rows below execute in the test suite and in `run_lab.py`; each states its own
falsification condition and checks it. Bounds are recorded in each result. None
was falsified at the bounds stated in the code.

| Row | What it executes |
|---|---|
| EXP-LOT-A1 | one-hot admission over all weight tuples (outcomes ≤ 4, D ≤ 6, sets ≤ 2) plus bounded traces asserting `remainder_required` unreachable |
| EXP-LOT-B1 | `L_i` is exactly the minimal exact-redemption modulus, both directions |
| EXP-LOT-B2 | lot-gated walks reach no Active-phase sub-lot balance and no exit-dead state; the P1-A split is refused with `lot_violation` |
| EXP-LOT-B3 | three-wallet bearer fragmentation: aggregate stays lot-aligned, trapping is sub-lot dust, recombination always recovers |
| EXP-LOT-B4 | lot magnitudes for the equal-weight compatibility families (data) |
| EXP-LOT-B5 | lot-aligned states never ceiling: reservation equals exact liability |
| EXP-LOT-C1 | credit conservation `q*v_i = D*paid + credit_delta`, carry in `[0, D)`, fragmentation-identical totals |
| EXP-LOT-C2 | fragmenting a redemption across positions never pays more and strands `< k` atoms |
| EXP-LOT-X1 | complete-set redemption is exact in every arm and exits the P1-A trap |
| EXP-LOT-X2 | retirement liveness per arm (data) |
| EXP-FEE-D1 | terminal-ceil pays exactly `ceil(exact)` per domain instance; cross-domain splitting never pays less |
| EXP-FEE-D2 | epoch domain with dropped carry collects zero while volume is positive |
| EXP-FEE-P1 | payer conservation, escrow head-room, untouched Hoard across every domain × close × side cell |
| EXP-FEE-P2 | both-sides versus charge-once-split (data) |
| EXP-FEE-G1 | the six seminorm identities, exact, at stated bounds |
| EXP-FEE-G2 | exact width maximum and u128 head-room for three proposed frozen-bound sets |
| EXP-FEE-W1 | self-wash sign over the whole policy matrix |
| EXP-FEE-A1 | allocation exactness with the executor cap |
| EXP-ALIGN-01/02/03 | the differential fixtures below, replayed through the lab |

Rows owned by other lanes are not implemented here: EXP-TERMS-01 targets the
static-client terms fixture, and the batch-relation obligations of sections 1.3
and 2.3 belong to the batch lane.

### Differential fixtures

`fixtures/economics/` holds three hand-authored, language-neutral families —
`admission_vectors.json`, `trace_vectors.json`, `fee_vectors.json` — with the
cross-language contract in `fixtures/economics/README.md`. The P1-A fixture is
trace vector #1. Expectations are authored from the policy analysis, not
generated from the model, and the lab is checked against them; the same files are
the contract for a future Rust consumer. Bytes are deterministic (sorted keys,
two-space indent, trailing newline, no timestamps) and regenerating must produce
no diff.

### Findings and resolved ambiguities

1. **The complete-set primitive must be lot-gated under candidate (b).** Section
   1.5 says the terminal complete-set redemption is exact for any quantity, and
   section 1.3 gates `merge` at `L_split`. Left ungated, the Resolved-phase twin
   of `merge` re-creates sub-lot internal balances and breaks (b)'s internal
   closure claim. The lab gates it at `L_split` in the `lots` arm only.
2. **Post-resolution sub-lot balances are normal and are not a defect.** After
   resolution the binding modulus is `D / gcd(D, resolved w_i)`, which can be
   smaller than the set-wide `L_i`; balances below the set-wide lot can still
   redeem exactly. EXP-LOT-B2 therefore checks Active-phase alignment and
   exit-liveness in both phases, and reports the resolved sub-lot count as data.
3. **The kernel landed the section 1.5 primitive while this lane ran.** Commit
   `d60ccf3` added `redeem_complete_set` for internal balances only. The lab's
   arms and fixtures already assumed the primitive, so the fixture step class
   for the internal side reads `landed_kernel`; the external-side complete-set
   exit used by `TRC-003` remains a lab extension with no kernel counterpart.
4. **Materialization can strand a complete set.** Moving one leg of a P1-A set
   across the Token-2022 boundary leaves an internal side and an external side
   that cannot each form a set, so the section 1.5 exit no longer applies and the
   position is exit-dead with liability outstanding (fixture `TRC-003`). Any
   complete-set exit rule has to say whether a set may be assembled across the
   internal/external boundary.
5. **Below the one-atom floor the two fee-side arms are indistinguishable.**
   At the section 5 dust bounds (`q <= 20` on the price grid), terminal-ceil
   charges one atom per intent under both readings, so per-intent-both-sides and
   charge-once-split collect the same pot. They separate only above the floor,
   where charge-once-split collects about half. EXP-FEE-P2 therefore runs a
   supra-atom grid as well as the dust grid.
6. **Zero-fee cells are non-negative, not positive.** With dropped carry and dust
   flow the pot is zero, so a washer's net is exactly zero rather than negative.
   EXP-FEE-W1 checks `net wash <= 0` everywhere and strict negativity in every
   terminal-ceil cell, and counts the zero-fee cells as the evasion evidence that
   EXP-FEE-D2 quantifies.
7. **`claim_credit` refuses when nothing whole has accrued.** The policy analysis
   does not say what a no-op credit claim does; the lab refuses with `no_credit`
   rather than succeeding with a zero payout, so the transition is never a
   silent no-op. This is a lab convention, not a decision.
8. **A negative weight is classified, not crashed.** It is unrepresentable in the
   kernel's `u64`; the lab classifies it as `invalid_payout_weights` and the
   shared fixtures avoid the case so no consumer is forced into an
   implementation-defined answer.

### Added stop and promotion rules

- Do not promote any payout candidate, redemption lot, credit layout, carry
  domain, close policy, fee side arm, or executor cap from these checks. The
  arms exist to be compared, and `kappa=0.004` and 60/15/25 remain unpromoted.
- Stop any payout policy that admits a reachable state holding claim value which
  no admissible transition can release. Solvency is not exit-liveness: the P1-A
  state satisfies the collateral invariant and is still dead.
- Stop any fee policy that credits the fee pot without an equal, simultaneous
  debit of a named payer's escrowed cash. A fee that increments revenue from thin
  air cannot be checked for conservation at all.
- Stop any fee policy whose carry domain instance can be closed for free; require
  the domain-lifetime charge to be `ceil(exact)` and cross-domain splitting to be
  weakly more expensive.
- Stop any redemption policy that floors silently. Refusing, lot-gating, or
  crediting the exact remainder are the admissible shapes; dropping the remainder
  is not.
- A differential fixture that fails on either side is a finding. Fixtures are
  never edited to match an implementation, and a minimized failure becomes a
  permanent named vector.
- Passing these fixtures shows that two implementations agree. It is not evidence
  for `P-SOLV-01` or `P-FEE-01` until the semantics they encode are frozen and
  both sides are the frozen ones.
