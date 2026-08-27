# Direct Selection V2: bounded authority-chain design

Status: **EXECUTABLE OFFLINE ACCOUNT/TRANSITION MODEL; LIVE SBF STOPPED ON
IMMUTABLE WINDOW SCHEDULE AND SHARED-ABI INTEGRATION**

Executable model:
[`research/batch-policy-identity/src/direct_window_v1.rs`](../../research/batch-policy-identity/src/direct_window_v1.rs)

Predecessors:

- `065f02a` — canonical 64-byte BatchPolicy and full-width relation identity;
- `1835b79` — live deterministic `SUBMITTED` direct proposal;
- `f529460` — live consumption of an already-authorized direct receipt; and
- `5e1edb1` — executable proof that submission is not selection.

This document specifies the smallest authority chain which could connect those
pieces without truncating a `Hash32`, manufacturing a policy, or inferring a
closed candidate set from missing accounts. It is intentionally not a general
batch-clearing design.

## 1. Bounded economic profile

The V2 specialization admits exactly:

- one frozen page containing exactly two live records and no tombstones;
- two distinct owners;
- two single-Egg orders on the same outcome and opposite sides;
- equal nonzero quantities, no AON flag, and zero minimum fill;
- a buy limit greater than or equal to the sell limit;
- one submitted outcome price which is an exact frozen-grid tick inside that
  closed interval, whose complement is also an exact grid tick;
- two full fills and one direct pairing slice;
- exact `quantity * price / price_scale` divisibility;
- zero virtual split, zero virtual merge, and zero fees; and
- reservation envelopes which are still exact untouched `ACTIVE` products of
  those two frozen orders.

The compact projection accepts one complete authenticated policy, byte for
byte:

```text
allocation          = PricePriorityMarginalProRata
self_cross           = AllowGateAtPairing
aon                  = RefuseAdmission
rounding             = None
residual_settlement  = UniqueSliceReceipts
transfer_phase       = ActiveOrResolved
portfolio_lots       = StrictWholeOrder
pairing_witness      = RecomputedConstructor
dust                 = Reject
score                = LexicographicDispersionV1
fee_base             = None
```

This value is `DIRECT_POLICY_V1` in the model. It is not an adapter default.
The final BatchPolicy artifact must decode canonically, its full digest must
equal `Epoch.policy`, and every different selector or parameter refuses before
projection. The model tests mutations across every policy family it does not
represent.

Fee-bearing, rounded, partial, portfolio, virtual, cumulative-residual,
Active-only, and explicit-witness candidates require additional state. They
are not variants hidden behind this account.

## 2. New hard prerequisite: immutable schedule

The existing Epoch account freezes economic identities and book dimensions,
but carries no candidate-window opening or closing slot. The existing 64-byte
BatchPolicy also carries no time rule. Therefore neither current artifact can
answer:

```text
when may the first submission land?
which slot is the first slot in which submission must refuse?
```

The first submitter cannot answer either question. A fixed duration beginning
at first submission is still submitter-controlled timing and is forbidden.

The current Epoch account is already tag 11, version 2, and exactly 328 bytes.
The minimum live prerequisite is a distinct tag-11/version-3 direct Epoch
schema carrying:

```text
submission_opens_slot:  u64
submission_closes_slot: u64  // opens < closes
```

Both values must be fixed before any candidate exists and authenticated by the
program-owned Epoch account. The canonical Epoch identity remains derived from
`(market, epoch_index)` so existing page and reservation identities do not
change; the version and exact account length prevent a V2 body from being read
as a V3 body. An alternative BatchPolicy-V2 duration rule would also need an
immutable Epoch freeze slot from which to derive both values; the current
schema has neither fact. Until one of those constructions lands, the live
selection instruction must not exist.

The offline `DirectWindowBindingV1` makes this dependency explicit. Its
constructor accepts already-frozen boundaries; the first submission can create
the Window PDA but cannot alter either boundary.

The half-open rule is exact:

