# Decision 0009: General's collection half is three more capability actions, and the executor still gets no branch

Status: accepted on 2026-08-27 as the resolution of `M-12` in
`docs/ASPIRATION_LEDGER.md` — *"General's collection half has no route, and the
root carries counters for it."* This is an ownership and flow decision. It is
not release evidence, and it does not claim that any collection action executes
today: the records and the pure transitions landed with this record (`751d702`),
the routes did not.

## Context

`M-12` states the gap as a missing route. Reading the settlement half
establishes that the gap is wider than that, and the wider statement is what
this record is written against.

## 1. What was actually missing: three holes, not one

The seven settlement actions consume records that name a `batch_id` and an
`order_id`. Neither had a producer anywhere in the tree.

| | The value | What produced it before `751d702` |
|---|---|---|
| A | `CandidateHeaderV2::batch_id`, and the same field on the verifier cursor, the selection cursor and the verified candidate | **Nothing.** A free 32-byte parameter. Every test in the family passes a literal — `BATCH = id(41)`, `BATCH = [0xb2; 32]` |
| B | `AuthenticatedOrderTermsV2` — `max_lots` and `max_quote_debit_per_lot`, the values the streamed verifier enforces its quote-limit discipline with | **Tests only.** `runtime_verify.rs:220`'s own doc says generic Trading builds it "only after its selected Account and Request Profiles authenticate the finalized order record". No such record type existed |
| C | `GeneralRootV2::open_batch` / `close_batch` | Called only by `root.rs:1032-1045` and one lifecycle-projection test |

Hole B is the sharpest. The verifier refuses `ExcessLots` and `QuoteLimit`
against terms *the caller hands it*. With no order record to project them from,
a caller could assert any limit it liked and the verifier would enforce that
assertion faithfully. The discipline was real; the thing it disciplined was not.

**The Lean semantic owner does not model the collection half either.**
`DClutchSemantics/GeneralClearing.lean` has `structure Order` (`:150`) and a
`Candidate.batchId : Nat` (`:182`) constrained only to be nonzero (`:302`).
There is no `Batch`, no admission, and no window. What there *is* is
`AdapterBoundary.orderSignaturesAuthenticated` (`:69`) — a named boundary
obligation, discharged outside the model. The collection half is precisely the
thing that discharges it. Substrate, said out loud: General's artifacts are
TransitionVM programs and AccountProfiles — interpreted data authored in Rust
contract crates — not AIR, and there is no constraint system in the Trading hot
path. `GeneralClearing.lean` is the semantic owner of *clearing*; it is not a
circuit and this record adds none.

## 2. The intended flow

Recovered from the settlement half's own input requirements and from gen-2's
implementation (`dclutch-general-contract`, recoverable at `35320756`;
`programs/dclutch-sbf/src/general.rs` at `dd1ec033`).

```text
OpenBatch     root.open_batch(revision, sequence)  ->  GeneralBatchV1
                 identity = SHA-256(immutable 160-byte opening)
                    |
PlaceOrder    maker signs; GeneralOrderV1 record created
                 identity = SHA-256(whole record)
                 batch.admit(order, funding, slot)  -- worst case reserved
                    |  (repeat, bounded by max_orders and collection_close_slot)
CloseBatch    root.close_batch(revision)  ->  order set final
                    |
              ~~~ everything below already exists and executes ~~~
SubmitCandidate / VerifyCandidatePage   [STILL MISSING -- see §6]
              evaluate_runtime_consider_row_with_manifest_v2
                 -> VerifiedCandidateV2 + SettlementManifestV2
                    |
Consider -> Freeze -> InitializeSettlement -> Collect* -> Materialize
         -> Distribute* -> Close
```

### Who opens: anyone, inside a slot window

**Permissionless**, and the window is what makes it safe. This overrides the
instinct to reach for an operator key, and the argument is in the root:

```text
root.rs:410   retire() -> if self.open_batches != 0 { OutstandingBatches }
```

An unbounded open batch is a *permanent denial of the retirement path*. An
authority key would fix that by making opens rare; a `collection_close_slot`
fixes it by making them terminate, without inventing a privileged role in a
family that has none. Gen-2 reached the same answer independently: **every verb
in its collection half was permissionless**, gated on windows and counters, with
`WorkActor` checked only by `require_system_wallet` — a shape check, not an
identity check. `a_batch_left_open_blocks_root_retirement` is that argument as a
test.

### Who places: the maker, signing directly

