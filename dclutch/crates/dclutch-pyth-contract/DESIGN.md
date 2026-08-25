# Pyth child contracts

Status: SDK-free semantic contracts. This crate does not authenticate Solana
accounts, hash content, read Pyth, calculate rent, invoke CPI, or transfer
lamports. Those remain explicit adapter boundaries.

## Resolution Fund authority

The physical Pyth Resolution Fund is exactly the canonical
`dclutch-capability-contract::FundingStateV1`. The Pyth crate adds no wrapper
header and persists no second copy of Market key, Market generation, sponsor
refund identity, provider reimbursement, or bounty.

Those omitted occurrence facts remain authenticated without Fund duplication:

- the generic `CapabilityFundingDerivationV1` PDA and its owner bind it to the
  authenticated Market occurrence and selected immutable manifest entry;
- the Market root owns the current generation and manifest content identity;
- the Market root's immutable `rent_refund` derives the sole permanent
  RentCredit receiving physical excess and Fund-closure residuals; and
- the manifest entry's funding quote solely owns rent, provider, and bounty.

The adapter authenticates the immutable capability manifest and its content
identity, selects the unique `RequiredAtFounding` entry whose config is the
Market's resolution-policy identity, and computes exact rent for
`FUNDING_BYTES`. `construct_required_resolution_funding` validates the current
specialized profile:

- quoted rent equals exact Fund rent;
- bounty principal is positive;
- creation, work, liquidity, and service principal are zero; and
- provider principal is the exact committed provider reimbursement.

Construction first creates an exactly prepaid Pending state and then models
its RequiredAtFounding activation in the same transition. Rent moves to
`released.rent_principal`, representing physical account rent. Creation is
exactly zero. Provider and bounty remain the only held non-rent principal. The
adapter must create and fund the physical account atomically with persistence;
a semantic return is not evidence that a transfer occurred.

The exact physical width is 192 bytes, identical to `FundingStateV1`:

| Geometry | Bytes |
| --- | ---: |
| Obsolete specialized record before canonical composition | 112 |
| Rejected outer wrapper around canonical state | 280 |
| Raw canonical Fund | 192 |

The correction removes 88 bytes from the rejected wrapper. Relative to the
old 112-byte specialized record, canonical authority costs 80 bytes rather
than 168. Actual lamport rent must be calculated from the cluster's current
Rent sysvar; this document does not hard-code a chain cost.

## Physical balance and release seam

`required_resolution_minimum_balance` derives the minimum solely from the raw
ledger:

```text
released rent + remaining provider + remaining bounty
```

Before using that minimum, the adapter calls
`validate_required_resolution_funding` with the authenticated manifest content
identity, manifest bytes, unique entry selected from the Market's resolution
policy, freshly calculated exact rent, and physically observed held non-rent
principal. It refuses wrong bindings, selection, activation status, rent,
compartments, conservation, and observations.

The adapter refuses a physical balance below the minimum. It credits any excess
to the permanent RentCredit derived from the authenticated Market root's
`rent_refund`; there is deliberately no Fund-local refund fact or Pyth
balance-classification DTO.

Provider reimbursement and bounty release use `FundingStateV1::release` and
execute the exact lamport transfers atomically with the state mutation or
terminal account close. A composing route must not read an
instruction-authored amount, reconstruct a parallel funding DTO, or treat
sponsor excess, Market Hoard principal, or expected future fees as an
obligation source.

## Bounds and lifting

The 192-byte width is an exact **schema bound** of canonical
`FundingStateV1`. Its `u64` amount and slot widths are **chain-derived
representation choices** aligned with Solana lamports and slots. Every sum is
checked and its one decoder requires exact length and canonical reserved bytes.

The categorical outcome bounds used by the other Pyth contracts are current
**provisional artifact-profile bounds**, not limitations imposed by funding.
Lifting them requires a newly identified wider policy/ledger profile and exact
child layouts. It does not require outcome arrays in the Fund, whose canonical
funding state is outcome-agnostic.

Rent and compute effects are **unmeasured** until the real SBF adapter is built
and exercised against a pinned local Solana toolchain. If measurement later
justifies a different physical funding layout, its lifting path is a new
canonical capability-funding schema. Reintroducing duplicate Pyth provider,
bounty, occurrence, or refund facts is not an acceptable rent optimization.