```text
submission allowed: opens_slot <= Clock.slot < closes_slot
selection allowed:  closes_slot <= Clock.slot
```

Selection has no upper deadline in this slice. A late selection is safe and
needed for permissionless liveness; an early selection refuses. Lapse and a
no-valid-candidate window remain outside this profile because the Window is
created atomically with its first valid candidate.

## 3. Why a streaming top three closes semantic selection

A fixed-capacity array must not mean “the first three candidates win access.”
That would let three valid but poor submissions crowd out a better later one.

V2 instead retains the best three candidates seen so far under the frozen
total score order. Every fresh valid submission does all of the following in
one transition:

1. verifies the complete direct relation under the exact policy and domain;
2. increments `admitted_count`;
3. advances an ordered full-width admission transcript;
4. compares the new full score with the exact retained candidate accounts;
5. inserts it into canonical best-to-worst order if the retained set is not
   full;
6. once full, replaces the worst only if it is strictly better; and
7. marks either the displaced candidate or the new non-retained candidate
   `SUPERSEDED`.

The invariant is the ordinary streaming top-k invariant: after every prefix of
the submission sequence, `top` is exactly the best `min(3, admitted_count)`
candidates in that prefix. Consequently `top[0]` is the best valid submitted
candidate over the complete admitted prefix at close, independent of arrival
order. The model executes both arrival orders of a real two-candidate tie and
obtains the same retained ordering and winner.

The ordered admission transcript is:

```text
SHA256("dragons-clutch/direct-admission-transcript/v1\0"
       || epoch || order_set || policy || relation_domain_digest
       || opens_slot || closes_slot || next_admitted_count
       || previous_transcript
       || candidate_id || relation_candidate_digest)
```

It records every successful admission, including candidates immediately
superseded outside the retained top three. It is deliberately order-sensitive
audit evidence, not mislabeled as a commutative set commitment.

Semantic closure does not come from checking that no other PDA exists. It comes
from one Window account being the required single writer for every successful
candidate admission, the immutable Clock boundary closing that writer, and the
streaming top-k invariant. A candidate content PDA proves freshness of one
candidate only; its creation and Window update must be atomic.

This closes semantic selection under the program and ledger assumptions. It
does not solve transaction censorship, front-end omission, or liveness. Those
are separate operational questions and must not be disguised as a candidate
hash property.

## 4. Candidate V2 fixed body

The model owns a 438-byte canonical body. A future layout allocation prepends
its independent two-byte account tag/version, for exactly 440 account bytes.
No live discriminator is allocated by the research crate.

| bytes | field |
| ---: | --- |
| 224 | seven full identities: candidate, Epoch, Market, order set, policy, relation-domain digest, relation-candidate digest |
| 128 | sixteen `u64` simplex prices; indices `2..16` zero |
| 16 | two exact `u64` fills |
| 16 | dispersion-weighted direct volume (`i128`) |
| 16 | exact limit surplus (`u128`) |
| 8 | authenticated submitted slot |
| 8 | one direct quantity; equals both fills |
| 3 | buy index, sell index, outcome |
| 2 | distinct owners; exactly two |
| 5 | order length, outcome count, status, stored bump, flags |
| 12 | zero reserved bytes |

The relation-candidate digest is also the final score coordinate; it is stored
once. Churn is structurally zero and is not persisted. Virtual quantities,
honored-AON mask, fees, remainder, and portfolio coefficients cannot be
encoded. The existing account-plane candidate identity is recomputed exactly
from Epoch, Market, the complete price vector, and the fixed zero virtual/AON
coordinates.

States are:

```text
VERIFIED -> SELECTED
VERIFIED -> SUPERSEDED
```

There is no `SUBMITTED` state in this record. Relation verification and account
creation are one instruction. The old V1 `SUBMITTED` record remains a separate
research/demo path and is never silently upgraded.

## 5. Window V1 fixed body

