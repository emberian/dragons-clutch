# Decision 0010: General's candidate half, its escrow, and a program set that grows without opening

Status: accepted on 2026-08-27 as the resolution of items 1-5 of
`docs/decisions/0009-general-batch-collection.md` §6, plus ADR-0006 §8 item 7.
This record is written against work that LANDED with it — commits `39c12d82`
(escrow), `6f654f94` (the EffectV4 envelope), `5987febc` (the candidate verbs)
and `211079f6` (the set relaxation). It is not release evidence: the collection
and candidate actions are protocol selectors with authenticated pure
transitions and **no artifact triple**, and §6 says exactly what that means.

## Context

Decision 0009 closed `M-12` — General's collection half had no records and no
producer for `batch_id` or `AuthenticatedOrderTermsV2` — and left five things
open. Two of them were not gaps in coverage but gaps in the family's central
claim:

- **Nothing submitted or verified a candidate.**
  `evaluate_runtime_consider_row_with_manifest_v2` is 1,987 lines of streamed
  verification with no caller outside tests, and `Consider` reads a
  `SubmittedVerifiedCandidate` that no action writes. "A candidate is verified
  by the protocol rather than asserted by whoever submits it" had never been
  executed as a protocol step in either generation of this tree.
- **`Collect` debited the maker's own external account at settlement time.**
  Between placement and settlement the collateral sat where the maker could
  spend it, and a maker who did stranded the whole candidate, not just their
  own row.

A third arrived from outside the family while this record was being written,
and it sits ahead of both — see §5.

## 1. The verb verdicts

Gen-2's candidate half is the design authority, recovered from the monolith's
`general.rs` at `dd1ec033` and from `dclutch-general-contract`. It had **nine**
verbs, not the four decision 0009 §6 named. Each is carried, folded, or dropped
with an argument. Nothing is dropped silently.

| gen-2 verb | tag | verdict | why |
|---|---|---|---|
| `SubmitCandidate` | 7 | **CARRIED** | `GeneralCandidateV1::submit`. The record it creates is the account `Consider` reads and nothing wrote. |
| `CreateCandidatePage` | 22 | **FOLDED into submission** | A page is an immutable content-addressed record bound to `(candidate_id, coordinate, revision)`, and its address is its replay guard. A verb whose only effect is to create an immutable record the submission already pins is a transaction, not a decision. |
| `VerifyCandidatePage` | 8 | **CARRIED, at row granularity** | `verify_candidate_row_v1`. Gen-3 streams a ROW at a time because the runtime width reaches 258 outcomes and a whole page does not fit a compute budget there. The verb is the same; the step is smaller, and the escrow is sized per row rather than per page. |
| `FinishCandidate` | 9 | **FOLDED into the terminal row** | The evaluator writes the certificate on the step that consumes the last row of the last page. A separate finish verb would be a second place deciding when verification is complete, and the two could disagree. `verify_candidate_row_v1` asserts they do not: it derives the terminal step from the page geometry and requires the evaluator's own `complete` flag to agree, then requires the revision to equal the declared row count. |
| `ConsiderCandidate` | 10 | **ALREADY EXISTS, now FUNDED** | `consider_verified_candidate_v2` was always able to read a certificate and had never had one written for it. The verb is unchanged; what is new is that performing it is paid — see below. |
| `CloseCandidate` | 18 | **CARRIED as `close_out`** | Pays the cleanup crank and refunds the unspent verification compartment to the solver. |
| `ClosePage` | 23 | **FOLDED into `close_out`** | Gen-3's pages are per-candidate immutable records with no independent lifecycle; closing them is part of closing the candidate that owns them. |
| `RejectCandidate` | 24 | **DROPPED, with an argument** | Gen-2 needed an explicit reject because a candidate could sit in a non-terminal state indefinitely. Gen-3 has no rejected state: a row that does not verify does not advance the cursor, so an invalid candidate simply never reaches `Verified` and is closed out like any other. A reject verb would be a second way to reach a state the absence of progress already reaches. |
| `ExpireSettlement` | 25 | **NOT THIS RECORD'S** | Settlement expiry belongs to the batch and its settlement cursor, not to a candidate. `GeneralBatchV1::release` covers the maker-facing half (escrow returned after the window); the settlement cursor's own expiry is open and named in §6. |
| a challenge / replacement verb | — | **NEVER EXISTED, and is deliberately not invented** | See below. |
| `CancelOrder` | 5 | **CARRIED** | `GeneralBatchV1::cancel`. Maker-only, while collecting. |
| `CloseOrder` | 6 | **CARRIED as `release`** | Permissionless after the settlement window. Renamed because "close" reads as an account operation and this one is a refund. |

