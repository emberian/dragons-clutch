# dclutch-claims-representation-codec

This standalone crate is the first physical refinement of the unified
`ClaimsRepresentation` semantics. Lean owns the descriptor, action, state, and
action-rule layouts. The Rust interpreter is safe, `no_std`, `no_alloc`, and
width-polymorphic: a descriptor is a fixed 224-byte header followed by exactly
one `u64` claim weight per Product outcome.

Bearer, Structured, and Fractional are descriptor shapes, not dispatch
families. The interpreter prepares:

- an exact wrapper-state successor;
- claimant-specific mint, burn, or adapter-retirement intent; and
- a borrowed iterator of exact EconomicKernel materialize, dematerialize, and
  terminal-redemption intents.

It never mutates Market supply, Hoard principal, token state, accounts, or
external systems. A physical Claims adapter must authenticate Registry/Core,
the descriptor account, the claimant, Token-2022 state and authorities, execute
the Economic and token effects atomically, and persist the successor only after
all effects succeed.

Run the isolated evidence:

```sh
cargo test --manifest-path crates/dclutch-claims-representation-codec/Cargo.toml
cargo clippy --manifest-path crates/dclutch-claims-representation-codec/Cargo.toml \
  --all-targets -- -D warnings
```

The integration test executes the Lean generator and requires byte-for-byte
equality with `src/generated.rs`.
