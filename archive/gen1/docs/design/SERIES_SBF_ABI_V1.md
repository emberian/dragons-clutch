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
| 13 | `RegisterSeries` | SeriesPlanV5Id (32), FundingTermsV2Id (32), RegistryReleaseId (32), CapabilityProfileId (32) |
| 14 | `ActivateFunding` | SeriesPlanV5Id (32) |
| 15 | `AdvanceOccurrence` | SeriesPlanV5Id (32), ordinal LE u32, zero reserved (4), SourceOccurrenceV1Id (32), MarketInstanceV2Id (32) |
| 16 | `LapseOccurrence` | SeriesPlanV5Id (32), ordinal LE u32, zero reserved (4) |
| 17 | `ObserveDonation` | SeriesPlanV5Id (32), component (u8), asset kind (u8), zero reserved (6) |
| 18 | `CloseFunding` | SeriesPlanV5Id (32) |

Local actions `1..=12` in this shared family belong exclusively to SourcePlane
V3. There are no compatibility aliases at the former Series coordinates.

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
| `0x7f/1` | 168 | withdrawn historical Series registration anchor |
| `0x7f/2` | 172 | current BundleV5-retaining Series registration/replay anchor |
| `0x80/1` | 376 | withdrawn QuoteV1-shaped funding/lifecycle wrapper |
| `0x80/2` | 716 | current BundleV5/QuoteV4 six-compartment funding wrapper |

The current 172-byte registration stores SeriesPlanV5Id, FundingTermsV2Id,
RegistryReleaseId, CapabilityProfileId, its exact payer-owned rent principal,
PDA bump, the canonical one-shot `activation_consumed` bit, and the exact
CompiledProductSeriesBundleV5Id. It has no reserved bytes. Activation changes
only that bit from false to true, atomically with
funding/custody creation. It never changes back.
It does not persist `RegistryCapabilityProjectionV2`: the central release stays
the single owner of selector mappings and every value-bearing consumer must
reauthenticate and reconstruct the projection.

The historical 376-byte funding account is exactly:

```text
tag 0x80 | version 1 | bump | zero flags | rent principal LE u64
| five collateral-vault rent principals LE u64 in component order
| SeriesFundingStateV1 (324)
```

The current 716-byte funding account wraps the exact 664-byte
`SeriesFundingStateV2`, retains BundleV5/QuoteV4/AttachmentV4 identities, owns
six component ledgers including SeriesAdmission, and represents one pending
ordinal explicitly. Five collateral-vault rent principals remain because
SeriesAdmission is lamport-only.

The wrappers add no phase, cursor, component amounts, or terminal ownership.
Each stores the exact state-account rent principal, and the funding wrapper also
stores the five exact collateral-vault rent principals, so closes can separate
refundable payer principal from unsolicited account surplus even if runtime
rent parameters later change. Predictable-address prefunding never discounts
the payer. FundingTerms V2 remains the sole owner of the refund destinations,
the receive-only collateral disposition token account, and the distinct
System-owned neutral lamport sink. Collateral residue never reaches the lamport
sink, and rent or donation lamports never reach the token sink. All other facts
remain owned by the pure state, quote, and FundingTerms V2.

The registration PDA persists after funding close as the replay anchor. Its
close remains disabled until a counted-retirement/nullifier successor can
preserve the consumed activation while safely refunding registration rent.

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
donation. Each collateral component is an independently admitted,
release-selected `SegregatedVault`: 165-byte legacy SPL for an admitted legacy
Realm or 170-byte ImmutableOwner Token-2022 for an admitted Token-2022-base
Realm. Its sole signing authority is the Series collateral-authority PDA; its
collateral semantic owner is the SeriesPlanV5Id and its external adapter
compartment is `component + 1` so zero remains invalid. Outcome/Egg issuance is
an independent Token-2022 plane and does not force the collateral family.

Series collateral is operational/passive-liquidity capital. It is never Egg
liability backing and never joins the market Hoard.

## Runtime activation boundary

The reusable immutable-artifact account segment is exactly nine read-only
program-owned content PDAs in this order: SeriesPlan V5, SeriesFundingTerms V2,
ProductTemplate V4, NativeClaimBasis V1, EvidenceOnlyRecoveryPolicy V1,
PriceMeasurePolicy V1, MarketGenesisProfile V2, SeriesFundingQuote V1, and
SeriesAttachmentPlan V1. Only the first two identities come from the
registration payload. Every other expected address and digest is taken from an
already authenticated parent body. The disabled SBF adapter now hostile-decodes
all nine and checks the complete body reference graph; that still does not turn
a caller-built registry projection into registry authentication.

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
  payer-principal component to FundingTerms V2 destinations, sends collateral
  donation residue only to the receive-only neutral token account and lamport
  donation residue only to the System-owned neutral lamport sink, enforces exact
  post-deltas, and closes the rent-funded state to the named lamport-principal
  owner.

Until these joins are simultaneously implemented, the central allocation is
visible but `ENABLED_EXTENSION_ACTIONS` stays empty and the dispatcher refuses
before reading any account.

## Implemented disabled account boundary

The laboratory SBF module now implements the mechanical subset that does not
need any missing semantic receipt:

- program-owner, exact-length, hostile-codec, PDA/bump, stored-rent-principal,
  and current-rent-coverage authentication for registry and funding accounts;
- prefund-safe creation of both accounts, with exact payer rent rather than a
  prefund discount and immediate neutral-sink disposition of account surplus;
- atomic one-shot consumption of the persistent registry replay anchor;
- derivation of the five expected lamport/collateral custody balances from the
  state-owned principal/donation fields;
- exact authentication of five distinct zero-data System-owned lamport PDAs;
- Realm-selected creation and hostile-byte admission of five legacy-SPL or
  Token-2022-base collateral vaults, with prefund swept to neutral before the
  payer supplies each separately persisted exact rent principal;
- free lapse from the real Solana Clock sysvar and the sole ClockPolicy body
  embedded in an authenticated content-addressed Source release, with no
  caller-provided bucket, shadow policy account, or liveness-work spend;
- exact-delta payer funding and PDA-signed component disbursement/refund;
- a private typed donation authority minted only from an observed positive
  lamport-vault surplus, which can authorize no other pure transition; and
- a private terminal receipt binding the consumed registry replay anchor, exact
  closed funding PDA/body, and authenticated FundingTerms/quote graph; and
- account close that returns only stored rent principal to its owner and sends
  account surplus to the distinct System-owned neutral lamport sink.

These helpers are deliberately not dispatched. Collateral-vault post-deltas,
complete activation, occurrence fulfillment, terminal multi-asset ordering,
and authoritative central registry-release authentication still depend on
typed runtime joins under active development.
