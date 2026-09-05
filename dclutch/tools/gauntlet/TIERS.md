# Authoring a gauntlet tier

A **tier** is one campaign: an ordered set of real transactions submitted to a
real validator, bound to census routes, with witnesses. Tier 1 is the
infrastructure floor. Family tiers get added here as their lanes land.

Read `DESIGN.md` first. This file is the mechanics.

## What exists today

```
tools/gauntlet/
  DESIGN.md              the five principles; read before changing anything
  TIERS.md               this file
  run.sh                 census report, and the tier-1 runner: `--mode full`
                         builds seven ELFs, launches a validator, campaigns
  blocked.json           NEVER-EXECUTED routes with reason + owning lane
  CU_BUDGETS.json        per-transaction compute budgets; ONE file, ONE owner
  CU_BUDGETS.md          what they catch, what they do not, how to re-pin
  census/                the route census tool (standalone cargo package)
  tier1/
    bindings.json        campaign transaction label -> census route
    witnesses.json       asserted witnesses with provenance
    check-witnesses.sh   evaluates them against the campaign evidence
```

Tier 1's transaction producer lives in
`tools/local-validator/bootstrap/successor/`, and `run.sh --mode full` builds
and drives it. It was parked at the retired `demo-market` boundary from
2026-08-31 to 2026-09-03; the repair was not to find it another Market planner
but to stop needing one, because the successor's own loopback planner
authenticates a checked-MUTABLE plan and refuses immutable-Core semantics --
which is what the infrastructure floor is. The spec now omits `market` and the
supervisor compiles a fixture input from the plan it builds
(`SuccessorRunSpec::market`). Named family runners still own their own build,
evidence, witnesses, and validator lifecycle.

**IT COMPLETES.** First at `93a2793bd`, 2026-09-03: 201 transactions, an
evidence document, 515 census observations, 21 witnesses green and every CU
budget under. Before that the atomic founding `DCLTGMF3` refused its last
transaction with Claims `0x518D ClaimsFoundingSbfErrorV5::PermitBody` -- one
byte of a `CoreState` the supervisor predicted with the wrong one of Core's two
Product-graph walks, three legs upstream of the code that could report it.

**Budget.** The completing run took 53m33s end to end against a warm `--work`,
of which 39m was the campaign, on a laptop under heavy concurrent load (a
one-minute load average of 137 while it started). That is NOT a clean budget
measurement and it is quoted as an upper bound, not a figure: the two
diagnostic runs that preceded it took 42m48s and shared the machine with each
other. The last uncontended measurement is the 25-31 minutes below, taken when
the campaign was 195 transactions and stopped at the founding; the completing
campaign is six transactions longer and does strictly more. For reference, that
earlier figure: 18m01s of campaign after 7m of archive, build, tool and inventory
against a warm `--work`, and a cold `--work` adds about 6m of SBF builds.

**THE CLEAN TIMING, and it is inside the budget: 19m33s.** Measured 2026-09-03 at
`7dc962c7f` on the twelve-core laptop, one-minute load average 5.03 at start, into
a fresh private `--work`: exit 0, 199 campaign transactions, 515 census
observations, 21 witnesses checked and 0 failed, every CU budget under. It splits
3m26s of archive/ELF/tool/inventory and 16m13s of campaign, census and witnesses.

Three things that figure is not, said here so nobody quotes it as more than it is.
It is not an IDLE machine: three other lanes were live in the same checkout (a
devnet cohort, a Registry/Core lane, a web lane) and the measuring lane itself ran
two `cargo check --tests` during it; the load average sat between 8 and 16 through
the campaign. It is not a cold BUILD: the `--work` directory was cold but
`~/.cargo` and the SBF caches were warm, so the seven ELFs rebuilt in about two
minutes rather than the six a cold cache costs -- a genuinely cold machine is
slower and this number does not cover it. And it is one run, not a band; the
figure above it was taken under a load average of 137, so the spread between them
is the machine, not the campaign.

