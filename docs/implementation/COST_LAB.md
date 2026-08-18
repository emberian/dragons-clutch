# Cost lab implementation

Status: offline deterministic harness implemented and validated on 2026-08-18  
Landed-ABI arm added 2026-08-18 (adversarial review P1-F)  
Source-constant snapshot: 2026-08-17  
Landed-ABI snapshot: `programs/solana-layout` at commit `da2fbf7` (OrderPage v3)  
Landed-ABI arm re-pinned 2026-08-18: OrderPage v3, 228-byte tagged order slots, 3,883-byte page  
ABI-audit repaired and re-pinned 2026-08-18: the gate had been dead since `927d4bc` — `MAX_KNOTS` entered `account_len::TERMS` unpinned, so every run aborted with `refusing to evaluate unknown token in ABI expression: MAX_KNOTS` and exit 2 before printing one drift line, and it therefore never reported TermsAccount v3 (1,304 -> 1,656) or OrderPage v4 at `e780d5b` (record 107, portfolio 235, tombstone 80, slot 236, header 236, page 4,012, MAX_INTENT_BYTES 302, CancelOrder 138, ORDER_PAGE schema 4, TERMS schema 3); the evaluator also substituted pinned values for identifiers referenced inside `account_len`, which masked lockstep drift (ORDER_SLOT_BYTES 228 -> 236 read as no drift and the page moved by 1 of 129 bytes), and its declaration parser was single-line only. Repaired: an unpinned or unreadable token is now a named drift line carrying the referencing constant and the pin-table fix, referenced identifiers are cross-checked against the codec instead of substituted, wrapped multi-line declarations and commented-out ones parse correctly, and the audit additionally re-derives `account_version`, account/intent discriminators, field-term decompositions and `Intent::encoded_len` arms; landed arm re-pinned to `e780d5b`, goldens regenerated, 193 `layout_hypothesis` rows byte-identical  
Owned path: `benchmarks/`

## Outcome

The cost lab generates 261 deterministic scenarios across three named arms: 193 rows in the
retained `layout_hypothesis` arm, 56 rows in the `abi_landed` arm, and 12 `abi_differential` rows.

The `layout_hypothesis` arm is the original sketch and is described in this section as it was
written: outcome counts `n = 2, 4, 8, 16, 24`; internal split, fully external split, one-Egg
materialization and all-Egg materialization; legacy inline and v0+ALT wire layouts; 4/8/10 KiB
dense order pages; terminal/TWAP/full accumulator summaries over 1/4/16 pages; and batch
verification over 32/128/512 alternating single-Egg/portfolio orders. Every finding below it is a
statement about that hypothesis, not about the landed program; the addendum dated 2026-08-18
carries the landed arm and the delta between the two.

It intentionally produces two evidence classes:

1. **Measured local serialization:** concrete deterministic legacy/v0 transaction bytes using one
   fixed-width placeholder signature, real compact-length framing, fixed-width synthetic keys,
   blockhash, compiled account indices, instruction data, and v0 lookup descriptors. An independent
   field-width sum must equal the emitted byte length for every row.
2. **Analytical estimates/lower bounds:** account topology, writable locks, Token CPI and trace
   counts, minimum order authentication and asset-closure work, portfolio dot-product terms,
   accumulator combines, page packing, and refundable rent principal under a pinned package
   default.

There is no measured Dragon CU result. No Dragon SBF program exists, so inventing per-operation CU
would turn a layout sketch into false evidence. The only reported CU field is Agave's pinned
946-CU CPI invocation charge multiplied by CPI count; it explicitly excludes invoked Token-2022
execution, account copying/serialization, syscalls, and Dragon logic.

## Running and evidence closure

```text
python3 benchmarks/cost_lab.py check
python3 benchmarks/cost_lab.py summary
python3 benchmarks/cost_lab.py abi-audit
python3 -m unittest discover -s benchmarks/tests -v
```

`abi-audit` re-derives every `account_len` constant from the codec source on disk and refuses when
the landed arm has gone stale. It is deliberately outside golden closure: goldens pin a commit,
the audit checks the working tree against that pin.