Gen-2's `OrderPhase` was `Open / Cancelled / Consumed / Released` with canonicity
by remaining lots. Gen-3 has three phases — `Placed / Cancelled / Released` — and
**`Consumed` is deliberately absent**: with a per-order escrow the vault's own
balance says what a candidate consumed, so a phase asserting it would be a second
authority over a number the chain already holds exactly.

### There is no challenge verb, and that is a verdict

Gen-2 had no way for a second solver to displace a first solver's candidate.
Candidates were independent PDAs and the chooser was a **permissionless
once-per-candidate fold into a running maximum** — score by preference surplus,
ties by lower identity, one consideration right per candidate, frozen by
`LockSelection` after the selection window. It made no optimality claim without
a checked certificate.

Gen-3 already had exactly that in `consider_verified_candidate_v2`, and it is
strictly better than a challenge: it needs no incumbent account in the frame
(the cursor persists the whole comparison key), it is permissionless, and it
terminates by `Freeze`. A challenge verb would be a second way to decide one
question, and the two would have to be kept in agreement forever.

### Who may submit: anyone, with no bond and an exact work escrow

**Gen-2 carried no bonds anywhere**, and neither does this. A bond is a fee on
being right as much as on being wrong: a solver whose valid candidate simply
loses the comparison has done the protocol a service, and slashing the honest
case is what makes an open solver set close.

What gen-2 had instead is the reusable invention, and it is carried:
a **compartmentalized, fully refundable work escrow**, re-proven at every
transition by `validate_capitalization`, drawing exactly one reward per crank to
the calling actor, refunded in full on loss. That is what allowed unbounded
permissionless submission with no candidate cap and no draw on the Market's
Hoard.

Gen-3's compartments are sized to gen-3's cranks:

| compartment | capacity | pays |
|---|---|---|
| verification | `(row_count + 1) · r` | one crank per execution row, plus the single consideration |
| cleanup | `r` | the one crank that closes a spent candidate out |

Funding must be EXACT in both directions. Underfunding buys work nobody is paid
for; overfunding leaves lamports with no rule for who gets them, which is the
same hole facing the other way. `row_count` is declared at submission because it
is what the escrow is sized against, and it is checked by construction: the
verifier cursor advances its revision once per row, so a candidate whose real
row count differs from its declaration cannot complete.

### The one thing gen-2 got wrong, fixed rather than copied

**Gen-2's consideration was permissionless and UNPAID.** That makes a verb
permissible rather than live: a valid candidate nobody cranked before the
selection window closed never competed at all, and a submitter whose
consideration was censored had no recourse. Liveness was hoped for.

Gen-3 funds it. The consideration is the last crank the verification compartment
was sized for, so whoever performs it is paid out of the candidate's own escrow
— the funded-permissionless-walk pattern the resolution walk already uses.
`every_permissionless_crank_is_paid_out_of_the_candidates_own_escrow` is the
witness; `an_abandoned_candidate_refunds_its_unspent_work_to_the_solver` is the
other half, and it pays the cleanup crank while returning the rest to the solver
rather than to whoever happened to call.

### Who may verify: anyone, and now for a reason

Verification is work anyone may do and **nobody may withhold**. A solver who
submits and then declines to verify would otherwise occupy a batch's selection
window with a candidate nobody can evaluate. Being paid is what turns that from
a permission into an expectation.

## 2. The escrow ruling, as implemented

**An order admits only by MOVING its exact worst case into escrow.** Admission
was a check against an authenticated observation; it is now a transfer.

Both legs address the escrow **by the order's own content identity**:

| leg | before | after |
|---|---|---|
| quote | `Collect` debits `External(owner) -> Settlement(candidate_id)` | admission moves `External(owner) -> Settlement(order_id)`; `Collect` moves `Settlement(order_id) -> Settlement(candidate_id)` |
| claims | `Collect` transfers the MAKER's Position -> settlement | admission transfers maker Position -> Position `(market, order_id)`; `Collect` transfers that -> settlement |

`Distribute` is unchanged and stays `Settlement(candidate_id) -> External(owner)`:
credits are earned, not escrowed.

### Per-order, not per-batch

The instruction was "a batch compartment"; the implementation is per-order, and
the argument is not thrift.

