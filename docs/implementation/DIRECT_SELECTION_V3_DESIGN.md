# Direct Selection V3: staged, bounded, donation-safe authority

Status: **routed lifecycle with model, host, and focused SBF-executed
evidence on `codex/r3-direct-v3-successor`; not merged, not deployed**

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
and intent codecs are frozen, and every intent in tags `36..=46` now routes to
a real handler and executes against the program ELF under
`solana-program-test`. That is branch evidence, not a release: see *Measured
SBF cost* and *Remaining promotion boundary* below.

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
real-bank evidence for each of those now exist on the successor branch.

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

### Direct Epoch V4 — 672 bytes

Neither the 512-byte nor 624-byte V4 Epoch draft was ever routed, emitted,
deployed, or accepted by the common request decoder. The version remains four
because this is a correction to an unexported proposed codec, not a migration
of live bytes; the exact length gate refuses both abandoned drafts.

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
| 508 | 32 | compile-time semantic verifier release ID |
| 540 | 32 | Epoch-bound DirectBatchPolicy V3 artifact ID |
| 572 | 32 | durable Epoch rent-principal payer |
| 604 | 8 | exact payer-funded Epoch rent principal |
| 612 | 8 | prior neutral donation lower bound |
| 620 | 32 | sole page-zero rent-principal payer |
| 652 | 8 | exact payer-funded page rent principal |
| 660 | 8 | page prior neutral donation lower bound |
| 668 | 4 | canonical zero reserve |

The two policy fields are deliberately disjoint. The preserved common Epoch
`policy` remains the legacy full-relation policy digest. The appended
`direct_policy_v3_id` must equal the digest of the exact canonical direct
policy bytes plus the appended `verifier_release_id` under this Epoch's
identity. Every later handler must reauthenticate that exact artifact account
and compare both persisted fields; call order is not authority.

Epoch creation uses the same DonationLedger ownership equation as transient
accounts even though the archive is durable. A predictable-PDA prebalance `B`
never discounts the authenticated payer's rent principal `P`: Init transfers
exactly `P`, proves the post-transfer balance is `B + P`, stores `P` as payer
principal, and stores `B` as neutral donation. Any later balance above those
compartments belongs only to the immutable sink.

The Epoch owns the page-zero funding ledger because this profile admits
exactly one page and the existing page codec has no versioned funding tail.
`page_count == 0` requires the page ledger to be all zero and no page account
to exist. `InitOrderPage` admits only page index zero/count one, transfers the
complete page principal `P` even when its predictable PDA already contains
`B`, proves `after == B + P`, and then atomically commits both
`page_count == 1` and the ledger. `page_count == 1` requires a nonzero valid
ledger, so replay refuses before any System CPI. Later V4 transitions observe
the durable Epoch and page balances and may only monotonically increase their
neutral-donation lower bounds.

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
of them. The settlement input is an exact buyer/seller Position pair in
economic-role order, independent of the two page slots' order. The checked
kernel maps the frozen buy/sell indices to the matching Reservations, requires
distinct owners and accounts, computes `quantity * price / price_scale` in
`u128` with exact divisibility and checked `u64` conversion, debits buyer cash,
credits seller cash, credits the bought Egg, and releases the buyer's complete
reserved-cash headroom. The exact-division refusal is the direct policy's own
`RoundingBoundaryV1::None` boundary — the relation refuses a remaindered
consideration (`RemainderRequired`) — so buyer cost and seller proceeds are the
same atom count with no rounding pot and no fee sink, and a zero price refuses.
Cash and per-outcome Egg conservation are both checked before either
Reservation becomes `CONSUMED`.

Consumed Reservation bodies archive the exact consumed amounts, never zeroes:
the buy's `remaining_cash_atoms` records the consideration actually spent and
the sell's `remaining_internal` records the filled quantity, so `initial_*`
minus `remaining_*` is the refunded portion in the durable effects. Any unspent
buyer envelope refunds implicitly through the reserved-cash release, and an
unfilled seller remainder refunds to the seller Position — unreachable through
this full-fill lifecycle, and covered at the kernel level. Deficits refuse
rather than clamp, and a tampered selected quantity refuses byte-identically
before any mutation. The archived `SETTLED` receipt stays bound to the frozen
page's quantity and outcome and to the exact-division rule after transient
authority is gone.

Before closing transient authority it writes the `SETTLED` receipt into Epoch
V4. It then closes the selected Candidate, Window, receipt, pot, WorkBudget,
and both reservations, pays Settle only from WorkBudget, returns unused rewards
to the sponsor, returns each principal to its payer, and sends all donations
only to the neutral sink. Positions survive with the exact checked poststates.

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

- One 672-byte Epoch archive remains per historical epoch. Its principal comes
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