The model owns a 454-byte canonical body. With a future two-byte layout
tag/version envelope the account is exactly 456 bytes.

| bytes | field |
| ---: | --- |
| 224 | Epoch, Market, order set, policy, relation-domain digest, admission transcript, selected candidate |
| 192 | three retained `(candidate_id, relation_candidate_digest)` pairs |
| 24 | immutable open slot, immutable close slot, selected slot |
| 8 | total admitted count |
| 4 | retained count, phase, stored bump, flags |
| 2 | zero reserved bytes |

The active retained prefix is dense and every unused pair is all zero. Retained
candidate ids are distinct. While `OPEN`, selected identity and slot are zero.
While `SELECTED`, the selected id equals `top[0]` and the selected slot is at or
after close.

Window-local decoding cannot prove score ordering because scores have one
semantic owner in Candidate V2. Every mutation and final selection therefore
requires the exact retained Candidate accounts in registry order and checks
their bodies, bindings, `VERIFIED` states, and strict best-to-worst order.

## 6. Proposed live PDA namespaces

These strings are design inputs, not allocated live constants yet:

```text
BatchPolicy final artifact: existing typed-artifact final namespace,
                            context = Epoch, digest = policy digest
Direct window:              "direct-window:v1"    || Epoch
Direct candidate:           "direct-candidate:v2" || Epoch || candidate_id
Direct receipt:             "direct-receipt:v2"   || Epoch || candidate_id || 0
Direct pot:                 "direct-pot:v2"       || Epoch || candidate_id
```

The receipt and pot use new namespaces because old V1 Candidate content can
have the same free-coordinate identity. Reusing the old namespace would make
the two authority versions contend for one PDA.

All creates must use the established prefund-safe state machine:

- zero-lamport/zero-data target: signed CreateAccount;
- prefunded System-owned zero-data target: signed Allocate then Assign;
- already program-owned or nonzero-data target: refuse;
- rent and exact target length checked before the first write; and
- a late create/encode/CPI refusal rolls back every account in the instruction.

### 6.1 Exact Epoch migration and construction

The legacy Epoch schema remains tag 11/version 2/328 bytes. It is not rewritten
or treated as a prefix. `DirectEpochV3Account` uses tag 11/version 3/344 bytes:
the same field order through `phase`, followed by
`submission_opens_slot: u64`, `submission_closes_slot: u64`, and then the
existing bump and flags. Its decoder accepts only that version and length.

Common read-only placement, cancellation, and page-initialization code may use
an explicit version dispatch which projects either schema into common facts.
Old `SubmitDirectPage` and `SettlePage` continue to decode V2 specifically.
Every instruction in this document decodes V3 specifically. Consequently an
old Epoch cannot acquire V2 selection authority, and a V3 Epoch cannot enter
the old candidate path. The shared Epoch PDA still derives from market and
index, so both schemas cannot exist for one epoch coordinate.

Construction needs two additional transitions before candidate submission:

1. `InitDirectEpochV3` authenticates active Market, Terms, PriceGrid, and the
   final exact BatchPolicy artifact; derives the Epoch and a domain-separated
   direct-book identity; fixes relation V1, owner count two, remainder seed
   zero, and the Terms/Grid shape; requires a bounded future half-open schedule
   with a minimum lead from the authenticated Clock; and creates one OPEN
   344-byte Epoch prefund-safely. The schedule is explicit creation state, not
   a Candidate-submission parameter, and the minimum lead forbids creation and
   admission in one slot.
2. `FreezeDirectEpochV3`, before `opens_slot`, authenticates the exact open
   two-order page, frozen grid, full policy, and both untouched ACTIVE
   reservations. It computes the one-page order-set commitment with the
   existing streaming helper, seals the page once, and writes the Epoch's
   order set, range, counts, and `FROZEN` phase while preserving both schedule
   fields byte-for-byte. Complete preflight precedes both writes.