The checked artifacts are:

- `benchmarks/golden/matrix.json`: all inputs, outputs, evidence labels, admission result, caveats,
  source pins, and exact harness/constants file digests;
- `benchmarks/golden/matrix.csv`: compact comparison surface;
- `benchmarks/golden/SUMMARY.md`: deterministic selected tables; and
- `benchmarks/golden/checksums.sha256`: closure over the three derived artifacts.

Generation uses Python's standard library, exact integers, no random input, and no timestamp in
rows. It performs zero RPC calls, validator calls, signatures, submissions, account mutations, or
package downloads. `check` regenerates the bytes in memory and refuses any golden drift.

## Findings in the retained hypothesis arm

### Claim-transition wire and CPI geometry

The fully external split hypothesis gives:

| n | legacy inline bytes | v0+ALT bytes | logical accounts | Token CPIs | trace entries | V1 disposition |
|---:|---:|---:|---:|---:|---:|---|
| 2 | 478 | 266 | 11 | 3 | 4 | admit |
| 4 | 610 | 274 | 15 | 5 | 6 | admit |
| 8 | 874 | 290 | 23 | 9 | 10 | admit |
| 16 | 1,402 | 322 | 39 | 17 | 18 | admit only under the existing `MAX_OUTCOMES=16` policy |
| 24 | 1,930 | 354 | 55 | 25 | 26 | refuse |

The `n=16` legacy form is 170 bytes over the 1,232-byte packet limit while the v0+ALT synthetic
form has 910 bytes of wire margin. This is not a claim that the v0 transaction executes: ALT does
not reduce 39 logical account locks, 17 Token CPIs, or their compute/account-copy work. It says a
V1 all-external atomic path is structurally ALT-dependent under this account hypothesis. The
one-Egg escape hatch remains much narrower: the `n=16` materialize-one legacy row is 379 bytes,
8 accounts, and one Token CPI.

`n=24` demonstrates why independent admission rules matter. Its v0 synthetic external split fits
the packet and the 64-account snapshot, but V1 still refuses it. A green byte column cannot override
the fixed kernel dimension, proofs, state layout, or safety headroom.

### Rent principal is not a fee

Under the pinned `solana-rent` 4.3.0 default, minimum balance is
`(128 + data_len) * 6,960` lamports. Therefore:

| Object/layout hypothesis | Data bytes | Package-default refundable principal |
|---|---:|---:|
| bare Token-2022 mint | 82 | 1,461,600 lamports |
| base Token-2022 account | 165 | 2,039,280 lamports |
| fixed V1 Position header + 16 `u64` balances | 192 | 2,227,200 lamports |
| hypothetical n=16 SupplyLedger header + internal/external `u64` totals | 320 | 3,118,080 lamports |
| 16 bare outcome mints | 1,312 total | 23,385,600 lamports across 16 accounts |
| 16 existing external destinations | 2,640 total | 32,628,480 lamports across 16 accounts |

The per-account 128-byte rent overhead means total data bytes alone do not determine capital.
These numbers are not a target-cluster quote and not an operation charge: outcome mints are
persistent Market state, destinations are assumed to pre-exist, and rent principal is potentially
refundable according to the eventual lifecycle. Funding must use the action-time cluster Rent
sysvar, not this package snapshot.

### Page tradeoff remains open

For `n=16`, 512 alternating single/portfolio records, the current packing hypothesis gives:

| Page bytes | Pages | Page accounts in all-pages batch | Package-default page principal | Legacy all-pages wire bytes |
|---:|---:|---:|---:|---:|
| 4,096 | 19 | 19 | 558,581,760 lamports | 1,146 |
| 8,192 | 10 | 10 | 579,072,000 lamports | 849 |
| 10,240 | 8 | 8 | 577,290,240 lamports | 783 |

This is a genuine trade rather than an 8-KiB victory. Smaller pages use more account locks and wire
bytes but happen to pack this alternating corpus with less paid data/overhead; larger pages reduce
the lockset and message size. None earns adoption until actual SBF folding CU, write contention,
creation/reallocation behavior, and single-versus-portfolio fragmentation are measured. The
10-KiB point is a layout hypothesis, not an instruction realloc recommendation.

