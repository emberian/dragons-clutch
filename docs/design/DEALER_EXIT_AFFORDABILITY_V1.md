# Dealer exit affordability: closing R4 without a force-exit verb

Date: 2026-08-30
Status: design ruling, written before implementation
Scope: `crates/dclutch-dealer-codec`, `formal/dclutch-semantics/.../DealerLiquidity.lean`
Census rows: R4 (queue Q4), R9 (queue Q4, with dealer)

This is a Dealer scenario accepted-transition family. It selects no price,
quotes nothing, and holds no inventory. It is not an AMM, an order book, or a
quote surface.

---

## What the census asked for, and why this note does not build it

The census costs Q4 as *"the least-live family — a full design pass, not a
patch"*, and names three things the Dealer family needs: an on-chain entry
route, a **force-exit / quiescence-timeout for an unresponsive dealer**, and a
bounded LP-share redemption.

The force-exit is the item this note refuses to build, and the refusal is the
result. A force-exit verb is a bailout: it exists to rescue a state the protocol
should never have been able to reach. Once the actual reachability is traced —
below — the bricked state turns out to be excluded by a single missing
invariant. **Making the bad state unrepresentable is strictly better than adding
a ninth action to reach into it**, and it costs no new field, no new action, no
ABI change, and no ELF.

The census was right that this is the least-live family. It was wrong about
which mechanism does the bricking, and the true mechanism is cheaper to close
than the one it described.

---

## Findings, each verified at its own line

Every claim below was re-read at source. The census's own R4 citations all hold;
these are what a trace of *every* write to the liveness vault adds to them.

### The vault, as it actually behaves

`crates/dclutch-dealer-codec/src/lib.rs` holds the whole transition machine.
There are exactly **two** writes in it that increase `active_work_remaining`:

- `schedule` — `:1674`, sets `liveness_custody = active_work_remaining + proposed.work_funding`
- `activate` — `:1722`, sets `active_work_remaining = pending_work_funding`

Every other write is a decrement (`fill` `:1823`, `unwind` `:1892`) or a zero
(`retire` `:1942`). And **both** increasing writes refuse unless
`state.phase == Phase::Open` (`:1641`, `:1688`).

### F1 — the vault cannot be refilled in Terminal by anyone, including the dealer

Once `enter_terminal` runs (`:1858`), the liveness budget is strictly
decreasing, with no refill route available to any party: not the dealer, not a
keeper, not a bounty hunter, not the market. The census frames R4 as *a vanished
dealer*. A vanished dealer is one way to arrive in Terminal underfunded.
Ordinary trading is another, with every party present and cooperative:

`fill` carries no reservation floor. It refuses only when the budget is below
**one** `work_reward` (`:1756`), so the last admitted fill may legitimately
leave `active_work_remaining` at zero, and `enter_terminal` carries that residue
into Terminal unchanged (`:1861`). In Terminal, `unwind` is the only route to
zero inventory and each call requires one `work_reward` (`:1873`); `retire`
refuses on any nonzero inventory (`:1914`).

So a market that simply traded enough before it resolved is bricked by the same
mechanism, and no absence is required to produce it.

### F2 — the dealer can shrink the vault deliberately, for free, and be refunded the remainder

`activate` **replaces** the budget rather than adding to it (`:1722`), after
refunding the outgoing remainder to `DealerOwner` (`:1710-1713`). The only floor
on a candidate's `work_funding` is `>= policy.minimum_work_funding` (`:498`) and
`>= work_reward` (`:489`) — that is, **one crank**.

A dealer may therefore schedule a one-crank replacement candidate, activate it,
collect the entire existing vault as a refund, and leave a multi-outcome
inventory with exactly one affordable unwind. That is a deliberate, profitable,
protocol-admitted brick. It is not an absence and a quiescence timeout would not
catch it, because the dealer is present and acting.

### F3 — WITHDRAWN: the zero-quantity unwind is not reachable, and the way that surfaced is worth keeping

The first draft of this note claimed a third finding: that
`DealerLiquidity.lean` states `0 < quantity` three times — `fillAccepts:457`,
`unwindAccepts:527`, `liquidityChangeAccepts:559` — while the Rust transcribed
it once, into `fill`'s transition alone, leaving a zero-quantity `Unwind` as a
paid no-op that anyone could repeat until the vault was empty.

