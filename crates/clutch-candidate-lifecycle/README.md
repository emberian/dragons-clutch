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
- monotone routing of unsolicited lamport surplus to the immutable neutral sink;
- atomic pure transitions that validate copies before publishing output.

It also contains the registry-independent `CandidateWindowV4` /
`CandidateAdmissionNodeV3` successor seam. That seam leaves every V2 byte
unchanged, splits submission into commit and reveal subintervals, removes the
shared finite sponsor-funded candidate page, and enumerates individually funded
nodes through a reverse-linked head plus exact live/closed counts. One
instruction creates, reveals, terminalizes, or closes one node. The checked
best rank remains independent of append order under the successor's specified
ordinal-independent canonical node derivation. Refused and abandoned nodes pay
separate exact policy penalties, and close outputs bind every recipient as well
as an exact partition of the observed lamports.

It does not own hashing, PDAs, Solana account memory, Clock authentication,
relation execution, score computation, lamport movement, CPI, or transaction
atomicity. The adapter obligations are listed in
[`../../docs/implementation/CANDIDATE_LIFECYCLE_V2_KERNEL.md`](../../docs/implementation/CANDIDATE_LIFECYCLE_V2_KERNEL.md).
For V2, copy-resistant admission and the fixed-capacity quality denial remain
explicit blockers. The successor prevents simple post-reveal witness copying
from redirecting the reward and removes the fixed shared admission slot, but it
does not claim private order flow, proposer-censorship resistance, general MEV
resistance, or unlimited verification throughput. Its hash/account adapter,
live tags, candidate-bundle join, SVM evidence, and counted Epoch integration
remain unimplemented. Candidate-bundle cleanup and selected-settlement terminal
liveness are not established; an externally unclosable newest head would delay
older reverse-linked refunds. The identity tie break is deterministic but can
be ground by searching over commitments or authorities.

Run independently:

```sh
cargo test --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml
cargo test --release --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml
cargo clippy --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo doc --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml --no-deps
```