At commit `0ccfa18` the research crate passes 40 tests total, including 24 V3
lifecycle tests, and strict Clippy. The exact archive was independently run in
release mode on `hbox` from
`/tank/dregg-build/dragons-clutch-r3-direct-0ccfa18`; all 40 tests passed,
including the exhaustive five-candidate arrival permutations and all 62
relation-admissible interior ticks of the 64-tick replay domain. The local and
remote archive SHA-256 is
`9805f897eded9515acf9a871cf7af895dff1c12e58b67393bb8e359a457b5f4e`;
the captured test log SHA-256 is
`35d3bc51e0f4b32529faee4cbc02931769ee2447aa8425d0c0f2e4969d7dd861`.
The host was Linux `6.11.0-29-generic` x86-64 with Cargo
`1.100.0-nightly (8a0d8afba 2026-08-15)` and rustc
`1.100.0-nightly (e71c0f1e3 2026-08-18)`. GPU was not used. V3 tests cover:

- min/max deadline spans, boundary slots, and strictly positive work rewards;
- exact full-verifier/grid-issued Candidates and hostile tick/ID/score changes;
- noncompetitive no-state outcome, replacement, replay, and top/live split;
- hostile counts, masks, padding, duplicate identities, and status mismatch
  refusing before any unsafe index or shift;
- verifier release-ID checks across staged work;
- exact `ACTIVE -> ENTITLED -> CONSUMED/RELEASED` effects;
- exact buyer/seller Position settlement independent of page orientation,
  including cash/Egg transfer, reserved-cash release, consideration
  divisibility, alias/substitution refusal, and atomic overflow refusal;
- hard-anchored two-sided settlement legs, consumed-amount (never zeroed)
  Reservation archival, kernel-level partial-fill remainder refunds on both
  sides, tampered-quantity refusal with byte-identical rollback, and mixed
  settle/lapse terminal conservation of cash, Eggs, and lamports;
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
  receipts;
- independent Candidate reconstruction from the frozen page's quantity,
  buy/sell indices, outcome, limits, and immutable schedule rather than the
  persisted Candidate's claims, plus explicit neutral-sink binding; and
- `FROZEN_EMPTY` refusal of every ghost admission bitmap/count/transcript and
  every impossible pre-admission work-payment history.

The layout crate additionally carries two cross-crate tripwires: the model's
recomputed frozen-page digest and order-set fold are asserted byte-identical
to the live page fold over the same two records, and the model's
`direct_policy_v3_digest` is asserted byte-identical to the codec's
`digest_for_epoch`.

## Routed runtime status

The whole family is now routed and executes in real SBF. `dispatch.rs` sends
every tag `36..=46` to one `Route::DirectSelectionV3` arm that decodes only
through the dedicated `DirectV3Request` envelope and calls an **exhaustive**
handler match with no fallback and no `NotYetImplemented` arm, so a new
lifecycle intent cannot compile without a handler. The legacy `Request`
decoder still refuses every tag in `36..=46`, and the V3 decoder refuses every
legacy tag, so no partially added tag can fall into a handler with different
account versions.

The `InitOrderPage` and `PlaceOrder` V4 branches left `cfg(test)` in the same
commit series. Each is selected only by the otherwise-unreachable 672-byte V4
Epoch schema, which can exist only through the routed `InitDirectEpochV4`, and
the legacy eight-account placement and six-account page ABIs stay byte- and
behavior-stable. That is the predecessor's exact failure mode, now covered by
two live regressions: a legacy-shaped `PlaceOrder` refuses against V4 state on
account count before any mutation, and a V4-shaped placement refuses against a
legacy 344-byte Direct Epoch V3 account.

Init's exact nine-account order is:

| index | role | access |
|---:|---|---|
| 0 | archive/prefund payer | signer, writable |
| 1 | canonical Epoch V4 target | writable, System-owned empty |
| 2 | Market | program-owned read-only |
| 3 | Terms | program-owned read-only |
| 4 | PriceGrid | program-owned read-only |
| 5 | 96-byte DirectBatchPolicy V3 artifact | program-owned read-only |
| 6 | System program | executable read-only |
| 7 | Rent sysvar | read-only |
| 8 | Clock sysvar | read-only |

The runtime release label is
`dragons-clutch/direct-verifier-release/v3/1`; its fixed SHA-256 identifier is
`038914d7913057589fa6bf303f02a6b9b5e12a1ee718561079ab57d375947704`.
This is a semantic release identifier, not an ELF, ProgramData, deployment, or
source hash. The exact DirectBatchPolicy artifact and its epoch-bound digest
must carry that identifier.

