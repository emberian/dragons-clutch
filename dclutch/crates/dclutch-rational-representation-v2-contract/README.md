# dclutch-rational-representation-v2-contract

This standalone `no_std`, safe, allocation-free contract is the physical ABI
between the RationalRepresentationV2 kernel and the current Claims, Token, and
Custody owners.

The Lean-owned request is 488 bytes plus one 160-byte asset row per selected
outcome. Selected actions carry one row; Structured issue/unwrap carries the
exact Product outcome width. The fixed 592-byte receipt is produced only after
all active child effects and postconditions join.

The five physical plans are:

- denominate: Claims `Materialize(q)` then mint `D*q` shard atoms;
- reconstitute: burn `D*q` shard atoms then Claims `Dematerialize(q)`;
- Structured issue: transfer `q*c_i` shards into each custody and mint `q`
  receipt atoms;
- Structured unwrap: burn `q` receipts then return every `q*c_i` shard balance;
- terminal: burn `D*q` selected shards, execute Claims terminal redemption,
  and require a canonical Custody Hoard-to-external transfer when payout is
  positive.

The adapter is the current Claims program: `representation_program ==
claims_program`. Token Mint supply and holder balances are always hostile
pre/post observations; the only local mutable fact is replay revision. Every
request and downstream Claims/Custody receipt is digest-bound.

Evidence:

```sh
lake build DClutchSemantics.RationalRepresentationV2PhysicalAbi
cargo test --manifest-path crates/dclutch-rational-representation-v2-contract/Cargo.toml
cargo clippy --manifest-path crates/dclutch-rational-representation-v2-contract/Cargo.toml \
  --all-targets -- -D warnings
```

The tests include exact Lean generator freshness, more than 2,000 independently
evaluated Lean↔Rust arithmetic cases, positive paths for all five actions,
partial-shard refusal, child receipt substitution checks, and late Token
postcondition refusal.

This is not yet an SBF adapter. Remaining work is exact account/PDA/Registry
authentication; real Token-2022 mint, burn, and transfer CPI; Claims/Custody
return-data producer checks; state-last rollback; verifier-clean SBF; and
packet, ALT, CU, rent, and local-validator evidence.
