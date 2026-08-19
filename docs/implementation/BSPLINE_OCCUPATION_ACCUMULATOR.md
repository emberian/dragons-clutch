# Native B-spline occupation accumulator

Status: **implemented pure crate / not integrated**, 2026-08-19.

Semantic owner: `crates/clutch-bspline-accumulator`. Its only dependency is
the exact native point evaluator `clutch-bspline`. It is safe Rust, `no_std`,
allocation-free, fixed-width, float-free, and contains no Solana, account,
clock, source adapter, hashing, custody, or resolution authority.

## 1. Frozen statistic

For equal-duration canonical buckets, an authenticated caller supplies either
one canonical integer point `x_j` or an explicit gap. Each accepted point is
evaluated by the frozen native basis evaluator:

```text
W_j = W_D(x_j),       sum_i W_j[i] = D.
M_i = sum accepted j W_j[i].
```

The summary retains:

- evaluator and occupation semantic versions;
- an opaque nonzero basis-spec digest and the exact validated `BasisSpec`;
- an opaque nonzero canonical-grid identity and nonzero equal bucket duration;
- contiguous `[start_bucket,end_bucket_exclusive)`;
- total sample count and accepted coverage count;
- sixteen `u128` masses with canonical zero padding.

The digest and grid identity are binding values, not authentication. The crate
cannot establish who computed them or whether an external artifact is genuine.
Keeping the exact spec and bucket duration in the domain ensures two summaries
cannot combine merely because a caller reused an opaque identity.

## 2. Algebra and gaps

Two non-empty summaries combine only when domains are equal and the left end
is exactly the right start. Counts and every mass add with checked arithmetic.
The domain-bound empty summary is the identity. For adjacent `a,b,c`, ordinary
integer associativity gives:

```text
(a combine b) combine c = a combine (b combine c).
```

Every accepted singleton has mass sum `D`; every missing singleton has zero
mass and increments only `sample_count`. Therefore, by induction over combine:

```text
sum_i M_i = D * coverage_count
gap_count = sample_count - coverage_count.
```

Validation recomputes both the span/count relation and partition sum. Missing
buckets are never silently dropped: both finalization modes refuse any gap.
An external window plane may choose another future policy, but it must name a
new semantic transition rather than reinterpret this output.

## 3. Finalization is explicit

`ExactOnly` succeeds only if each `M_i / coverage_count` is integral at the
original denominator `D`. Otherwise it returns `InexactAverage`.

`LargestRemainderV1` is separately named. It floors each exact average and
awards the remaining atoms to the largest `M_i mod coverage_count`, with the
lowest outcome index winning exact ties. The output is a fixed-width
`FinalWeights { active_len, denominator, weights[16] }` that revalidates exact
sum `D`, component bounds, and zero padding.

This is occupation of the canonical **quantized native basis**, not the
one-final-quantization exact-rational control arm in
`research/bspline-window-semantics`. They can differ by payout atoms and must
not share a statistic identifier.

## 4. Solvency inheritance

Every accepted point sums to `D`; the accumulator invariant and either valid
finalizer preserve sum `D`. Thus final weights remain a nonnegative simplex.
For outstanding supplies `T_i`:

```text
sum_i (w_i/D) T_i <= max_i T_i.
```

The central maximum-supply collateral theorem therefore survives occupation
finalization. This does not solve per-wallet fractional redemption; the frozen
lot/remainder-credit rule remains a separate requirement.

## 5. Cost and measurements

Logical worst-case work, independent of host timing:

| operation | bound |
|---|---:|
| accepted bucket | one degree-0..3 native evaluation; copy 16 fixed weights |
| missing bucket | no basis evaluation; zero masses |
| combine | two validations plus 16 checked mass additions |
| exact finalize | 16 divisions/remainders and output validation |
| largest-remainder finalize | above plus at most 15 bounded winner scans over 16 entries |

On `aarch64-apple-darwin`, `rustc 1.98.0-nightly (91fe22da8 2026-06-21)`, the
measurement example reports the following in release compilation:

| type | bytes |
|---|---:|
| `BasisSpec` | 288 |
| `BasisDomain` | 368 |
| `Summary` | 656 |
| `FinalWeights` | 144 |

One non-normative 100,000-iteration release run measured 116.6 ms for cubic
accepted-singleton construction, 13.7 ms for two-singleton combine, and 5.15
ms for largest-remainder finalization. These are host totals, not per-Solana-
instruction estimates or a compute-budget claim.

Run the non-normative host timing/size probe with:

```sh
cargo run --release \
  --manifest-path crates/clutch-bspline-accumulator/Cargo.toml \
  --example measure
```

Wall-clock timings vary by machine and compiler and are not protocol evidence.
The fixed operation bounds above are the consensus-relevant cost statement.

## 6. Tests and promotion boundary

The crate tests:

- degrees zero through three and endpoint/clamp behavior;
- partition mass for every accepted span;
- associativity and all small split points;
- boundary joins, overlaps, reverse order, and unrepresented holes;
- explicit gaps and no-coverage distinction;
- exact versus largest-remainder finalization and tie breaking;
- digest and exact-spec domain separation;
- basis arithmetic and maximum-bucket overflow refusals;
- corrupt mass, padding, count, range, version, and output mutants; and
- point evaluator/output shape agreement.

Before adapter/layout/SBF promotion, a separate lane must define and bind the
source/window identity, authenticate the bound grid identity and duration, set
the confidence policy, and define the archive-replay or work-account lifecycle,
maximum bucket count, serialization, rent, compute budget, and prepaid
liveness. This crate deliberately supplies none of those authorities.