The BatchPolicy artifact uses kind 4, exact length 64, context equal to the
already-derivable Epoch id, and a final PDA namespaced separately from the
collateral-policy PDA. The canonical policy codec remains one semantic owner;
the adapter cannot recreate selector mappings.

The Market has no epoch-creation authority field. A permissionless caller can
therefore race to initialize a canonical `(market, index)` under the existing
namespace model. This plan makes the result immutable and prevents a
same-slot first submission, but does not pretend that it supplies governance.
If creator authorization is required, the next prerequisite is an immutable
Market/Realm epoch-creation policy, not an ad hoc adapter signer.

## 7. Proposed instruction ABI

Numeric Intent tags are intentionally unassigned while another lane owns the
shared dispatch/layout range.

### `SubmitDirectCandidateV2`

Wire coordinates:

```text
market: Hash32, epoch: Hash32, page_index: u16, outcome_price: u64
```

Accounts, exactly `12 + retained_count`:

```text
0  payer                 writable signer
1  DirectEpoch V3        read-only
2  final BatchPolicy     read-only
3  frozen PriceGrid      read-only
4  frozen OrderPage      read-only
5  reservation slot 0    read-only
6  reservation slot 1    read-only
7  DirectWindow          writable, creatable only on first admission
8  new DirectCandidate   writable, creatable
9..9+n current retained Candidate V2 accounts, writable, registry order
9+n    System program    read-only
10+n   Rent sysvar       read-only
11+n   Clock sysvar      read-only
```

Retained accounts are writable because a replacement may atomically mark the
old worst `SUPERSEDED`. On first admission `n = 0`; Window and Candidate are
created together after the immutable schedule is checked. On every later
admission `n` must equal `window.top_count`. Missing, extra, reordered, or
substituted retained accounts refuse.

### `SelectDirectWindowV1`

Wire coordinates:

```text
market: Hash32, epoch: Hash32, page_index: u16
```

Accounts, exactly `13 + retained_count`:

```text
0  payer                 writable signer
1  DirectEpoch V3        writable
2  final BatchPolicy     read-only
3  frozen PriceGrid      read-only
4  frozen OrderPage      read-only
5  reservation slot 0    writable
6  reservation slot 1    writable
7  DirectWindow          writable
8..8+n retained Candidate V2 accounts, writable, registry order
8+n    DirectReceipt V2  writable, creatable
9+n    DirectPot V2      writable, creatable
10+n   System program    read-only
11+n   Rent sysvar       read-only
12+n   Clock sysvar      read-only
```

After complete preflight and re-verification, selection performs one atomic
transition:

```text
Window OPEN -> SELECTED
best Candidate VERIFIED -> SELECTED
other retained Candidates VERIFIED -> SUPERSEDED
reservation 0 ACTIVE -> ENTITLED, amounts unchanged
reservation 1 ACTIVE -> ENTITLED, amounts unchanged
receipt absent -> exact unconsumed direct receipt
pot absent -> CLOSED zero pot
Epoch FROZEN -> CLEARED
```

The Epoch must not become `CLEARED` before both entitlements and the complete
receipt/pot set exist. The zero pot is still material authority: its immutable
closed-zero bytes prove this selected candidate has no virtual, fee, or
rounding balance.

### `SettleDirectV2`

Wire coordinates:

```text
market: Hash32, epoch: Hash32, page_index: u16
```

Accounts, exactly 10:

```text
0 DirectEpoch V3        read-only, CLEARED
1 DirectWindow          read-only, SELECTED
2 selected Candidate V2 read-only, SELECTED
3 frozen OrderPage      read-only evidence
4 buyer Position        writable
5 seller Position       writable
6 buyer reservation     writable, ENTITLED
7 seller reservation    writable, ENTITLED
8 DirectReceipt V2      writable, unconsumed
9 DirectPot V2          read-only, CLOSED and zero
```

This is a new consumer. The old `SettlePage` continues to expect its V1
Candidate/Feed/receipt authority and must not be weakened.

