# Fractional-redemption research model

Status: **RETAINED DERIVATION MODEL**. The promoted fixed-layout runtime
contract is `crates/clutch-fractional-redemption-runtime`; this standalone
crate remains the exhaustive small-domain algebra and comparison harness. It
does not own runtime accounts, enable an SBF instruction, or make a deployment
or release claim.

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
