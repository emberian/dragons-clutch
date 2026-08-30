# The Registry outer composition for Direct Hot: fix, retire, or keep

Prepared for a ruling. The decision is ember's; this document supplies the
measurements it needs and one recommendation.

Measured at `/Users/ember/dev/dclutch`, commit `3dde1b9c`, on a clean
`git archive` export — never the shared working tree, which several other lanes
were writing to under `programs/` and `crates/` while this ran. All eight SBF
artifacts were built from that archive by
`tools/ci/run.sh programs --commit HEAD`. Trading ELF sha256
`aea436740301bc3ffb9268480037e787aef51eff1c7b3c5a8e02d7edd3393d08`; Registry
`18e17e63ee1443b740ec4e831fe24d1143c9072e76c61e83eacc506405d37fe6`.

---

## 0. The two answers

**Why `hot_heap_frame_is_inert` is red: the Registry continuation route does
not fit the runtime's 1,400,000 CU ceiling.** Not the heap. At fixture seed 0
— the seed the test runs by default — the Registry outer consumes 1,399,794 of
1,399,850 and the transaction dies `InstructionError(2,
ProgramFailedToComplete)`, with Trading inside it reporting `exceeded CUs meter
at BPF instruction`. Across 32 pinned seeds on those ELFs the route **passes 13
and fails 19**. The heap question the file is named for is settled and stayed
settled: the route completes its allocation work at the protocol default, and
removing the heap grant entirely changes none of the eight seeds it was tested
on from pass to fail or back.

**What the outer composition buys, for this route: nothing that the top-level
route does not already have.** It authenticates the same two roles (Core and
Trading) over the same activation cache, hands Trading the same children, and
executes the same trade with the same child costs to the compute unit. It is
not a capability boundary — it is a second implementation of one. For that it
charges a measured **+35,127 CU, identical on all thirteen seeds where both
routes complete**, and it adds a `find_program_address` search whose depth is
an independent coin flip worth up to another 12,000 CU. No caller outside the
program-test harness constructs it: not the SDK, not the web app, not the CLI,
not the devnet drivers, not the relayer, and not any binary in this tree.

---

## 1. The control, stated before the findings

A red measurement is worth nothing without a green one taken the same way. The
control is a **matched pair**: the same eight ELFs, the same fixture, the same
pinned seed, the same harness, the same machine, one variable changed — the
route.

| | continuation (`hot_heap_frame_is_inert`) | top-level (`direct_hot_top_level`) |
|---|---|---|
| seed 0 | **FAILS**, meter exhausted | **PASSES**, 1,373,063 CU |
| 32 seeds | 13 pass / **19 fail** | **31 pass** / 1 fail (seed 13) |
| passing range | 1,382,675 – 1,397,675 | 1,347,548 – 1,397,047 |
| passing mean | 1,391,328 | 1,365,112 |
| worst passing margin | **2,325 CU** | 2,953 CU (seed 15) |

So the failure is the route. It is not the build, not the box, not a
franken-tree, and not the harness — those are shared by the column that passes.

Both routes are at the wall. The top-level route's own single failure at seed
13 (1,399,692 of 1,399,700) is the same draw CI-2 hit at 14:10, reproduced
here independently on ELFs built from a different commit. That is wall #28 and
it is ruled elsewhere. What this document is about is the 35,127 CU that
separates the two columns.

---

## 2. Why the test is red

### 2.1 The failure, exactly

```
Program <Registry> invoke [1]
  Program <Trading> invoke [2]
    Program <Claims> consumed 155940 of 449707 compute units
    Program <Custody> consumed 142517 of 267442 compute units
  Program <Trading> consumed 1302516 of 1302572 compute units
  Program <Trading> failed: exceeded CUs meter at BPF instruction
Program <Registry> consumed 1399794 of 1399850 compute units
Program <Registry> failed: Program failed to complete
```

The test's own error taxonomy does not cover this. It checks for
`InstructionError::Custom(TradingSbfError::Content)` — the named heap refusal —
and panics on anything else, which is correct: a compute exhaustion is not a
refusal this program authored.

```
panicked at tests/hot_heap_frame_is_inert.rs:
  Hot refused outside its own error taxonomy:
  InstructionError(2, ProgramFailedToComplete)
```

The instruction index is 2, which is the Registry outer. The packet assertion
above it passes: the wire is still 1,206 bytes of the 1,232 limit. Nothing
about the heap, the aliases, the fixture or the wire has drifted.

### 2.2 The 32-seed picture

