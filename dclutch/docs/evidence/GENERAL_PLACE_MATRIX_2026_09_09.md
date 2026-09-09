# General Place production matrix, 2026-09-09

The existing same-bank General campaign now accepts explicit diagnostic outcome
width, Buy/Sell side and deterministic participant seed parameters. Its default
remains the full two-outcome Buy continuation. `DCLUTCH_GENERAL_PLACE_ONLY=1`
stops only after the actual Place transaction and canonical Order, Claims and
Custody poststate checks; it is not a settlement-completion mode. A standalone
Sell diagnostic requires that mode because the subsequent singleton solver
corpus is explicitly a Buy. It carries no alternate validator or raised budget.

## Observed matrix

Warm diagnostic source is under
`/tank/dregg-build/dclutch-general-delegated-56904d11-20260908/source`, with
accepted Effect visitor72d39ef8d, Verify profilea3dd6962f, replay hints228c8fe58,
and lifecycle cachea505da08d plus these host parameter changes. The continuing
checkout retains its older Registry V1 closure and coordinated patches. This
is local program-test evidence using genesis Claims/token fixtures, not a fresh
current-source cohort, devnet execution, or completed founding/settlement.

The no-profile Trading ELF is
`trading-lifecycle-static-production-elf/dclutch_trading_sbf.so`, SHA-256
`67eab8624f45f769200e62cc9ab88b6de3bce098701155de16fc5b82ba9d64db`.
It compiled explicitly with `-p dclutch-trading-sbf` after a native check. Other
program links are unchanged from `production-v47-sha256.txt`. All runs retain
the chain's 1,400,000 CU limit and accepted 128 KiB General heap.

| Two-outcome Buy participant seed | Place CU | Headroom CU | Canonical poststates |
| ---: | ---: | ---: | --- |
| 0 | 1,355,216 | 44,784 | passed |
| 1 | 1,337,006 | 62,994 | passed |
| 2 | 1,353,214 | 46,786 | passed |
| 3 | 1,334,813 | 65,187 | passed |
| 4 | 1,344,831 | 55,169 | passed |
| 5 | 1,329,473 | 70,527 | passed |
| 6 | 1,341,229 | 58,771 | passed |
| 7 | 1,340,090 | 59,910 | passed |

Changing the participant seed changes payer, fee payer, seal payer, solver and
output-page signer deterministically; it changes founded Market and dependent
PDA geometry while the ELF set stays fixed. This finite matrix is not a
worst-case derivation proof or a guarantee for every release identity.

All eight two-outcome Sell seeds stopped before submission at the shared
builder's `Projection("general-place-order-source-owner-token")`. Its zero
quote reserve reaches the token/delegation prelude before the inactive quote
route; no Sell poststate success is claimed. This is the next concrete Sell
construction boundary.

| Other width, Buy seed0 | Result |
| ---: | --- |
| 1 | categorical fixture constructor refused before transaction construction |
| 3 | Place1,348,730 CU, headroom51,270; canonical poststates passed |
| 8 | Place exhausted its compute allowance |
| 258 | Place returned `TradingSbfError::Content` after250,201 CU, before Accelerator CPI; cause still to be localized |

The actual failures remain failures. None was replaced by a smaller profile,
a larger budget or a native-only pass. Matched nonzero trade, settlement and
durable production entrypoint/poststate reconciliation remain outstanding.

## Evidence

The base contains `place-matrix-cache-production-summary.txt`, one log per row
named `place-matrix-cache-production-n{width}-{buy|sell}-seed{seed}.log`, and
`general-place-matrix.py`, which ran every row sequentially and recorded every
exit status. Each process selected only
`one_founded_market_opens_and_then_closes_its_batch_in_one_bank` with one test
thread. Host compile log: `host-general-place-matrix.log` (13.89 seconds).
Production check/link logs: `check-general-lifecycle-production.log` and
`sbf-general-lifecycle-production.log` (actual SBF compile19.55 seconds).

Environment parameters are `DCLUTCH_GENERAL_OUTCOMES`,
`DCLUTCH_GENERAL_PARTICIPANT_SEED`, `DCLUTCH_GENERAL_ORDER_SIDE` and
`DCLUTCH_GENERAL_PLACE_ONLY`; each selected value is printed before execution.
