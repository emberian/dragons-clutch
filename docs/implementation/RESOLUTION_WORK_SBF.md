# ResolutionWork SBF cut (PROPOSED, unreachable)

Status: semantic kernel and isolated codec only. No layout export, request
router, entrypoint, PDA registry, numeric refusal allocation, release cost
schedule, or live handler currently reaches these bytes. The proposed tags and
account lists below are not ABI until the shared integration commit and its
real-SBF evidence land.

The purpose of this cut is to replace the monolithic occupation fold with a
bounded, restartable state machine without creating a second source of payout
truth:

```text
Begin -> active program-owned Work -> Fold* -> Finalize -> sole Resolution v4
                                      \-----> permitted Abort -> no Resolution
```

Work is public program-owned state, not confidential state. Its accumulator is
an internal exact accumulator. No caller supplies records, Merkle material,
proofs, points, masses, weights, or a payout vector.

## Immutable authority and source sealing

Begin authenticates the complete Market/Terms/SourceSpec relation and the one
canonical SourceArchive PDA. It calls
`verify_recorded_sealed_archive_view`, which checks the archive key, program
owner, executable bit, exact 2,560-byte length, source release, window,
lineage, sealed flag, zero padding, and recomputed full page commitment. Work
freezes all of these identities, the exact bucket span, basis artifact and
digest, grid and duration, semantic versions, finalizer, expiry, and cost
schedule.

Fold receives the same SourceSpec and archive accounts. It reconstructs the
same verified lifetime-bound view, compares the exact archive PDA and full
stored commitment with Work, then reads only cursor-indexed records through
`VerifiedSealedArchiveViewV1::archived_observation`. The 107-byte Fold body is
only:

```text
tag/version, work_id[32], archive_account[32], archive_commitment[32],
expected_cursor:u64, record_count:u8
```

There is no proof or record-data tail.

The current archive has no authenticated gap record. Therefore this release
accepts only exact point records (`low == high`) and refuses genuine intervals;
it does not silently skip them. The accumulator already preserves explicit
gaps, so a future archive version may map a typed gap to `append_missing`
without changing Work semantics.

There is no post-seal mutator. The only archive mutation entrypoints are
initialization, authenticated append, and seal. Their common release verifier
is invoked with `require_sealed = false` for append/seal and returns
`AlreadySealed` when the sealed flag is already set. Resolution-time reads use
`require_sealed = true`. The instruction router exposes no unseal, rewrite,
truncate, or repair-in-place operation. A future archive revision must preserve
this property or it cannot be consumed by ResolutionWork.

## Isolated wire shape

The isolated codec commit `a6da401` proposes:

- Work account tag 22, version 1, exactly 1,296 bytes;
- common intent envelope version 3;
- Begin/Fold/Finalize/Abort tags 32/33/34/35;
- exact instruction lengths 83/107/74/74 bytes;
- at most four records per Fold and a 32-record capacity in the current archive.

The isolation test checked that account tag 22 did not collide with committed
account tags 1–21 or artifact-stage tag 33, and that intent tags 32–35 followed
the committed 1–31 range. Integration must repeat this check against the
then-current registry; the isolated allocation has no priority over intervening
work.

The fixed Work image contains:

- Work, payer, reserve, nonce, Market, Terms, Resolution target, and program
  identities;
- exact archive PDA, full commitment, source-spec digest, archive-domain
  digest, generation, grid, duration, start/end, and record count;
- the canonical 304-byte BasisSpec V1 artifact plus its domain-separated
  digest;
- evaluator, summary, and Resolution-v4 versions and the finalization mode;
- open, expiry, last-progress, completion, and exact next-cursor fields;
- sample/coverage/fold counts and sixteen checked `u128` masses;
- the complete cost schedule and digest;
- deposit, locked rent, remaining prepaid funds, charges, rewards, bumps, and
  canonical zero padding.

Only ACTIVE is encodable. Finalize and Abort close Work. A terminal Work image
cannot survive as an alternate resolution or redemption authority.

## Safe resume and atomic Fold

