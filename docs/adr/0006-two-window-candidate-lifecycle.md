# ADR-0006: two-window, prepaid candidate lifecycle

Status: **proposed**

## Context

The general clearing plane currently has one candidate deadline. `FreezeEpoch`
stamps `selection_deadline_slot = freeze_slot + 1_000`; `SubmitCandidate` and
`SealCandidate` require `Clock.slot < selection_deadline_slot`,
`CompleteClearWork` must persist its verdict before that same boundary, and
`FinalizeSelection` opens at the boundary. `WriteCandidateFeed`, checkpoint
creation/growth, and the streamed advances do not themselves read the Clock.
Only candidates which finish verification and enter the retained top-three
registry before the boundary can compete.

This is safe against admitting a late verdict, but it is not submission-fair:
an otherwise timely candidate can lose simply because its multi-transaction
verification did not finish in the same window in which other candidates were
still arriving. The Window also does not enumerate sealed-but-unverified,
refused, superseded, or valid noncompetitive children. That prevents exhaustive
cleanup and contributed to the current fail-closed `CloseGeneralEpoch` posture.

Direct V3 already demonstrates that separate submission and staged-verification
intervals can remain permissionless. The general plane needs the same semantic
separation without tying its account layout to `ScoreV1`, assuming a privileged
solver/keeper, or pretending future fees can capitalize current work.

## Decision

Introduce a new general candidate lifecycle for **new epochs only**. It has two
exclusive slot boundaries after the book freezes:

```text
F = frozen_slot
S = submission_closes_slot
V = verification_closes_slot (and ordinary finalization opens)

       submission                 verification                 terminal
F -------------------- S ------------------------------ V ----------------->
  Begin/Write/Seal       Begin/Grow/Advance/Complete      Finalize/Expire/Close
  admitted iff slot < S  admitted iff S <= slot < V       admitted iff slot >= V
```

At an exact boundary, the interval on the left is closed and the one on the
right is open. Thus `slot == S` refuses candidate creation and sealing but
admits verification; `slot == V` refuses every verification mutation and
admits finalization. This convention must be shared by the layout model, SBF
adapter, keeper, UI, and fixtures.

### Schedule construction

`FreezeEpochV2` remains permissionless at or after the existing
`freeze_deadline_slot`. In the successful transaction it reads the canonical
Clock sysvar and atomically stamps:

```text
frozen_slot               = Clock.slot
submission_closes_slot    = frozen_slot + submission_span_slots
verification_closes_slot  = submission_closes_slot + verification_span_slots
```

Both additions are checked. The spans come from the immutable
`CandidateLifecyclePolicyV2`, are nonzero, and lie within code-supported
bounds. No signer may choose, extend, shorten, or reopen either interval after
epoch initialization. A late freeze shifts both intervals together and does
not silently shorten them.

Slots, not Unix timestamps or block height, are authoritative. Slot duration is
variable, skipped slots still advance the comparison, and a fork rollback rolls
back the whole state transition. Wall-clock values shown by clients are
estimates. Every gated instruction authenticates the Clock sysvar by its
well-known identity and reads one `u64` slot for the whole transition.

### Immutable policy bindings

The Window binds three independent content identities:

- `candidate_lifecycle_policy_id`: spans, capacity, page geometry, transition
  rules, and expiry behavior;
- `score_policy_id`: score rule and canonical rank-key format; and
- `liveness_policy_id`: required deposits, exact keeper rewards, invalidity and
  abandonment penalties, and destinations.

The epoch's existing clearing-relation policy remains a fourth, separate
binding. All presented policy accounts must re-derive their identities and be
authorized by the immutable market/profile. No policy is selected by a service
or changed inside a live epoch.

`ScoreV2` therefore does **not** imply `EpochWindowV4` or a new timing state
machine. A verified candidate produces a `CandidateVerdictV1` containing the
bound `score_policy_id` and a canonical, fixed-capacity rank key. The generic
selection lifecycle compares rank keys lexicographically; the score
implementation owns validation and encoding. The key includes a final
candidate-identity tie component, so it defines a total order. A future score
rule uses a new policy identity and code-supported key encoder for future
epochs, while the lifecycle wire and account versions stay unchanged. Mixed
score policies inside one epoch refuse.

### What counts as submitted

`BeginCandidateV2` creates a `STAGING` record, feed stage, and funding escrow.
This is not a submitted candidate. Sequential `WriteCandidateFeedV2` calls may
fill the stage. Only `SealCandidateV2`, committed in a slot strictly below `S`,
does all of the following atomically:

