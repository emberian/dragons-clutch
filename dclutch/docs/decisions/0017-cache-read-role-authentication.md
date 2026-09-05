# Decision 0017: children read the activation cache — ratifying the answer to the reentrancy wall

Status: **RATIFIED 2026-08-30 — A ratified, C refused. B BUILT AND MEASURED
2026-08-30; the §7 tripwire ships with it.**
§7's recommendation was adopted whole (`DECISION_PACKET_2026_08_30.md` §3,
orchestrator ruling with ember's veto window open; `27f7944b`), including the
condition: ratification ships with the tripwire this record asks for at the end
of §7 — a per-family test exercising a child under a real continuation.
**Option B is no longer sold on a qualitative label**: SEALWIDE measured it at
**52,592 CU, invariant across 32 keys and two builds**
(`docs/design/TRUST_RATCHET_V1.md`, `028f6047`), which makes it ordinary costed
work and the single largest remaining routine CU win. M-23 closes with this
record.

**B landed and the number is bigger than the charter's.** The public Direct
route's key-independent floor fell **1,319,672 → 1,252,751 CU**, a measured
**66,921**, over the same 32 seeds and the same statistic
(`programs/dclutch-trading-sbf/program-test/tests/direct_hot_top_level_margin_gate.rs`,
whose constant fell with it). The ratchet's arithmetic net of ~49,500 was low
because it sized the CPI pair and not the third full cache decode the pair made
redundant — 25 `decode_role` calls, worth about 14,300. The fee-bearing arm fell
by the same amount, 1,501,503 → 1,435,274, which does not make it fit: it is
still 35,274 CU over the ceiling, down from 101,503. See §9.

Ledger M-23, as originally posed: **ratification requested.** The change was
implemented tree-wide on 2026-08-27 and is load-bearing at ~39 sites; the lane
that found the wall yielded the question to ember and it was never answered.
This record asks for the ruling that was skipped, and rules on one residual.

## 1. The question

Was "children authenticate their release set by **reading** the Registry's
activation cache instead of **invoking** the Registry" a decision, or an
accident that stuck — and is it still the right shape?

## 2. What happened, measured

Under a Registry continuation the stack is `Registry [1] → Trading [2] → child
[3]`. Every child role program re-authenticated its release set by CPI-ing
back into the Registry (`RegistryInstructionV1::Reauthenticate`), which the
Solana runtime refuses as `ReentrancyNotAllowed` because the Registry is
already at depth one. It was found by execution, not by reading:

> with the heap and CU ceilings artificially lifted, phase 8+ now reaches the
> FIRST CHILD CPI and dies on `ReentrancyNotAllowed`, not on heap and not on
> CU. — `docs/ledger/board-archive-2026-08-27.md:8199`

The measured shape is carried into the shipped source, which is the best kind
of evidence — `programs/dclutch-claims-sbf/src/lib.rs:1058-1062`:

> This route used to CPI `RegistryInstructionV1::Reauthenticate`. Under a
> Registry continuation the Registry sits at CPI depth one and Claims runs at
> three, so that invocation was reentrancy: Solana refused it after Claims had
> spent 16,033 CU and before it did any work, which made every child of a
> Registry-entered continuation unreachable for every family.

**This was not one route.** `authenticate_releases` made three Registry CPIs,
and the Direct route the gate drives was one of six Claims routes calling it —
so every child of a Registry-entered continuation was unreachable, for every
family (`docs/ledger/board-archive-2026-08-27.md:8763-8783`).

## 3. What the tree does now

`crates/dclutch-registry-activation-auth-v1` (488 lines) replaces the CPI with
four checks the child performs itself: a **PDA derivation** of the cache
address under the Registry program, an **ownership and fixed-width** check, a
**hostile decode** of the cached bytes, and a **live Loader-deployment
observation** re-checked against the cached release
(`authenticate_activation_cache_bump_v1`, `src/lib.rs:183-209`).

The trust statement is preserved rather than weakened, and the crate's own
module doc says why (`src/lib.rs:16-22`):

> The fact the Registry returns from `Reauthenticate` is not privileged
> knowledge held inside the Registry program: it is written in a
> Registry-OWNED account at a Registry-DERIVED address, and every child frame
> already carries that account.

**Two properties are better than the CPI had**, and both matter to the ruling:

- **No drift is structural.** `dclutch-registry-sbf` calls
  `authenticate_activated_role_in_cache_v1` from its own `Reauthenticate`
  handler, so the surviving top-level CPI and every child-local read are the
  same code (`src/lib.rs:47-49`).
- **A check the CPI could not make.** The caller names the release-set
  generation and the cache address is *derived from it*, so a valid cache for
  another Market refuses at its address rather than at a receipt comparison
  someone had to remember to write (`src/lib.rs:41-45`).

**The rule is enforced by deletion, not by a guard — state this plainly.**
There is no runtime depth check anywhere: `get_stack_height` and
`stack_height` return zero hits tree-wide, and `ActivationAuthErrorV1`
(`src/lib.rs:77-95`) has no "you are under a continuation" variant. What
enforces it is that the CPI code was removed from all five child programs with
no fallback — *"There is no fallback path and there must not be one"*
(`programs/dclutch-claims-sbf/src/lib.rs:1064`, and the same sentence in
dealer, core, `signed_delta_v3`, `rational_representation_v2`). The illegal
call is not refused; it is unwriteable without re-adding an import.

Trading is the one program keeping both arms, and it selects between them from
the **instructions sysvar** rather than a depth query: if the top-level program
is Trading it takes `ReauthenticateRegistry`, otherwise the top-level program
must be the Registry and it takes `AuthenticatedContinuation`
(`programs/dclutch-trading-sbf/src/hot_v3.rs:1711-1764`). A
continuation-entered Trading can never reach the CPI — but by a derived
discriminant, not a refusal.

## 4. Decision or accident? Neither, exactly

The lane that found the wall **declined to decide it** and yielded it properly
(`docs/ledger/board-archive-2026-08-27.md:8785-8800`):

> **Recommended: the children should read the activation cache instead of
> invoking the Registry.** … The alternative — stop entering through the
> Registry — discards the Registry continuation, which is the protocol's
> designed authentication shape, so I do not recommend it. **Someone with
> authority over what authenticates a release should say yes to this before
> anyone writes it.**

It was then listed under "Not done, with owners" — *"The reentrancy decision
above. **Needs ember or the protocol owner.**"* (`:8950`). The *next* lane
implemented it tree-wide under cut-the-knot doctrine, titling its report *"THE
RULING, IMPLEMENTED. All five families, CPI DELETED, no fallback."* (`:9347`).

So: **a discovered structural wall, a recommended answer written by the lane
that found it, implemented by the following lane, never ratified by the named
authority.** No ADR records it. Decision 0005 is a *different* cache — the
Trading-owned capability seal — and it cites the activation cache twice as
settled precedent (*"exactly as the Registry activation cache already is"*,
`docs/decisions/0005-per-market-authentication-cache.md:283-288`) without ever
giving or referencing the reentrancy reason; the word does not appear in the
file. `docs/ledger/WAVE_2026-08-26_to_2026-09-02.md:629` records the *wall as down* in a list of "walls found by
execution, eleven down" — a status line, no authority, no rationale.

## 5. Is it load-bearing? Yes, and reverting is not a cost tradeoff

Nineteen direct on-chain call sites of the crate's five entry points, and
~39 release-set read sites across seven programs
(`docs/evidence/RELEASE_SET_READ_SITE_CENSUS_2026_08_30.md:24`); counting the
program-local wrappers, Claims alone has 14 and Dealer 5. Families converted:
claims, custody, core, dealer, rent (`docs/ledger/board-archive-2026-08-27.md:9363-9369`).

Reintroducing a Registry CPI from a child route is **a hard Solana runtime
error, not slower code** — `ReentrancyNotAllowed`, unconditional under a
continuation. There is no measured A/B of cache-read versus CPI under
identical hashing policy, and there cannot be a meaningful one, because after
the wall the CPI was not a slower alternative but an unexecutable one.

The one real CU number in the neighbourhood belongs to decision 0012, not to
this one: the slot pin replaced the whole-ELF re-hash on the cached path at a
measured cost of **+73 CU** on all twenty seeds
(`docs/decisions/0012-devnet-iteration-substrate.md:97`), and that arm lives
entirely inside this crate (`src/lib.rs:422-485`).

## 6. Options

