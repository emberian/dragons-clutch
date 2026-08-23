# Candidate lifecycle V2 kernel boundary

Status: **production-bound kernel; not connected to SBF; not release evidence**

Normative crate: `crates/clutch-candidate-lifecycle`

ADR: `docs/adr/0006-two-window-candidate-lifecycle.md`

This document fixes the safe-Rust boundary for the two-window candidate
lifecycle. The crate is independently buildable, dependency-free, `no_std`,
`no_alloc`, fixed-capacity, and forbids unsafe code. It does not modify the live
general-clearing dispatcher or Score implementation.

The kernel is total over decoded inputs: account and instruction codecs require
exact lengths and known versions; public state validators refuse malformed
values; and transitions accept values by copy and return complete replacements.
An error therefore cannot expose a partially mutated kernel value. The adapter
must still make the returned multi-account update and lamport transfers atomic.

## Schedule and authority

Freeze is permissionless at or after the immutable `freeze_deadline_slot`. A
successful freeze reads one authenticated Clock slot `F` and stamps, with
checked addition:

```text
S = F + submission_span_slots
V = S + verification_span_slots

[F, S)  submission: begin and seal
[S, V)  verification: progress and checked completion
[V, +∞) hard terminal path: finalize and expire unverified work
```

At `S`, begin and seal refuse while verification is admitted. At `V`, every
verification mutation refuses while hard finalization and sealed-candidate
expiry are admitted. Early finalization is admitted at or after `S` only when
`verdict_count == sealed_candidate_count`. An unfinished candidate cannot block
the hard path at `V`.

Slots are the sole consensus clock. Wall time is a client estimate. A fork
rollback rolls back the schedule transition with the rest of account state.
No operator can extend, shorten, reopen, or manually close either interval.

## Fixed geometry and semantic owners

```text
CandidateIndex page capacity     16
CandidateIndex page count         4
maximum begun candidates         64
retained rank candidates          3
rank-key capacity                64 bytes
```

The lifecycle policy can select a lower begun-candidate cap but never a higher
one, and fixes a maximum feed byte length. **Begin**, not Seal, consumes capacity and appends the identity to the
canonical page. Consequently staging records are bounded and enumerable; an
attacker cannot create an unbounded unindexed tail and strand rent. Pages are
sequential and zero padded. Candidate identities are append-only; a monotone
closed bit records terminal cleanup for each enumerated identity. All four pages are pre-created and
their rent is paid by the epoch sponsor so a same-slot begin does not create a
shared rent-ownership ambiguity.

Persisted facts have one owner:

| Fact | Semantic owner |
| --- | --- |
| `F`, `S`, `V`, begun/sealed/verdict/expiry counters, top candidates, selection | `CandidateWindowV3` |
| Lifecycle status and immutable solver/reward destination | `CandidateRecordV2` |
| Valid/refused kind, relation digest, and generic rank key | immutable `CandidateVerdictV1` |
| Candidate work, bond, cleanup, solver-credit, and rent accounting | `CandidateEscrowV2` |
| Freeze/finalize/index-cleanup/solver-prize funding | `EpochCandidateBudgetV2` |
| Exhaustive begun enumeration | `CandidateIndexPageV1` |
| Epoch cleared/lapsed phase and live-child retirement counters | existing Epoch/retirement owner, atomically mirrored by the adapter |

Candidate lifecycle status is deliberately small:

```text
0 STAGING  -> 1 SEALED -> 2 VERDICTED
     |            |
     v            v
3 EXPIRED_STAGING 4 EXPIRED_UNVERIFIED
```

Valid versus refused belongs only to the referenced Verdict. Selection belongs
only to the Window. A valid loser is not rewritten as “superseded,” and a
selected record is not given a second lifecycle status.

Window counters obey:

```text
sealed + expired_staging <= begun
verdict + expired_unverified <= sealed
valid_verdict <= verdict
top_count = min(valid_verdict, 3)
candidate_page_count = ceil(begun / 16)
```

The index plus terminal status schema supports exhaustive cleanup. The index
does not by itself authorize closing the Epoch root; the retirement owner must
also prove its separately maintained live-child counters are zero.

## Score-policy independence

The Window binds a `score_policy_id` and one exact `rank_key_len`. The kernel
does not import ScoreV1, ScoreV2, clearing math, or a score DTO. A checked
adapter outcome supplies the immutable Verdict rank.

Rank comparison is descending lexicographic order over exactly
`rank_key_len` active bytes, followed by canonical zero padding to 64 bytes.
The final 32 active bytes must be the bitwise complement of the candidate
identity. Thus smaller candidate identity wins an otherwise exact score tie,
and injectivity can be validated locally without scanning every Verdict. The
score policy owns the prefix and its byte order. A different score rule uses a
different policy identity for a future epoch without changing the timing state
machine. Mixed score identities or key lengths refuse.