| route | pass | fail | failing seeds |
|---|---:|---:|---|
| Registry continuation | 13 | **19** | 0, 3, 4, 5, 6, 9, 10, 11, 12, 13, 15, 16, 18, 20, 23, 25, 26, 28, 30 |
| top-level Direct | 31 | 1 | 13 |

The file's own header records "one failure in twenty" against an older ELF.
`tools/gauntlet/blocked.json` records "the seeded compute instrument has passed
20/20 under the 1,400,000-CU ceiling" as the stated reason for two blocked
routes. Both are stale by more than an order of magnitude, and §5.1 says what
to do about them.

### 2.3 Where the compute goes

The two routes differ in exactly one place, and it is measurable on both sides
of the boundary.

**Top-level.** Trading is invoked directly and makes two Registry
`Reauthenticate` CPIs. They cost **27,757 + 27,756 = 55,513 CU**, and that
figure is constant on every one of the 32 seeds (the cache address is handed
in, so nothing searches).

**Continuation.** The Registry outer authenticates the same two roles itself
before invoking Trading. Its prologue cost is the difference between its own
budget and the budget Trading starts with:

| outer prologue | seeds |
|---:|---|
| 95,778 | 21 seeds (the modal draw) |
| 97,278 | 6 |
| 98,778 | 2 |
| 100,278 | 1 |
| 101,778 | 1 |
| 107,778 | 1 |

Every value is `95,778 + 1500·k`. That is the admission PDA's
`find_program_address` bump search — `RegistryContinuationAdmissionSeedsV1`
derives a seven-seed address, and each rejected bump costs 1,500 CU. **The
continuation introduces a second independent bump draw into a transaction that
has 2,325 CU of margin at its best passing draw.**

The end-to-end arithmetic closes:

```
outer prologue (best draw)                  95,778
  less the two reauthentication CPIs       -55,513
  less what Trading saves by not making
  them itself (instruction, metas,
  return-data decode)                       -5,138
                                          ---------
  net penalty of the continuation           35,127   <- measured, 13/13 seeds
```

The children cost the same on both routes to the unit — at seed 0, Claims
consumes 155,940 either way — which is what makes the delta attributable to the
boundary rather than to the trade.

### 2.4 The heap grant is no longer inert on this route, by 517 CU

The test's premise, and its function name, is that a `RequestHeapFrame` grant
does not touch the continuation. Measured against a variant of the test with
the grant removed (built and run in the same archive, same ELFs):

| seed | with grant | without grant | delta |
|---|---:|---:|---:|
| 1 | 1,397,673 | 1,397,156 | +517 |
| 21 | 1,382,675 | 1,382,158 | +517 |
| 22 | 1,397,675 | 1,397,158 | +517 |

The mechanism is that `entrypoint_adapter::declares_extended_heap_profile_v1`
keys on the Hot magic in the **instruction data**, and
`registry/hot_continuation_v2::process` forwards `instruction_data.to_vec()` —
byte-for-byte the same Hot bytes. So the declaration cannot tell the two routes
apart. Commit `8ee544e4` put `DCLTHOT3` on that list for the top-level route
and its message states "The continuation route is unchanged and must stay so";
that is not what the code does. Trading under a continuation now also runs
`lift_declared_heap_profile_v1`, scans the instructions sysvar, and lifts its
ceiling.

**This is not what made the test red** — with the grant removed, seed 0 still
exhausts the meter. It is 517 CU of a 35,127 CU problem. It is recorded because
the test's name now asserts something false, and because a route-blind
declaration is the kind of thing that is cheap to fix while someone is looking
at it and expensive to discover later.

### 2.5 Which commit made it red — and why that is the wrong question

I did not bisect, and the reason is a decision rather than an omission.
`docs/evidence/TRADE_DIRECT_ACTIVATION_WALL_2026_08_29.md` records this exact
route being bisected at a single seed, producing a confident and specific wrong
culprit (`df404c56`), refuted by a control showing the parent commit already
failed 4 of 12 seeds. A route that fails 19 of 32 does not have a commit that
broke it; it has a cost curve that crossed a ceiling, and a bisect over a
59%-failing route identifies whichever commit flipped the one seed being
watched.

What can be attributed, and is: the pass rate at `49da8191`, the immediate
parent of the heap-profile commit `8ee544e4`, over the same 32 seeds, on ELFs
built from that commit by the same runner (Trading
`5399e4b19a6f324d8cf6ac9f627999f478969e52a0d17090273e6f21bdb0873d`).

