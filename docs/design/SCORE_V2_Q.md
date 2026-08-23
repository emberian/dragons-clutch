# ScoreV2-Q: quotient-risk candidate ranking

Status: **EXACT-PRICE-GATED GENERAL V2 SOURCE PATH / NON-PRODUCTION PROFILE
ONLY** (2026-08-23). The safe, `no_std`, allocation-free arithmetic lives
in the `clutch-batch::score_v2` production kernel. The owner-blind coefficient
relation and its fixed-domain best-valid-submitted fold live in
`relation_v2.rs` and `relation_v2_ranking.rs`; and
`crates/clutch-general-v2-runtime` compares the resulting checked certificates
while composing the sealed General feed and exact quantized degree-two/three
price certificate. The isolated General SBF handler now authenticates that
certificate before it can create resumable work, and repeats the gate for its
empty-book completion projection. Production profiles remain disabled, and no
build, committing execution, deployed program, fee, bond, or solver-payment
claim follows from the source checkpoint.

## 1. Question answered

ScoreV2-Q answers one bounded question:

> Which valid submitted candidate certifies the most aggregate noncash risk
> flow under an owner-blind normalization contract?

It does not decide whether a candidate is valid, whether its prices are good,
whether two public keys have independent controllers, or how a solver should be
paid.

For every active outcome:

```text
B_i = aggregate executed buy atoms
E_i = aggregate executed sell atoms
sigma = virtual complete sets created
mu = virtual complete sets destroyed
d_i = B_i - sigma = E_i - mu
```

The economic objective is the exact range seminorm:

```text
rho(d) = max_i(d_i) - min_i(d_i).
```

It has no price, owner count, multiplication, division, float, or rounding
boundary.

## 2. Why range is the identity-free lower bound

For `R(a) = max(a) - min(a)`:

```text
R(a + c*1) = R(a)
R(q*a) = q*R(a), q >= 0
R(a + b) <= R(a) + R(b)
R(a) = 0 iff a is constant
```

If the real transfers were `a^1, ..., a^m` but the verifier retains only the
owner-blind aggregate `d = sum_k a^k`, subadditivity gives:

```text
sum_k R(a^k) >= R(d).
```

The one-vector decomposition containing `d` attains the bound in the
nonnegative payoff cone. Therefore `rho(d)` is the least range-risk consistent
with the aggregate, without trusting an order, pairing, or owner grouping.

The conservatism is deliberate. Independent binary trades of `q` units of Egg
0 and Egg 1 aggregate to `(q,q)` and receive zero certified risk, even if the
participants were honest. Crediting both requires an identity or grouping
assumption the public aggregate does not prove.

## 3. Frozen total order

`ScoreV2::total_order` compares:

1. `certified_risk_flow_atoms = rho(d)`, descending;
2. `cash_equivalent_direct_flow_atoms = min_i(d_i)`, ascending;
3. `virtual_churn_atoms = sigma + mu`, ascending; and
4. the full candidate digest, lexicographically ascending.

Only item 1 is the economic objective. Item 2 selects the min-zero direct-flow
representative, so an empty candidate beats a pure directly crossed complete
set before digest. Item 3 selects the lower-conversion representation. Item 4
makes the order total.

Raw limit surplus, distinct public-key count, state prices, fee amount, bonds,
and solver identity are absent.

## 4. Representation-neutral is not person-neutral

The precise claim is **representation-neutral after owner-blind admission**.
Splitting the same admitted aggregate across orders or relabeling its public
keys cannot alter ScoreV2-Q because neither appears in the score input.

It is impossible to claim person-neutrality from public keys alone. Two worlds
can have byte-identical accounts, keys, signatures, orders, fills, and
settlement while:

- two people control the keys in one world; and
- one person controls both keys in the other.

Every deterministic onchain function must return the same score in both.
Nonconstant common-control wash is therefore indistinguishable from honest
trade at this boundary.

V1 also has a pre-score counterexample: its owner-tagged self-cross policies
can cancel or refuse one-key overlap while admitting the same economic orders
after key fragmentation. Removing `distinct_owners` from the score does not
repair a feasible set that already changed.