1. authenticates the frozen epoch, Window, lifecycle/score/liveness policies,
   record, complete feed stage, escrow, and candidate identity;
2. converts the feed to its immutable sealed tag;
3. changes the record `STAGING -> SEALED` and stamps `sealed_slot`;
4. guarantees the policy's complete verification rent and reward reserve are
   funded, without counting future fee revenue or Hoard principal;
5. appends the candidate identity to the canonical CandidateIndex page set;
6. increments `sealed_candidate_count` and the candidate-set fold.

CandidateIndex pages have fixed sequential indices and canonical padding, like
OrderPages. The initial format allows a policy-selected cap up to a compile-time
maximum; a concrete deployment cap must be justified against measured
worst-case verification throughput and prepaid work, not chosen as a marketing
number. A full set refuses another seal before touching the stage or funds.

The index makes every sealed child enumerable for verification and terminal
cleanup. It is not a score registry and its append order never affects rank.
Same-slot appends serialize on the Window/tail page. A candidate's canonical
identity/PDA makes the same free coordinates idempotent: the second begin sees
an existing target and refuses.

An incomplete stage may still receive its unique next chunk after `S`; that
cannot seal it, add it to the set, or affect selection. This preserves the
current one-account sequential writer without adding Clock to every upload
chunk. `ExpireCandidateV2` wins any serialized race after `S` and closes or
marks the unsealed stage. Clients should stop uploading at `S`; the protocol's
semantic boundary is the atomic seal.

### Verification and rank retention

Verification begins only at `S`. `InitClearWorkV2` authenticates a `SEALED`
record and CandidateIndex membership before creating work. Every grow, order
advance, slice advance, and completion reads the bound Window and Clock and
requires `S <= Clock.slot < V`. These instructions remain permissionless. The
checkpoint cursor, consumed fold, and account state are their replay authority;
there is no keeper identity in the relation.

`CompleteCandidateVerificationV2` is one atomic terminal verdict:

- valid relation: create/stamp an immutable valid `CandidateVerdictV1`, change
  `SEALED -> VERIFIED_VALID`, increment `verdict_count`, and insert or replace
  the candidate in the Window's verified-only top registry using its canonical
  rank key;
- relation refusal: create/stamp a refused verdict, change
  `SEALED -> VERIFIED_REFUSED`, and increment `verdict_count` without touching
  the top registry.

A valid candidate outside the retained top remains `VERIFIED_VALID`. Eviction
from the top registry does not rewrite or erase its verdict and is not a
candidate lifecycle state. This removes the current need to make the displaced
record writable and preserves an auditable proof of validity.

The top registry is only an acceleration structure. CandidateIndex pages plus
the verdict counter are the exhaustive submitted-set truth. Final selection
still means the **best valid submitted candidate which received a checked
verdict under the epoch's score policy**, never globally optimal clearing.

### Finalization

`FinalizeSelectionV2` is permissionless and one-way. It is admitted when
submission is closed and either:

```text
verdict_count == sealed_candidate_count
```

or `Clock.slot >= V`. The first branch permits safe early finalization when
every sealed candidate has a checked terminal verdict; the second prevents one
withheld, crashed, or deliberately expensive candidate from blocking the epoch
forever. With no sealed candidates, the epoch may lapse at `S`.

The instruction authenticates the retained verified candidates and their
verdict rank keys, selects the best retained candidate, stamps
`selected_candidate` and `finalized_slot`, and changes the Epoch to `CLEARED`.
If there is no verified candidate it changes the Epoch to `LAPSED`. After `V`,
unverified work cannot complete or enter the registry even if it was one step
from completion. A delayed finalizer cannot change the winner because every
candidate and verification mutation is already closed.

Finalization requires only the bounded top registry, not every unverified
candidate account. Account withholding therefore cannot block the deadline
path. The second finalize refuses on the terminal Epoch phase.

### Candidate terminal states

Candidate lifecycle and ranking membership are separate facts:

```text
STAGING -> SEALED -> VERIFIED_VALID -> SELECTED
    |          |             `--------> (valid loser; state remains verified)
    |          `-> VERIFIED_REFUSED
    |          `-> EXPIRED_UNVERIFIED       (only at/after V)
    `-> EXPIRED_STAGING                      (only at/after S)
```

