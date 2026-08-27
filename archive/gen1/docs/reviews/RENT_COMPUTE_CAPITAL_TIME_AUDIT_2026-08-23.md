# Rent, compute, transaction, and capital-time audit — 2026-08-23

Status: **BOUNDED QUANTITATIVE REVIEW / OFFLINE CALCULATOR / NO CURRENT LINKED
ELF OR CU CLAIM**

## Result first

The largest safe near-term state reduction is still active-width General
candidate state. At the recorded small geometries, one ClearWork projection
saves `332,186,880` lamports and one CandidateFeed projection saves
`41,780,880` lamports. Receipt pages can save `935,201,280` lamports at 416
simultaneously live receipt/ledger pairs, but only if page lifetime and write
contention are acceptable. A page locks more principal than six individual
pairs and less than seven, so a page is not automatically a win.

Historical capability-profile artifacts also show that program selection is a
first-order rent tool: at one pinned commit, the General/SourceV2/point profile
locked `4,508,409,600` fewer lamports than the full profile. That is historical
artifact evidence, not a size or rent claim about the present working tree.

Counted retirement has the opposite short-term shape: it adds small fixed
tails and writable-parent costs while accounts are live, in exchange for safe
later refunds and permanent replay protection. Series prepayment likewise
cannot reduce capital-time for free. Sharing one funding owner can reduce
account rent, but reducing how long future allocations are locked either
shortens the Series, moves activation later, or weakens the prepayment
guarantee.

There is no exact account-rent, transaction-width, compute, stack, or crank
number yet for a live Product/Series, covered dealer, evidence-only recovery,
or counted-retirement SBF adapter. Their present evidence is a pure-core model,
codec proposal, or integration seam. Treating Rust structure sizes or host-test
timings as Solana measurements would be false precision.

## Evidence grammar and reproduction

Every quantitative claim in this review has one of three labels:

- **HISTORICAL_ARTIFACT** — measured against the named older ELF, commit, or
  sealed evidence record. It is not silently promoted to the current tree.
- **SOURCE_DERIVED** — exact integer arithmetic over a named and digest-pinned
  source constant or codec formula. It is not a cluster quote.
- **MODEL_ONLY** — proposed account format, transaction composition, or
  illustrative economic projection. It must be measured after an adapter
  exists.

The standard-library-only calculator refuses booleans, floats, negative sizes,
invalid geometries, noncanonical Series schedules, and final-slot overflow. It
also checks SHA-256 digests for the sources used by its frozen examples.

```sh
python3 scripts/rent_capital_time_audit.py --check
python3 -m unittest scripts/test_rent_capital_time_audit.py
python3 scripts/rent_capital_time_audit.py
```

The last command emits JSON and performs no writes. The calculator does not
build SBF, inspect wallets, contact RPC, or infer a current ELF from historical
evidence.

The arithmetic rent model is:

```text
minimum_balance(data_bytes) = (data_bytes + 128) * 6,960 lamports
```

It is the pinned default-local-runtime model recorded by the 2026-08-22 state
audit. Every removed data byte changes modeled rent principal by exactly 6,960
lamports; removing a whole account also removes the 128-byte overhead. A fresh
validator run must re-read its rent sysvar before making a runtime claim.

## General V1 state and active-width successors

The existing account widths below are **SOURCE_DERIVED** layout facts. The
successor ClearWork body is **SOURCE_DERIVED** from the exact active-width
relation codec. CandidateFeed V2 is **MODEL_ONLY** until a fixed hostile-byte
codec and live versioned route land.

| Object | Existing bytes | Existing modeled rent | Successor geometry | Successor bytes | Successor modeled rent | Saving |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| ClearWork | 50,054 | 349,266,720 | `O=2,N=4,U=3` | 2,326 | 17,079,840 | 332,186,880 |
| CandidateFeed | 6,266 | 44,502,240 | `O=2,N=2,S=1` | 263 | 2,721,360 | 41,780,880 |
| CandidateFeed | 6,266 | 44,502,240 | `O=2,N=4,S=1` | 279 | 2,832,720 | 41,669,520 |

