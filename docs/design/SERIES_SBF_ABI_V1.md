# Recurring Series SBF ABI V1

Status: **FROZEN NON-PRODUCTION BYTES / RUNTIME DISABLED** (2026-08-23)

This document fixes the first account and instruction namespace for the V5
recurring-Series core. It does not claim that decoding a claimed registry ID,
SourcePlane record, collateral account, or funding body authenticates it. The
executable extension capability set remains empty until every action's complete
adapter join exists.

## Wire allocation

The outer reference request remains unchanged. Its inner layout bytes use the
Source/Series family `(77 decimal, version 2)`:

```text
family 77 | version 2 | local action | exact action payload
```

| local action | name | exact payload |
| ---: | --- | --- |
| 1 | `RegisterSeries` | SeriesPlanV5Id (32), FundingTermsV2Id (32), RegistryReleaseId (32), CapabilityProfileId (32) |
| 2 | `ActivateFunding` | SeriesPlanV5Id (32) |
| 3 | `AdvanceOccurrence` | SeriesPlanV5Id (32), ordinal LE u32, zero reserved (4), SourceOccurrenceV1Id (32), MarketInstanceV2Id (32) |
| 4 | `LapseOccurrence` | SeriesPlanV5Id (32), ordinal LE u32, zero reserved (4) |
| 5 | `ObserveDonation` | SeriesPlanV5Id (32), component (u8), asset kind (u8), zero reserved (6) |
| 6 | `CloseFunding` | SeriesPlanV5Id (32) |

Amounts are deliberately absent. Activation principal comes only from the
authenticated `SeriesFundingQuoteV1 × SeriesPlanV5.instance_count` join.
Occurrence debit comes only from authenticated exact-existing/absent component
receipts. Donation is only actual custody surplus over the state-owned accounted
balance. Terminal destinations and amounts come only from FundingTerms V2 and
the closed funding state.

`clutch_solana_layout::product_series` owns these codecs. The action allocation
does not enable dispatch.

## Accounts

The global account ledger reserves:

| tag/version | bytes | semantic owner |
| --- | ---: | --- |
| `0x7d/1` | 168 | immutable Series registration |
| `0x7e/1` | 336 | mutable Series funding/lifecycle wrapper |

The 168-byte registration stores only SeriesPlanV5Id, FundingTermsV2Id,
RegistryReleaseId, CapabilityProfileId, its exact payer-owned rent principal,
PDA bump, and zero flags/reserved bytes.
It does not persist `RegistryCapabilityProjectionV2`: the central release stays
the single owner of selector mappings and every value-bearing consumer must
reauthenticate and reconstruct the projection.

The 336-byte funding account is exactly:

```text
tag 0x7e | version 1 | bump | zero flags | rent principal LE u64
| SeriesFundingStateV1 (324)
```

The wrappers add no phase, cursor, component amounts, or terminal ownership.
Each stores the exact rent principal so a close can separate refundable payer
principal from unsolicited account surplus even if runtime rent parameters
later change. FundingTerms V2 remains the sole owner of the refund destination;
surplus is donation residue for its neutral sink. All other facts remain owned
by the pure state, quote, and FundingTerms V2.

## Address schema and custody

All seeds below are exact byte strings:

```text
Series registry       ["dc:series-registry:v1", SeriesPlanV5Id]
Series funding        ["dc:series-funding:v1", SeriesPlanV5Id]
Lamport component     ["dc:series-lamports:v1", SeriesPlanV5Id, component]
Collateral authority  ["dc:series-collateral-auth:v1", SeriesPlanV5Id]
Collateral component  ["dc:series-collateral:v1", SeriesPlanV5Id, component]
Source occurrence     ["dc:source-occurrence:v1", SourceOccurrenceV1Id]
```

Components are the closed discriminants `0..=4` in quote/state order. Each
lamport component is a zero-data, System-owned PDA custody address. The funding
state is the semantic balance owner; the physical vault balance is an
authenticated observation and may exceed the accounted balance only as
donation. Each collateral component is an independently admitted Token-2022
`SegregatedVault`. Its sole signing authority is the Series collateral-authority
PDA; its collateral semantic owner is the SeriesPlanV5Id and its external
adapter compartment is `component + 1` so zero remains invalid.

Series collateral is operational/passive-liquidity capital. It is never Egg
liability backing and never joins the market Hoard.

## Runtime activation boundary

No action may be enabled from the codecs alone. At minimum:

- `RegisterSeries` must authenticate every exact Product/Series artifact PDA,
  the immutable Realm/Profile collateral binding, the central registry release
  and selector mapping, SourcePlane program/spec/summary identities, and the
  selected liveness policy. A Rust trait implementation or caller-supplied
  projection is not authentication.
- `ActivateFunding` must enforce the registry and funding PDAs, program owner,
  exact empty-state/replay condition, rent-exempt post-state, all five lamport
  vault addresses and exact post-deltas, all five collateral vault admissions
  and exact post-deltas, and the quote-derived total without cross-component
  borrowing.
- `AdvanceOccurrence` must consume the exact SourcePlane occurrence receipt,
  the successor failure admission receipt for the same Series/ordinal/V2 market
  and funding quote, the exact liveness funding receipts, the collateral
  transfer receipts, and the authenticated Clock window. It applies child
  effects and the pure funding-state write atomically. Exact-existing components
  debit zero; absent components debit exactly one quote allocation.
- `LapseOccurrence` must authenticate the Clock and next ordinal, spend no
  component, produce no paid-work receipt, and apply only the pure cursor/lapse
  transition.
- `ObserveDonation` derives one positive delta from the named physical balance;
  the request never supplies an amount.
- `CloseFunding` requires the pure state to be closed, refunds every remaining
  payer-principal component to FundingTerms V2 destinations, sends only donation
  residue to the immutable neutral sink, enforces exact post-deltas, and closes
  the rent-funded state to the named lamport-principal owner.

Until these joins are simultaneously implemented, the central allocation is
visible but `ENABLED_EXTENSION_ACTIONS` stays empty and the dispatcher refuses
before reading any account.
