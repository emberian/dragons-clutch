# Computational and capital cost model

## 1. Objective

The target is not merely low Rust instruction count. It is the cheapest trustworthy
and composable mechanism permitted by the information and state-transition lower
bounds. Measure:

- serialized transaction bytes;
- account count and writable locksets;
- top-level and CPI instruction trace entries;
- actual and requested compute units;
- account-data bytes and refundable rent principal;
- priority fees under contention;
- required authenticated information over time;
- user transaction count and failure recovery.

All constants in this document are design-time snapshots and must be remeasured
against the pinned release toolchain.

## 2. Three lower bounds

### Standard outcome assets

An ordinary fungible outcome identity requires its own Token-2022 mint. Therefore
`n` outcomes require `n` mint accounts and permanent `Omega(n)` mint rent. A fully
external complete-set split performs one collateral transfer and `n` mint CPIs;
a complete external merge performs `n` burns and one transfer. Dragon cannot
directly mutate Token-2022-owned balances or supplies.

This is the price of ordinary SPL composability, not an Anchor artifact.

### Path-dependent evidence

If the authenticated source does not retain history, distinguishing `T` path
intervals requires `Omega(T)` accepted observations. Supporting arbitrary future
predicates over `b`-bit prices requires `Omega(T*b)` information in the worst case.
Sharing across `M` Markets improves `O(M*T)` work to `O(T+M)`, never to O(1).

### Trustless batch clearing

Without a succinct proof, a deterministic result over `m` ordinary orders must
authenticate every order and therefore requires `Omega(m)` verification. An
offchain solver may propose a result but cannot eliminate that lower bound.

## 3. Hybrid claim design

The canonical design keeps all outcome mints but uses compact internal Positions
for the common path. At `n=16`, a minimal Position is roughly a small header plus
16 `u64` balances, far less state than 16 user token accounts. A fixed market-local
SupplyLedger conservatively accounts for internal and materialized supply; native
trades conserve those totals and do not write it. The operations are:

| Operation | Token CPIs | Local work |
|---|---:|---:|
| Internal split | 1 collateral transfer | O(n) fixed stores |
| Materialize one Egg | 1 mint | O(1) |
| Dematerialize one Egg | 1 burn | O(1) |
| Internal merge | 1 collateral transfer | O(n) fixed stores |
| External one-hot redemption | 1 burn + 1 transfer | O(1) |
| Fully external split | 1 transfer + n mints | O(n) external writes |

External users can compose `materialize + swap` in one transaction where limits
permit. Native simplex trading avoids Hoard and outcome-mint locks entirely after
the initial internal split.

The main risk is conservation ownership: the protocol cannot scan all internal
balances. Eggcrate must own every Position, SupplyLedger, and order escrow
transition. A separately upgradeable venue may request a conservation-preserving
kernel transition through CPI, but must never write canonical balances directly.
Direct external burns are safe donations; an authenticated reconciliation may
only reduce accounted external supply to the canonical mint's observed supply.

## 4. Outcome count and transaction envelope

At design time, Solana v0 transactions have a 1,232-byte packet limit, 64 runtime
accounts, a 64-entry top-level-plus-CPI instruction trace, and a 1.4 million CU
ceiling. A complete external split uses about `2n+5` unique accounts and `n+2`
trace entries. Address lookup tables help wire size, not account locks or CPI work.

Freeze `MAX_OUTCOMES = 16` for V1. It provides categorical expressiveness while
leaving composition and safety headroom. A future transaction format must be
feature-detected and benchmarked before raising the bound. More outcomes require
paginated overcollateralized issuance receipts and lose atomic full external
materialization.

Official references:

- [Solana transactions](https://solana.com/docs/core/transactions)
- [Compute budget](https://solana.com/docs/core/fees/compute-budget)
- [Cross-program invocation](https://solana.com/docs/core/cpi)

## 5. Account strategy

- One Hoard per Market: local theorem and parallelism outweigh refundable rent.
- Canonical outcome mint PDA: `['egg', market, outcome_index]`.
- Market owns the mint-signing PDA/bump; no separate human authority.
- Fixed raw layouts; explicit version/tag/length checks.
- Store validated PDA bumps and use `create_program_address` rather than searching
  on every hot instruction.
- No realm-global writable collateral or reward vault.
- Require destination token accounts to exist before external split/materialize;
  account creation is a separate user-funded operation.
- Keep metadata out of hot accounts. Outcome semantics live in the immutable
  Market terms digest.

Bare outcome mints are sufficient for SPL identity. Immutable in-mint TokenMetadata
is a one-time product/UX cost and must be decided at mint initialization. It does
not create Jupiter routing; a supported venue and sufficient liquidity do.

Token-2022 references:

- [Token-2022](https://www.solana-program.com/docs/token-2022)
- [Wallet and ImmutableOwner behavior](https://www.solana-program.com/docs/token-2022/wallet)

## 6. Accumulator at the information bound

Restrict V1 path metrics to an associative interval-summary family. A FeedHead
maintains current coverage, conservative bounds, first/last, price-time integral,
extrema, squared-return terms, and a bounded drawdown summary. Each accepted
observation updates O(1) state. Aligned page boundaries seal compact summaries;
raw observations need not remain indefinitely.

Recommended storage hybrid:

1. fixed FeedHead/current summary;
2. compact page summaries retained for maximum market horizon plus repair grace;
3. permissionlessly produced immutable WindowResults;
4. safe recycling only after no live Market can reference the page.

A fixed ring alone is cheaper but requires resolution before overwrite. Raw append
history preserves arbitrary future queries but locks rent proportional to every
sample. The hybrid preserves exactly the frozen metric family and refuses other
claims.

Threshold-independent summaries can prove sampled extrema/crossing. They cannot
answer arbitrary “above H for k consecutive fine buckets” queries without raw
history, a threshold-specific registered automaton, or coarser semantics.

Ordinary update hot set:

- keeper signer, writable;
- FeedConfig, read-only;
- authenticated source accounts, read-only;
- FeedHead, writable;
- internal KeeperCredit, writable.

Parse authenticated source state directly rather than invoking the source program.
Accrue rewards internally and claim them in batches rather than taking a Token CPI
and shared vault lock per observation.

## 7. Simplex auction at the verification bound

Use 8 KiB dense append pages, sharded by Market/Epoch to admit intents in parallel.
A compact single-Egg order contains Position identity, outcome, quantity, limit
tick, side/flags, and status/index. A portfolio intent additionally references a
canonical fixed payoff vector and one scalar size/limit. Dense pages amortize
Solana's fixed account-rent overhead far better than one PDA per order.

There is no independent clearing tick per outcome. A solver proposes an integer
price vector `p[0..n)` with nonnegative components summing exactly to
`PRICE_SCALE`, plus fills and any virtual complete-set splits/merges. Paginated
onchain verification scans every frozen page, checks limits and portfolio dot
products, and accumulates conservation for collateral and each Egg. An invalid
proposal loses a bounded bond. Candidate search is offchain and permissionless;
candidate validation is onchain and authoritative.

For single-Egg orders plus complete-set conversion, the host laboratory should
compare a separable/dual-price solver to exhaustive small-book enumeration. For
the admitted proportional portfolio language, it should compare against an exact
rational LP oracle where the relaxation is sound. Unrestricted all-or-none basket
matching is not assumed tractable and is rejected in V1. Without a succinct proof
or a compact primal/dual certificate, authenticating `m` orders and `n` conserved
assets costs at least `Omega(m+n)` and ordinarily `O(m*n)` arithmetic.

The public score is lexicographic—feasibility first, then executable surplus or
matched risk transfer, then deterministic tie-breakers. A strictly better valid
candidate may replace the current head during a bounded window. This selects the
best valid *submitted* candidate; it is not a false claim of global optimality.
Marginal fills use a frozen proportional/remainder rule. A second deterministic
pass writes page-local fill totals and pots. Pages remain inactive until the Epoch
atomically becomes Final; afterward users settle lazily with a Position and its
order page. This avoids a global settlement lock.

Offchain signed orders reduce rent but introduce withholding and signature cost.
Commit/reveal adds a hash, reserved funds, second transaction, reveal state, and
non-reveal semantics. It is a privacy/last-look option, not a cost optimization.

## 8. Accidental costs to remove

- no Anchor in the final hot path;
- no Borsh, `Vec`, `String`, dynamic dispatch, float, or formatted logging;
- no target-dependent `usize` financial arithmetic;
- no runtime PDA search;
- no ATA creation inside split/materialize;
- no per-fill token transfer;
- no per-observation keeper token transfer;
- no global writable market registry;
- no metadata or URI parsing in consensus;
- no requested compute limit based on a generous constant rather than simulation.

Solana's own optimization guide illustrates the scale of stored-bump and zero-copy
savings: [compute optimization](https://solana.com/developers/guides/advanced/how-to-optimize-compute).

## 9. Required benchmark matrix

Before choosing byte layouts or claiming cheapness, measure with the exact pinned
SBF and Token-2022 versions:

- outcome counts 2, 4, 8, 16, and refusal at 17;
- Realm/Market initialization and bare versus immutable-metadata mints;
- internal and external split/merge;
- materialize/dematerialize one and all outcomes;
- one-hot and rational-vector redemption;
- existing versus missing destination account;
- v0 inline addresses versus lookup table;
- accumulator terminal/TWAP/full-summary updates and page rollovers;
- 1, 4, and 16-page Window folds;
- order pages of 32, 128, and 512 single/portfolio intents;
- outcome counts 2, 4, 8, and 16 under simplex candidate verification;
- valid, inferior, invalid, withheld, and tied candidate proposals;
- candidate quality against exhaustive/LP small-book oracles;
- virtual complete-set conversion, allocations, and lazy settlement;
- one versus 8/16 order shards;
- lock-contention load with local versus accidentally shared vaults;
- annotated versus erased Verus source and unannotated baseline.

CI records transaction size, accounts, trace entries, actual/requested CU, stack,
heap, ELF size, account-data delta, rent, and landing priority. Fail on protocol
ceilings or unexplained 5–10% regression. Do not publish synthetic CU constants as
network guarantees.
