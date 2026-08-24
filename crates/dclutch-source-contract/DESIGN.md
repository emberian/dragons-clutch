# Source-contract V1

`dclutch-source-contract` is the `no_std`, `no_alloc`, SDK-free semantic owner
for provider-neutral Source resolution. It owns canonical bytes, exact integer
evaluation, lifecycle transitions, provider-extension selection, instruction
grammar, and ordered account roles. It does not hash content, derive Solana
addresses, inspect accounts, read Clock, invoke a provider, transfer principal,
settle a Market, or close an account. Those are SBF adapter obligations.

## One immutable Source authority

`SourceMaterialV1` is the only persisted immutable authority for a resolution.
Its exact 4,032-byte preimage embeds, by value and closed content ID:

- the Product occurrence resolution policy;
- capacity profile;
- primary source and provider release;
- window and statistic;
- result mapping and its finite mapping artifact; and
- optional ordered recovery policy plus each active recovery source and
  provider release.

The reusable pure subtypes remain public because they define and evaluate the
semantics, but no route accepts separately authenticated policy, source,
window, statistic, mapping, recovery, or provider records. Every route that
uses immutable Source semantics has one finalized-record authentication triple,
in this order: raw `SourceMaterialV1`, its derived vacant staging PDA, and the
Rent sysvar. The adapter must verify the raw record owner and exact length,
decode it, hash all 4,032 bytes to the instruction/state material ID, and derive
the vacant staging address under the authenticated record release. Static
clients and indexes are untrusted projections.

The closed Source-material derivation release commits use of the record
contract's content-addressed raw and staging domains with seed order
`domain, Source-material schema ID, exact material digest`. Source does not
invent a second material PDA or staging convention.

The material constructor and hostile decoder recheck every embedded link,
inactive zero slot, capacity bound, access profile, recovery order, mapping
release, and provider-extension release. V1 recognizes only its closed Pyth
provider extension; an unknown adapter release is refused rather than treated
as generic authenticated evidence.

## Provider-owned acceptance extension

An accept instruction consists of a fixed Source-owned prefix followed by the
exact payload owned by the provider release selected from authenticated
`SourceMaterialV1`. A caller never supplies a pre-authenticated normalized
evidence record. The provider adapter authenticates its accounts and payload in
the same atomic route and only then produces a `NormalizedProviderEvidenceV1`
value for the pure transition.

The first closed extension is the Pyth Receiver V2 post/reclaim boundary. Its
ordered ten-account extension is:

1. resolver signer and writable authority;
2. temporary update signer and writable account;
3. Receiver executable program;
4. Receiver ProgramData;
5. Receiver configuration;
6. encoded VAA message;
7. router executable program;
8. router ProgramData;
9. Receiver writable treasury; and
10. System Program.

The SBF adapter must authenticate the material-selected pinned provider
release; executable Program/ProgramData linkage and deployment slots; expected
program/data/config digests; router, configuration, treasury, and update
derivations; message ownership and exact payload; fully verified Receiver
update status; feed/config/time/value semantics; and update-account reclaim.
Only after all those checks may it call
`PythProviderAdapterObligationV1::normalize_authenticated_update`. The closed
extension ABI does not by itself authorize an arbitrary deployed Pyth release;
the production release catalog must recognize the material-selected release.

The normalized 208-byte output binds source ID, provider-release ID,
provider-evidence content ID, adapter-release ID, schedule ID and index,
observation/publication Unix times, and one signed `i128` value. These bytes are
provider-neutral transition input after the named adapter boundary, not a
provider attestation or separately persisted caller record.

Inline primary/recovery acceptance uses the Pyth extension atomically. A
resolution using the shared profile instead consumes an already accepted,
provider-authenticated shared child. Shared-child appends use the same Pyth
extension atomically.

## Exact statistics and Product mapping

Values are signed `i128` atoms. Scheduled averages retain exact `sum / count`
until the named `ExactRational`, `Floor`, or `Ceiling` boundary. There are no
floats or implicit divisions.

`OddScheduledMedian` is allocation-free and exact. Its required sample count is
odd and at least three, its window is `ScheduledInterval`, timestamps are
strictly ordered and unique, and samples occupy the exact committed equal
cadence from closed window start through end. Bounded O(n²) selection returns
the exact total-order median atom with denominator one and no rounding. A Pyth
TWAP is not part of this release; it requires a separately pinned and measured
provider adapter.

`FiniteResultMapV1` is the closed built-in Product mapping release. Its fixed
512-byte profile admits at most sixteen exhaustive ordered rational regions.
Strictly increasing rational boundaries and fixed selectors map the accepted
statistic to one Product cell without overflow-prone cross multiplication. The
same artifact owns the failure selector, so no accept or failure instruction
carries a caller-selected result. `SourceResolutionDecisionV1` exposes only the
derived primary/recovery/failure route, selector, outcome count, evidence ID,
and positive terminal sequence for the provider-neutral Market adapter.

## Persisted resolution lifecycle

`SourceResolutionStateV1` is exactly 224 bytes. Its PDA seed tuple is:

`dclutch/source-state/v1`, Market key, little-endian generation, bump.

It binds the Market, immutable generation, one Source-material ID, a
pre-existing RentCredit beneficiary, optional reopen-link ID, terminal evidence,
and terminal sequence. Its only phase graphs are:

`Primary -> Resolved -> Retired`

`Primary -> Recovery(0) -> ... -> Recovery(last) -> Resolved -> Retired`

or

`Primary/Recovery(last) -> Exhausted -> FailureCommitted -> Retired`.