The exact active ClearWork body is:

```text
body(O,N,U) = 678 + 73N + 68U + 336O + 16NO + 16UO
account(O,N,U) = 160 + 32U + body(O,N,U)
```

The proposed feed is:

```text
feed(O,N,S) = 218 + 8O + 8N + 13S
```

At the admitted maximum `O=16,N=64,U=64`, active ClearWork is exactly the
existing `50,054` bytes. At `O=16,N=64,S=416`, the feed projection is exactly
the existing `6,266` bytes. The successor therefore removes inactive padding;
it does not lower the protocol's 16-outcome, 64-order, or 416-slice ceilings.

The frozen headline `1,121,903,280`-lamport saving for three candidates adds
the separately recorded four-order ClearWork and two-order feed comparisons.
It is useful component evidence, not one coherent market scenario. Using one
coherent `O=2,N=4,U=3,S=1` geometry gives `1,121,569,200` lamports for three
candidates. The calculator keeps `feed_orders` explicit so these cannot be
silently conflated.

Safe implementation requires a fresh account version, immutable active counts,
exact-length decoding, zero/trailing-byte refusal, canonical reconstruction of
omitted padding, and byte-for-byte equivalence at maximum width. A variable
allocation without those rules would exchange rent for malleability.

## Receipt pages, funding records, and lifetimes

The current 217-byte receipt plus 85-byte funding ledger locks
`3,883,680` lamports. A **MODEL_ONLY** 3,632-byte, 16-entry ReceiptPage locks
`26,169,600` lamports.

```text
6 individual pairs = 23,302,080 lamports  < one page
7 individual pairs = 27,185,760 lamports  > one page
```

Thus seven simultaneously live entries are the strict rent crossover. At 416
live receipts, 26 pages lock `680,409,600` lamports instead of
`1,615,610,880`, saving `935,201,280`.

This comparison is sensitive to lifetime, not just cumulative throughput. If
six entries remain after ten others become terminal, an individually closeable
layout may release more principal earlier. A page successor therefore needs:

- an exact active/terminal bitmap and exhaustive count;
- per-entry replay identity and settlement finality;
- a page close rule that cannot strand a live endpoint;
- a measured same-page write-contention envelope; and
- adversarial tests at occupancies 0, 1, 6, 7, 16, and across the 16/17-page
  boundary.

Embedding a mandatory 56-byte funding tail into a new governed account instead
of retaining a separate 85-byte ledger is another **MODEL_ONLY** exact-size
comparison. For one receipt it would lock `2,790,960` lamports rather than
`3,883,680`, saving `1,092,720`. This is safe only when every construction path
must persist the payer, refundable principal, and donation disposition. Making
the ledger optional again would lose deletion authority.

## Product codecs and compressed semantic ownership

The Product/Series pure core freezes exact semantic bodies, not live Solana
account codecs. Applying account overhead to each body as if it were separately
persisted is therefore **MODEL_ONLY** and must not be totaled as current market
rent.

| Pure artifact | Body bytes | Modeled standalone rent | Evidence boundary |
| --- | ---: | ---: | --- |
| NativeClaimBasis V1 | 2,352 | 17,260,800 | fixed pure codec; no SBF account |
| RecoveryPolicy V1 | 208 | 2,338,560 | fixed pure codec; no SBF account |
| ProductTemplate V4 | 256 | 2,672,640 | fixed pure codec; no SBF account |
| MarketGenesisProfile V1 | 352 | 3,340,800 | fixed pure codec; no SBF account |
| SeriesFundingQuote V1 | 264 | 2,728,320 | fixed pure codec; no SBF account |
| SeriesAttachmentPlan V1 | 112 | 1,670,400 | fixed pure codec; no SBF account |
| SeriesPlan V4 | 152 | 1,948,800 | fixed pure codec; no SBF account |
| SeriesFundingTerms V1 | 208 | 2,338,560 | fixed pure codec; no SBF account |

The biggest obvious codec target is the basis. A **MODEL_ONLY** active codec
that reconstructs all omitted V1 padding has body width:

```text
degree 0: 32 + 8 * payout_count * outcomes + outcomes + 16 * knot_count
degree 1..3: 32 + 16 * knot_count
```

For a binary, two-payout, degree-zero basis this is 82 bytes and
`1,461,600` modeled standalone lamports, `15,799,200` below the 2,352-byte
body treated the same way. This optimization is safe only if the compressed
body remains the sole semantic owner, its content ID is versioned, active
rows/knots are exhaustive and canonical, and every omitted field has one exact
reconstruction. Repeating the expanded V1 body in Terms or Market would erase
the saving and create parallel truths.

## Series prefunding and capital-time

For one separately denominated allocation `A`, a fully prepaid finite Series
of `N` instances activated at slot `t0`, first debited at `t1`, and spaced by
`d` slots has **MODEL_ONLY** locked capital-time:

```text
A * [N * (t1 - t0) + d * N * (N - 1) / 2]
```

The calculator names the result `atom-slots`. Lamports, collateral atoms,
source-work lamports, recovery lamports, and liquidity collateral must be
computed and reported as separate compartments; they cannot be added without
a price convention. For the deliberately small fixture
`A=7,N=4,t0=10,t1=20,d=3`, the exact result is 406 atom-slots.

The formula measures the opportunity cost of the guarantee: later instances
remain fully capitalized before their debit. Packing all allocations into one
SeriesFunding account can remove repeated 128-byte account overhead and permit
atomic per-instance debits, but it does not reduce this capital-time. Safe ways
to reduce it are limited to:

- activate closer to the first creation window;
- choose shorter finite Series tranches and roll only by a new fully funded
  activation; or
- lower an immutable component quote after measured route-cost reductions.

Allowing future fees, Hoard principal, volume forecasts, or later sponsor
promises to fill the gap would weaken the guarantee and is not an optimization.

For comparison, the historical ResolutionWork minimum prefund is
`49,431,920` lamports. If all of it remained locked for the maximum 4,096-slot
lifetime, the upper capital-time is `202,473,144,320` lamport-slots, or
`202.473144320` SOL-slots. This is **HISTORICAL_ARTIFACT** arithmetic over the
sealed prefund and lifetime constants, not evidence that every execution holds
the whole amount for every slot.

## Counted retirement: live overhead versus terminal release

The retirement crate is a production-bound pure seam with no live route. The
following is **SOURCE_DERIVED** size arithmetic over its current proposed
constants and **MODEL_ONLY** rent application. It is not a wire allocation or
SBF measurement.

| Family | Existing bytes/rent | Proposed live bytes/rent | Live increment | Terminal form/rent | Principal released from proposed live form |
| --- | ---: | ---: | ---: | ---: | ---: |
| Position | 220 / 2,422,080 | 280 / 2,839,680 | 417,600 | 76 / 1,419,840 tombstone | 1,419,840 |
| General Epoch | 329 / 3,180,720 | 429 / 3,876,720 | 696,000 | 84 / 1,475,520 tombstone | 2,401,200 |
| Market | 726 / 5,943,840 | 734 / 5,999,520 | 55,680 | retained | 0 |
| Reservation, count only | 618 / 5,192,160 | 627 / 5,254,800 | 62,640 | deleted | 5,254,800 |
| Reservation, deletable funding owner | 618 / 5,192,160 | 675 / 5,588,880 | 396,720 | deleted | 5,588,880 |
| Replay projection | 84 / 1,475,520 | 132 / 1,809,600 | 334,080 | deleted | 1,809,600 |

The exact account version assignment is still moving across the retirement
README, ADR, live-promotion plan, and pure constants: the source currently
distinguishes count-only 627-byte and fully deletable 675-byte reservations,
while some prose calls the 675-byte shape V5/V6. This naming drift must be
resolved through the authoritative central registry before any route lands.
The economic comparison depends on the bytes, but activation safety depends on
one collision-free historical tag/version ledger.

