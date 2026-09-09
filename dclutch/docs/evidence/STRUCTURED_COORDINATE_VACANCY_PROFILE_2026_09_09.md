# Structured coordinate vacancy profile — 2026-09-09

This is local-validator and saved-account native-interpreter evidence. It does
not establish accepted coordinate activation or retirement, devnet execution,
or mainnet readiness.

## Retained V2 execution and located refusal

The preserved validator is PID 443622, RPC 30400, work directory
`/tank/dregg-build/structured-a8e3-v2-runtime-20260909`. Its immutable runtime
source is `a8e3b4e8c593a5a63eef94a51375f46ba5ad75d2`; checked Upgrade gate SHA-256
is `46af94470e79e86d7ceeb456574baddf1ea4c8bc705f944273b34e2df91dc90a`.
No loaded program or validator was replaced or restarted for this diagnosis.

`continuation-fd685.log` records accepted selector-255 root activation at slot
15598 (516065 CU), receipt activation at 15828 (374092 CU), coordinate-zero
seal at 16021 (263651 CU), and rent prepayment at 16053 (1050 CU). Coordinate
activation then simulates `TradingSbfError::Content` before Claims CPI:
188627 Trading CU, 189085 transaction CU.

`coordinate0-capture-939ba.json` preserves the unsigned local test transaction
and account images (SHA-256
`5e58a2ada1accf268d4764c97b1caa8f923b8163d22f3115d116e0437f8b2996`).
`coordinate0-replay-normalization.json` explicitly records diagnostic-only
normalization of ALT last-extension and Loader deployment/cache slots. The
unaltered original capture and bank remain authoritative. The normalized
production replay reproduces the same refusal and exactly 189085 CU in
`replay-coordinate0-production-normalized.log`. The separate a8
`hot-cu-profile` Trading ELF localizes the account-profile refusal to logical
26, expected 160 bytes, observed zero, after `p5r-projection-banks` in
`replay-coordinate0-profile-normalized.log`.

The detailed same-bank capture at finalized slot 21412 is
`coordinate0-capture-detailed2.json`, SHA-256
`5ecc02675cb7d7aff8220395cb9867c26fb9d853dbaa89e68b45ad08d37cc7ea`.
It names the two exact mismatches:

| Logical / Claims child | Native resource | Address | Selected / observed bytes |
| --- | --- | --- | --- |
| 26 / 21 | LiabilityBasisPositionV2 | `AtZwGFBG861vSDjShw3Eq832rhP9NsuY7eoJFeV6eibi` | 160 / 0 |
| 27 / 22 | ProtocolPositionAdmissionV2 | `2uWphC5aHxDY47DstcwBAniwSJ1sYBzmhWEVR79sqj4p` | 512 / 0 |

Both are prefunded System-owned empty accounts. Claims owns their allocation:
`LifecycleRequestV2::protocol_position_request` maps ActivateCoordinate to
ProtocolPosition Admit with Vacant presence. The old selected compiler wrongly
required the account lengths that only RetireCoordinate/Close consumes.

## Correction and controls

`structured_lifecycle_selected_v1` now emits Exact zero-byte prestate for both
activation resources and retains exact native live lengths for retirement.
It does not introduce a wildcard, LifecycleBound prestate, preallocation, or a
change to native ownership, PDA, signer, writable, or refusal checks.

The hbox source is `/tank/dregg-build/structured-host-c0a3388ee-20260909`, base
`fd685475da4e9f34be10bb79792d787172f83da8`, with its own target and separately
hashed patches. Commands print the checkout root and HEAD. The test-only
old-guard run in `structured-coordinate-vacancy-oldguard.log` fails the exact
ActivateCoordinate coordinate-26 assertion (144 versus zero at basis width 2).
After the correction, the filtered
`structured_lifecycle_selected_v1::tests::` suite passes all four tests, including
creation/retirement controls at widths 2 and 258. The green log and exit status
are `/tank/dregg-build/structured-coordinate-vacancy-green.{log,status}`.

The saved-account replay now accepts an explicit lifecycle action and preserves
the full coordinate projection. Its content-key substitution follows Trading's
canonical representative mapping for packed aliases; the initial diagnostic
omitted that mapping and reached `AliasMismatch` after the width repair. This
was a diagnostic defect, not a demonstrated native alias failure.

The corrected diagnostic in
`coordinate0-profile-fixed-alias-replay.json` reports the original profile as
`Err(DataLengthMismatch)` and the candidate as `Ok(())`, with no width or
privilege mismatches. The original profile SHA-256 is
`3d9954dd5a0558f3c8ff4a3888f9555382b2f5f34fa5bc8e7da6fdcabd832150`;
the corrected profile is
`0ca600ea8c782cde4e641f87bbe7cc8ac372fb43771536560e01bacd2200c068`.
Two derived hostile captures independently insert a synthetic live-size body
at logical 26 or 27. Both corrected-profile replays refuse exactly
`DataLengthMismatch`; the corresponding `coordinate0-hostile-live-26-replay.json`
and `coordinate0-hostile-live-27-replay.json` identify the changed coordinate.
These are explicit synthetic controls, not observations of the bank.

The final diagnostic/fresh-selection host binary SHA-256 is
`4910abb445f1eed8ba64e22df3fc71733da3a8369e03369bf0f3846ce7a9790a`,
built from the fd685 base plus `structured-fresh-selection-host.patch`
(SHA-256 `6a8220c32414522e8520b9cbbf49d660d417cd32be4a43f5f9a505ec86109703`).
The targeted local-founding role test also passes, proving that the fresh
role set excludes administration authority and the public-only substituted
founder. Build and test output is
`/tank/dregg-build/structured-fresh-selection-build2.log`.

## Remaining runtime boundary

Changing AccountProfile changes the immutable selected closure and Market
commitment. The original Market cannot be repinned. The bounded
`structured-claims-fresh-selection` campaign entrance authenticates the retained
plan, completed administration report and live genesis, derives only fresh local
founding roles, then calls the existing founding and lifecycle continuation
stages. It neither creates a validator nor prepares or injects a new genesis
account set. The old Market and evidence remain independently named.

Accepted coordinate minting, conservation and retirement still require actual
execution of that fresh selection on the preserved V2 bank. The fresh run
started at 2026-09-09 08:21:28 UTC under host PID 895101, recording
`fresh-selection-vacancy.log` and `fresh-selection-source.json` in the retained
job. Its work directory is `fresh-selection-vacancy`; the original `campaign`
directory remains unchanged. This document makes no claim about that run's
later outcome.