The current Realm account has no neutral-lamport-sink field. Consequently the
first runtime profile specializes the model's immutable sink to Solana's
canonical incinerator and requires the intent field to equal that address; a
creator-selected donation beneficiary refuses. A future Realm version may own
a different immutable neutral sink, but inferring one from an arbitrary caller
is not a migration.

The new Window, Candidate, WorkBudget, receipt, and pot PDAs use five disjoint
V3 seed namespaces. Epoch and Reservation keep their existing semantic PDA
coordinates and are separated by exact version/length.

The page seam retains the existing six-account shape and the funded-placement
seam has exactly nine accounts: the legacy eight in their unchanged order,
with the exact epoch-bound 96-byte DirectBatchPolicy artifact appended
read-only. They admit `PREFREEZE_OPEN`, page zero of one, at most two
single-Egg orders, zero fees/minimum-fill/flags, an exact grid tick, and a
618-byte Reservation V2 with its own payer/principal/donation ledger. A
zero-envelope placement — a zero-limit buy under the profile's forced zero fee
— refuses at creation, because its release would be the Position no-op the
release kernel's unchanged-poststate rule refuses forever. The prior attempt
to length-select these seams from live legacy wires was rejected because an
injected program-owned V4 prestate could create a Reservation V2 dead end
before Freeze, Abort, and lapse were all available atomically; they are live
now only because the complete lifecycle lands with them. Legacy
`InitOrderPage`, eight-account placement, and Reservation V1 production bytes
retain their prior behavior exactly.

## Measured SBF cost

Every number below is `compute_units_consumed` from the routed campaign in
`programs/clutch-sbf/svm-tests/tests/direct_selection_v3.rs`, driving the real
program ELF under `solana-program-test`. The transaction ceiling is
1,400,000 CU. **No lifecycle instruction reached the ceiling: there is no
measured CU STOP in this cut.**

| instruction | measured CU | headroom |
|---|---:|---:|
| `InitDirectEpochV4` | 680,723 | 51.4% |
| `InitOrderPage` (V4 branch) | 407,028 | 70.9% |
| `PlaceOrder` (V4 branch, buy) | 784,232 | 44.0% |
| `PlaceOrder` (V4 branch, sell) | 780,758 | 44.2% |
| `FreezeDirectEpochV4` | 1,018,901 | 27.2% |
| `AbortUnfrozenDirectV4`, zero reservations | 161,507 | 88.5% |
| `AbortUnfrozenDirectV4`, one reservation | 371,980 | 73.4% |
| `AbortUnfrozenDirectV4`, two reservations | 462,166 | 67.0% |
| `SubmitDirectCandidateV3`, first admission | 904,313 | 35.4% |
| `SubmitDirectCandidateV3`, retained | 991,101 | 29.2% |
| `SubmitDirectCandidateV3`, retained (full top) | 1,091,329 | 22.0% |
| `SubmitDirectCandidateV3`, **replacement (worst)** | **1,123,392** | **19.8%** |
| `SubmitDirectCandidateV3`, noncompetitive no-state | 959,561 | 31.5% |
| `BeginDirectVerificationV3` | 174,667 | 87.5% |
| `VerifyDirectCandidateV3` (worst of three) | 607,601 | 56.6% |
| `FinalizeDirectSelectionV3` (three retained) | 654,731 | 53.2% |
| `SettleDirectV3` (incl. all seven closes) | 454,375 | 67.5% |
| `LapseEmptyDirectV3` | 469,018 | 66.5% |
| `LapseUnselectedDirectV3` | 507,294 | 63.8% |
| `LapseSelectedDirectV3` | 516,557 | 63.1% |

The staged design is what buys this: the V2 `SelectDirectWindowV1` reached the
cap re-executing three Candidates in one transaction, while V3's Begin plus
three Verifies plus Finalize each stay under 660,000 CU. `SubmitDirectCandidateV3`
at 1,123,392 CU is the tightest row at just under 20% headroom; it is the shape that runs
the full verifier, decodes three retained Candidates, and closes the displaced
worst, and it is the row to watch if any later change adds work to submission.

Two measured cost corrections landed with the campaign. Deferring direct
lamport moves past every create CPI was required for correctness, not cost:
the runtime syncs only the accounts passed to a callee at CPI entry, so an
earlier direct move on a caller-only account desynchronizes the
instruction-wide lamport sum and the transaction refuses as unbalanced.
Single-site epoch validation in the two V4 branches cut `PlaceOrder` from
1,249,641 to 784,232 CU and `InitOrderPage` from 641,047 to 407,028 CU by not
re-running the decode-time hostile validation (including two software-SHA
policy digests) up to four more times on the same immutable bytes; no refusal
was removed, and the host substitution suite still refuses every
policy/schedule/page/replay/release/funding mutation.