Primary can enter only recovery attempt zero. An expired recovery leg can enter
only its immediate successor. Only the final leg can exhaust. Failure semantics
are refused before explicit exhaustion, and retirement is refused before a
resolved or failure-committed terminal decision. Acceptance computes the
statistic and Product mapping inside the transition. Every mutation checks the
persisted Market generation and uses the current Clock supplied by the adapter.

Every resolution-state creation is a direct Market child registration. Its wire
carries the expected Market `child_count`; the pure creation plan validates the
authenticated count and owns exactly one checked `before -> before + 1` delta.
Every terminal resolution retirement similarly carries the expected count and
owns exactly one checked `before -> before - 1` delta. The adapter must apply the
state and Market delta together atomically. Mismatch is a replay refusal and
does not mutate the candidate state.

The core Market identity has no predecessor field. `ReopenLinkV1` is a narrow
128-byte by-value immutable preimage binding Market, predecessor-state content
ID, predecessor terminal-evidence ID, previous generation, and exactly
`previous + 1`. The create wire embeds the link, while the reopen frame supplies
the readonly predecessor state so the adapter can authenticate the preimage
against terminal persisted authority. It does not change or compete with Market
authority.

## Ordered recovery and funding seam

Every `RecoveryAttemptV1` exposes exact indexed getters for source, provider
release, inclusive Unix deadline, and capability funding-allocation ID.
Attempts are finite, contiguous, and strictly deadline-ordered; inactive slots
must be zero.

The source contract never stores a funding amount. On `fail_next` and recovery
acceptance it compares the attempt's allocation ID with one adapter-authenticated
allocation ID. Recovery frames therefore include the capability manifest and
mutable `FundingState`, which are capability-owned witnesses rather than
parallel Source truth. The SBF adapter must authenticate the Market-selected
manifest and actual `FundingState`, validate their binding and conservation,
and observe presently held prepaid principal for that allocation. The
capability contract remains the sole owner of rent, creation, work, provider,
bounty, liquidity, and service amounts. Hoard principal and future fees are not
representable as recovery funding.

## Shared observation child

`SharedObservationStateV1` exists only for
`SourceAccessProfile::SharedObservationChild`. It is a direct Market child and
is exactly 3,616 bytes: a 288-byte header plus sixteen fixed 208-byte normalized
observation slots. It binds Market, generation, Source material, source,
provider release, window, pre-existing RentCredit beneficiary, exact expected
and observed counts, replay sequence, final evidence-set ID, and
creation/retirement times. Its PDA seed tuple is:

`dclutch/shared-obs/v1`, Market key, generation, source ID, window ID, bump.

Creation checks the material-selected shared profile and capacity, validates an
expected Market child count, and returns exactly one Market registration delta.
Each provider-authenticated append fills only the next schedule index, requires
a strictly increasing sequence, and may supply the complete evidence-set ID
only on the final append. Resolution revalidates exact stored observation bytes
and the complete ID before reuse. Retirement can close an open, collecting, or
accepted child into its RentCredit beneficiary and returns exactly one Market
retirement delta. There is no archive and no shared child for inline profiles.

## Exact wires and account frames

Immutable preimage widths are: provider release 144, capacity profile 112,
source 192, window 112, statistic 176, result mapping 144, resolution policy
240, recovery policy 528, normalized evidence 208, finite result map 512, Source
material 4,032, source-resolution state 224, shared observation state 3,616,
and reopen link 128 bytes.

The closed instruction header is 16 bytes. Exact fixed widths are:

- CreateResolution: 288 bytes;
- AcceptEvidence: 32-byte Source prefix plus selected provider payload;
- FailNext and Exhaust: 24 bytes;
- CommitFailure: 32 bytes;
- RetireResolution and RetireSharedObservation: 32 bytes;
- CreateSharedObservation: 208 bytes; and
- AcceptSharedObservation: 64-byte Source prefix plus nonempty selected provider
  payload.

An empty AcceptEvidence extension is structurally decodable because the same
action covers an accepted shared child; the SBF route must require a nonempty
provider payload for inline material and an empty payload plus accepted child
for shared material. AcceptSharedObservation always requires a nonempty provider
payload.

Ordered frame counts are: create fresh 8, create reopen 9, primary inline 16,
primary shared 7, recovery inline 18, recovery shared 9, fail-next 8, primary or
recovery exhaust 6, commit failure 5, resolution retire 4, shared create 9,
shared accept 15, and shared retire 4. Inline accept and shared append frames are
the fixed Source prefix followed by the exact ten-account Pyth extension.
Frame validators require exact count, exact signer/writable/executable
privileges, and no aliases.

Market is writable on every resolution/shared creation and retirement, and on
the acceptance/commit routes that produce a neutral Market settlement
decision. Creation frames include the beneficiary's pre-existing permanent
RentCredit. The adapter must authenticate that PDA and immutable beneficiary
before allocating refundable state; this is the narrow creation seam until the
SBF RentCredit route is integrated.

Every decoder rejects prefixes or forbidden trailing bytes, unknown actions,
unknown schemas/releases, nonzero reserved bytes, zero required identities,
invalid provider payload shape, and noncanonical phase fields.

## Closed releases and tests

Exact preimages and SHA-256 IDs are closed for:

- the single Source-material schema;
- its raw/staging record derivation;
- the Pyth provider-extension ABI;
- source-resolution state schema and PDA derivation;
- shared-observation schema and PDA derivation;
- finite-result mapping release; and
- reopen-link schema.

Tests derive every ID from its exact preimage, verify PDA seed components remain
within the 32-byte chain bound, hostile-decode records and wires, and exercise
early failure, skipped attempts, stale generations, wrong or absent funding,
replay, child-count mismatch, access-profile mismatch, ordered progressive
shared evidence, cadence/order/duplicate-time refusal, and adversarial signed
integer values.
