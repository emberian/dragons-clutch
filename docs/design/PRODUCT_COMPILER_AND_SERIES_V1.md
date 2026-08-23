# Product compiler and recurring Series V1

Status: **PROPOSED / EXECUTABLE HOST MODEL / NON-PRODUCTION SBF ARTIFACT CATALOG ONLY** (2026-08-23)

Executable model: [`research/product-compiler-v1`](../../research/product-compiler-v1)

Successor note (2026-08-23): the allocation-free
[`clutch-product-series`](../../crates/clutch-product-series) core now adds a
strictly versioned quantized price-policy cascade. `PriceMeasurePolicyV1`
selects the V3 degree-zero-through-three price-measure checker and its single
exact quantized evaluator/reconstruction semantics version. Degree zero keeps
the canonical finite payout table, including repeated cell mappings and
non-one-hot rows. `MarketGenesisProfileV2`,
`MarketInstancePreimageV2`, `SeriesPlanV5`, and `SeriesFundingTermsV2` use fresh
typed IDs, magics, domains, and schemas while preserving every V1 byte. No SBF
production route selects these artifacts. A separately identified
non-production SBF profile now publishes the nine frozen Product/Series bodies
through the existing resumable artifact transport. This is a typed immutable
catalog only: it does not register or activate a Series, authenticate registry
selectors, capitalize a funding account, compile an occurrence, or create a
Market. Runtime price-witness activation remains blocked on an authenticated
registry-selector and exact-price adapter join.

The laboratory artifact ABI is exact:

- artifact kinds `32..=40` correspond, in order, to
  `NativeClaimBasisV1`, `EvidenceOnlyRecoveryPolicyV1`, `ProductTemplateV4`,
  `PriceMeasurePolicyV1`, `MarketGenesisProfileV2`, `SeriesFundingQuoteV1`,
  `SeriesAttachmentPlanV1`, `SeriesPlanV5`, and `SeriesFundingTermsV2`;
- Begin/Write/Seal/Abort remain layout tags `18/19/20/21`, version `3`;
- the stage PDA remains
  `["dragons-clutch:upload:v1", funder, kind, zero-context, typed-digest]`;
- the final PDA is
  `["dc:product-artifact:v1", kind, typed-digest]`; and
- the final account is the exact canonical codec body, owned by the program
  and rent-funded by the uploader, with no wrapper, duplicate bump, or mutable
  registry projection.

Product artifact transport context is canonically zero because these typed
bodies are reusable content, not Realm children. Realm/Profile collateral
binding remains immutable inside `MarketGenesisProfileV2`; a future activation
route must authenticate that body against the actual Realm/Profile and central
registry before it may create liabilities. Ordinary profiles refuse kinds
`32..=40`; only `non-production-product-series-lab` admits them, under its own
capability-profile identity.

## Result first

The intended Template/Instance/Series architecture is coherent, but it cannot
be implemented as a thin wrapper around the current source plane.

The host model now defines deterministic canonical artifacts, exact recurring
window arithmetic, finite prepayment, deterministic Instance convergence,
shared raw-window identities, current Terms/Market compatibility lowering, a
liquidity-policy bridge, and an associative conservative maximum-drawdown
summary. The laboratory route above transports those artifacts without
claiming that the host compiler has become an onchain activation boundary.

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
| Realm/Profile, price grid, exact quantized price-measure policy, fee policy, lifecycle policies | immutable MarketGenesisProfileV2 | MarketInstanceV2 commits its typed ID |
| Recurrence, cap, work/liquidity/wrapper references | immutable SeriesPlanV5 and Attachment | Instance descriptor derives only the economic subset |
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

The concrete successor refines those shorthand rows as:

```text
PriceMeasurePolicyV1 body
  -> PriceMeasurePolicyV1Id

Realm/Profile + PriceGridId + PriceMeasurePolicyV1Id + fee/relation/score/
candidate-lifecycle/liveness/retirement/capability owners + bearer lot +
closed coordinate bounds
  -> MarketGenesisProfileV2Id

TemplateId + MarketGenesisProfileV2Id + exact start + collateral cap
  -> MarketInstanceV2Id

TemplateId + MarketGenesisProfileV2Id + AttachmentPlanId + finite recurrence/cap
  -> SeriesPlanV5Id
```

The current V1 Genesis lacks `PriceMeasurePolicyV1Id` and cannot authorize a
RelationV2 price-coherence route. It remains frozen rather than gaining a field
under the same 352-byte codec. The V2 Genesis accepts only the typed first
quantized policy, which covers Product degrees zero through three. A future
continuous/unquantized checker requires its own typed policy and another Genesis
successor; transparent 32-byte wrappers must never be cast across those
meanings. `NativeClaimBasisV1` owns the payout body and exact ambiguity/edge
registry selectors. Genesis V2 owns the closed coordinate minimum and maximum,
so `MarketInstanceV2Id` commits them transitively and no Epoch may choose a
different range. The registry owns the selector-to-semantics mapping. A live
adapter must authenticate that mapping before constructing the ephemeral
checker input. The price-measure `basis_digest` remains the exact
`NativeClaimBasisV1Id`; the Relation/EconomicDomain digest joins the committed
Market domain and per-Epoch facts rather than becoming a second owner.

Exhaustiveness further constrains a smooth basis whose authenticated registry
mapping resolves to `Refuse`: Genesis minimum and maximum must equal the first
and last stored knots inclusively. A broader domain is admitted only with
`Clamp`, which maps both exterior intervals to their nearest endpoint. The full
registry/Series join enforces this before compiling a Market occurrence.

`InstanceId` does not contain SeriesId, ordinal, creator, or a free nonce.
Two Series that schedule the exact same economic Instance converge on one ID.
The pure transition can authenticate the existing Instance and advance without
spending the second Series' reserved allocation.

The current adapter still demands `canonical_market_id(realm, profile, u64
nonce)`. Compatibility lowering deterministically derives that nonce from the
first eight InstanceId bytes. This removes caller choice but does not turn a
64-bit projection into an injective identity. A future Market/Instance account
must bind the full InstanceId. The Product successor therefore does not call
this lowering a compatibility route: a live V2 Market must persist the full
`MarketInstanceV2Id`. A separate collision registry could make legacy
admission fail closed, but it cannot make the 64-bit projection an economic
identity.

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
2. Add fixed hostile-byte codecs for HatcheryProgram, SummaryProgram, and a
   compact persisted Instance. Product Template, basis, price policy, Genesis,
   Series Plan, attachment, quote, and funding-terms codecs now have a
   non-production immutable SBF publication path, but there is still no
   authenticated Series registry or mutable funding state.
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
