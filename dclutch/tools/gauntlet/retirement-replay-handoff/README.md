# retirement-replay-handoff — the Trading-to-Core replay handoff, on real ELFs

Two routes row **C-10** owned and no campaign drove:
`core/retirement_replay_handoff_v1::process` and, beneath it by CPI,
`custody/retirement_replay_handoff_v1::process`. The retirement-only handoff
closes the Trading-role Custody replay and creates the Core-role one at the same
seeds, which is the act that lets the aggregate-retirement chain close a replay
Core owns rather than one Trading does.

`programs/dclutch-core-sbf/tests/retirement_replay_handoff_program_test.rs`
has driven both against the real Core and Custody ELFs since it landed — with
EXACT refusal codes for the replay and all six hostilities, no bare `is_err()`
anywhere. It still read `NEVER-EXECUTED, no stated reason` in the route
register, because it called `record()` for nothing. The only change to the
campaign is that `submit` takes a label and records; `record()` is a no-op
unless a runner sets `DCLUTCH_PROGRAM_TEST_EVIDENCE_DIR`.

## What the eight transactions say

| transaction | CU | code |
|---|---|---|
| the handoff | 404,580 | accepted; Custody alone spends 259,511 |
| replayed against a prestate it no longer has | 75,814 | `Reference` |
| hostile Context | 53,435 | `Reference` |
| hostile ReplayDigest | 75,274 | `Reference` |
| hostile PartialCoreReplay | 68,406 | `Reference` |
| hostile Rent | 8,579 | `Creation` |
| hostile Phase | 11,242 | `Market` |
| hostile Release | 47,255 | `Release` |

Two facts the fold made visible that reading the campaign would not:

- **Only the accepted handoff reaches Custody.** All seven refusals stop inside
  Core before the CPI, and no Custody frame appears in their logs. So the
  Custody route is bound to exactly one row, and a witness asserts the count —
  binding it to a refusal Custody never saw would be the false green the census
  exists to remove.
- **Every frame fits a legacy packet.** All eight 23-account frames serialise to
  1,209 bytes against Solana's 1,232-byte maximum, with no Address Lookup Table.
  That is worth asserting separately, because the aggregate-retirement chain
  this route feeds does *not* fit without one — see
  `tools/gauntlet/retirement-checkpoint/README.md`.

## Running it

    tools/gauntlet/run.sh --mode census          # once, for the inventory
    tools/gauntlet/retirement-replay-handoff/run-retirement-replay-handoff.sh

The runner gates on `cargo check` before it builds anything, refuses on any SBF
stack-frame-overwrite diagnostic, refuses if the fold is not exactly eight
transactions, and evaluates five witnesses before folding. It is a **ProgramTest
fast lane**; `TIERS.md` states the bar.