The `n=16`, 512-order, 8-KiB batch has 512 unavoidable order authentications, 512 fill-bound checks,
17 asset-closure checks, and 4,096 dot-product terms at 50% portfolio share before hashing,
allocation, fee, score, and account-load work. All ten page accounts fit the synthetic legacy
message, but that does not collapse the verification to one executable transaction. Without a
separately verified succinct proof, the relation remains at least `Omega(orders)`.

### Accumulator geometry is small on wire, unmeasured in CU

The full-summary layout hypothesis is 272 data bytes, or 2,784,000 lamports under the pinned
package default. Folding 1/4/16 page summaries yields legacy messages of 307/406/802 bytes and
0/54/270 scalar combine steps respectively. Those steps are semantic counters, not CU. Summary
widths remain contingent on associative conservative semantics for variance and drawdown; the lab
must not make an unproved field permanent merely because 272 bytes appears affordable.

## Addendum 2026-08-18: the landed ABI arm (P1-F)

Adversarial review P1-F recorded that the lab's layout values had become hypotheses coexisting
with a landed ABI, and that no cost conclusion could be attributed to the current layout until the
lab consumed it. This addendum closes that item. The hypothesis arm is retained under its own
name; nothing in it was edited, replaced, or quietly re-pointed.

### Where the landed arm comes from

`programs/solana-layout/src/lib.rs` is the single codec owner, pinned at commit `da2fbf7`
(blob SHA-256 `d0228c5a...`), with the relation bounds from `crates/clutch-batch`. Rather than
quoting totals, `benchmarks/constants.json` stores each width as the codec's own field terms and
formula, and `cost_lab.py` refuses to run unless every pinned total equals the sum of its terms.
`python3 benchmarks/cost_lab.py abi-audit` goes further: it re-derives the nine pinned size
identifiers and all fifteen `account_len` constants from the Rust source on disk and refuses when
the arm has gone stale, so a later ABI change turns the cost lab red instead of silently ageing
it. It did exactly that when OrderPage v3 landed: the audit exited 2 on the unknown
`ORDER_SLOT_BYTES` token rather than publishing the old page width, and this addendum records the
re-pin that followed.

### Landed account inventory

| account | Rust constant | data bytes | package-default rent principal |
|---|---|---:|---:|
| Realm | `account_len::REALM` | 70 | 1,378,080 |
| Profile | `account_len::PROFILE` | 100 | 1,586,880 |
| Market | `account_len::MARKET` | 726 | 5,943,840 |
| Hoard | `account_len::HOARD` | 108 | 1,642,560 |
| Position | `account_len::POSITION` | 220 | 2,422,080 |
| FeedHead | `account_len::FEED` | 124 | 1,753,920 |
| OrderPage | `account_len::ORDER_PAGE` | 3,883 | 27,916,560 |
| SupplyLedger | `account_len::SUPPLY_LEDGER` | 333 | 3,208,560 |
| Terms | `account_len::TERMS` | 1,304 | 9,966,720 |
| PriceGrid | `account_len::PRICE_GRID` | 589 | 4,990,320 |
| Epoch | `account_len::EPOCH` | 328 | 3,173,760 |
| CandidateRecord | `account_len::CANDIDATE` | 305 | 3,013,680 |
| FinalPot | `account_len::FINAL_POT` | 262 | 2,714,400 |
| SettlementReceipt | `account_len::SETTLEMENT_RECEIPT` | 217 | 2,401,200 |
| Resolution | `account_len::RESOLUTION` | 165 | 2,039,280 |
| **one instance of each (15)** | | **8,734** | **74,151,840 lamports** |

Of that principal, 13,363,200 lamports is the per-account 128-byte storage overhead, so the
account count is a first-class capital term and not a rounding detail. A one-instance inventory is
an accounting unit, not a deployment plan: a live market holds many Positions, pages and receipts.

### The order page stopped being a parameter