A batch-wide escrow pools every maker's collateral. Refunding maker A exactly
then requires a ledger of what each order consumed — and `Collect` never touches
an order record, so there is nowhere honest to keep one. A wrong ledger pays A
out of B's collateral, which is precisely the class the compartment taxonomy
exists to prevent.

With a per-order escrow the refund is the vault's own balance, so **"a maker can
never be paid more than they escrowed" is a property of the address** rather
than an invariant something has to maintain. `release` therefore quotes no
amount at all: whatever a winning candidate collected has already left, so what
remains IS the refund, and a computed figure would be a second authority over a
balance the chain already holds exactly.

The cost is named: each order carries an escrow vault and an escrow Position
beside its record, all rent-bearing, all paid and reclaimed by the maker.

### No new compartment tag

An order's escrow and a candidate's settlement inventory are the **same economic
pool**; what separates them is the vault context, which is already a PDA seed.
`CompartmentV1` separates pools that must NOT be interconvertible — a Hoard
principal receipt must never be accepted as a fee effect — and a new tag here
would separate two things that must be interconvertible, while putting a
General-shaped row in a taxonomy every family reads. Custody permits a
`Settlement -> Settlement` transfer between distinct contexts, and the vault PDAs
differ because the contexts do.

### The batch counter became a real bound

`committed_quote_reserve` was a sum of promises and is now the sum of balances
actually held. `authenticate_batch_verified_candidate_v1` uses it: a candidate
whose whole quote debit exceeds the batch's escrow could not be paid, and is
refused before any settlement account exists rather than stranding at its first
short `Collect`.

## 3. Two defects the work found

Both were invisible for the same reason: the only things exercising these paths
were fixtures that shared an author with the code.

### A candidate could fill an order with a portfolio its maker never signed

`authenticate_order_execution_v1` bound the compact header fields — `order_id`,
`owner_id`, `nonce`, `max_lots` — and **not the per-lot vectors**. But the
verifier accumulates `claim_input = deliver_per_lot * lots` and
`claim_output = receive_per_lot * lots` from the vectors the CANDIDATE PAGE
carries, and `AuthenticatedOrderTermsV2` has no coordinate for either. So a
candidate author could pay a maker in the outcome they were delivering and take
the one they were buying, at the maker's own signed limits, with a digest that
matched.

Nothing else closed it: the row's vectors are re-read from the same page on
every step, so they were self-consistent and wrong together. This is also what
makes the admission escrow a bound at all — the reserved claims are computed
from the record's `deliver_per_lot`, and would otherwise bound nothing the row
does.

### A candidate could name any identity it liked

`CandidateV2::decode` treats `candidate_id` as a **declared field** and checks
nothing about it. It is the join key for the verifier cursor, the certificate,
the selection cursor, the settlement cursor and every manifest row.
`general_candidate_identity_v1` masks exactly the 32 bytes that carry the
identity and digests the rest — the only way a self-describing record can be
content-addressed — and submission requires it.

## 4. The set relaxation: four profiles, not an inequality

`authenticate_general_program_set_v3` required **exactly seven**
`CapabilityProgramV4` entries. Decision 0009 §3 needs coordinates for the new
actions; ADR-0006 §8 item 7 needs one for the activation descriptor, which is a
`CapabilityProgramV1` and therefore not one more of the same.

Relaxing `== 7` to `>= 7` is one character and the wrong shape. An open table
admits a coordinate nobody enumerated, and the whole argument for a program SET
rather than one permissive descriptor is that the reachable programs are named
in advance. **The entry count selects one of four named profiles and the profile
fixes an exact table with a role per coordinate.**

| profile | entries | contents |
|---|---|---|
| `SettlementOnly` | 7 | the seven settlement actions |
| `SettlementWithActivation` | 8 | plus the activation descriptor |
| `Complete` | 14 | every action |
| `CompleteWithActivation` | 15 | both |

Counts 6, 9, 10, 13 and 16 are refused, and those are exactly the tables a `>=`
rule would have admitted. Adding a fifth profile is a visible edit, not a
consequence of publishing a longer set.

The activation coordinate carries selector `255`, which `Action::decode`
refuses, so no controller request can select it. It exists to be NAMED — so the
ProgramSet identity and the capability seal cover the activation descriptor —
not to be dispatched to.

**A wider profile is a legal SET and not yet an admissible RELEASE.** Hot
release admission joins the seven settlement bundles; the collection and
candidate coordinates have no artifacts to join, and admitting them unvalidated
would be worse than refusing, so it refuses by name (`UnjoinedProfile`).

