# Fee geometry on the state simplex

## 1. Objective

The venue should charge for transferring contingent risk, not for moving claimant
principal, redeeming a correct claim, or carrying a risk-free complete set. A flat
percentage of every leg ignores the fact that Eggs are a basis of one common state
space. The binary candidate

```text
q * p * (1 - p)
```

has an exact generalization to any payoff vector on the simplex — provably the
unique one, given two axioms, and provably not the model-free risk norm. Both
facts are proved in
[research/RISK_SUMMED_POSITIONS.md](research/RISK_SUMMED_POSITIONS.md) §3 and
stated below.

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
intermediate width were to be frozen before implementation. That ordering has
already been violated: `dispersion_fee_step`
(`programs/solana-layout/src/portfolio_settlement.rs`) implements §4 in checked
`u128`, and none of the five bounds is frozen. The checked arithmetic makes the
implementation safe but changes the claim — its domain is "whatever does not
overflow", not an audited envelope. Freezing the five bounds is still owed.

### Exact relation to the risk quotient (proved, not assumed)

[research/RISK_SUMMED_POSITIONS.md](research/RISK_SUMMED_POSITIONS.md) §3 pins
this base exactly, in both directions.

- **`G` is not the quotient (range) norm — refuted (Proposition 10).** For the
  model-free range `R(a) = max_i a_i - min_i a_i`,

  ```text
  G(a,p) <= R(a)/4
  ```

  with equality only when the implied measure puts mass `1/2` on argmax and
  `1/2` on argmin outcomes, and the bound is the exact envelope:
  `sup_p G(a,p) = R(a)/4`. The single-Egg case displays the whole gap: the
  ratio `2p(1-p)` tends to zero at extreme prices while the model-free at-risk
  capital stays fixed. `G` measures expected payoff variability under the
  market's own implied measure; it does not measure model-free risk moved.
- **`G` is characterized, not merely constructed (Propositions 11-12).** It is
  the *unique* positively 1-homogeneous functional that reduces to `q(1-q)` on
  digitals and is additive over layer-cake decompositions; and within the
  pairwise family `sum p_i p_j phi(a_i - a_j)`, relabeling symmetry plus
  homogeneity force `phi(t) = c|t|`. Accept the binary calibration and layer
  additivity, and `G` is derived — uniquely. What is not derivable is the
  binary calibration itself: the price-free quotient-norm base `kappa' * R(a)`
  satisfies every axiom in this section and is a mandatory control arm (§6).

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
is `kappa=0.004`. For one Egg at `p=0.5`, this is exactly 20 basis points of
cash consideration as a rational — and only at size. Under the terminal-ceil
close the minimum charge per fee-bearing intent is one atom, which dominates
small intents: the laboratory's own fee vector (`FEE-001`,
`research/economics/fixtures.py`) records a 1-atom fee on 1 atom of
consideration — 10,000 basis points on the smallest fill. This number is an
experiment, not a natural constant.

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
- including a complete-set component in quantity/reservation inconsistently;
- clearing risk transfer entirely on zero-priced outcomes. At boundary prices
  the kernel of the dispersion base is `span(1) ⊕ R^{Z(p)}`
  ([research/RISK_SUMMED_POSITIONS.md](research/RISK_SUMMED_POSITIONS.md)
  Proposition 9), strictly larger than the risk quotient, so a transfer
  supported on the zero-priced outcomes `Z(p)` is feeless however large its
  model-free range. The same kernel invariance §3 presents as the fee's
  central virtue degenerates into this evasion channel at extreme prices.
  Whether the batch relation can clear fills at price zero — and what the
  one-tick floor bounds the hole at — is a named zero-price laundering
  falsifier the laboratory must exercise before any base is selected.

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
5. the same bases with several maker/executor/treasury allocations;
6. the price-free quotient-norm base `kappa' * R(a)`
   ([research/RISK_SUMMED_POSITIONS.md](research/RISK_SUMMED_POSITIONS.md)
   §3.4), with incidence measured by implied probability — the two bases
   differ most exactly where the burden-by-probability axis below already
   demands data.

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
strong advantage over naïve per-token fees. Formal verification of that property
remains open. What exists today: bounded exhaustive Python
(`research/economics/experiments.py` `exp_fee_g1`, `n <= 5`, `S <= 12`) plus the
admission lab's bounded sweeps, and three unit tests in
`portfolio_settlement.rs` — a file no runtime path calls. Nothing is closed in
Verus or Rocq; Rocq currently contains zero theorems.

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
