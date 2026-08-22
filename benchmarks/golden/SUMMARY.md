# Deterministic cost-lab summary

Evidence ceiling: offline synthetic wire measurement plus analytical lower bounds. No SBF, validator, RPC, fee-market, or landing measurement occurred.

Arms: `layout_hypothesis` (design sketch, retained) and `abi_landed` (read from `programs/solana-layout/src/lib.rs` at `41c231f`), plus their `abi_differential`. A landed width is an encoding fact, never a measured cost.

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

## Landed ABI inventory

Source: `programs/solana-layout/src/lib.rs` at `41c231f`, one instance of each account.

| account | Rust constant | data bytes | package-default rent principal (lamports) |
|---|---|---:|---:|
| realm | `account_len::REALM` | 70 | 1378080 |
| profile | `account_len::PROFILE` | 100 | 1586880 |
| market | `account_len::MARKET` | 726 | 5943840 |
| hoard | `account_len::HOARD` | 108 | 1642560 |
| position | `account_len::POSITION` | 220 | 2422080 |
| feed_head | `account_len::FEED` | 124 | 1753920 |
| order_page | `account_len::ORDER_PAGE` | 4012 | 28814400 |
| supply_ledger | `account_len::SUPPLY_LEDGER` | 333 | 3208560 |
| terms | `account_len::TERMS` | 1656 | 12416640 |
| price_grid | `account_len::PRICE_GRID` | 589 | 4990320 |
| epoch | `account_len::EPOCH` | 329 | 3180720 |
| candidate_record | `account_len::CANDIDATE` | 337 | 3236400 |
| final_pot | `account_len::FINAL_POT` | 262 | 2714400 |
| settlement_receipt | `account_len::SETTLEMENT_RECEIPT` | 217 | 2401200 |
| resolution | `account_len::RESOLUTION` | 165 | 2039280 |
| clear_work | `account_len::CLEAR_WORK` | 50054 | 349266720 |
| candidate_feed | `account_len::CANDIDATE_FEED` | 6266 | 44502240 |
| **one instance of each (17)** | | **65568** | **471498240** |

Of that principal, 15144960 lamports is the per-account 128-byte storage overhead, so account count is a first-class capital term.

## Landed epoch book

| orders | representable | pages | padding slot bytes | page rent principal (lamports) | SettlePage instructions |
|---:|---|---:|---:|---:|---:|
| 1 | yes | 1 | 3540 | 28814400 | 1 |
| 16 | yes | 1 | 0 | 28814400 | 1 |
| 17 | yes | 2 | 3540 | 57628800 | 2 |
| 32 | yes | 2 | 0 | 57628800 | 2 |
| 48 | yes | 3 | 0 | 86443200 | 3 |
| 64 | yes | 4 | 0 | 115257600 | 4 |
| 65 | no: order_count_above_landed_max_epoch_orders_64 | - | - | - | - |

## Landed intent payloads on the wire

| intent | payload bytes | legacy bytes | v0+ALT bytes | accounts (hypothesis) |
|---|---:|---:|---:|---:|
| create_market | 139 | 508 | 358 | 8 |
| split | 74 | 409 | 290 | 7 |
| merge | 74 | 409 | 290 | 7 |
| materialize | 107 | 508 | 327 | 9 |
| dematerialize | 107 | 508 | 327 | 9 |
| feed_advance | 74 | 310 | 284 | 4 |
| place_order | 310 | 646 | 527 | 7 |
| cancel_order | 138 | 441 | 353 | 6 |
| settle_page | 68 | 436 | 286 | 8 |

Payload widths are landed; the account sets are hypotheses and are labeled as such in every row.

## Landed relation at MAX_ORDERS=64

| n | orders | pages | order authentications | relation steps floor | frozen epoch state bytes | frozen epoch rent principal (lamports) | V1 |
|---:|---:|---:|---:|---:|---:|---:|---|
| 2 | 16 | 1 | 16 | 37 | 5267 | 40221840 | admit |
| 2 | 32 | 2 | 32 | 69 | 9279 | 69036240 | admit |
| 2 | 64 | 4 | 64 | 133 | 17303 | 126665040 | admit |
| 4 | 16 | 1 | 16 | 41 | 5267 | 40221840 | admit |
| 4 | 32 | 2 | 32 | 73 | 9279 | 69036240 | admit |
| 4 | 64 | 4 | 64 | 137 | 17303 | 126665040 | admit |
| 8 | 16 | 1 | 16 | 49 | 5267 | 40221840 | admit |
| 8 | 32 | 2 | 32 | 81 | 9279 | 69036240 | admit |
| 8 | 64 | 4 | 64 | 145 | 17303 | 126665040 | admit |
| 16 | 16 | 1 | 16 | 65 | 5267 | 40221840 | admit |
| 16 | 32 | 2 | 32 | 97 | 9279 | 69036240 | admit |
| 16 | 64 | 4 | 64 | 161 | 17303 | 126665040 | admit |
| 24 | 16 | 1 | 16 | 81 | 5267 | 40221840 | refuse |
| 24 | 32 | 2 | 32 | 113 | 9279 | 69036240 | refuse |
| 24 | 64 | 4 | 64 | 177 | 17303 | 126665040 | refuse |