**That is wrong, and the claim should not be carried forward.** The conjunct is
transcribed for all four economic actions. It lives one layer up, at the wire:
`Request::validate_shape` refuses `quantity == 0` for `Unwind` (`:1272`) and for
both liquidity actions (`:1290`) with `NonCanonicalPadding`. Every entry point
decodes before it interprets — `interpret`, `interpret_projected`, and
`dealer-sbf`'s own `Request::decode` at
`programs/dclutch-dealer-sbf/src/lib.rs:383-386` — so no such packet can reach a
transition, on chain or off.

The error was reading the transition functions in isolation and concluding from
`fill`'s redundant guard that its siblings had none. It is recorded here rather
than deleted because of **how** it surfaced: the hostile was written before the
claim was believed, and it could not be *constructed* — `Request::to_bytes`
refused the zero-quantity request at the encode. A case answered by an earlier
gate is not the case it claims, and the cheapest way to discover that a guard is
two layers deep is to make the case actually run.

What is kept is a test, not a conjunct.
`the_wire_owns_the_zero_quantity_rule_for_every_economic_action` pins where the
guard lives for all four actions, and states in its own header what a
zero-quantity unwind *would* do if that arm of `validate_shape` were ever
relaxed: `Plan::push` no-ops on a zero amount (`:1387-1389`), so the redemption
and the hoard transfer would vanish and the `LivenessVault -> Executor` reward
would be the only surviving transfer. The two conjuncts the first draft added to
`unwind` and `adjust_liquidity` were removed — unreachable guards that read as
load-bearing are the same defect class as a dormant plan that reads as a design.

Nothing in F1, F2, or either ruling depends on F3.

### F4 — entry has no on-chain route, and this is a different problem than a liveness gap

`programs/dclutch-dealer-sbf/` **never creates an account**. It has no
`create_account`, no `allocate`, no `assign`, no `realloc`, and no
`system_instruction` import anywhere in the crate. Its single `invoke_signed`
(`src/lib.rs:1506`) is a CPI relay that signs an already-built Claims or Custody
instruction under a re-derived caller-authority PDA. Its 23-account common frame
carries no System program and no payer (`validate_prefix`, `:407-450`), and
`:417-419` requires the State account to **already exist** when the instruction
arrives.

Nor does any test create one. `family.rs` injects `Policy`, `Candidate` and
`State` as pre-existing accounts via `ProgramTest::add_account` (`:366-398`);
the accepted campaign carries the candidate as an opaque byte body
(`accepted.rs:1575`). The only real allocator that could mint these is the
generic Trading lifecycle executor (`trading-sbf/src/hot_v3.rs:7159,7165`), and
`programs/dclutch-trading-sbf/src/dealer/` only ever *plans* allocation
(`v3_lifecycle.rs:124`, `v3_obligation.rs:103`, `v3_multi_lp.rs:213`) — no
Dealer program-test drives it.

