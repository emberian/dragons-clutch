# Accumulator Verus shadow

`accumulator.rs` is the proof-facing semantic shadow for the executable
prototype in `crates/clutch-accumulator/src/lib.rs`. It deliberately contains
no Solana, source adapter, RPC, account, serialization, allocation, or chain
assumption.

The executable crate is the focused validation artifact:

```text
cargo test --manifest-path crates/clutch-accumulator/Cargo.toml
cargo check --manifest-path crates/clutch-accumulator/Cargo.toml --lib
```

The shadow records the bounded well-formedness relation and the algebraic
obligations to discharge under the repository's pinned Verus toolchain. It is
not called "formally verified" in this scaffold: Verus is installed and pinned
(see
[`toolchain/PINNED_PROOF_TOOLS.md`](../../toolchain/PINNED_PROOF_TOOLS.md)),
and run directly against `accumulator.rs`, it **fails** with 4x `E0308`
(`int`-typed sums returned from `u64`/`u128` spec fns) at lines 71, 75, 79, and
83. No proof log records a pass. In particular, this directory contains no
`assume`, `admit`, axiom, external-body, or proof-only executable branch.

## Closed claims

For well-formed summaries of one versioned grid and adjacent ranges, the
intended proof obligations are:

* bucket coverage and explicit missingness conserve the half-open range;
* first and last accepted values select by non-empty side, not by midpoint;
* extrema and exact integral numerators combine by associative min/max/add;
* every arithmetic operation is admitted only under the frozen bounds; and
* identity summaries do not manufacture coverage.

The shadow does **not** claim arbitrary threshold predicates, crossing counts,
run lengths, path-dependent drawdown, realized variance, or reconstruction of
discarded observations. The executable API returns a refusal for those
statistics. A registered automaton or a new versioned feature family is needed
before any such result can be considered.