## Hypothesis versus landed ABI

| object | unit | hypothesis | landed | delta | what changed |
|---|---|---:|---:|---:|---|
| position_account | bytes | 192 | 220 | +28 | The 128-byte 16-outcome balance vector is unchanged; the landed account also stores market and owner identities, a replay generation, cash and reserved-cash atoms, a stored bump and a close state, so the header is 92 bytes rather than the hypothetical 64. |
| supply_ledger_account | bytes | 320 | 333 | +13 | Both arms carry two u64 totals per outcome (256 bytes); the landed header is 77 bytes of market, realm, generation, outcome count, bump and flags rather than the hypothetical 64. |
| single_egg_order_record | bytes | 80 | 107 | +27 | The landed record spends 64 bytes on owner and order identity plus quantity, limit, minimum fill, generation, outcome, side, flags and, since v4, an eight-byte per-order expiry epoch; the 80-byte sketch had no room for the replay generation, the expiry horizon or dual 32-byte identities. |
| portfolio_order_record | bytes | 208 | 235 | +27 | The portfolio order has a persisted page encoding: the same 128-byte 16-slot coefficient vector plus dual 32-byte identities, side, active length, flags and five u64s (lots, per-lot collateral bound, minimum fill, replay generation, expiry epoch), so the landed body is 235 bytes against the 208-byte sketch. It rides a 236-byte tagged slot shared with the single-Egg family and with retirements. |
| order_page_account | bytes | 8192 | 4012 | -4180 | The landed page is a fixed 16-slot array with cross-page closure fields, not a variable byte budget, so page size stopped being a tunable parameter. The slot is wide enough for every admitted slot kind, both order families and a retirement, which is why it is 4012 bytes rather than the 1819 of the single-family v2 page. |
| order_page_header | bytes | 128 | 236 | +108 | The landed header carries seven 32-byte identities (market, epoch, order set, page digest, first, last and previous-page-last order ids) that the hypothesis never budgeted for, plus the page/set counters and the v4 retirement count. |
| order_page_record_capacity | records_per_page | 100 | 16 | -84 | A landed page holds 16 slots, not about a hundred records, so any per-page cost is amortized over six times fewer orders. |
| epoch_book_order_capacity | orders_per_book | 512 | 64 | -448 | One frozen book is capped at 64 orders across 4 pages, so the 128- and 512-order cases describe multiple epochs, never one relation instance. |
| claim_instruction_internal_split | bytes | 11 | 74 | +63 | The landed payload names market and owner by 32-byte identity instead of packing an outcome count and a u64 into 11 bytes. |
| claim_instruction_materialize_one | bytes | 11 | 107 | +96 | The landed payload adds a 32-byte destination and an outcome index to the market/owner pair, still far inside MAX_INTENT_BYTES. |
| accumulator_full_summary | bytes | 272 | absent | absent | No accumulator summary account exists in the landed family; FeedHead is a 124-byte cursor plus evidence digest, not a fold summary, so the accumulator arm stays entirely hypothetical. |
| landed_only_account_family | bytes | absent | 61003 | absent | Realm, Profile, Market, Hoard, FeedHead, Terms, PriceGrid, Epoch, CandidateRecord, FinalPot, SettlementReceipt, Resolution, ClearWork and CandidateFeed were never in the hypothesis arm, so most of the landed rent inventory is not represented by the design sketch. |

## Interpretation

- `n=24` is always a V1 refusal even when one synthetic resource axis appears green, and in the landed arm the codec itself refuses it.
- Legacy inline addresses become the first obvious byte bottleneck for broad outcome operations; ALT is not relief from locks or CPIs.
- Rent values are refundable principal under the pinned package default, not fees and not a cluster quote.
- No total-CU number appears because no Dragon SBF program exists to measure. The only CU field is the pinned runtime CPI invocation charge component.
- Batch verification remains Omega(orders) without a separately verified succinct proof; page layout changes rent and transaction partitioning, not that information bound.
- The landed page is not a tunable byte budget: 16 slots per page is forced, so the 4/8/10 KiB page trade in the hypothesis arm no longer describes the current layout.
- One frozen landed book holds 64 orders, so the 128- and 512-order hypothesis rows describe several epochs rather than one relation instance.
- Portfolio orders have a persisted page encoding: one 236-byte tagged slot holds either order family and a retirement, so the seam `relation_v1` opened against the page is closed. The price is a common slot width, which is the whole of the page growth from 1819 to 4012 bytes.
- No landed candidate-verification instruction exists, so the landed arm reports relation work and rent without any wire byte count for that step.
