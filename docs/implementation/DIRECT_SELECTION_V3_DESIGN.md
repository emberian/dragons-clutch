# Direct Selection V3: staged, bounded, donation-safe authority

Status: **executable model; live ABI/runtime STOP**

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
`research/batch-policy-identity/src/direct_lifecycle_v3.rs`. It allocates no
live account tag or intent.

## Exact boundary

The V3 lifecycle begins atomically with a successful two-order direct Freeze.
At that boundary:

- the page and exact two Reservation V2 accounts are authenticated and frozen;
- both reservations are `ACTIVE`;
- the immutable schedule and verifier-semantics identity are fixed;
- the WorkBudget account is created and fully funded; and
- the lifecycle enters `FROZEN_EMPTY`.

WorkBudget must not exist before successful Freeze. A never-frozen OPEN Epoch
therefore cannot trap its work reserve. Promotion still requires a separate
`AbortUnfrozenDirectV4` route for the existing pre-Freeze Epoch and any zero,
one, or two OPEN-order reservations. That route is not modeled here and remains
an explicit migration STOP.

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
9. Each staged action authenticates one frozen verifier-semantics/build
   identity. This prevents accidental mixing across upgrades. A malicious
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
| 363 | 1 | terminal flags, zero |
| 364 | 8 | `selected_slot`, zero before selection |
| 372 | 32 | terminal Candidate ID |
| 404 | 32 | terminal relation-candidate digest |
| 436 | 8 | terminal quantity |
| 444 | 8 | terminal price |
| 452 | 16 | terminal consideration price-units |
| 468 | 8 | terminal slot |
| 476 | 32 | immutable neutral-lamport sink |
| 508 | 4 | canonical zero reserve |

Common Epoch phase gains explicit values for `FROZEN_EMPTY`, `WINDOW_OPEN`,
`VERIFYING`, `SELECTED`, and `TERMINAL`. Phase, not the placeholder receipt
bytes, distinguishes a nonterminal Epoch from `EMPTY_LAPSE`.

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

The Epoch-bound account stores Epoch, BatchPolicy, verifier identity, reward
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

### Reservation V2 — 490 bytes

Reservation V1 is 442 bytes (`MAX_OUTCOMES = 8` in the account-plane layout).
V2 adds rent payer (32), exact payer principal
(8), and neutral donation lower bound (8). Both exact Reservation V2 accounts
are authenticated by every transition. Their state path is typed:

```text
Finalize:       ACTIVE   -> ENTITLED
Settle:         ENTITLED -> CONSUMED
empty/pre lapse ACTIVE   -> RELEASED
post lapse:     ENTITLED -> RELEASED
```

Terminal routes close and refund both reservation rent principals exactly
once. Existing Reservation V1 accounts cannot promise this cleanup and stay
outside the V3 promotion claim.

### BatchPolicy/deployment identity

BatchPolicy must gain a full verifier-semantics/deployment-manifest identity.
Submit, Begin, Verify, Finalize, and Settle compare it to the Epoch/WorkBudget
binding; Lapse retains a versioned escape path. The exact BatchPolicy artifact
extension belongs in the codec migration, not an adapter projection.

## Routed transition plan

These are design names, not allocated intent tags.

### `SubmitDirectCandidateV3`

Preflight authenticates Epoch V4, BatchPolicy/deployment identity, Grid, frozen
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
and verifier identity. It runs the cached full verifier, changes only that
Candidate `VERIFIED -> REVERIFIED`, sets one mask bit, and pays one fixed Verify
reward. Replay, corrupt source, code substitution, or deadline failure is
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

The research crate passes 28 tests total, including 12 V3 lifecycle tests, and
strict Clippy. V3 tests cover:

- min/max deadline spans, boundary slots, and strictly positive work rewards;
- exact full-verifier/grid-issued Candidates and hostile tick/ID/score changes;
- noncompetitive no-state outcome, replacement, replay, and top/live split;
- hostile counts, masks, padding, duplicate identities, and status mismatch
  refusing before any unsafe index or shift;
- verifier identity checks across staged work;
- exact `ACTIVE -> ENTITLED -> CONSUMED/RELEASED` effects;
- partial-verification and selected-path WorkBudget equations;
- canonical DonationLedger create deltas, monotone donations, shortfall refusal,
  close-time neutral disposition, payer/reward separation, and checked aggregate
  overflow; and
- empty, pre-selection, post-selection, and settled durable receipts.

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
   score/digest, policy/grid/page/reservation/code substitution, early/late
   work, reward replay, recipient substitution, later account donation, and
   rollback after writes/creates/closes.
4. CU and stack reports for maximum account shape of every staged route, with
   explicit headroom below the transaction cap.
5. Rent, donation, WorkBudget, cash, claim, fee, and reservation snapshots
   before/after success and every late failure.
6. Durable Epoch receipt decoding after transient authority is gone.
7. Pre-Freeze abort/release for zero, one, and two OPEN reservations.

## Still out of scope

V3 does not add partial fills, fees, portfolios, more than two orders, more than
one Egg, general relation settlement, multiple pages, archive pruning, or an
operator policy. Existing outcome mints and older account versions do not gain
retroactive rent cleanup. No live intent, account tag, deployment claim, or
mainnet path exists until the complete versioned schema and real-SBF campaign
land together.