| option | cost | consequence |
|---|---|---|
| **A. Ratify what is built** | zero code | The recommendation becomes a decision; M-23 closes with a record instead of a fourth month of "needs ember". |
| **B. A, plus convert Trading's three surviving top-level CPIs** | one lane, mechanical | `outer.rs::reauthenticate_role`, `direct_begin_retiring_v1.rs:685`, and `hot_v3.rs`'s non-continuation arm are legal (Trading is at depth one there) but pay two CPIs, their account/data vectors, return-data round trips, and a repeated cache search — the syscall audit's open P0 (`docs/evidence/RESOLUTION_RUNTIME_SYSCALL_AUDIT_2026_08_29.md:270-282`, whose payoff labels are explicitly qualitative, not measured). |
| **C. Reverse: stop entering through the Registry continuation** | large | Children could CPI again, at the price of discarding the protocol's designed authentication shape. The lane that found the wall considered and rejected this. |

## 7. Recommendation

**Ratify A now; charter B as a costed lane; refuse C.**

A costs nothing and buys a record — the authentication shape is currently
held up by two board entries in an archived file and a set of doc comments.
B is real but ordinary work whose payoff is asserted rather than measured, so
it should be scheduled on the audit's own terms and measured when it lands,
not sold on the qualitative label. C is refused because the continuation *is*
the authentication design; the wall did not make it wrong, it made the
children's redundant re-ask impossible.

