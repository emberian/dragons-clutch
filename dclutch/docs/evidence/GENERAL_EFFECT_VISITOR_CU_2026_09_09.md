# General Effect visitor compute, 2026-09-09

The Trading Effect visitor now omits account-mutation bookkeeping for resolved
request patches, exact balance assertions and no-ops. The kernel still applies
each operation after the visitor: request bounds/narrowing and exact balance
checks remain authoritative. Data writes and lamport transfers retain the
funding, root, immutable-binding, participation and retained-write checks.
The binding bank is still allocated to its exact validated width and complete
binding coverage is still checked after projection.

## Controlled diagnostic measurement

Source base: `8716d5e408ae0cdf25f7f229ded1f6298a526c0f` plus this change.
The continuing warm checkout is
`/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/source`.
It retains its older Registry V1 closure and accepted coordinated patches;
these are local program-test measurements, not an exact-current-source cohort
or devnet evidence. All limits remain the chain's 1,400,000 CU and the accepted
128 KiB General heap. The fixture uses two outcomes and a single Buy.

Both profile links enable only the existing `observations-released` and
`p7-effect-projection` checkpoints, with the full profile feature off. The same
unhinted host and other program links are used. The measured interval includes
projection, so changes to release identities and PDA searches before the first
checkpoint do not masquerade as local savings.

| Action | Baseline interval CU | Optimized interval CU | Difference |
| --- | ---: | ---: | ---: |
| PlaceOrder | 143,493 | 127,282 | 16,211 saved |
| OpenBatch | 30,011 | 30,104 | 93 added |

The baseline run exhausted PlaceOrder's compute allowance. The optimized profile
accepted PlaceOrder at 1,368,318 CU, leaving 31,682 CU; canonical Order, Claims and
Custody poststates passed. It also accepted SubmitCandidate at 483,520 CU with
canonical Candidate and Batch poststates. The next refusal was the host's Verify
account projection, `InvalidVariableDataPrestate`; no Verify transaction was sent.
This one seed does not establish robust compute headroom for Buy/Sell or other
widths, and a nonzero trade and settlement remain outstanding.

Evidence under the same base:

- `program-test-delegated-v45-effect-baseline.log`
- `program-test-delegated-v46-effect-optimized.log`
- `trading-effect-visitor-baseline-profile-elf/dclutch_trading_sbf.so`, SHA-256
  `011f357c4511d12c5ec41a579dc15be560406ef5dcef3c2b1cfa4c98fa2af3f5`
- `trading-effect-visitor-optimized-profile-elf/dclutch_trading_sbf.so`, SHA-256
  `2836d116d352f4ba6bfb088ef3630bd2c85f5ff1264be076929bb7b888c0bde4`

## Production boundary

The subsequent no-profile production link, SHA-256
`5e2c8849a513a582fb2faa29ea8c6331ad34d14e5666b7a3808bf698aab3dc71`,
was compiled explicitly in 45.05 seconds and run with the same host. Its changed
release/PDA geometry exhausted PlaceOrder's compute allowance; Claims Admit
alone used 107,491 CU. Thus the profile's accepted Place/Submit is not production
route acceptance, and the measured local savings do not solve worst-case
headroom. `program-test-delegated-v47-effect-production.log` records this wall;
`production-v47-sha256.txt` records every diagnostic ELF. Further host bump-hint
and varied-seed controls remain necessary.

## Refutation and build controls

The filtered `commit_last_` controls pass 4/4: root commits last, rent and
geometry refuse exactly, retained writes preserve values, disabled writes stay
absent, request narrowing refuses, and unequal lamports refuse. A temporary
kernel mutation accepting an unequal balance caused the new exact assertion to
fail (exit 101); the kernel was restored byte-for-byte and all four controls
passed again. Logs are `native-effect-visitor-green.log`,
`native-effect-visitor-lamport-red.log`, and
`native-effect-visitor-restored-green.log`. The warm test-only ArtifactRelease
constructor uses its Registry V1 spelling; production visitor code is identical.

Native package checks precede SBF builds. Commands select
`-p dclutch-trading-sbf` explicitly after the build-sbf separator and each uses a
fresh output directory. Both profile logs record an actual Trading compile,
not silent default-package success. Temporary checkpoints and kernel mutations
are absent from production source. This change leaves the frame ratchet red;
root's fresh all-program cohort and frame capture own that boundary.