`EXPIRED_UNVERIFIED` says only that no verdict landed before `V`. It must not be
displayed as invalid. A work account may be resumed after a process or machine
crash only while the verification interval is open. At/after `V`, a
permissionless expiry/close path refunds or distributes its already recorded
funds and preserves the terminal record/index fact required for exhaustive
root cleanup.

## Prepaid bonds and rewards

Candidate economics use a distinct `CandidateEscrowV2`; rent principal is not
a fee or bond. The escrow records the payer/refund destination and four
separately accounted balances:

- exact record/feed/work rent principal;
- bounded verification-work reward reserve;
- invalidity bond; and
- abandonment/cleanup reserve.

The epoch has a separately prepaid `EpochCandidateWorkBudgetV2` for freeze,
finalization, index cleanup, and any solver prize. Neither account may include
expected fees, future trading revenue, Hoard principal, or collateral
reservations in its solvency calculation.

Rewards are earned only by monotone state progress and paid to the signer who
executes that transition. A grow step pays once per new allocation step; an
advance pays per newly consumed canonical order/slice unit, not per transaction;
completion, expiry, and finalization each pay at most once. Replays and refused
transitions pay zero. The reward schedule and maximum total are frozen in the
liveness policy and fully funded before the obligation exists.

| Terminal fact | Invalidity bond | Unused work reserve | Rent principal | Additional reward |
| --- | --- | --- | --- | --- |
| `VERIFIED_VALID` | Refund claimable immediately | Refund claimable after work closes | Refund only as each account safely closes | A winner prize, if nonzero, is credited from the prepaid epoch budget at selection |
| `VERIFIED_REFUSED` | Fixed policy penalty goes to the immutable neutral/liveness sink; remainder, if any, refunds | Refund after earned keeper rewards | Refund on safe close | Verifier receives only scheduled work rewards, not the invalidity penalty |
| `EXPIRED_STAGING` | Fixed abandonment penalty funds the one cleanup reward/sink; no invalidity judgment | No verification reserve was admitted | Refund on close | Exactly one cleanup reward |
| `EXPIRED_UNVERIFIED` | Refund; no invalidity was proved | Earned progress rewards stay paid; unused balance refunds | Refund on safe close | Exactly one expiry/cleanup reward |
| `SELECTED` | Already refundable as valid | Already handled as valid | Subject to settlement dependencies | Prepaid solver prize becomes claimable; settlement success is not funded by confiscating the bond |

Paying a fixed verifier reward independent of the verdict avoids making an
invalid verdict financially preferable. Crediting a solver prize to escrow
rather than requiring an arbitrary recipient in `FinalizeSelectionV2` keeps a
withheld external account from blocking finalization. Claim and refund actions
are separately replay-protected by escrow bits and the recorded destination.

This policy cannot guarantee inclusion against chain-wide censorship or an
insufficient verification interval. It does guarantee that no privileged
service is required, all admitted work is capitalized before it exists, no late
candidate can consume the verification interval, and one unfinished candidate
cannot block the deadline path.

## Account and wire changes

No live layout changes meaning in place.

### `EpochWindowAccountV3`

Keep the existing tag and use a new exact version/length, or use a new tag if
the decoder cannot dispatch safely by version. Proposed semantic fields:

```text
epoch, market, epoch_index
freeze_deadline_slot, frozen_slot
submission_closes_slot, verification_closes_slot, finalized_slot
selected_candidate, candidate_set_fold
candidate_lifecycle_policy_id, score_policy_id, liveness_policy_id
sealed_candidate_count, verdict_count
candidate_page_count
top_verified[3], top_count
stored_bump, flags
```

Validation requires canonical zero pre-freeze fields, `freeze < S < V`, count
bounds, unique/nonzero top entries, `top_count <= verdict_count <= sealed_count`,
zero selection fields before terminality, and a selected candidate present in
the top registry. Do not store a second phase that can disagree with Epoch;
the schedule intervals are derived from Clock and the terminal phase lives in
Epoch.

### New accounts

- `CandidateIndexPageV1`: epoch, page index/count geometry, prior/page fold,
  fixed candidate-identity entries, count, bump, flags.
- `CandidateRecordV2`: immutable free coordinates plus candidate/index/policy
  bindings, `submitted_slot`, `sealed_slot`, `verdict_slot`, lifecycle status,
  escrow identity, bump, and flags. Remove claimed ScoreV1 components.
- `CandidateVerdictV1`: candidate/epoch/relation digest, score policy identity,
  valid/refused kind, canonical rank-key length and bytes, refusal code,
  verified slot, bump, flags. It is immutable after creation.