## The three files a tier needs

### 1. A transaction producer

Something that submits real transactions and emits a machine-readable record of
what it submitted. Tier 1 reuses the successor bootstrap's
`dclutch-local-successor-run-evidence-v2` document, whose `transactions[]`
entries carry `label`, `signature`, `slot`, `error`, `compute_units_consumed`,
and — critically — the finalized `logs`.

**The logs are not optional.** `census observe` cross-checks every claimed route
against the chain's own `Program <address> invoke [n]` lines. A producer that
does not surface finalized log messages cannot feed the census, because its
claims would be unverifiable and the census would be a mirror again.

A new producer must emit at minimum, per transaction:

| field | why |
|---|---|
| `label` | the binding key; must be stable |
| `signature` | so an observation names a specific finalized transaction |
| `slot` | ordering, and proof of finality |
| `error` | `null` on success; the structured `InstructionError` on refusal |
| `logs` | the chain's account of which programs ran and which code refused |
| `compute_units_consumed` | recorded in the ledger; a real limit, not a note |

### 2. `bindings.json`

```json
{
  "campaign": "tier1",
  "note": "why this campaign exists",
  "bindings": [
    {
      "label": "create canonical Found31 Market",
      "routes": ["core/found::process#Found"],
      "program": "core",
      "outcome": "executed",
      "note": "Canonical Core Found31, the 31-account frame routed over a finalized ALT."
    },
    {
      "label": "Found31 refuses substituted lifecycle credit",
      "routes": ["core/found::process#Found"],
      "program": "core",
      "outcome": "refused",
      "refusal": "core/CoreSbfError::RentCredit",
      "note": "The hostile case; the named refusal must be what the chain reports."
    }
  ]
}
```

Rules the census enforces, all of them hard errors:

- **Every campaign transaction must be bound.** An unbound label fails the
  census. Unbound labels are how coverage silently rots.
- **Every transaction must match exactly ONE binding.** Overlapping globs fail.
  A binding may cover many transactions; a transaction may not have two owners.
- **A binding that matched nothing fails.** A stale binding overstates coverage.
- **Named routes must exist in the inventory.** A route id that the enumerator
  no longer produces fails, so a dispatch refactor cannot quietly orphan a
  binding.
- **The chain must corroborate the program.** If the finalized logs do not show
  `program`'s address invoked, the observation is refused.
- **A named refusal must be the refusal the chain reported.** The census
  compares the census's enumerated numeric code against the transaction's
  `custom program error: 0x…`.

`label` accepts `*` as a wildcard anywhere in the pattern
(`publish Product graph: *Begin`), and it is the only metacharacter — a binding
pattern is read by a human deciding whether a campaign step is covered, and a
regex would make that harder rather than easier.

`program: ""` with `routes: []` is the honest form for a transaction that drives
no protocol route — an airdrop, a Loader `SetAuthority`, an Address Lookup Table
extension. Say so explicitly rather than leaving it unbound.

A refusal raised *before* the program's own taxonomy — a runtime privilege or
frame refusal that never reaches the program — is recorded as an unnamed
refusal rather than credited to a code. That is deliberate: crediting it would
overstate what the program proved.

### 3. `witnesses.json`

```json
{
  "campaign": "tier1",
  "witnesses": [
    {
      "id": "found31-packet-fits",
      "kind": "evidence-jq",
      "query": ".transactions[] | select(.label == \"create canonical Found31 Market\") | .error",
      "expect": "null",
      "provenance": "Solana's legacy packet maximum is 1,232 bytes ... Found31 misses by ten with keys inline (docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md), so it must ride a finalized ALT as v0."
    }
  ]
}
```

`provenance` is **required** and is checked for non-emptiness. A witness with no
provenance is rejected by `check-witnesses.sh`, because a number with no
provenance is a mirror wearing a hat.

The three admissible provenance kinds, from `DESIGN.md`:

1. A Lean-emitted vector, byte-checked by the owning crate's
   `check-generated.sh`.
