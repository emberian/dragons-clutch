# Decision 0017: children read the activation cache — ratifying the answer to the reentrancy wall

Status: **OPEN — ratification requested.** Ledger M-23. The change was
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
> CU. — `docs/board-archive-2026-08-27.md:8199`

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
family (`docs/board-archive-2026-08-27.md:8763-8783`).

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
(`docs/board-archive-2026-08-27.md:8785-8800`):

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
file. `WAVE.md:629` records the *wall as down* in a list of "walls found by
execution, eleven down" — a status line, no authority, no rationale.

## 5. Is it load-bearing? Yes, and reverting is not a cost tradeoff

Nineteen direct on-chain call sites of the crate's five entry points, and
~39 release-set read sites across seven programs
(`docs/evidence/RELEASE_SET_READ_SITE_CENSUS_2026_08_30.md:24`); counting the
program-local wrappers, Claims alone has 14 and Dealer 5. Families converted:
claims, custody, core, dealer, rent (`docs/board-archive-2026-08-27.md:9363-9369`).

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
