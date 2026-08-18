# Deterministic cost-lab summary

Evidence ceiling: offline synthetic wire measurement plus analytical lower bounds. No SBF, validator, RPC, fee-market, or landing measurement occurred.

## Claim transition envelope

| n | external legacy bytes | external v0+ALT bytes | accounts | token CPIs | trace entries | V1 |
|---:|---:|---:|---:|---:|---:|---|
| 2 | 478 | 266 | 11 | 3 | 4 | admit |
| 4 | 610 | 274 | 15 | 5 | 6 | admit |
| 8 | 874 | 290 | 23 | 9 | 10 | admit |
| 16 | 1402 | 322 | 39 | 17 | 18 | admit |
| 24 | 1930 | 354 | 55 | 25 | 26 | refuse |

ALT compression changes wire bytes, not logical account locks, CPI work, or the V1 outcome bound. The account topology itself is a Dragon layout hypothesis.

## 8 KiB page hypothesis at n=16

| orders | single pages | 50% alternating pages | portfolio pages | package-default rent for 50% mix (lamports) |
|---:|---:|---:|---:|---:|
| 32 | 1 | 1 | 1 | 57907200 |
| 128 | 2 | 3 | 4 | 173721600 |
| 512 | 6 | 10 | 14 | 579072000 |

## Batch verification example: n=16, 512 orders, 8 KiB pages

| format | pages | all-pages bytes | one-page bytes | wire/account pages per transaction | minimum transactions from wire/accounts only | order authentications | 50% portfolio dot terms |
|---|---:|---:|---:|---:|---:|---:|---:|
| legacy_inline | 10 | 849 | 552 | 21 | 1 | 512 | 4096 |
| v0_alt | 10 | 451 | 433 | 58 | 1 | 512 | 4096 |

These minimum transaction counts ignore compute. They cannot be used to claim that an all-pages verification will execute or land.

## Accumulator full-summary fold

| pages | legacy bytes | v0+ALT bytes | combine steps | summary data bytes | package-default rent (lamports) |
|---:|---:|---:|---:|---:|---:|
| 1 | 307 | 250 | 0 | 272 | 2784000 |
| 4 | 406 | 256 | 54 | 272 | 2784000 |
| 16 | 802 | 280 | 270 | 272 | 2784000 |

## Interpretation

- `n=24` is always a V1 refusal even when one synthetic resource axis appears green.
- Legacy inline addresses become the first obvious byte bottleneck for broad outcome operations; ALT is not relief from locks or CPIs.
- Rent values are refundable principal under the pinned package default, not fees and not a cluster quote.
- No total-CU number appears because no Dragon SBF program exists to measure. The only CU field is the pinned runtime CPI invocation charge component.
- Batch verification remains Omega(orders) without a separately verified succinct proof; page layout changes rent and transaction partitioning, not that information bound.