The landed page is a 235-byte header plus a dense array of sixteen 228-byte order slots, so
`235 + 16 * 228 = 3,883` bytes exactly, and a frozen non-final page must hold exactly sixteen
slots. One slot is a kind byte plus the widest admitted body — 227 bytes of `PortfolioRecord`,
against 99 for the single-Egg `OrderRecord` — with required-zero padding out to the common width,
so both order families share one page, one order-id chain and one fold at the cost of a page that
grew from 1,819 to 3,883 bytes. That growth is the price of the shared slot, not slack. Page count
is therefore forced by order count rather than chosen: 1 order still costs a whole page (3,420
bytes of paid zero padding), 17 orders cost two, and `MAX_ORDER_PAGES = 4` with
`MAX_ORDERS_PER_PAGE = 16` caps one frozen book at `MAX_EPOCH_ORDERS = 64`, which is exactly
`clutch_batch::MAX_ORDERS`. A 65-order book is a codec refusal, not an expensive case. A full
four-page book is 111,666,240 lamports of page rent principal, and the frozen epoch state around it
(Epoch, PriceGrid, CandidateRecord) adds 1,222 bytes and 11,177,760 lamports.

The hypothesis arm's 4/8/10 KiB page trade-off is consequently a statement about a design that no
longer matches the codec. It is retained for its falsification history and must not be quoted as
current layout.

### Differential

| object | unit | hypothesis | landed | delta | what changed |
|---|---|---:|---:|---:|---|
| Position account | bytes | 192 | 220 | +28 | Same 128-byte 16-outcome balance vector; the landed header is 92 bytes (market, owner, generation, cash and reserved cash, bump, close state) rather than 64. |
| SupplyLedger account | bytes | 320 | 333 | +13 | Same two u64 totals per outcome; the landed header is 77 bytes rather than 64. |
| single-Egg order record | bytes | 80 | 99 | +19 | The landed record carries dual 32-byte identities plus a replay generation the sketch had no room for. |
| portfolio order record | bytes | 208 | 227 | +19 | `PortfolioRecord` persists the 16-slot coefficient vector the sketch had, plus dual identities, lots, per-lot collateral bound, minimum fill and a replay generation. |
| order page account | bytes | 8,192 | 3,883 | -4,309 | A fixed 16-slot array with cross-page closure fields replaced a variable byte budget; the slot is sized for the wider family. |
| order page header | bytes | 128 | 235 | +107 | Seven 32-byte identities (market, epoch, order set, page digest, first, last, previous-page-last) were never budgeted. |
| records per page | records | 100 | 16 | -84 | A page now holds 16 slots rather than about a hundred records, so any per-page cost amortizes over six times fewer orders. |
| orders per book | orders | 512 | 64 | -448 | The 128- and 512-order rows describe several epochs, never one relation instance. |
| internal-split instruction | bytes | 11 | 74 | +63 | `Intent::Split` names market and owner by 32-byte identity. |
| materialize-one instruction | bytes | 11 | 107 | +96 | `Intent::Materialize` adds a 32-byte destination and an outcome index. |
| accumulator full summary | bytes | 272 | absent | - | No summary account exists in the landed family; FeedHead is a 124-byte cursor, not a fold. |
| landed-only accounts | bytes | absent | 4,298 | - | Twelve landed accounts had no counterpart in the hypothesis arm, so most of the landed rent inventory was previously unmodeled. |

One of these is structural rather than numeric: the accumulator family remains entirely
hypothetical, so its summary widths inherit no landed support at all. The other structural gap
recorded here on 2026-08-18 — the persisted page could not represent a portfolio order that the
landed relation admits — was closed the same day by OrderPage v3, and the portfolio row above now
carries a landed width instead of `absent`.

### Landed intent payloads

Nine intents have landed encoded widths: CreateMarket 139, Split 74, Merge 74, Materialize 107,
Dematerialize 107, FeedAdvance 74, PlaceOrder 165, CancelOrder 130 and SettlePage 68 bytes, all
far inside `MAX_INTENT_BYTES = 256`. The lab emits real transaction bytes around those exact
payloads, so the payload column is landed. The account set around each payload is not: those
counts stay explicitly named `layout_hypothesis_not_landed`.

