# Source-contract V1

`dclutch-source-contract` is a no-std, no-alloc, SDK-free preimage contract.
It intentionally has no Solana account, RPC, oracle SDK, hash, signature,
transaction, archive, or transport implementation.

## Semantic ownership

`ProviderReleaseV1` names the provider-family and the reviewed adapter/parser
release. It is not evidence that a particular wire message, account, or URL is
true. `SourceSpecV1` owns the semantic observation domain, exact atom unit,
selected provider release, and access profile. `ResolutionPolicyV1` is the one
persisted bridge from a Product occurrence to source, window, statistic, and
Product result mapping. The result mapping is Product truth and has no provider
transport fields.

The compositing adapter must authenticate every content ID against its selected
hash policy and validate the explicit `validate_*` linkage methods. Static
clients and indexes remain untrusted projections.

## Exact values and time

All observation values are signed `i128` atoms. The unit is a nonzero content
identity in `SourceSpecV1`; it is never a float decimal. A scheduled average is
an exact `sum / count` rational. `ExactRational`, `Floor`, and `Ceiling` name
the sole rounding boundary between the statistic and its result mapping. The
adapter must verify the finite schedule committed by `WindowSpecV1::schedule_id`;
the kernel checks count and closed time bounds. No implicit rounding occurs.

`Terminal` windows require equal start/end times. `ScheduledInterval` requires a
strictly increasing interval. Their `max_age_seconds` and
`max_future_skew_seconds` are immutable source acceptance constraints for the
adapter, not wall-clock claims by the static contract.

## Profiles, children, and archives

`SourceAccessProfile::PythTerminalOneTransaction` supports the cheap terminal
Pyth-style path: the adapter can normalize and resolve a single terminal sample
in one transaction. `SharedObservationChild` is an explicit alternative for a
bounded reusable observation child. There is no archive field or archive
requirement in the universal records; an adapter that needs archival data must
provide it as a separately selected child/configuration and stay within the
source capacity profile.

`SourceCapacityProfileV1` bounds samples, recovery attempts, observation bytes,
and shared children. Its envelope is `Measured` (the basis ID names evidence) or
`Provisional` (the basis ID names a lifting plan). To lift a provisional bound,
create a new profile content preimage and new policies; old policies retain their
old bounded semantics.

## Funding and recovery

Each recovery attempt has a nonzero source, provider release, deadline, and
prepaid funding reference. The reference is either a content ID for an
authenticated capability-funding quote or the exact seven present-principal
compartments used by dclutch capability funding: rent, creation, work, provider,
bounty, liquidity, and service. The representation contains neither Hoard
principal nor future fee revenue.

Recovery attempt deadlines are strictly increasing. `RecoveryState` reaches
`Exhausted` only after the final committed attempt; terminal failure is refused
before that point. A policy with no recovery is represented by the all-zero
optional recovery ID in `ResolutionPolicyV1`, not a universal archive or
unbounded workflow graph.
