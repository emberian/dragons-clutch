# Candidate lifecycle V2 model

Status: **MODEL-ONLY / HOST-TESTED**. This dependency-free `no_std` crate
models the proposed general-candidate timing, replay, ranking, expiry, and
prepaid reward transitions. It does not modify the kernel, SBF program,
accounts, wire, deployment, or release claim.

The model pins the half-open boundaries:

```text
[freeze, submission_close)        candidate staging and seal
[submission_close, verify_close)  permissionless verification
[verify_close, infinity)          deadline finalization and expiry
```

It also checks the safe early-finalization branch, generic score-policy rank
keys, monotone reward payment, invalid/refused/expired bond treatment, and
one-shot finalization/refund behavior. The implementation-ready account and
wire proposal is
[`../../docs/adr/0006-two-window-candidate-lifecycle.md`](../../docs/adr/0006-two-window-candidate-lifecycle.md).

Run:

```sh
cargo test --manifest-path research/candidate-lifecycle-v2/Cargo.toml
cargo test --release --manifest-path research/candidate-lifecycle-v2/Cargo.toml
cargo clippy --manifest-path research/candidate-lifecycle-v2/Cargo.toml \
  --all-targets --all-features -- -D warnings
```
