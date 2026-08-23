# Product compiler and recurring Series V1

Status: **PROPOSED / EXECUTABLE HOST MODEL / NO SBF ROUTES** (2026-08-22)

Executable model: [`research/product-compiler-v1`](../../research/product-compiler-v1)

## Result first

The intended Template/Instance/Series architecture is coherent, but it cannot
be implemented as a thin wrapper around the current source plane.

The host model now defines deterministic canonical artifacts, exact recurring
window arithmetic, finite prepayment, deterministic Instance convergence,
shared raw-window identities, current Terms/Market compatibility lowering, a
liquidity-policy bridge, and an associative conservative maximum-drawdown
summary. It deliberately does not add SBF dispatcher routes.

Current source ingestion is a hard recurrence blocker:

- the singleton Feed PDA is keyed only by Feed identity;
- its account stores one Realm, cursor, next boundary, archive count, and a
  summary initialized from one exact Terms digest;
- initialization requires the cursor to equal that Terms' window start;
- append/seal advances the singleton state;
- a second recurring window cannot restore the first-window precondition, and
  another Realm cannot share the singleton.

Accordingly `HatcheryProgramV1::validate_recurring` admits only a reviewed
source-plane generation `>= 3` with all of these exact capabilities:

1. a Realm-neutral, source-only monotone FeedHead;
2. reusable raw observation pages written once;
3. immutable exact windows over ordered page slices; and
4. statistic-specific result children derived from a shared raw window.

The current V1/V2 plane gets `CurrentSourcePlaneNotRecurring`. This refusal is
not weakened by the fact that one isolated current Terms account can still be
compiled and exercised.

## Two compilers, not one

The product compiler and payoff-shape compiler remain separate:

```text
product compiler
SourceSpec -> raw Window -> Statistic -> Partition/Basis -> Template
               |                                     |
               +-> exact Instance/Terms -------------+

payoff compiler
human payoff shape -> exact/certified Egg coefficient vector -> order
```

The first creates state-contingent asset semantics. The second creates a
portfolio over those already-defined assets. A put, tent, range, or Gaussian
shape is not another state partition.

## Normalized semantic ownership

| Fact | One semantic owner | References/projections |
| --- | --- | --- |
| Provider, deployment, asset, orientation, normalization, grid, freshness, confidence | immutable SourceSpec | Template references SourceSpecId |
| Raw-page/window lifecycle and retention rules | HatcheryProgram | Template references HatcheryProgramId |
| Statistic evaluator and retained feature set | SummaryProgram | Template references SummaryProgramId |
| Relative span, statistic, partition/basis, payouts, ambiguity, edge, repair, failure | Template | Instance/Terms reference TemplateId |
| Human explanation and labels | presentation sidecar | `TemplatePresentationId = H(TemplateId, sidecar digest)` |
| Realm/Profile, price/fee policy, recurrence, cap, work/liquidity policy references | immutable SeriesPlan | Instance descriptor derives from them |
| Exact raw source window | Hatchery WindowKey | many statistic results and Instances reference it |
| One derived statistic result | StatisticResult | one or more compatible Templates reference it |
| Liability and lifecycle | Instance/Market | clients and indexers project it |
| Remaining prepaid creation/work/liquidity balances and next ordinal | SeriesFunding | keeper UI projects it |
| Passive-liquidity risk and quote-generation parameters | LiquidityBlueprint | per-Instance LiquidityPolicy binds Market/Terms |

Human presentation is committed but is not part of semantic equivalence. A
label correction changes `TemplatePresentationId`, not `TemplateId` or an
existing market's economics.

## Canonical identity graph

```text
SourceSpecId ─────────────┐
HatcheryProgramId ────────┼─> RawWindowId
exact range/coverage/gen ─┘        |
                                  + SummaryProgramId + Statistic
                                  v
                           StatisticResultId

SourceSpecId + HatcheryProgramId + SummaryProgramId
 + relative window recipe + statistic + partition/payouts/policies
                                  -> TemplateId

TemplateId + Realm/Profile + price/fee/work/liquidity policies
 + finite bucket schedule         -> SeriesId

TemplateId + exact start + Realm/Profile + price/fee/work/liquidity
 + market collateral cap          -> InstanceId
```

`InstanceId` does not contain SeriesId, ordinal, creator, or a free nonce.
Two Series that schedule the exact same economic Instance converge on one ID.
The pure transition can authenticate the existing Instance and advance without
spending the second Series' reserved allocation.

The current adapter still demands `canonical_market_id(realm, profile, u64
nonce)`. Compatibility lowering deterministically derives that nonce from the
first eight InstanceId bytes. This removes caller choice but does not turn a
64-bit projection into an injective identity. A future Market/Instance account
must bind the full InstanceId; current deployment admission would additionally
need to reserve/refuse a truncation collision.

## Recurrence and prepayment

For ordinal `j`:

```text
start_j    = first_start_bucket + j * stride_buckets
end_j      = start_j + window_span_buckets
maturity_j = end_j + repair_grace_buckets
```

All operations are checked integers. The Series fixes a finite nonzero count,
creation lead, and the whole final range before activation. A caller may create
only `next_ordinal`, only in `[start - creation_lead, start)`. After the start,
anyone may lapse that ordinal so an expired item cannot block the rest of the
Series. Lapse spends nothing and leaves its allocation explicitly refundable.

Activation requires exact present funding for:

```text
instance_count * creation/rent allocation
instance_count * mandatory work/keeper allocation
instance_count * liquidity-blueprint tranche cap
```

The mutable state stores these as three segregated compartments. Instance
creation atomically debits one item from each. There is no future-fee input,
Hoard principal input, volume forecast, or implicit borrowing. A production
account design should split the work envelope further into source/archive,
auction, and resolution reserves when their exact route quotes are frozen.

Series collateral is passive-liquidity capital, not claimant backing. User
split/endowment collateral enters the market-local Hoard separately and remains
the sole backing of Egg liabilities.

## Shared terminal and drawdown surfaces

RawWindowId excludes statistic and SummaryProgram. Terminal and drawdown over
the same SourceSpec, exact range, coverage, generation, and Hatchery release
therefore share the same authenticated raw pages and immutable seal. Their
StatisticResultIds and TemplateIds remain distinct.

The model's drawdown feature is:

```text
ceil(1_000_000 * max_{i <= j, x_i > 0}(x_i - x_j, 0) / x_i)
```

It has one named rounding boundary and returns ppm in `0..=1_000_000`. For
interval observation points it retains the conservative pairwise enclosure.
An ordered summary stores low/high extrema and low/high drawdown; combining an
earlier range `A` with a later range `B` includes the exact cross terms
`A.max -> B.min`. Adversarial vectors prove chronology matters, interval
enclosures, integer-ceiling cases, adjacency refusal, and all parenthesizations
of representative paths.

This is a new summary family over reusable raw observations. It does not call
the older accumulator's `maximum_drawdown`, which correctly refuses because
that summary discarded ordering. Drawdown Template identity and modeling are
valid now; current `TermsAccount` lowering remains a typed refusal until an
onchain statistic registry and SourcePlane V3 implement the same semantics.

The host compiler also closes a naming gap in current failure policy: when
`FAIL-UNIFORM-REFUND-01` is selected, it requires the selected failure vector
to be positive and equal across every active outcome. Current runtime Terms
validation checks only the policy ID and payout index, so an untrusted compiler
could still label a nonuniform vector “uniform.” Runtime enforcement or an
honest explicit-preset rename is required before this compiler can be trusted
as a product admission boundary. Four categorical outcomes plus a uniform
failure vector fit the present eight-vector bound; eight one-hot outcomes plus
a ninth failure vector do not, which is a schema limitation rather than a
reason to truncate the failure state.

## Current compatibility lowering

For terminal Templates, the host compiler expands a normalized Instance into a
fully validated current `TermsAccount` v3 and deterministic current
`MarketAccount` projection. This is deliberately lossy architecture, not a new
semantic owner. Current Terms repeats:

- Realm/Profile and Feed;
- absolute window and maturity;
- source/grid/evaluator facts;
- complete payout, knot, and policy bodies; and
- the per-Instance collateral cap.

The current account is 1,656 bytes. Current Market is 726 bytes and includes a
512-byte outcome-ID array even though every entry is already derivable from
`(market,index)`. Normalization therefore identifies 2,168 bytes of repeated or
derived payload per recurring Instance (`1,656 + 512`) before accounting for
the compact replacement fields. This is a first-order removal opportunity,
not a measured net rent saving: the future Instance header, funding ownership,
child counters, and exact runtime account widths do not exist yet.

## Required onchain sequence

1. Design and land SourcePlane V3: source-only FeedHead, reusable raw pages,
   immutable WindowResult, statistic-result children, retention/lease rules.
2. Add fixed hostile-byte codecs for HatcheryProgram, SummaryProgram,
   Template, SeriesPlan/SeriesFunding, and compact Instance.
3. Bind the full InstanceId in Market identity and add a monotone market epoch
   cursor; do not retain caller-chosen market or epoch identities.
4. Give every Series compartment payer-principal/donation/terminal ownership
   and move one complete allocation into each child atomically.
5. Run host vectors through an independent implementation, current Terms
   decoding, SourceSpec decoding, local-bank creation/refusal, and shared-page
   terminal/drawdown resolution.
6. Measure actual account rent and CU before promoting an onchain profile.

No step requires shrinking the degree-0..3 basis, general clearing, occupation,
or future multidimensional product ambitions. Multidimensional partitions need
a typed Template variant and Terms/account generation with more than one
statistic/axis; current Terms v3 has one statistic and one knot vector, so the
current lowering must refuse rather than flatten such a product.
