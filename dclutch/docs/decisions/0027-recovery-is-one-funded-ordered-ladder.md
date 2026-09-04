# Decision 0027: recovery is kept, as one funded ordered ladder that exhausts into the failure selector

Status: **PROVISIONAL — ruled by the orchestrator on 2026-09-04 under ember's
standing goal, amended by ember at 10:15 EDT to require robust failure pathways,
and reversible at the cost §7 states**. Docket item D5. Ember's amendment is at
`GOAL.md:4654`. This record answers the contract's `Recovery ontology: keep or
cut` register row (`docs/MASTER_COMPLETION_CONTRACT.md:187`), and it is the
companion half of decision 0025: the ladder is the pathway in, the escrow is the
pathway out. **The transition system landed the same morning at `332b432e6`
(lane RECOVERY), Lean first and with no on-chain route yet; §5 records which of
this record's properties are now theorems.**

## 1. The question

The contract puts it compactly: **does dClutch intend markets that buy named
alternative sources?**

The ontology is live, green and kernel-complete with no route.
`RecoveryPolicyV2` and `RecoveryAttemptV2` are 385 lines of contract
(`crates/dclutch-source-contract/src/source_recovery_policy_v2.rs:25`, `:118`,
`:174`), with a Lean-owned emitted twin
(`generated_source_recovery_policy_v2.rs`), and the resolution codec carries
`FundedTransitionActionV3::FailNext` end to end
(`crates/dclutch-resolution-codec/src/lib.rs:284`, `:294`, `:305`, `:786`).

**And a market founded with a recovery policy cannot be terminalized at all.**
The live conjunct, stated by the funded walk's own module doc
(`programs/dclutch-resolution-proof-sbf/src/funded.rs:29-34`):

> `SourceResolutionStateV2::exhaust_after_primary_deadline` refuses
> `recovery_policy().is_some()` outright, so a recovery-bearing material has no
> terminal regardless of what any ladder does.

That is the refusal `CoreSbfError::RecoveryWalkUnavailable` is actually
justifying. **A correction the contract row and the reader's brief both need:**
the V1 per-leg `FailNext` walk and the `#[cfg(any())]` call site are **gone, not
parked** — deleted with the other thirteen in `lib.rs`, per `AGENTS.md`'s rule
that a superseded authority path is deleted in the same convergence cycle as its
successor (`funded.rs:14-28`). `funded::process_funded_transition` has no
definition anywhere in the tree, and four comments in `dclutch-core-sbf` and the
local-validator bootstrap still cite it in the present tense. So the row cannot
be closed by finding the old walk and re-enabling it; keeping recovery means
building the ladder, and cutting it means deleting the ontology.

`AGENTS.md` forbids parallel legacy and current authority paths, so it cannot
simply sit.

## 2. The ruling

**Keep, as one funded ordered-recovery ladder with a permissionless advance and
exhaustion into the failure selector.**

The shape, and each clause is a check:

- **Ordered and named.** The source spec names alternative sources **in order**,
  each with its own window.
- **Funded at founding.** Each attempt's funding is paid when the market is
  founded, so nobody is ever asked to pay for a recovery mid-flight. This is
  already how the terminal schema thinks:
  `ResolutionCertificateV2::validate_shape` refuses a `ResolutionFailure` whose
  `funding_allocation` or `work_paid` is zero, *"so the Lean-owned terminal
  schema encodes prepayment as a decode-time invariant. There is no unfunded
  failure certificate to emit"* (`funded.rs:7-12`).
- **Permissionless advance.** When a window closes unobserved, a permissionless
  crank advances the machine to the next funded source. The advance is paid from
  the attempt's own funding, at the funded-crank floor decision 0024 carves —
  the same mechanism, not a second one.
- **The honest path is unchanged.** Once advanced, resolution runs on the next
  source exactly as it runs on the first, byte-identical.
- **Exhaustion into failure.** Only when the ladder is exhausted, and only after
  the last window's maximum age, does the failure selector resolve — into
  decision 0025's escrow, not into the founder's Position.

