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
`LAYOUT_VERSION` is the largest of them (`3`), not one wire version shared by
every account. An account keeps the version its current bytes were introduced
at; an account whose bytes change moves to the next version and refuses every
earlier one explicitly with `WrongVersion`, so the pair `(tag, version)` never
names two different shapes.

| Account | Version | Change |
| --- | ---: | --- |
| Realm, Market, Hoard, Position, Feed head | 1 | bytes unchanged |
| Profile | 2 | gained the 32-byte collateral-policy digest |
| Supply ledger, Terms, Epoch, Price grid, Candidate, Final pot, Receipt, Resolution | 2 | introduced at 2 |
| Dense order page | 3 | version 2 gained the page-set commitment fields; version 3 replaced its bare 99-byte records with tagged fixed-width order slots and refuses both 1 and 2 |

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
| Dense order page | 8 | 3883 | market/epoch, 5 page-set commitments, page metadata, 16 × 228-byte tagged order slots |
| Supply ledger | 9 | 333 | market/realm, generation, 16 internal + 16 external `u64` |
| Immutable terms | 10 | 1304 | terms digest, realm/profile/feed/price-grid, 8 × payout vector, window policy, failure policy |
| Epoch (book domain) | 11 | 328 | epoch/market/book/terms/grid/policy/order-set IDs, order range, shape, seed, phase |
| Price grid | 12 | 589 | grid identity, realm, price scale, 64 `u64` ticks |
| Candidate record | 13 | 305 | candidate digest, epoch/market, 16 prices, sigma/mu, AON mask, score, status |
| Final pot | 14 | 262 | epoch/market/candidate, 16 pot balances, pot cash, rounding pot, phase |
| Settlement receipt | 15 | 217 | epoch/market/candidate, buy/sell order ids, slice, quantity, price, consideration, consumed flags |
| Resolution | 16 | 165 | market/terms/feed, sealed window digest, cursor, repair generation, payout index |

One order slot is 228 bytes: a one-byte kind discriminator, that kind's exact
body (99 bytes single-Egg, 227 bytes portfolio), and canonical zero padding out
to the common width. The 235-byte page header is unchanged.

One instance of every listed account is 8,734 bytes; a market whose epoch book
uses the full four pages is 20,383 bytes. This is the byte-size inventory only;
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
- each page opens strictly above the previous page's last order id, which makes
  the order-id sequence strictly increasing across the whole set, not per page;
- every non-final page of a frozen set is dense and the final page closes the
  count exactly;
- the per-page order counts sum to the committed set order count;
- the portfolio records across the whole set do not exceed
  `MAX_PORTFOLIO_ORDERS = 8`, which a single page cannot decide; and
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
because a page alone can only bound an outcome index by `MAX_OUTCOMES` — refuses
any single-Egg outcome at or above, or any portfolio `active_len` above, the
epoch's own `outcome_count`.

While an epoch is open it commits to nothing: order-set digest, order range,
page count, and order count must all be zero, and any nonzero value there is
refused as noncanonical padding rather than treated as a stale hint.

## Limit-to-tick mapping

`OrderRecord.limit` remains an opaque `u64` on the venue scale and its 99-byte
body is unchanged. The frozen mapping to the relation's tick domain lives in
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
width: every slot is `ORDER_SLOT_BYTES = 228` bytes even though a single-Egg body
is 99, which is why the page grew from 1,819 to 3,883 bytes. That is not slack.
The unused tail of a single-Egg slot is *required* to be zero, exactly like every
other padded field in this crate, so a slot has one encoding, the account keeps
one exact length, and padding can never influence a digest.

### Slot layout

A slot is a one-byte kind discriminator, that kind's exact body, and canonical
zero padding to the common width. Kind `0` is padding and the whole slot is
zero; kind `1` is a single-Egg `OrderRecord`; kind `2` is a `PortfolioRecord`.
Any other kind byte is `WrongTag`, and a nonzero byte anywhere in a slot's
padding is `NonCanonicalPadding` — including an all-zero record smuggled into a
padding slot under a real kind byte, which is a record and not padding.

