# Fee geometry on the state simplex

## 1. Objective

The venue should charge for transferring contingent risk, not for moving claimant
principal, redeeming a correct claim, or carrying a risk-free complete set. A flat
percentage of every leg ignores the fact that Eggs are a basis of one common state
space. The binary candidate

```text
q * p * (1 - p)
```

has a useful exact generalization to any payoff vector on the simplex.

This document defines an experimental fee base. It is not canonical until it beats
flat-notional and per-Egg controls under the economic laboratory.

## 2. State-contingent dispersion

Let integer prices `p_i` satisfy:

```text
p_i >= 0
sum_i p_i = S
```

Let one transferred portfolio lot have integer terminal payoffs `a_i`. Define its
state-contingent dispersion numerator:

```text
G_num(a,p) = sum_{i<j} p_i * p_j * abs(a_i - a_j)
```

and its scaled value:

```text
G(a,p) = G_num(a,p) / S^2
```

This is one half of the expected absolute payoff difference between two
independently drawn states under the implied distribution. Consensus arithmetic
retains the numerator and applies the fee scale with one checked final rounding.
No floating point or pairwise intermediate truncation is permitted.

For a quantity `q` of one Egg `k`, the payoff vector is `a_k=q` and every other
component is zero:

```text
G(q*e_k,p) = q * p_k * (S - p_k) / S^2
```

which is exactly `q*p*(1-p)` in normalized units.

## 3. Why this geometry fits Clutch

### Complete-set invariance

Adding `c` to every payoff state is adding `c` complete Clutches, a risk-free
collateral leg. Since pairwise differences do not change:

```text
G(a + c*1, p) = G(a,p)
```

A constant payoff vector has zero dispersion. Split, merge, and risk-free basket
transfer therefore need no percentage fee.

### Relabeling symmetry

Permuting outcomes and the matching prices leaves `G` unchanged. The policy does
not privilege YES, NO, tails, or a human label.

### Homogeneity

For nonnegative integer `q`:

```text
G(q*a,p) = q*G(a,p)
```

With a persistent fractional carry, splitting the same proportional intent cannot
erase its fee.

### Diversification-aware subadditivity

Pairwise absolute differences obey the triangle inequality:

```text
G(a+b,p) <= G(a,p) + G(b,p)
```

An atomic portfolio is charged for its net state-contingent shape rather than the
sum of artificial leg labels. This rewards expressing the actual risk transferred
and makes adding an offsetting complete-set component neutral.

### Cheap exact verification

At `MAX_OUTCOMES=16`, at most 120 unordered pairs contribute. Fixed nested loops,
checked wide intermediates, and one division are practical to verify and benchmark.
The exact maximum coefficient, price scale, lot count, fee coefficient, and
intermediate width must be frozen before implementation.

## 4. Fee equation

Candidate policy:

```text
fee_num = kappa_num * lots * G_num(a,p) + prior_carry
fee     = floor(fee_num / (kappa_den * S^2))
carry   = fee_num mod   (kappa_den * S^2)
```

The carry belongs to the same Position/policy domain and prevents economically
equivalent repeated small fills from rounding to zero. Fee is paid in Realm
collateral on top of buy consideration or withheld from sell proceeds. It never
comes from the Hoard.

The initial comparative arm corresponding to the previous single-Egg hypothesis
is `kappa=0.004`. For one Egg at `p=0.5`, this is 20 basis points of cash
consideration. This number is an experiment, not a natural constant.

## 5. Portfolio semantics that must be frozen

The fee applies to the exact net payoff vector transferred between counterparties
by one atomic fill. It does not inspect a wallet's existing holdings or claim to
measure profit, utility, or risk appetite. Cost basis is unknowable for freely
transferable Eggs.

Candidate construction must prevent fee laundering through:

- reporting a false net vector while settling different legs;
- pairing unrelated counterparties into a synthetic net solely to reduce fees;
- self-crossing or Sybil loops that recover maker/executor rebates;
- resetting fractional carry across Positions, Epochs, or order fragmentation;
- separately rounding vector components or page fragments;
- including a complete-set component in quantity/reservation inconsistently.

The simplest V1 rule is to compute the fee per filled signed intent, using the
canonical vector committed by that intent. A more efficient netting rule across
several intents needs a separately proven attribution mechanism; it is not implied
by the seminorm.

## 6. Economic controls

Compare at least:

1. zero fee;
2. flat cash-consideration basis points;
3. per-Egg `q*p_i*(1-p_i)` charged leg by leg;
4. atomic portfolio `G(a,p)`;
5. the same bases with several maker/executor/treasury allocations.

Measure:

- total and net protocol contribution;
- all-in cost for single Eggs and standard payoff portfolios;
- depth, participation, fill rate, and route leakage;
- complete-set and simplex coherence;
- whether atomic portfolios gain legitimate price improvement or merely enable
  fee avoidance;
- fee burden by implied probability, outcome count, and payoff roughness;
- wash-cycle loss after all rebates and network costs;
- sensitivity to partition refinement: the same economic payoff should not become
  arbitrarily cheaper or dearer merely because a Template uses more bins.

The partition-refinement test is especially important. `G` is invariant to
splitting a state into identical-payoff subcells when their prices add exactly, a
strong advantage over naïve per-token fees. Verify that property formally.

## 7. Promotion criteria

Promote the simplex-dispersion fee only if:

- Verus and Rocq close translation, homogeneity, complete-set invariance, bounded
  arithmetic, carry conservation, and partition-refinement invariance;
- adversarial simulation finds no cheaper equivalent encoding or fragmentation;
- user costs are no worse than the lowest sustainable control on the primary
  payoff families;
- the protocol contribution remains positive under conservative route elasticity;
- executor and maker rewards never depend on future volume for liveness; and
- the explanation remains comprehensible in the signing UI.

Otherwise use the simplest control that meets the product floors. Novelty is not a
license to impose an opaque tax.
