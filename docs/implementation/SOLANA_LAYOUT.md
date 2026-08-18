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
`MAX_PAYOUTS = 8` (kernel), `MAX_GRID_TICKS = 64`, `MAX_ORDERS = 64` and
`MAX_PORTFOLIO_ORDERS = 8` (batch), `MAX_BUCKET_SECONDS` and `MAX_BUCKETS`
(accumulator), `RELATION_VERSION = 1`.
The crate stays dependency-free, so they are restated rather than imported and a
codec test pins each one. A divergence from an owning crate is a real defect,
not a local policy choice.

## Version discipline

Each account carries its **own** schema version (`account_version::*`).
`LAYOUT_VERSION` is the largest of them (`4`), not one wire version shared by
every account. An account keeps the version its current bytes were introduced
at; an account whose bytes change moves to the next version and refuses every
earlier one explicitly with `WrongVersion`, so the pair `(tag, version)` never
names two different shapes.

| Account | Version | Change |
| --- | ---: | --- |
| Realm, Market, Hoard, Position, Feed head | 1 | bytes unchanged |
| Profile | 2 | gained the 32-byte collateral-policy digest |
| Supply ledger, Terms, Epoch, Price grid, Candidate, Final pot, Receipt, Resolution | 2 | introduced at 2 |
| Dense order page | 4 | version 2 gained the page-set commitment fields; version 3 replaced its bare 99-byte records with tagged fixed-width order slots; version 4 made order ids positional, added the retirement slot kind and its header count, and gave every record a persisted expiry. It refuses 1, 2, and 3 |

Intent bytes are versioned separately and moved to `INTENT_VERSION = 2` with the
same revision: a placement now carries an `OrderSlot` rather than a bare
`OrderRecord`, and a cancellation carries the retirement's generation. Every
decoder refuses `INTENT_VERSION_V1 = 1` explicitly with `WrongVersion`.

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
| Dense order page | 8 | 4012 | market/epoch, 5 page-set commitments, page metadata + retirement count, 16 × 236-byte tagged order slots |
| Supply ledger | 9 | 333 | market/realm, generation, 16 internal + 16 external `u64` |
| Immutable terms | 10 | 1656 | terms digest, realm/profile/feed/price-grid, 8 × payout vector, window policy, failure policy |
| Epoch (book domain) | 11 | 328 | epoch/market/book/terms/grid/policy/order-set IDs, order range, shape, seed, phase |
| Price grid | 12 | 589 | grid identity, realm, price scale, 64 `u64` ticks |
| Candidate record | 13 | 305 | candidate digest, epoch/market, 16 prices, sigma/mu, AON mask, score, status |
| Final pot | 14 | 262 | epoch/market/candidate, 16 pot balances, pot cash, rounding pot, phase |
| Settlement receipt | 15 | 217 | epoch/market/candidate, buy/sell order ids, slice, quantity, price, consideration, consumed flags |
| Resolution | 16 | 165 | market/terms/feed, sealed window digest, cursor, repair generation, payout index |
| Clearing checkpoint | 17 | 48750 | 158-byte header (market/epoch/candidate, order-set binding, consumed fold, walk cursor) + 48,592 opaque body |
| Candidate feed | 18 | 6266 | 346-byte header (candidate/epoch/market/order-set, prices, sigma/mu, mask, claimed score) + 64 fills + 416 slices |

One order slot is 236 bytes: a one-byte kind discriminator, that kind's exact
body (107 bytes single-Egg, 235 bytes portfolio, 80 bytes retirement), and
canonical zero padding out to the common width. The page header is 236 bytes —
one more than v3, for `tombstone_count`.

One instance of each of the fifteen **protocol** accounts (tags 1-16, the two
clearing-plane rows excluded) is 9,215 bytes; a market whose epoch book uses the
full four pages is 21,251 bytes. **Both figures are corrected here**: they read
8,863 and 20,899 until this revision, which is the pre-`927d4bc` terms width —
the v3 terms revision grew that account by 352 bytes and these two sums were not
re-added. The clearing plane is not per-market and is excluded on purpose: it is
one checkpoint and one feed per `(market, epoch, candidate)` under verification,
adding 55,016 bytes for each candidate a crank is working on. This is the
byte-size inventory only; it is not a rent, account-metadata,
transaction-message, or compute-unit estimate, and it excludes page multiplicity
beyond the one case named.

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
| One portfolio order's coefficient vector and cash bound | `PortfolioRecord` inside an `OrderSlot` | `validate`, `validate_on_scale`, `binds_page_set` |
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
- each page's stored `prev_page_last_order_id` is exactly the canonical id of
  the rank its own index fixes, which makes the order-id sequence dense and
  strictly increasing across the whole set, not per page;
- every non-final page of a frozen set is dense and the final page closes the
  count exactly;
- the per-page order counts sum to the committed set order count;
- the portfolio records across the whole set do not exceed
  `MAX_PORTFOLIO_ORDERS = 8`, which a single page cannot decide;
- at least one order in the whole set is still live, that is not retired, which
  a single page also cannot decide; and
- folding the page digests in index order reproduces the stored order-set
  digest.

