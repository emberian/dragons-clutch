# Claim-neutral resolution model

This dependency-free executable model asks one narrow question: may a market
record an immutable v2, v3, or v4 resolution fact without presenting every
Token-2022 outcome mint?

The answer is conditional. It is safe on reachable states when the cached
external vector is a conservative upper bound, the kernel closes exactly
against that cache, active collateral covers every resolution the immutable
terms admit, resolution changes no claim or value quantity, and every later
payout-moving consumer authenticates and synchronizes current mint truth before
moving value. It is not equivalent to the current Resolve refusal surface: an
unaccounted mint increase cannot be detected from accounts the instruction does
not receive.

The model makes that distinction explicit with two validators:

- `check_cached_invariants` is everything a mint-free Resolve can decide;
- `check_reachable_invariants` additionally requires actual mint supply not to
  exceed the last observed cache.

`resolve_claim_neutral` reads only the first class. `resolve_current_full`
models the current first-resolution synchronization and exact-repeat behavior.
Every economic consumer in the model runs a full synchronization first.

Run the adversarial and bounded state-machine campaign with:

```sh
cargo test --manifest-path research/claim-neutral-resolution/Cargo.toml
cargo test --release --manifest-path research/claim-neutral-resolution/Cargo.toml
cargo clippy --manifest-path research/claim-neutral-resolution/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo doc --manifest-path research/claim-neutral-resolution/Cargo.toml \
  --no-deps
```

This is a research model, not SBF implementation or runtime evidence. The
corresponding exact architecture and STOP conditions are in
`docs/implementation/CLAIM_NEUTRAL_RESOLUTION.md`.
