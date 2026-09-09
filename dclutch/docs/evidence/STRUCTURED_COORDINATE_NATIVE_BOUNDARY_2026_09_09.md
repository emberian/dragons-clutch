# Structured coordinate native boundary — 2026-09-09

This records host-native regression evidence, not accepted local-validator or
devnet execution. The retained pre-RegistryV2 runtime remains untouched.

## Located source defects

Trading's V6 Claims composition admission added the common account prefix to
`LIFECYCLE_COORDINATE_ACCOUNT_COUNT_V2`, although Claims declares that constant
as the entire coordinate frame. The released V6 artifact builder produces a
34-account child; the old Trading admission required 54. Trading now consumes
the full Claims-owned count directly. It admits both published single-coordinate
actions, ActivateCoordinate and RetireCoordinate. Complete receipt retirement
continues through its separate compact support-derived route.

Coordinate execution also requires the Trading-derived protocol-Position caller
at child coordinate 20 to sign. Claims constructs its exact Admit/Close request
internally, with the specialized V2 lifecycle digest as parent context. Trading
previously signed only the outer lifecycle caller (and the unrelated optional
Fractional root). It now derives and verifies the exact nested authority,
verifies its Claims capability owner, marks only coordinate 20 as the additional
signer, and supplies its PDA seeds to the existing Claims CPI. No Claims
refusal or lifecycle invariant changed.

## Controls

Executed in `/Users/ember/dev/dclutch` at base HEAD
`7eb4d69ada25fc8386110e666a511b8c6b27fb51` plus this lane's change, using Rust
1.97.1 and the workspace's own `target/`. Each measured command printed the
repository root and HEAD. No SBF build was performed; this commit leaves the
frame ratchet red pending cohort capture.

- `cargo check --locked -p dclutch-trading-sbf`: passed.
- `cargo test --locked -p dclutch-trading-sbf --lib structured_external_frames_match_production_selected_artifacts -- --test-threads=1`:
  one passed. The test invokes the actual released V6 artifact builder for
  receipt activation and both coordinate actions, then compares its Claims
  route width with Trading admission. The compact-only receipt retirement
  refusal is pinned to `TradingSbfError::Content`.
- Restoring the old common-plus-coordinate arithmetic made that exact control
  red: ActivateCoordinate returned `Ok(54)` against the production artifact's
  `Ok(34)`. This mutation was removed before the green run.
- `cargo test --locked -p dclutch-trading-sbf --lib rational_coordinate_ -- --test-threads=1`:
  two passed. Both actions reproduce the exact PDA from returned signer seeds;
  six hostile mutations cover authority, owner, lifecycle parent bytes,
  AccountInfo writability, meta writability and meta key. Each refuses exactly
  `TradingSbfError::Content` before granting any signer.
- Removing the secondary signer assignment made the signer positive control
  red (`[]` versus `[20]`). The assignment was restored before the green run.

The initial filtered test invocation omitted `--lib` and failed to compile an
unrelated integration target's pre-existing `crate::TradingSbfError` import;
it is not recorded as a test pass. The corrected filtered library targets
above ran and passed.

Final source SHA-256:

| Path under `programs/dclutch-trading-sbf/src/` | SHA-256 |
| --- | --- |
| `claims_composition_v3.rs` | `f9a301aac7f0b79e6e7a7445492a3270f760bab0f528e15ed6e6e3253726721d` |
| `hot_v3/children.rs` | `6affa7999e4207e055b0060d5f856f4144b4d672d2368356faff7a2e1de95451` |
| `rational_lifecycle_signer_v3.rs` | `7abfb19b70ea7bbcc2c30e55b356fe3f681584a702638b1e77913e82b72ba9e5` |

## Runtime boundary still owed

The retained runtime's
`/tank/dregg-build/structured-validator-32991-691-20260909/continuation-2f2.log`
records successful rent prepayment at slot 20627 followed by simulated
ActivateCoordinate refusal `TradingSbfError::Content` before Claims CPI,
235,915 total CU. It contains no Claims-composition reason marker. Therefore
these are proven source-level blockers, but that log alone does not localize
the earliest runtime refusal to the frame-width conjunction. A fresh committed
cohort with the existing `hot-cu-profile` checkpoints must establish the next
boundary and accepted activation/retirement. No program or validator in the
retained cohort was replaced or restarted for this work.