The Window stores the retained **candidate identities**. Verdict accounts own
the corresponding ranks. Any transition that changes or consumes the retained
top set must receive and authenticate those immutable Verdict values, verify
strict best-first order, then recompute the bounded set.

The selected result is the **best valid submitted candidate** among checked
Verdicts. It is not “optimal clearing” without a separately checked optimality
certificate.

## Exact present funding

No transition treats future fees, expected trading revenue, collateral, or
Hoard principal as funding.

Begin prepays the candidate bond, staging rent principal, and both cleanup
rewards. Seal is an exact top-up boundary: it prepays verification rent and
exactly

```text
verification_units * progress_reward_per_unit + completion_reward
```

The Epoch sponsor separately prepays the freeze reward, finalizer reward, four
index-page close rewards, optional solver prize, Epoch budget rent, and exact
rent principal for all four pre-created index pages. Every multiplication and
addition is checked integer arithmetic.

The kernel caps declared feed bytes and verification units. The adapter must
derive their canonical values and exact Solana rent from the authenticated
relation/profile; it must not trust Begin intent declarations. Until that
boundary is connected, this crate alone is not runtime-capitalization evidence.

Candidate balances are separate conservation compartments:

```text
work_remaining + work_paid + work_refunded = work_initial
bond_remaining + bond_slashed + bond_refunded = bond_initial
cleanup_remaining + cleanup_paid + cleanup_refunded = cleanup_initial
solver_remaining + solver_paid = solver_credited
```

Epoch balances have equivalent freeze, finalizer, index-cleanup, and solver
compartments. Rent principal is never a fee, keeper reward, penalty, or solver
prize.

| Terminal fact | Bond | Work | Cleanup/rent | Solver prize |
| --- | --- | --- | --- | --- |
| valid Verdict | all refundable | completion paid; remainder after authenticated work close | close reward; unused expiry reward and rent refund last | selected solver may claim the already credited epoch prize |
| refused Verdict | exact invalidity penalty to neutral sink; remainder refundable | same fixed progress/completion schedule as valid | same close path | none unless impossible state is supplied, which refuses |
| expired staging | exact abandonment penalty; remainder refundable | no work compartment exists | expiry then close reward; staging rent refunds | none |
| expired unverified | no invalidity slash; all bond refundable | paid progress stays paid; unused work refunds after authenticated abort/close | expiry then close reward; rent refunds | none |

Malformed instructions and unchecked adapter outcomes do not enter a slashing
transition and pay zero. Bond refund for `VERDICTED` requires the immutable
Verdict: valid must have zero slash; refused must have exactly the configured
invalidity penalty. Deadline failure is never classified as invalidity.

Cleanup is last. It requires final selection, bond refund, work closure/refund
for sealed candidates, and solver claim when credited. This prevents closing a
record that is still needed to authenticate a claim. Cleanup atomically marks
the candidate's index bit closed. Index pages close in reverse order only when
every active bit is closed; each returns exactly one quarter of their divisible prepaid rent
principal. Unused epoch rewards refund only after all four pages close. Epoch
root retirement remains separate.

## Versioned accounts and wire

No existing tag/version changes meaning in place. The tags below are
**kernel-local envelope tags**, not globally reserved Solana account tags. The
adapter must reserve/map semantic families without colliding with the live
layout before promotion.

| Account | Tag/version | Exact bytes | Notes |
| --- | ---: | ---: | --- |
| `CandidateWindowV3` | 1/3 | 379 | six policy/epoch identities, schedule, selected/top identities, six counters |
| `CandidateIndexPageV1` | 2/1 | 552 | epoch, 16 canonical begun identities, and monotone closed mask |
| `CandidateRecordV2` | 3/2 | 421 | 12 identities, three slots, feed/work geometry, status |
| `CandidateVerdictV1` | 4/1 | 240 | immutable relation/score result and 64-byte rank capacity |
| `CandidateEscrowV2` | 5/2 | 311 | exact candidate funding compartments and claim bits |
| `EpochCandidateBudgetV2` | 6/2 | 295 | exact epoch funding compartments and page-close cursor |
| `CandidateLifecyclePolicyV2` | 7/2 | 60 | spans, feed-byte cap, and work/candidate bounds |
| `CandidateLivenessPolicyV2` | 8/2 | 156 | immutable present-funding amounts and sink |

Instruction intents use magic `0xc7`, version `2`, a one-byte kind, exact
variant-specific fields, and no trailing bytes. They cover Freeze, Begin, Seal,
Progress, Complete, Finalize, both expiry classes through one intent, work
closure, the four claim/cleanup paths, reverse page close, and epoch-unused
refund. The Complete intent names the expected Verdict identity; it does not
carry trusted score or relation claims.