Adversarial tests cover a dropped middle page, a duplicate order id across a
page boundary, a page-order swap, a post-freeze mutation of one order byte
(including the case where the mutator also recomputes that page's own digest), a
broken predecessor link, an unfrozen page smuggled into a closed set, a
post-freeze change to one portfolio coefficient atom or cash bound, a slot
re-typed from portfolio to single-Egg, and a ninth portfolio added on the page
after the eighth. `EpochAccount::binds_page_set` then ties the verified set to
the epoch's committed order set, page count, order count, and order range, and —
because a page alone can bound neither an outcome index (it knows only
`MAX_OUTCOMES`) nor a horizon (it stores a 32-byte epoch identity that cannot be
inverted into an epoch index) — refuses any live single-Egg outcome at or above,
or any live portfolio `active_len` above, the epoch's own `outcome_count`, and
any live record whose `expiry_epoch` is already below the epoch's own
`epoch_index`. Retired records are skipped by both checks: nothing will ever be
fed to the relation from one.

While an epoch is open it commits to nothing: order-set digest, order range,
page count, and order count must all be zero, and any nonzero value there is
refused as noncanonical padding rather than treated as a stale hint.

## Limit-to-tick mapping

`OrderRecord.limit` remains an opaque `u64` on the venue scale; its body is
107 bytes at v4, the 99 of v3 plus `expiry_epoch`. The frozen mapping to the relation's tick domain lives in
`PriceGridAccount`: a strictly increasing tick vector, each tick at most the
price scale, with the grid identity being the digest of that body. A limit maps
to a tick by exact membership; a limit that is not exactly one of the ticks has
no tick. `OrderPageAccount::decode_on_grid` therefore refuses off-grid limits at
decode time with `InvalidTick`, and the plain `decode` — which cannot see a grid
— performs only the structural checks. A portfolio record has no tick to look
up: its bound is a per-lot collateral in complete-set units, not a per-outcome
limit price, and it may legitimately exceed the price scale. What the grid
contributes there is the frozen scale, against which `decode_on_grid` refuses
per-lot values and per-lot bounds that could never be evaluated at all
(`ArithmeticOverflow`). `TermsAccount.price_grid` binds the grid
to the market's immutable terms, and `EpochAccount.price_grid` and
`price_scale` bind it to the clearing epoch.

## Portfolio order records (addendum, 2026-08-18)

The cost lab recorded this as a structural seam rather than a cost result:
`crates/clutch-batch` `relation_v1` admits up to eight portfolio orders carrying
a 16-slot coefficient vector, and the landed 99-byte `OrderRecord` carried a
single `outcome: u8` and no coefficients, so a portfolio order the relation
admits had no persisted page encoding at all. This addendum is that encoding.
Like everything else here it is PROPOSED and is not a frozen deployment ABI.

### Representation: tagged fixed-width slots in the same page

Two shapes were on the table: a distinct record type discriminated by a tag
inside the existing page, and a separate `PortfolioPageAccount`. The page won,
for a reason that is about the commitment rather than about bytes. The relation
book is one array of orders in one strictly increasing canonical order-id order,
interleaving both families, and at most 64 orders in total; the page geometry
exists precisely so that a full page set *is* one book. A separate account would
split the one order-id chain into two, so cross-family uniqueness — a portfolio
and a single-Egg record can never share an order id — would stop being a
consequence of the checks that already close the set and would become a new
merge check nothing commits to; it would also let a full set of four dense pages
plus a portfolio account hold 72 orders, which is not a book. Keeping both
families in one page keeps one chain, one fold, one `set_order_count`, and one
`MAX_EPOCH_ORDERS == MAX_ORDERS` identity. The cost of that is a common slot
width: every slot is `ORDER_SLOT_BYTES` bytes — 228 at v3, 236 at v4 — even
though a single-Egg body is 99 and then 107, which is why the page grew from
1,819 to 3,883 and then to 4,012 bytes. That is not slack.
The unused tail of a single-Egg slot is *required* to be zero, exactly like every
other padded field in this crate, so a slot has one encoding, the account keeps
one exact length, and padding can never influence a digest.

### Slot layout

A slot is a one-byte kind discriminator, that kind's exact body, and canonical
zero padding to the common width. Kind `0` is padding and the whole slot is
zero; kind `1` is a single-Egg `OrderRecord`; kind `2` is a `PortfolioRecord`;
kind `3` is a `TombstoneRecord` (v4). Any other kind byte is `WrongTag`, and a
nonzero byte anywhere in a slot's padding is `NonCanonicalPadding` — including
an all-zero record smuggled into a padding slot under a real kind byte, which is
a record and not padding.

The tables below are the **v4** shapes; a unit test pins every offset in all
three against the encoder, so the tables and the codec cannot drift apart.

Kind 1, single-Egg (`ORDER_RECORD_BYTES = 107` of body):

| Slot-local offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | kind = 1 |
| 1 | 32 | `owner` |
| 33 | 32 | `order_id` |
| 65 | 1 | `outcome` |
| 66 | 1 | `side` |
| 67 | 8 | `quantity` |
| 75 | 8 | `limit` |
| 83 | 8 | `minimum_fill` |
| 91 | 1 | `flags` |
| 92 | 8 | `generation` |
| 100 | 8 | `expiry_epoch` |
| 108 | 128 | zero padding to the common width |

Kind 2, portfolio (`PORTFOLIO_RECORD_BYTES = 235` of body, no padding):

| Slot-local offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | kind = 2 |
| 1 | 32 | `owner` |
| 33 | 32 | `order_id` |
| 65 | 1 | `side` |
| 66 | 1 | `active_len` |
| 67 | 1 | `flags` |
| 68 | 128 | `coefficients[0..16]`, 16 x `u64` |
| 196 | 8 | `lots` |
| 204 | 8 | `limit_collateral_per_lot` |
| 212 | 8 | `minimum_fill_lots` |
| 220 | 8 | `generation` |
| 228 | 8 | `expiry_epoch` |

Kind 3, retirement (`TOMBSTONE_RECORD_BYTES = 80` of body):

| Slot-local offset | Bytes | Field |
| ---: | ---: | --- |
| 0 | 1 | kind = 3 |
| 1 | 32 | `order_id` — the retired rank, unchanged |
| 33 | 32 | `owner` — the retired record's owner |
| 65 | 8 | `retired_generation` |
| 73 | 8 | `generation`, strictly above `retired_generation` |
| 81 | 155 | zero padding to the common width |

At v3 the single-Egg body was byte-identical to the previous bare 99-byte record
and simply moved one byte right, behind its kind discriminator; at v4 it is that
body plus `expiry_epoch`.

### What the codec refuses

`PortfolioRecord::validate` is the scale-free half: zero owner or order id;
`side > 1` or any reserved flag bit; `active_len` outside `1 ..= 16`; any nonzero
coefficient at or beyond `active_len` (the "coefficient count disagrees with the
declared outcome width" refusal); an all-zero active vector, which asks for
nothing at any price; `lots == 0`; `minimum_fill_lots > lots`; an all-or-none
flag whose `minimum_fill_lots` is not `lots`; and a `lots × Σcoefficients`
product that is not representable.

`PortfolioRecord::validate_on_scale`, reached through
`OrderPageAccount::decode_on_grid`, adds the two products that need the frozen
price scale: `lots × Σcoefficients × price_scale` and
`lots × limit_collateral_per_lot × price_scale`. A record failing either could
never be classified against any candidate, so refusing it is a representability
fact rather than an economic judgement.

Set-wide, `verify_page_set` refuses more than `MAX_PORTFOLIO_ORDERS = 8`
portfolio records across the frozen set, and a single page refuses more than
eight by itself. `EpochAccount::binds_page_set` refuses an `active_len` above
the epoch's `outcome_count` — a page alone can only bound a width by
`MAX_OUTCOMES`, and the epoch is the account that names the market's actual
width. The same binding now also refuses a single-Egg `outcome` at or above the
epoch's `outcome_count`, which no account checked before.

The page digest domain moved to `dragons-clutch/order-page/v2` here, and again
to `/v3` at v4, because its preimage shape changed both times. The order-set fold
keeps `dragons-clutch/order-set/v1` through both: its preimage — market, epoch,
page count, order count, page digests — did not change, and the leaves it folds
already carry the new domain.

### Mapping contract to `PortfolioOrderV1`

The layout owns bytes; the relation owns semantics. A decoded `PortfolioRecord`
maps onto `clutch_batch::relation_v1::PortfolioOrderV1` exactly as follows. The
rustdoc on `PortfolioRecord` carries this as a doc-tested example.

| `PortfolioOrderV1` field | Source in the decoded record |
| --- | --- |
| `coefficients: [u64; 16]` | `coefficients`, verbatim, including its zero padding |
| `active_len: u8` | `active_len` |
| `lots: u64` | `lots` |
| `limit_collateral_per_lot: u64` | `limit_collateral_per_lot` |
| `minimum_fill_lots: u64` | `minimum_fill_lots` |
| `side: Side` | `side`: `0` → `Buy`, `1` → `Sell` |
| `partial_policy: PartialPolicy` | `flags` bit 0 set → `AllOrNone`, clear → `Allow` |
| `owner: u16` | the adapter's owner-tag image of the 32-byte `owner`, which must land in `0 .. EpochAccount.owner_count` |
| `canonical_order_id: u64` | the record's **live rank** in the canonical page-set walk, plus one — the walk skips retirements, so this is not the same number as the stored `order_id` on a set that has any (see the v4 addendum) |
| `expiry_epoch: u64` | `expiry_epoch`, verbatim; v4 persists it, and `EpochAccount::binds_page_set` refuses a frozen set holding a live record already past it |
| — | `generation` is replay protection for the placing instruction and has no relation field |

The owner row is a conversion the adapter performs, not a value this crate
stores, and it is not checked here: nothing in this crate proves that an owner
tag is a bijection into `owner_count`. The order-id row changed shape at v4:
the stored id is now itself a rank (`canonical_order_id(rank)`), so the *slot*
numbering is a value this crate fixes rather than one a caller chooses, and the
only work left to the projection is the live-rank renumbering that skips
retirements.

Correspondingly, these relation refusals are *not* pre-empted here and remain
`clutch-batch`'s: `active_len <= domain.outcome_count` (checked here only against
the epoch, not the relation domain), `owner < domain.owner_count`, the
all-or-none admission policy, and every eligibility, fill, conservation, and
pairing question. A byte-valid portfolio record is not an admissible order.
`expiry_epoch >= domain.epoch` is now checked here too, against the epoch
account's index — the relation still owns it against its own domain.

## Streaming page decoders (addendum, 2026-08-18)

The SBF foundation lane measured a hard blocker rather than predicting one: on
the pinned `cargo-build-sbf` toolchain, `OrderPageAccount::decode` is reported
at an estimated **8,640-byte** call frame and `decode_on_grid` at **8,320**,
against SBF's 4,096-byte per-frame maximum. The v3 page — a 235-byte header and
16 tag-discriminated 228-byte slots, 3,883 bytes in all; v4 is 236 and 236, 4,012
in all, which only widens the gap — could therefore not be read on-chain at all, and `clutch-sbf` compiles its `read_order_page` wrapper
off-chain only so that reaching for it is a compile error instead of a frame
overflow the loader would happily run.

The `stream` module is the fix, in the shape this crate already used for
`OrderPageAccount::recomputed_page_digest`: nothing ever holds more than one
slot. It is **additive**. The buffered decoders are unchanged, and they remain
the golden reference every equivalence test is written against.

### On-chain consumers use only the streaming path

`OrderPageAccount::decode`, `OrderPageAccount::decode_on_grid`, and
`verify_page_set` are host-only. No instruction compiled for
`target_os = "solana"` may call them; the frame overflow is undefined behaviour
at execution time, not a refusal. An on-chain consumer reads a page with
`stream::verify_page` (or `stream::verify_page_on_grid`), walks its orders with
`stream::OrderSlotCursor`, and closes a frozen set with
`stream::verify_page_set` or `stream::epoch_binds_page_set`.

This is a rule about which functions may appear on-chain, not a compiler
guarantee: the buffered decoders are still codegen'd into an SBF build of this
crate, so their frame diagnostics still appear in an SBF build log even when
nothing calls them.

### API surface

| Streaming entry point | Buffered counterpart |
| --- | --- |
| `stream::OrderPageHeader::decode` | — (the header half of `OrderPageAccount::decode`) |
| `stream::OrderPageHeader::validate_shape` | — (the header-local half of `OrderPageAccount::validate`) |
| `stream::OrderSlotCursor` | — (`OrderPageAccount.orders`, one slot at a time) |
| `stream::streamed_page_digest` | `OrderPageAccount::recomputed_page_digest` |
| `stream::verify_page` | `OrderPageAccount::decode` |
| `stream::verify_page_on_grid` | `OrderPageAccount::decode_on_grid` |
| `stream::verify_page_set` | `verify_page_set` |
| `stream::epoch_binds_page_set` | `EpochAccount::binds_page_set` |

| Streaming writer | What it writes |
| --- | --- |
| `stream::init_page` | an empty open page over a fresh account |
| `stream::append_slot` | one order, of either family, at the derived id |
| `stream::write_single_slot` | `append_slot` at the single-Egg family |
| `stream::write_tombstone` | one retirement, in place, over a live record |
| `stream::frozen_set_commitment` | nothing — it computes what a freeze would stamp |
| `stream::seal_page` | one page's three freeze fields |
| `stream::OrderPageHeader::next_order_id` | nothing — the id a placement must carry |
| `stream::OrderPageHeader::slot_index_of` | nothing — the slot an order id names |
| `stream::OrderPageHeader::live_count` | nothing — `order_count - tombstone_count` |

`OrderPageHeader` is every page field except the slots. `OrderSlotCursor`
decodes exactly one `ORDER_SLOT_BYTES` slot per step — kind byte, that kind's
exact body, canonical zero padding to the common width — and carries across
steps the one fact a single slot cannot decide: which position it is at, so that
a slot below `order_count` can be required to hold a valid record carrying
exactly *that* position's canonical order id, and a slot at or above it can be
required to be canonical padding. A refused step fuses the cursor.

`stream::verify_page` reads the header alone, sweeps the slot array once
structurally while folding the page digest, checks the header's own shape,
walks the array a second time for record semantics and the order-id chain, and
compares the stored digest last. Two passes over the bytes is what buys the
bounded frame; the bytes are in the account, not on the stack.

`stream::verify_page_set` is the same trade one level up. Every cross-page fact
— page index, market, epoch, order-set digest, set order count, stored range,
predecessor link, page digest — is a **header** field, and the only slot facts
the closure needs are per-page folds already computed while each page was
verified. So it verifies each page's bytes in index order and then closes the
set over four headers: under a kilobyte of stack, where four pages would be
fifteen.

### Equivalence contract

For any byte input, the streaming path returns exactly what the buffered path
returns — the same `Ok`, or the identical `CodecError`, including which of
several faults is reported first. `stream::verify_page_set` is stated against
the composition an on-chain caller would otherwise have written: decode every
page in index order, then close the set. One exception is stated rather than
hidden: a set of more than `MAX_ORDER_PAGES` page slices is refused with
`InvalidCount` before any page bytes are read, because no such set could be a
book whatever its pages hold.

Equivalence is a property of the whole verdict, not of every helper.
`OrderPageHeader::validate_shape` deliberately decides only what the first 236
bytes can decide; it is a cheap precondition, never a page's verdict.

The tests are written as harnesses rather than as fixture-by-fixture assertions:
each fixture is decided by both paths and the two verdicts are compared. They
cover every accepted page shape, sixteen structural refusals, the frozen-set
density and commitment refusals, every hostile byte fixture the buffered slot
decoder already had (unknown kind, kind byte on a padding slot, nonzero
single-Egg tail, nonzero slot end, dirty padding, truncation, trailing byte,
wrong tag, wrong version, all-zero page), and every adversarial page-set fixture
(dropped middle page, duplicate order id across a boundary, page-order swap,
post-freeze mutation with and without a repaired page digest, broken predecessor
link, unfrozen page in a closed set, a ninth portfolio across two pages, an
undecodable page in either position). A fixture that the buffered path refuses
is encoded through a validation-free writer, because `OrderPageAccount::encode`
validates first and so cannot produce the bytes of a page the codec refuses.

### Measured frames

`-Zemit-stack-sizes` on the pinned `cargo-build-sbf` toolchain
(platform-tools v1.53, `sbpf-solana-solana`), read back with
`llvm-readelf --stack-sizes`. These are the compiler's per-function frame sizes,
not a measurement of any executed instruction.

| Function | Frame (bytes) |
| --- | ---: |
| `OrderPageAccount::decode` (host-only) | 8,640 |
| `OrderPageAccount::decode_on_grid` (host-only) | 8,320 |
| `stream::verify_page_set` | 1,856 |
| `stream::verify_page_on_grid` | 1,024 |
| `stream::OrderSlotCursor::next_slot` | 960 |
| `stream::verify_page` (with its folding body) | 896 |
| `stream::streamed_page_digest` | 512 |
| `stream::epoch_binds_page_set` | 512 |
| `stream::fold_page_digest` | 448 |
| `stream::OrderPageHeader::decode` | 128 |

The largest streaming frame is 45% of the 4,096-byte maximum, and the SBF build
of this crate reports no frame overflow for any `stream::` function. Each of
these is a separate call frame — SBF caps a frame at 4,096 bytes and the call
depth at 64 — so the nesting `verify_page_set` → `verify_page` → `next_slot` →
`decode_slot` costs four frames, not one sum.

A unit test keeps the shape from regressing without needing the SBF toolchain:
it pins `size_of` for every value the streaming API puts on a frame, and it
reads the module's own source to assert that no slot array, slot-width buffer,
or page-width byte buffer appears anywhere in it. Those are exactly the
regressions that would put the 8,640-byte frame back.

Compute units are **not** measured here. The work is close to the buffered
path's rather than double it: both fold the page preimage into SHA-256 exactly
once, and where the buffered path decodes each slot once and then *re-encodes*
all sixteen to hash them, the streaming path decodes each slot twice and hashes
the account's own bytes. Hashing the raw bytes is sound precisely because a slot
that decodes has exactly one encoding. Whether an instruction that verifies a
four-page set fits an SBF compute budget is still an open obligation-10 question
for the owning lane, and this addendum answers only the frame question.

## Order page v4: positional ids, retirements, expiry (addendum, 2026-08-18)

Three landed findings and one proposed spec converge on one page-format
revision, so they were done as one rather than three:

* the orders lane (5cb4ad1) found the layout publishes a streaming **reader**
  and no streaming **writer**, so the SBF placement instruction hand-writes four
  regions at offsets it computes itself and re-verifies the page afterwards to
  learn whether it guessed right — recorded there as debt;
* the same lane found `CancelOrder` **unrepresentable**: the frozen page format
  had no way to say "this order is retired", so the instruction refuses;
* the same lane found caller-chosen 32-byte order ids are a **page-burning
  griefing vector** — a caller may place `0xff…ff` and no later order can extend
  a strictly increasing chain past it — and that portfolio placement is a **wire
  gap**, because `Intent::PlaceOrder` carried a bare `OrderRecord` while pages
  hold `OrderSlot`;
* `STREAMING_RELATION_DESIGN.md` §10 proposed that the relation's
  `canonical_order_id` be a **derived** page-set rank and that per-order expiry
  be "folded into the same format revision as the tombstone later".

Later is now. The page moves to `account_version::ORDER_PAGE = 4`, refusing 1,
2, and 3 explicitly, and the page-digest domain moves to
`dragons-clutch/order-page/v3` because the preimage shape changed again. The
order-set fold keeps its own `v1` domain by the rule it was given when the slots
were introduced: its preimage shape — market, epoch, page count, order count,
page digests — did not change, only the leaves it folds, and those carry the new
domain themselves.

### Order ids are positional

An order id is no longer a value a caller supplies. It is
`canonical_order_id(rank)`: the rank encoded big-endian in the low eight bytes
of a `Hash32`, zero elsewhere, where

```
rank = page_index * MAX_ORDERS_PER_PAGE + slot_index + 1
```

so page 0 owns ranks 1..16, page 1 owns 17..32, and the last rank any book can
hold is `MAX_EPOCH_ORDERS = 64`. `order_id_rank` inverts it and refuses
everything else: a nonzero prefix byte is `NonCanonicalIdentity`, rank zero is
`ZeroIdentity` (the all-zero identity stays reserved for "no order", as
everywhere else in this crate), and a rank above 64 is `InvalidCount` before any
page is consulted.

Four things follow from the encoding rather than from a check:

1. **The griefing vector is gone.** A caller has no id to choose. The only id a
   placement may carry is `OrderPageHeader::next_order_id()`, which is
   arithmetic over the page's own header.
2. **Byte order is rank order**, so the page's lexicographic id chain and the
   numeric rank chain are the same chain and the cross-page closure did not have
   to change shape.
3. **The chain check got stronger.** v3 asked that each id be strictly above its
   predecessor; v4 asks that each id be *exactly* the one its own slot's
   position admits. That refuses a gap as well as a repeat, it refuses a page-one
   slot carrying a page-zero rank — the cross-page duplicate v3 could only catch
   at closure time — and it needs no state carried between slots.
4. **A cancellation names a slot by arithmetic.** `slot_index_of` recovers the
   position from the id, so no search over the page is needed to find the order
   being retired.

`prev_page_last_order_id` is correspondingly the canonical id of
`page_index * MAX_ORDERS_PER_PAGE`, zero on page zero. It is a fact about the
page's index, not about how full its predecessors happen to be — which is
exactly why a rank is globally unique the moment it is written: a half-filled
page zero can never reach a rank page one has already used.

### Retirements

`OrderSlot` gains a third populated kind, `ORDER_KIND_TOMBSTONE = 3`, carrying a
`TombstoneRecord`: the retired order id, the retired record's owner, the retired
record's `generation`, and the retirement's own `generation`, which must be
strictly above it.

The rule is **retire in place**. A cancellation replaces the record in its slot,
keeping the slot and keeping the id. Removing a record instead would either
leave a hole the dense-page rules forbid, or renumber every later order — and
under positional ids renumbering silently rewrites identities that receipts,
candidates, and clients already name. So:

| Question | Answer under v4 |
| --- | --- |
| Does a retirement count toward `order_count`? | Yes. The slot is populated. |
| Does it move `first_order_id` / `last_order_id`? | No. The id did not move. |
| Is it covered by the page-set commitment? | Yes. Its bytes are slot bytes, so they are in the page digest and therefore in the order-set fold. It cannot be added, undone, or moved after a freeze without changing `order_set`. |
| Does the relation projection see it? | No. It is skipped, and takes no live rank. |
| Can it be retired again? | No. `write_tombstone` refuses a slot that is not a live record, which is what makes a replayed cancellation refuse on state. |

The header gains `tombstone_count: u8`, checked exactly against a fold over the
page's own slots, and folded into the page-digest preimage next to
`order_count` for the same reason `order_count` is there: both are header bytes
a writer stores, and a digest that did not cover them would let a page disagree
with its own header without disagreeing with its own digest. `live_count()` is
`order_count - tombstone_count`, which lets the header-only page-set closure
size a book's live order feed without touching a slot — and lets it refuse a
frozen set in which every order has been retired, which has nothing to clear and
no feed to project.

### Per-order expiry

Both order families gain `expiry_epoch: u64`. §10 recommended an epoch-level
single expiry *now* and a per-order field folded into this revision *later*;
this addendum takes the per-order field, for three reasons stated plainly:

* **It is the cheaper revision, not the more expensive one.** An epoch-level
  expiry is a new `EpochAccount` field, which is a *second* account format
  revision (`EPOCH` 2 → 3) on top of this one. The per-order field rides the
  page revision already happening.
* **It gives the horizon a real refusal.** No page can check an expiry: a page
  stores a 32-byte epoch identity, which is not invertible into an epoch index.
  The epoch account owns the index, so `EpochAccount::binds_page_set` and
  `stream::epoch_binds_page_set` refuse a frozen set holding a **live** record
  whose `expiry_epoch` is below `epoch_index`. That is the same place the
  outcome-width bound already lives, for the same reason.
* **It costs no slot width in the family that pays for it.** The single-Egg body
  grows 99 → 107 bytes and stays far inside the common slot width, which the
  portfolio body sets. The portfolio body grows 227 → 235, so the slot grows
  228 → 236 and the page grows 3,883 → 4,012.

What per-order expiry does **not** do is worth stating, because the field would
otherwise read as more than it is: a page set belongs to one epoch, and no
mechanism carries an order from one epoch's book into the next. So today the
field's whole effect is the dead-on-arrival refusal above. It persists the
relation's `expiry_epoch` coordinate — which nothing persisted before — and it
is the coordinate a carry-over mechanism would need; it is not itself GTC.

### Exact widths

| Quantity | v3 | v4 |
| --- | ---: | ---: |
| `ORDER_RECORD_BYTES` | 99 | 107 (`+ expiry_epoch`) |
| `PORTFOLIO_RECORD_BYTES` | 227 | 235 (`+ expiry_epoch`) |
| `TOMBSTONE_RECORD_BYTES` | — | 80 (`32 + 32 + 8 + 8`) |
| `ORDER_SLOT_BYTES` | 228 | 236 (`1 + PORTFOLIO_RECORD_BYTES`) |
| page header | 235 | 236 (`+ tombstone_count`) |
| `account_len::ORDER_PAGE` | 3,883 | 4,012 (`236 + 16 × 236`) |
| `MAX_INTENT_BYTES` | 256 | 302 (the widest intent, exactly) |

The slot is still exactly as wide as the widest body and no wider, and every
byte between a body and the common width is still required to be zero. The
retirement is by far the narrowest body, so cancellation costs no width at all.

### The streaming writer

`stream` was a reader-only module; it now publishes the write side, and the
whole of the placement and cancellation transitions live in it rather than in an
instruction:

```
stream::init_page(page, market, epoch, page_index, page_count, bump)  -> header
stream::append_slot(page, slot)                                       -> header
stream::write_single_slot(page, &order)                               -> header
stream::write_tombstone(page, order_id, owner, generation)            -> header
stream::frozen_set_commitment(&[&page…])            -> (order_set, set_order_count)
stream::seal_page(page, order_set, set_order_count)                   -> header
```

Three properties hold for all of them:

1. **No offsets escape the module.** A header is written through the same
   `Writer` field sequence `OrderPageAccount::encode` uses and a slot through the
   same `encode_slot`, so the write side is not a second transcription of the
   layout that could drift from the first. There are no `OFF_*` constants.
2. **One fold per mutation**, at the end, to store the page digest — and that
   fold decodes every slot as it folds, so a page left non-canonical in any slot
   has no digest rather than a digest over junk.
3. **The post-state is the writer's**, returned as a header, so "what did the
   page become" is an answer rather than a second decode and a field-by-field
   comparison against a guess.

What a writer does not re-establish is the whole pre-state: it decodes and
shape-checks the header, and it does not re-walk the record semantics of slots
it is not touching. The caller reads the page once with `verify_page_on_grid`,
which it needs anyway for the grid, and then writes. A test pins the writer's
output byte-for-byte against `OrderPageAccount::encode` of the same page — an
empty page, after two appends of different families, and after a retirement —
so "the writer writes what the codec would have written" is checked rather than
asserted.

Freezing is two calls because it is two facts: `frozen_set_commitment` verifies
every page of an open set, checks the density and link rules a freeze is allowed
to assume, and returns the `(order_set, set_order_count)` a freeze would stamp;
`seal_page` stamps one page with it and shape-checks the post-state header
before writing it. The page digest is deliberately not refolded by `seal_page`,
because none of the three freeze fields is in its preimage: what commits to a
freeze is `order_set`, which every page stores and which `verify_page_set`
recomputes from the page digests themselves. A test drives the whole path —
init, sixteen appends, a retirement, commitment, seal, closure — and shows
`verify_page_set` accepting the result and refusing the half-frozen intermediate.

### Integration note: what `orders_batch` deletes

The SBF placement instruction currently owns thirteen `OFF_*` header offsets,
nine `SLOT_OFF_*` record offsets, two `const _: () = assert!` tripwires against
the layout's constants, four writer helpers (`write_single_slot`,
`write_stored_range`, `seal_page_digest`, and the `read_slot` used only by the
post-write proof), and a post-write re-verification that decodes the page a
second time and compares the resulting header field by field against an
intended post-state. All of it goes. Both tripwires already fire on this
revision, which is what they were for.

