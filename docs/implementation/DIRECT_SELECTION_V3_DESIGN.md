# Direct Selection V3: staged, bounded, donation-safe authority

Status: **executable model and frozen codecs; lifecycle runtime STOP**

The current V2 direct-selection path is not promotable. Its full-width
authority checks are correct, but a successful three-Candidate
`SelectDirectWindowV1` reaches the absolute 1,400,000-CU transaction limit and
rolls back. V2 also creates rent-bearing Candidate accounts without a bounded
terminal cleanup path. Commit `e874db1` preserves that negative result; it is
not evidence of successful live selection or settlement.

V3 is one schema migration, not an adapter shortcut. It keeps full relation
re-execution, stages that work across transactions, bounds physical Candidate
accounts to three, uses the canonical `clutch_liveness::DonationLedger` for
every transient account, distinguishes reservation ownership transitions, and
gives every successfully frozen phase a permissionless terminal route.

The executable transition model is
`research/batch-policy-identity/src/direct_lifecycle_v3.rs`. Versioned account
and intent codecs are frozen, but no lifecycle intent is dispatched yet.

## Exact boundary

The V3 lifecycle begins as an unfrozen Epoch with no WorkBudget and an exact
active prefix of zero, one, or two Reservation V2 accounts. Each Reservation
creation records its own rent payer, exact principal, and neutral donation
lower bound. If the Epoch has not frozen by `submission_opens_slot`, anyone may
abort it: the exact active prefix is released and closed, and Epoch records the
distinct `PREFREEZE_ABORT` terminal reason.

Successful Freeze requires exactly two observed ACTIVE reservations. At that
atomic boundary:

- the page and exact two Reservation V2 accounts are authenticated and frozen;
- both reservations are `ACTIVE`;
- the immutable schedule and verifier release identifier are fixed;
- the WorkBudget account is created and fully funded; and
- the lifecycle enters `FROZEN_EMPTY`.

WorkBudget does not exist before successful Freeze, so a never-frozen Epoch
cannot trap a work reserve. The executable model covers Reservation placement,
monotone donation observation, exact zero/one/two pre-Freeze release, hostile
prefix refusal, and the Freeze boundary. Runtime account construction and
real-bank evidence remain STOP.

## Non-negotiable authority rules

1. Market, Epoch, Book, order-set, BatchPolicy, relation-domain, Candidate, and
   relation-candidate identities remain full 32-byte values.
2. Submission open/close, selection deadline, and settlement deadline come
   only from an immutable Epoch. Window repeats all four byte-exactly.
3. Schedule spans are bounded in the semantic owner, not only the adapter:

   ```text
   2 <= submission span <= 216,000 slots
   5 <= selection span  <=  21,600 slots
   2 <= settlement span <= 216,000 slots
   ```

   The five-slot selection minimum supplies distinct opportunities for Begin,
   three Verify actions, and Finalize. It does not guarantee inclusion under
   congestion or censorship.
4. A Candidate lease can be constructed only by the existing cached full
   direct verifier. Construction rechecks canonical Candidate ID, both exact
   grid ticks, simplex, limits, exact division, fills, quantity, score,
   full relation digest, coordinates, and padding.
5. The frozen grid has at most 64 ticks. A `u64` bitmap closes replay for every
   competitively admitted tick. The verifier proves the prerequisite that one
   frozen tick has one canonical Candidate and total-order score.
6. A full top three is monotone. A valid Candidate not strictly better than the
   current worst is `REJECTED_NONCOMPETITIVE`: no Candidate or Window account
   is created and no bitmap, count, transcript, or payer balance changes. It is
   not called an admission.
7. Selection never weakens to a program-issued certificate. Each retained
   Candidate is re-executed in one `VerifyDirectCandidateV3` transaction.
   Finalize requires both the complete mask and every exact Candidate status
   `REVERIFIED`.
8. Window `top_count` and all top entries stay immutable after Finalize. A
   separate physical-live mask records that loser Candidate accounts were
   closed, so the encoded Window and the economic ledger do not disagree.
