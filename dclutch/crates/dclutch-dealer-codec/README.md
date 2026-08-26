# dclutch-dealer-codec

This standalone crate is the first physical refinement of the Lean successor
Dealer semantics. It is safe Rust, `no_std`, allocation-free, fixed-capacity,
and total over hostile byte input.

Lean owns the five layouts and generated constants:

- immutable Market policy: 216 bytes;
- immutable runtime-curve Candidate: 4,576 bytes;
- persistent Dealer state: 840 bytes;
- normalized current Trading release receipt: 176 bytes;
- optimistic transition request: 144 bytes.

The interpreter checks cumulative bid/ask rounding, cumulative fees, inventory
risk, prepaid work funds, delayed revision-ordered replacement, release-set
joins, Core-Market-bound terminal entry, and terminal unwind. It emits
fixed-capacity claim and custody intents. The public borrowed Candidate encoder
constructs exact runtime curves without exposing or duplicating generated
offsets, allocation, or inactive padding.

It does **not** authenticate Solana account ownership, canonical Core state,
signatures, Loader V3 or Registry observations, SPL programs, CPI effects,
persistence, or transaction rollback. Those are obligations of the separately
named SBF adapter and its runtime campaign. The receipt flags and terminal Core
Market coordinate are accepted only from that authenticated adapter boundary;
there is no caller-selected resolver or release bypass path.

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
