# Decision 0028: the accelerator output page — a pooled page, cohort-17

Status: **CONFIRMED (ember, 2026-09-04 15:50 EDT, in conversation; reversible
on request) — option (a), the client-provisioned pooled output page, ruled
2026-09-04 14:10 EDT by the orchestrator under the standing goal (see the
addendum), and reversible at the cost §6 states**. It was OPEN until 14:10 EDT
and PROVISIONAL from then until 15:50 EDT, when ember read the docket and
accepted it in conversation without amending it; the confirmation line below is
the whole of what was said. **It was originally OPEN** because ember asked at 10:15 EDT
2026-09-04 for the best architectural course rather than for a switch, so this
record carries a READ as well as a ruling, and decision 0003 is not amended yet
— the addendum states the amendment option (a) carries. Docket item D6. Ember's
words are at `GOAL.md:4655`. The question itself is stated in
`docs/decisions/0003-fixed-role-capability-execution.md` under *"The open
question — docket D6, ember's"*, added by the amendment note of 2026-09-03;
this record does not close it and does not amend 0003. **The fifth condition §3
adds — remeasure the chunk cost on the post-0023 routes before deciding — was
measured by the CHUNK-REMEASURE lane at `faacc7ba8` and is REPORTED in the
second addendum at the end of this record; the tables are
`ACCELERATOR_OUTPUT_CHANNEL_2026_09_02.md` §4. It does not change the read's
direction and it does move one of its numbers by 42 per cent.**

**Confirmed, 2026-09-04 15:50 EDT.** Ember, after reading the docket and the
mechanism cohort page:

> you aren't waiting on me for rulings are you? i was reading the docket and
> contemplating it, but overall find your takes reasonable