**The maker signs their own placement.** `owner_id` is the maker's pubkey — the
same identity Custody routes against (`child_packets.rs:298`
`resources.destination_owner == context.owner_id`). There is no signed-intent
relay and no Ed25519 sysvar: gen-2 had none either (zero `ed25519` hits across
its whole General implementation), and `AdmitOrder` refused unless
`order.owner() == owner.key` where `owner` was a transaction signer.

The order record carries no signature field because it does not need one: the
transaction signature *is* the authorization, and `order_id` is the digest of
the record it authorizes.

### Who closes: anyone, once closing takes nothing from anyone

**Permissionless after `collection_close_slot`, or as soon as the batch is
full** — `GeneralBatchV1::close_is_permissionless`. A full batch can admit
nothing further, so closing it early truncates no maker's opportunity. Before
either condition an early close is a griefing vector, and the transition
refuses.

### Funding: checked at placement, moved at settlement — and that is a gap

`quote_reserve = max_quote_debit_per_lot * max_lots` and
`claim_reserve[i] = deliver_per_lot[i] * max_lots` are the exact worst case if
the order fills completely (gen-2's formulas, with V3's already-scaled per-lot
limit in place of gen-2's numerator/`price_scale` division). `admit` refuses
`Unfunded` unless an authenticated observation covers both.

**It checks; it does not move.** The settlement half debits the maker at
`Collect` time — `CompartmentV1::External -> CompartmentV1::Settlement`
(`child_packets.rs:316-323`) — so the collateral sits in the maker's own
account between placement and settlement.

Gen-2 escrowed at admission instead: quote atoms moved into a per-order SPL
escrow and negative coefficients were debited from the owner's native Position,
atomically with replay creation. **Gen-3's collect-time debit is a regression in
credit discipline**, and it is a live one: a maker can place a funded order and
spend the collateral before the batch settles, at which point `Collect` fails
and the whole candidate is stuck.

Real escrow at placement is not free — it needs `Collect` to move *within*
`Settlement` rather than from `External`, which is a Collect-side compartment
change and therefore an artifact regeneration for the family. Recorded here as
an owned gap rather than assumed away. See §6.

## 3. The ownership ruling: three more capability actions

**The collection routes are three new General actions — `OpenBatch`,
`PlaceOrder`, `CloseBatch` — in General's `CapabilityProgramSetV2`, reached
through the existing `DCLTHOT3` outer route with zero hot-executor change.**

This is ADR-0006 §3 applied rather than contradicted: *"A General hot action
does not need a dispatch point, a selector, or an account-suffix contract to be
added."* The action byte at `GENERAL_CONTROLLER_ACTION_SELECTOR_OFFSET_V3`
selects the descriptor at `hot_v3.rs:1738`; the account frame comes from the
selected AccountProfile; the family is proved by a PDA derivation joined to a
Market manifest entry and a sealed descriptor. Exactly as the seven settlement
actions are reached today.

**No STOP, no yield.** The one question that could have forced an outer route is
the maker's signature, and the data-defined path already carries it:

```text
account-profile-contract/src/v2.rs:354   PrivilegesV2::signer()
hot_v3.rs:4168   logical.is_signer = declared & DECLARED_SIGNER_V3 != 0
hot_v3.rs:1530   refuses when  meta.is_signer != info.is_signer
```

An AccountProfile declares a per-action, per-coordinate signer bit and the
family-neutral executor enforces it. `PlaceOrder` requires the maker's signature
by *declaring* it, not by a branch. Account creation has the same answer:
`local_state_v3.rs` exists to "authenticate a live PDA or create its vacant
successor", which is how `InitializeSettlement` already creates its settlement
state account with a rent quote.

### The two alternatives, and why not

- **A new Trading outer route** (a `DCLTGOB1` beside `DCLTGMF1` / `DCLTPCB1`) is
  the cheapest to build and installs a General-shaped route in a program whose
  whole design claim is family-neutrality (`outer.rs:5`, `lib.rs:5-11`).
  Rejected for the same reason ADR-0006 rejected wiring the V1/V2 hot slice.
- **A new family program with its own entrypoint**, modelled on the General
  accelerator, cannot write the root — the root is owned by the Trading program,
  so it would have to CPI back through a Trading route, collapsing into the
  first alternative. The accelerator is also structurally read-only: every
  account in its frame must be non-writable (`accelerator/src/lib.rs:227-233`).

### What the ruling costs

`authenticate_general_program_set_v3` (`release_v3.rs:102-113`) requires
**exactly seven** `CapabilityProgramV4` entries whose selectors are
`GENERAL_ACTIONS_V3[i] as u8`. Three more actions widen that to ten — and
ADR-0006 §8 item 7 already requires widening it for the activation entry
(an eighth, and a `CapabilityProgramV1` at that). **One batched identity
regeneration pays for both.** The first such regeneration cost 16 CU per action
and moved no account, packet or scratch page, so the shape of the cost is known.

The action tags are Lean-owned: `ACTION_CONSIDER..ACTION_CLOSE` are emitted into
`generated_general_controller.rs` by
`formal/dclutch-semantics/EmitGeneralControllerAbiRust.lean`. Three new tags are
a Lean change first, a regenerated ABI second, and Rust never.

## 4. The records

Landed in `crates/dclutch-general-adapter-contract/src/collection_v1.rs`, in the
crate that owns what consumes them.

**`GeneralBatchV1`** — 224 bytes: an immutable 160-byte opening, then mutable
counters. `batch_id` is the digest of **the prefix only**, so the identity a
Candidate names is fixed at open and cannot move as orders arrive. Because that
prefix commits to market, generation, product and price scale together, one
digest comparison decides substitution across all four at once — which is why
`authenticate_batch_candidate_v1` is four conjuncts rather than a table.

**`GeneralOrderV1`** — `160 + 16N`, wholly immutable, `order_id` the digest of
the whole record, `terms()` the first non-test producer of
`AuthenticatedOrderTermsV2`.

**A batch is a window, not a ledger.** It bounds and counts its orders; it never
enumerates them. Gen-2 made the same choice — *"there is no order count on the
batch; orders are never enumerated by the batch"* — with each order an
independent PDA and the candidate carrying the execution set. Keeping it is the
weakest binding that still refuses substitution, and it keeps admission O(1).

**Replay** is address occupancy, as in gen-2: the order PDA is seeded on
`(market, generation, owner, nonce, order_id)`, so a second admission of the
same signed order hits an occupied address. No bitmap and no per-owner cursor.

## 5. What the campaign proved

`programs/dclutch-general-accelerator-sbf/program-test/tests/lifecycle.rs` now
opens a real batch, places three real signed orders, closes it, and drives the
existing seven-action graph against exactly those artifacts on real ELFs. All
19 tests green at N=1 and N=258.

The control is that **accounts, legacy packet bytes and scratch pages are
identical to every row of
`docs/evidence/GENERAL_ACCELERATOR_CAMPAIGN_2026_08_27.md`** — fourteen rows,
three columns, no movement. Real identities in place of literals should change
no geometry, and did not.

It also found a rule no fabricated fixture could have tested.
`runtime_verify::le_numeric_id` (`:1345`) orders a 32-byte identity as a
**little-endian 256-bit integer**, which is not `[u8; 32]`'s lexicographic
`Ord`. The old fixture's identities were `[low, 0, 0, ...]`, where the two
orderings agree; a real digest makes them disagree, and sorting the wrong way
refused three of four suites with `NonCanonicalOrder`. **A candidate builder
must sort by the protocol's identity order.** That is now written where the
sort happens.

## 6. What General still lacks

1. **The three routes themselves** — the Lean action tags, the artifact triples
   (TransitionVM program, EffectProgram, AccountProfile) and the set-count
   relaxation. This record is their design; batch the regeneration with
   ADR-0006 §8 item 7's activation entry.
2. **Candidate submission and page verification.** Gen-2 had `SubmitCandidate`,
   `CreateCandidatePage`, `VerifyCandidatePage`, `FinishCandidate`. Gen-3 has
   the evaluator — `evaluate_runtime_consider_row_with_manifest_v2` — and **no
   caller outside tests**. `Consider` reads a `SubmittedVerifiedCandidate` from
   a readonly account that no action writes. This is the same shape of gap as
   `M-12` and it is not closed by this record; the campaign runs the verifier in
   the harness, off-chain. It deserves its own ledger entry.
3. **Escrow at placement**, per §2 — the Collect-side compartment change that
   turns the funding check into a funding *hold*.
4. **Order cancellation.** Gen-2 had `CancelOrder` and `CloseOrder` with an
   `OrderPhase` and exact custody release. `GeneralOrderV1` is immutable and a
   placed order is binding until its batch closes. That is the weakest thing
   that works; it is not what a maker will want.
5. **Rent and its beneficiary.** Batch and order accounts are rent-bearing and
   nothing yet says who pays or who reclaims. Gen-2 prepaid continuation rewards
   at open and routed rent to a permanent RentCredit.
6. **Two griefing questions the windows do not answer**: an order whose maker
   spends the collateral before `Collect` (§2), and — inherited from gen-2 and
   worth not re-adopting — its unsigned `Quiesce`, which let anyone permanently
   stop new batches for a generation.