The transition body becomes three layout calls with the module's own checks
between them:

```rust
fn apply_place_order(page: &mut [u8], placement: &Placement<'_>) -> Outcome<()> {
    let epoch = accounts::read_epoch(placement.epoch)?;
    let mut grid = ZERO_GRID;
    load_grid(placement.grid, &mut grid)?;

    // 1. The whole pre-state, on the frozen grid.
    let header = verify_page_on_grid(page, &grid)?;

    // Checks 4..10 are unchanged: epoch phase, page/grid/intent identities,
    // the replay counter, actor == owner, the record, the epoch's outcome
    // width, and the exact tick.

    // 2. The id is not a choice; the page's own state fixes it.
    require(
        placement.order.order_id == header.next_order_id()?,
        ClutchError::MismatchedState,
    )?;

    // 3. Slot bytes, header, and digest, in one call that returns the
    //    post-state rather than leaving the caller to reconstruct it.
    stream::write_single_slot(page, &placement.order)?;
    Ok(())
}
```

Call 2 is optional — `write_single_slot` refuses the same mismatch with
`NonCanonicalIdentity` — and is worth keeping only to preserve the module's
stated property that every check runs before any byte is written, in the
module's own error vocabulary. It costs no fold. The writers themselves already
hold that property: `append_slot` validates the header shape, the free slot, the
record, and the id before it touches a byte, so a refused placement leaves the
account unchanged whether or not the instruction pre-checks.

