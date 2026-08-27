# Deterministic invariant campaign

This std-only executable adversarially exercises the pure Rust surfaces without
adding fuzzing-framework or random-number-generator dependencies. It is not a
proof and does not exercise SVM account locking, CPI, token movement, or
validator behavior.

The frozen seed set lives in `src/main.rs`. Every generated action and verdict
is folded into a stable 128-bit transcript digest. A failure prints the seed,
case, and step in its assertion message; replay is therefore a normal `cargo
run`, not a dependency on a fuzz corpus service.

Run the bounded campaign:

```sh
cargo run --manifest-path tools/invariant-campaign/Cargo.toml --release
```

The lanes cover:

- arbitrary kernel transition sequences, refusal atomicity, multi-position
  aggregate closure, complete-set exits, exact rational payout dust, and
  `u64::MAX` arithmetic edges;
- canonical order-page encode/decode, byte-mutation differential verdicts, and
  buffered/streaming page equivalence;
- coupled batch/stream verifier verdict identity over generated books,
  candidate mutations, and high-bound arithmetic inputs.

The checked run transcript is recorded in `evidence/campaign-v1.txt`.