- `CandidateEscrowV2`: exact principal, reserve, bond, destination, paid/refund/
  slash bits, and funding generation.
- `EpochCandidateWorkBudgetV2`: exact prepaid epoch-level rewards and solver
  prize reserve.

CandidateIndex pages solve enumeration; they do not authorize a close by
themselves. Epoch root retirement additionally requires exhaustive live-child
counters and monotone market epoch identity, as the state/rent audit already
requires.

### New intent family

Use fresh tags and exact encodings:

1. `InitEpochV2 { market, epoch_index, relation_policy_id,
   candidate_lifecycle_policy_id, score_policy_id, liveness_policy_id,
   freeze_deadline_slot }`
2. `FreezeEpochV2 { market, epoch }`
3. `BeginCandidateV2 { market, epoch, free_coordinates,
   declared_slices, refund_destination }`
4. `WriteCandidateFeedV2 { market, epoch, candidate, cursor, chunk }`
5. `SealCandidateV2 { market, epoch, candidate }`
6. `InitClearWorkV2`, `GrowClearWorkV2`, `AdvanceClearWorkV2`,
   `AdvanceClearSlicesV2`, `CompleteCandidateVerificationV2`
7. `FinalizeSelectionV2 { market, epoch }`
8. `ExpireCandidateV2 { market, epoch, candidate }`, followed by the
   dependency-ordered close/refund instructions.

`BeginCandidateV2` carries no claimed score. Every V2 verification mutation
adds the Window and Clock to its authenticated account frame. Begin/grow also
authenticate enough record/index state to prove the work belongs to a sealed
candidate in this epoch. Close instructions accept both a full ClearWork and
the canonical growing-prefix lengths so a crash during allocation cannot strand
principal.

## Adversarial transition table

| Starting condition / attempt | Required result | Reason |
| --- | --- | --- |
| Freeze at `freeze_deadline - 1` | Refuse, no writes | Permissionless does not mean early |
| Freeze at deadline; span addition overflows | Refuse, no writes | No wrapped or missing window |
| Begin at `S - 1`, seal at `S` | Seal refuses; stage later expires | Atomic seal defines submission |
| Begin or seal at `S` | Refuse, no debit/append | Submission close is exclusive |
| Write next stage chunk after `S` | May advance only the inert stage; never seals or indexes | Upload cursor is not admission |
| Seal same candidate twice | Second call refuses on record/feed/index state | Candidate PDA and one-way tag are replay guards |
| Seal beyond policy capacity | Refuse before funds or page mutation | Verification workload stays bounded |
| Verify at `S - 1` | Refuse | Candidate set is not closed |
| Verify at `S` | Admit | Verification interval is left-inclusive |
| Advance the same cursor twice | First advances/pays; replay refuses/pays zero | Cursor and fold are authority |
| Advance with another candidate's page/feed/work | Refuse | Full epoch/candidate/index bindings |
| Final verification step lands at `V` | Refuse atomically; prior work remains | Verification close is exclusive |
| Complete with a different score-policy identity | Refuse | One total order per epoch |
| Valid candidate ranks outside top three | Persist valid verdict; registry unchanged | Bounded acceleration must not erase truth |
| Better valid candidate displaces a top entry | Replace Window entry only; old verdict remains valid | Ranking membership is not lifecycle state |
| Finalize before `V` with unresolved candidates | Refuse | Early path requires exhaustive verdict count |
| Finalize before `V` after all candidates have verdicts | Select/lapse once | Candidate set is closed and exhaustive |
| Finalize at/after `V` with unfinished work | Select verified top or lapse; do not require unfinished accounts | Withholding cannot block terminality |
| Complete work after finalization or after `V` | Refuse; cannot enter registry | Winner is immutable |
| Expire sealed candidate before `V` | Refuse | Keeper still has time to resume |
| Expire sealed unresolved candidate at `V` | Mark expired, no invalidity slash | No verdict was proved |
| Re-run finalize/refund/slash/reward claim | Refuse/pay zero | Terminal phase and paid bits are monotone |
| Supply fake Clock or mutable policy account | Refuse | Sysvar/PDA/content identity authentication |
| Keeper process dies mid-pass | Any keeper resumes exact stored cursor before `V` | No service or signer owns progress |
| Solver withholds its refund account | Finalization still succeeds; credit remains in escrow | Recipient availability is not protocol liveness |

## Consequences

