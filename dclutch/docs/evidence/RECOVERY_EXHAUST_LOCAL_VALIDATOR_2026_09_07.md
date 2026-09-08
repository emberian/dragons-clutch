# Recovery exhaust — local-validator evidence, 2026-09-07

This is working-tree diagnostic evidence, not checked-release evidence. Host revision `71286f9bb0e6b30a320e39f9de332936ceced59e`; checked gate revision `56767e555d05ecd005edec4fc939d269e8757417`; gate SHA-256 `6031115cd75da111c10d248a8489d7179d8edd5d320bc59e024a5f76e7107909`.

Artifact root: `/tank/dregg-build/dclutch-codex-recovery-diag-71286f9bb/runs/20260907T220441Z-both/exhaust`.

The exhaust arm accepted advance (`3rTwJnsMXSC4vgyvpLgfmGWmKQLUAW1Vb1aeCMYsUsdCDPrv6YxYuvQpVBkBsjDbgPHkBAMqAoA1qYJvShHUswBf`), exhaust (`4U2iDkGiZrsDkePGR4UNWtvWhjFRxMfUBgtB5PQgLTMmDYDttmCWRBPuyagBAW2dieAYxLdR5jjUa5cWnJ9cweLf`), and deadline failure (`63xrm4AH6raExHPFcPfEn6oCuFgRxvYrdnRVmyamTLFxbsEHiLmz3sNdjQFDiDmrrdnVaHyjLDBRyvVYv9qjptnj`). Core admitted ResolutionFailure winner 3/4; Claims custody replay was created.

The hoard drained from 500000001 to zero through three ordinary founder payouts of 166666667 atoms each (indices 0–2). Index 3 is the failure escrow, whose prescribed result is `recorded-no-op`: it is a program-derived account without a signer and is owed no failure payout. It is not a fourth refund.

`transcript.json` and all three `refund-*-evidence.json` files are retained at the artifact root above; SHA-256 digests must be read from that immutable run before any release claim.

## Addendum — 2026-09-07, artifact inventory

A bounded read of the preserved run records the following file hashes. Refund files
are under `campaign/`, not directly at the artifact root.

| Artifact | SHA-256 |
|---|---|
| `transcript.json` | `191990a1f9a51e729875874ad217de4166c634b957db2cf93ec6e0d79ba2cf8c` |
| `campaign/refund-0-evidence.json` | `083f682fa5c3eeaa0c393abc18e6ae88aa93862c8eeae85f3c764816e9fb9107` |
| `campaign/refund-1-evidence.json` | `70758e340624bf3f6437570113f472a5270037acbac9e59a7ec2338e82f30595` |
| `campaign/refund-2-evidence.json` | `fc91e3b5391aab2d2a5ef4d0b055fed66cdf6e6c9688d16f6937d195e9b92916` |

| Claim index | Finalized slot | Paid atoms | Signature |
|---|---|---|---|
| 0 | 8453 | 166666667 | `4aVF6GxukyRtMKfvQpxSiBJfH9CzJb6HPGKUbWtSNa8X3Ek1CyNBf7odrP9sC5g8Wmx8nVL6aFgP3Ye8q8f8G18j` |
| 1 | 8698 | 166666667 | `3JptyZxpDbiXQvxnB1iha4UcmVfEbp7rjSdu7wG9B9m7CofZFd8kATkmXK5hoerEGqUrh8FyHpbY9hDd4HBAjTR9` |
| 2 | 8924 | 166666667 | `Pq29NF47sQwLEjb8ZFrwC41mjFBfzhXiqHgUUFWQYZM4tVf33iwD1UQB8ZchnsK7wyKcP13Bm6TK4P5pW6ZYNUy` |

The run records the host and gate revisions above and mutable-source tree digest
`35013864908446676eb583d7a27eaf97755c2f7639f4ec7f110829a6c0710d94`.
Its deployment source is `observed-programdata-account`; it has no `source.json`
and no clean-commit attestation. These facts support the diagnostic poststates,
not a claim that this was a fresh deployment of all current sources.