Kind 1, single-Egg (`ORDER_RECORD_BYTES = 99` of body):

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
| 100 | 128 | zero padding to the common width |

Kind 2, portfolio (`PORTFOLIO_RECORD_BYTES = 227` of body, no padding):

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

The single-Egg body is byte-identical to the previous 99-byte record and simply
moved one byte right, behind its kind discriminator. A unit test pins every
offset in both tables against the encoder, so the tables and the codec cannot
drift apart.

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

The page digest domain moved to `dragons-clutch/order-page/v2` because its
preimage shape changed. The order-set fold keeps `dragons-clutch/order-set/v1`:
its preimage — market, epoch, page count, order count, page digests — did not
change, and the leaves it folds already carry the new domain.

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
| `canonical_order_id: u64` | the record's rank in the verified page set, plus one — the set already fixes one strictly increasing order, so rank is nonzero, strictly increasing, and order-preserving by construction |
| `expiry_epoch: u64` | **not persisted by any record.** An adapter must supply it from an authenticated source; the single-Egg family has the same hole and always did |
| — | `generation` is replay protection for the placing instruction and has no relation field |

The two identity rows are conversions the adapter performs, not values this
crate stores, and neither is checked here: nothing in this crate proves that an
owner tag is a bijection into `owner_count`, and nothing proves that a caller
ranked the set the way the relation will. What the crate does guarantee is that
the ranking exists and is unique — `verify_page_set` establishes a single
strictly increasing order-id chain across both families and the whole set — so
the conversion is well defined rather than a choice.

Correspondingly, these relation refusals are *not* pre-empted here and remain
`clutch-batch`'s: `active_len <= domain.outcome_count` (checked here only against
the epoch, not the relation domain), `owner < domain.owner_count`,
`expiry_epoch >= domain.epoch`, the all-or-none admission policy, and every
eligibility, fill, conservation, and pairing question. A byte-valid portfolio
record is not an admissible order.

## Streaming page decoders (addendum, 2026-08-18)

The SBF foundation lane measured a hard blocker rather than predicting one: on
the pinned `cargo-build-sbf` toolchain, `OrderPageAccount::decode` is reported
at an estimated **8,640-byte** call frame and `decode_on_grid` at **8,320**,
against SBF's 4,096-byte per-frame maximum. The v3 page — a 235-byte header and
16 tag-discriminated 228-byte slots, 3,883 bytes in all — could therefore not be
read on-chain at all, and `clutch-sbf` compiles its `read_order_page` wrapper
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

`OrderPageHeader` is every page field except the slots. `OrderSlotCursor`
decodes exactly one `ORDER_SLOT_BYTES` slot per step — kind byte, that kind's
exact body, canonical zero padding to the common width — and carries across
steps the two facts a single slot cannot decide: that a slot below
`order_count` holds a valid record whose order id is strictly above its
predecessor, and that a slot at or above it is canonical padding. A refused step
fuses the cursor.

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
`OrderPageHeader::validate_shape` deliberately decides only what the first 235
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
   `clutch-batch::relation_v1::RelationDomainV1`, each decoded order slot to a
   `SingleEggOrderV1` or `PortfolioOrderV1` under the field-by-field contract in
   the portfolio addendum above, and `CandidateRecord` to that relation's
   candidate witness. The batch crate owns eligibility, fills,
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

Gates run offline and locked: 53 unit tests and 2 doc tests,
`clippy --all-targets -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc
--no-deps`, and `cargo fmt --check`.

The order page's width change makes `benchmarks/cost_lab.py abi-audit` refuse:
its Rust size-expression evaluator has no value pinned for `ORDER_SLOT_BYTES`.
Re-pinning the landed arm — `abi_landed` `order_page` bytes/formula/field terms
and schema version, `layout_version`, and the new slot widths — belongs to the
cost lab, not here.
