# Transparent V1 Solana layout prototype

Status: offline account-layout and instruction-codec prototype (2026-08-18).
This is not an entrypoint, deployable program, CPI implementation, Token-2022
integration, RPC client, key/signing implementation, or chain-readiness claim.
Nothing below is a frozen deployment ABI; it is a codec prototype whose shapes
are expected to move again before any adapter writes bytes on a network.

## Scope and invariants

`programs/solana-layout` is a standalone dependency-free `no_std` Rust crate.
It owns only fixed bytes and deterministic intent bytes. Every account starts
with a one-byte discriminator and a one-byte schema version; every decoder
requires the exact byte length, rejects unknown versions/tags, validates enum
ranges, checks nonzero identities, and requires zero canonical padding.

V1 freezes `MAX_OUTCOMES = 16`. Market outcome slots `0..outcome_count` must
equal `SHA-256("dragons-clutch/outcome/v1" || market_id || index)`; remaining
slots are zero. Realm, profile, market, outcome, feed, epoch, terms, price-grid,
candidate, page, and order-set identities are 32-byte domain-separated values.
The SHA-256 implementation is included to keep the prototype dependency-minimal;
deployment must bind the selected primitive in an immutable profile before
treating an identity as a PDA seed.

Stored bumps are bytes in the layouts and are never recomputed or trusted as a
substitute for adapter PDA/account checks. Owner, authority, destination, and
source fields are opaque 32-byte identities here. Account owner/executable/
signer/writable/alias/PDA checks remain outside this crate.

Several bounds are restatements of constants owned by other crates —
`MAX_PAYOUTS = 8` (kernel), `MAX_GRID_TICKS = 64` and `MAX_ORDERS = 64` (batch),
`MAX_BUCKET_SECONDS` and `MAX_BUCKETS` (accumulator), `RELATION_VERSION = 1`.
The crate stays dependency-free, so they are restated rather than imported and a
codec test pins each one. A divergence from an owning crate is a real defect,
not a local policy choice.

## Version discipline

Each account carries its **own** schema version (`account_version::*`).
`LAYOUT_VERSION` is the largest of them (`2`), not one wire version shared by
every account. Accounts whose bytes never changed still encode and require
version `1`; accounts whose bytes changed encode `2` and refuse `1` explicitly
with `WrongVersion`. New accounts start at `2`, so the pair `(tag, version)`
never names two different shapes.

| Account | Version | Change |
| --- | ---: | --- |
| Realm, Market, Hoard, Position, Feed head | 1 | bytes unchanged |
| Profile | 2 | gained the 32-byte collateral-policy digest |
| Dense order page | 2 | gained every page-set commitment field |
| Every account introduced with this revision | 2 | new |

Intent bytes are versioned separately (`INTENT_VERSION = 1`) and did not change.

Two validation tightenings apply to unchanged bytes, and are refusals rather
than new fields: `RealmAccount.max_outcomes` must now be exactly `16` (the
documented V1 rule, previously only documented), and
`PositionAccount.reserved_cash_atoms` may not exceed `cash_atoms`. That freezes
the cash decomposition: `cash_atoms` is the total and `reserved_cash_atoms` is
the encumbered part of that total, so free cash is their difference.

## Fixed byte inventory

All integers are little-endian. The first two bytes are `(tag, version)`.