The orchestrator's reply: nothing was waiting on ember — the rulings were
provisional and already in force, and the lanes had been working under them
since they were made; *"overall find your takes reasonable"* is taken as
confirmation rather than as an invitation to re-argue them; and the one thing
still genuinely ember's is the flagship conditional market's feature gate, its
slot and its metric (decision 0029's tenth item). So the status above is
CONFIRMED and no longer PROVISIONAL: accepted in conversation, unamended, and
reversible on request at the cost §6 states.

## 1. The question

Put to ember at `GOAL.md:2863-2867`, and put identically by the design note at
`docs/design/ACCELERATOR_OUTPUT_CHANNEL_2026_09_02.md:273-279`, which calls it
*"the only judgment call in the design; the rest is measured or derived"*:

> is an admitted accelerator that owns exactly one client-provisioned,
> digest-bound scratch page — written only inside its CPI, read only by Trading
> in that window, never read by any route — still the "stateless accelerator"
> 0003 admits, or does 0003 need an amendment saying so?

**The wall it answers.** An accelerator hands its result back through the CPI
return channel, which carries at most 880 bytes. A wider bank is chunked, and
Trading loops once per chunk: one CPI each, one release-pinned caller authority
each, and the accelerator re-enters from zero every time
(`crates/dclutch-execution-strategy-contract/src/v2.rs:944-957`, `:1784-1795`;
`ACCELERATOR_OUTPUT_CHANNEL:28-45`). General's batch at N=13 is 3,272 bytes,
four chunks.

**The transport is built for both accelerators and inert.** Nothing flips until
a Strategy record names it (`93bd4f603`, `4f30d4ce8`, `0f53b668a`, `a4c5add46`;
`GOAL.md:3004-3006`).

## 2. Ember's disposition — why this is not a ruling

Recorded at `GOAL.md:4655`:

> D6 — wants the best architectural course understood, not a switch

The docket asked a yes/no. Ember declined the yes/no and asked for the
architecture. So the honest record is an **open question carrying a read**, not
a decision with a status of accepted. Writing it down as ruled would be exactly
the durability failure decision 0018 exists to correct, run in reverse:
recording a ruling nobody made.

## 3. The read

**Option (a), the client-provisioned pooled output page, for cohort-16, after
the chunk cost is remeasured.**

Under four conditions, three of which the design note already states:

1. **Client-provisioned and pooled per Trading root.** The accelerator creates
   nothing and closes nothing; the page is made by a plain
   `SystemProgram::CreateAccount`, rent paid once by the provisioner and reused
   (`ACCELERATOR_OUTPUT_CHANNEL:114-124`). Trading creating, paying for or
   closing the page was priced and **refuted**: the Hot frame has no System
   program and no fee-paying signer among its 39 coordinates
   (`programs/dclutch-trading-sbf/src/hot_v3.rs:52-134`), "closed after" needs a
   second accelerator route, and it adds an unmeasured System CPI per
   transaction (`:97-112`).
2. **Written only inside the accelerator's CPI, read only by Trading in that
   window, never consumed by any route.** The note is honest about what this
   costs: *"A self-owned page is a weaker invariant, and it is honest to say so:
   the accelerator now holds one account. It is not a weaker AUTHORITY: the
   page's bytes are read by exactly one party inside exactly one CPI window and
   bound by a digest the runtime attributes to the writer; no route reads it
   later, it names no semantic owner, and it can move no lamports"* (`:83-93`).
3. **The census law gets stronger, not weaker**: *"every runtime observation
   unchanged, plus page bytes == digest preimage"* (`:93-95`) — a stronger
   statement than the ack's, measured today at
   `tools/gauntlet/general/bindings.json:10`.
4. **0003 is amended to say the accelerator may hold exactly one page under
   those conditions**, so the invariant is restated rather than quietly broken.
   Both accelerators state it today as *"never writes an account, invokes a
   child, or owns protocol state"* (dealer `lib.rs:12`, General `lib.rs:10-11`)
   and 0003 states it as *"may not create a second state, claim, custody, or
   release authority"* (`0003:13-16`). That amendment is ember's to make.

**And a fifth condition the note does not state: remeasure the chunk cost
first.** The measured case for the page is August's: Dealer equity Add chunk 0
at 445,816 CU, *"of which 328,702 (74%) is authentication that is byte-identical
between chunks and 100,064 is the family evaluation that recomputes the same
bank. Only the 880-byte slice differs"* (`ACCELERATOR_OUTPUT_CHANNEL:37-45`).
**Decision 0023's slot-free caller-authority seed and the accelerator prelude
have since taken most of that re-authentication out of every chunk**, so the win
may have shrunk from "one whole chunk" to a smaller number. The decision should
carry the number as it is now, not the one that motivated the design. This is
the one thing in the read that could change the read. **Measured 2026-09-04 at
`faacc7ba8`: still one whole chunk, and the chunk is 42 per cent smaller —
259,537 CU rather than 445,816. See the second addendum below.**

**Why cohort-16 rather than cohort-15.** The request/ack pair *is* the transport
identity (`v2.rs:139-146`), so switching on is *"a new Strategy record content,
not a flag"* (`:130`): it re-digests the cohort's Strategy, Certificate and
Admission records, which makes it a cohort boundary. Cohort-15 is deployed;
cohort-16 already carries a redeploy for the founding changes in decisions 0025
and 0027, and the note observes that a re-digest *"strands nothing that
AGENTS.md's standing full-redeploy grant does not already abandon"*
(`:279-281`).

**What is measured and not in question.** General OpenBatch N=2 runs its whole
bank in ONE CPI at 51,404 CU against four chunks of which one was 50,201; the
Dealer equity Add runs its whole route in one CPI at 455,790 and exceeds the
budget in the tail with 3,773 left — *"the route's own weight, not the
transport"* (`GOAL.md:3006-3009`). Packet and frame: one page coordinate
appended after `ADMITTED_ACCELERATOR_PROGRAMDATA_ACCOUNT_V3`
(`admitted_v3.rs:106-108`), the account count going *minus (chunks − 1) caller
authorities, plus one ALT-eligible page*, and the ALT identity
`accounts(258) − accounts(1) = 2 × pages` becoming `1 ×` (`:132`, `:140`,
`:146-147`).

**The alternatives, priced and refuted by the note itself** (`§2`): (b) a
FrameReference register space — *"a bank diet that fits exactly one route, at
zero margin"*, and a new register kind in every evaluator and the Lean for a win
that still leaves the loop; (c) chunking without re-authentication — *"≥160,904
per extra chunk"*, resting on runtime return-data persistence *"nothing in this
tree pins"*; (d) shrink the bank — fits Add at exactly 880 with zero to spare
and makes Trading a second author of a bank whose whole admission claim is that
the accelerator reproduces TransitionVM's complete output, *"refuted as a
channel"*.

## 4. The lanes this touches

None is chartered on it, because it is not ruled. The blast radius is enumerated
by file at `ACCELERATOR_OUTPUT_CHANNEL:126-140`: Lean `ExecutionStrategyV2Abi.lean`
and its emitter, `v2.rs`, `admitted_v3.rs`, `admitted_composition_v3.rs`,
`hot_v3.rs` frame carving at `:3785-3835`, both accelerator programs, the host
bundle builder, the TS and WASM twins (`generalPlanV5.ts:573`,
`GeneralWorkspace.tsx:161`), the operator, and the extent pins.

## 5. The hostiles that would have to guard it

**The page is the first account an accelerator owns**, so the second hostile walk
must treat it as an authority to adjudicate rather than as a buffer. The
two-slot proof
(`programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs:1190`)
and the campaign margins must be re-run on the one-CPI shape before any cohort
deploys it, because both were measured on the chunked shape.

**The census law is the page's own guard** and it is stronger than what it
replaces: page bytes equal to the digest preimage, checked alongside every
runtime observation.

**The named debt, not a precondition:** rent reclamation needs a Close route the
accelerator does not have — *"a named debt, not a precondition, because a pooled
page is created once"* (`:123-124`).

**Client arithmetic moves.** The browser hand-counts
`FIXED + 8 + scratchPageCount` (`generalPlanV5.ts:573`), one caller authority per
page today; that changes, and neither `AcceleratorAckV2` nor the chunk constants
has a TS twin — *"the browser never decodes an ack"* (`:138`).

## 6. What each answer costs

**Yes** costs a cohort boundary — every Strategy, Certificate and Admission
record re-digests — plus the blast radius above, plus the first account an
accelerator has ever owned, and it buys one CPI per action where General and the
Dealer equity route pay four and six today.

**No** costs nothing immediately: the transport stays inert and the chunks stay,
so the loop and its multiples persist on every General and Dealer equity action.
The note's own summary of the status quo: the invariant *"the accelerator owns
no account"* stays literally true.

**Deferring** costs the measurement's freshness. The chunk numbers are already
stale by one ruling; each further cohort makes the August figures a worse basis
for the choice.

## 7. What would make this a decision

A ruling by ember, recorded here and as a real amendment to 0003's §Decision —
which `0003`'s own note says is *"the precondition for the transport being
switched on"*. Until then this record's status is OPEN, and `GOAL.md:3006` and
`:3059`, which both refer to *"ember's 0003 ruling"* as already given, are
referring to the question having been **put**, not answered.

## Addendum, 2026-09-04 13:20 EDT: the fifth condition is being measured, and the read has gained a second consumer

**The record stays OPEN.** Nothing here rules the question; this section records
what is being done about the one thing §3 says *"could change the read."*

**§3's fifth condition is now a lane.** CHUNK-REMEASURE was started at 13:20 EDT
(`GOAL.md:4818-4820`) to measure the chunk cost **on the post-0023 routes**, three
draws each. The read's case for the page is August's -- Dealer equity Add chunk 0
at 445,816 CU, *"of which 328,702 (74%) is authentication that is byte-identical
between chunks"* -- and decision 0023's slot-free caller-authority seed plus the
accelerator prelude have since taken most of that re-authentication out of every
chunk. **The decision should carry the number as it is now, not the one that
motivated the design**, and until the lane reports, this record's §3 is a read
resting on a figure the tree has already moved.

**And the read has a second consumer it did not have when it was written.** The
joint-clearing design (decision 0032, `MECHANISM_JOINT_CLEARING:358-389`) prices
D6 for the batch: the output page removes `chunks − 1` re-evaluations per
transaction, **−22 % at `K ≤ 13`, −40 % at `K = 60`**, taking a General
verification transaction from ≈ 0.67 M CU to ≈ 0.53 M, and flattening the
K-dependence to one evaluation. At `N = 258` that is ≈ 395 M CU against ≈ 313 M
per batch. The note also observes that D6, if ember rules it in, **rides the same
cohort-17 boundary** the clearing rule needs, so the cohort cost §3 attributes to
the page is shared rather than additional.

Neither fact changes the question, and this addendum does not answer it. What it
changes is that a decision described in §7 as needing *"a ruling by ember"* now
has a measurement in flight and a second beneficiary, and both belong in front of
whoever rules it.

## Addendum, 2026-09-04 17:05 EDT: the fifth condition, measured

CHUNK-REMEASURE, at `faacc7ba89480349b2ceff2f331630ab69c0a5b4`, in a detached
worktree, on real ELFs, three draws for every Dealer row and an unprofiled
control for every whole-transaction figure. The tables are
`docs/design/ACCELERATOR_OUTPUT_CHANNEL_2026_09_02.md` §4; this is the number
§3's fifth condition asked for and nothing more. **The record stays OPEN. The
lane did not rule and this addendum does not.**

**The number.** The Dealer equity Add, measured end to end on both transports
through the same campaign fixture, unprofiled ELFs: **1,120,274 CU chunked
against 860,737 CU on one page — the page saves 259,537 CU per action.** The
read's case was one whole 445,816-CU chunk. **It is still one whole chunk, and
the chunk is 42 per cent smaller.**

**Where the missing 186,000 went, and where it did not.** Not to decision 0023,
whose own record says *"What it saved: Not CU — statability"* and which this
measurement confirms: `acc-caller-authority` is 3,529 to 5,030 CU and every
movement in it is a whole `create_program_address` iteration. It went to the
prelude move (`742d7b7be`) and to `5709672aa`, whose activation-cache repair
alone is 16,509 CU of it. A chunk's authentication fell from 328,702 to
117,110–118,610 and its family evaluation from 100,064 to 73,555–76,555.

**The wall §1 states got sharper, not weaker.** The 880-byte slice — the only
part of a chunk that a chunk index changes — is **1,562 CU**, identical to the
digit across three draws and across both legs of one transaction. So **195,165
of a 196,727-CU chunk, 99.2 per cent, is byte-identical between chunks**, where
the note could previously only say 74 per cent.

**What the smaller number changes for the read.**

- **The equity Add loses its execution argument.** Under the chunked pair it now
  completes, at 1,120,274 of 1,399,700 with **279,426 CU of headroom**, where in
  August its second chunk had 45,764 left and needed 445,816. For the Add the
  page is margin, not the difference between running and not running.
- **The equity Remove does not.** Its bank is three chunks. On the page it
  completes at 1,232,300–1,253,268 of 1,399,700; the same per-chunk arithmetic
  puts its chunked form near 1.85M, which no headroom reaches. Derived, not
  measured — the campaign's chunked run aborts at the Add.
- **General does not.** Its accelerator never ran the admitted prelude, so
  nothing the prelude move did could reach it: one chunk is 50,475 CU in the
  isolated harness against `a4c5add46`'s 50,201, and 66,167–75,194 CU through
  real Trading, where OpenBatch pays four — **210,555 CU, 30 per cent of the
  action**. The page removes three of them. That saving is DERIVED and still not
  measurable end to end, because the General operator cannot build a page frame
  (`0f53b668a` named that as owed and it remains owed).

**The first addendum's second consumer survives this.** Decision 0032 prices D6
for the batch at −22 per cent at `K ≤ 13`. The measurement here is the same
order for a different action and by the same mechanism — three of General's four
chunks, 30 per cent of an OpenBatch — so nothing in the joint-clearing case rests
on the Dealer figure that moved.

**Two of §5's and §6's own numbers move with it.** §6's *"one CPI per action
where General and the Dealer equity route pay four and six today"* stands for
General, whose bank is four chunks at every Product width, and the Dealer equity
Add pays two and the Remove three. And §5's ALT identity
`accounts(258) − accounts(1) = 2 × pages` is **vacuous for OpenBatch**: its
account count is 55 at N=2, N=13 and N=258, because the two batch actions declare
a zero per-outcome stride and their bank is 2,648 bytes at every width. The
identity still binds for the thirteen actions that declare a stride of 6, and for
those the page changes its coefficient from 2 to 1.

## Evidence pointers

`docs/design/ACCELERATOR_OUTPUT_CHANNEL_2026_09_02.md` (whole; esp. `:28-45`,
`:61-147`, `:149-217`, `:233-241`, `:248-261`, `:273-281`, and §4, the
2026-09-04 remeasurement at `faacc7ba8`);
`docs/decisions/0003-fixed-role-capability-execution.md:10-16` and its 2026-09-03
amendment note; `docs/decisions/0023-slot-free-caller-authority-seed.md`;
`GOAL.md:2863-2867`, `:3004-3011`, `:3059`, `:4655`, `:4818-4820`;
`docs/design/MECHANISM_JOINT_CLEARING_2026_09_04.md:358-389`;
`docs/decisions/0032-joint-clearing-residual-tie-break-and-seal.md`;
`crates/dclutch-execution-strategy-contract/src/v2.rs:130`, `:139-146`,
`:944-957`, `:1784-1795`;
`programs/dclutch-trading-sbf/src/hot_v3.rs:52-134`, `:3785-3835`;
`apps/dclutch-web/lib/generalPlanV5.ts:573`;
commits `93bd4f603`, `4f30d4ce8`, `0f53b668a`, `a4c5add46`.

## Addendum 2026-09-04 14:10 EDT — the fifth condition measured; a provisional ruling

The CHUNK-REMEASURE lane (`1b2bd47fa`) measured the chunk cost at `faacc7ba8` on the post-ruling routes, three draws
each, profiled and unprofiled: the Dealer equity Add's accelerator chunk is **216,252 / 214,752 / 211,752 CU** (August:
445,816), of which **195,165 of a 196,727-CU chunk — 99.2% — is byte-identical between chunks**; the 880-byte slice and
ack cost 1,562 to the digit. The page saves **259,537 CU per equity Add — still one whole chunk, 42% smaller** — and the
Add now executes chunked with 279,426 headroom, so the page is margin on that route, not the difference between running
and not. General OpenBatch at N=13: four chunks = 210,555 CU, **30% of the action**; its chunk has not moved since 09-02
(its accelerator never runs the prelude). Packet: General 55 → 53 accounts; the Dealer Add net zero.

**Provisional ruling (the orchestrator under the standing goal; ember may reverse):** option (a) — a client-provisioned,
pooled output page under the four conditions in this record, amended into decision 0003 as "the accelerator may hold
exactly one page under those conditions" — rides cohort-17 with the mechanism cohort (0031), since it re-digests every
Strategy record and cohort-17 already carries the order-layout move. The number that decided it is the 30% on every
General action, which the joint clearing multiplies by 9 + 4M transactions per batch; the Dealer's 260k is margin.
Status: PROVISIONAL (was OPEN).
