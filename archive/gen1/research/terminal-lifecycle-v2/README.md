# Terminal lifecycle V2 model

Status: **MODEL-ONLY / HOST-TESTED**. This crate is an allocation-free,
dependency-free `no_std` state model. It changes no kernel, SBF program,
Token-2022 mint, account layout, market term, or release claim.

It specifies a terminal protocol for **new V2 markets only**. Every closeable
Market, Hoard, Kernel, Supply, Resolution, Position, outcome Mint, and Replay
tombstone carries the same versioned `(market, version, generation)` tag plus a
distinct role/account rent principal. A per-role ledger makes each refund
exactly once; the Replay principal is independently prepaid and permanent. New
outcome mints carry `MintCloseAuthority`, and terminal mint/market close takes
an explicit authority input. Internal claims plus external bearer supply must
equal both Supply and authoritative mint truth. V1 mints without close authority
and fractional-credit paths are explicit STOPs, not upgrade plans. Because this
model has no per-bearer credit identity, it is an internal-only terminal
profile: materializing or burning bearer claims returns an explicit STOP.

Run:

```sh
cargo test --manifest-path research/terminal-lifecycle-v2/Cargo.toml
cargo test --release --manifest-path research/terminal-lifecycle-v2/Cargo.toml
cargo clippy --manifest-path research/terminal-lifecycle-v2/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo doc --manifest-path research/terminal-lifecycle-v2/Cargo.toml --no-deps
```

The design boundary and adapter obligations are in
[`docs/implementation/TERMINAL_LIFECYCLE_V2.md`](../../docs/implementation/TERMINAL_LIFECYCLE_V2.md).