| Account | Tag | Exact bytes | Main fields |
| --- | ---: | ---: | --- |
| Realm | 1 | 70 | realm/profile, max outcomes, profile version, bump, flags |
| Profile | 2 | 100 | profile/realm, collateral-policy digest, version, flags |
| Market | 3 | 726 | market/realm/profile/terms IDs, 16 outcome IDs, feed, cap, slot |
| Hoard | 5 | 108 | market/realm/authority, collateral atoms, bump, flags |
| Position | 6 | 220 | market/owner, generation, 16 `u64` balances, cash/reserved cash |
| Feed head | 7 | 124 | feed/realm, cursor/boundary/pages, summary digest, bump |
| Dense order page | 8 | 1819 | market/epoch, 5 page-set commitments, page metadata, 16 × 99-byte records |
| Supply ledger | 9 | 333 | market/realm, generation, 16 internal + 16 external `u64` |
| Immutable terms | 10 | 1304 | terms digest, realm/profile/feed/price-grid, 8 × payout vector, window policy, failure policy |
| Epoch (book domain) | 11 | 328 | epoch/market/book/terms/grid/policy/order-set IDs, order range, shape, seed, phase |
| Price grid | 12 | 589 | grid identity, realm, price scale, 64 `u64` ticks |
| Candidate record | 13 | 305 | candidate digest, epoch/market, 16 prices, sigma/mu, AON mask, score, status |
| Final pot | 14 | 262 | epoch/market/candidate, 16 pot balances, pot cash, rounding pot, phase |
| Settlement receipt | 15 | 217 | epoch/market/candidate, buy/sell order ids, slice, quantity, price, consideration, consumed flags |
| Resolution | 16 | 165 | market/terms/feed, sealed window digest, cursor, repair generation, payout index |

One instance of every listed account is 6,670 bytes; a market whose epoch book
uses the full four pages is 12,127 bytes. This is the byte-size inventory only;
it is not a rent, account-metadata, transaction-message, or compute-unit
estimate, and it excludes page multiplicity beyond the one case named.

`MAX_ORDERS_PER_PAGE = 16` and `MAX_ORDER_PAGES = 4`, so one frozen page set is
exactly `MAX_EPOCH_ORDERS = 64` orders — the batch relation's `MAX_ORDERS`. A
page geometry that could not hold exactly one relation book would make the
closure check below meaningless.

## Persisted-state ownership

One persisted fact, one account field, one codec. The accounts added here exist
so a future adapter can reconstruct kernel/protocol state from authenticated
bytes instead of scanning positions, which is not an onchain option.

| Fact | Owning bytes | Binding checked in this crate |
| --- | --- | --- |
| Internal vs. accounted-external supply | `SupplyLedgerAccount.internal_supply` / `.external_supply` | `binds_market`, `check_position_bound` |
| Immutable payout set and window policy | `TermsAccount` | `binds_market` against `MarketAccount.terms` |
| Limit-to-tick domain | `PriceGridAccount.ticks` | `binds_terms`, `OrderPageAccount::decode_on_grid` |
| Frozen book domain | `EpochAccount` | `binds_terms`, `binds_page_set` |
| Frozen order set | every `OrderPageAccount`'s commitment fields | `verify_page_set` |
| One candidate's free coordinates | `CandidateRecord` | `binds_epoch` |
| Settlement pot | `FinalPotAccount` | `binds_candidate` |
| One settled slice | `SettlementReceiptAccount` | `binds_candidate` |
| Resolution | `ResolutionAccount.payout_index` | `binds_terms` |

`SupplyLedgerAccount` persists the aggregate as the **two terms whose sum it
is** — claims still credited internally and claims materialized outside the
internal ledger and accounted for — because that is exactly the decomposition
the reference adapter's closure equality needs. `check_position_bound` is a
necessary condition only: one position can never exceed the market-wide internal
aggregate. It is not the multi-position closure equality, and no single account
can decide that.

`FinalPotAccount` holds only pot-phase balances. A byte cannot be both an order
reservation and a settlement pot (`ARCHITECTURE.md` §3), so the account carries
no reservation field, Hoard principal never appears in it, and a closed pot must
be empty in every term.

`ResolutionAccount` records the payout index into the immutable terms set plus
the sealed-window evidence that selected it. It is bytes only: it is not
evidence that a window was sealed, and an adapter must still authenticate the
window result before trusting it.

## Terms bind payouts