Why one transition rather than the V1 walk's six: the V1 shape debited an
allocation per leg. `exhaust_after_primary_deadline` refuses a recovery-bearing
material *"precisely because skipping paid-for legs would take an outcome away
from the holders who paid for them"* (`funded.rs:39-43`) — the refusal that
blocks the ladder today is stating the ladder's own correctness condition, and
the build replaces the refusal with the advance rather than removing the reason
for it.

## 3. Ember's amendment

Recorded at `GOAL.md:4654`:

> D5 — robust failure pathways (keep recovery)

Ember asked for **robust**, which is more than *keep*. The orchestrator's
recommendation was a keep on the argument that with the two-scale fix and the
in-window scheduler landed, a second source is the only remaining reason a
window can close unobserved. The amendment makes that a requirement rather than
an argument: the ladder must be a pathway a stranger will actually walk, which
means the crank is permissionless *and cheap enough that a stranger with a stake
turns it*, and the last rung is a source no third party can redeploy from under
us — the relayed family, which reads mainnet's own state. That is a second
reason decision 0029 rules Series and the relay as first-class families rather
than optional ones: cohort-13's outage was Pyth redeploying their devnet
receiver under every market's release pin.

## 4. The lane implementing it

**RECOVERY** (`GOAL.md:4657-4658`), paired with **ESCROW** (decision 0025). The
properties are stated in Lean before any Rust. It is a founding change, so it
rides cohort-16 with the escrow.

## 5. The hostiles and laws that will guard it

The properties the lane states first, each a hostile as much as a theorem:

- **every attempt is funded before it is enterable**, so nobody can be asked to
  pay for a recovery mid-flight — already half-held by
  `validate_shape`'s decode-time prepayment invariant;
- **the ladder is finite**;
- **exactly one of resolved or exhausted terminates a market** — the property
  whose current violation is that *neither* terminates a recovery-bearing one;
- **the honest path is byte-identical to today's**, which the Direct fill
  campaign and the census laws hold.

The standing instruments that already apply: the funded walk's headline property
— *"a silent provider cannot make a market unresolvable, only drive it to a
pre-disclosed outcome, along a bounded, prepaid, permissionless path that pays
whoever walks it"* (`funded.rs:3-6`) — every clause of which is a check in that
module; and `MAINNET_STATE_RELAY.md:1077`, *"`CommitFailure` commits the
**Product's own** failure selector out of `FiniteResultMapV1`. A caller never
selects a result."*

### What landed at `332b432e6`: the properties above are now theorems

`Phase::Recovery` had been *"a phase the record could describe and no route
could reach"* — three transitions, not one of which moved `active_attempt`, so
*"every holder's principal sat in a market with no exit."*

**Lean first, over the REAL `RecoveryPolicyV2` rather than a second model of
it.** `SourceResolutionStateV2Abi` carries the ladder split into a decidable
guard and a total successor, *"so the guard is a value the theorems can name."*
The four properties §5 listed as owed are now among the theorems, plus three
this record had not asked for:

- **every entered attempt is funded** — an advance lands on `Recovery` and on an
  index the policy funds, *"so no crank puts a market on a leg nobody paid
  for"*;
- **the ladder is finite** — a measure starting at `attemptCount + 1` strictly
  descends on every advance, so no market is cranked forever and **no crank
  revisits a rung**;
- **`Exhausted` is reached only after the LAST funded window's own deadline**,
  and the rung it leaves is funded, spent, and has nothing after it;
- **from a closed window exactly one of advance and exhaust fires** — *"the walk
  is a walk and not a choice"*, and no market sits in `Recovery` with nothing to
  do;
- advancing before the window closes refuses, and so does advancing past an
  unfunded attempt;
- **the two ends are `Resolved` and `FailureCommitted`**, no crank lands on a
  terminal read, and `Exhausted` is neither end — which is this record's
  *"exactly one of resolved or exhausted terminates a market"* stated more
  precisely than it was ruled;
- and **every rung the ladder enters fits the record**, the bridge from the
  transition system to the persisted layout.

