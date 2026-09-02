# dealer-checkpoint — the Dealer scenario-checkpoint lifecycle, on real ELFs

This tier exists to witness eight routes that row **C-06** owned and no campaign
drove: the seven `dealer_scenario_checkpoint_v1` stages from create through
cleanup, and the Custody reservation route beneath them.

**It is a ProgramTest FAST LANE.** `TIERS.md` states the bar; the answers are in
`fast-lane.json`, one clause at a time, and they ride the evidence document
beside the numbers they qualify rather than sitting in prose here. Two of them
are only partly satisfied and say so: the Hot rows request a 65,536-byte heap
frame, and the v0 commit and activation banks are not measured against the
packet limit because this package has no serializer for a versioned transaction.
A fast lane is additional evidence, never a substitute for a validator tier.

## What the campaign is

`programs/dclutch-dealer-accelerator-sbf/program-test/tests/accepted.rs`, run
with `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` set. Nothing about the campaign is
test-only: it is the suite that already drove these routes, and what was missing
was never coverage but *evidence emission* — which is exactly what the
unwitnessed-routes list warned about when it said an unwitnessed route "is a
statement about coverage and not about correctness".

## Why the labels are derived and not written

A campaign label is the key a `bindings.json` row matches on, so a hand-written
label can drift from the transaction it names while both still look right — a
second author for the route claim, which is the defect class this census exists
to catch.

So the label is read off the bytes the campaign submits, using **the program's
own dispatch predicates**: `is_dealer_scenario_checkpoint_create_v1` and its six
siblings, the same functions `process_dealer_family_v1` branches on. A label
cannot name a route Trading would not take, and if the predicate set ever drifts
from the dispatcher, the witness
`every-checkpoint-row-names-the-route-the-program-dispatches-to` fails rather
than binding a route to the wrong evidence.

The other half of the label is the libtest thread name — the test's own path,
because every case here is a current-thread `#[tokio::test]` — so a row can
always be traced back to the case that produced it.

## What it found, beyond the eight rows

Measuring rather than assuming the packet limit turned up two facts the tier now
pins with witnesses:

- **Commit does not fit a legacy Solana packet.** The one commit submitted as a
  legacy transaction measures 1,366 bytes against the 1,232-byte maximum, which
  is exactly why every other commit travels as a v0 transaction over an Address
  Lookup Table. The other six stages fit with room — cleanup 276, evaluate 409,
  create 541, page 871, reserve 1,026, rollback 1,026.
- **The Hot rows do not fit either**, at 2,342 to 3,084 bytes. They execute under
  ProgramTest because ProgramTest submits no packet; they could not be submitted
  to a cluster as they stand. That is why they are recorded with **no route
  claim**: crediting a route on a frame no validator would accept is worse than
  recording no coverage at all.

All eight **execute**. `..._rollback_v1` was refused-only for one campaign
generation — its only driver was the hostile case that substitutes a bare
rollback magic into the Custody reservation bundle — and
`an_expired_reservation_rolls_back_in_reverse_order_and_returns_the_collateral`
is the accepting twin: reserve, expire, roll back at the reverse-order ordinal,
and assert the escrow drained and the vault came back byte-for-byte. Its
`blocked.json` entry is deleted, which is what that file's own rule asks for the
moment a route executes.

Writing it found a helper that could not express the case: the receipt PDA is
seeded by the ACTION, so a reserve and the rollback reversing it write two
different receipts, and `reservation_receipt_address` hard-wired `Reserve`.
Handing a rollback the reserve's receipt refuses `AccountFrame` (0x6001) — after
the token movement has already run, because the identity check sits behind it.

Ten routes, not eight. Custody dispatches `Reserve` and `Rollback` as routes of
their own beneath `dealer_reservation_v1::process`, and the campaign drives both
— so the bundle bindings claim the action arm as well as the parent. Which arm a
transaction drove is not a guess: the Trading ingest magic the label was derived
from is emitted by the same builder call that chose the Custody action, so the
two cannot disagree.

## Running it

    tools/gauntlet/run.sh --mode census          # once, for the inventory
    tools/gauntlet/dealer-checkpoint/run-dealer-checkpoint.sh

The runner gates on `cargo check` before it builds anything. That is not
belt-and-braces: this tree is shared, and three runs on 2026-09-01 died ten
minutes into an SBF build against another lane's half-applied refactor, each
time looking like a campaign defect rather than a scheduling one.
