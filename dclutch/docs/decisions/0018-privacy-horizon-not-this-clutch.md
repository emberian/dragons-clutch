# Decision 0018: the privacy/FHE/MPC horizon is not this Clutch, and O-019 is what keeps the door open

Status: **RULED 2026-09-01 by ember — ruled OUT of the accepted current
project, dated, with a named prerequisite, and therefore terminal rather than
deferred**. The ruling is `GOAL.md:2067-2094`, landed at commit
`5a371810dd6706a91f8e65f529cc482443b59363` (2026-09-01 12:47:56 -0400, *"ruling:
C-15 is not this Clutch, and O-019 is what keeps the door open"*) — thirty
insertions in one file. This record exists because that ruling lived for two
days inside a single narrative file while three authority documents still
carried C-15 as open; it changes no decision and adds none.

C-15 (`docs/MASTER_COMPLETION_CONTRACT.md:100`) closes with this record.

## 1. The question

> The specialized batch relation and the original privacy/energy ambition are
> implemented in the accepted project or explicitly ruled out by Ember; an old
> horizon decision is not silently treated as permanent completion scope.
> — `docs/MASTER_COMPLETION_CONTRACT.md:100` (C-15)

The contract's own vocabulary admits exactly two terminal states, implemented or
ruled out, and forbids the third. The 2026-08-27 ruling
(`docs/ASPIRATION_LEDGER.md:3-7` — *"dark-FHE is NOT a near/medium-term ambition
for dragons-clutch — its Tier-0 rows are DROPPED-BY-DECISION for this horizon"*)
was **horizon-scoped, not permanent**, which is precisely why C-15 continued to
exist after it.

## 2. The ruling, verbatim

Ember, `GOAL.md:2071-2074`:

> *"privacy/FHE is a 'not yet' for sure for sure, that would be a much later
> version of Clutch, solana isn't ready for that kinda awesomeness onchain yet
> (we'd want to use minidregg, which isn't ready yet)."*

The orchestrator's framing of it, in the same commit (`GOAL.md:2076-2079`):
this is a **scope ruling on the accepted project**, not a third state. The
FHE/MPC/energy objective is *not in this Clutch*. The condition for revisiting
is named — a later version of Clutch, on a substrate that can carry it, using
minidregg — and `GOAL.md:2093-2094` states the consequence:

> **Nothing may report the privacy horizon as deferred, future work, or
> in-progress.** It is ruled out, dated, with a stated prerequisite — which is a
> terminal state, and the difference matters.

## 3. What the ruling does NOT delete

**Zero lines of implementation, because there are none.** A source-only sweep
of `crates/ programs/ packages/ apps/ tools/ formal/` finds no first-party hit
for `FHE`, `homomorphic`, `MPC`, `zkML`, `DrEX`, `shielded`, `dark-book`,
`dark-pool`, `ciphertext` or `commit-reveal` (the only `confidential_transfer`
matches are transitive `spl-token-confidential-transfer-*` entries in a
program-test `Cargo.lock`). `BatchRelation` appears in prose only.
Corroborated at `docs/evidence/DEBT_OWNERSHIP_LEDGER_2026_09_01.md:775-779`
(*"What exists in code: nothing. … There is no crate, module, kernel or type
serving this ambition"*) and `:812-817` (zero of the retained charter's eight
items has a foundation).

The **specialized batch relation** exists by that name only in the neighbouring
compost repository's gen-1 archive
(`~/dev/dragons-clutch/archive/gen1/crates/clutch-batch/src/relation_v1.rs`,
spec at `archive/gen1/docs/SPECIALIZED_BATCH_RELATION.md`), which `AGENTS.md`
forbids importing from. gen-3's successor is the General candidate/verifier
plane, which is not called a relation and carries no privacy affordance.

**So the ruling costs no code and buys no code.** Its whole effect is on what
the tree is allowed to say, and on one invariant.

## 4. What the ruling obliges — and where each obligation is executed

`GOAL.md:2082-2091` records two obligations. Both are executed in the commit
carrying this record.

**(1) Remove contradictory claims.** *"Anything in the tree that implies the
privacy ambition is in scope, planned, or partially built must say what this
ruling says instead. C-15's row closes on this ruling rather than on code."*

| path:line | state before | state after |
| --- | --- | --- |
| `docs/MASTER_COMPLETION_CONTRACT.md:185` | *"open; do not infer from the old horizon park"* — the register that owns the two-terminal-states vocabulary carrying a row in the third state | **RULED OUT 2026-09-01 by ember**, quoting the ruling and citing `GOAL.md:2071` / `5a371810` and this record |
| `docs/evidence/C16_ENTRY_LIST_2026_09_01.md:417` | R-8 listed open — written at `3466740e` (2026-09-01 12:02), **forty-five minutes before** the ruling landed | R-8 closed by the ruling, same citations |
| `docs/evidence/DEBT_OWNERSHIP_LEDGER_2026_09_01.md:887` (`:897` after this edit) | R-8 owner Ember, unruled; §8 (`:772`) the standing framing | R-8 closed; §8 carries a SUPERSEDED marker naming it historical framing, same citations |

Both of the first two were **routed but not performed** at
`docs/evidence/ARCHITECT_SCHOLAR_2026_09_01.md:1320-1335`, diagnosed at
`:846-857`. This record is the reason a fourth reviewer will not re-route them.

**(2) `O-019` becomes load-bearing** — see §5, which is the inverse edit and
the part that is easy to get backwards.

Prose superseded by the ruling but not itself an authority register, left in
place as dated narrative: `GOAL.md:1316`, `:1400-1412`, `:1748-1749`;
`docs/LETTER_TO_CLAUDE_2026_09_01.md:319`, `:850-859`, `:898`;
`docs/ASPIRATION_LEDGER.md:1885`, `:1407-1412` (where *"parked"* is now the
wrong verb — the state is ruled out, not parked).

## 5. The one row that must NOT close: O-019

`docs/OMISSION_INDEX.md:59` (landed `eaa4a1fa`, 2026-09-01 09:25, sixteen hours
before the ruling) records the invariant *"the batch relation is small and
specialized ON PURPOSE"*, carrying ember's own reason from
`docs/INTENT.md:112-116` and its consequence from `:118-120` — *if it is ever
"simplified" by someone who does not know why, a door closes permanently*.

Its disposition column read, in part: *"if Ember RULES IT OUT, this row is what
stops the option being lost silently on the way, and may then be closed by a
dated ruling that says the door may shut."*

**The 2026-09-01 ruling is that dated ruling, and it says the opposite.**
`GOAL.md:2086-2091`: O-019 *"is now the thing keeping the door open… That
invariant is the whole reason the ruling is safe to make."* Ruling the ambition
out of THIS Clutch while naming a later Clutch as the condition for revisiting
only works if the property that makes a later Clutch feasible survives the
interval. O-019 is that property.

So O-019 takes the **inverse** edit: the clause admitting its closure is
struck, and the row is marked load-bearing BY the ruling. This is the trap the
row was written to catch, and the ruling walked straight into its mouth: a
reader with the ruling in hand and not this record would have closed it.

## 6. Consequences

- **C-15 closes on a ruling, not on code**, and C-00's prohibition on a third
  state holds across the register, the C-16 entry list and the debt ledger.
- **The ruling stops depending on one narrative file.** `GOAL.md` is 3,900+
  lines and is trimmed; `docs/decisions/` and `docs/reference/decisions.md` are
  the register a reviewer reads.
- **O-019 is now guarded by a dated ruling in both directions**: widening the
  batch relation is refused, and closing the row is refused, until a later
  Clutch is actually the project.
- **Nothing is scheduled by this record.** No lane, no charter, no queue entry.
  Reporting the horizon as future work is what `GOAL.md:2093` forbids.

## 7. What would reopen it

Only ember, and only by the condition the ruling itself names: a later version
of Clutch on a substrate that can carry the objective, with minidregg ready.
A reopening is a new record, not an amendment to this one.

## Evidence pointers

`GOAL.md:2067-2094`, commit `5a371810`; `docs/INTENT.md:108-137`, `:179-183`;
`docs/ASPIRATION_LEDGER.md:3-7`, `:118-170`, `:1403-1412`, `:1885-1913`;
`docs/MASTER_COMPLETION_CONTRACT.md:100`, `:185`;
`docs/evidence/DEBT_OWNERSHIP_LEDGER_2026_09_01.md:772-818`, `:887`;
`docs/evidence/C16_ENTRY_LIST_2026_09_01.md:417`;
`docs/evidence/ARCHITECT_SCHOLAR_2026_09_01.md:846-857`, `:1320-1335`;
`docs/OMISSION_INDEX.md:59` (`eaa4a1fa`).
