# Resumable occupation resolution V1

Status: **executable model oracle plus routed live SBF V1; the exact measured
route is liveness-profile admitted**

Executable witness: `research/resolution-work-v1`

Live instruction ABI: **implemented at intent tags 32 through 35**

Live account layout: **implemented at account tag 22, exactly 1,296 bytes**

SBF compute/rent evidence: **measured; see `RESOLUTION_WORK_SBF.md`**

## Decision

A long native degree-0-through-3 occupation reduction should not be one
all-or-nothing instruction and should not accept a caller-computed payout
vector. The implemented architecture is an immutable, prepaid, cursor-bound state
machine:

```text
authenticated immutable inputs
           |
           v
  Begin -> Active(cursor = start, program-owned exact accumulator)
             |
             | Fold 1..MAX_FOLD_RECORDS exact next records
             | (read only from the same sealed archive PDA)
             v
          Active(cursor = end)
             |
             +-- Finalize -> one canonical Resolution V4 -> closed/refunded
             |
             +-- deterministic gap/no-coverage/inexact refusal
                            -> named Abort -> closed/refunded
```

This decomposition moves work across transactions without moving semantic
authority offchain. The payout vector is still produced by the native basis
evaluator and occupation accumulator. A caller may choose a small or large
bounded chunk, but cannot choose the archive, basis, interval, record order,
gap meaning, final rounding rule, or payout vector.

The model is safe Rust, `no_std`, allocation-free outside tests, float-free,
fixed-width, and unpublished. It directly depends on the current
`clutch-bspline` and `clutch-bspline-accumulator` semantics. It is deliberately
kept as a pure oracle beside the live SBF adapter: the model owns state-machine
and accumulator semantics, while the adapter owns Solana accounts, rent,
donations, CPIs, and transaction rollback.

The trigger is measured, not speculative. On optimized monolithic commit
`87d2dbd`, no initial occupation Resolve among measured spans 1 through 3 and
degrees 1 through 3 clears the frozen 25%-headroom threshold of 1,120,000 CU;
the best initial result is 1,236,364 CU. Span-3 retry measurements of
1,086,756 through 1,108,857 CU do clear that threshold. These results are
nonmonotonic across paths and inputs: they do not justify interpolation,
extrapolation to unmeasured spans, or a claim that every retry passes.

## Begin: validate once, freeze once, fund once

`ResolutionWorkV1::begin` validates and freezes:

- work semantic version;
- exact market identity;
- complete immutable Terms digest;
- unique resolution target identity;
- the existing 304-byte revision-one native BasisSpec artifact and its existing
  domain-separated digest, recomputed over every field including knot padding,
  edge policy, degree, denominator, and domain;
- native evaluator and occupation-summary versions;
- source-specification digest;
- archive-domain digest and generation;
- exact program owner, sealed SourceArchive PDA, 2,560-byte account length,
  terminal seal state, and stored full archive commitment;
- grid identity and nonzero equal bucket duration;
- exact half-open bucket interval and record count;
- `ExactOnly` or `LargestRemainderV1` finalization;
- the complete cost schedule digest;
- payer, segregated prepaid-reserve identity, unique work nonce, opening slot,
  and tightly bounded Fold expiry; and
- a deposit covering locked rent plus the complete worst-case work lifecycle.

The archive span must be nonempty and satisfy exactly:

```text
record_count = end_bucket_exclusive - start_bucket
```

The current SourceArchive V1 has an explicit 32-record capacity and a fixed
2,560-byte account. Begin therefore refuses a span outside `1..=32`; there is
no silent truncation. Both the model and live adapter bound each Fold to four
records. The live compile-time lifetime is `8..=4096` slots from Begin.

Counts are bounded by that exact admitted span. Each accepted bucket contributes
at most one `u64` denominator of mass, so even the conservative product
`u64::MAX * u64::MAX` is strictly below `u128::MAX`; component additions and
the cross-component unity check nevertheless remain checked operations in the
current accumulator. The edge test processes a final bucket of
`u64::MAX - 1` to the largest representable exclusive cursor without wrapping.