Retirement is not merely a rent feature. It adds writable root accounts to
child create/close transactions and may increase CU, account metas, lock
contention, and serialized width. Those costs are currently unknown. The safe
slice is the already-selected monotone identity plus exhaustive disjoint child
counts, exact payer/donation ownership, atomic count mutation with child
creation/deletion, and a persistent tombstone where replay requires it. Do not
re-enable legacy closes to capture the modeled refund.

## Dealer and evidence-only recovery

The covered signed dealer has an exact host economic model but no account
codec, custody route, SBF instruction, or rent/fee budget. Its present capital
requirements are semantic rather than deployment measurements:

- LP cash and existing backed Eggs are present before activation;
- sponsor capital satisfies both the curve-loss bound and lower-corner cash
  financing;
- Egg custody covers the entire signed sale box; and
- keeper rewards and rent require separately prepaid compartments.

No generic lamport amount follows without a chosen outcome width, depth, signed
box, share supply, unit basket, and account inventory. The next useful number is
not `size_of` a host struct; it is an exact hostile-byte account design followed
by blank-validator rent, meta, packet, CU, and custody-delta measurements.

Evidence-only recovery likewise has a fixed pure phase/conservation model but
no persisted Solana account codec. Its FundingQuote fixes up to eight progress
caps/rates and a distinct recovery-rent principal. Account rent, number of Work
accounts, attempt crank count, and transaction width remain **UNMEASURED** until
one adapter owns the state carrier, reserve, evidence joins, Clock mapping, and
terminal deletion. The safe implementation must keep work principal, rent,
donations, and collateral disjoint and preserve late-evidence resolution after
the expendable reserve closes.

## Historical ELF, stack, CU, and packet evidence

Nothing in this section is current-tree evidence.

### Loader rent

For loader-v3 exact-size allocation, the calculator uses:

```text
ProgramData data bytes = 45 + max_elf_bytes
Program data bytes     = 36
persistent rent        = rent(45 + max_elf_bytes) + rent(36)
```

| Historical artifact | ELF bytes | Persistent loader rent | Difference from full |
| --- | ---: | ---: | ---: |
| Full capability profile at `625cd65…` | 2,083,112 | 14,500,805,040 | 0 |
| General + SourceV2 + point at `625cd65…` | 1,435,352 | 9,992,395,440 | -4,508,409,600 |
| Direct V3 + SourceV2 + point at `625cd65…` | 1,056,864 | 7,358,118,960 | -7,142,686,080 |
| Unsealed engineering artifact `193c0872…` | 2,082,320 | 14,495,292,720 | separate source closure |

Capability profiles safely reduce deployment rent only when the release
manifest binds the admitted instruction/source/resolution surface and disabled
tags refuse before account reads. Splitting programs can reduce each binary but
adds CPI, account-meta, upgrade, and atomicity boundaries; it must be compared
as a measured system, not chosen from ELF size alone.

The `193c0872…` final-LTO audit historically found deepest direct `r10` access
at 4,096 bytes and no out-of-frame direct access. Capability-profile builder
diagnostics explicitly were not final-ELF reachability proof. A present-tree
stack claim requires a fresh linked ELF and the same final-symbol/disassembly
audit.

### CU and sendability

Pinned historical evidence includes:

| Route/shape | Historical result | Boundary |
| --- | ---: | --- |
| FreezeEpoch, 4 pages / 64 orders | 988,469 CU | historical sealed shape row |
| EntitleSlice, 4 pages | 759,892 CU | page-set-wide cost |
| direct Entitle, 416-slice witness | 803,935 CU | unsealed current-tree campaign at the time |
| FoldBatch(6) | 486,413 CU / 1,216 bytes | six fit legacy packet |
| FoldBatch(7) | 1,347 bytes | did not fit 1,232-byte legacy packet |
| FoldBatch(12) | 2,002 bytes | bank-computable but unsendable in that plan |

This shows why CU alone cannot select a crank batch. The sendable historical
32-fold plan was `[6,6,6,6,6,2]`, six transactions. A later unsealed record-dense
plan measured two Fold transactions at `514,332 CU / 1,228 bytes` and
`171,765 CU / 704 bytes`; it remains unsealed historical engineering evidence.

## Account-presentation and crank-count model

