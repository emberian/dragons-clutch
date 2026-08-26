# dClutch Series codec and interpreter

This standalone crate is the first physical refinement of
`DClutchSemantics.Series`. It is safe Rust, `no_std`, `no_alloc`, fixed-layout,
and total over hostile byte slices.

Lean owns five canonical ABI values:

- immutable recurring-Market template: 240 bytes;
- Series replay cursor: 96 bytes;
- exactly prepaid occurrence ticket: 216 bytes;
- normalized current Registry/Core receipt: 168 bytes;
- optimistic transition request: 64 bytes.

The pure interpreter emits an owned atomic candidate with at most four custody
transfers and an optional complete-set founding instruction. It never mutates
accounts. A Solana adapter must authenticate the finalized template and selected
release set, authenticate the Registry receipt and its current deployment,
derive and bind every PDA/account identity, obtain current Rent and Clock,
execute claims/custody/account-creation effects, and commit the returned state
only if every operation succeeds.

The crate deliberately has its own empty workspace until shared Cargo and SBF
integration are coordinated.

Regenerate and compare the checked-in module from the repository root:

```sh
cd formal/dclutch-semantics
lake build DClutchSemantics.SeriesAbi
lake env lean --run EmitSeriesAbiRust.lean > /tmp/generated_series.rs
cmp /tmp/generated_series.rs ../../crates/dclutch-series-codec/src/generated_series.rs
```

Then run the standalone evidence:

```sh
cargo test --manifest-path crates/dclutch-series-codec/Cargo.toml --offline
cargo clippy --manifest-path crates/dclutch-series-codec/Cargo.toml \
  --all-targets --offline -- -D warnings
```