Positive:

- candidate search and candidate verification no longer compete for the same
  slots;
- the sealed candidate set is bounded and enumerable;
- an unfinished candidate cannot block finalization;
- every keeper obligation is prepaid and permissionless;
- validity, ranking membership, and payment status have one semantic owner
  each;
- ScoreV2 and later score rules can change without another timing migration;
- crash recovery and terminal cleanup have explicit transitions.

Costs:

- every verification transition gains Window and Clock reads;
- sealing serializes on the Window/index tail and must fund more state;
- CandidateIndex, Verdict, Escrow, and epoch budget accounts add rent;
- a finite verification interval still cannot promise inclusion under chain
  censorship or congestion;
- the policy capacity and spans require measured worst-case CU/transaction
  evidence before adoption.

## Rejected alternatives

- **Keep one longer shared deadline.** More time does not remove the structural
  last-submission disadvantage.
- **Let verification finish after finalization.** A late verdict could change
  the winner or become a meaningless expenditure.
- **Let an operator close submissions or choose extensions.** This introduces
  discretionary authority and a censorship surface.
- **Finalize only after every candidate completes, with no hard deadline.** One
  abandoned checkpoint can lock every order reservation forever.
- **Slash every unverified candidate as invalid.** Deadline failure is not a
  checked relation refusal.
- **Pay keepers from expected fees or collateral.** Future revenue is not
  capitalization and Hoard principal has a different owner.
- **Store ScoreV2 fields directly in Window/Candidate.** It forces an unrelated
  lifecycle migration for every score experiment and permits mixed-order bugs.
- **Use unindexed candidate PDAs only.** They are individually authentic but
  not exhaustively enumerable, so they cannot support bounded work or safe root
  cleanup.

## Verification impact

Before implementation is promoted, require:

1. a dependency-free executable state model covering every boundary and
   terminal transition;
2. byte-exact codec tests for all new versions and wrong-version/trailing-byte
   refusals;
3. SVM tests at `S-1`, `S`, `V-1`, and `V` for every gated instruction;
4. crash/restart tests at every grow, order, and slice cursor;
5. hostile same-slot seal, capacity, duplicate, withheld-account, rank-policy,
   and reward-replay campaigns;
6. exact lamport conservation for every valid/refused/expired/selected path;
7. worst-case measured verification throughput used to justify each admitted
   `(capacity, verification_span)` profile; and
8. a migration test proving old shared-deadline epochs still decode and finish
   only under their old instruction family.

The companion model is in
[`../../research/candidate-lifecycle-v2`](../../research/candidate-lifecycle-v2).
It is model-only evidence, not an SBF implementation claim.

## Migration and versioning

There is no in-place migration. Existing `EpochWindowAccount` v2, Candidate,
Feed, ClearWork, intents 47–57, status values, and the frozen 1,000-slot policy
retain their historical meanings. Old epochs finish or lapse under the current
shared deadline.

New epochs opt into the new intent/account family and policy identities. A
market/profile must explicitly authorize that family. Clients dispatch from
the authenticated Window version and refuse unknown versions; they never infer
semantics from account length alone. Candidate/index/verdict PDA seeds include
their family/version domain where reuse could otherwise alias an old child.

A ScoreV2-to-ScoreV3 change creates a new `score_policy_id` for future epochs,
not a new lifecycle version. A change to interval boundary semantics, candidate
enumeration, escrow ownership, or terminal transitions requires a successor
lifecycle ADR and fresh wire/account versions.

## Legal and authority impact

This decision adds no signing, deployment, market operation, external funding,
or release authority. It specifies offline model and future local-validator
work. Any public-network action remains separately gated.

## Evidence consulted

- `programs/solana-layout/src/clearing.rs` (`EpochWindowAccount` v2 and the
  1,000-slot shared window)
- `programs/solana-layout/src/lib.rs` (`EpochAccount`, `CandidateRecord`, and
  intents 47–57)
- `programs/clutch-sbf/program/src/instructions/orders_batch/selection.rs`
- `programs/clutch-sbf/program/src/instructions/orders_batch/clear_work.rs`
- `programs/clutch-sbf/program/src/instructions/orders_batch/clear_walk.rs`
- `programs/solana-layout/src/direct_selection_v3.rs` and the Direct V3 staged
  verification implementation
- `programs/clutch-sbf/svm-tests/tests/candidate_selection.rs`
- `docs/reviews/STATE_RENT_AUDIT_2026-08-22.md`