`CancelOrder` stops being a refusal and becomes the same shape:

```rust
let header = verify_page(page)?;                    // 1
require(epoch.phase == EPOCH_PHASE_OPEN, ClutchError::NotActive)?;
require(header.frozen == 0, ClutchError::NotActive)?;
require(header.market == epoch.market && header.epoch == epoch.epoch, …)?;
require(actor == intent_owner, ClutchError::UnauthorizedActor)?;
stream::write_tombstone(page, intent_order_id, intent_owner, generation)?;  // 2
```

Three notes for whoever lands it:

* **Replay.** Placement uses the page's `order_count` as its counter; a
  cancellation does not move that count, so it needs a different one. It has
  two: the slot kind (a retired slot refuses a second retirement) and the
  generation rule (`generation > retired_generation`). Passing the request
  envelope's `sequence` as the generation is the natural binding.
* **Which page.** The target order id *is* the page and the slot:
  `rank / MAX_ORDERS_PER_PAGE` selects the page and
  `OrderPageHeader::slot_index_of` the slot. An account list that supplies the
  wrong page refuses with `MismatchedBinding` rather than searching.
* **Cost.** The page-digest preimage grows from 3,743 bytes to 3,872 — a
  28-byte domain, market, epoch, `page_index`, `order_count`, the new
  `tombstone_count`, and sixteen 236-byte slots — so one fold is 61 SHA-256
  compression blocks rather than 59. But the module stops folding three times
  per placement and folds twice, so the documented per-placement figure moves
  from `3 x 59 = 177` blocks to `2 x 61 = 122`, and full slot-decode passes drop
  from five to three. That is arithmetic over the frozen widths, in the same
  form the module's existing `the_documented_page_fold_follows_from_the_frozen_widths`
  test states it — not a compute-unit measurement. Re-measuring CUs against v4
  is the integration lane's.

