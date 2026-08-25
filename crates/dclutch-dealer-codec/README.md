# dclutch-dealer-codec

This standalone crate is the first physical refinement of the Lean successor
Dealer semantics. It is safe Rust, `no_std`, allocation-free, fixed-capacity,
and total over hostile byte input.

Lean owns the five layouts and generated constants:

- immutable Market policy: 248 bytes;
- immutable runtime-curve Candidate: 4,576 bytes;
- persistent Dealer state: 840 bytes;
- normalized current Trading release receipt: 176 bytes;
- optimistic transition request: 144 bytes.

The interpreter checks cumulative bid/ask rounding, cumulative fees, inventory
risk, prepaid work funds, delayed revision-ordered replacement, release-set
joins, and terminal unwind. It emits fixed-capacity claim and custody intents.

It does **not** authenticate Solana account ownership, signatures, Loader V3 or
Registry observations, SPL programs, CPI effects, persistence, or transaction
rollback. Those remain obligations of the future SBF adapter and its runtime
campaign. The receipt flags are accepted only from that authenticated adapter
boundary; there is no caller-selected bypass path.

The physical `16 × 8 × 2` curve capacity is provisional and liftable by
regenerating the ABI. It is not a semantic outcome-width restriction.

Run the standalone evidence suite with:

```sh
cargo test --manifest-path crates/dclutch-dealer-codec/Cargo.toml
cargo clippy --manifest-path crates/dclutch-dealer-codec/Cargo.toml \
  --all-targets -- -D warnings
```

The integration test executes the Lean emitter and requires byte-for-byte
identity with `src/generated_dealer_liquidity.rs`.
