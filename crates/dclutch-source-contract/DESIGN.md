# Source-contract V1

`dclutch-source-contract` is the no-std, no-alloc, SDK-free semantic owner for
provider-neutral Source resolution. It owns canonical bytes, exact integer
evaluation, lifecycle transitions, instruction grammar, and ordered account
roles. It does not hash content, derive Solana addresses, inspect accounts,
invoke providers, read Clock, transfer principal, settle a Market, or close an
account.

## Immutable truth and provider boundary

`ProviderReleaseV1` names the reviewed provider parser/normalizer boundary.
`SourceSpecV1` owns observation domain, signed-atom unit, provider release,
access profile, adapter configuration, and capacity profile.
`ResolutionPolicyV1` is the one Product occurrence linkage to source, window,
statistic, result mapping, and optional recovery policy. Static clients and
indexes are untrusted projections.

`NormalizedProviderEvidenceV1` is a canonical 208-byte provider-neutral record.
It binds source ID, provider-release ID, provider-evidence content ID, adapter
release ID, schedule ID and index, observation/publication Unix times, and one
signed `i128` value. These normalized bytes are not a provider attestation.
Before calling the pure contract, the selected adapter release must authenticate
the real provider evidence and every supplied content ID. The contract then
checks immutable linkage, ordered schedule position, closed window, and
Clock-relative age/future skew.

## Exact statistics and Product mapping

Values are signed `i128` atoms. Scheduled averages retain exact `sum / count`
until the named `ExactRational`, `Floor`, or `Ceiling` boundary. There are no
floats or implicit divisions.

`OddScheduledMedian` is allocation-free and exact. Its sample count is odd and
at least three, its window is `ScheduledInterval`, timestamps are strictly
ordered and unique, and samples occupy the exact equal cadence from the closed
window start through end. Selection is bounded O(n²) under the authenticated
capacity profile. It returns the exact median atom with denominator one; it does
not add another rounding boundary. Provider-specific TWAP semantics are not in
this release.

`FiniteResultMapV1` is the closed built-in Product mapping release. Its fixed
512-byte profile admits at most sixteen exhaustive ordered rational regions.
Strictly increasing rational boundaries and fixed selectors map a statistic to
one finite Product cell without overflow-prone cross multiplication. The same
artifact owns the failure selector, so no accept or failure instruction carries
a caller-selected result. `SourceResolutionDecisionV1` exposes only the derived
primary/recovery/failure route, selector, outcome count, evidence ID, and
positive terminal sequence for a neutral Market adapter.

Sixteen regions are a provisional artifact-profile bound. Lifting it requires a
new mapping release/preimage and does not reinterpret existing content.

## Persisted resolution lifecycle

`SourceResolutionStateV1` is exactly 224 bytes and its PDA seed tuple is:

`dclutch/source-state/v1`, Market key, little-endian generation, bump.

It binds Market key, immutable generation, resolution-policy ID, a pre-existing
RentCredit beneficiary authority, optional reopen-link ID, terminal evidence,
and terminal sequence. Its only phase graph is:

`Primary -> Recovery(i) -> Recovery(i+1) -> Resolved`

or:

`Primary/Recovery(last) -> Exhausted -> FailureCommitted -> Retired`.

Primary can enter only recovery attempt zero. An expired recovery leg can enter
only its immediate successor. Only the final leg can exhaust. Failure semantics
are refused before explicit exhaustion. Accept computes statistic and Product
mapping inside the transition; it cannot accept a caller-selected success.
Every mutation checks the persisted Market generation. `Retired` retains the
terminal decision until the adapter closes the account into the beneficiary's
RentCredit.

The core Market identity currently has no predecessor field. `ReopenLinkV1` is
a narrow 128-byte immutable preimage that binds Market key, predecessor-state
content ID, predecessor terminal-evidence ID, previous generation, and exactly
`previous + 1`. `SourceResolutionStateV1::reopened` requires an authenticated
link but does not alter or compete with Market authority.

## Ordered recovery and funding seam

Every recovery attempt exposes exact source, provider release, inclusive Unix
deadline, and capability funding-allocation IDs. Attempts are strictly ordered
and exact indexed access refuses inactive slots.

The source contract never stores funding amounts. On `fail_next` and recovery
acceptance it compares the attempt's immutable allocation ID with an
`authenticated_funding_allocation_id`. This is the deliberately narrow adapter
seam: the adapter must obtain that identity only after authenticating the actual
capability manifest and `FundingStateV1`, checking its binding/conservation, and
observing that remaining principal is presently held. The capability contract
remains the sole owner of rent, creation, work, provider, bounty, liquidity, and
service amounts. Hoard principal and future fees are not representable here.

## Shared observation and retirement

`SharedObservationStateV1` exists only when the authenticated source selects
`SourceAccessProfile::SharedObservationChild`. Its exact 288 bytes bind Market,
generation, source, provider release, window, capacity profile, pre-existing
RentCredit beneficiary, accepted evidence ID/sequence, and creation/retirement
times. Its PDA seed tuple is:

`dclutch/shared-obs/v1`, Market key, generation, source ID, window ID, bump.

The child accepts one bounded evidence set and then becomes replay-stable for
compatible policies. It can retire from open or accepted state and close into
RentCredit. Inline sources reject a supplied child; shared sources require the
matching accepted child. No universal archive exists, and retirement creates no
archive. Creation checks the profile's observed shared-child bound, while both
inline and shared acceptance check sample count and exact canonical evidence
bytes against the selected capacity profile.

## Exact widths and wires

Immutable preimages retain these widths: provider release 128, capacity profile
112, source 192, window 112, statistic 176, result mapping 144, resolution
policy 224, and recovery policy 528 bytes. New exact records are normalized
evidence 208, finite result map 512, source-resolution state 224, shared
observation state 288, and reopen link 128 bytes.

The closed instruction header is 16 bytes. Exact request widths are:

- CreateResolution: 160
- AcceptEvidence: 64
- FailNext / Exhaust / RetireResolution / RetireSharedObservation: 24
- CommitFailure: 32
- CreateSharedObservation: 160
- AcceptSharedObservation: 64

Every decoder rejects prefixes, trailing bytes, unknown actions, unknown
schemas, nonzero reserved bytes, zero required identities, and noncanonical
phase fields. Ordered SDK-free frames separately cover fresh/reopened creation,
inline/shared primary acceptance, inline/shared recovery acceptance with actual
capability funding, fail-next, primary/final-recovery exhaustion, failure
commit, state retirement, and shared-child create/accept/retire. Frame validators
require exact count, exact signer/writable/executable privileges, and no aliases.

Creation frames include the beneficiary's pre-existing permanent RentCredit.
The adapter is responsible for authenticating that PDA and its immutable
beneficiary before allocating refundable state; this remains the only intended
API seam until SBF routing lands.

## Closed releases

Schema/derivation/mapping preimages and their SHA-256 IDs are constants for:

- source-resolution state schema and PDA derivation;
- shared-observation schema and PDA derivation;
- finite-result mapping release; and
- reopen-link schema.

Tests derive every ID from its exact preimage, verify PDA seed components stay
within the chain-derived 32-byte bound, hostile-decode every new record family,
and exercise early failure, skipped attempt, stale generation, wrong funding,
replay, child-profile, cadence/order/duplicate-time, and adversarial integer
refusals.