### Projection contract: slot rank vs live rank

Two numbers exist and they are not the same number once anything is cancelled.

* The **slot rank** is the stored `order_id`: positional, dense over populated
  slots, and unchanged by a cancellation.
* The **live rank** is the relation's `canonical_order_id`: the one-based
  position among *live* records in the canonical page-set walk. The walk visits
  pages in `page_index` order and slots in index order — which the frozen set's
  own closure already fixes — and increments the live counter only on a slot
  that `is_live()`. A retirement is skipped, and the skip is recorded in the
  projection walk's fold, so a walk over a set with a retirement is
  distinguishable from a walk over a set that never had one.

Worked example. A frozen page 0 holds ranks 1..5 and rank 3 has been retired:

| slot | slot rank (stored id) | live? | live rank (relation id) |
| ---: | ---: | :---: | ---: |
| 0 | 1 | yes | 1 |
| 1 | 2 | yes | 2 |
| 2 | 3 | **no** | — |
| 3 | 4 | yes | 3 |
| 4 | 5 | yes | 4 |

Ranks are assigned over live orders only. That is not a convenience: the
relation's `canonical_order_id` indexes the candidate's fill array, which carries
`order_len` entries read in step with the walk. A numbering with holes would
force either filler entries for dead orders — inflating `order_len` and putting
cancelled orders back into the priced set — or a broken index/fill
correspondence. Dense-over-live keeps `order_len` equal to the set's live count,
which the header-only closure can compute from `live_count()` alone.

