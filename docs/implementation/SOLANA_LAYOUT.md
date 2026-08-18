# Transparent V1 Solana layout prototype

Status: offline account-layout and instruction-codec prototype (2026-08-18).
This is not an entrypoint, deployable program, CPI implementation, Token-2022
integration, RPC client, key/signing implementation, or chain-readiness claim.

## Scope and invariants

`programs/solana-layout` is a standalone dependency-free `no_std` Rust crate.
It owns only fixed bytes and deterministic intent bytes. Every account starts
with a one-byte discriminator and `LAYOUT_VERSION = 1`; every decoder requires
the exact byte length, rejects unknown versions/tags, validates enum ranges,
checks nonzero identities, and requires zero canonical padding.

V1 freezes `MAX_OUTCOMES = 16`. Market outcome slots `0..outcome_count` must
equal `SHA-256("dragons-clutch/outcome/v1" || market_id || index)`; remaining
slots are zero. Realm, profile, market, outcome, feed, and epoch identities are
32-byte domain-separated values. The SHA-256 implementation is included to keep
the prototype dependency-minimal; deployment must bind the selected primitive
in an immutable profile before treating an identity as a PDA seed.

Stored bumps are bytes in the layouts and are never recomputed or trusted as a
substitute for adapter PDA/account checks. Owner, authority, destination, and
source fields are opaque 32-byte identities here. Account owner/executable/
signer/writable/alias/PDA checks remain outside this crate.

## Fixed byte inventory

All integers are little-endian. The first two bytes are `(tag, version)`.

| Account | Tag | Exact bytes | Main fields |
| --- | ---: | ---: | --- |
| Realm | 1 | 70 | realm/profile, max outcomes, profile version, bump, flags |
| Profile | 2 | 68 | profile/realm, version, flags |
| Market | 3 | 726 | market/realm/profile/terms IDs, 16 outcome IDs, feed, cap, slot |
| Hoard | 5 | 108 | market/realm/authority, collateral atoms, bump, flags |
| Position | 6 | 220 | market/owner, generation, 16 `u64` balances, cash/reserved cash |
| Feed head | 7 | 124 | feed/realm, cursor/boundary/pages, summary digest, bump |
| Dense order page | 8 | 1656 | market/epoch, page metadata, 16 × 99-byte records |

One instance of every listed account is 2,972 bytes, before Solana account
metadata, rent, transaction-message overhead, or any page multiplicity. This is
the byte-size inventory only; it is not a rent or compute-unit estimate.

The codec tests assert these lengths and round-trip each implemented shape.
Order pages require strictly increasing order IDs and zero unused records;
duplicate, unsorted, malformed, and truncated pages are refused. `OrderRecord`
does not carry a fill or clearing result: matching economics remain transparent
and are verified by the batch relation.

## Intent bytes

Intent bytes use the same `(tag, version)` prefix and no variable-length fields:
CreateMarket (139 bytes), Split/Merge (74), Materialize/Dematerialize (107),
FeedAdvance (74), PlaceOrder (165), CancelOrder (130), and SettlePage (68).
`Intent::encode` writes into caller-owned storage; `Intent::decode` accepts only
the exact length implied by its tag. Zero quantities, invalid outcomes, zero
identities, invalid order flags, and unsupported tags are refusals. The intent
is data for a future adapter, not authority to sign or submit anything.

## Seam to the semantic crates

The seam is deliberately one-way:

1. A future native adapter authenticates Solana account metadata and hostile
   instruction accounts, then parses these bytes.
2. It converts checked `PositionAccount` balances and `MarketAccount` terms into
   the already-owned `clutch-kernel::MarketState`/`Position` values. Kernel
   transitions return logical state changes and CPI intents; this codec does not
   duplicate those transitions or invent amounts.
3. It maps `FeedAccount.summary`/cursor evidence to an authenticated input for
   `clutch-accumulator::Summary`; the accumulator owns coverage and exact
   interval-summary algebra, not feed account bytes.
4. It maps dense `OrderPageAccount` records to the transparent
   `clutch-batch::FixedBook` relation. The batch crate owns eligibility, fills,
   conservation, tie rules, and its “best valid submitted candidate” wording;
   this crate owns only page bytes and order identity/order.

No account parser may write a Position directly from an external venue. A
future adapter must check all aliases and authenticated mints/programs before
applying the kernel's logical writes. CPI construction, return-data checking,
clock/replay policy, and SBF/runtime behavior are explicit unverified seams.

## Non-goals and evidence

There is no Solana dependency, account-info type, PDA derivation, Token-2022
dependency, RPC read, key material, signing, deployment, or entrypoint. Tests
are host-side golden/adversarial codec tests only; they provide no chain or
runtime evidence and do not establish formal verification or chain readiness.