9. Each staged action authenticates one frozen compile-time verifier release
   identifier. It is not an onchain code hash or deployment identity. This
   prevents accidental mixing across upgrades. A malicious
   upgrade authority can replace the checking program itself; promotion must
   therefore use an immutable deployment or explicitly name and accept that
   authority as a trust boundary.
10. Every public model transition validates hostile prestate before shifting,
    indexing, or constructing a mask. Oversized counts, padding, duplicate
    tick/ID/digest, status-mask disagreement, and phase-shape mismatch refuse
    rather than panic.

## DonationLedger is the lamport semantic owner

Rent principal, keeper work, and unsolicited lamports are separate owners. V3
uses the committed `clutch_liveness::DonationLedger`; it does not approximate
that relation with a shortfall credit or prefund sweep.

For target balance `B`, exact payer deposit `P`, and post-create balance `A`:

```text
A = B + P
```

`P` is the complete independently required principal (and, for WorkBudget, the
complete reward deposit as an additional accounted compartment). `B` is a
neutral donation. A predictable one-lamport prefund never reduces `P` and never
becomes the payer's principal. Create/allocate/assign must authenticate this
exact delta.

At every later observation:

```text
live balance >= accounted principal/work + prior neutral donation
new neutral donation = live balance - accounted principal/work
```

At close, exact payer principal returns only to its recorded payer, remaining
WorkBudget rewards return only to their sponsor, and the complete monotone
donation goes only to the immutable Epoch/Realm neutral sink. A balance below
the prior donation plus independently accounted compartments refuses. Rent is
never keeper compensation.

A noncompetitive Candidate target is never assigned to the program and no
payer deposit occurs. If such a rejected PDA was externally prefunded, it
remains a System-owned zero-data target; it is not captured by the submitter,
keeper, or protocol. A separate permissionless neutral-normalization route may
be designed, but cannot be smuggled into an error path whose effects roll back.

## Proposed fixed schema cut

Sizes below are the intended fixed byte lengths for the codec campaign. They
are not live allocations.

### Direct Epoch V4 — 512 bytes

Bytes `0..344` preserve Direct Epoch V3. The extension is:

| Offset | Bytes | Field |
|---:|---:|---|
| 344 | 8 | `selection_deadline_slot` |
| 352 | 8 | `settlement_deadline_slot` |
| 360 | 1 | lifecycle phase |
| 361 | 1 | terminal reason |
| 362 | 1 | terminal outcome |
| 363 | 1 | exact terminal Reservation prefix count, `0..=2` |
| 364 | 8 | `selected_slot`, zero before selection |
| 372 | 32 | terminal Candidate ID |
| 404 | 32 | terminal relation-candidate digest |
| 436 | 8 | terminal quantity |
| 444 | 8 | terminal price |
| 452 | 16 | terminal consideration price-units |
| 468 | 8 | terminal slot |
| 476 | 32 | immutable neutral-lamport sink |
| 508 | 4 | canonical zero reserve |

The direct lifecycle byte owns `PREFREEZE_OPEN`, `FROZEN_EMPTY`,
`WINDOW_OPEN`, `VERIFYING`, `SELECTED`, and `TERMINAL`. The preserved common
Epoch phase is only a checked coarse projection (`OPEN`, `FROZEN`, `CLEARED`,
`SETTLED`, or `LAPSED`); it is not a second semantic owner. Phase, not terminal
reason zero, distinguishes a nonterminal Epoch from `EMPTY_LAPSE`.
`PREFREEZE_ABORT` is a distinct nonzero terminal reason. Because no frozen
order-set exists, its preserved common Epoch phase remains `OPEN`; the exact V4
lifecycle byte is `TERMINAL` and is the sole terminal semantic owner.
For that reason only, the two 32-byte terminal commitment fields form a tagged
union containing Reservation IDs zero and one according to the exact count;
they are zero-padded beyond the authenticated prefix. Frozen terminal reasons
always archive count two and retain their Candidate/relation meanings.

### Direct Candidate V3 — 488 bytes

Bytes `0..440` preserve the V2 Candidate. The extension stores:

| Offset | Bytes | Field |
|---:|---:|---|
| 440 | 32 | rent-principal payer |
| 472 | 8 | exact payer-funded rent principal |
| 480 | 8 | monotone neutral donation lower bound |

The existing status byte gains `REVERIFIED`. The PDA still commits
`(epoch,candidate_id)`, not every Candidate byte. Authority comes from unique
program-signed creation, exact decoding, immutable inputs, staged full
re-execution, and no writer for immutable Candidate fields.

### Direct Window V3 — 632 bytes

Bytes `0..456` preserve the V1 Window, including all top entries and
submission open/close. The extension is:

| Offset | Bytes | Field |
|---:|---:|---|
| 456 | 32 | Window rent payer |
| 488 | 8 | Window payer principal |
| 496 | 8 | Window neutral donation lower bound |
| 504 | 8 | competitive-tick bitmap |
| 512 | 1 | staged verification mask |
| 513 | 1 | physical-live Candidate mask |
| 514 | 2 | extension flags, zero |
| 516 | 8 | selection deadline |
| 524 | 8 | settlement deadline |
| 532 | 32 | receipt rent payer |
| 564 | 8 | receipt payer principal |
| 572 | 8 | receipt neutral donation lower bound |
| 580 | 32 | pot rent payer |
| 612 | 8 | pot payer principal |
| 620 | 8 | pot neutral donation lower bound |
| 628 | 4 | canonical zero reserve |

Existing `admitted_count` is renamed `competitive_admission_count`, bounded by
64, and equals the bitmap population. The transcript commits competitive
admissions only. Window phase gains `VERIFYING`.

### Direct WorkBudget V1 — 248 bytes

The Epoch-bound account stores Epoch, BatchPolicy, verifier release ID, reward
sponsor/rent payer, rent principal, neutral donation lower bound, current and
initial spendable reward balance, rewards paid, five strictly positive frozen
rewards, bump, phase, flags, and padding. This V3 profile requires one payer to
fund both WorkBudget rent and rewards so the create delta has one authenticated
owner.

Initialization requires at least:

```text
begin + 3 * verify + finalize + max(settle, lapse)
```

This proves solvency for the named finite payments. It does not prove the
positive amounts are sufficient incentives; final values require measured SBF
costs and policy review.

### Reservation V2 — 618 bytes

The live account-plane `MAX_OUTCOMES` is 16, so Reservation V1 is 570 bytes:
its `initial_internal[16]` occupies bytes `314..442` and its
`remaining_internal[16]` occupies `442..570`. V2 preserves all 570 bytes and
appends rent payer (32), exact payer principal (8), and neutral donation lower
bound (8). The resulting account is exactly 618 bytes. A 490-byte encoding
would truncate live reservation semantics and is explicitly refused. Both
exact Reservation V2 accounts are authenticated by every transition. Their
state path is typed:

```text
Finalize:       ACTIVE   -> ENTITLED
Settle:         ENTITLED -> CONSUMED
empty/pre lapse ACTIVE   -> RELEASED
post lapse:     ENTITLED -> RELEASED
```

Terminal routes close and refund both reservation rent principals exactly
once. Existing Reservation V1 accounts cannot promise this cleanup and stay
outside the V3 promotion claim.

### DirectBatchPolicy V3 / verifier release identity

Legacy BatchPolicy remains its exact 64-byte artifact. A disjoint 96-byte
DirectBatchPolicy V3 artifact stores those exact policy bytes followed by one
full `verifier_release_id`. Its digest is domain-separated over the canonical
Epoch context and all 96 body bytes, and its final PDA uses a disjoint seed.
`verifier_release_id` is not called an onchain program-data or deployment hash:
it is a compile-time release identifier owned by the exact verifier
implementation. Submit, Begin, Verify, Finalize, and Settle compare it to the
Epoch/WorkBudget binding; Lapse retains a versioned escape path. Upgradeable
deployment trust or immutable deployment remains an explicit promotion
boundary.