**Why this is sound for resumed verification.** The live counter is a pure
function of the walk prefix, and the walk prefix is fixed by the frozen set's
bytes: the visit order is `(page_index, slot_index)`, which `verify_page_set`
pins, and the liveness of each slot is a byte fact the page digest covers, which
the order-set fold covers in turn. So the whole live-rank sequence is a function
of `order_set`. A pass that stops at `(page p, slot j)` and resumes there
therefore yields the same numbering as an unbroken walk provided it carries the
live counter in its checkpoint alongside the cursor — which is the same
obligation the checkpoint already has for its consumed fold, and the same
anchoring §10 assigns: bind `(order_set, consumed_fold)` at pass-1 finalize and
refuse any later pass whose epoch shows a different `order_set`. Nothing here is
a proof; it is a contract, and the layout side of it is that a retirement is
inside the commitment rather than outside it.

### Intent v2

`INTENT_VERSION = 2`; `INTENT_VERSION_V1 = 1` is refused explicitly by every
decoder.

* **`PlaceOrder` carries an `OrderSlot`**, not an `OrderRecord`, closing the
  portfolio wire gap. The encoding is the slot's kind byte and that kind's
  *exact* body, with none of the padding a page slot carries: 174 bytes
  single-Egg, 302 portfolio. `ORDER_KIND_EMPTY` and `ORDER_KIND_TOMBSTONE` are
  recognized kinds that are not placements and are refused with `InvalidEnum`;
  any other kind byte is `WrongTag`.
* **The intent still carries the order id, and it is an assertion rather than a
  choice.** The alternative — dropping the field and letting the writer fill it
  in — saves 32 wire bytes and loses a property worth more than they cost: a
  caller states the rank it believes it is taking, so a placement that lost a
  race to another placement is *refused* (`NonCanonicalIdentity`) instead of
  quietly landing at a different rank. The griefing vector dies either way,
  because the page's state fixes the only acceptable value.
* **`CancelOrder` carries the retirement's generation** alongside the target id
  and owner, which is exactly the tombstone write: 138 bytes. Its `order_id` is
  now checked as a canonical rank on the wire, and because ids are positional it
  names the page and the slot with no search — no slot index is carried.

### Deliberately out of scope

Named here so they are debt rather than oversight:

* `SettlementReceiptAccount.buy_order_id` / `.sell_order_id` are still opaque
  32-byte identities that may also be zero (the virtual split/merge legs). Under
  v4 they should be canonical ranks. That is a receipt-side tightening on an
  account this revision does not otherwise touch, and it was left out to keep
  the blast radius at the page and the intent.
* `EpochAccount` records `order_count` (populated slots) and not the set's live
  count, and its `first_order_id` / `last_order_id` are now derivable from
  `order_count` alone. Collapsing them, and adding a frozen live count, is an
  `EPOCH` revision this one deliberately does not open.
* Nothing here proves the owner-tag bijection §10 describes; the epoch's
  `owner_count` is still an unchecked claim from this crate's point of view.