The page authenticates the two order definitions but never authorizes value.
Value authority is the selected Window, selected full-width Candidate,
once-frozen receipt, CLOSED-zero pot, and exact `ENTITLED` reservations.
Successful settlement transfers the exact Egg quantity and exact collateral
atoms, consumes both reservations, and exhausts the receipt. Replay refuses.

## 8. Selection order and tie evidence

The score order is unchanged:

1. maximize dispersion-weighted direct volume;
2. maximize limit surplus;
3. maximize distinct owners;
4. minimize churn; and
5. prefer the lexicographically smaller complete 32-byte relation-candidate
   digest.

The executable model uses a crossed book with buy limit 7,500, sell limit
2,500, and equal quantity four. Candidate prices 2,500 and 7,500 both fully
fill. Their first four score coordinates tie by symmetry; the complete SHA-256
digest selects the winner. Submitting them in reverse order retains and selects
the same winner. A separate vector holds the first 16 digest bytes equal and
changes byte 20, proving the comparator reads beyond a 128-bit prefix.

The correct public claim is “best valid submitted candidate admitted before
the immutable close boundary,” not “optimal clearing.” No optimality
certificate exists.

## 9. Executed offline evidence

The crate command is:

```sh
cargo test --manifest-path research/batch-policy-identity/Cargo.toml \
  --locked --offline --all-targets
```

At this draft the full crate passes 15 tests. The six window tests cover:

- exact 438/454-byte body round trips and reserved-byte refusal;
- immutable early/late timing behavior;
- retained-candidate replay refusal;
- admission count and transcript advancement past capacity;
- immediate supersession of a worse fourth candidate;
- displacement of the old worst by a better later candidate;
- all 120 arrival permutations of five candidates retaining the same exact
  top three and winner;
- late selection success, early selection refusal, and selection replay;
- the real crossed-book relation tie and order-independent winner;
- full-digest comparison beyond 128 bits;
- registry omission and policy substitution refusal; and
- fail-closed projection of every unrepresented policy family.

This is host model evidence. It is not SBF execution, a proof-assistant
theorem, a release, or deployment evidence.

## 10. Required real-SBF campaign after schema integration

No live claim is warranted until real SBF proves all of the following:

1. BatchPolicy artifact construction through both blank and one-lamport
   prefunded targets;
2. first Candidate+Window construction through blank and prefunded targets;
3. immutable Epoch schedule substitution refusal;
4. two-candidate crossed-book tie and the full-width winning digest;
5. a worse fourth candidate cannot crowd the retained set;
6. a later better candidate displaces the exact worst account;
7. missing, extra, reordered, stale, or substituted retained accounts refuse;
8. early select refuses and late select succeeds;
9. late submission and all replay paths refuse;
10. policy artifact address, owner, length, bytes, and digest substitutions
    refuse;
11. receipt/pot prefund safety;
12. a late pot-creation failure rolls back receipt creation, Window/Candidate
    statuses, reservation entitlements, and Epoch phase;
13. successful selection makes only the named state changes;
14. successful V2 settlement conserves cash and the Egg claim exactly; and
15. double settlement refuses with every account byte unchanged.

Each success/refusal path needs measured compute units, the exact ELF digest,
and byte snapshots for rollback. Host-only emulation is insufficient.

## 11. Remaining explicit STOPs

Even after this bounded chain is live, the following remain unimplemented:

- multi-page and tombstoned books;
- more than two orders or outcomes;
- partial and multi-slice fills;
- all portfolio settlement;
- virtual split/merge pots;
- fees and fee recipients;
- every policy other than `DIRECT_POLICY_V1`;
- explicit pairing witnesses;
- cumulative residual families;
- zero-candidate lapse and reservation refunds;
- general receipt/pot set closure;
- Epoch `SETTLED` transition and terminal sweep; and
- censorship-resistant submission or selection liveness.

The bounded direct pot is born `CLOSED` and zero, and its one receipt and two
reservations can be consumed. That does not imply general epoch-terminal
closure.
