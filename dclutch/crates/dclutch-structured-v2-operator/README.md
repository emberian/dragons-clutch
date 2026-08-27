# dclutch-structured-v2-operator

Chain-derived effect planning for shard-backed Structured receipts.

This layer authenticates nothing on its own. It consumes already-decoded
immutable Structured terms, already-decoded exact claim-shard terms, and an
explicitly named adapter observation whose `finalized` flag is the trust
boundary; it re-derives every amount through the pure kernel and emits typed
effect plans plus the borrowed inputs of the onchain-safe candidate.

Solana SDK types are deliberately absent. Instruction construction, PDA
derivation, AccountProfile expansion, and CPI belong to the physical integration
adapter, which consumes `StructuredHotCandidateInputV2` and revalidates every
field independently. The operator therefore adds nothing to the SDK version
surface.

`tests/actions.rs` runs the full round trip for all four actions: chain
observation to operator plan to profile lowering to `StructuredHotCandidateV2::
prepare`. That is the check that the operator emits exactly what the onchain
contract accepts.

Evidence:

```sh
cargo test --manifest-path crates/dclutch-structured-v2-operator/Cargo.toml
cargo clippy --manifest-path crates/dclutch-structured-v2-operator/Cargo.toml \
  --all-targets -- -D warnings
```