### What is still a hypothesis, and what is still unmeasured

- **Wire format.** No landed Solana message exists. Every transaction byte count in either arm is
  the lab's own synthetic legacy/v0 framing with placeholder signatures and keys.
- **Account topology.** Which accounts an instruction locks, which are writable, which live in a
  lookup table, and whether one intent is exactly one instruction are all unlanded.
- **Instruction bytes in composite paths.** Payload widths are landed for the nine intents. There
  is no landed instruction for candidate verification, so the landed relation rows report work and
  rent and emit no wire byte count rather than inventing a payload.
- **Everything about execution.** No Dragon SBF program exists. There is still no measured CU,
  heap, stack, account-copy, write-contention, or landing figure in any arm; the harness refuses
  any landed or differential output key ending in `_cu`.
- **Account lifecycle.** Instance counts per market, reallocation, closure and rent refunds are
  not modeled, so the inventory is a unit price list and not a capital plan.
- **The accumulator arm in full**, plus the summary algebra it depends on.

A landed byte width is an exact encoding fact. It is not evidence that the operation executes,
fits, or lands.

### Promotion path is unchanged

The five-step promotion path below is unaffected by this addendum: step 1 is now partly done for
account state (the codec is frozen and the lab consumes it) and untouched for messages, so the
differential comparison against a pinned Solana SDK serializer still has to happen before any
packet or lock conclusion is drawn from these rows. Steps 2 through 5 stand exactly as written.
No result here opens Gate L0 or authorizes devnet or mainnet activity.

## Facts, pins, and model assumptions

### External facts

