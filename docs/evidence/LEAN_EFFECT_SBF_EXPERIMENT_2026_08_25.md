# Lean Effect SBF experiment — 2026-08-25

## Scope

This evidence concerns only the isolated `dclutch-effect-sbf` measurement
adapter at source commit `2ddfdd0`. It is not evidence for a complete Direct
lifecycle or a deployable replacement for `dclutch-sbf`.

The adapter trusts the signer named in its program-owned projection to have
performed semantic admission. It does not authenticate signed intents, bind a
Product, move SPL collateral, execute custody CPIs, or prove the caller is a
particular controller program. Those are required successor gates, not details
hidden by this measurement.

## Reproducible artifact

The exact committed tree was reconstructed with `git archive 2ddfdd0` and
built with:

```sh
cargo build-sbf \
  --manifest-path programs/dclutch-effect-sbf/Cargo.toml \
  --lto --optimize-size --dump
```

- cargo-build-sbf: 4.0.0
- platform-tools: v1.53
- SBF rustc: 1.89.0
- default SBF architecture: v0
- stripped ELF bytes: 12,016
- SHA-256:
  `550af7638294d5c0bbce20cbbfafd666eb857132b8eb86a46ce9c0f355efe4a1`
- `.text`: 9,808 bytes (`0x2650`)
- `.rodata`: 350 bytes (`0x15e`)
- `.data.rel.ro`: 128 bytes (`0x80`)
- verifier diagnostics: zero

Commit `2ddfdd0` replaces the allocating SDK entrypoint with its fixed-capacity
no-allocation entrypoint. Against the otherwise identical optimized artifact at
`3b8bf29`, that reduced the ELF from 14,392 to 12,016 bytes (16.51 percent),
the successful path from 1,423 to 1,238 CU (13.00 percent), and the late
refusal from 1,271 to 1,086 CU (14.56 percent).

The dump's largest frame-relative offsets were 3,232 bytes in the generic
64-account no-allocation `entrypoint`, 440 bytes in `process_instruction`, 128
bytes in `Plan::decode`, and 56 bytes in `execute`. The generated call wrapper
reaches its conventional 4,096-byte frame boundary; the build verifier accepted
it. This is evidence for generating an exact two-account entrypoint rather than
treating the SDK's generic entrypoint as the final proof target.

## Real-SVM execution

`crates/dclutch-svm-harness/tests/effect_executor.rs` loaded the exact optimized
ELF through `solana-program-test` 4.2.1. No native processor or mock adapter was
registered.

- successful seven-effect Lean plan: 1,238 compute units;
- deliberately late `u64` overflow: refused after 1,086 compute units; and
- the program-owned 104-byte projection was byte-for-byte unchanged after the
  refused transaction.

The successful post-state also matched the existing authenticated Direct
reference transition through the separate differential test in
`dclutch-direct-contract`.

## Loader V3 permanent capitalization

Using Solana `Rent::default()`, the 36-byte Loader V3 Program account and the
45-byte ProgramData metadata, permanent rent-exempt capitalization is:

| Artifact | ELF bytes | Program + ProgramData capitalization |
|---|---:|---:|
| Integrated `dclutch-sbf` at `103635d` | 9,771,616 | 68.012792880 SOL |
| Isolated Effect executor at `2ddfdd0` | 12,016 | 0.085976880 SOL |

The isolated artifact is 791.06 times cheaper by this narrow measure, a
67.926816000 SOL difference. This excludes transient buffer capitalization,
transaction fees, state accounts, controller programs, and every microprogram
still required by a complete architecture. It therefore establishes the
economic importance of program partitioning; it does not establish the final
number or aggregate rent of successor programs.

The integrated comparison ELF was 9,771,616 bytes with SHA-256
`33879c28d0d50cf2a5c408f6d94f29f8ee6b5c312f71a4d293385c8f5ba7442c`.

## qedsvm artifact-bridge result

The pinned qedsvm v0.11.0 source is annotated tag object
`ef3f165761fb20269e96bad9d8df7c851a463dcc`, peeled commit
`2356bc6865ed36a454d2a7285bd3989518ddd31f`, with Lean toolchain 4.30.0.
Its 294-job `lake build` completed locally.

qedsvm executed the exact 12,016-byte ELF successfully and independently
reported 1,238 CU. `QEDSVM_TRACE_OUT` captured a 1,208-step successful PC trace.
This is execution in a Lean interpreter, not verification.

`qedlift` then failed closed rather than emitting a theorem:

- without a trace, satisfiability-witness construction could not steer the
  data-dependent account-array address expression to a witness base; and
- with the successful trace, its H8 alias-soundness check found overlapping
  mixed-width stack/account footprints and refused to emit a vacuous
  separation-logic precondition.

No qedsvm theorem is claimed. The observed obstacle is ordinary adapter
codegen—generic account deserialization, stack structures, mixed-width accesses,
and copies—not the seven-effect economic transition. The next proof-target
experiment must generate an exact two-account serialized-input parser and
alias-simple byte transition, or wait for a reviewed qedsvm H8 byte-granularity
extension. The existing adapter remains the real-SVM measurement baseline.

## Result

The experiment passes the first physical feasibility gate: canonical Effect IR
can be executed by a verifier-clean, very small, low-CU SBF program with
transactional refusal. The qedsvm run also identified a precise compiler/adapter
shape that prevents an honest artifact theorem today. Architectural succession
remains unearned until that bridge, semantic authority, real custody,
controller-to-executor authentication, and differential property space are
closed.
