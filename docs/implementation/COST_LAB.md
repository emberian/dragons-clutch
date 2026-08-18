# Cost lab implementation

Status: offline deterministic harness implemented and validated on 2026-08-18  
Source-constant snapshot: 2026-08-17  
Owned path: `benchmarks/`

## Outcome

The cost lab now generates 193 deterministic scenarios covering outcome counts
`n = 2, 4, 8, 16, 24`; internal split, fully external split, one-Egg materialization and
all-Egg materialization; legacy inline and v0+ALT wire layouts; 4/8/10 KiB dense order pages;
terminal/TWAP/full accumulator summaries over 1/4/16 pages; and batch verification over
32/128/512 alternating single-Egg/portfolio orders.

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
python3 -m unittest discover -s benchmarks/tests -v
```

The checked artifacts are:

- `benchmarks/golden/matrix.json`: all inputs, outputs, evidence labels, admission result, caveats,
  source pins, and exact harness/constants file digests;
- `benchmarks/golden/matrix.csv`: compact comparison surface;
- `benchmarks/golden/SUMMARY.md`: deterministic selected tables; and
- `benchmarks/golden/checksums.sha256`: closure over the three derived artifacts.

Generation uses Python's standard library, exact integers, no random input, and no timestamp in
rows. It performs zero RPC calls, validator calls, signatures, submissions, account mutations, or
package downloads. `check` regenerates the bytes in memory and refuses any golden drift.

## Current findings

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
  record, and `80 + 8n` byte portfolio record;
- 120/160/272-byte terminal/TWAP/full summaries and their scalar combine counts; and
- one signature, one top-level Dragon instruction, existing destination accounts, no compute-budget
  instruction, and no ATA creation.

They are centralized in `benchmarks/constants.json`, labeled `layout_hypothesis`, and guarded by
goldens so any change is explicit. Actual fixed ABI work should replace them, not quietly add a
second source of truth.

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

- 193 unique scenario IDs with exact family counts;
- every emitted transaction length equals the independent analytical sum;
- every `n=24` row is refused by V1;
- every external split reports `n+1` Token CPIs and `n+2` trace entries;
- every batch row authenticates exactly its input order count;
- short-vector boundary encodings and non-splitting page packing have adversarial tests;
- golden JSON/CSV/Markdown and checksums reproduce byte-for-byte; and
- 12/12 unit tests pass.

No RPC, validator, wallet, key, purchase, deployment, root manifest, or external state was used or
changed.
