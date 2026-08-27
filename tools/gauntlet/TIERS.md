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
  run.sh                 build -> deploy -> campaign -> census, resumable
  blocked.json           NEVER-EXECUTED routes with reason + owning lane
  census/                the route census tool (standalone cargo package)
  tier1/
    bindings.json        campaign transaction label -> census route
    witnesses.json       asserted witnesses with provenance
    check-witnesses.sh   evaluates them against the campaign evidence
```

Tier 1's transactions are produced by the transaction-only bootstrap in
`tools/local-validator/bootstrap/successor/` (owned by the W1d lane; the
gauntlet consumes it read-only, as a subprocess). The gauntlet owns the build,
the ELF pinning, the spec, the resumable staging, the witnesses, the census, and
the report.

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

## The tiers that exist

| tier | directory | backing | what it drives |
|---|---|---|---|
| 1 | `tier1/` | localhost validator | the infrastructure floor: seven-artifact bootstrap through Found31 |
| Direct | `direct/` | `solana-program-test` | the stateless Direct V2 AOT accelerator |

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

## Family lanes are named, not numbered

Tier 1 is a number because it is the infrastructure floor and there is one of
it. Everything after it is a FAMILY, several land at once, and four lanes
independently numbering their own tier is a race nobody wins -- it happened, on
2026-08-27, and the numbering had to be renegotiated live while a committed
`tier2/` was being moved to `tier4/`. A family lane therefore lives in a
directory named for its family:

```
tools/gauntlet/
  tier1/            the infrastructure floor: a real validator, a real deploy
  tier4/            the Series occurrence fast lane (numbered before this rule)
  claims-custody/   the Claims and Custody family fast lanes
  dealer/           the Dealer family fast lane
  direct/           the Direct family fast lane
```

Each family lane owns a `run-<family>.sh` that builds its ELFs, runs its
campaigns, folds the evidence, checks its witnesses and calls `census observe`
itself. It does NOT add a stage to `run.sh`: `run.sh` owns tier 1 and the
census, and a shared script that every family edits is the same race one level
down. Render the report afterwards with `run.sh --mode census`, which is cheap
and reads the accumulated ledger.

A family lane may carry more than one census campaign. It has to when its
campaigns disagree about an address: a census campaign has ONE program map, and
`claims-custody` pins different `registry` addresses in its two families.

## A refusal the census cannot name

A campaign that proves rollback does it by refusing AFTER the child committed,
and the program that refuses is a test-only caller the census does not
enumerate. The chain reports THAT program's code, which can collide numerically
with a first-party refusal it has nothing to do with -- `DeliberateLateFailure
= 3` is also `claims/ClaimsSbfError::Release` and
`custody/CustodySbfError::CallerAuthority`.

Naming the first-party refusal in that binding is a lie the census cannot
detect, because the numbers match. Such a binding instead carries:

```json
{
  "outcome": "refused",
  "unnamed_refusal": {
    "code": 3,
    "reason": "the test-only caller's DeliberateLateFailure, raised after the child committed"
  }
}
```

The code is still checked against what the chain reported, so a campaign whose
claim and chain disagree is still refused; the observation simply credits no
enumerated taxonomy. Exactly one of `refusal` and `unnamed_refusal` is
admissible per binding, and an empty reason is refused: an uncredited refusal
with no account of where it came from is how a real refusal launders itself out
of the taxonomy.

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

# the whole thing: build seven ELFs, bootstrap a fresh localhost ledger by
# transaction, run tier 1, fold the evidence into the ledger, render the report
tools/gauntlet/run.sh --mode full

# re-run one stage onward
tools/gauntlet/run.sh --from campaign
```

On hbox, `run.sh` routes every build through `swarm-build` automatically when it
is on `PATH`. hbox is co-tenant with codex's HOL build; keep waves small.

`--mode full` is a **single global slot per machine**: the successor launcher is
pinned to `127.0.0.1:20890` and refuses to start while anything else listens
there, whatever `--work` root you pass. Coordinate on the wave board before a
full run, and never kill a `solana-test-validator` whose `--ledger` is not under
your own `--work` root. `--mode census` needs no port and may run concurrently.

And never edit `run.sh` while a run is in flight — bash reads a script
incrementally by byte offset, so a mid-run edit makes it re-execute or skip a
block.
