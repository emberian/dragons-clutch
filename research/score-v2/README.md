# ScoreV2-Q: quotient-risk candidate objective

Status: **RESEARCH MODEL / NO CONSENSUS CHANGE** (2026-08-22). This directory
changes no runtime score, account layout, market terms, deployment artifact,
fee, solver payment, or release claim. It uses only Python's standard library,
integer atoms, and deterministic exhaustive tests.

The independent safe-Rust core reproduction has since landed in
`crates/clutch-batch/src/score_v2.rs`; see
`docs/design/SCORE_V2_Q.md`. No SBF profile selects it yet.

## Decision

Replace ScoreV1's economic prefix with one owner-blind quantity:

```text
d_i       = directly crossed Egg atoms on outcome i
c(d)      = min_i d_i
rho(d)    = max_i d_i - min_i d_i
```

`rho(d)` is the **certified aggregate risk flow**. It is maximized. The word
"certified" matters: it is the least total model-free range risk compatible
with an owner-blind decomposition of the aggregate executed flow, not a claim
that every real counterparty's risk appetite is observable.

The proposed total selection order is:

1. maximize `rho(d)`;
2. minimize `c(d)`, the directly crossed complete-set layer;
3. minimize `virtual_split + virtual_merge`; and
4. prefer the lexicographically smaller full SHA-256 candidate digest.

Only item 1 is the economic objective. Items 2–4 select the cheaper canonical
representation or make the order total. They do not measure price quality,
personhood, welfare, or solver merit.

The verifier must apply this only to the active `outcome_count` prefix. All
inactive cells in the fixed `[u64; 16]` representation must be zero and must not
enter `min`/`max`; otherwise inactive zero padding turns a constant active
complete set into fake risk.

Do not carry these ScoreV1 fields into ScoreV2:

- `weighted_direct_volume`: it rewards complete-set flow and lets a
  candidate-controlled price change the volume objective;
- `limit_surplus_price_units`: it is useful telemetry but a raw complete-set
  shift can increase it, so it is not quotient-invariant;
- `distinct_owners`: a public key is not a person, and fragmentation increases
  the field; and
- owner-level self-overlap subtraction: key relabeling changes the admitted
  flow before the score is reached.

## Why `rho` is the conservative identity-free objective

Let `R(a) = max(a) - min(a)`. `R` is the same price-free range seminorm used by
the composite fee floor and by the protocol's model-free payoff geometry. It
has four exact properties needed here:

```text
R(a + k*1) = R(a)                 complete-set shift invariance
R(q*a)     = q*R(a), q >= 0       exact quantity scaling
R(a + b)  <= R(a) + R(b)          subadditivity
R(a)       = 0 iff a is constant  exact quotient kernel
```

Repeating a coordinate under a payoff-preserving state refinement also leaves
`R` unchanged. This matters because a range objective does not reward a market
merely for representing an identical payoff region with more cells.

Suppose the real transfers were vectors `a^1, ..., a^m`, but an identity-free
verifier can safely retain only their aggregate `d = sum_k a^k`. Then:

```text
sum_k R(a^k) >= R(sum_k a^k) = R(d).
```

The lower bound is attainable by the one-vector decomposition containing `d`
itself. Therefore `rho(d)` is exactly the minimum range-risk compatible with
the public aggregate. It does not depend on orders, owners, counterparties,
pairing decomposition, outcome labels, candidate price, or a rounding rule.

This conservatism has a real cost. In a binary market, independent trades of
`q` units of Egg 0 and `q` units of Egg 1 aggregate to `(q,q)`, so ScoreV2-Q
credits zero certified risk even when the participants were honest. Crediting
both trades requires trusting some grouping or identity boundary. ScoreV2-Q
refuses to turn that unobservable fact into public score.

## Complete-set wash result

For a direct complete-set wash `d = q*1`:

```text
rho(d) = 0
c(d)   = q
```

The economic objective ties the empty candidate at zero, and the canonical
tie-break prefers the empty candidate because `0 < q`. The wash cannot improve
selection by grinding the final digest.

At the binary midpoint, frozen ScoreV1 instead reports
`2*q*5000*5000 > 0`. This is the executable defect already retained in
`crates/clutch-batch/src/relation_v1_fee_tests.rs`.

## What “Sybil-neutral” can and cannot mean

ScoreV2-Q is **representation-neutral after admission**: it receives no owner
or order fields, so splitting the same admitted aggregate flow across orders or
keys cannot move `rho`.

It is not—and no key-only deterministic mechanism can be—proof that a trade
has independent beneficial owners. Consider two worlds with the exact same
accounts, public keys, signed orders, fills, prices, and settlement:

- in world H, two people control the two keys;
- in world S, one person controls both keys.

Every onchain input is byte-identical. Any deterministic score must return the
same value. A nonconstant wash such as `(q,0,...)` is therefore
indistinguishable from an honest transfer and can still earn `rho=q`.

There is also a pre-score V1 counterexample. Same-owner/same-outcome
normalization cancels a buy and sell under one key, but admits the same orders
when their owner tag is split across two keys. Removing `distinct_owners` alone
does not close that seam. A V2 relation must either:

- remove owner-dependent economic admission and rely on non-recoverable fees,
  capital, and transaction cost to make nonconstant wash negative; or
- name an external identity/stake credential and its trust assumptions.

The first route is compatible with permissionless keys. It requires a separate
proof that maker/executor allocations cannot recover the washer's fee. A score
cannot supply that proof.

## Why the existing state-contingent Gini is reported, not selected

The composite fee computes the price-weighted numerator

```text
G_num(d,p) = sum_{i<j} p_i*p_j*abs(d_i-d_j).
```

This is an excellent diagnostic and a legitimate fee component. It is
complete-set invariant, homogeneous, relabeling symmetric with prices, and
subadditive. It is not automatically a candidate objective:

- the candidate supplies `p`, so maximizing `G_num` also chooses prices;
- one wide binary crossing executable at every price is pulled toward the
  midpoint by `p*(S-p)`, whether or not that is the frozen price-quality rule;
- tail flow is suppressed, and flow on a zero-priced state has zero Gini; and
- the composite fee's range floor fixes the kernel, but fee rates and solver
  compensation are policy choices, not evidence of price quality.

The model exposes `price_weighted_gini_numerator` as telemetry and tests its
boundary behavior. It does not include it in `RiskObjectiveV2`.

This conclusion was checked against the existing claim and both executable
implementations, rather than inferred from the ScoreV1 defect alone:

- `docs/COMPETITIVE_POSITION.md`, which names the state-contingent Gini as a
  research claim;
- `docs/FEE_GEOMETRY.md` and `docs/research/RISK_SUMMED_POSITIONS.md`, which
  separate the price-weighted Gini seminorm from the price-free range norm;
- `crates/clutch-batch/src/relation_v1.rs::composite_fee_quote`, the checked
  `u128` runtime arithmetic; and
- `research/economics-admission/model.py`, the independent composite-fee model.

## Exact arithmetic envelope

With `2 <= n <= 16` and `d_i: u64`:

- `rho(d): u64` is one checked subtraction after comparisons;
- `c(d): u64` is comparisons only;
- `virtual_split + virtual_merge` is a checked `u64` addition;
- there is no multiplication, division, floating point, or rounding boundary;
- the final tie uses the already required full 32-byte candidate digest.

The executable model refuses booleans, negative values, width drift, non-u64
atoms, churn overflow, and non-32-byte digests.

It separately reconstructs both relation identities
`d_i = B_i - sigma = E_i - mu`. Adding the same complete-set quantity to every
`B_i` and `sigma`, or to every `E_i` and `mu`, leaves `d` byte-exact. The
selection key then prefers the lower-churn representation without calling that
preference additional risk.

## Price quality and solver compensation stay separate

ScoreV2-Q answers only: **which valid submitted candidate certifies the most
aggregate noncash risk flow?** It does not prove that the candidate's price is
informative or fair. Before runtime promotion, a separate versioned price rule
must decide ties or certify price quality without reintroducing a
candidate-controlled risk weight. Possibilities belong in a separate lane:
maximum executable surplus certificates, a canonical clearing interval rule,
or an externally bound reference-price policy.

Likewise, solver reimbursement should pay bounded verified work from a prepaid
work budget. It must not be a percentage of `rho`, owner count, fee pot, or
reported volume. Any maker/executor rebate needs its own wash-recovery bound.

## Migration plan

1. **Landed:** independently reproduce this model in safe, allocation-free Rust
   with `u64` fields, full-width digest, and frozen differential vectors.
2. **Landed at the core boundary:** specify the V2 input as
   `d_i = B_i - sigma = E_i - mu` and check both sides exactly. Next, prove
   that both the monolithic and streamed verifiers derive identical values.
3. Decide the owner-normalization policy. Do not claim Sybil neutrality while
   the feasible set itself changes under key relabeling.
4. Freeze a separate price-quality/tie policy and a separate solver-payment
   policy; adversarially compose all three.
5. Add V1/V2 differential fixtures for empty, complete-set wash, tails,
   complement baskets, virtual conversion, portfolios, AON, order/key
   fragmentation, maximum integers, and digest ties.
6. Introduce new version/account tags. Never reinterpret persisted
   `ScoreV1` bytes as `ScoreV2`.
7. Run monolithic/streaming/Direct-V3 agreement, SBF compute measurements,
   candidate-window replay tests, and fee/rebate wash cycles before enabling a
   public Realm profile.
8. Retain ScoreV1 only as a clearly named experimental legacy profile.

Promotion is blocked on steps 3–7. The runtime is intentionally unchanged.

## Reproduce

```sh
python3 -m unittest discover -s research/score-v2 -p 'test_*.py' -v
python3 research/score-v2/run_lab.py
```

The suite exercises quotient shifts, complements, complete-set wash, tails,
payoff-preserving refinement, inactive zero padding, partial-fill splitting,
virtual complete-set translations, key and order fragmentation, the
owner-normalization counterexample, exact scaling, the u64 boundary,
deterministic ties, and the identity impossibility.