The executable model recomputes this epoch-bound artifact identity
(`direct_policy_v3_id`) on every state validation and keeps it disjoint from
the legacy 64-byte policy digest that keys the relation domain, candidate, and
reservation bodies. A cross-crate test asserts the model digest is
byte-identical to the codec's `digest_for_epoch`.

## Frozen intent codec registry

The layout allocates common intent version 3 tags 36 through 46. These bytes
are codec authority only until each corresponding runtime handler and real-SBF
campaign lands:

| Tag | Intent | Bytes |
|---:|---|---:|
| 36 | `InitDirectEpochV4` | 138 |
| 37 | `FreezeDirectEpochV4` | 114 |
| 38 | `AbortUnfrozenDirectV4` | 66 |
| 39 | `SubmitDirectCandidateV3` | 74 |
| 40 | `BeginDirectVerificationV3` | 66 |
| 41 | `VerifyDirectCandidateV3` | 67 |
| 42 | `FinalizeDirectSelectionV3` | 66 |
| 43 | `SettleDirectV3` | 66 |
| 44 | `LapseEmptyDirectV3` | 66 |
| 45 | `LapseUnselectedDirectV3` | 66 |
| 46 | `LapseSelectedDirectV3` | 66 |

No payer, account index, keeper identity, outcome, quantity, settlement price,
or consideration is caller-selected by the terminal wires. Transaction account
roles and the authenticated Clock supply operational facts; persisted authority
supplies every economic fact.

## Routed transition plan

### `InitDirectEpochV4` / V4 Reservation placement

Init creates only the durable pre-freeze Epoch. Each V4 order placement creates
one append-only Reservation V2 and persists its authenticated payer principal
and neutral donation lower bound. Every later placement observes the complete
existing reservation prefix. A third reservation, nonzero padding, late
placement, release-ID mismatch, or balance below accounted principal plus the
prior donation refuses before mutation. A reservation that encumbers nothing —
a zero-limit, zero-fee buy — refuses at creation, because its release is a
Position no-op the release kernel's unchanged-poststate refusal would
otherwise block forever.

The existing `PlaceOrder` business wire may be reused, but its V4 branch must
require lifecycle phase `PREFREEZE_OPEN` in addition to coarse Epoch `OPEN` and
must create Reservation V2, never V1. The program derives the page-zero prefix
rank; the caller cannot choose a prefix count. The model authenticates the
complete Reservation body (canonical identity/PDA input, owner, Position
generation, order ID/generation, market, Epoch, policy, grid, terms,
direct-single kind, side, outcome width, cash/Egg envelope, ACTIVE phase,
bump, and flags) against the exact page record.

### `FreezeDirectEpochV4`

Freeze requires the exact two ACTIVE Reservation V2 accounts, authenticates
their latest DonationLedgers, and creates the WorkBudget with one exact create
delta covering rent principal plus the full reward-only deposit. It cannot
infer a payer or principal from Reservation V1 and cannot freeze zero or one
reservation.

### `AbortUnfrozenDirectV4`

At `submission_opens_slot` or later, anyone may terminate a still-unfrozen V4
Epoch. The route releases and closes exactly the zero-, one-, or two-account
Reservation prefix, returns each recorded principal only to its payer, routes
all observed donations only to the immutable neutral sink, and writes the
distinct durable `PREFREEZE_ABORT` receipt. It has no WorkBudget and pays no
keeper reward. The authenticated page determines the required Reservation and
Position accounts in exact prefix order; the intent contains no count. Before
any close, the existing release arithmetic subtracts remaining reserved cash,
adds every remaining Egg back to the matching live Position, assigns canonical
`order_generation + 1` as the release generation, zeros the Reservation
assets, and marks it `RELEASED`. A repeated owner aggregates both releases into
one Position poststate; stale generations, aliases, missing/extra/reordered
accounts, and arithmetic failure refuse atomically.

### `SubmitDirectCandidateV3`

Preflight authenticates Epoch V4, BatchPolicy/release identity, Grid, frozen
page, two ACTIVE reservations, Window (or creatable PDA), exact retained
Candidate accounts, the new target, and the displaced payer when applicable.
It runs the full verifier before deciding:

- `REJECTED_NONCOMPETITIVE`: successful explicit no-state outcome, no create or
  payer debit;
