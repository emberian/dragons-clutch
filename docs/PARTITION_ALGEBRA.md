# State partitions and bounded payoff algebra

## 1. Primitive

Let `Omega` be the set of possible authenticated terminal histories admitted by a
Market's frozen source and time semantics. A `StatePartition` is a finite ordered
family:

```text
P = [P_0, ..., P_{n-1}]
```

such that:

```text
exhaustive: for every omega in Omega, some P_i contains omega
disjoint:   no omega belongs to two distinct P_i
bounded:    2 <= n <= MAX_OUTCOMES
```

Normal resolution selects exactly one basis outcome `e_i`. One complete Clutch
contains one unit of every `e_i` and is backed by one collateral atom.

This makes the Eggs a basis for nonnegative bounded payoffs over the partition. A
portfolio `a = [a_0,...,a_{n-1}]` pays `a_i` collateral atoms when state `P_i`
occurs, subject to holding that many Egg units and exact atom divisibility.

The protocol need not create a separate token for “put,” “range,” or “straddle.”
Those are portfolio vectors over the basis.

## 2. Prices live on a simplex

Let `pi_i` be the collateral price of one Egg `i`. Frictionless complete-set
arbitrage implies:

```text
pi_i >= 0
sum_i pi_i = 1
```

The reference auction enforces the scaled integer version of this relation at its
clearing point. External venues may deviate temporarily; anyone can materialize,
split, acquire, merge, or route when the economics cover external fees.

The vector `pi` is an implied distribution only under the usual market assumptions.
It is not a claim that participants are calibrated, independent, or rational.

## 3. Compiler pipeline

```text
SourceProgram
  -> ObservationWindow
  -> StatisticProgram
  -> StateValue or conservative interval
  -> ordered boundaries / categorical mapping
  -> StatePartition
  -> canonical outcome descriptors and mints
```

Every stage is content-addressed and versioned. A Market stores the final compiler
artifact digest and enough canonical parameters for independent recomputation.

### SourceProgram

Identifies the exact source adapter, program/account/deployment identity, subject,
quote, orientation, decimal normalization, quorum, confidence, dispersion, and
coverage policy.

### ObservationWindow

Freezes origin, start/end, grid, permitted repair interval, boundary semantics,
and accepted clock/source-time relation.

### StatisticProgram

V1 is an audited closed enum, not arbitrary bytecode:

- terminal interval;
- TWAP interval;
- sampled minimum or maximum;
- conservative maximum drawdown interval;
- bounded realized-variance statistic without square root;
- relative terminal or TWAP return over synchronized feeds;
- sustained threshold for a registered threshold/automaton;
- exact lifecycle or protocol-state enum.

Each variant declares its required associative summary, units, width bounds,
missing-data behavior, and cost class.

### PartitionProgram

Candidate forms:

- ordered numeric half-open bins covering the full representable domain;
- closed finite categorical mapping with an explicit unknown source state;
- bounded product of two small partitions where `n_a * n_b <= MAX_OUTCOMES`.

The compiler proves or refuses exhaustiveness, disjointness, ordering, unit
compatibility, arithmetic bounds, and outcome-count limits.

## 4. Conservative interval resolution

Authenticated observations may produce an interval `[x_low,x_high]`, not a point.
Normal one-hot resolution is valid only if the entire interval belongs to one
partition cell. If it intersects several cells, the Market enters its frozen
ambiguity procedure.

This prevents a midpoint convention from laundering oracle confidence or missing
path information into false precision.

The ambiguity procedure may eventually produce a fractional payout over compatible
cells, but that is a separate finite vector in the solvency theorem. It must not be
confused with normal basis selection.

## 5. Payoff intents

A `PayoffVector` is a fixed `[u64; MAX_OUTCOMES]` plus active length and scale. It
may describe:

- a desired portfolio acquisition;
- a proportional portfolio sale;
- a client-side payoff visualization;
- an atomic auction basket;
- a read-only mark under a simplex price vector.

For scaled prices `p_i` with `sum p_i = PRICE_SCALE`, the collateral value of a
portfolio unit is the checked dot product:

```text
value(a,p) = floor(sum_i a_i * p_i / PRICE_SCALE)
```

The compiler and auction must specify whether rounding occurs once after the dot
product or per component. V1 should round once; per-leg rounding makes equivalent
portfolio decompositions economically unequal.

A client may normalize a human payoff diagram into a vector, but the signed intent
contains only exact canonical integers and a terms digest.

## 6. Bounds and expressiveness

The finite basis is intentionally not a general programming language. It cannot
represent every continuous payoff exactly. Approximation quality is explicit:

- more bins improve payoff resolution but increase mint/rent/account costs;
- two-dimensional partitions consume the outcome limit multiplicatively;
- fine path predicates require more authenticated information;
- materialized basket tokens would add new vault/mint surfaces and are not V1.

The protocol should publish approximation error for standard payoff shapes over a
given partition. A user can inspect where a discrete crash hedge or call differs
from the desired continuous payoff.

## 7. Template, Instance, Series

### Template

Immutable compiler program and human-readable terms digest. It contains no time-
specific liability.

### Instance

Binds Template, exact window, collateral Realm, collateral cap, liveness booking,
fee policy, outcome mints, and lifecycle. This is the solvency unit.

### Series

An optional prepaid schedule for repeated Instances. It freezes allowed window
derivation and maximum obligations. Anyone may instantiate the next eligible
window when its capital is already booked. It creates no automatic or unbounded
future debt.

## 8. Proof obligations

**Lean should cover the following** — Lean is the proof substrate of record
([adr/0005-lean-proof-substrate-of-record.md](adr/0005-lean-proof-substrate-of-record.md),
adopted 2026-08-20 —
[decisions/ADOPTED_2026-08-20.md](decisions/ADOPTED_2026-08-20.md) item 2,
superseding ADR-0003's Rocq-and-Verus assignment). The obligations keep their
content; only the substrate moved. Verus is retained solely for
checked-Rust-subset results verifying actual executable bodies, and the Rocq
shadow role is retired:

- partition exhaustiveness and disjointness for every compiled preset;
- deterministic mapping from exact statistic/interval to compatible cells;
- complete-set basis solvency;
- portfolio payoff bounded by held Eggs and Hoard;
- dot-product width and one-time rounding;
- Template/Instance digest stability;
- refusal of unit, scale, boundary, or unsupported-program mismatch;
- failure vectors drawn only from the finite admitted payout set.

## 9. Future composition

Gnosis Conditional Tokens permit nested conditions and outcome collections. A
future Dragon version might support compositional bases or materialized portfolio
claims. V1 should preserve canonical identifiers and algebraic terminology that do
not foreclose this, while declining the exponential state/mint surface now.