## 5. The EffectV4 envelope, and why it is in this batch

GEN-HOT found it from outside the family and could not fix it from the Trading
side. `process_hot_execution_v3` accepts exactly one effect schema —
`dclutch_effect_kernel::v4::SCHEMA_RELEASE_ID_V4` — and General published a bare
`ProgramV3`. **Nothing General emitted could enter the Hot executor, for any
action.**

It survived every fixture because the two sides agreed with each other:
`authenticate_general_artifacts_v3` hard-pinned the V3 schema, and the deployed
accelerator ELF runs that same join. One author emitting V3 and one
authenticator requiring V3 is a closed loop that no adversarial testing inside
it can open. It took a family-neutral executor that had never run a General
release to say the shape was wrong. **The lesson generalises: a family's own
emitter and its own authenticator are not two authorities.**

The envelope is a pure extension — zero dynamic spans, zero borrowed ranges.
General's sole dynamic span is declared by its ACCOUNT PROFILE (the trailing
Trading-owned scratch-page span), so an effect span here would be a second
author disagreeing with the profile.

It rides this batch because it moves the effect digest, and
`ExecutionStrategyCertificateV2` cross-checks `effect_program` against both the
descriptor and the artifacts. The certificate, the admission, the strategy, the
descriptor, the ProgramSet and the capability seal all move with it — one
regeneration, this one.

| action | V3 bytes | V4 bytes | V3 digest | V4 digest |
|---|---|---|---|---|
| Consider | 560 | 584 | `6fdb746f3de3bcf5` | `8519f4993121c31a` |
| Freeze | 416 | 440 | `969e7736cc455f0e` | `fca257e4caa37ff6` |
| InitializeSettlement | 3,120 | 3,144 | `6b56a4b86e04a70c` | `b00c2a6e4ee22a5b` |
| Collect | 2,592 | 2,616 | `1257c853920da3d9` | `7cc4afd1ad994078` |
| Materialize | 2,552 | 2,576 | `a3b08cfcdfbc2226` | `f6b83291ad445732` |
| Distribute | 2,592 | 2,616 | `9cd699d3847e5708` | `aaced5ebcc7c313f` |
| Close | 4,736 | 4,760 | `03d12bcc7b67e28c` | `a35ec85e4d0b7eac` |

Every envelope is exactly 24 bytes wider than its base and preserves it byte for
byte.

## 6. What General still lacks

1. **Seven artifact triples.** The collection, escrow and candidate actions have
   Lean-owned tags, reserved ProgramSet coordinates and complete authenticated
   pure transitions. They have **no TransitionVM program, no EffectProgram and
   no AccountProfile**, and no Lean-emitted RequestProfile. Every artifact
   generator refuses them by name (`UnauthoredAction`), every dispatcher writes
   them out explicitly, and `general_request_profile_bytes_v1` returns the empty
   slice — so an unauthored action is admissible with NO profile rather than a
   permissive one. Until they exist, `Complete` is a legal set and not an
   admissible release.
2. **The census rows do not flip on this work.** The seven-action campaign is
   ProgramTest on legacy packets, and at N=258 six of seven actions serialise to
   1,207-1,329 bytes against Solana's 1,232-byte legacy maximum. At N=1 every
   packet is 745-867 and the clause holds. Flipping the rows needs the ALT/v0
   route `blocked.json` names, plus the accelerator reached through the real
   Trading Hot path.
3. **The work escrow is accounted and not yet moved.** `close_out` returns the
   exact cleanup reward and the exact solver refund, and every crank returns its
   `WorkRewardV1`; nothing yet moves lamports, because a transfer is an account
   operation and these are pure transitions. Rent ownership is the same shape:
   the solver owns a submission's rent and the maker owns an order's, and
   closure has no route.
4. **`ExpireSettlement` has no gen-3 counterpart.** Gen-2 could expire a
   settlement that stalled. `GeneralBatchV1::release` covers the maker-facing
   half; a settlement cursor that stalls mid-graph is still stuck.
5. **The claim escrow's Position lifecycle.** The escrow Position is
   `(market, order_id)` with the `TradingRecord` owner kind, which needs no
   Claims-family change — but nothing yet creates or closes it.
6. **`GeneralClearing.lean` still does not model the collection or candidate
   halves**, and `AdapterBoundary.orderSignaturesAuthenticated` remains a named
   boundary obligation discharged outside the model. This record does not change
   that; it makes the thing that discharges it executable.
