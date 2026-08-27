# Resumable resolution-work model

Status: **isolated executable MODEL**. This crate is safe Rust, `no_std`,
allocation-free in production code, fixed-width, float-free, and unpublished.
It does not change a live account layout, instruction ABI, or SBF program.

The model turns a long native B-spline occupation reduction into three checked
phases:

1. `begin` validates and freezes the complete market, Terms, basis, source,
   sealed archive, interval, finalization, cost, and funding identities;
2. `fold` rechecks the exact immutable program-owned sealed archive account,
   reads a bounded contiguous prefix directly at the stored cursor (accepting
   no caller record bytes or proofs), then atomically advances one internal
   accumulator and pays only from the frozen prepaid budget; and
3. `finalize` succeeds only at the exact archive end, writes one canonical V4
   payout vector and commitment, pays the finalizer, closes the work state, and
   returns all remaining prepaid work funds plus the locked rent reserve.

See
[`RESOLUTION_WORK_V1.md`](../../docs/implementation/RESOLUTION_WORK_V1.md) for
the protocol argument, trust boundary, cost equations, cache analysis, and
explicit release stops.

Run the isolated gates with:

```sh
cargo test --manifest-path research/resolution-work-v1/Cargo.toml
cargo test --release --manifest-path research/resolution-work-v1/Cargo.toml
cargo clippy --manifest-path research/resolution-work-v1/Cargo.toml --all-targets -- -D warnings
cargo doc --manifest-path research/resolution-work-v1/Cargo.toml --no-deps
```
