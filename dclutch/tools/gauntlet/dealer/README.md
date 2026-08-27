# The Dealer family campaign

A **ProgramTest fast lane** for `dealer/process_dealer_family_instruction`, the
one route `programs/dclutch-dealer-sbf` exposes. It runs the real
`dclutch_dealer_sbf.so` against the real `dclutch_registry_sbf.so`, with the
real Core and Custody artifacts installed as genuine Loader-v3 deployments.

```sh
tools/gauntlet/run.sh --mode census     # once, for the inventory
tools/gauntlet/dealer/run-dealer.sh
```

The campaign itself is
`programs/dclutch-dealer-sbf/program-test/tests/family.rs`, an ordinary
`cargo test` that emits census evidence only when
`DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` is set. It is runnable without the gauntlet.

## What it claims, and what it does not

**Twenty-three transactions, every one of them a refusal. This campaign claims NO
executed row.** A route is EXECUTED in the census only on a succeeding
transaction, and nothing here succeeds — `this-tier-claims-no-executed-row` in
`witnesses.json` pins that to zero so a future change cannot flip a census row
by weakening one of these refusals.

What it does evidence, against bytes a loader would run:

- **All eight canonical Dealer actions reach the Registry reauthentication
  CPI.** ScheduleReplacement, ActivateReplacement, Fill, EnterTerminal, Unwind,
  Retire, AddLiquidity and RemoveLiquidity each pass instruction shape, the
  23-account common frame, Policy/Candidate/State identity, their own actor
  rule and their own Clock rule before anything refuses them.
- **Six of the ten `DealerSbfError` codes are raised by the chain**, not
  asserted by a unit test: `Instruction` (0), `AccountFrame` (1),
  `AccountIdentity` (2), `Signature` (3), `Clock` (4), `Release` (5). Every case
  asserts the exact code, so a refusal arriving for a different reason fails the
  case rather than passing it.
- **`Release` is a real CPI refusal.** Dealer maps a failed `invoke`, a missing
  return value and a foreign producer to the same code, so the code alone cannot
  distinguish "the Registry refused" from "Dealer refused before calling it".
  The chain's own `invoke [2]` line is the discriminator and a witness requires
  it on every transaction whose label claims that depth.

## The four codes it does not raise, and why

`Semantic` (6), `Claims` (7), `Custody` (8) and `Commit` (9) all live past the
release stage. Reaching them needs a Registry activation cache with Core,
Trading and Custody activated against this release set, a Core Market in an
admitting phase, a Realm, and three real SPL token vaults whose balances agree
with the persisted Dealer `State`. This campaign builds none of that and says
so rather than approximating it. Those four codes, and the executed row, are
the next increment.

## A defect this campaign executed, and then outlived

`AddLiquidity` and `RemoveLiquidity` were **unreachable on-chain at every slot
the chain can offer**, and two transactions were the two halves of the proof:
`Request::validate_shape` required `now == 0` for both, `authenticate_clock`
required `now == clock.slot` for every action but `Retire`, and no slot the
chain can offer satisfies both. The witness that pinned it,
`both-liquidity-actions-are-unreachable-at-every-offered-slot`, was written to
fail the day the contradiction was fixed.

**It was fixed, and it is now two witnesses pointing the other way.** The rule
has one owner: `Action::now_discipline` in `crates/dclutch-dealer-codec`, read
by both the request shape and the adapter's Clock authentication. It follows the
semantics rather than restating it — `DClutchSemantics.DealerLiquidity.Command`
carries a slot inside `Replacement`, `Activation` and `Fill` and nowhere else,
so those three bind `now == clock.slot` and the other five require canonical
zero. `EnterTerminal` and `Unwind` moved into the zero class with the two
liquidity actions; they had carried a slot no transition ever read.

- `both-liquidity-actions-reach-the-release-stage-at-a-real-slot` — both now
  refuse at the Registry CPI (`Release`, 5) like the other five actions. A
  reintroduced Clock disagreement shows up here as code 4.
- `a-slot-in-the-padding-is-refused-at-decode` — a slot patched straight into
  the wire bytes of RemoveLiquidity, EnterTerminal and Unwind refuses at decode
  (`Instruction`, 0). Relaxing the padding rule instead would have admitted 2^64
  encodings of the same liquidity adjustment, each with its own request digest.

## The fast-lane bar

`TIERS.md` requires each fast lane to state which of the four conditions it
satisfies. This one:

- **Loader-v3 / `SetAuthority` / deployment slots** — satisfied by not depending
  on them. The ProgramData accounts are constructed by the campaign with a
  surrendered upgrade authority, and no case turns on a deployment slot. The
  Dealer route reads ProgramData only through the Registry CPI, which refuses
  this campaign for a reason (no activation) that is upstream of any ProgramData
  layout question.
- **Packet serialisation limits** — NOT satisfied, and it does not need to be.
  ProgramTest submits no packet. The Dealer common frame is 23 accounts and the
  widest action frame in the family is 35, so nothing here is near the 1,232-byte
  legacy maximum; but the exemption is stated rather than assumed, and a Dealer
  campaign that ever grew a large frame would have to move to a validator.
- **1,400,000 compute and 32,768 heap, neither adjustable** — satisfied. The
  campaign sets `set_compute_max_units(1_400_000)` and never raises it; the heap
  is the SBF default and is never lifted. The deepest transaction consumed
  19,501 units.
- **Real Agave account shapes** — PARTIALLY satisfied and this is the honest
  weak point. Policy, Candidate and State are real encoder output at real PDAs
  with real owners; the Realm, mint, custody-authority and vault slots carry
  placeholder bodies, because every case in this campaign refuses upstream of the
  checks that read them. A campaign that reached `authenticate_token_accounts`
  would have to make them real Token/Token-2022 accounts, and the next increment
  does.