No current message builder closes the complete future V2/Product/Series/dealer/
recovery/retirement transaction surface. The following is an explicitly
**MODEL_ONLY**, optimistic General composition, not serialized-byte evidence:

```text
setup actions              = N + P + 2
per-candidate actions      = 4 + ceil(N/24) + ceil(S/16) + 2P + ceil(S/b)
selection/settlement       = 2 + 2S
total(P,N,S,C,b)           = setup + C * per-candidate + selection/settlement
```

Here `P` is pages, `N` orders, `S` slices, `C` candidates, and `b` slices per
settlement batch. It counts logical account presentations/actions under the
model; it does not prove that actions can share a transaction or that repeated
metas remain distinct after message-key deduplication.

| Shape | Modeled total |
| --- | ---: |
| `P=4,N=64,S=32,C=1,b=8` | 157 |
| `P=4,N=64,S=416,C=1,b=8` | 997 |
| same, optimistic ABI-wide `b=416` | 946 |
| same ABI-wide batch with `C=3` | 1,030 |

The dominant term is endpoint settlement `2S`; active-width account storage
does not solve that transaction surface. Receipt pages can reduce account-key
fanout only if an instruction updates several entries per writable page and
the packet/CU/lock measurements agree. Counted retirement adds at least the
writable counter parent to governed child transitions. Series, recovery, and
dealer adapters add funding/custody owners whose account metas cannot be
optimized away without losing authentication.

## Ranked implementation and measurement queue

1. **Fresh current-tree artifact profiles.** On the pinned local builder, build
   full, General-only, and Direct-only profiles from clean targets. Record
   commit/tree, feature set, ELF/text bytes and hashes, loader allocation,
   source closure, final-LTO stack audit, and disabled-tag refusals. This is the
   only way to replace the historical ELF table.
2. **Freeze exact message builders before choosing batch widths.** For maximum
   General shapes and every proposed V2 route, record unique/read/write/signer
   metas, instruction bytes, legacy and versioned-message packet bytes, address
   lookup assumptions, and rejection at the actual packet boundary.
3. **Measure V1 and active-width V2 side by side.** After new codecs exist, run
   small, mid, and exact maximum ClearWork/Feed geometries. Record create/grow/
   append/seal/verify/close CU, peak heap/stack evidence, transaction count,
   rent, and byte-equal semantic projections. Maximum-width V2 must match V1.
4. **Test ReceiptPage lifetime, not just capacity.** Drive occupancies 1, 6, 7,
   16, 17, and 416; early terminal holes; competing same-page writers; paired
   settlement; restart; close; and late rollback. Compare capital-time as well
   as final rent.
5. **Promote counted retirement only as an atomic vertical slice.** Resolve the
   reservation version-name drift, centralize tags, then measure every
   counter-bearing create/close at maximum metas and packet width. Inject a
   late failure after each mutation and require byte/lamport rollback. Never
   infer safe deletion from a zero host-model count alone.
6. **Exercise whole-Series funding on a blank validator.** Measure activation,
   duplicate Instance convergence, occurrence debit, lapse, refund, final-slot
   overflow refusal, and per-compartment capital-time for singleton, two-item,
   and capability-maximum finite schedules. A shared funding account must not
   turn one compartment's surplus into another's authorization.
7. **Give recovery and dealer real account inventories.** Freeze exact codecs,
   payer/refund ownership, PDA/version coordinates, custody accounts, and
   terminal close rules before quoting rent. Then measure all eight recovery
   attempts and dealer adverse corners, partial fills, terminal redemption,
   retry, and hostile prefund/alias cases.
8. **Compare binary partitioning as a system.** Measure monolith versus
   capability profile versus multi-program composition including CPI CU,
   extra metas, atomic rollback boundaries, deployment/upgrade liquidity, and
   total persistent rent. Adopt the smallest verified capability surface, not
   the smallest ELF in isolation.

The first four experiments rank highest because they can change billion-
lamport state decisions or packet feasibility. Dealer and recovery economics
are meaningful, but quoting their deployment cost before their account
inventories exist would not help choose an architecture.
