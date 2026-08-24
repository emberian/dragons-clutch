# Compact exact settlement index plane V1

Status: source implementation in progress, structurally disabled, and absent
from every deployable capability profile.

## Purpose and semantic ownership

The compact plane accelerates two exact lookups without creating a second
owner of settlement facts:

- `FrozenOrderLocatorV1` maps every dense live order rank to its authenticated
  V5 `(page_index, physical_page_slot)`; and
- `CandidateOrderSliceIndexV1` stores a per-order directory followed only by
  grouped, unique `u16` indices into the retained CandidateFeed slice tail.

CandidateFeed remains the sole persisted owner of slice legs, counterparty,
outcome, quantity, and route. The compact adjacency does not repeat those
facts, entitlement totals, owners, balances, mutable Reservation/Position
state, fees, price rounding, or a clearing verdict. It indexes the best valid
submitted candidate already selected by the counted root.

## Construction authority

Action 39 authenticates the complete hostile OrderPage V5 set, the sealed
CandidateFeed V2 body, the owner-blind order stream, and the immutable
Market/Realm/collateral/Genesis joins once. Its private borrow-bound authority
retains the exact Feed and page accounts and exposes only bounded reads joined
to a compact projection. The compact constructor accepts only that authority; no
payload supplies a locator row, directory, slice reference, count, digest, or
candidate identity.

Construction writes directly into caller-owned account buffers. For every
dense live order it derives one unique physical page location. Tombstones are
not dense orders: each page's physical populated width is retained only to
bound the local slot and is never equated to the live Feed order count. For
each order, the referenced Feed quantities must sum exactly to its authenticated
entitlement. Each group is emitted in strictly increasing Feed-slice order.

The plane ID commits the root/candidate coordinates, retained Feed account and
full body ID, traversal binding, both child accounts and rent owners, every
locator row, directory, and slice reference. Each child also has a
domain-separated full-body ID held by the indexed root.

The traversal no longer materializes `[416]` slices or a fixed-capacity book:
it streams one exact Feed slice or one page-local order at a time from the
accounts retained by its private authority. During the same authenticated Feed
pass that validates slice geometry it records only the total and per-order
reference widths. The exact-index constructor then emits every locator row and
directory once and streams the Feed slices once into their pre-sized groups; it
does not repeat membership/entitlement derivation.

The compact noncopyable rent preparation persists only source/poststate IDs,
the updated rent compartment, balances, and its exact projector transcript; it
does not carry two 980-byte Root values. Authentication consumes the
preparation, joins one borrowed source Root, and mints a noncopyable authority
consumed by the builder. The builder borrows its construction input, streams
the 1,196-byte indexed root directly into caller-owned account memory, and
hashes that encoded buffer without constructing an indexed-root value or a
second base scratch array. The disabled action-39 composer preauthenticates the
single payer's aggregate principal for the root, both compact children, and
the direction-dependent cash pots before any CPI. Compiled end-to-end frame
measurement is still required before promotion: source account widths are not
frame measurements.

## Compact active geometry

Both bodies have a 272-byte sealed common header. It contains only the indexed
root account, sibling account, shared plane ID, retained Feed account, full Feed
body ID, traversal binding ID, active counts, per-page physical populated
widths, stored bump, and exact deletable-rent owner.

```text
locator   = 272 + live_order_count * 4
adjacency = 272 + live_order_count * 8 + slice_reference_count * 2
```

`slice_reference_count` is one for a split or merge slice and two for a direct
slice. It lies in `slice_count..=2*slice_count` and is at most 832. At the
protocol maxima, the locator is 528 bytes and adjacency is 2,448 bytes. Both
account bodies fit below 4 KiB and the Solana 10,240-byte per-instruction allocation-increase
limit; no staged partial account or partial root liability is needed.

## Authenticated bounded reads

A pair query first hostile-decodes the complete `0xa9/2` root, checks the exact
root/child/Feed PDAs, bumps, program ownership, mutability, and pairwise account
nonaliasing, and recomputes both child full-body IDs and the retained Feed full
body ID. Only then does it mint a private read authority.

The query reads two locator rows, two directory rows, and exactly their
referenced 13-byte CandidateFeed slice records. It verifies local directory
ordering, side-correct legs, the requested counterparty join, and pair symmetry.
It returns the exact shared slice prefix, buy/sell totals derived from the Feed,
and whether either order also touches another real counterparty or virtual
split/merge. Static clients and index bytes remain untrusted projections.

## Rent and terminal order

Each child persists its payer, the full rent-exempt principal paid without a
prefund discount, and the observed hostile-prefund donation floor. Atomic close
returns only that principal to its payer and routes every remaining lamport to
the root-bound MarketBinding neutral sink.

The compact adjacency depends on the retained Feed, so terminal order is
strict:

1. the base root reaches `Retiring` with every non-Feed child liability
   discharged while the retained Feed is still `Live`;
2. the adapter full-body-authenticates that Feed and atomically retires both
   compact children;
3. a separately authenticated transition retires the Feed and promotes the
   base root to `Terminal`; and
4. the 1,196-byte indexed root returns its exact principal and sends all
   nonprincipal lamports to the neutral sink.

Closing the Feed first, presenting a replacement Feed body, partially closing
the index pair, or stranding the indexed-root principal is not representable by
the promoted path.

The historical 980-byte root cannot count the two children. The reserved
`IndexedSettlementRootV1AccountV1` successor uses `0xa9/2` at the unchanged
canonical Root PDA and owns both child accounts, both full body IDs, the plane
and capability-profile identities, and an exhaustive two-live/two-retired
partition. The fresh path funds the full 1,196-byte root principal plus both
compact child principals in the same rollback domain. The pure contract also
defines exact in-place-upgrade rent equations, but no generic caller-shaped SBF
upgrade writer is exposed.

## Reserved coordinates and refusal

The indexed root is reserved at `0xa9/2`, the locator at `0xb5/1`, and the
adjacency at `0xb6/1`. The child PDA seed domains are unique, one-per-root, and
at most Solana's 32-byte seed limit.

`EXACT_INDEX_PLANE_LIVE_ENABLED_V1` remains false. No deployable capability
profile admits action 39 through this implementation. Promotion additionally
requires action-specific migration of every root reader/writer, the
authenticated Feed-retirement and root-close successors, compiled frame/CU
measurement, and an independent review of the complete capability unit.
