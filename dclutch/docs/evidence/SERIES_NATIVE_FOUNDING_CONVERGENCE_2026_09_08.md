# Series native founding convergence — 2026-09-08

Owner: correctness_architecture. Native component and default SBF build evidence;
no selected execution or local-validator acceptance is claimed here.

The shared Claims founding owner now constructs the matched Claims request and
Core permit from one checked quantity and intent. Trading uses the authenticated
Product payout scale, the realized projected-State replay identity, and the
native Hoard replay revision. Core retains admission and actual permit writes.

## Exact-source native controls

Source `7de925e43558a47495105a46d14aaa0271570c61` was archived to
`/tank/dregg-build/dclutch-series-founding-7de925e43-20260908/source`, with its own
target. The integration owner independently verified all 3,273 tracked entries
with zero mismatches; expected inventory SHA256 is
`4d4fb6c442ebb6e1d1f34b0aba5cf63e06b9cb2bc089f63c2defdac33324f252`.
Rust and Cargo were 1.97.1. The bounded `swarm-build` job used six Cargo jobs and
a 32 GiB memory limit. Every test used its named filter and `--test-threads=1`;
library rows also used `--lib`. Logs below are under that checkout's sibling
`logs/` directory.

| Row | Result | Log SHA256 |
| --- | --- | --- |
| `cargo check --locked` for Claims, Core SBF and Trading SBF | exit 0 | `4d6f43c0a45150a9dde47f27d4b5d43d241d42bcc82a86ee8de67eef495158ea` |
| Claims `founding_plan_v1` | 2 passed | `65528dc349645b1c0e9b0aa97bc109ea7ba2abce20c346cb2ccfb0e64ab8f482` |
| Trading `derives_actual_children_from_hoard_open` | 1 passed | `f2b56e6ab5d1d5a9cdb203a2d359576a6b9562bbfb602a316041a4ece60b36fa` |
| Trading `closing_source_revision_cannot_masquerade_as_realized_hoard_replay` | 1 passed | `634dcd8bc0540ec2bb7acbd8963de386560c9c7295d898a928844a69f45e54c2` |
| Core `permit_compiler_binds_exact_claims_request_and_refuses_conservation_drift` | 1 passed | `3439e3f590e5fc7bf3916ddc82102789dfb551d78a1a0a7e496011b9edc877b2` |
| Bootstrap `same_bundle_native_consume_then_expire_preserves_all_replay_poststates` | did not execute: compile exit 101 | `c25c09fda661ec3d77ca7f4c02aa7b1b4ef603c76ea14043d4cdc12aa660827c` |

The last row failed compilation at `terminal_sequence.rs:6760`: missing
`derived_failure_escrow_v1` (`E0425`). It is not a failed recurrence assertion and
does not establish a new exact-source recurrence result. The earlier recurrence
result remains separately qualified in
[the native recurrence evidence](SERIES_NATIVE_RECURRENCE_2026_09_08.md).
No scheduler reuse marker occurred. An initial job launched before archive
completion was rejected as an instrument failure and retained under
`instrument-before-archive-complete/`; none of its rows are included above.

Diagnostic mutation controls preceded the exact-source repeat. Restoring the old
Trading constructor produced quantity 1 where the native scale required 3;
restoring the old child-bank revision predicate admitted the stale 2→3 replay
that the new test requires to refuse with `ClaimsReplay`. Restored implementations
passed both controls. These are overlay diagnostics, not source identity proof:
their logs are respectively `native-founding-old-mirror-red.log` (SHA256
`253407d7a82f5ef1a8ab594bf7aff0326848df942d0e7745597e45cf01f72ae1`)
and `native-child-replay-old-red2.log` (SHA256
`3f6382d931103edc2a2824404fd6d316a03cf9abb9fda9731a953e581bfc9bc0`)
under `/tank/dregg-build/dclutch-series-115frame-diagnostic-20260908`.
An earlier revision mutation that changed no source was rejected as an invalid
instrument.

## Exact-source frame repair

Source `c2737a0eb033c3356917d9de76a36f7c56ebf5f7` outlines the same pure fixed-layout
constructor without changing its API, guards or bytes. Its completed archive is
`/tank/dregg-build/dclutch-series-founding-frame-c2737a0eb-20260908/source`;
its exclusive target is the sibling `build-target/`. Native package checks
preceded SBF builds. The bounded `swarm-build` jobs used four Cargo jobs and a
16 GiB memory limit. Each row ran `cargo build-sbf --manifest-path
programs/dclutch-<program>-sbf/Cargo.toml` with default features. All five rows
exited 0, and all logs contained zero frame-overflow, frame-overwrite or scheduler
reuse diagnostics.

| Program | SBF log SHA256 | ELF SHA256 |
| --- | --- | --- |
| Accelerator | `7bf42073f15f40561405aa5176427e47e212f412c6d1f4d6ed28f5759240fe31` | `67e5c8865ab4cdd618c9971f090cbd5a6b60ba8397c4de895172cedd87cad8c6` |
| Claims | `32453e9605eed52f9067d45e95c712be6a1198c6fa0f4ff800cc61865ea55eb4` | `9b8fb9215b7e4c3cb70055b6cb7cf89871f689b136c19f95b5c1fdb866c9b087` |
| Trading | `2b5d2a367549791715e84616e251b2b710851a9a126d48bc4d2cf3186e136059` | `eaf62abe3a693868fa84ce67b18b7ea1fe118236604e0c7d2950a7f9122f0b8c` |
| Core | `5aa780e92fb0f2251ced9afd4e4e4ea671d5fb927079d5e14d1af1c92d113d4e` | `8d4629e5d74f123b40e90710c77d8c2c6b8940cd6c957630f17332b977a1cfc9` |
| Custody | `bef2bac14e71650697b8fb99f64fb623b3e35d37a19c427aef5f8fcc3fd7b651` | `ec24a38d1f5ea5256d10867ebf35b3557122cc5b325625903d93e94557254e59` |

Logs are `logs/<program>-sbf.log`; ELFs are
`build-target/deploy/dclutch_<program>_sbf.so`. Default links do not establish the
selected Series path, heap or CU acceptance. Actual Core-written permit bytes,
the entire child CPI sequence, token movement, transaction rollback, unchanged
publication across validator occurrences, and publication-wide geometry
admission remain separate obligations.