2. A hand-stated constant naming its source — a Solana runtime limit, an SPL
   layout, a measured validator observation with date and validator version.
3. A cross-check against a second implementation.

Reading a value out of the code under test and asserting it equals itself is
not a witness, however many lines it takes.

### 4. A CU budget witness, if the tier's transactions are worth budgeting

Compute is the one resource whose exhaustion is a hard refusal with no partial
result, and it moves under a tier from OTHER lanes' work. `DCLTGMF1` went from
84.6% to 91.3% of the 1,400,000 maximum in one evening from concurrent changes
to Core, Claims and Trading, and nothing was watching.

A tier opts in with **one** witness entry that carries no number of its own:

```json
{
  "id": "the-golden-transactions-are-inside-their-cu-budgets",
  "kind": "cu-budget",
  "campaign": "tier1",
  "provenance": "why these transactions are worth a budget, and what the independent value is"
}
```

`campaign` is matched against the `campaign` field of entries in
`tools/gauntlet/CU_BUDGETS.json`, which is where every number lives. **A tier
does not carry a copy of a budget.** The witness expands to one row per budget
entry, so an over-budget campaign names the transaction and the delta rather
than saying the campaign got more expensive.

Two things to read in `CU_BUDGETS.md` before adding budgets for a new campaign:

- **These numbers are not deterministic.** Fresh keypairs per run move
  `find_program_address` bump-search iteration counts, and each iteration is
  1,500 CU. Measured bands range from 0 to 79,500 CU depending on the
  transaction. Pin the HIGHEST draw you observed, over several runs, never one.
- **A budget above 1,400,000 is refused.** That is deliberate: it is how the
  file says out loud that a transaction has stopped fitting, rather than letting
  someone widen a tolerance past the ceiling.

A budget that matches no transaction in the campaign is red, on the same
reasoning as a stale binding.

## The tiers that exist

