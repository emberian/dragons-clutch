# Failure-policy runtime join

Status: **PURE SUCCESSOR CONTRACT WITH FIXED STATE CODEC / NOT AN SBF ROUTE**.

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

The live maturity routes consume `FailurePolicySourceHandoffV1`, not a caller
assertion or raw result DTO. Admission freezes the exact authenticated
Product/Series occurrence receipt and Clock-policy identities. The no-result
path requires the predictable unallocated StatisticResult PDA plus permanent
never-created lineage; the refused path requires the exact account-authenticated
Window evidence and evaluator result. Both handoffs bind the immutable primary
Window and derive the recovery Clock from the same frozen bucket policy.

Admission creates a real `RecoveryState` through `admit_v2`: the exact
`SeriesFundingQuoteV1` work principal and separate rent principal must already
have reached the reserve, while prior prefund remains a donation. The returned
private-field admission receipt is keyed by `SeriesPlanV5Id`, ordinal,
`MarketInstanceV2Id`, `SeriesFundingQuoteId`, recovery state, and generation.
Series code may consume that receipt; it must not copy or reinterpret budget
balances.

The canonical runtime codec persists the complete immutable binding, primary
Window, funded recovery state, replay nonce, and optional first maturity
trigger. Account headers, PDAs, owner checks, sysvar parsing, transfers, and
instruction dispatch remain adapter work; there is still no central SBF route.

Finite repair windows derive from the compiled schedule and SourcePlane
generation. Accepted work is still paid only by the recovery core's monotone
progress transition. A liveness work receipt additionally binds the exact
FundingQuote ID, accepted-progress delta, rate, remaining maximum, and its own
authenticated per-call ceiling before its receipt ID becomes the Work identity.
On exhaustion, the same core sends unused work principal
and donations to the immutable neutral sink, refunds only rent principal, and
enters recoverable dormancy. Hoard principal, claim backing, owner cash, fees,
future revenue, and treasury funds do not appear in this API.

An accepted-resolution capability binds a successful exact SourcePlane result
to the frozen relation-policy identity and an adapter-authenticated resolution
record. It does not expose a resolver identity. A dormant market can still
resolve caller-funded. Resolved and dormant phases emit distinct typed receipts
for closing the finite liveness Recovery compartment as success or failure;
dormancy does not settle or retire the market. The full lifecycle terminal join
is a separate typed output, available only after resolution plus independently
authenticated retirement, replay tombstone, and source release. These outputs
are the receipts projected into liveness and never consume a liveness terminal
receipt as an input, avoiding a cyclic authorization graph. This crate cannot
infer zero liabilities or retire those owners itself.