* ~~`init_page` exists, but no intent creates a page and no instruction calls
  it; page creation remains unrepresentable on the wire.~~ **Closed
  2026-08-19** by `Intent::InitOrderPage` and `instructions::genesis`; the
  *freeze* half of the same row is still open — `frozen_set_commitment` and
  `seal_page` are still called by nothing.

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
FeedAdvance (74), PlaceOrder (174 single-Egg or 302 portfolio), CancelOrder
(138), SettlePage (68), and the genesis appends below — InitRealm (44),
InitProfile (69), InitPriceGrid (66), InitTerms (66), InitOrderPage (70), Endow
(74). `MAX_INTENT_BYTES = 302` is exactly the widest of them — a portfolio
placement — rather than a round number with slack in it, and **none of the six
appends widened it**.
`Intent::encode` writes into caller-owned storage; `Intent::decode` accepts only
the exact length implied by its tag **and its slot kind**. Zero quantities,
invalid outcomes, zero identities, invalid order flags, non-rank order ids, a
placement whose slot kind is padding or a retirement, and unsupported tags are
refusals. The intent is data for a future adapter, not authority to sign or
submit anything. No
intent exists yet for freezing a page set, submitting a candidate, freezing an epoch's
page set, or settling a slice; those state accounts are currently written by no
encoded intent in this crate. Page *creation* is no longer on that list — see
the genesis addendum.

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
   `clutch-batch::relation_v1::RelationDomainV1`, each decoded **live** order
   slot to a `SingleEggOrderV1` or `PortfolioOrderV1` under the field-by-field
   contract in the portfolio addendum above — retired slots are skipped and take
   no live rank, per the v4 addendum — and `CandidateRecord` to that relation's
   candidate witness. The batch crate owns eligibility, fills,
   conservation, pairing feasibility, tie rules, and its "best valid submitted
   candidate" wording; this crate owns only bytes, identity, and order.

No account parser may write a Position directly from an external venue. A
future adapter must check all aliases and authenticated mints/programs before
applying the kernel's logical writes. CPI construction, return-data checking,
clock/replay policy, and SBF/runtime behavior are explicit unverified seams.

## Genesis intents (addendum, 2026-08-19)

Six intents that bring accounts into existence, plus the one that credits a
position's opening cash. All are `INTENT_VERSION = 2` and all refuse
`INTENT_VERSION_V1 = 1` explicitly — not because a version-1 encoding of these
tags ever existed, but because the pair `(tag, version)` must never name two
shapes, and version 1 names none at tags 10-15 and must keep naming none.

| intent | tag | bytes | fields |
| --- | ---: | ---: | --- |
| `InitRealm` | 10 | 44 | parent Profile identity, Realm nonce, outcome width, Profile version |
| `InitProfile` | 11 | 69 | Realm, child collateral-policy digest, subfield schema version, Profile version |
| `InitPriceGrid` | 12 | 66 | Realm, grid digest |
| `InitTerms` | 13 | 66 | Realm, terms digest |
| `InitOrderPage` | 14 | 70 | market, epoch, page index, page count |
| `Endow` | 15 | 74 | market, owner, amount |

`MAX_INTENT_BYTES` is unchanged at 302. That is the point of every decision
below: the intent budget is the width of the widest *transition*, and an
initialization that carried a whole artifact would make every instruction's
envelope as wide as the largest artifact anyone might ever found.

### Identities are derived, never carried

`InitRealm` does not carry a Realm identity, because there is none to carry:
`canonical_realm_id` derives it from exactly `(profile, realm_nonce)`. The
`profile` it does carry is the *parent* Profile identity, which is itself a
total function of the Realm's 266-byte collateral policy through
`collateral::ParentProfile` — so an adapter holding those bytes recomputes the
claim and refuses a mismatch. The same rule runs through the family: a Profile's
identity comes from its policy digest, a grid's and a terms artifact's from
their own bodies, and a page's address from its position in its epoch. Nothing
here lets a caller name an address; a caller names *evidence*, and the address
follows.

Two fields are carried anyway and are checked rather than trusted, for the
reason `PlaceOrder` carries an order id it cannot choose: `InitRealm`'s
`max_outcomes` (V1 admits exactly `MAX_OUTCOMES`) and `InitProfile`'s
`subfield_schema_version`. A caller states the shape it believes it is
creating, and a caller that believes wrong is refused instead of quietly
creating something else.

### Where the bodies travel: the evidence-buffer pattern, made general

A `TermsAccount` is 1,656 bytes and a `PriceGridAccount` is 589. Neither fits an
intent and neither should. Both ride an **evidence buffer**: a read-only account
holding exactly the artifact's encoded bytes, presented from anywhere the caller
likes, authenticated by *recomputation* rather than by address. That is
precisely the pattern the collateral-policy account established (`market_init`'s
`IX_POLICY`), generalized from one artifact to three.

The pattern works here because both artifacts are already **self-certifying**:
`TermsAccount::recomputed_terms_digest` and
`PriceGridAccount::recomputed_grid_id` are decode-time refusals, so a buffer
that decodes has already proved its digest is its own. The intent then carries
that digest, and the adapter compares. A well-formed artifact belonging to
another Realm therefore earns a *binding* refusal, not a decode refusal — the
two are different facts and the adapter reports them as different codes.

The decision recorded plainly, because the alternative was live: `InitProfile`
could have carried the 266 policy bytes in a widened intent. It does not. The
policy already has a carrier, and a second carrier for the same bytes would be
a second truth — the thing this crate spends most of its refusals preventing.
What the intent carries is the digest binding.

### `Endow`, and what it is not

`Endow { market, owner, amount }` credits `PositionAccount::cash_atoms`. It
moves no collateral, touches no Hoard, and is backed by nothing: the value leg
is a Token-2022 `TransferChecked` into the market's Hoard token account, which
is constructed in `clutch-sbf`'s token module and wired by no instruction.

This is not a new hole. It is the existing one, made auditable. The bring-up
harness already conjures opening cash by writing a `cash_atoms` into a genesis
fixture, which `LIFECYCLE_WALK.md` names as the sharpest gap in the walk; a
number that appears in a fixture has no signer, no sequence, no log line and no
ceiling, and an `Endow` has all four. Naming the instruction after the honest
thing it does — an internal-ledger credit — is what keeps a later reader from
mistaking it for a deposit.

## The clearing plane (addendum, 2026-08-19)

`STREAMING_RELATION_DESIGN.md` §10 names two accounts the streaming verifier
needs and assigns both to this crate. They are here, in `src/clearing.rs`, and
**nothing consumes them**: no instruction reads or writes either, and no claim
is made that the streaming verifier has been integrated. What landed is byte
ownership, frozen and adversarially tested before the lane that needs it starts.

| account | tag | version | bytes | shape |
| --- | ---: | ---: | ---: | --- |
| `ClearWorkAccount` | 17 | 1 | 48,750 | 158-byte header + 48,592-byte opaque body |
| `CandidateFeedAccount` | 18 | 1 | 6,266 | 346-byte header + 64 × 8 fills + 416 × 13 slices |