`MarketAccount.terms` stores `TermsAccount.terms`, and that value is the
domain-separated SHA-256 digest of every other terms field: the payout-vector
set (up to 8 vectors of 16 exact integer weights over one common denominator),
the feed, the price grid, the observation grid family/version/bucket duration,
the exact expected bucket range, the coverage/repair policy ids, the maturity
horizon, and the failure policy id. A market therefore cannot be pointed at a
different payout set or window policy without contradicting a digest it already
committed to. That closes "payouts are not cryptographically bound to terms" at
the byte level.

The account-local `stored_bump` and `flags` are deliberately outside the digest:
they are address-derivation artifacts, and a PDA derived from the digest cannot
also be an input to it. Zero coverage, repair, or failure policy ids are refused
rather than defaulted, so no code path can canonize a policy by omission. A
maturity horizon shorter than the expected range is refused, so no prefix can
resolve.

The digest algorithm here is this crate's own terms digest. It is not the
collateral-policy digest below, and the two are never interchangeable.

## Cross-page closure

Order pages previously enforced sorting only within one page. Each page now also
carries its own digest, its order-id range, the previous page's last order id,
the set-wide order-set digest, and the frozen set order count.
`verify_page_set` takes the pages in index order and checks that:

- page indices are `0..page_count` with no gap, repeat, or reordering;
- market, epoch, order-set digest, and set order count agree across all pages;
- each page's stored range is exactly its records' first and last order id;
- each page opens strictly above the previous page's last order id, which makes
  the order-id sequence strictly increasing across the whole set, not per page;
- every non-final page of a frozen set is dense and the final page closes the
  count exactly;
- the per-page order counts sum to the committed set order count; and
- folding the page digests in index order reproduces the stored order-set
  digest.