The core API makes this boundary explicit through `NormalizationPolicyV2`.
`OwnerBlindAggregate` is admitted. The three owner-tagged V1 families are named
and refuse with `NormalizationNotRepresentationNeutral`; a caller cannot omit
or silently translate the policy.

## 5. Candidate-delta validation

`CandidateDeltaV2` carries the active width, buy flow, sell flow, claimed direct
flow, virtual conversion, digest, and normalization contract. The core refuses:

- an active width outside `2..=16`;
- owner-tag-dependent normalization;
- nonzero inactive padding in any flow array;
- simultaneous nonzero `sigma` and `mu`;
- `sigma > B_i` or `mu > E_i`;
- checked-add overflow in either conservation side;
- `B_i + mu != E_i + sigma`;
- disagreement between the two derived direct flows; and
- a claimed direct flow or total score that differs from recomputation.

Padding is excluded from `min` and `max`. This is consensus-critical: including
the inactive zeros of a `[u64; 16]` array would turn a constant active complete
set into fake risk.

`CheckedCandidateScoreV2` has private fields and retains the validated domain,
the exact canonical `CandidateDeltaV2`, the independently derived direct flow,
and the total score key. Cross-domain comparison refuses. The bounded fold
starts from a real checked submission, retains the earlier submission on exact
equality, and counts checked score submissions. The RelationV2 ranking wrapper
admits to that fold only after full candidate reverification.

## 6. State-contingent Gini, price quality, and solver payment

The existing state-contingent Gini

```text
sum_{i<j} p_i*p_j*abs(a_i-a_j)
```

remains legitimate fee geometry and useful telemetry. It is not ScoreV2-Q:
candidate-controlled prices alter it, midpoint flow dominates tail flow, and a
zero-priced state lies in its kernel. The composite fee's price-free range
floor fixes the fee kernel but does not turn a fee rate into a price-quality
rule.

Price quality requires its own versioned policy or certificate. Solver
reimbursement should pay bounded verified work from a prepaid work budget.
Neither belongs in the score, and no maker/executor rebate is safe without a
separate wash-recovery bound.

The quantized RelationV2 successor now makes the distinction executable. Its
policy identity hashes the original owner-blind RelationV2 arithmetic policy
plus the exact finite atom-mixture schema, production evaluator semantics,
degree-two/three restriction, and payout-denominator price scale. Every public
ranked candidate must first carry a private admission minted from that exact
certificate. The semantic price and successor policy enter candidate identity;
the nonunique atom witness body does not. This proves coherence with one
selected production payout image, not fair value, welfare, liquidity, or an
oracle statement.

## 7. Remaining promotion gates

Before a production SBF profile selects ScoreV2-Q:

1. adopt the staged 17-account nonempty Work tuple in the shared account-meta
   and capability owners, then retain its exact-price fact through streamed
   completion;
2. freeze a separate price-quality rule; quantized measure coherence is not a
   price-quality or welfare theorem;
3. finish successor-policy projection across every selected-artifact and
   settlement transition; never reinterpret ScoreV1;
4. make monolithic, streamed, and Direct-V3 derivations agree on every frozen
   vector and refusal;
5. compose ScoreV2-Q with fee, bond, and reward policies under adversarial wash
   cycles; and
6. measure SBF compute and account effects before enabling a Realm profile.

The generic two-window candidate lifecycle already binds score policy by
content identity; this score change does not require another timing state
machine.

## 8. Reproduce

Frozen vectors are in
`crates/clutch-batch/fixtures/score_v2_q_vectors.txt`. The Rust tests cover
quotient shifts, complements, relabeling, exact scaling, payoff-preserving
refinement, inactive padding, virtual translations, complete-set wash, u64
bounds, overflow refusals, normalization policies, score lies, every total
order direction, all sixteen active outcomes, cross-domain refusal, and
state-preserving selection failures.

```sh
cargo +1.93.1 test --manifest-path crates/clutch-batch/Cargo.toml --locked
cargo +1.93.1 clippy --manifest-path crates/clutch-batch/Cargo.toml \
  --locked --all-targets -- -D warnings
```

The independent Python falsifier remains in `research/score-v2/`; agreement
between two implementations is evidence, not a formal-verification claim.