`Summary::from_canonical_parts` validates the complete restored domain, range,
sample/coverage counts, inactive padding, and exact partition mass.
`SequentialSummaryBuilder::resume` revalidates that Summary before copying it.
Every Fold operates on a by-value Work copy and a local builder. It checks the
chunk end, reads and appends every record, quotes and debits funding, constructs
the complete next image, and validates that image before the account plane may
write one byte or transfer one lamport. A wrong cursor, duplicate, skip,
reorder, interval, overflow, underfunding, or archive substitution leaves Work
unchanged.

The accumulator tests compare persisted resume with uninterrupted sequential
folding and every parenthesization over degrees zero through three. They also
kill malformed empty state, range/count disagreement, wrong total mass,
inactive padding, mass overflow, nonadjacent resume, maximum-bucket overflow,
and both finalization-mode drift cases.

## Active-work lock and terminal identity

V1 should use one deterministic Work PDA per Market, not a nonce-derived family
of simultaneous accounts:

```text
Work    = PDA("resolution-work-v1", market)
Reserve = PDA("resolution-reserve-v1", market)
```

The Work account's presence is the Market-local active-work lock. The nonce is
inside the recomputed Work identity and separates retries, but cannot create a
second concurrent address. Begin requires an active Market, an unresolved
canonical Resolution v4, and an absent Work target. Finalize writes that one
Resolution, transitions the same Market/kernel/supply plane as monolithic v4,
then closes Work and Reserve. Abort writes no Resolution and closes the same
lock. Once the Market is resolved, Begin refuses even though the Work PDA has
closed. Redemption continues to read only immutable Terms and Resolution;
neither Work nor Reserve is accepted on the redemption account list.

This deterministic-address lock is equivalent to an embedded Market
`active_work` field without revising the 728-byte Market layout. If integration
instead adds such a field, it must prove why the redundant field and Work
presence cannot disagree. V1 should choose one owner, not persist both.

Terminal identities are exact:

- all rewards name the authenticated caller of the successful transition;
- every refund names the payer frozen at Begin, never a caller-selected
  destination;
- the Reserve is the one frozen PDA and can pay only the quoted charge/reward
  and payer refund;
- the Resolution target is the one canonical Market Resolution PDA frozen in
  Work;
- Finalize binds the exact end cursor and archive commitment and may execute
  after Fold expiry;
- expired incomplete Abort is permissionless only strictly after expiry;
- unstarted Abort requires the payer;
- complete Abort is permitted only for no coverage, explicit gaps, or an
  inexact exact-only average; a successfully finalizable Work cannot abort.

## Proposed account envelopes

These are semantic role counts, not measured transaction admissions.

Begin, 11 fixed roles:

1. payer (signer, writable, sole refund identity);
2. active Market (read-only);
3. immutable Terms (read-only);
4. canonical unresolved occupation Resolution v4 (read-only);
5. immutable SourceSpec (read-only);
6. exact sealed SourceArchive (read-only);
7. absent deterministic Work target (writable);
8. absent deterministic zero-data Reserve target (writable);
9. System program;
10. Rent sysvar;
11. Clock sysvar.

Fold, 9 fixed roles:

1. worker (signer, writable reward destination);
2. active Market (read-only);
3. immutable Terms (read-only);
4. immutable SourceSpec (read-only);
5. exact sealed SourceArchive (read-only);
6. Work (writable);
7. Reserve (writable);
8. System program;
9. Clock sysvar.

Finalize starts from the current monolithic occupation Resolve plane: actor,
Market, Hoard, kernel aggregate, supply ledger, Terms, Resolution, Feed,
SourceSpec, SourceArchive, and `outcome_count` mint accounts. It additionally
needs payer, Work, Reserve, System, and Clock, for a proposed total of
`15 + outcome_count`. Factoring the existing monolithic transition is required;
Finalize must not create a second transcription of Market/kernel/supply and
mint-synchronization semantics.

Abort, 8 fixed roles: aborter signer, frozen payer, active Market, immutable
Terms, Work, Reserve, System, and Clock. It has no Resolution write role.

If nonzero protocol charges are selected, every charged transition also needs
one authenticated fee sink. Until such a sink and policy exist, the release
schedule must set every charge to exactly zero; a handler may not burn, retain,
or redirect an unnamed charge. Rewards may still be nonzero and are paid only
from Reserve.

## Rent and prepaid conservation

At Begin, Rent determines the exact minimum for 1,296 Work bytes. The payer
funds that rent and transfers the remaining declared deposit into the zero-data
Reserve. No later transition accepts a funding account or top-up.