| commit | continuation pass rate | passing seeds |
|---|---:|---|
| `49da8191` (`8ee544e4^`) | **6 / 32** | 9, 13, 17, 19, 25, 29 |
| `3dde1b9c` (HEAD) | **13 / 32** | 1, 2, 7, 8, 14, 17, 19, 21, 22, 24, 27, 29, 31 |

**The route is better at HEAD than it was before the heap commit, by seven
seeds.** So nothing in that range made it red: it was already red at more than
twice the rate, and the commits since have taken cost out of it — more than
paying back the 517 CU of §2.4. And the two seed sets overlap on three
elements, which is ledger M-61 stated as data: a different ELF redraws every
seed, so a per-seed comparison across commits carries no information and only
the rate is a fact.

One thing the attempt turned up on the way and is worth a line to whoever hits
it next: `65c6fc15` does **not** build from a clean archive.
`cargo build-sbf` on `programs/dclutch-trading-sbf` at its default feature set
fails there with four `E0432`s — `hot_v3.rs` imports `claims_composition_v3`,
which `lib.rs` gates behind `families`/`series-family`/`dealer-family`. Later
commits build. That commit is not a usable bisect point.

The honest statement is the pass-rate curve, not a culprit: 19 of 20 seeds
passing against ELF `14b22a31bb9cabf7…` when the file's header was written,
3 of 12 over the ceiling at `fd8cad39` in the wall document, 13 of 32 passing
at `3dde1b9c`. The route has been failing for arbitrary keys for weeks and
nothing ran it.

---

## 3. What the outer composition buys

### 3.1 It authenticates the same two roles

`RegistryContinuationRequestV1::new_core_trading_hot` fixes the role batch to
`CORE_TRADING_HOT_CONTINUATION_ROLES_V1 = [Core, Trading]`
(`crates/dclutch-registry-svm/src/continuation_v1.rs`). Those are exactly the
two roles `hot_v3::reauthenticate_top_level_root_roles_v3` re-authenticates by
CPI on the top-level route. Same roles, same activation cache, same live
Loader observation, same generation binding. Under ADR 0017 both sides run the
same code: `dclutch-registry-activation-auth-v1` is what the Registry's own
`Reauthenticate` handler calls, so the receipt is not privileged knowledge held
inside the Registry program.

The continuation carries the receipt as an ephemeral admission PDA signer
instead of as CPI return data. That is a different transport for the same fact.

### 3.2 The one behavioural difference, and it is a relaxation

`authenticate_hot_invocation_v3` returns `permits_fixed_market_union: true`
only on the continuation arm, which lets `HotFrameV3::parse` accept a
**writable** Market account that the top-level arm refuses.

Nothing on the Hot path writes the Market. `authenticate_market` borrows it,
decodes it, re-encodes it and compares the bytes; it is a read. The relaxation
exists so that a composed outer frame may present the Market with elevated
physical privilege, and its only exercise in the tree is the unit test
`registry_continuation_authenticates_admission_and_market_union`, which
constructs the writable case to assert the parse accepts it.

So the continuation's one distinguishing property is a **weaker** check than
the top-level route runs, on an account neither route writes.

### 3.3 What it costs the rest of the protocol

Under a continuation the Registry sits at CPI depth one, so every descendant is
forbidden from invoking it — `ReentrancyNotAllowed`, unconditional. That wall
is what forced the tree-wide conversion of five child programs to reading the
activation cache instead (ADR 0017, ~39 release-set read sites across seven
programs). Enforcement is subtractive: nothing refuses a child that re-adds the
CPI, the runtime does, at the cost of the whole transaction.

That constraint is not removed by retiring the *Hot* continuation. Market open
runs through `open_market_continuation_v1` (Core + Custody), so children still
execute under a Registry at depth one, and the cache-read discipline stays
load-bearing either way. Whatever is ruled here, ADR 0017's ratification is
unaffected.

### 3.4 Who uses it: nobody outside the harness

Swept across the whole repository — SDK, web app, CLI, devnet and local
validator drivers, gauntlet campaigns, the relayer, every operator crate and
every binary.

| surface | route it builds |
|---|---|
| `packages/dclutch-sdk/lib/directInlineV3.ts` `compileDirectInlineTransactionV3` | top-level (`programId` = Trading) |
| `packages/dclutch-sdk/lib/directWalletPreparationV1.ts` | top-level |
| `apps/dclutch-web/components/MarketTradePanel.tsx` — the only browser-wallet submission in the tree | top-level |
| `tools/local-validator/bootstrap/successor/src/direct_trade.rs` (devnet + local drivers) | top-level |
| `tools/load-simulator/simulator.py` | top-level, via the driver |
| `tools/local-validator/.../family_hot_campaign.rs` (General/Series) | top-level |
| `tools/relayer` | builds no Hot instruction at all |
| `crates/dclutch-operator/src/registry/hot_continuation_v{1,2}.rs` | **continuation — and its only callers are its own tests** |