This is a **new-epoch-only** family. Existing shared-deadline epochs decode and
finish under their existing version and intent family. A profile must authorize
all four policy identities and the new account family before creating a V2
epoch. Unknown tags, versions, flags, enum values, lengths, padding, or mixed
policy identities refuse.

## Adversarial transition table

| Attempt | Kernel result | Adapter consequence |
| --- | --- | --- |
| Freeze at `freeze_deadline - 1` | refuse, no replacement values | move no lamports |
| Freeze at or after the opening slot | stamp checked `F/S/V` once | authenticate one Clock read and commit Window+budget atomically |
| Begin at `S`, or beyond cap | refuse | create no record, debit no funds |
| Begin succeeds but never seals | identity remains enumerated STAGING | at/after `S`, anyone can expire it from prepaid cleanup |
| Seal with short feed, noncanonical padding, or nonexact work reserve | refuse | preserve stage and all balances |
| Verification at `S-1` / `S` | refuse / admit | use the same authenticated Clock slot for the transition |
| Repeat a progress checkpoint | refuse as replay | pay zero |
| Complete without all declared units | refuse unresolved | keep exact checkpoint/work reserve |
| Malformed refused outcome | refuse before slash or reward | pay/slash zero |
| Relation-checked refusal | immutable refused Verdict; exact penalty | transfer penalty only to immutable neutral sink |
| Final verification mutation at `V` | refuse | winner set cannot change |
| Early finalize with any sealed record lacking Verdict | refuse unresolved | pay zero |
| Finalize at `V` with unfinished work | select checked top or lapse | does not require withheld unfinished accounts |
| Expire unverified candidate at `V` | terminal without invalidity slash | pay prepaid expiry reward; later refund unused work/bond |
| Supply top candidates out of order or with wrong Verdict rank | refuse | no selection mutation |
| Replay finalize/expiry/refund/reward/close | refuse | pay zero |
| Close candidate before claims/work closure/finalization | refuse | preserve rent and state |
| Close index page out of reverse order | refuse | preserve rent and close cursor |
| Crash between successful transactions | resume from persisted counter/cursor/status | no keeper identity or service lease exists |

## Adapter obligations and unresolved promotion blockers

The crate exposes `ADAPTER_OBLIGATIONS` as typed documentation. A conforming
SBF adapter must:

1. authenticate the Clock sysvar;
2. derive and authenticate every identity and PDA;
3. prove Candidate, Escrow, and Feed admission PDAs are fresh canonical targets;
4. implement solver authentication, reward-destination authorization, and a
   copy-resistant admission rule;
5. derive canonical feed geometry, work units, and exact rent from the bound profile;
6. authenticate exact complete feed bytes, content digest, and padding;
7. execute the bound relation policy and derive, rather than trust, the result;
8. execute the bound score policy and derive the canonical rank key;
9. authenticate work checkpoint progress and terminal closure/abort;
10. authenticate settlement-terminal evidence before selected-witness cleanup;
11. move each typed lamport disposition exactly;
12. commit every returned account replacement atomically;
13. atomically mirror Window finality into the existing Epoch owner; and
14. close only owned accounts and return rent to the recorded destination.

The crate also exposes `PROMOTION_BLOCKERS`. They are not claims of solved
security:

- **Copy/front-running admission:** immutable solver fields do not themselves
  prove that a copier cannot steal admission or the prize. The admission proof
  and transaction account frame remain undecided.
- **Quality/capacity denial of service:** a fixed, prepaid cap bounds resource
  use but does not prove that low-quality validly admitted candidates cannot
  crowd out a better candidate. Capacity and admission economics require a
  separate adversarial design and measured throughput.
- **SBF adapter not connected:** no live handler currently authenticates the
  listed evidence, moves funds, or commits this state.
- **Epoch root retirement:** index cleanup does not replace exhaustive live
  child counters or the separate retirement kernel/adapter.
- **Global account tags:** kernel-local envelope tags are not live Solana tags;
  a collision-free global mapping/reservation is still required.

Until those obligations are implemented and tested at the SVM boundary, this
crate is production-bound source and executable design evidence only. It is
not a deployment, devnet, mainnet, or formal-verification claim.

## Evidence commands

```sh
cargo test --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml
cargo test --release --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml
cargo clippy --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml \
  --all-targets --all-features -- -D warnings
cargo doc --manifest-path crates/clutch-candidate-lifecycle/Cargo.toml --no-deps
```

The adversarial tests cover both exact boundaries, begun enumeration and cap,
feed/funding refusals, valid/refused completion, hard finalization with
unfinished work, both expiry classes, exact slash/refund compartments, local
rank injectivity, reverse page cleanup, codec versions/lengths, score-policy
mixing, arithmetic overflow, and typed unresolved blockers. The remaining
evidence must be supplied by the adapter and SVM integration tests named above.
