# Rent-credit contract

`dclutch-rent-contract` is an SDK-free, `no_std`, `no_alloc`, safe, fixed-layout semantic contract. It has no account-memory access, PDA implementation, Rent deserialization, CPI, wallet inspection, or transaction construction.

The live rent path is `lifecycle_v2`: the Market-generation-scoped `LifecycleRentCreditV2` that tier 1 creates, sweeps, and closes. **The V1 Create and Withdraw instructions were deleted on 2026-08-27** as superseded by it, taking with them the wire and frame grammar, the role/alias policy, `SystemWalletFactsV1`, and `WithdrawBalancePlanV1`. What this document describes below is the V1 *record* and the accounting primitives that survive it. Superseded source: `~/dev/dclutch-legacy/dclutch-rent-credit-v1-routes/`.

## Persistent ownership and width

`RentCreditV1` is exactly 48 bytes:

| byte range | field |
|---|---|
| `0..8` | `DCLTRNT1` magic |
| `8..10` | little-endian schema `1` |
| `10` | derived PDA bump |
| `11..16` | five canonical zero bytes |
| `16..48` | nonzero immutable refund/beneficiary authority |

It is a permanent program-owned account. Data validation never consults observed lamports, so a credit remains admitted even if a later Rent increase makes its balance temporarily below the new minimum.

`RENT_CREDIT_PDA_DOMAIN_V1` is the 22-byte seed domain `dclutch/rent-credit/v1`. The adapter derives one PDA from this domain, the authority, and persisted bump. The semantic crate cannot perform curve/PDA checks.

Legacy source `rent_refund` bytes mean this immutable authority, never a direct payout account. A source close authenticates its stored authority and credits that derived PDA.

**No route creates one of these any more.** The record, its width, and its PDA domain are kept because live code still reads them — most consequentially `dclutch-direct-codec`, which pins `RENT_CREDIT_BYTES_V1` at registered artifact coordinates 7 and 10, where the RentCredit V1/V2 width skew is a known emitter defect owned by DP2. That migration retires the last of V1.

## Balance semantics

`claimable_lamports(observed, current_minimum)` is exactly `observed.saturating_sub(current_minimum)`. It honestly includes unsolicited donations.

`CreateBalancePlanV1` is the exact fund-at-current-Rent-minimum creation plan. It outlived the V1 Create route it was named for: lifecycle V2 Create funds by the same rule and uses it.

`CreditBalancePlanV1` is the generic checked credit-only delta for a Fund split payout or terminal shrink; the surrounding adapter owns total-account conservation. Every complete source close uses the stronger `SourceCloseCreditPlanV1`: source observed balance must equal explicit credited amount, source post-balance is zero, and credit addition is checked. Thus a complete source cannot misclassify a partial or arbitrary amount as credit, while donations still remain normally claimable.

## Hotspot scope

One writable PDA per authority was a V1 operational hotspot. Lifecycle V2's per-Market-generation credit is a finer key and does not have it. This was recorded as a **provisional measured-successor-sharding concern**, not protocol ontology; neither version infers shards or adds a universal treasury/source-account abstraction.