The basis digest is recomputed by a fixed-array, no-allocation mirror of the
already-owned host artifact codec; a golden digest pins byte-for-byte parity.
This model does not introduce a second basis identity. Treating a
caller-supplied opaque digest as sufficient would permit “same digest,
different basis bytes” at an adapter boundary. The work also retains the exact
validated `BasisDomain`, not only its digest.

### Begin trust boundary

The executable model checks internal canonical bytes and relationships. The
live adapter additionally loads the exact Market, Terms, Resolution,
SourceSpec, and SourceArchive accounts; verifies owners, lengths, canonical
PDAs and stored bumps; authenticates the Terms digest and full sealed archive
commitment; and freezes those facts into Work. The real-bank campaign covers a
malformed stored SourceSpec bump and same-domain alternate-archive
substitution. The model alone must not be cited as proof of this adapter layer.

## Fold: authenticate before advancing

`FoldRequestV1` contains only the expected work identity, archive PDA, full
archive commitment, cursor, and a count in `1..=4`. It contains **no archive
record bytes, point vector, mass vector, or auxiliary authentication payload**. Fold receives the
same runtime archive account, rechecks exact key, program owner, 2,560-byte
length, non-executable status, seal state, source/domain/grid/generation,
window/count, and stored commitment against Work, and then reads the next
records directly from account-owned bytes.

Only this sequence is accepted:

```text
request.expected_cursor = work.next_bucket
archive.record[work.next_bucket - archive.start].bucket = work.next_bucket
request.expected_cursor + request.count <= frozen_archive.end
```

This makes skip, duplicate cursor, overlap, replay, and cursor substitution
ordinary deterministic refusals. A caller has no surface on which to reorder
or replace record bytes. Header identities additionally prevent a different
generation, source, archive domain, PDA, owner, or exact commitment from
entering the work state.

Begin recomputes/authenticates the full current archive once. Fold deliberately
does not rehash all 2,560 bytes: it relies on the program-owned terminal seal.
The executable archive model has private fields; `append` is its only record
mutator and atomically refuses after `seal`; there is no unseal, replacement,
or post-seal mutation API. The live write-surface audit finds only init,
append, and seal; append and seal refuse an already sealed archive, and the
router exposes no unseal or record-rewrite path.

The fold clones the internal validated accumulator and funding ledger, verifies
and evaluates the complete chunk, computes checked costs, and commits all
fields only after every check succeeds. Authenticated `Missing` records advance
the cursor and add a gap; they never contribute payout mass. An authenticated
`Accepted(point)` can still be refused by the frozen basis edge policy. Either
failure leaves cursor, summary, counters, and funds byte-for-byte unchanged.
This exact no-caller-record rule is the live ABI.

## Finalize: exact end, one canonical write

Finalize first requires:

```text
work.next_bucket = archive.end_bucket_exclusive
summary.sample_count = archive.record_count
```

It then invokes the existing accumulator's exact finalization mode. It does
not accept weights from the caller. Successful output binds:

- Resolution version 4;
- work, market, Terms, and unique resolution-target identities;
- basis, SourceSpec, and exact sealed archive digests;
- complete interval;
- accepted and gap counts;
- exact finalization mode; and
- active length, denominator, and the complete fixed-width weight vector.

The resolution commitment hashes all of those fields. Only after constructing
that output does the transition pay the finalizer from prepaid funds, mark the
work finalized, retain the single canonical output, release rent, and refund
the unused budget to the payer. A repeated Finalize, Fold, or Abort
returns `AlreadyTerminal` and cannot create a second vector or transfer.

The model stages the complete Work, funding, and Resolution post-state,
validates it, and only then commits. A late failure—including time rollback,
bad finalizer, incomplete cursor, gap, exact-average remainder, funding error,
or target mismatch—must leave Work, reserve, Resolution target, and Market
unchanged. In the live transition, canonical V4 Resolution is the sole
post-finalization payout authority: Finalize must atomically write that unique
target and close/refund Work. Redemption must never read a Work account, and a
closed Work PDA must not be recreatable while Resolution exists.

`ExactOnly` remains exact: an average with any nonzero component remainder is
not silently rounded. `LargestRemainderV1` remains the existing deterministic
lowest-index tie rule owned by the accumulator.