In Rust, **one transition rather than a family**: `crank_recovery_ladder`
decides the current window has closed and then either the policy funds another
attempt or it does not, each arm returning the attempt whose compartment pays
for it. `resolve_recovery_from_authenticated_domain` is the honest branch and is
*stricter about time than the primary one on purpose* — the attempt's deadline
is a field of the policy the transition already holds, so refusing a late
capture costs nothing and closes the second in which a crank and a capture could
both claim.

Hostiles, each naming its discriminant: advancing on the primary deadline second
refuses `DeadlineNotReached`; a capture against a foreign source spec, a foreign
provider release, or the PRIMARY spec refuses `LinkageMismatch`; a capture one
second past the attempt's deadline refuses `DeadlineElapsed`; a policy the
material does not select refuses `LinkageMismatch`; and a decided market cannot
be cranked out of its terminal. 84 tests green in `dclutch-source-contract`; the
Lean library builds with no warnings. The generated file moved by exactly one
constant, *"every offset and every corpus byte identical"*.

**Owed and named:** the on-chain route, which the lane says is the next commit;
and the four stale comments in `dclutch-core-sbf` and the local-validator
bootstrap that cite `funded::process_funded_transition` and its `#[cfg(any())]`
call site in the present tense. They justify a refusal that is still correct for
a different reason; the citation is history, not a pointer, and the lane that
lands the route retires them.

## 6. What was given up, named

**The V1 walk is not coming back.** Six funded transitions in the worst case,
each debiting its own allocation, is the shape of a market that *bought* named
alternative sources leg by leg. The build is one ordered ladder with funding
allocated at founding, which is a different accounting, and the V1 code is
already deleted rather than available to restore.

**Recovery is a founding-time commitment.** A market that did not buy
alternative sources at founding cannot acquire them later, and the funding is
spent whether or not the ladder is walked. That is the price of *"every attempt
is funded before it is enterable"*.

**A recovery-bearing market costs more to found**, and every rung's window
extends the worst-case time to resolution. Markets that want a fast terminal
found with no policy and accept decision 0025's escrow as their only failure
pathway.

## 7. The cost of reversal

**Cutting recovery deletes decision 0025's option C** and makes the failure
selector the only terminal for every outage — which returns the product to the
state cohort-13 executed and cohort-14 declined to repeat, mitigated only by the
escrow's refund.

**The cut is not a park.** `AGENTS.md` forbids parallel legacy and current
authority paths, so ruling it out obliges deleting `RecoveryPolicyV2`,
`RecoveryAttemptV2`, `source_recovery_policy_v2.rs` and its emitted twin, the
codec's `FailNext`/`Exhaust` actions and their Lean, and re-deriving
`exhaust_after_primary_deadline` without the recovery conjunct — a wire change
and a re-found, not a feature flag. The harness README's CU table for a funded
recovery campaign (`294,002` / `292,213`) documents a route that no longer
compiles and would go with it.

**And the last rung disappears with it.** The argument for the relayed family as
every ladder's final source is only an argument while there is a ladder;
cutting recovery removes the strongest reason decision 0029 keeps Series.

## Evidence pointers

`docs/MASTER_COMPLETION_CONTRACT.md:187`; `GOAL.md:807`, `:4654-4658`;
`programs/dclutch-resolution-proof-sbf/src/funded.rs:3-12`, `:14-34`, `:36-52`,
`:348`;
`crates/dclutch-source-contract/src/source_recovery_policy_v2.rs:25`, `:32`,
`:118-126`, `:174`;
`crates/dclutch-source-contract/src/source_resolution_v2.rs:449-451`, `:525`;
`crates/dclutch-resolution-codec/src/lib.rs:284`, `:294`, `:305`, `:786`;
`crates/dclutch-source-contract/src/lib.rs:217`, `:4747`, `:4767`;
`docs/design/MAINNET_STATE_RELAY.md:1077`;
`docs/decisions/0025-an-outage-refunds-rather-than-paying-the-founder.md`;
`docs/design/FUNDED_CRANK_V1.md` §3.