- first/under-cap: exact DonationLedger create of Candidate and optionally
  Window; or
- replacement: create the new Candidate, close the former worst with exact
  payer/donation split, then atomically commit Window.

At most three Candidate accounts remain live. A seen tick cannot replay.

### `BeginDirectVerificationV3`

At `submission_close <= now < selection_deadline`, checks the frozen verifier
identity, changes `WINDOW_OPEN -> VERIFYING`, clears the mask, and pays the
strictly positive Begin reward from WorkBudget. It performs no relation work.

### `VerifyDirectCandidateV3(index)`

Authenticates the same frozen source, exact Window entry, exact Candidate PDA,
and verifier release ID. It runs the cached full verifier, changes only that
Candidate `VERIFIED -> REVERIFIED`, sets one mask bit, and pays one fixed Verify
reward. Replay, corrupt source, release-ID substitution, or deadline failure is
atomic refusal.

### `FinalizeDirectSelectionV3`

Requires exact full mask and exact `REVERIFIED` status for every retained
Candidate. It creates donation-ledger-bound receipt/pot accounts, changes both
reservations `ACTIVE -> ENTITLED`, closes loser Candidate accounts to their own
payers/neutral sink, preserves Window top count/entries, marks only top zero
live and selected, and pays the Finalize reward. No full relation hash remains
in this transaction.

### `SettleDirectV3`

Consumes the existing narrow two-order, full-fill, one-Egg, zero-fee economic
plan from ENTITLED reservations. Outcome, quantity, price, and consideration
come byte-exactly from the selected, reverified Candidate; callers supply none
of them. Before closing transient authority it writes the `SETTLED` receipt into
Epoch V4. It then transitions reservations `ENTITLED -> CONSUMED`, closes the
selected Candidate, Window, receipt, pot, WorkBudget, and both reservations,
pays Settle only from WorkBudget, returns unused rewards to the sponsor, returns
each principal to its payer, and sends all donations only to the neutral sink.

## Permissionless lapse

- `LapseEmptyDirectV3`: at the selection deadline from `FROZEN_EMPTY`, releases
  both ACTIVE reservations and closes WorkBudget plus both reservations.
- `LapseUnselectedDirectV3`: at the same deadline from `WINDOW_OPEN` or
  `VERIFYING`, releases ACTIVE reservations and closes all live Candidates,
  Window, WorkBudget, and both reservations.
- `LapseSelectedDirectV3`: at the settlement deadline, releases ENTITLED
  reservations and closes selected Candidate, Window, receipt, pot, WorkBudget,
  and both reservations without executing a trade.

Each writes a distinct durable Epoch receipt before closes. Every route checks
all recipients, DonationLedgers, live balances, and poststates before the first
mutation; transaction rollback is the atomicity boundary.

## Durable history is not transient refund

V3 returns every **transient authority** principal. It does not claim every
lamport in the protocol is refunded.

- One 512-byte Epoch archive remains per historical epoch. Its principal comes
  from the separately named archive/storage endowment and remains locked under
  the current permanent-audit policy. This is bounded per epoch but grows
  linearly with history.
- BatchPolicy artifacts are content-addressed and reusable across epochs. Their
  publisher/storage endowment and neutral donation ledger are separate from
  direct WorkBudget.
- Existing Market, Realm, Grid, Terms, Position, mint, and token-account rent is
  outside this transition.

Permissionless archive pruning, an immutable retention deadline, and an
accumulator that preserves terminal audit commitments are worthwhile future
work, but remain STOP. They must not be implied by transient cleanup.

## Executed model evidence

The research crate passes 37 tests total, including 21 V3 lifecycle tests, and
strict Clippy. V3 tests cover:

- min/max deadline spans, boundary slots, and strictly positive work rewards;
- exact full-verifier/grid-issued Candidates and hostile tick/ID/score changes;
- noncompetitive no-state outcome, replacement, replay, and top/live split;
- hostile counts, masks, padding, duplicate identities, and status mismatch
  refusing before any unsafe index or shift;