## Gaps, refusal, and abort

A source gap is protocol information, not permission to silently shorten the
denominator. V1 therefore retains the accumulator's behavior:

- zero accepted records: `NoCoverage`;
- any explicit gap: `IncompleteCoverage`; and
- nonintegral exact-only average: `InexactAverage`.

These conditions cannot write a payout vector. The model permits a terminal
named Abort only in these narrow cases:

1. no Fold has occurred and the payer authorizes cancellation, or the item is
   already expired;
2. work remains incomplete strictly after its inclusive Fold expiry; or
3. the exact end has been reached and finalization deterministically returns
   one of the three refusals above.

Partial-progress abort before expiry is forbidden. A Fold at a slot earlier
than the last progress slot or strictly after expiry refuses atomically.
Completion records the exact slot; a valid vector completed by expiry may
Finalize later and may not be aborted merely because wall time passed. This
prevents an expiry caller from destroying already-complete resolution work.
An incomplete expired item may close; prior worker rewards remain paid and only
the unspent reserve plus rent returns to the original payer. A completed
refusal may close and return value
without pretending that the underlying market resolved. A live design still
needs to specify the market-level escalation or successor-source policy for
such a refusal; this model does not invent one.

## Exact abstract funding model

All money-like values in the crate are abstract cost units. They are not
claimed to be current lamports, rent, priority fees, compute units, or account
sizes. Nevertheless the accounting equations are exact and checked.

For `N = record_count`, the required Begin deposit is:

```text
single_fold_max = fold_base_charge
                + fold_per_record_charge
                + fold_base_reward
                + fold_per_record_reward

terminal_max = max(finalize_charge + finalize_reward,
                   abort_charge + abort_reward)

minimum_deposit = rent_reserve
                + begin_charge
                + N * single_fold_max
                + terminal_max
```

The `N * single_fold_max` term assumes the most expensive allowed partition:
every record folded alone. Batching may save frozen per-call charge and reward,
but is never necessary for economic completion. Every multiplication and sum
is checked; an unrepresentable quote refuses Begin.

The ledger maintains, while active:

```text
deposit = rent_locked
        + prepaid_remaining
        + charges_paid
        + rewards_paid
```

and after Finalize or permitted Abort:

```text
deposit = charges_paid + rewards_paid + payer_refund
rent_locked = prepaid_remaining = 0
```

No Fold, Finalize, or Abort takes an external funding parameter. Its complete
charge and caller reward are debited from the frozen prepaid ledger. This
prevents the state machine from assuming future fees, Hoard principal, market
collateral, an operator, or a benevolent caller.

`CostScheduleV1.work_state_bytes` and `rent_reserve` remain explicit abstract
model inputs. The live adapter instantiates them with a compile-time policy:
1,296-byte Work rent plus zero-data Reserve rent, 1,160,000 lamports per Fold
call, 1,510,000 for Finalize, 860,000 for Abort, and zero charges. Under
`Rent::default()` its exact rent principal is 10,801,920 lamports and
`minimum_deposit(n) = 12,311,920 + n * 1,160,000`; the maximum 32-record
deposit is 49,431,920. Unsolicited pre- or post-Begin surplus is tracked
separately as monotone donation, can never satisfy a shortfall or subsidize a
reward, and goes only to the canonical SDK incinerator at terminal close. Both
Work and Reserve close; exactly the locked principal and unused prepaid budget
return to the immutable payer.

### Live semantic ABI

The model and live adapter share this instruction-data authority boundary:

```text
BeginV1:
  work_nonce[32], finalization_mode:u8, expires_slot:u64,
  declared_deposit:u64, cost_schedule_digest[32]

FoldV1:
  work_commitment[32], archive_account[32], archive_commitment[32],
  expected_cursor:u64, record_count:u8

FinalizeV1:
  work_commitment[32], expected_cursor:u64, expected_archive_commitment[32]

AbortV1:
  work_commitment[32], expected_cursor:u64, expected_archive_commitment[32]
```