`cargo-build-sbf` reports zero stack-frame errors for every `clutch_sbf`
function. The frames that remain in its output are the pre-existing
research/reference-crate debt already present in the sealed-main baseline ELF.

## Remaining promotion boundary

The three former model blockers are closed by `e77238f`, `6267fde`, and
`081bd81`: Settle now owns the actual Position transfer, verification derives
all economic facts from frozen authority and takes the real neutral sink, and
`FROZEN_EMPTY` pins admission/work history to zero. The live adapter, the
routed dispatch, and the real-SBF campaign have now landed as well. What
remains before this branch is promotable:

1. **No per-order V4 cancellation exists.** The legacy `CancelOrder` epoch
   role admits only the legacy lengths, so a 672-byte V4 Epoch refuses on data
   length (asserted live). A V4 order can therefore be retired only by
   aborting the whole unfrozen Epoch. That is a bounded, refusal-shaped gap,
   not a dead end, but it is a product gap and must be named as one.
2. **The campaign is one bank profile, not the whole hostile surface.** It
   drives five candidates over an 11-tick grid; the codec's 64-tick replay
   domain, tied scores, and omitted/reordered retained accounts are covered in
   the executable model and the host suite, not yet in real SBF.
3. **Upgrade authority remains the trust boundary.** `verifier_release_id` is
   a compile-time semantic label, not an onchain code or deployment hash; a
   malicious upgrade authority can replace the checking program. Promotion
   needs an immutable deployment or an explicit accepted-authority statement.
4. **Reward amounts are unpriced.** The campaign's frozen rewards are
   arbitrary positive lamports. Real values need a policy pass over these
   measured CU numbers.
5. Existing Reservation V1 accounts stay outside the V3 promotion claim, and
   the durable Epoch archive still grows linearly with history.

Run:

```sh
cargo test --manifest-path research/batch-policy-identity/Cargo.toml --locked --offline --all-targets
cargo clippy --manifest-path research/batch-policy-identity/Cargo.toml --locked --offline --all-targets -- -D warnings
cargo test --manifest-path programs/clutch-sbf/Cargo.toml -p clutch-sbf --locked --offline
programs/clutch-sbf/svm-tests/run_svm_tests.sh direct_selection_v3
```

## Required live evidence before promotion

Each item records what the routed campaign now covers. **Covered** means a
real-SBF assertion exists in `direct_selection_v3.rs`; **partial** names the
exact remainder.

1. **Covered (codec/host).** Exact hostile codecs for every new version, phase,
   length, padding, payer, principal, donation, deadline, verifier, and receipt
   field, in the layout crate's suite.
2. **Covered.** Every predictable PDA in the campaign starts as a one-lamport
   System account; Epoch, page, Candidate, Window, WorkBudget, receipt, pot,
   and both Reservation V2 accounts are created over that prefund, and each
   asserts `balance == rent_exempt + 1` with the prefund recorded as neutral
   donation and never as payer principal.
3. **Partial.** Covered live: replacement, the noncompetitive no-state
   outcome, admitted-tick replay (both a retained tick and a displaced tick),
   post-close submission, wrong-policy substitution, recipient substitution at
   Finalize and at Abort, early/late staged work, verification replay, wrong
   retained index, and rollback after staged writes. Not yet live: all 64
   ticks, exact score ties, and omitted/reordered retained accounts — those
   remain model and host evidence.
4. **Covered.** See *Measured SBF cost*: every routed instruction with its
   worst measured account shape, explicit headroom, and zero `clutch_sbf`
   stack-frame errors.
5. **Covered.** The campaign asserts exact per-keypair lamport conservation at
   terminal (keeper earns exactly the frozen rewards, sponsor loses exactly
   those rewards, submitter and both owners recover every principal exactly),
   exact Position cash/Egg poststates, and byte-identical account snapshots
   across every late failure.
6. **Covered.** The durable Epoch V4 receipt is decoded after Settle and after
   each lapse, with every transient account asserted closed.
7. **Covered.** Zero-, one-, and two-reservation pre-freeze aborts each run,
   with recipient substitution refusing first and both Positions restored
   exactly.
8. **Covered.** An unsolicited transfer to the live Window is observed and
   persisted as a monotone lower bound by the next mutating transition, and
   the noncompetitive no-state path refuses while that observation is pending
   because nothing may persist it.

## Still out of scope

V3 does not add partial fills, fees, portfolios, more than two orders, more than
one Egg, general relation settlement, multiple pages, archive pruning, or an
operator policy. Existing outcome mints and older account versions do not gain
retroactive rent cleanup. The frozen tags and codecs are not live transition
claims: no lifecycle dispatch, deployment claim, or mainnet path exists until
the complete routed schema and real-SBF campaign land together.
