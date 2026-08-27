# dclutch-structured-v2-operator

Chain-derived effect planning for shard-backed Structured receipts.

This layer authenticates nothing on its own. It consumes already-decoded
immutable Structured terms, already-decoded exact claim-shard terms, and an
explicitly named adapter observation whose `finalized` flag is the trust
boundary; it re-derives every amount through the pure kernel and emits typed
effect plans plus the borrowed inputs of the execution candidate.

Solana SDK types are deliberately absent. Instruction construction, PDA
derivation, AccountProfile expansion, and CPI belong to a physical adapter that
does not exist yet. Decision 0011 records the shape it must take: the chain
reaches this family through a sealed artifact closure — an `AccountProfileV2`,
a `TransitionProgramV3`, an `EffectProgramV4` and three more, named by a
`CapabilityProgramV4` — and never by calling into these crates. Token atoms
themselves move only through a `FixedRole` child, since no effect operation can
move them; decision 0011 §3a records the open choice between adopting the
Rational child ABI and giving Structured its own. The operator adds nothing to
the SDK version surface either way.

`tests/actions.rs` runs the full round trip for all four actions: chain
observation to operator plan to frame-derived lowering to
`StructuredHotCandidateV2::prepare`. The account coordinates are asked of
`StructuredFrameSpecV2` rather than supplied, so the plan and the candidate
cannot agree on a layout the frame does not specify. That round trip is
host-side evidence about planning, not evidence that Token-2022 executed.

Evidence:

```sh
cargo test --manifest-path crates/dclutch-structured-v2-operator/Cargo.toml
cargo clippy --manifest-path crates/dclutch-structured-v2-operator/Cargo.toml \
  --all-targets -- -D warnings
```