`validateDirectInlineInstructionSequenceV3` hard-refuses anything but the
four-instruction top-level shape, and `directHotRouteManifest.ts` models
exactly 39 fixed accounts with no admission row — **the continuation frame is
not representable in the SDK's types.** The only TypeScript in the tree that
knows the route exists is the explorer's instruction decoder, which reads it
and never builds it.

Inside the harness it is the dominant route, and this is the real cost of
retirement: `registry_hot_continuation.rs` (the hostile suite, including
`late_custody_refusal_rolls_back_registry_hot_claims_and_lifecycle` and the
four `real_registry_refuses_*_atomically` cases), `hot_tail_profile.rs`,
`slot_pin_supersession.rs` and `hot_heap_frame_is_inert.rs` all drive it.
Those tests carry protocol properties that must not be deleted with the route —
they must be re-pointed at `direct_top_level_instructions`, which the same
harness already provides.

### 3.5 The project's own Hot-CU instrument measures this route, not the shipped one

`tools/gauntlet/hot-cu/run-hot-cu.sh` is the tier that produces the "Hot CU"
figures this project quotes, and its witness is `hot_heap_frame_is_inert` — so
**every number it has ever printed is the continuation's, not the public
route's.** Its README says so plainly ("the compute a Hot continuation
consumes"), which is why this is a consequence rather than an accusation: the
instrument is honest and the inference drawn from it downstream is not.

Two things follow immediately. Under the instrument's own reporting rule it now
reports `PASS 13/32` and, correctly, no MEAN at all — the rule only permits one
at N/N. And the route it measures is 35,127 CU more expensive than the one the
web panel submits, so any budget reasoning that took a hot-cu figure as the
public route's cost has been reasoning about the wrong route in the
conservative direction. `direct_hot_top_level_margin_gate.rs` is the gate that
covers the shipped route, and it is a different test on a different route with
its own constant.

### 3.6 What it does not buy, said explicitly

- **Not a capability boundary.** Same roles, same cache, same children.
- **Not packet room.** The continuation is 1,206 bytes against the 1,232 limit
  with a heap request aboard; the top-level route is 1,167 bytes with four
  instructions. Both fit.
- **Not composition headroom for other callers.** Any program can wrap the
  top-level Hot instruction the same way; what the Registry outer adds is its
  own role authentication, which Trading performs anyway.
- **Not the continuation primitive.** `RegistryContinuationRequestV1` is
  general over role batches and is used by market open and market retirement.
  Retiring the Hot continuation retires one consumer, not the mechanism.

---

## 4. Options

Sizes are estimates with numbers, not walls.

**A. Fix the continuation's compute so it fits for arbitrary keys.**
The requirement is not 35,127 CU. The worst passing top-level draw is 1,397,047
(seed 15), so a continuation of that seed would need `1,397,047 + 35,127 =
1,432,174` — 32,174 over the ceiling — and the admission bump can add up to
12,000 more on top of that. Making the continuation fit wherever the top-level
route fits therefore means removing **roughly 44,000 CU from a 95,778 CU
prologue**, close to half of it. Levers found while measuring:

| lever | measured or estimated saving |
|---|---:|
| carry the admission bump instead of searching for it | 0 on the modal draw, up to 12,000 on the worst |
| the activation cache and the Hot bytes are hashed on *both* sides of the CPI (Registry, then again in `authenticate_hot_invocation_v3`) | ~1,500 |
| the duplicated two-role authentication itself | unmeasured; it is the bulk of the 95,778 |

No identified lever reaches the requirement, and the largest term is the one
that is duplicated work by construction. **I would not charter this without a
costed plan.** Estimated: two lanes to find out whether it is possible, with a
real chance the answer is no.

**B. Retire the Registry Hot continuation outright.**
Delete `registry/hot_continuation_v2.rs` and the `continuation_v1` Hot arm,
Trading's `AuthenticatedContinuation` arm and `permits_fixed_market_union`,
`crates/dclutch-operator/src/registry/hot_continuation_v{1,2}.rs`, and port
roughly 20 program-tests to `direct_top_level_instructions`. Estimated: **one
to two lanes**, mechanical but not small, and it is an ABI removal — the
Registry stops accepting `DCLTHOT3`. Two things must be measured afterwards
rather than assumed: both programs shrink, and under ledger M-61 a one-byte ELF
difference redraws every fixture seed, so the top-level margin gate must be
re-swept at 32 seeds after the deletion, not before.

