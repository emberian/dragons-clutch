# Rent-credit contract

`dclutch-rent-contract` is an SDK-free, `no_std`, `no_alloc`, safe, fixed-layout semantic contract. It has no account-memory access, PDA implementation, Rent deserialization, CPI, wallet inspection, or transaction construction.

The live rent path is `lifecycle_v2`: the Market-generation-scoped `LifecycleRentCreditV2` that tier 1 creates, sweeps, and closes. **The V1 Create and Withdraw instructions were deleted on 2026-08-27** as superseded by it, taking with them the wire and frame grammar, the role/alias policy, `SystemWalletFactsV1`, and `WithdrawBalancePlanV1`. What this document describes below is the accounting primitives that survive it. Superseded source: `~/dev/dclutch-legacy/dclutch-rent-credit-v1-routes/`.

## The V1 record is gone

`RentCreditV1` -- a 48-byte permanent program-owned record keyed by refund
authority under the seed domain `dclutch/rent-credit/v1` -- **was deleted on
2026-08-27**, together with `RentCreditPdaSeedsV1`, `RENT_CREDIT_BYTES_V1`, the
PDA domain, the magic, the schema version and every V1 field offset. Its
Create route went first (see above), which meant no such account could come into
existence; the type outlived the route only because two readers were still
compiling against it.

Both readers are gone now. `dclutch-direct-codec` pinned `RENT_CREDIT_BYTES_V1`
at registered artifact coordinates 7 and 10; those are a 128-byte
`LifecycleRentCreditV2` and a Rent program coordinate. Two SVM-harness Markets
planted a V1 record as their rent beneficiary; nothing on any path they exercise
decodes a beneficiary's bytes -- Core compares that account by key and credits
lamports to it -- and they now say so where they plant one.

Superseded source: `~/dev/dclutch-legacy/dclutch-rent-credit-v1-routes/`.

## Balance semantics

`claimable_lamports(observed, current_minimum)` is exactly `observed.saturating_sub(current_minimum)`. It honestly includes unsolicited donations.

`CreateBalancePlanV1` is the exact fund-at-current-Rent-minimum creation plan. It outlived the V1 Create route it was named for: lifecycle V2 Create funds by the same rule and uses it.

`CreditBalancePlanV1` is the generic checked credit-only delta for a Fund split payout or terminal shrink; the surrounding adapter owns total-account conservation. Every complete source close uses the stronger `SourceCloseCreditPlanV1`: source observed balance must equal explicit credited amount, source post-balance is zero, and credit addition is checked. Thus a complete source cannot misclassify a partial or arbitrary amount as credit, while donations still remain normally claimable.

## Hotspot scope

One writable PDA per authority was a V1 operational hotspot. Lifecycle V2's per-Market-generation credit is a finer key and does not have it. This was recorded as a **provisional measured-successor-sharding concern**, not protocol ontology; neither version infers shards or adds a universal treasury/source-account abstraction.
