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

### F3 — the specification forbids the zero-quantity unwind; the Rust admits and pays for it

This one is not a design gap. It is a **divergence between the Lean model and
the hand-written Rust**, and the model is the half that is right.

`formal/dclutch-semantics/DClutchSemantics/DealerLiquidity.lean` states the same
conjunct three times:

| Lean predicate | line | conjunct |
|---|---|---|
| `fillAccepts` | `:457` | `0 < fill.quantity` |
| `unwindAccepts` | `:527` | `0 < unwind.quantity` |
| `liquidityChangeAccepts` | `:559` | `0 < change.quantity` |

The Rust transcribed it **once** — `fill` refuses `request.quantity == 0`
(`:1755`). `unwind` (`:1869-1875`) and `adjust_liquidity` (`:1955-1960`) do not.

For `unwind` the consequence is a funded anti-liveness verb. A zero-quantity
unwind subtracts nothing from inventory and pays out nothing, but still runs
`state.active_work_remaining -= active.work_reward` (`:1892`) and
`plan.push(LivenessVault, Executor, active.work_reward)` (`:1897`). `Plan::push`
no-ops on a zero amount (`:1387-1389`), so the two real transfers vanish and the
executor reward is the only transfer that survives.

**In Terminal, anyone may drain the entire liveness vault into their own
account, one `work_reward` at a time, doing zero work, and thereby guarantee the
market can never retire.** `dealer-sbf` adds no quantity guard — it takes the
codec's plan as given (`programs/dclutch-dealer-sbf/src/lib.rs:1905-1915`) — and
`authenticate_actor` (`:564-577`) requires `actor_id == [0; 32]` for `Unwind`,
i.e. it is open to anyone.

The census scored Dealer `Fill`/`Unwind` **GREEN**, *"the tree's second real
caller-funded verb"*. It is GREEN for whether the caller is paid and RED for
what the payment buys. Same family as Y3b: funded **anti**-liveness, not
unfunded liveness.

For `adjust_liquidity` the consequence is milder — the route is dealer-gated, so
it is not a grief — but it is the same dropped conjunct, and it lets a no-op
advance `state_revision` (`:2010`).

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
exists on any cluster, and none can be created by any route that is executed
anywhere in this tree.

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

**One invariant and one conjunct close F1, F2 and F3. No new action, no new
field, no ABI change, no ELF, no force-exit verb.**

### Ruling 1 — an unwind retires exactly one coordinate

`unwind` refuses unless `request.quantity` is nonzero **and** equal to
`state.inventory[outcome]`.

This restores the specification's `0 < quantity` (F3) and additionally makes the
cost of the terminal walk exactly computable: one crank per nonzero coordinate,
never more. That exactness is what Ruling 2 needs — a reserve cannot be sized
against a walk whose length a caller chooses.

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
- **F3** — with Ruling 1, each unwind decreases both sides by exactly one term,
  so the invariant is preserved exactly along the terminal walk.

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
- **The family's one-command runner does not run the family's own program-test.**
  `run-program-test.sh` covers the accelerator campaign only; the Dealer
  transition machine's real-ELF campaign is
  `programs/dclutch-dealer-sbf/program-test/tests/family.rs`, reachable only
  through `tools/gauntlet/dealer/run-dealer.sh`. Two runners, one family.
- **No accepted Dealer transition is executed against a real ELF anywhere.**
  `family.rs` says so in its own header: it is evidence for the authentication
  prefix, and *"NOT evidence that any Dealer action commits"*. So Rulings 1 and 2
  are provable at the codec and in Lean, and their real-ELF evidence is
  refusal-shaped only. That gap is F4's gap wearing a different hat, and it is
  the honest ceiling on this note's implementation.