**C. Keep it and leave the test red, or `#[ignore]` it.**
This is the option the wave has already named: a gate that cannot fail is
decoration, and a red one nobody may fix is worse — it trains every lane to
read a red suite as normal. Costs nothing today and costs the next reader their
trust in the suite.

**D. Keep the route, and re-bar the test on what is actually true.**
Change `hot_heap_frame_is_inert` from asserting the absolute ceiling at one
seed to asserting the **delta against the top-level route**, which is exactly
constant at 35,127 CU across 13/13 comparable seeds — a tighter and far less
drift-prone invariant than any absolute figure on this path — plus one named
passing seed, labelled as chosen. Estimated: **one lane-hour**, no ABI change.
This tells the truth without deciding which route is production.

---

## 5. Recommendation

**Rule the top-level route the production route for Direct Hot, and demote the
Registry Hot continuation to harness-only. Take D now; do not take A; hold B
until the tests are ported.**

The reasons, in the order they carry weight:

1. **The ruling is already made in fact, everywhere except in writing.** The
   SDK cannot express the continuation, the web panel does not build it, the
   devnet drivers do not build it, and the operator functions that do build it
   have no caller. Ratifying that costs one sentence and stops the project
   spending optimization effort on a route no external caller can reach.
2. **The continuation is the more expensive of two routes for identical
   semantics**, by a constant that does not depend on the keys. There is no
   argument from capability to set against that, because §3.1–§3.2 found none —
   its one behavioural difference is a relaxation of a check on an account
   neither route writes.
3. **B is right eventually and wrong this week.** The hostile suite that drives
   it is the strongest coverage this route has, and deleting the route before
   porting those tests would trade a compute problem for a coverage hole.
4. **D makes the tree stop lying today** at one lane-hour, and the invariant it
   installs (the constant 35,127 CU delta) is the number a future ruling on B
   or A would want watched anyway.

One consequence to take with the ruling rather than discover afterwards:
`tools/gauntlet/hot-cu` is pointed at this route (§3.5). Demoting the
continuation to harness-only means that tier is measuring a harness route, so
it either moves to `direct_top_level_instructions` or its README says out loud
that its figure is not the public route's. Half a lane-hour either way, and it
belongs to the gauntlet owner, not to this lane.

### 5.1 Three corrections that follow from the measurement, whatever is ruled

- `hot_heap_frame_is_inert.rs`'s header records "one failure in twenty". It is
  nineteen in thirty-two. Corrected in this commit; the assertions, the test
  name and the behaviour are untouched, because those belong to the ruling.
- `tools/gauntlet/blocked.json` entries for
  `trading/hot_v3::process_hot_execution_v3` and
  `registry/hot_continuation_v2::process` both state "the seeded compute
  instrument has passed 20/20 under the 1,400,000-CU ceiling" as their stated
  reason. That is now false by a factor of nineteen. **Not corrected here** —
  the file belongs to the gauntlet owner and the wording is part of a blocked-
  route contract, not a stray comment.
- `8ee544e4`'s claim that "the continuation route is unchanged" is false by 517
  CU (§2.4). Making `declares_extended_heap_profile_v1` route-aware requires
  reading the instructions sysvar to know the route, which is a judgment about
  which route is production — so it belongs to the ruling and not to a
  tidy-up.

---

## 6. What was not verified

- **No bisect.** §2.5 says why, and names the precedent that makes it a bad
  spend on this route.
- **The failing seeds' true cost is unknown.** A seed that exhausts the meter
  reports consumption up to exhaustion, so 19 of 32 continuation figures are
  censored at 1,399,794. The +35,127 delta is measured on the 13 seeds where
  both routes complete; it is not established for the other 19, and seed 18's
  arithmetic (top-level 1,364,047 + 35,127 = 1,399,174, which is inside the
  budget, yet it failed) says the delta is not constant everywhere — its outer
  prologue drew 100,278 rather than 95,778.
- **One substrate, one host.** `Immutable`, one machine, `solana-program-test`.
  No validator run, no devnet transaction. No devnet writes were made.
- **The 95,778 CU prologue was not attributed internally.** Option A's largest
  term is unmeasured; the table says so rather than estimating it.
- **`hot_v3.rs` was read and not modified** — an incoming carry wave owns it.
- **The porting estimate in option B is a count of test functions, not a
  compile.** Nothing was ported.
