# Pyth child contracts

Status: SDK-free semantic contracts. This crate does not authenticate Solana
accounts, hash content, read Pyth, calculate rent, invoke CPI, or transfer
lamports. Those remain explicit adapter boundaries.

## Resolution Fund authority

`ResolutionFundV1` is the physical one-shot funding child for exactly one
Market identity and generation. Its sponsor identity is the only destination
for physical excess after the committed obligations are paid. The Fund embeds
the canonical `dclutch-capability-contract::FundingStateV1` by value. It does
not persist parallel provider-fee or bounty fields.

The adapter authenticates the immutable capability manifest and its content
identity, selects the unique `RequiredAtFounding` entry whose config is the
Market's resolution-policy identity, and computes exact rent for
`FUNDING_BYTES`. Construction validates the current specialized profile:

- quoted rent equals exact Fund rent;
- bounty principal is positive;
- creation, work, liquidity, and service principal are zero; and
- provider principal is the exact committed provider reimbursement.

Construction first creates an exactly prepaid Pending `FundingStateV1`, then
models its RequiredAtFounding activation in the same transition. Rent is moved
to `released.rent_principal`, representing the physical Fund account's rent.
Creation is exactly zero. Provider and bounty remain the only non-rent
principal held by the Fund. The adapter must create and fund the physical
account atomically with persistence; a semantic return is not evidence that a
transfer occurred.

The resulting width is exactly 280 bytes:

| Component | Bytes |
| --- | ---: |
| Fund header | 16 |
| Market identity | 32 |
| generation | 8 |
| sponsor refund identity | 32 |
| canonical `FundingStateV1` | 192 |

This replaces the old 112-byte record, a 168-byte increase. The increase is
deliberate: the record now carries one complete manifest binding, entry index,
activation fact, and per-compartment conservation ledger instead of a second
specialized funding truth. Actual lamport rent must be measured from the
cluster's Rent sysvar for 280 bytes; this document does not hard-code or claim
a stable chain cost.

## Physical balance and release seam

`minimum_balance()` derives the minimum solely from the embedded ledger:

```text
released rent + remaining provider + remaining bounty
```

`classify_balance()` refuses underfunding and binds any excess to the immutable
sponsor. Before relying on either result, the adapter calls `validate_against`
with the authenticated manifest content identity, manifest bytes, freshly
calculated exact rent, and the physically observed held non-rent principal.

Provider reimbursement and bounty release must use `FundingStateV1::release`
and execute the exact lamport transfers atomically with the updated Fund or its
terminal close. A later SBF route must not read an instruction-authored amount,
reconstruct a parallel funding DTO, or treat sponsor excess, Market Hoard
principal, or expected future fees as an obligation source.

## Bounds and lifting

All widths here are **schema bounds**: exact fixed-layout facts of release V1.
The `u64` amount and slot widths are **chain-derived representation choices**
aligned with Solana lamports and slots. Every sum is checked and every decoder
requires exact length and canonical reserved bytes.

The categorical outcome bounds used by the other Pyth contracts are their
current **provisional artifact-profile bounds**, not a limitation imposed by
Fund funding. Lifting them requires a newly identified wider policy/ledger
profile and corresponding exact child layouts. It does not require adding
outcome arrays to `ResolutionFundV1`, whose funding ledger is outcome-agnostic.

Rent and compute effects are **unmeasured** until the real SBF adapter is built
and exercised against a pinned local Solana toolchain. The lifting plan for
the 192-byte generic funding ledger is schema composition or child-account
separation only if measurement shows material value; duplicating provider and
bounty fields is not an acceptable rent optimization because it reinstates a
second semantic authority.
