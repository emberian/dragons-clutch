# dclutch-rational-representation-v2-kernel

This standalone `no_std`, `no_alloc`, safe Rust kernel refines
`DClutchSemantics.RationalRepresentationV2`.

It replaces the prototype's exact-lot-only Fractional refusal with explicit
claim shards:

```text
F_i = D * C_i
F_i = K_i + R_i
K_i = S * c_i
therefore D * C_i = S * c_i + R_i
```

`C_i` is Claims-owned native custody. `F_i`, `K_i`, receipt supply `S`, and
every holder balance contributing to `R_i` are Token-owned observations.
`R_i` is an ordinary transferable/coalescible shard balance, never a hidden
rounding credit or wrapper-local ledger. The kernel validates borrowed
runtime-width projections and prepares exact effects; it persists nothing.

The same crate hostile-decodes a canonical representation DAG. Nodes are
ordered by `(rank, content_id)`, every edge decreases rank, child identities
are strictly ordered, all nodes reach the selected last root, and every node's
common-scale native exposure is checked from its children. The SBF adapter
must additionally authenticate finalized Record ownership and recompute the
content digest over the exact graph bytes; this pure crate deliberately does
not contain SHA-256, Solana accounts, Token-2022, or CPI.

Evidence:

```sh
cargo test --manifest-path crates/dclutch-rational-representation-v2-kernel/Cargo.toml
cargo clippy --manifest-path crates/dclutch-rational-representation-v2-kernel/Cargo.toml \
  --all-targets -- -D warnings
```

This is kernel/theorem evidence only. Physical Token-2022 transfer, mint,
burn, Claims/Custody composition, late rollback, rent, packet, CU, and SBF
verifier evidence remain adapter work.
