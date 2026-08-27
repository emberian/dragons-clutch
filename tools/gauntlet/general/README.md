# General accelerator campaign — a fast lane at runtime width 1

```sh
tools/gauntlet/run.sh --mode census      # once, if there is no inventory yet
tools/gauntlet/general/run-general.sh
tools/gauntlet/run.sh --mode census      # render the report
```

Pure `solana-program-test`. No validator, no port, no ledger, no keypairs to
seed — every address in the fixtures is a literal and the one PDA is a
fixed-seed `find_program_address`, so this campaign's compute band is genuinely
zero rather than merely small.

## What it drives

One route: `general-accelerator/process_instruction`. The accelerator exposes no
others — its seven actions are selected by the authenticated
`ControllerRequestV2` inside a Trading `DCLTHOT3` envelope, not by separate
entrypoints — so the campaign's eight bindings all name that route and are
distinguished by label. The labels are emitted by the campaign from the
authenticated action and the runtime width; nothing hand-writes a label per
transaction.

18 recorded transactions: `Consider` ×1, `Freeze` ×2, `InitializeSettlement` ×3,
`Collect` ×5, `Materialize` ×1, `Distribute` ×3, `Close` ×2, and one corrupted
scratch page. Six of those are hostile.

**A semantic refusal from this accelerator is a SUCCEEDING transaction.** It
returns a typed refused `AcceleratorAckV2` rather than a program error, which is
exactly how Trading distinguishes transport failure from a failure-atomic
semantic refusal. So the accepted and refused *dispositions* are both bound as
`executed`, and the single `refused` binding is the one transaction that really
did reach a program error — the corrupted scratch page, whose reassembled
whole-bank digest no longer matches the digest the request commits to, refused
with `0xC003 InvalidScratchBank` before any evaluation. That distinction is
witnessed, not assumed: `exactly-one-transaction-reached-a-program-error` pins
the count, because a campaign whose hostiles all began erroring would otherwise
look exactly as green as one whose hostiles all began passing.

## The four fast-lane clauses

`TIERS.md` requires a ProgramTest-backed tier to state which of the four
conditions it meets. They are answered one at a time, in machine-readable prose,
in `fast-lane.json` — which `run-general.sh` merges into the evidence document
so the answers sit beside the numbers they qualify. That is `direct/`'s habit and
the reason `TIERS.md` names it the worked example: a fast-lane claim asserted in
aggregate is unfalsifiable.

The short version, and the one that binds:

**This tier records runtime width 1 and nothing else.** At N=258 six of the seven
actions serialise to 1,273–1,329 legacy-message bytes against Solana's 1,232-byte
maximum, and ProgramTest submits no packet, so it cannot notice. Recording an
N=258 transaction here would flip a route to EXECUTED on a frame no validator
would accept — precisely the laundering the packet clause exists to forbid. Those
transactions still **run**: the campaign exercises every action at both widths,
and the measured extents are the evidence that the clause fails there. They are
simply not recorded, and `the-tier-recorded-only-the-runtime-width-it-claims`
is the witness that keeps a later test from quietly changing that.

At N=1 every packet measures 745–867 bytes. The campaign serialises each
transaction itself and records `wire_bytes`, so the clause is *measured against*
rather than *relied upon*.

The `real_account_shapes` clause is answered narrowly and deliberately: this
route touches no Token-2022 mint, no System Program metadata and no
upgradeable-loader account, which are the three shapes that have historically
differed between ProgramTest and Agave. "Those shapes do not appear in this
route" is a smaller claim than "the account shapes are real", and it is the one
this tier can defend.

## What flipping this row does NOT mean

`general-accelerator/process_instruction` is now EXECUTED, from a fast lane, at
one runtime width. It is not validator evidence and the report says which
campaign it came from. Two things are still owed, in order:

1. **The canonical runtime width.** N=258 needs the ALT/v0 route
   `blocked.json` named; `compile_general_hot_v0` and
   `canonical_general_lookup_addresses_v3` exist and are tested, but this
   campaign submits legacy transactions.
2. **The real path.** The accelerator is reached here by a purpose-built test
   caller, not by Trading. Reaching it through `process_hot_execution_v3` needs a
   General Hot bundle — an admitted-AOT frame of fixed(39) ++ extras(8) ++
   caller authorities ++ runtime, plus a deployed accelerator ELF — which does
   not exist. See GEN-HOT and `docs/decisions/0010` §6.

Neither is a reason to leave the row unclaimed. A route observed only by a fast
lane is recorded with that campaign's name and the report shows it, which is the
mechanism that keeps the distinction visible instead of resting on someone
remembering it.

## Files

| file | what it is |
|---|---|
| `run-general.sh` | build (with a frame-diagnostic gate) → test → fold → merge clauses → witnesses → `census observe`, under a ledger lock |
| `bindings.json` | eight bindings, one route, labels emitted by the campaign |
| `programs.json` | the one address the census corroborates against the logs |
| `witnesses.json` | eight `evidence-jq` witnesses; no `cu-budget` witness, because General has no rows in `CU_BUDGETS.json` and a `cu-budget` witness naming a campaign with no entries is a red `NOCAMPAIGN` row |
| `fast-lane.json` | the four clauses, merged into the evidence document |

`run-general.sh` takes the same `mkdir` lock on the ledger that `run.sh` uses.
The other family lanes do not, and `census observe` is a read-modify-write of one
shared file while family lanes run concurrently.

## CU

General has **no rows in `tools/gauntlet/CU_BUDGETS.json`**, so this tier carries
no `cu-budget` witness — one naming a campaign with no budget entries produces a
red `NOCAMPAIGN` row, which would be a worse statement than silence. The widest
recorded draw here is 61,322 CU at N=1; the whole per-action table at both widths
is in `docs/evidence/GENERAL_ESCROW_PHYSICAL_2026_08_27.md` §4. Pinning budgets is
the CU-BUDGET lane's file and its call.