No record, observation, mass, weight, archive receipt, or payout vector is
instruction data. Account-derived identities are repeated only as optimistic
concurrency/replay guards and must equal the loaded accounts. Clock comes from
the Clock sysvar, deposit comes from the actual reserve transfer, and final
costs come from the stored schedule; caller duplicates never override them.
The common envelope uses account tag 22 and intent tags 32 through 35. Exact
body lengths are 83, 107, 74, and 74 bytes respectively. Deterministic accounts
are `PDA("resolution-work-v1", market)` and
`PDA("resolution-reserve-v1", market, Work)`. Hostile codec and collision tests
freeze tags, lengths, reserved bytes, status, cost digest, TTL, and funding
invariants.

The Work account must persist exactly one semantic owner for:

```text
work/version/status; payer; prepaid reserve; nonce; open/expiry/progress slots;
market/Terms/resolution-target; program/archive PDA/owner/full commitment;
SourceSpec/archive domain/generation/grid/duration/window/count;
BasisSpec artifact bytes+digest and evaluator/summary versions;
finalization mode; cursor/fold count/completion slot;
sample/coverage counts and 16 checked u128 masses;
cost schedule+digest; donation/rent/remaining/charges/rewards; bump/reserved bytes.
```

Work is a transparent program-owned account, not confidential state. On
Finalize, the adapter stages a canonical V4 Resolution write and all lamport
transfers, validates the complete post-state, writes Resolution, clears the
Market's active-work lock, and closes Work/reserve atomically. On Abort it
clears only the active-work lock and closes/refunds; it cannot write Resolution.

### Live account-role envelope

The exact ordered roles are:

| Transition | Read-only authority | Mutable semantic owner | Transfer roles |
|---|---|---|---|
| Begin | 11: payer, Market, Terms, Resolution, SourceSpec, SourceArchive, Work, Reserve, System, Rent, Clock |
| Fold | 8: worker, Market, Terms, SourceSpec, SourceArchive, Work, Reserve, Clock |
| Finalize | `15 + outcome_count`: monolithic prefix 10 plus outcome mints, payer, Work, Reserve, canonical incinerator, Clock |
| Abort | 8: caller, payer, Market, Terms, Work, Reserve, canonical incinerator, Clock |

Every role has exact signer, writable, owner, length, PDA, and alias checks.
Terminal roles contain no redundant System program. The live adapter uses the
one existing sealed archive account directly and no auxiliary archive-data
account.

## Differential and adversarial evidence

The isolated test suite currently covers:

- every native degree 0 through 3;
- every bounded composition of a five-record span versus one monolithic
  current accumulator (all groupings permitted by the per-call bound);
- aggregate commutativity under reversed values while preserving distinct
  archive and resolution commitments;
- alternate archive substitution with the same archive domain;
- wrong archive, source, generation, account, owner, cursor, count bound, end,
  duplicate cursor, and replay;
- post-seal append/reseal refusal and immutable account-owned record reads;
- mutable basis bytes behind the original digest;
- evaluator, summary, receipt, and resolution version mismatch;
- underfunding and full-span cost overflow;
- premature Finalize and all terminal replays;
- explicit gaps and out-of-range accepted points;
- unstarted payer/nonpayer, partial-progress, expired-incomplete,
  valid-complete, gap-complete, and exact-inexact Abort cases;
- short expiry, backward slot, late Fold, and late valid Finalize;
- exact funding conservation; and
- batch-size savings affecting only frozen per-call cost, never payout.

These are executable refinements and adversarial examples, not a proof of
Solana runtime behavior or cryptographic security.

## Shared BasisDomain summary cache

Many markets could deliberately share the same basis and source archive
domain. Re-evaluating an identical sealed interval for each market is wasteful.
A future immutable summary cache can be safe, but “same domain” alone is too
weak. Its complete key must bind at least:

```text
evaluator_version
occupation_summary_version
canonical BasisSpec digest and exact BasisSpec
SourceSpec digest
archive-domain digest and generation
grid identity and bucket duration
exact sealed archive PDA, owner, and full stored commitment
segment start and end buckets
summary codec version
```

The value must contain the exact fixed-width masses, sample count, coverage
count, and gap count, plus a commitment over key and value. Segment caches can
be joined only under the existing equal-domain and exact-adjacency rules. Cache
construction should itself be cursor-bound, authenticated, prepaid, and
permissionless; otherwise it merely moves the operator assumption.