Adversarial tests cover a dropped middle page, a duplicate order id across a
page boundary, a page-order swap, a post-freeze mutation of one order byte
(including the case where the mutator also recomputes that page's own digest), a
broken predecessor link, and an unfrozen page smuggled into a closed set.
`EpochAccount::binds_page_set` then ties the verified set to the epoch's
committed order set, page count, order count, and order range.

While an epoch is open it commits to nothing: order-set digest, order range,
page count, and order count must all be zero, and any nonzero value there is
refused as noncanonical padding rather than treated as a stale hint.

## Limit-to-tick mapping

`OrderRecord.limit` remains an opaque `u64` on the venue scale and its 99 bytes
are unchanged. The frozen mapping to the relation's tick domain lives in
`PriceGridAccount`: a strictly increasing tick vector, each tick at most the
price scale, with the grid identity being the digest of that body. A limit maps
to a tick by exact membership; a limit that is not exactly one of the ticks has
no tick. `OrderPageAccount::decode_on_grid` therefore refuses off-grid limits at
decode time with `InvalidTick`, and the plain `decode` — which cannot see a grid
— performs only the structural checks. `TermsAccount.price_grid` binds the grid
to the market's immutable terms, and `EpochAccount.price_grid` and
`price_scale` bind it to the clearing epoch.

## Collateral-policy digest field

`ProfileAccount` gained a 32-byte `collateral_policy_digest` at byte offset
`66..98` (immediately after `realm`, before `version` and `flags`). It is zero
until the policy is frozen, and nonzero exactly when
`PROFILE_FLAG_POLICY_FROZEN` (flag bit 0) is set; every other combination is
refused.

This crate owns **only** those bytes and that zero-until-frozen rule. The digest
*algorithm* — the domain string, the exact preimage, and the Python/Rust
cross-language equality — is owned by the collateral-profile lane, so this crate
deliberately provides no derivation function for it and none may be added here.

## Candidate records

A candidate's only free economic coordinates are the price vector, the virtual
split/merge pair, and the honored all-or-none mask. Fills are derived
canonically from those plus the frozen book, so they are deliberately **not**
persisted. The candidate identity is the digest of exactly those coordinates
plus the epoch/market domain and the book length; score, status, submitted slot,
and bump are outside it, because they are claims and lifecycle rather than
coordinates.

The codec refuses only what no recomputation could accept: a mask bit above the
book length (a claim about an order that does not exist), a candidate that both
splits and merges, a churn field disagreeing with `sigma + mu`, and prices
outside the frozen simplex or scale. Verifying that a candidate is *valid* — let
alone the best valid submitted candidate — is `crates/clutch-batch`'s job, not
this crate's.

## Refusal codes

`CodecError::code()` maps each refusal to the stable taxonomy code of
`VECTOR_SPINE_PROPOSAL.md` §2.3 (which is itself PROPOSED). Per its rule TAX-3
the enum's own discriminants are never a taxonomy code, and this function is the
only sanctioned mapping; a test pins the numbers and asserts no two facts share
one. The five codes added with this revision are `shape.invalid-price-grid`
(2049), `shape.invalid-tick` (2050), `auth.mismatched-state` (4011),
`cons.aggregate-closure-mismatch` (5011), and `cons.invalid-consideration`
(5015).

## Intent bytes

Intent bytes use a `(tag, INTENT_VERSION)` prefix and no variable-length fields:
CreateMarket (139 bytes), Split/Merge (74), Materialize/Dematerialize (107),
FeedAdvance (74), PlaceOrder (165), CancelOrder (130), and SettlePage (68).
`Intent::encode` writes into caller-owned storage; `Intent::decode` accepts only
the exact length implied by its tag. Zero quantities, invalid outcomes, zero
identities, invalid order flags, and unsupported tags are refusals. The intent
is data for a future adapter, not authority to sign or submit anything. No
intent exists yet for freezing a page set, submitting a candidate, or settling a
slice; those state accounts are currently written by no encoded intent in this
crate.

## Seam to the semantic crates

The seam is deliberately one-way:

1. A future native adapter authenticates Solana account metadata and hostile
   instruction accounts, then parses these bytes.
2. It converts checked `PositionAccount` balances, `SupplyLedgerAccount` terms,
   `TermsAccount` payout vectors, and `ResolutionAccount.payout_index` into the
   already-owned `clutch-kernel::MarketState`/`Position` values. Kernel
   transitions return logical state changes and CPI intents; this codec does not
   duplicate those transitions or invent amounts.
3. It maps `FeedAccount.summary`/cursor evidence and the `TermsAccount` window
   policy to an authenticated input for `clutch-accumulator::Summary`; the
   accumulator owns coverage and exact interval-summary algebra, not feed
   account bytes, and this crate stores policy identities without interpreting
   them.
4. It maps a verified page set plus `EpochAccount` to
   `clutch-batch::relation_v1::RelationDomainV1`, and `CandidateRecord` to that
   relation's candidate witness. The batch crate owns eligibility, fills,
   conservation, pairing feasibility, tie rules, and its "best valid submitted
   candidate" wording; this crate owns only bytes, identity, and order.

No account parser may write a Position directly from an external venue. A
future adapter must check all aliases and authenticated mints/programs before
applying the kernel's logical writes. CPI construction, return-data checking,
clock/replay policy, and SBF/runtime behavior are explicit unverified seams.

## Non-goals and evidence

There is no Solana dependency, account-info type, PDA derivation, Token-2022
dependency, RPC read, key material, signing, deployment, or entrypoint. Tests
are host-side golden/adversarial codec tests only; they provide no chain or
runtime evidence and do not establish formal verification or chain readiness.

Specifically not established here: that a page set's orders are economically
admissible, that a persisted candidate is valid or best, that a stored score was
computed correctly, that a resolution's window was really sealed, or that any
multi-position aggregate closes. Each of those needs the owning semantic crate
plus an authenticated adapter; the bytes only make the question askable.

Gates run offline and locked: 37 unit tests, `clippy --all-targets -D warnings`,
`RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`, and `cargo fmt --check`.
