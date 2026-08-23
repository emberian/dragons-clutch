# Deletion/replay V2 model

Status: **MODEL-ONLY / NOT A LIVE ABI**. This allocation-free `no_std` crate
specifies the minimum persisted facts needed before `ClosePosition` or
`CloseGeneralEpoch` can be re-enabled. The current V1 instructions remain
fail-closed.

The model makes four claims executable:

- a Position is a permanent generation anchor; creating a reservation debits
  assets and increments its outstanding count in one transaction, while the
  first terminal transition decrements that count exactly once;
- a Market admits only the next epoch index, and a retired epoch shrinks to a
  permanent identity tombstone instead of making its index or PDA namespace
  reusable;
- every independently addressed epoch child is created and retired through an
  authenticated registration, including every candidate bundle and ClearWork
  bundle regardless of candidate status; and
- injected failure after any modeled write rolls the whole value transition
  back, matching the required single-Solana-transaction geometry.

Candidate bundles stand for CandidateRecord + CandidateFeed + their funding
identities. ClearWork bundles stand for the work account + funding identity.
Independent counts also cover CandidateIndex pages, verdicts, escrows, order
pages, reservation archives, receipts, and the epoch pot. The implementation
ADR defines the corresponding runtime account and instruction changes.

Run:

```sh
cargo test --manifest-path research/deletion-replay-v2/Cargo.toml
cargo test --release --manifest-path research/deletion-replay-v2/Cargo.toml
cargo clippy --manifest-path research/deletion-replay-v2/Cargo.toml \
  --all-targets -- -D warnings
```
