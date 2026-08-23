# Exact frozen-order and candidate-adjacency index plane V1

Status: source-complete format, structurally disabled, not an instruction or
capability in any deployable profile.

## Purpose

`EntitleSlice` currently pays for two unrelated complete scans: locating dense
order ranks in the frozen page set and finding every selected slice involving a
pair. V1 moves those facts into an immutable, candidate-bound account pair:

- `FrozenOrderLocatorV1` maps each dense live-order rank to its canonical V5
  `(page_index, physical_page_slot)`; and
- `CandidateOrderSliceIndexV1` maps every dense order to a grouped active edge
  interval, candidate-wide entitled quantity, side, real-counterparty count,
  and virtual-edge count. Each edge retains the exact slice, counterparty class,
  counterparty rank, outcome, side, and positive Egg quantity.

The index does not contain owners, balances, mutable Reservation or Position
state, fees, price rounding, or a second clearing verdict. It accelerates lookup
of the best valid submitted candidate already selected by the counted root.

## One construction authority

The first construction recomputes, in order:

1. the complete hostile-decoded canonical OrderPage V5 set and its RelationV2
   projection;
2. the complete hostile-decoded sealed CandidateFeed V2 traversal;
3. the exact selected-feed/candidate/count/generation equality join to the
   counted `SettlementRootV1AccountV1`; and
4. the MarketBinding V2, EconomicDomain V2, PriceGrid, Genesis V2
   Realm/Profile/capability-profile, and collateral-profile joins.

Only after those checks does it derive locations, aggregate rows, and every real
slice end. A caller cannot provide any row, aggregate, semantic count, digest,
generation, or candidate identity. Page account IDs and V5 page-body digests
enter a fresh ordered page-set digest. The exact MarketBinding account ID and
canonical V2 body enter a separate digest.

The shared plane identity commits all semantic coordinates, both future account
IDs, both rent owners, every locator, every aggregate, and every active edge.
Each sibling also has a domain-separated exact active-body digest.

## Active geometry

Both bodies use a 664-byte V1-only sealed header. The header binds the Market,
Epoch, V5 order set, settlement candidate, selected feed account and bundle,
Realm, Profile, capability profile, Genesis profile, MarketBinding account and
body digest, EconomicDomain digest, exact page-set digest, plane identity,
sibling account, SettlementRoot account, owner/order-set digest, epoch
generation, all active counts, per-page populated-slot widths, stored bump, and
the exact deletable-rent owner.

The active lengths are:

```text
locator   = 664 + order_count * 4
adjacency = 664 + order_count * 32 + real_slice_end_count * 16
```

`real_slice_end_count` is one for a split or merge slice and two for a direct
slice, so it lies in `slice_count..=2*slice_count` and is at most 832. Inactive
fixed-capacity rows are never persisted. Decoders accept only V1, one exact
active width, zero reserved bytes, and canonical row ordering.

## Query and hostile-read invariants

A pair query reads two locator rows and only the two grouped adjacency ranges.
It returns the exact pair slice prefix, candidate-wide buy/sell entitled totals,
and whether either order trades with another real counterparty or a virtual
split/merge. It does not scan unrelated pages or unrelated witness slices.
The bounded sealed-account reader rechecks both constant headers, both local
locations and aggregates, every selected local edge, and direct-edge symmetry.
It requires an unforgeable root/account read authority; no constructor exists
until the counted-root successor and program-owner/PDA adapter join lands.

Standalone decoding rechecks:

- strictly increasing in-range physical page locations;
- contiguous aggregate edge directories and exact account-wide closure;
- strictly increasing slice indices within each order;
- side-correct split/merge use and zero virtual counterparty rank;
- reciprocal direct edges with identical slice, outcome, and quantity;
- exactly two opposite-side real ends for direct slices or one virtual edge for
  split/merge slices;
- distinct-real and virtual-edge counts; and
- equality of edge quantity totals and the selected entitlement aggregate.

## Rent, retirement, and the deliberate promotion refusal

Each body persists the exact payer, full rent-exempt principal paid without a
prefund discount, and hostile prefund donation floor. Terminal close returns
only principal to that payer and routes every other lamport to the immutable
MarketBinding neutral sink. Both siblings close atomically.

The current `SettlementRootV1AccountV1` does not count these two accounts. It is
therefore not lawful to create them merely because Root V1 later becomes
terminal. The runtime makes this structural: construction requires an
unforgeable `CountedExactIndexAdmissionV1`, closure requires an unforgeable
`CountedExactIndexRetirementV1`, and this source defines no constructor for
either. `EXACT_INDEX_PLANE_LIVE_ENABLED_V1` is false. There is no instruction,
PDA prefix, artifact discriminator, dispatch route, or profile capability.

Promotion requires a breaking counted-root successor that owns exact
expected/admitted/live/retired index-child counts and mints those private
capabilities only while atomically changing the corresponding count. It must
count exactly both siblings, not reuse receipt or Dealer counters. Until that
join exists, the private typed postwrites are review/test artifacts and no
persistent child is reachable.