| tier | directory | backing | what it drives |
|---|---|---|---|
| 1 | `tier1/` | localhost validator | the infrastructure floor: seven-artifact bootstrap through Found37, the atomic founding, and the readiness ladder after it |
| 4 | `tier4/` | `solana-program-test` | the Series occurrence waist (numbered before the rule below) |
| Claims/Custody | `claims-custody/` | `solana-program-test` | the protocol Position lifecycle, the composed Admit -> SparseNativeTransfer -> Close chain, and ordinary plus delegated Custody against real SPL Token and Token-2022 |
| Dealer | `dealer/` | `solana-program-test` | the Dealer equity pool's rounding boundary |
| Direct | `direct/` | `solana-program-test` | the stateless Direct V2 AOT accelerator |
| Hot CU | `hot-cu/` | `solana-program-test` | not a campaign: the **Registry Hot continuation's** compute against the 1,400,000 ceiling, swept over N fixture seeds. It admits no evidence and observes no route, so it warns where a campaign tier refuses. **Not the public route**: the continuation was demoted to harness-only on 2026-08-30 and runs a constant +35,127 CU above top-level (`docs/decisions/DECISION_PACKET_2026_08_30.md` §4; `docs/evidence/CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md`) — for a public-trade figure use `direct_hot_top_level_margin_gate.rs`. Read its README before quoting a number from it — the per-seed figure is a bump-search draw (M-61) and only the pass count and the mean mean anything |
| Ladder | `ladder/` | localhost validator | LADDER: a market's funded ordered recovery ladder. One validator, stood up once through the relayed vertical's own checked-mutable `substrate.rs` (linked, not forked), then THREE commands against it: the two-source market compiled through the shipped `--recovery-rungs` parser, the founding that buys the ladder, and the shipped `advance-recovery` crank driven against the market that founding just produced. Its build stage is the CHECKED RELEASE GATE, which strict mode emits only at zero SBF frame diagnostics. It warps no clock: a leg not yet due is recorded as not yet due, because the conjunct the whole ladder rests on is that the last second an honest observation may land and the first second a crank may run are different seconds . **Timing, 2026-09-04, one hbox run into a fresh `--work`: 1,780 s end to end** -- about 15 minutes of substrate bring-up, administration and a 201-transaction founding, then the two cranks, each of which WAITS in real time for its leg's deadline (761 s and 115 s here) because that is the only honest way to reach one. The waiting is the budget: a walk's wall-clock is whatever deadlines its market bought, and `--max-wait-seconds` is the ceiling on it |
| Journey | `journey/` | localhost validator | JRNY-1: one Market's whole life, tier 1's founding continued in-process into post-Open collateral distribution to N holders, a holder ring, and rent recovery — under one conservation ledger |
| Relayed vertical | `relayed-vertical/` | two localhost validators | DEMO-VERT: the relayed graduation market end to end — found with no recovery policy, the real relayer daemon observes a mainnet twin, seal, consume, terminalize — plus the silent-relayer sibling where the funded deadline walk pays a walker on a bare legacy packet; journey-shaped (tier 1 compiled in by `#[path]`, the journey's conservation ledger threaded, tier-1 bindings merged at fold time) |
| Resolution core-v3 | `resolution-core-v3/` | `solana-program-test` | the Resolution funding lifecycle against real Core/Custody/Registry/Resolution ELFs: Core-composed CreateFund at depth two, the V7 direct activation and its idempotent replay, the V7 direct CloseFund, both halves of the permissionless provider-abandon reclaim, and Core's terminal admission — which two witnesses show invokes NO child. Wired 2026-09-01 (C-09 WITNESS) around a campaign that had been green and census-invisible for weeks |
| Resolution sponsored | `resolution-sponsored/` | `solana-program-test` | all five `SponsoredPushActionV1` actions — Capture, Settle, CloseCandidate, CloseHead, CommitFailure — plus nine hostiles raising five distinct codes. **Eight of its sixteen transactions do not fit a legacy packet**; read its README before quoting a wire figure |
| Resolution pre-market funding | `resolution-pre-market-funding/` | `solana-program-test` | the pre-Market funding pair: initialize a future Market's Resolution-owned ledger through Core's ProjectFound36 projection, and abort it when the prepared checkpoint expires. **Its initializer overruns the legacy packet maximum by 565 bytes** and costs 488,773 CU |

`journey/` is the first tier that is a **superset** of another rather than a
sibling. It compiles the tier-1 producer's own source files into its binary by
`#[path]` and calls `runtime::found_through_open`, so its evidence document
carries every tier-1 transaction before its own. Two things follow, and both are
in `journey/run-journey.sh` rather than in a second copy of anything: the
bindings handed to `census observe` are tier 1's merged with the journey's at
run time, and the shared witness evaluator is called twice against the same
evidence with two different context files. A tier that continues another one
should copy that shape; a tier that forks the other's bindings will discover the
census failing on somebody else's change.

**Numbered directories turned out to be a bad idea.** `tier<N>` is a global
namespace with no allocator, and on 2026-08-27 three lanes claimed the same two
numbers inside twenty minutes, each silently overwriting the previous lane's
`bindings.json`. One of those collisions produced a green-looking run that
evaluated one campaign's evidence against another campaign's bindings. **Name a
new tier's directory after its family, not after a number.** A family name
cannot collide, and it tells a reader what the tier is for.

The witness evaluator `tier1/check-witnesses.sh` is SHARED, not tier-1-specific;
it only lives there for historical reasons. Call it, do not fork it — a second
copy is a parallel authority path under `AGENTS.md`, and the two copies will
diverge on the day one of them learns something. Its third argument is
whatever CONTEXT file the tier wants `expect_from` to read; tier 1 passes the
bootstrap plan, `direct/` passes a hand-derived expectations file merged with
its build stage's artifact record.

## What a family lane owns

A family lane owns a `run-<family>.sh` that builds its ELFs, runs its campaigns,
folds the evidence, checks its witnesses and calls `census observe` itself. It
does NOT add a stage to `run.sh`: `run.sh` owns the census/report, and a shared
script every family edits is the numbered-directory race one level down. Render
the report afterwards with `run.sh --mode census`, which is cheap and reads the
accumulated ledger.

A family lane may carry more than one census campaign, and has to when its
campaigns disagree about an address: a census campaign has ONE program map, and
`claims-custody` pins different `registry` addresses in its two families. One
`run-<family>.sh` then loops the groups, each with its own bindings, program map
and witness file.

## A refusal the census cannot name

A campaign that proves rollback does it by refusing AFTER the child committed,
and the program that refuses is a test-only caller the census does not
enumerate. The chain reports THAT program's code.

That code used to collide with a first-party refusal it had nothing to do with
-- `DeliberateLateFailure = 3` was also `claims/ClaimsSbfError::Release` and
`custody/CustodySbfError::CallerAuthority` -- and naming one of those in the
binding was a lie the census could not detect, because the numbers matched.
`docs/decisions/0007-namespaced-refusal-codes.md` ended the collision: every
test caller owns a band at `0x100000` or above, which no deployed program can
reach, so a caller's code now identifies the caller. It did not end the reason
for this field. A test caller is still not an enumerated program, so there is
still no taxonomy entry to credit its refusal to. Such a binding carries:

```json
{
  "outcome": "refused",
  "unnamed_refusal": {
    "code": 1077251,
    "reason": "test/custody-caller's DeliberateLateFailure (0x107003), raised after the child committed"
  }
}
```

`code` is decimal because JSON has no hexadecimal literal; write the hex form in
`reason`, which is the form a validator log shows. The code is still checked
against what the chain reported, so a campaign whose claim and chain disagree is
still refused; the observation simply credits no enumerated taxonomy. Exactly
one of `refusal` and `unnamed_refusal` is admissible per binding, and an empty
reason is refused: an uncredited refusal with no account of where it came from
is how a real refusal launders itself out of the taxonomy.

## Adding a family tier

1. **Check the census first.** `run.sh --mode census` takes seconds and prints
   the routes your family exposes and which of them have never executed. That
   list is your tier's target set.
2. **Write the tier's `blocked.json` entries before you write the tier.** Any
   route your family exposes that you cannot drive yet gets an entry naming the
   reason and the owning lane. A route with no entry and no observation shows up
   in the report's "NO stated reason at all" row, which is the row that should
   make someone uncomfortable.
3. **Add a producer** under `tools/gauntlet/<family>/`, emitting the evidence
   shape above. A ProgramTest campaign gets there through
   `tools/gauntlet/program-test-evidence`: one `record` call per submitted
   transaction, a no-op unless `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR` is set, and
   `fold-program-test-evidence` to assemble the document.
4. **Add a `run-<family>.sh`**, not a `run.sh` stage. Outputs under a `--work`
   root, never in the repo; the ledger and inventory default to the shared
   `run.sh` output so campaigns accumulate. A campaign must never be keyed on
   its bindings file — authoring a binding must never cost a campaign re-run.
5. **Bind, witness, observe.** `census observe` is called once per campaign; the
   ledger accumulates across tiers.

## The ProgramTest fast-lane bar

`solana-program-test` may back a tier's fast lane **only** when every one of
these holds, and the tier must state which:

- The tier does not depend on genesis Loader-v3 ProgramData layout, on a real
  `SetAuthority(Some -> None)`, or on ProgramData deployment slots.
- The tier does not depend on packet serialisation limits. ProgramTest does not
  submit a packet, so it cannot catch a 1,242-byte frame against a 1,232-byte
  limit. Found31 is exactly this defect; it survived every fixture test.
- The tier sets the compute limit to 1,400,000 and the heap to 32,768 and treats
  neither as adjustable. A diagnostic budget is a *measurement*, and a
  measurement never satisfies a gate.
- The tier's account shapes are the real Agave ones — the all-zero System
  Program with its NativeLoader metadata, Token-2022 mints with extensions.

Tier 1 satisfies none of these and therefore has no fast lane. `run.sh --mode
census` is the cheap mode; it does static enumeration only and says so.

A fast lane is always **additional** evidence, never a substitute: a route whose
only observation came from a fast lane is recorded with that campaign name, and
the report shows the campaign.

`direct/` is the worked example of a tier that meets all four. It answers them
one at a time inside its own evidence document, in a `fast_lane` block beside
the numbers they qualify, rather than in prose in a file nobody opens next to
the table. Copy that habit: a fast-lane claim asserted in aggregate ("this tier
satisfies TIERS.md") is unfalsifiable, and four separate sentences are not.

Two of its answers are worth stealing. **The packet limit**: ProgramTest submits
no packet, so the producer serialises every transaction itself and records
`wire_bytes`, and a witness checks the measured extent against Solana's stated
1,232 bytes. The tier does not depend on the runtime to enforce the limit; it
measures against it. **Frame diagnostics**: `cargo build-sbf` exits ZERO when
the SBF backend reports that a call overwrites its own stack frame, so the
tier's build stage counts them and refuses to run the campaign at all if the
count is nonzero. An artifact the toolchain calls potentially-undefined should
not be producing evidence, and only the build stage is in a position to say so.

One honest gap no ProgramTest tier can close: **ProgramTest has no finalized
commitment.** `slot` orders a campaign and proves nothing about finality. Say
that in the tier rather than letting the field's name imply otherwise.

## Extension points in the census tool

- `TARGETS` in `census/src/main.rs` lists the programs enumerated. A program
  directory that exists and is not in the list makes `inventory` **fail**, so a
  new program cannot become invisible by being forgotten.
- `arm_selectors` / `selectors_from` in `census/src/enumerate.rs` classify wire
  discriminants. A new dispatch shape goes here. The rule when extending: an
  unrecognised dispatch position must land in `unclassified` and be printed, not
  dropped.
- A `blocked.json` entry that no longer describes anything true is reported in
  the census's **Stale blocking entries** section: either it matches no
  enumerated route at all, or it still blocks a route that has since executed.
  A blocking entry outlives its reason as easily as a test outlives its
  invariant; delete it when it appears there.
- `MAX_DISPATCH_DEPTH` bounds how far the enumerator follows a dispatch chain.
  Raising it finds more action tags and more internal-branch noise; the current
  value of 2 is entry dispatch plus one action match.

## Running it

```sh
# seconds, no chain: the static census and the report
tools/gauntlet/run.sh --mode census

# 25-31 minutes: seven ELFs, a localhost validator, the tier-1 campaign, the
# fold. Unparked 2026-09-03 (`c9eac1738`); measured on an M-series laptop.
tools/gauntlet/run.sh --mode full --rpc-port auto
```

`--mode census` needs no port and may run concurrently; so does `--mode full`
with `--rpc-port auto`, which takes a free 42-port block instead of the fixed
default. Run a supported family campaign through its named runner; on hbox its
runner must use `swarm-build` and respect co-tenant workloads.

**These two paragraphs said the opposite until 2026-09-03**: that `full` was an
explicit pre-build refusal, which it was from 2026-08-31 until `c9eac1738`
unparked it that morning. Four documents and one test went on saying it after
the park was gone -- `tools/gauntlet/test-run-cli.sh` asserted the refusal and
was red, and nothing runs that file either. A park is easy to install and its
paperwork is what nobody remembers to take back out.

A tier that starts a validator inherits two rules from this. It must take its
origin from the same `--rpc-port`/`$DCLUTCH_GAUNTLET_RPC_PORT` parameter rather
than writing a port down, and it must hold the ledger lock around
`census observe`, which is a read-modify-write of a file every family runner
defaults to sharing.

And never edit `run.sh` while a run is in flight — bash reads a script
incrementally by byte offset, so a mid-run edit makes it re-execute or skip a
block.
