# Recovery exhaust — local-validator evidence, 2026-09-07

This is working-tree diagnostic evidence, not checked-release evidence. Host revision `71286f9bb0e6b30a320e39f9de332936ceced59e`; checked gate revision `56767e555d05ecd005edec4fc939d269e8757417`; gate SHA-256 `6031115cd75da111c10d248a8489d7179d8edd5d320bc59e024a5f76e7107909`.

Artifact root: `/tank/dregg-build/dclutch-codex-recovery-diag-71286f9bb/runs/20260907T220441Z-both/exhaust`.

The exhaust arm accepted advance (`3rTwJnsMXSC4vgyvpLgfmGWmKQLUAW1Vb1aeCMYsUsdCDPrv6YxYuvQpVBkBsjDbgPHkBAMqAoA1qYJvShHUswBf`), exhaust (`4U2iDkGiZrsDkePGR4UNWtvWhjFRxMfUBgtB5PQgLTMmDYDttmCWRBPuyagBAW2dieAYxLdR5jjUa5cWnJ9cweLf`), and deadline failure (`63xrm4AH6raExHPFcPfEn6oCuFgRxvYrdnRVmyamTLFxbsEHiLmz3sNdjQFDiDmrrdnVaHyjLDBRyvVYv9qjptnj`). Core admitted ResolutionFailure winner 3/4; Claims custody replay was created.

The hoard drained from 500000001 to zero through three ordinary founder payouts of 166666667 atoms each (indices 0–2). Index 3 is the failure escrow, whose prescribed result is `recorded-no-op`: it is a program-derived account without a signer and is owed no failure payout. It is not a fourth refund.

`transcript.json` and all three `refund-*-evidence.json` files are retained at the artifact root above; SHA-256 digests must be read from that immutable run before any release claim.
