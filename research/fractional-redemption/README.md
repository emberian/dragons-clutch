# Fractional-redemption research model

Status: **MODEL-ONLY / HOST-TESTED**. This standalone crate changes no kernel,
SBF instruction, account layout, market terms, deployment artifact, or release
claim.

The safe `no_std`, allocation-free Rust model compares:

- exact-lot refusal, including gcd-derived resolved, reachable-family, integer
  simplex, and structured-claim lots; and
- persistent numerator credits with exact claimant/market/denominator/
  generation identity, mixed-outcome aggregation, custom transfer/merge,
  direct-burn donations, and the market-level credit liability.

Run:

```sh
cargo test --manifest-path research/fractional-redemption/Cargo.toml
cargo clippy --manifest-path research/fractional-redemption/Cargo.toml \
  --all-targets -- -D warnings
```

The design, Solana implications, terminal impossibility result, and V1
recommendation are in
[`docs/implementation/FRACTIONAL_REDEMPTION.md`](../../docs/implementation/FRACTIONAL_REDEMPTION.md).