While active:

```text
deposit = rent_locked + prepaid_remaining + charges_paid + rewards_paid
```

The minimum deposit reserves the most expensive partition—every archive record
folded alone—and the more expensive terminal path. Every multiplication and sum
is checked. Fold/Finalize/Abort quote from the frozen schedule, debit a staged
ledger, and validate complete conservation before transfers.

Predictable PDAs may be prefunded. Integration must normalize unsolicited
Work/Reserve lamports through a PDA-signed System transfer before applying the
payer's exact funding, or explicitly account for the donation outside the
declared ledger. It must not let one lamport squat the lock, attribute a donor
as payer, or silently include an unknown donation in a quoted refund.

At a terminal transition, the payer receives exactly the remaining prepaid
budget plus released Work rent. Work data is zeroed and Work/Reserve lamports
become zero in the same transaction. A repeated Fold, Finalize, or Abort then
finds no active Work and refuses. Solana transaction rollback is necessary but
not sufficient evidence: the real-bank suite must inject a failure after all
preflight work but before terminal completion and compare every account byte
and lamport with prestate.

## Finalize equivalence and late rollback

`finalize_state` restores the exact Summary, requires the exact end cursor,
applies the frozen finalizer, and stages the canonical existing
`OccupationResolutionAccount` v4. The live account plane must then reuse a
factored form of current monolithic occupation Resolve for:

- Market active-to-resolved transition;
- kernel payout-vector resolution;
- supply-ledger and observed outcome-mint closure;
- canonical Resolution-v4 validation and one-time write.

Only after the complete poststate and all terminal transfer amounts validate
may the handler perform writes/transfers. A late failure—including bad output
mint state, wrong Resolution bump, insufficient Reserve, refund overflow, or
borrow failure—must roll back Market, kernel, supply, Resolution, Work, Reserve,
and every destination balance. Finalize is deliberately allowed after expiry;
expiry stops new Fold work but cannot strand a complete valid result.

## Shared BasisDomain cache analysis

A cache keyed by `(basis artifact digest, archive-domain digest)` could retain a
validated BasisSpec/grid/domain summary for many Markets that share immutable
Terms and one archive. It may reduce repeated basis/domain validation, but it
must not cache a payout vector or cursor and must be immutable/content-addressed.
Each Work still owns its counts, masses, finalizer, cost ledger, and exact
archive commitment. A cache hit must compare the full key and semantic versions
and reconstruct the same `BasisDomain`; a digest-only off-chain vector is never
accepted.

Source append must not fan out to Markets or Work accounts. Append touches only
the one SourceArchive/Feed lineage. Markets consume a sealed commitment lazily
at Begin. This keeps source cost independent of subscriber count and prevents
one provider update from acquiring an unbounded write set.

## Required evidence before promotion

No CU, rent, account-count admission, ELF identity, or liveness claim exists
yet. Promotion requires one clean integration commit and a real `cargo
build-sbf` artifact, then ProgramTest measurements for:

- Begin at the exact rent minimum and one lamport below the minimum;
- Fold sizes 1 through 4 at first/middle/final positions, including retries;
- Finalize before end, at end, and late after expiry;
- payer-only unstarted Abort, permissionless expired Abort, every complete
  invalid Abort reason, and valid-complete Abort refusal;
- alternate same-domain archive account and commitment substitution;
- wrong Work/archive/cursor/count/order, duplicate/replayed Fold, mutable Terms
  or basis version, genuine interval, overflow, and reserve underfunding;
- terminal replay after account close;
- byte-and-lamport equality after injected late Finalize/Abort failure;
- payout/vector/Market/kernel/supply equivalence with monolithic v4 for every
  supported degree, finalizer, chunk partition, and measured archive span.

Record the exact account count, transaction message size, allocated bytes,
runtime rent minimum, consumed CU, retry CU, build commit, ELF SHA-256, feature
set, toolchain, validator/runtime versions, and complete logs. Preserve the
existing nonmonotonic compute warning: optimized monolithic v4 initial Resolve
did not clear the 1,120,000-CU 25%-headroom gate for any measured span 1–3 or
degree 1–3; the best measured initial attempt was 1,236,364 CU, while span-3
retries measured 1,086,756–1,108,857 CU. Do not extrapolate from either series.
