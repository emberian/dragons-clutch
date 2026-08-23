# Failure-policy runtime join

Status: **PURE SUCCESSOR CONTRACT / NOT AN SBF ROUTE OR ACCOUNT ABI**.

This crate joins the current successor Terms owners (`ProductTemplateV4`,
`MarketGenesisProfileV2`, `SeriesPlanV5`, and the complete capability
projection), SourcePlane V3, and the independently funded
`clutch-evidence-recovery` state without creating a second payout, schedule, or
budget truth.

The only implemented consequence is `EvidenceOnlyRecoveryV1`. A missing,
unusable, ambiguous, or stale submission never selects a payout. The economic
trigger is the immutable primary maturity bucket from `CompiledOrdinalV2`.
Authenticated source/evaluator and relation refusals may classify the trigger,
but they cannot move it earlier or change its transfers. Stale or wrong-
generation source objects fail their exact SourcePlane joins and are not proof
that the source failed.

Admission creates a real `RecoveryState` through `admit_v2`: the exact
`SeriesFundingQuoteV1` work principal and separate rent principal must already
have reached the reserve, while prior prefund remains a donation. The returned
private-field admission receipt is keyed by `SeriesPlanV5Id`, ordinal,
`MarketInstanceV2Id`, `SeriesFundingQuoteId`, recovery state, and generation.
Series code may consume that receipt; it must not copy or reinterpret budget
balances.

Finite repair windows derive from the compiled schedule and SourcePlane
generation. Accepted work is still paid only by the recovery core's monotone
progress transition. On exhaustion, the same core sends unused work principal
and donations to the immutable neutral sink, refunds only rent principal, and
enters recoverable dormancy. Hoard principal, claim backing, owner cash, fees,
future revenue, and treasury funds do not appear in this API.

An accepted-resolution capability binds a successful exact SourcePlane result
to the frozen relation-policy identity and an adapter-authenticated resolution
record. It does not expose a resolver identity. A dormant market can still
resolve caller-funded. A terminal join is only a typed boundary to separately
authenticated retirement, replay tombstone, and source-release owners; this
crate cannot infer zero liabilities or retire those owners itself.