The cache is not a resolution authority. Each market must still:

- prove its immutable Terms names the exact basis/source/archive/window;
- select its own frozen finalization mode;
- compute or verify its canonical V4 vector from the cached exact summary;
- bind its own market and resolution target; and
- perform its own one-time terminal write and liability transition.

A cache entry keyed only by grid/basis domain would allow an alternate archive
with the same domain to substitute. A cache entry keyed only by a final vector
would erase counts, gaps, interval, source lineage, and rounding authority.
Neither is acceptable.

Cache rent reclamation, concurrent builders, duplicate cache races, reward
allocation, poisoning recovery, and whether a cache can safely close while any
market may still use it are unresolved. Content-addressed immutable entries
avoid mutation races, but a live implementation still needs either permanent
funding, explicit leases, or deterministic reconstruction path. This remains
analysis, not a proposed live account.

## Why source append must not fan out to markets

Appending one source observation should update the canonical source archive
once. It should not discover and mutate every market that might later consume
that source. Fan-out on append would introduce:

- an unbounded or silently capped market set;
- write-lock and account-list growth proportional to downstream consumers;
- partial-update states when one market account is absent or locked;
- a new censorship surface in which an unrelated market blocks source
  progress;
- duplicated evaluator work before a market's final archive/window is known;
- rent paid for abandoned or superseded markets; and
- ambiguous rollback when source admission succeeds for some consumers but
  not others.

The safer factorization is:

```text
source producer -> append/seal one canonical market-agnostic archive
                                      |
                         permissionless on-demand work/cache
                                      |
                        per-market exact finalization
```

This keeps the source lifecycle independent and lets consumers pay only for
the immutable interval they actually use. Shared summaries, if later admitted,
are populated on demand from the sealed archive rather than maintained as an
append-time side effect.

## Explicit promotion stops

The routed ResolutionWork V1 establishes the live ABI/layout, exact rent and
reward schedule, deterministic lock/recovery behavior, sealed-source
authentication, full-byte monolithic v4 equivalence, late rollback, donation
segregation, and real-SBF Begin/Fold/Finalize/Abort rows documented in
`RESOLUTION_WORK_SBF.md`. The current sealed default artifact's first-party
final-LTO stack audit passes for exact ELF
`af6bb79cc3766bd0d889b46dc1becfebe140c7df2746971943e9edf4efc2014b`
(sealed at `7931e23`; the preceding `bd20711b…` and `a5725a3d…` seals remain
valid historical evidence only and keep their complete artifact directories).
The following remain explicit stops or scope limits:

- route-level evidence is not permission to call the program or deployment
  live; production source release, direct selection, and system-wide liveness
  retain independent gates;
- the default source registry is empty, so Endow refuses
  `SourceReleaseUnavailable` (`0x79`); mock-source success belongs to a distinct
  `non-production-mock-source` ELF;
- live Direct V2 Select reaches exactly 1,400,000 CU and rolls back, while
  Direct V3 remains MODEL/DESIGN only;
- no global `LivenessPolicy`, terminal-closure result, or no-stranding theorem
  has been promoted;
- the measured spans and degrees do not justify interpolation or extrapolation;
- the current SourceArchive has no authenticated gap-record encoding, so the
  live adapter refuses genuine intervals rather than silently capping them;
- V1 charges are zero and makes no nonzero charge-disposition claim;
- no shared BasisDomain cache account, funding rule, or reclamation policy is
  implemented;
- the state machine and hash constructions are executable refinements, not a
  formal proof; and
- no path may accept an unchecked off-chain payout, mass vector, record bytes,
  or proof material.

Claims about live ABI, rent, accounts, compute, or rollback must cite the SBF
evidence document and its exact artifact, not the model alone.

## Reproduction

```sh
cargo test --manifest-path research/resolution-work-v1/Cargo.toml
cargo test --release --manifest-path research/resolution-work-v1/Cargo.toml
cargo clippy --manifest-path research/resolution-work-v1/Cargo.toml --all-targets -- -D warnings
cargo doc --manifest-path research/resolution-work-v1/Cargo.toml --no-deps
```