- The official transaction documentation currently states a 1,232-byte packet limit, 64 enforced
  accounts (with a documented inactive 128-account feature), 64 top-level-plus-CPI trace entries,
  and 64-byte signatures. These are snapshot inputs, not eternal constants:
  [Solana transactions](https://solana.com/docs/core/transactions).
- The official constants and compute-budget references state a 1.4-million-CU transaction maximum
  and distinguish runtime execution limits from scheduler cost:
  [constants reference](https://solana.com/docs/core/constants-reference),
  [compute budget](https://solana.com/docs/core/fees/compute-budget).
- The runtime source is pinned to Agave `v4.2.1`, commit
  `c4b48df969a9e4f121e14a389bd7bec34c752507`. Its execution-budget source fixes the modeled CPI
  invocation component at 946 CU and the maximum at 1.4 million:
  [Agave release](https://github.com/anza-xyz/agave/releases/tag/v4.2.1),
  [pinned execution budget](https://github.com/anza-xyz/agave/blob/c4b48df969a9e4f121e14a389bd7bec34c752507/program-runtime/src/execution_budget.rs),
  [pinned trace limit](https://github.com/anza-xyz/agave/blob/c4b48df969a9e4f121e14a389bd7bec34c752507/transaction-context/src/lib.rs).
- Agave's locked `solana-rent` dependency is version 4.3.0 with crates.io checksum
  `39f0d780bf8e8a1fe8b5b5fce1acad6b209485b86dec246e7523d5e4a8b7c7fc`; its default is 6,960
  lamports per byte with 128 bytes overhead and the SIMD-0194 one-times formula:
  [pinned rent source](https://docs.rs/solana-rent/4.3.0/src/solana_rent/lib.rs.html).
- Bare mint and base token-account lengths are 82 and 165 bytes in Token-2022 interface `v3.1.1`,
  commit `e18f9c6f9bf6044b934f48e3090e8e59e4820f02`:
  [pinned Token-2022 state](https://github.com/solana-program/token-2022/blob/e18f9c6f9bf6044b934f48e3090e8e59e4820f02/interface/src/state.rs).

The exact source-file and lockfile SHA-256 values are retained in `benchmarks/constants.json`.

### Dragon hypotheses

The following values are deliberately not called facts:

- the explicit 8-account internal/one-Egg paths, `2n+7` external split, and `2n+6`
  materialize-all account topologies;
- 11-byte claim instructions, `88 + 8n` byte candidate-verification instructions, and which
  accounts live inline versus in one lookup table;
- a 64-byte Position header, 64-byte SupplyLedger header, 128-byte page header, 80-byte single-Egg
  record, and `80 + 8n` byte portfolio record (all superseded by the landed arm in the 2026-08-18
  addendum, and retained here only as the design arm's own record);
- 120/160/272-byte terminal/TWAP/full summaries and their scalar combine counts; and
- one signature, one top-level Dragon instruction, existing destination accounts, no compute-budget
  instruction, and no ATA creation.

They are centralized in `benchmarks/constants.json`, labeled `layout_hypothesis`, and guarded by
goldens so any change is explicit. Where a landed ABI now exists, it lives beside them in the
`abi_landed` arm with its own source pin and its own rows, and the difference is published rather
than left implicit.

## Falsifiers and promotion path

The present lab can falsify “the proposed message necessarily fits,” “ALT removes CPI/account
cost,” “n=24 may be admitted because one envelope fits,” “page size has no capital tradeoff,” and
“batch verification is independent of book size.” It cannot falsify total-CU, heap/stack, account
copy cost, lock contention, Token-2022 extension behavior, validator feature activation, fee-market
landing, or runtime rollback behavior.

Promotion requires, in order:

1. freeze the real wire/account layouts and differentially compare every synthetic byte count with
   the pinned Solana SDK serializer;
2. add Eggcrate primitive-operation counters without mapping them to CU;
3. build the adapter and measure pinned `solana-program-test`/SBF actual and requested CU, stack,
   heap, trace, account-data delta, and rollback with raw samples;
4. add a local-validator control only when it tests behavior program-test cannot, still without a
   public RPC; and
5. bind target-cluster features, Rent sysvar and program deployments only under a later explicit,
   bounded read authorization.

The design must narrow or paginate if the real `n=16` internal or one-Egg path approaches packet,
lock, trace, stack, heap, or CU ceilings after safety margin; if full external materialization
requires undocumented client assumptions; if a summary family is not associative/conservative; or
if batch pages cannot authenticate every order under bounded resumable work. No result here opens
Gate L0 or authorizes devnet/mainnet activity.

## Validation record

Validated with Python 3.14.6 on Apple arm64 macOS:

- 261 unique scenario IDs with exact family counts, of which the retained `layout_hypothesis` arm
  is exactly 193 and is asserted to stay that size;
- every emitted transaction length equals the independent analytical sum;
- every `n=24` row is refused by V1;
- every external split reports `n+1` Token CPIs and `n+2` trace entries;
- every batch row authenticates exactly its input order count;
- short-vector boundary encodings and non-splitting page packing have adversarial tests;
- every landed width equals the sum of the codec field terms it was transcribed from, and
  `abi-audit` re-derives the nine pinned size identifiers and all fifteen `account_len` constants
  from the Rust source with no drift;
- the landed order page is exactly its header plus sixteen 228-byte slots and one slot is exactly
  a kind byte plus the widest admitted record body, a 65-order book is refused, and no landed
  relation row exceeds `MAX_ORDERS = 64`;
- `n = 24` is refused in the landed arm by the codec's own `check_count` bound as well as by V1
  policy;
- no landed or differential row carries a compute-unit field;
- golden JSON/CSV/Markdown and checksums reproduce byte-for-byte; and
- 29/29 unit tests pass (the original 12 plus 17 covering the landed and differential arms).

No RPC, validator, wallet, key, purchase, deployment, root manifest, or external state was used or
changed.

## Note (2026-08-19, post-repair)

Numeric values in the addendum sections above that describe OrderPage v3
(3,883-byte pages, 228-byte slots, 1,304-byte terms, PlaceOrder 165,
CancelOrder 130, MAX_INTENT_BYTES 256) are superseded by the v4/terms-v3
re-pin; `benchmarks/constants.json` and the golden matrix are the truth,
verified by the now-hardened `abi-audit` on every run. The gate was dead
(erroring, not reporting) between commits 927d4bc and this repair; the
34 drift lines it owed are recorded in the repair commit.