- verifier release-ID checks across staged work;
- exact `ACTIVE -> ENTITLED -> CONSUMED/RELEASED` effects;
- partial-verification and selected-path WorkBudget equations;
- canonical DonationLedger create deltas, monotone donations, shortfall refusal,
  close-time neutral disposition, payer/reward separation, and checked aggregate
  overflow;
- exact zero/one/two pre-Freeze Reservation/Position release, deterministic
  release generation, hostile prefix/order/side/policy/state/alias refusal,
  late Freeze, and pre-Freeze donation observation;
- persisted observation of every surviving Candidate, Window, WorkBudget, and
  Reservation plus next-transition refusal after donation drain;
- zero-envelope reservation refusal at placement and the exact fee-only
  zero-limit placement/release path;
- the epoch-bound DirectBatchPolicy V3 artifact identity with wrong-epoch,
  wrong-release, and substituted-identity refusal; and
- pre-Freeze, empty, pre-selection, post-selection, and settled durable
  receipts.

The layout crate additionally carries two cross-crate tripwires: the model's
recomputed frozen-page digest and order-set fold are asserted byte-identical
to the live page fold over the same two records, and the model's
`direct_policy_v3_digest` is asserted byte-identical to the codec's
`digest_for_epoch`.

## Known model open items

- Settle consumes ENTITLED reservations and writes the exact economic receipt,
  but does not yet embed the existing economic Position-transfer kernel: the
  selected trade's cash/Egg movement between the two Positions is not modeled.
- `verify_lease` authenticates a lease account against its own ledger's
  neutral sink, which is tautological in isolation. Both call sites first
  validate against the Epoch authority sink, so this is a latent trap for
  future callers rather than a live hole; the redundant self-check should be
  removed or given the real sink.
- The `FROZEN_EMPTY` validator does not pin `seen_competitive_ticks`,
  `competitive_admission_count`, or the admission transcript to zero. The
  state is unreachable through model transitions and not runtime-encodable
  (those fields live in the Window account, which must not exist in
  `FROZEN_EMPTY`), and pre-set ticks could only cause extra Replay refusals.

Run:

```sh
cargo test --manifest-path research/batch-policy-identity/Cargo.toml --locked --offline --all-targets
cargo clippy --manifest-path research/batch-policy-identity/Cargo.toml --locked --offline --all-targets -- -D warnings
```

## Required live evidence before promotion

1. Exact hostile codecs for every new version, phase, length, padding, payer,
   principal, donation, deadline, verifier, and receipt field.
2. Real-SBF blank and one-lamport prefund allocate/assign for Candidate, Window,
   WorkBudget, receipt, pot, and Reservation V2, proving the full payer deposit
   and neutral DonationLedger split.
3. Real-SBF multiple/tied Candidates, all 64 ticks, replacement,
   noncompetitive repetition, admitted replay, omitted/reordered top, corrupt
   score/digest, policy/grid/page/reservation/release-ID substitution, early/late
   work, reward replay, recipient substitution, later account donation, and
   rollback after writes/creates/closes.
4. CU and stack reports for maximum account shape of every staged route, with
   explicit headroom below the transaction cap.
5. Rent, donation, WorkBudget, cash, claim, fee, and reservation snapshots
   before/after success and every late failure.
6. Durable Epoch receipt decoding after transient authority is gone.
7. Real-SBF pre-Freeze abort/release for zero, one, and two OPEN reservations,
   including late failure rollback and exact recipient substitution refusal.
8. Real-SBF persistence of DonationLedger `observe` updates for every surviving
   mutable Candidate, Window, WorkBudget, and Reservation account, proving that
   an observed donation cannot later be drained while an older lower bound is
   persisted.

## Still out of scope

V3 does not add partial fills, fees, portfolios, more than two orders, more than
one Egg, general relation settlement, multiple pages, archive pruning, or an
operator policy. Existing outcome mints and older account versions do not gain
retroactive rent cleanup. The frozen tags and codecs are not live transition
claims: no lifecycle dispatch, deployment claim, or mainnet path exists until
the complete routed schema and real-SBF campaign land together.
