# Authenticated source construction V1

Status: **implemented as a fail-closed production ABI and exercised only by a
NON-PRODUCTION mock-source ELF** (2026-08-19).

This distinction is load-bearing. The default `clutch-sbf` artifact contains
zero registered provider/parser releases. Every source construction or
mutation intent reaches the closed registry and refuses with
`SourceReleaseUnavailable` (`0x0079`) before its first CPI or state write. The
feature `non-production-mock-source` changes the ELF and registers one fixed
laboratory account codec. Its bank result proves the program-owned lifecycle
and source-admission join; it proves nothing about Pyth, Switchboard, a DEX, or
any other live provider.

## Exact wire and account planes

All four intents name a canonical sealed Terms digest. No intent carries a
price, source cursor, predecessor, window, Realm, FeedId, deployment account,
or provider program. Those values come from Terms, SourceSpec, archive state,
the exact runtime accounts, and the Clock sysvar.

| intent | signed body | exact account plane |
| --- | ---: | --- |
| `InitSourceSpec` | 290 bytes | 9: payer(w,s), SourceSpec target(w), Feed target(w), Terms(r), provider program(r,x), deployment evidence(r), source account(r), System(r,x), Rent(r) |
| `InitSourceArchive` | 34 bytes | 7: payer(w,s), SourceSpec(r), Feed(r), Terms(r), archive target(w), System(r,x), Rent(r) |
| `AppendSourceArchive` | 34 bytes | 8: SourceSpec(r), Feed(r), Terms(r), archive(w), provider program(r,x), deployment evidence(r), source account(r), Clock(r) |
| `SealSourceArchive` | 34 bytes | 8: SourceSpec(r), Feed(w), Terms(r), archive(w), provider program(r,x), deployment evidence(r), source account(r), Clock(r) |

Exact account counts and mutability are consensus checks. Legacy and trailing
planes refuse. The 256-byte SourceSpec body is inside the signed request; it is
decoded by the typed source codec and its digest must equal `Terms.feed`.
Consequently no arbitrary evidence account becomes a second owner of the
immutable specification.

## Construction and mutation

`InitSourceSpec` performs every metadata, Terms, PDA, deployment-release,
parser-release, provider-account and target-availability check against a local
292-byte image before moving lamports. It then uses PDA-signed System
`Allocate` + `Assign` to create the canonical SourceSpec and Feed atomically.
The Feed begins at the Terms window start with a domain-separated empty-state
anchor. Both targets accept System-owned, zero-data prefunding: the payer funds
only the rent shortfall and excess SOL remains an unowned donation.

`InitSourceArchive` authenticates the SourceSpec and unchanged initial Feed,
derives the exact `WindowDomain` from Terms, constructs a complete local
2,560-byte image through the registered deployment type, then creates the
canonical `(feed, window)` PDA by the same prefund-safe sequence.

`AppendSourceArchive` authenticates:

- the canonical Terms/SourceSpec/Feed/archive addresses and owners;
- the compile-time provider deployment and parser releases;
- provider program executability and exact deployment/source account keys;
- the real Clock sysvar, freshness, future skew and confidence policy;
- `request.sequence == archive.record_count`;
- the unique next bucket, strictly increasing provider sequence, and monotone
  publish slot/time.

Only the parser's admitted conservative interval is archived. A retry, skipped
sequence, wrong bucket, stale record, provider substitution, deployment
substitution, or parser-release substitution refuses before a page byte
changes.

`SealSourceArchive` admits no caller cursor. V1 supports exactly
`maturity == end + 1`: a registered parser must admit the unique next source
record at bucket `end`, and only its resulting cursor may seal the page. Seal
then re-verifies the immutable recorded receipt and atomically advances Feed to
that cursor, page count one, and the page commitment. The Resolve path already
consumes this exact canonical receipt.

## Real-bank evidence (mock ELF only)

`programs/clutch-sbf/svm-tests/tests/source_ingest.rs` drives the real SBF
program and real System program. Terms are installed as a canonical
program-owned prerequisite. SourceSpec, Feed and SourceArchive have no hidden
program-owned prestate. A wallet creates them, appends two provider-admitted
records, and seals with a third maturity witness.

The recorded run used the runtime Rent sysvar and reported:

| operation/account | bytes | rent-exempt minimum / retained prefund | CU |
| --- | ---: | ---: | ---: |
| `InitSourceSpec` (creates Spec + Feed) | 292 + 124 | 2,923,200 + 1,753,920 lamports | 448,896 |
| `InitSourceArchive` | 2,560 | 18,708,480 lamports | 553,723 |
| append record 0 | no growth | — | 656,962 |
| append record 1 | no growth | — | 656,346 |
| seal + Feed update | no growth | — | 866,718 |

The Feed retained a public over-rent prefund of 1,758,241 lamports and the
archive retained a public over-rent prefund of 18,716,134 lamports. The
SourceSpec exercised the one-lamport recovery branch. ProgramTest cannot leave
a newly credited public zero-data account below rent exemption, so that one
lamport was injected as an otherwise-valid System account; it is hostile-bank
coverage, not a claim that the public transfer itself succeeded. Replay,
deployment-account substitution, source-account substitution, and a wrong
maturity bucket all refused with byte-for-byte archive/Feed rollback.

Both default and mock-feature `cargo-build-sbf` builds completed. They still
emit the repository's pre-existing oversized-stack diagnostics in
`clutch-solana-reference` and `clutch-solana-layout`; no new diagnostic names a
source-ingest function. Completion is therefore runtime evidence, not a clean
deploy gate. The default artifact was rebuilt after the mock run so the shared
`target/deploy` output is not accidentally left as the mock ELF.

## Exact STOPs

1. **No production provider ABI.** The default registry is empty. A reviewed
   live deployment authenticator and immutable/finalized bucket parser must be
   implemented, audited and registered before the default artifact can ingest.
2. **Only complete, generation-zero, at-most-32-record windows.** Bounded gaps,
   repair-generation transitions and multi-page windows have no admission
   rule in this lifecycle.
3. **Only a one-bucket maturity witness.** Longer maturity horizons need a
   canonical persisted source head or intermediate archive lineage; a caller
   cursor is not an acceptable substitute.
4. **One Realm binding per Feed PDA.** The existing global `feed(feed_id)` seed
   carries one stored Realm. Cross-Realm reuse needs a deliberate seed/layout
   decision, not first-writer convention.
5. **No archive/Feed cleanup, rent release or successor-page instruction.** The
   immutable receipt is retained indefinitely. Shared-feed liveness and reserve
   models must keep terminal release as a STOP.
6. **Provider availability is external.** Permissionless submission removes a
   Clutch operator, not the source provider or transaction inclusion
   dependency.
7. **Legacy `FeedAdvance` still exists.** It cannot substitute for the
   canonical sealed source receipt Resolve authenticates, but removing or
   formally deprecating that unauthenticated summary path is separate work.

The strongest honest claim is therefore: the protocol has a permissionless,
canonical, prefund-safe SourceSpec/Feed/archive lifecycle whose source choice is
closed at compile time, and the full path is real-SBF proven for a deliberately
non-production registered mock. The shipped default artifact remains safely
inert until an actual provider release earns registration.
