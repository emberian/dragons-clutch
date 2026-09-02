# resolution-core-v3 — the Resolution funding lifecycle, as census evidence

A ProgramTest fast lane over
`crates/dclutch-svm-harness/tests/resolution_core_v3_lifecycle.rs`, against the
real compiled Core, Custody, Registry and Resolution ELFs. That campaign has
been green for weeks and was **invisible to the census the whole time** —
`blocked.json` described the gap in its own words: *"runs green under
ProgramTest ... but that campaign emits no census evidence."* This tier is that
evidence.

```sh
tools/gauntlet/resolution-core-v3/run-resolution-core-v3.sh
```

It records **seven** of the campaign's fifty-plus transactions:

| label | what it is |
|---|---|
| CreateFund | Core-composed fund creation, reaching Resolution at depth two |
| activation | the V7 direct activation, Pending → Active |
| activation replay | the same route's idempotence |
| CloseFund | the V7 direct terminal close |
| abandon (early) | the one recorded hostile: `SubmissionStillConsumable` |
| abandon | a stranger reclaims the losing provider submission |
| terminal admit | Core admits the terminal state with **no child invocation** |

Everything else the campaign submits is deliberately unrecorded: a campaign that
labelled every transaction it happens to send would be claiming coverage no
binding was written for.

## What this is NOT

1. **Not validator evidence.** Nothing deploys through Loader-v3 and ProgramTest
   has no finalized commitment.
2. **Not provider evidence.** The Pyth receiver and router are the
   provenance-pinned local-validator projection of captured artifacts, and every
   price body is a fixture. The honest sentence is *"the bank executed the
   funding lifecycle against the real ELFs,"* never *"the protocol resolved a
   market on chain."*

## The fast-lane bar, answered one at a time

- **Loader-v3 / ProgramData / `SetAuthority`.** Not depended on. Immutable
  ProgramData bodies are installed for every role and no authority transition is
  exercised.
- **Packet serialisation.** Depended on, so **measured**. Three of the seven
  recorded transactions do not fit — see below.
- **Compute and heap.** ProgramTest's compute maximum is exactly Solana's
  1,400,000 and is never raised. The largest observed consumption is 286,481 CU
  on CreateFund. This matters more here than usual: the V7 **direct** close
  exists precisely because the composed Core-CPI close exceeded this ceiling.
- **Real Agave account shapes.** Core state, the activation cache, the Source
  graph, the capability manifest and the three-row funding ledger are the real
  encoders' output; the provider accounts are the captured receiver/router
  layouts.
- **Frame diagnostics.** The runner counts SBF stack-frame-overwrite
  diagnostics per artifact and refuses to run at all if the count is nonzero.

## !! BOTH ENDS OF THE LIFECYCLE MISS A LEGACY PACKET !!

Measured 2026-09-01, first measurement of this family. Legacy maximum 1,232
bytes.

| transaction | bytes | over |
|---|---:|---:|
| CreateFund | 1,275 | **+43** |
| terminal admit | 1,456 | **+224** |
| CloseFund | 1,237 | **+5** |
| activation | 1,189 | fits |
| activation replay | 1,172 | fits |
| abandon | 1,052 | fits |

Creating a fund, admitting a terminal, and closing a fund all need v0 messages
over an Address Lookup Table. `CloseFund` misses by **five bytes**, which is the
kind of margin that reads as an accident and behaves like a wall.

## Two witnesses that check an ABSENCE

Both read the runtime's own `Program <id> invoke [n]` lines, recorded as depths
only so they survive a run whose gauntlet-local addresses move.

- **`core-v3-activation-and-close-are-not-composed`** — activation and close
  never reach depth 3. A depth 3 appearing here would mean the composed
  Core-CPI path, which `resolution::process` refuses at its top for exceeding
  the compute ceiling, had come back.
- **`core-v3-terminal-admit-invokes-no-child`** — the terminal admit is a single
  depth-1 entry and nothing else. That is Core's own sentence — it accepts the
  Resolution-owned certificate *"without asking Resolution to repeat the same
  release, product, Source, ledger, and certificate work in a child
  invocation"* — measured rather than believed. It is also why
  `resolution/process_admit#AdmitTerminal` stays blocked with no observation
  while this campaign drives Core's admission to completion: two routes, one
  name, and only one of them runs.

## What the terminal-admit row convicted

`core/resolution::process#AdmitTerminal` (`resolution.rs:266`) is a **dead arm**,
and this tier is what proves it rather than arguing it. AdmitTerminal is the only
action that names the arm; the recorded transaction submits exactly that action
and **executes**; had the arm been live the same transaction would have refused
`CoreSbfError::Instruction`. Two more dead arms sit beside it in the same match.
All three are filed in `blocked.json` with the deletion named.
