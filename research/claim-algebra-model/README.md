# Bounded claim-algebra model

Status: **MODEL**, offline only. This crate changes no kernel, account layout,
market terms, deployment artifact, or release claim. It is an original,
dependency-free executable model of the smallest payoff language needed to
test the generality claim in
[`docs/design/continuous-claims`](../../docs/design/continuous-claims/README.md).

## Semantic owner and dependency direction

This model owns only compilation of human-sized payoff shapes into one exact,
fixed-capacity coefficient vector. It does not own evidence, resolution,
issuance, pricing, matching, custody, or token representation. It has no
dependencies. The intended future direction is:

```text
immutable terms -> claim compiler -> exact coefficients
sealed evidence -> basis evaluator -> exact simplex weights
coefficients x weights -> terminal payoff
```

The compiler may eventually become a sibling pure crate consumed by a client,
terms builder, and verifier. `clutch-kernel` must not depend on it: the kernel
needs only validated payout weights and supplies. The Solana adapter must not
run a general expression evaluator.

Toolchain posture matches the repository's Rust 2021, safe-Rust, `no_std`,
`no_alloc` prototype discipline. The crate is AGPL-3.0-or-later and imports no
code or assets from another project.

[`ONE_HOT_VS_DERIVED.md`](ONE_HOT_VS_DERIVED.md) evaluates the especially
useful V1 option in which externally composable primitive Eggs remain one-hot
and every shaped product is an integer portfolio over them.
[`ARCHITECTURE_REVIEW.md`](ARCHITECTURE_REVIEW.md) records the full inclusion,
liability, approximation, SVM-cost, and promotion analysis.

## Frozen language boundary

`PayoffSpecV1` has seven bounded constructors:

- constant;
- one categorical basis claim;
- a contiguous hard range of basis claims;
- a triangular curve over an exact knot grid;
- a capped linear curve over an exact knot grid;
- an exact sampled coefficient table; and
- a Gaussian approximation compiled by a fixed, checked interval algorithm.

There is no loop bytecode, call, branch, user-selected iteration count,
recursion, storage read, oracle read, floating point, or arbitrary program.
`ExactSamples` is intentionally the algebraic escape hatch: at an admitted
finite basis size it represents every nonnegative bounded vector, including
samples of piecewise-polynomial and other kernel families. Its semantics are
the exact vector, not an unverifiable analytic label.

The Gaussian constructor is more specific. At every knot it encloses
`exp(-distance^2/(2 sigma^2))` using a fixed 32-term positive Taylor interval
for the reciprocal exponential. Beyond eight exponent units it emits zero;
`exp(-8) < 1/2048` follows already from the integer lower bound
`sum(floor(8^k/k!), k=0..10) = 2429 > 2048`. The returned certificate adds:

1. the checked knot-enclosure and integer-quantization error; and
2. the standard linear-interpolation error `height * gap^2 / (8 sigma^2)`,
   using `|Gaussian''| <= height / sigma^2`.

Thus the compiler never asks SVM consensus to evaluate `exp`, `log`, `erf`, or
floating point. The chain evaluates the already-frozen coefficient vector and
the sparse basis weights.

## Liability and exactness

For coefficients `a_i >= 0` and settlement weights `w_i >= 0` with
`sum(w_i) = D`, one basket lot pays

```text
P = sum_i a_i w_i / D <= max_i(a_i).
```

The model computes that bound directly. It also computes the least lot count
that makes redemption integral for **every** weight vector in the full integer
simplex:

```text
L = lcm_i D / gcd(D, |a_i - a_0|).
```

Because `sum(w_i)=D`, `L * sum(a_i w_i)` is divisible by `D` exactly when all
`L(a_i-a_0)` are. This exposes the fractional-redemption constraint instead of
silently flooring it. A particular basis may admit a smaller lot after a
separate reachable-weight proof; V1 reports the conservative full-simplex lot.

## Run

```sh
cargo test --manifest-path research/claim-algebra-model/Cargo.toml
cargo clippy --manifest-path research/claim-algebra-model/Cargo.toml --all-targets -- -D warnings
RUSTDOCFLAGS='-D warnings' cargo doc --manifest-path research/claim-algebra-model/Cargo.toml --no-deps
```

The exhaustive tests enumerate small integer simplexes and coefficient cubes,
checking partition validity, nonnegativity, the maximum-liability theorem,
the universal lot formula and its minimality, padding refusals, curve
constructors, and the Gaussian compiler's symmetry and monotonicity.