### Neither account is ever a value

The order page taught the 4 KiB lesson at 4,012 bytes; the checkpoint is twelve
times the page. So there is deliberately **no** `decode` returning either
account: every entry point returns a small header by value, walks one element at
a time (`FillCursor`, `SliceCursor`, `fill_at`, `slice_at`), or writes into a
caller-owned slice. The largest value any function in the module holds is one
`CandidateFeedHeader` at 346 bytes. `cargo-build-sbf` emits no frame diagnostic
for anything in `clearing`, which is the measurement rather than the intent.

### The body is opaque, and that is a finding

`CLEAR_WORK_BODY_BYTES = 48,592` is the pinned `size_of::<ClearWorkV1>()` of
`clutch_batch::relation_v1_stream`. That struct is `#[derive(Clone, Debug,
PartialEq, Eq)]` over a plain **`repr(Rust)`** layout, and Rust guarantees
nothing about `repr(Rust)` field order or padding across compiler versions.

So the number is a measurement of one build, not a wire fact, and this crate
refuses to give the body any interpretation at all. It owns the length, the
framing, the identity binding, and two window accessors (`clear_work_body`,
`clear_work_body_mut`) that hand the region to whoever does own it. **Casting
these bytes to a `&mut ClearWorkV1` is not sanctioned by anything here.** The
obligation that would sanction it is on `clutch-batch` and is one of:

1. declare `#[repr(C)]` on `ClearWorkV1` and its five nested value types, add a
   `Pod`/`Zeroable` bound, and re-pin the size — after which a zero-copy cast is
   defensible and the account body becomes the struct; or
2. grow an explicit serializer in `clutch-batch`, at which point this crate's
   length constant follows the serializer's width rather than `size_of`.

Until one of those lands, an integration that byte-casts is unsound, and a
toolchain bump can silently change the required account length. A codec test
pins the constant so the change is a red test rather than a mainnet surprise.

### The consumed-fold binding

§10 assigns the cryptographic anchoring of P-BATCH-03 to this crate: SHA-256
page digests authenticate the *bytes*, the in-crate `mix` fold authenticates the
*walk*. `bind_order_set` is the layout half — it stamps `(order_set,
consumed_fold)` **once**, at pass-1 finalize, and refuses a second stamp — and
`require_continuation` is the refusal a later pass runs into when its epoch
shows a different `order_set`. Neither function verifies a fold; the fold is
`clutch-batch`'s, and saying so is the point.

The header carries one more thing the checkpoint body cannot: the **walk
position**. `ClearWorkV1`'s cursor counts pushes; `page_cursor`/`slot_cursor`
name a page and a slot, and `live_rank` is the relation's order index, which
counts records and not retirements. Those are layout facts and only this crate
knows them. What the header deliberately does **not** restate is the feed phase,
the pass number, the push cursor, the interned owner count, or the running fold
— all of which the body already decides, and restating them would be a second
truth that could disagree with the first.

### One candidate, one identity

`CandidateFeedHeader::recomputed_candidate_digest` uses **exactly** the preimage
`CandidateRecord::recomputed_candidate_digest` uses — epoch, market, order
length, outcome count, prices, sigma, mu, honored mask. So a feed and a record
for the same candidate agree by construction or one of them does not decode. The
feed is not a second candidate account; it is the fill and witness half of the
one candidate, and the record stays the coordinates half.

The feed adds one field the record has no use for: `order_set`, the frozen
page-set digest the fills were computed against. It is required nonzero, because
a fill vector against an unfrozen book is a claim about a book that can still
change.

`declared_slices` is a real `Option<u16>` on the wire, carried as a flag bit
plus a count. "No witness" and "a witness of zero slices" are different feeds —
the second asserts an empty canonical decomposition and the first asserts
nothing — and collapsing them would make an assertion unrepresentable.

### The 10 KiB creation ceiling — the real design point

The Solana runtime caps how much an account's data may grow inside one
instruction at `MAX_PERMITTED_DATA_INCREASE = 10,240` bytes. An account created
through a cross-program invocation grows from zero inside the creating
instruction, so **10,240 bytes is also the largest account a program can
allocate in one CPI**. (The absolute account ceiling,
`MAX_PERMITTED_DATA_LENGTH`, is 10 MiB and is not the binding constraint here.)

| account | bytes | one CPI creation? |
| --- | ---: | --- |
| every protocol account, order page included | ≤ 4,012 | yes |
| candidate feed | 6,266 | yes |
| **clearing checkpoint** | **48,750** | **no** |

The checkpoint is the only account in the inventory that a program cannot
create. Two paths exist and both are real:

* **Client-signed top-level `CreateAccount`.** A `SystemProgram::CreateAccount`
  submitted as a *top-level* instruction is not subject to the per-instruction
  growth cap and allocates 48,750 bytes directly. The cost: the checkpoint
  becomes a keypair-addressed account, not a PDA, so the program must
  authenticate it by its stored `(market, epoch, candidate)` header rather than
  by derivation — which the header is already shaped to support, but which is a
  strictly weaker authentication than every other account in this program has.
* **CPI create, then realloc.** Create a PDA at ≤ 10,240 bytes and grow it by at
  most 10,240 per instruction: `⌈48,750 / 10,240⌉ = 5` instructions, which may
  sit in one transaction because the cap is per instruction. This keeps the
  checkpoint a PDA. The cost: the account is *observable in a partially grown
  state* between instructions, so the header must carry a length field the
  decoder checks — which it does (`body_len`, refused unless it equals
  `CLEAR_WORK_BODY_BYTES`) — and each realloc step must also top up rent.

**Recommendation: the realloc path**, and the reason is authentication rather
than ergonomics. A checkpoint at a keypair address can be substituted; a
checkpoint at `(epoch, candidate)` cannot. The `body_len` field exists precisely
so a half-grown checkpoint is a refusal and not a short read, and the walk
cursor's monotonicity means a partially grown account can never be advanced.
Rent for the whole account at the default parameters is `(128 + 48,750) × 3,480
× 2 = 340,190,880` lamports, about **0.34 SOL**, which is a real number a
clearing crank has to fund and is worth quoting before the design is committed
to.

Neither path is implemented. `instructions::genesis` creates no checkpoint, and
its `create_pda_account` refuses any space above the CPI ceiling outright rather
than emitting a CPI that would fail deeper.

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

Gates run offline and locked: 113 unit tests and 2 doc tests,
`clippy --all-targets -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc
--no-deps`, and `cargo fmt --check`.

The order page's width change makes `benchmarks/cost_lab.py abi-audit` refuse:
its Rust size-expression evaluator has no value pinned for `ORDER_SLOT_BYTES`.
Re-pinning the landed arm — `abi_landed` `order_page` bytes/formula/field terms
and schema version, `layout_version`, and the new slot widths — belongs to the
cost lab, not here.
