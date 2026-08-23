# Clutch candidate lifecycle

This crate is the dependency-free, `no_std`, `no_alloc`, safe-Rust kernel for
the proposed two-window general candidate lifecycle. It is production-bound
source, but it is not connected to the SBF adapter and makes no deployment or
release claim.

It owns:

- exact half-open submission and verification slot intervals;
- fixed-capacity begun-candidate enumeration, including abandoned staging;
- versioned Window, Candidate, Index, Verdict, Escrow, Budget, policy, and wire
  codecs;
- generic score-policy-bound rank keys, without importing a score or clearing
  implementation;
- prepaid progress/completion/finalization rewards;
- checked validity/abandonment penalties, expiry, refunds, and winner credit;
- atomic pure transitions that validate copies before publishing output.

It does not own hashing, PDAs, Solana account memory, Clock authentication,
relation execution, score computation, lamport movement, CPI, or transaction
atomicity. The adapter obligations are listed in
[`../../docs/implementation/CANDIDATE_LIFECYCLE_V2_KERNEL.md`](../../docs/implementation/CANDIDATE_LIFECYCLE_V2_KERNEL.md).
Copy-resistant admission, capacity-quality denial of service, the SBF adapter,
and epoch-root retirement remain explicit promotion blockers.

Run independently:

```sh
cargo test --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml
cargo test --release --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml
cargo clippy --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo doc --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml --no-deps
```
