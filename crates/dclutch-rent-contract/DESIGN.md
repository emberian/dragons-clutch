# Rent-credit contract V1

`dclutch-rent-contract` is an SDK-free, `no_std`, `no_alloc`, safe, fixed-layout semantic contract. It has no account-memory access, PDA implementation, Rent deserialization, CPI, wallet inspection, transaction construction, or close operation.

## Persistent ownership and width

`RentCreditV1` is exactly 48 bytes:

| byte range | field |
|---|---|
| `0..8` | `DCLTRNT1` magic |
| `8..10` | little-endian schema `1` |
| `10` | derived PDA bump |
| `11..16` | five canonical zero bytes |
| `16..48` | nonzero immutable refund/beneficiary authority |

It is a permanent program-owned account. V1 intentionally has no close, drain, migration, or caller-selected redirect action. Data validation never consults observed lamports, so a credit remains admitted even if a later Rent increase makes its balance temporarily below the new minimum.

`RENT_CREDIT_PDA_DOMAIN_V1` is the 22-byte seed domain `dclutch/rent-credit/v1`. The adapter derives one PDA from this domain, the authority, and persisted bump, and verifies derivation for Create and Withdraw. The semantic crate cannot perform curve/PDA checks.

Legacy source `rent_refund` bytes mean this immutable authority, never a direct payout account. Before a source stores that authority, the adapter establishes the associated credit PDA with Create. A source close authenticates its stored authority and credits that derived PDA.

## Wire and frame grammar

All wires begin with the RentCredit-specific `DCLTRCI1`, schema `u16=1`, an action byte, and five zero reserved bytes. This domain is deliberately distinct from immutable-record construction. Create is exactly 56 bytes: 16-byte header, 32-byte nonzero authority, one bump, and seven zero reserved bytes. It is permissionless; authority is payload data and not an account or signer. Its ordered frame is `[payer, vacant_credit, SystemProgram, Rent]`. Payer is signer+writable and creation funds exactly the current Rent minimum.

Withdraw is exactly 24 bytes: 16-byte header plus nonzero little-endian `u64` requested amount. Its ordered frame is `[credit, authority, recipient, Rent]`. Credit is writable non-signer. A distinct authority is only checked as the exact bound key and a readonly signer; the contract intentionally does not inspect its owner, data, executable status, or lamports. Recipient is a separately supplied writable, nonexecuting, data-empty, System-owned wallet authenticated by the adapter. Rent is canonical and readonly. Authority may equal recipient only under merged runtime signer+writable privilege union; every other role alias is refused.

## Balance semantics

`claimable_lamports(observed, current_minimum)` is exactly `observed.saturating_sub(current_minimum)`. It honestly includes unsolicited donations. A withdrawal is nonzero, exact, and at most claimable. Its plan keeps the current minimum floor and uses checked recipient addition.

`CreditBalancePlanV1` is the generic checked credit-only delta for a Fund split payout or terminal shrink; the surrounding adapter owns total-account conservation. Every complete source close uses the stronger `SourceCloseCreditPlanV1`: source observed balance must equal explicit credited amount, source post-balance is zero, and credit addition is checked. Thus a complete source cannot misclassify a partial or arbitrary amount as credit, while donations still remain normally claimable.

## Hotspot scope

One writable PDA per authority is a V1 operational hotspot. This is a **provisional measured-successor-sharding concern**, not protocol ontology. A later independently versioned successor may add a measured shard scheme with an explicit authority-to-shard commitment. V1 does not infer shards or add a universal treasury/source-account abstraction.