Stated plainly, as the census asked: **a family whose participants cannot enter
on chain does not have a liveness gap, it has a reachability gap.** No Dealer
`Policy`, `Candidate` or `State` can be created by any route that is executed
anywhere in this tree — that part is measured above. That none exists on a
cluster is the campaign's own statement rather than mine
(`DEALER_ACCEPTED_TRANSITION_2026_08_29.md`, "No devnet anything. No selected
Market, no live participant, no public caller"), and no evidence document in
`docs/evidence/` records a founded Dealer.

Two consequences, and they point in opposite directions:

1. It **de-urgents** R4. Nothing is stranded today because nothing is founded.
2. It makes every fix in this note **free to land**. There is no deployed state
   to migrate and no live participant to break, so a tightening that would
   normally need a compatibility story needs none. This is the cheapest this
   family will ever be to correct, and that window closes the day entry is
   built.

---

## The standard this is held to

The GREEN exemplar is the compartmentalized work escrow at
`crates/dclutch-general-adapter-contract/src/candidate_v1.rs`. Held against it,
the Dealer liveness vault has two of its four properties:

| `candidate_v1` property | line | Dealer vault |
|---|---|---|
| Funds **exactly the cranks its own life requires**, in separate compartments | `:200-219` | ✗ — one undifferentiated counter shared by `Fill` and `Unwind` |
| Every transition draws a **pre-debited** reward | `:396-408` | ~ — the reward is drawn at call time from a pool that may be empty |
| **Re-proves at every transition** that the escrow still covers the work still owed | `:363-394` | ✗ — `validate_state` checks the vault's *bookkeeping* (`:1551-1557`), never its *sufficiency* |
| Unspent escrow refunds to the funder | `:410-430` | ✓ — `retire` returns the remainder to `DealerOwner` (`:1935`) |

The third row is the whole of R4. `candidate_v1` names it in its own voice:

> *"A funded escrow that is only checked when it is created decays into a
> balance nobody can reason about after the first crank. Re-proving it at every
> transition means the remaining lamports are always exactly the remaining
> cranks, so an over-draw is caught at the draw rather than at the last crank
> that finds the compartment empty."*

The Dealer vault is that un-re-proved escrow, and F1/F2 are precisely "the last
crank that finds the compartment empty".

There is also a fourth-row observation worth recording, because it is the
doctrine's sharpest sentence and the Dealer family violates it: *"the solver
signs only to own the escrow and its refund — not to be authorized."* The Dealer
funder **is** the authority — `policy.dealer_id` gates `ScheduleReplacement`,
`AddLiquidity` and `RemoveLiquidity` (`:1642`, `:1956`; enforced on chain at
`dealer-sbf:572-574`). Three of eight actions, where the census names one. That
is a real deviation from the standard, but it is *not* what bricks the market,
and this note does not change it. It is recorded as debt below.

---

## The ruling

**One invariant and one conjunct close F1 and F2. No new action, no new field,
no ABI change, no ELF, no force-exit verb.**

### Ruling 1 — an unwind retires exactly one coordinate

`unwind` refuses unless `request.quantity` equals `state.inventory[outcome]`.

The zero case needs no conjunct here: the wire already owns it (F3). What this
adds is exactness — the terminal walk costs one crank per nonzero coordinate,
never more, because no caller can split a coordinate across calls. That
exactness is what Ruling 2 needs: a reserve cannot be sized against a walk whose
length a caller chooses.

The capability given up is the *partial* unwind. It has no identified use: an
unwind redeems terminal inventory the dealer already owns, no per-unit cost
scales with quantity, and splitting a coordinate across calls only buys more
cranks. The considered alternative — keep partial unwinds but pay the
`work_reward` only on the call that zeroes the coordinate — is strictly more
faithful to *"one reward per execution row"* and is the better long-run shape,
but it makes the payment conditional in both `unwindPost` and `rawCustodyPlan`,
which is a larger change to the Lean model than the defect warrants. Recorded as
an alternative, not as debt: Ruling 1 is sound on its own.

### Ruling 2 — the exit must be affordable in every reachable state

Add to `validate_state` (Rust, `:1541`) and to `valid` (Lean, `:293`) the
conjunct

```
active_work_remaining  >=  work_reward * |{ o < outcome_count : inventory[o] != 0 }|
```

Read it as: **a Dealer state may never hold more inventory than its liveness
vault can pay to unwind.**

The placement is the point. All eight transitions already end by revalidating
the post-state — Rust `:1679, 1726, 1834, 1863, 1910, 1944, 2011`, and Lean's
`execute?` returns `.postInvariantFailure` unless `valid` holds of the post-state
(`:762-764`). So a single conjunct in the one function every transition already
calls is inherited by the whole machine, in both halves, with no per-route edit
and no new proof obligation.

What it does to each finding:

- **F1** — `fill`'s post-state must satisfy the invariant, so a fill that would
  leave the market unable to afford its own exit is refused. The reserve the
  census found missing now exists, and it is enforced at the draw.
- **F2** — `activate`'s post-state sets `active_work_remaining` from the
  incoming candidate's `work_funding` against the standing inventory, so a
  one-crank replacement candidate is refused whenever inventory is nonzero at
  two or more coordinates. The deliberate shrink becomes unrepresentable rather
  than merely discouraged.
- **The walk itself** — with Ruling 1, each unwind decreases both sides of the
  inequality by exactly one term, so the invariant is preserved exactly along
  the terminal walk rather than merely holding at its start.

And the property that closes R4:

> At `enter_terminal`, `active_work_remaining >= work_reward * k` where `k` is
> the number of nonzero inventory coordinates. Each unwind retires exactly one
> coordinate for exactly one `work_reward`. Therefore the full walk to zero
> inventory costs exactly `k * work_reward`, which the vault is holding, and
> `retire` is reachable. **The exit is affordable by construction, in every
> state the machine can reach.**

That is why no force-exit verb is needed, and why a permissionless
liveness top-up action — the obvious alternative fix — should also **not** be
built: it is a bailout route for a state that can no longer occur, and it would
cost a ninth action, a Lean regeneration, a new custody role for the funder, and
a dispatch arm in an ELF, to insure against nothing.

### What this does not close

Ruling 2 makes the exit affordable. It does **not** make the exit *free of the
dealer* in the sense the doctrine's headline sentence wants, because the
underfunded state is now excluded rather than escapable. If a state predating
this invariant were ever founded, nothing here rescues it. Since F4 establishes
that no Dealer has ever been founded on any cluster, that set is empty — and it
must be kept empty: **entry must not be built until this invariant is in place**,
or the window this note is exploiting closes and a genuine migration is needed.
That ordering is the note's one hard constraint on other lanes.

---

## What was executed

Both rulings are landed in both halves, and the two halves are still welded.

**Rust** — `crates/dclutch-dealer-codec`, 44/44 (39 before). The five new cases,
and what each would do without the change:

| case | pins | red without the change |
|---|---|---|
| `a_fill_may_not_spend_the_reserve_its_own_exit_needs` | the reserve, from both sides of one fill: admitted at a vault of 6, refused at 4, the states differing only in `active_work_remaining` | yes — the old rule admits both (4 clears `fill`'s own `work_reward` of 2) |
| `a_replacement_may_not_price_the_standing_inventory_out_of_its_exit` | F2, from both sides: the same 50-funding candidate activates at a reward rate of 20 and is refused at 30 | yes — the old rule admits both (50 ≤ `work_funding`) |
| `an_unwind_retires_one_whole_coordinate` | Ruling 1 — a 49-of-50 unwind refuses | yes |
| `the_exact_reserve_walks_the_whole_way_out_and_retires` | the property itself: a state holding *exactly* the reserve walks every coordinate to zero and reaches `Retire`, ending at a vault of zero rather than short | no — a control, and it is there to prove the invariant is not over-strict |
| `the_wire_owns_the_zero_quantity_rule_for_every_economic_action` | F3's correction — where the guard actually lives, for all four economic actions | no — it pins pre-existing behaviour I had misread |

The boundary pairs are the point. A case that only shows a refusal cannot
distinguish a reserve from an off-by-one, so each of the first two admits the
neighbouring state that differs in exactly one field.

**Lean** — `DClutchSemantics.DealerLiquidity` and `.DealerLiquidityExamples`
build, with a new `native_decide` theorem,
`a_state_may_not_hold_inventory_it_cannot_afford_to_unwind`: the exact reserve is
a valid state, one below it is not, and a fill from the exact reserve **passes
every acceptance conjunct and is still rolled back** — `accepts = true`,
`run = pre` — by the post-state invariant alone. That last clause is the
mechanism stated exactly: what the fill cannot afford is not itself but the exit
it would leave behind.

**One existing Lean fixture was reclassified, and it is the invariant doing its
job.** `hostileUnderfundedWork` — `initial` with a vault of 1 — was asserted
`valid = true`, a state that merely could not afford one more fill. It is now
`valid = false`, because `initial` holds inventory at both coordinates and a
vault of 1 cannot pay the two-crank walk out. Generalised: **being too poor to
fill while holding inventory now implies being too poor to exit**, so that
configuration is gone rather than merely discouraged. The hostile's operational
content is unchanged and still asserted — the fill refuses, nothing moves.

**The weld held.** `crates/dclutch-dealer-codec/tests/generator_fresh.rs` rebuilds
the Lean library, re-runs `EmitDealerLiquidityAbiRust.lean` and compares bytes:
green after the Lean edit, so `generated_dealer_liquidity.rs` is unchanged. The
change is semantic only — no offset, no magic, no width, no action tag moved,
which is what "no ABI change, no ELF" above means measured rather than asserted.

---

## R9 — Custody delivery, verified and ruled separately

R9's citations hold, and the mechanism is confirmed at source:

- **No Clock, no deadline.** The `ACT_*` frame is 20 common accounts plus four
  per effect (`programs/dclutch-custody-sbf/src/dealer_reservation_v1.rs:96-110`)
  and carries **no Clock**. The `CLOCK` index and `Clock::from_account_info`
  appear only on the reserve/rollback legs (`:234`, `:297`). `activate_batch`
  (`:917`) reads no clock and therefore can express no deadline.
- **The caller is unpaid and pays.** `create_activation_receipt_account`
  (`:1418-1450`) charges `ACT_PAYER` for a receipt PDA with no close route,
  while the escrow's recovered rent goes to `ACT_REFUND` — the beneficiary fixed
  at reservation, whom the caller cannot be (`:1346-1352`). The activator is
  net-negative on every delivery.
- **The only exit is forward.** `activate_batch` refuses any checkpoint whose
  phase is not `Committed` (`require_committed_checkpoint`, `:926`, defined
  `:1109`), and
  rollback/cleanup refuse `Committed`, so a committed-but-undelivered
  reservation has no route in either direction.

**Ruling:** R9 is the P1 work-escrow conversion, not a new mechanism. The
delivery crank must be prepaid at `Reserve` — where the reserver is already
paying escrow, state, receipt and batch rent — into a compartment the activator
draws exactly once, with the receipt's rent recoverable by a close route so the
compartment is conserving rather than a donation.

It is **not built in this note**, and the reason is scope discipline rather than
difficulty: it is a Custody ABI change (a new field on the reservation state)
plus a receipt close route, it touches the executed delivery leg of the accepted
campaign, and it is independent of R4 — bundling them would put an ABI change in
the same commit as an invariant that needs none. Costed for the queue: one
Custody ELF, ~200-300 lines, one new route, and four hostiles.

---

## Named debt

- **The dealer funder is the dealer authority.** Three of eight actions gate on
  `policy.dealer_id`. Against the `candidate_v1` standard the funder should sign
  only to own the escrow and its refund. Not load-bearing for R4 and not
  addressed here.
- **Entry (F4).** No on-chain creation route for `Policy`/`Candidate`/`State`.
  Large, and ordered strictly **after** Ruling 2.
- **The LP veto.** `prepare_obligation_close_v3`
  (`programs/dclutch-trading-sbf/src/dealer/v3_obligation.rs:197-206`) refuses
  unless `total_equity_shares() == 0` and every obligation is zero, so one
  sleeping LP share blocks obligation close. This is R3's shape in the Dealer
  family — a sleeping holder blocking retirement — and it should be ruled with
  R3, not separately, because the two want the same terminal-value policy.
- **The family has two runners, and only one of them is advertised.** This is
  not architectural debt — the split is deliberate and both halves earn their
  keep. `run-program-test.sh` builds six ELFs and runs the accepted
  checkpoint/reserve/commit/deliver campaign (31 tests across four targets);
  `tools/gauntlet/dealer/run-dealer.sh` builds five, gates on the toolchain
  reporting zero frame diagnostics, runs the Dealer program's own real-ELF
  refusal campaign (`programs/dclutch-dealer-sbf/program-test/tests/family.rs`),
  then folds evidence, checks witnesses and files a census observation. Only the
  second one ever loads `dclutch_dealer_sbf.so`. The debt is documentary: the
  campaign write-up presents `run-program-test.sh` as *the* one-command
  reproduction for the Dealer family, and a reader taking that at face value
  never builds the Dealer program at all.
- **No accepted Dealer transition is executed against a real ELF anywhere.**
  `family.rs` says so in its own header: it is evidence for the authentication
  prefix, and *"NOT evidence that any Dealer action commits"*. So Rulings 1 and 2
  are provable at the codec and in Lean, and their real-ELF evidence is
  refusal-shaped only. That gap is F4's gap wearing a different hat, and it is
  the honest ceiling on this note's implementation.