**One thing to keep visible when ratifying:** the enforcement is subtractive.
Nothing refuses a child that CPIs the Registry — the runtime does, at the cost
of the whole transaction. A future contributor who re-adds the import gets a
route that works in every test that does not run under a continuation and
fails on the one that does. If ratification comes with a single piece of
implementation, make it that: a test that exercises a child under a real
continuation for each family, so the wall has a tripwire and not only a
comment. The crate already holds the pattern
(`crates/dclutch-registry-activation-auth-v1/src/tests.rs:246-264`, *"This is
the case the ninth wall refused as reentrancy"*).

## 8. What changes downstream once ruled

- M-23 closes; `WAVE.md`'s wall line gains a pointer to this record so the
  status line stops being the only account of it.
- Decision 0005's two "as the Registry activation cache already is" citations
  gain a referent.
- If B is chartered, the syscall audit's P0 acquires an owner and its
  qualitative payoff becomes a measurement.

## 9. Option B as built (2026-08-30, lane CACHEREAD)

Commits `f04654a0` (the conversion) and `09c1c8fc` (the tripwire), on top of
`3ec7f415`.

### The measurement

**Floor 1,319,672 → 1,252,764 CU, −66,908**, at the key-independent statistic
`min over seeds of (CU(seed) − 1,500 · T_known(seed))` over 32 seeds — the one
`TRUST_RATCHET_V1.md` §8.2 asked for, and the one that survives the fact that
converting the route changes the five ELF digests and therefore redraws every
bump search on it. Before and after were measured by the same script on the same
machine, `tools/gauntlet/hot-cu/cacheread-floor.sh`.

| term | CU |
|---|---|
| the two `Reauthenticate` CPIs (SEALWIDE's measured 26,296 each) | −52,592 |
| the third full cache decode `authenticate_activated_child_programs_v3` paid | ≈ −14,300 |
| the local replacement: two role decodes, two observations, one identity check | ≈ +3,000 |
| **measured total** | **−66,921** |

The third decode is SEALWIDE's own bonus finding arriving as a number: `decode`
validates the complete five-role projection and all ten aliasing pairs, so it is
25 `decode_role` calls, and the route ran it three times over one immutable
1,288-byte account. Seventy-five role decodes, one answer; now twenty-five.

Two other gates moved, both because their subtrahend got cheaper and neither
because anything got worse: `hot_heap_frame_is_inert`'s continuation-versus-
top-level delta floor 36,713 → 103,307 (re-measured over twelve seeds, residuals
on the 3,000 CU grid), and the fee-bearing lower bound 1,501,503 → 1,435,274.

### The conjunction

Every conjunct of `process_reauthenticate`
(`programs/dclutch-registry-sbf/src/lib.rs`) is reproduced locally, and the
deployment half is not a reproduction but the same function object — the
Registry's handler and Trading's arm both call
`dclutch_registry_activation_auth_v1::authenticate_activated_role_in_cache_v1`,
so §3's "no drift is structural" property now covers this arm too.

One conjunct is **stricter**: `authenticate_cache_identity` derived the cache
address from the release set the CACHE names, so a valid cache for another
Market passed it and only the caller's after-the-fact receipt comparison refused
it. `authenticate_activation_cache_identity_v1` derives it from the release set
THIS Market selected, so that cache refuses at its own address. This is §3's
second "better than the CPI had" property, now applied where the CPI survived.

Three caller-side checks became vacuous rather than being dropped: `producer ==
registry.key` has no meaning without a CPI, and `receipt.role()` and
`receipt.program()` are constructions of a local receipt rather than assertions
about a returned one — the latter still required by
`cached_role_deployment_observation_v1` before it observes anything.

**One refusal code changes on purpose.** A superseded deployment slot reaches
the caller as `TradingSbfError::ReleaseSuperseded` (`0x4007`) instead of the
generic `Release` a failed `invoke` could only carry. Decision 0012's
operator-actionable refusal is now sayable on this arm.

### The subtractive wall, now complete

`RegistryInstructionV1` no longer appears in Trading's code at all. §3 described
the enforcement as deletion with no fallback across five child programs; it now
holds for six, and the Registry's `Reauthenticate` route is reachable only from
a top-level transaction — `dclutch-operator` still submits it as an attestation,
so the route is not dead, it is simply no longer invoked by any program.

### §7's condition, in two halves, both seen to fire

`programs/dclutch-trading-sbf/program-test/tests/registry_hot_continuation.rs`:

- **Dynamic.** `claims_and_custody_execute_as_children_under_a_real_continuation`
  reads the runtime's invoke log and requires Registry [1], Trading [2], Claims
  [3], Custody [3], with the transaction succeeding. Demonstrated red: a Registry
  `invoke` restored in Claims' `sparse_native_transfer_v1` produced
  `InstructionError(2, ReentrancyNotAllowed)`.
- **Structural.** `assert_no_family_reaches_the_registry_by_cpi` refuses any of
  the seven role adapters whose code names `RegistryInstructionV1`, over every
  route of each. Demonstrated red: an import added to `dclutch-rent-sbf`.

**What the dynamic half does not cover, measured rather than assumed.** Reading
the invoke log of the canonical Direct Hot continuation: **Core is never invoked
at all** on that route, and Dealer and Rent have no continuation fixture anywhere
in the tree. So the dynamic half is two families of five. Worse, and more useful:
the first attempt at its negative control put the `invoke` in Claims'
`lib.rs::authenticate_activated_role` — the helper thirteen Claims sites share —
and the test stayed GREEN, because this fixture drives
`sparse_native_transfer_v1`, which takes the bump-witness API instead. The
dynamic half covers ONE ROUTE of Claims. The structural half is what covers the
other twelve and the three families with no fixture.

(This record said *fourteen* and *thirteen* when written on 2026-08-30. The
helper had thirteen call sites by then: `9c25e741`, the day before, moved
`founding_v5` onto the bump-witness API. Re-counted 2026-08-31 — the correction
is arithmetic, and it does not move the finding, which is that the dynamic half
covers one route.)

**Still owed, and sized rather than claimed:** a real continuation fixture for
Core, Dealer and Rent. Each needs its own release waist, market and continuation
admission, and there is no existing fixture to extend — `registry_hot_continuation`
drives the Direct Hot bundle and that bundle does not reach them. Estimate 4–8
hours per family, and the honest first question is whether a continuation into
those families is a shape the protocol actually runs, because if it is not then
the structural half is the whole of what the tripwire can be.

### Core: owed above, and closed 2026-08-31 (lane TRIPWIRE)

**"There is no existing fixture to extend" was true of the Direct Hot bundle and
false of the tree, and the difference cost nothing to find.** Core has had a real
Registry continuation since `2dc53776` — the **founding** continuation, Registry
`invoke_signed` into Core `OpenMarket`, which the same 2026-08-30 packet ruling
that demoted the Hot continuation to harness-only carved out explicitly as
load-bearing production (§4, CORESTATE-3). `programs/dclutch-core-sbf/tests/open_market_program_test.rs`
has been driving it with real ELFs the whole time. What it never did was ASSERT
the stack, so the wall was being exercised by a test that would have gone red
about a Custody replay revision.

`core_and_custody_execute_as_children_under_the_founding_continuation` reads the
runtime's invoke log and requires Registry [1], Core [2], Custody [3], with the
market reaching `Phase::Open`. **Demonstrated red:** the deleted CPI restored in
`release.rs::authenticate_continuation_roles` produced
`InstructionError(0, ReentrancyNotAllowed)`, Core dying at depth two on 12,449 CU.

One thing this corrects for whoever takes Dealer and Rent: the reentrancy rule is
**deeper than the Registry**, not "depth three". `registry_hot_continuation`'s
`REENTRANT_CHILD_DEPTH = 3` is right for a topology where Trading sits at two;
under the founding continuation Core sits at two and is exposed exactly as its
children are. Stating it as a fact about the stack makes both topologies one rule.

So the dynamic half is now **three families of five** — Claims and Custody on the
harness-only Hot route, Core and Custody on the production founding route. Dealer
and Rent remain owed at the estimate above. The Claims shared helper is also still
uncovered dynamically and is NOT closed by this: the founding leg drives
`founding_v5`, which takes the bump-witness API, so no founding fixture reaches
the thirteen-site helper either. That needs a continuation into one of
`process_core_effect`/`process_generic_plan`, `affine_batch_v2`, `signed_delta_v3`,
`protocol_position_v2`, `rational_*`, `market_closure_v1` or
`terminal_settlement_v3` — a separate 4–8 hour shape, named rather than absorbed.

### The frame the conversion broke, and what it cost to find

`process_direct_begin_retiring_v1` shipped to main at **4,096 of the 4,096 bytes
SBPF v0 gives a call frame**, with 43 frame-overwrite diagnostics, and it was
this conversion. Fixed in `lane/cacheread-frame-split-20260831`; measured with
`tools/sbf-frame-sizes.py` at every step:

```text
  f6596ffb   caller 3,712   authenticate_market 2,304 out of line, reauthenticate_role 576
  converted  caller 4,096   ONE call site now, so LLVM inlined the two-role read   43 diags
  +inline(never) on the read
             caller 4,352   the read left, and authenticate_market CAME IN         48 diags
  +inline(never) on authenticate_market
             caller 3,712   both out of line, byte-identical to f6596ffb            0 diags
```

Two things in that are worth more than the fix.

**The conversion's own shape caused it.** Folding two role reads into one did not
make the helper bigger; it made it *single-call-site*, and the inliner took it.
The 576 bytes it carries landed on a caller with exactly 384 spare. Nothing about
either function's code was the defect — the call COUNT was, and that is not a
property anyone reviews.

**The first fix made it worse, which is the general lesson.** Splitting one frame
made the caller look cheaper to score, so the inliner swallowed a 2,304-byte
callee it had previously declined. A frame that must stay split has to say so;
the inliner is not a party to the constraint, and relieving pressure in one place
invites it somewhere else. `outer::reauthenticate_roles` therefore carries the
attribute prophylactically: it has two call sites today and is out of line for
LLVM's own reasons, and `outer::process_close` — 3,968, next-deepest, pre-existing
— has 128 bytes to absorb the day that changes.

**And the lane's own gate said zero.** `cacheread-floor.sh` grepped three
plausible-sounding patterns invented from memory; the backend emits none of them.
It reported a confident zero on a build carrying 43 diagnostics. The pattern is
now copied from `tools/gauntlet/run.sh`, which is where it should have come from,
and the script says why in the place someone would next be tempted to paraphrase
it. A checker with a wrong pattern does not fail to answer — it answers no.

### A drift seam this created, named rather than left

`hot_v3::require_activation_cache_account_v3` is now a SECOND spelling of the
identity conjunction the crate owns. It survives because it is the
CONTINUATION's, is `#[inline(always)]` for a measured reason — extracting it as
an ordinary call took the continuation route from passing to
`consumed 1399850 of 1399850` — and a cross-crate call cannot be inlined at all.
Both arms authenticate the same account under the same rule; the clean fix is
for the continuation to afford the crate call, which is a compute problem.

Separately, the continuation arm authenticates its role deployments through
`dclutch-shadow-accelerator-auth-v4::authenticate_activated_current_deployment`
rather than through this crate — a second implementation of the same rule, which
is exactly what §3 says the cache-read shape avoids. It was already true before
this lane and is out of its fence; it is recorded here so it is not discovered
again.
