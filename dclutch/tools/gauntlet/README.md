# tools/gauntlet — the standing outside-in functional suite

```sh
tools/gauntlet/run.sh --mode census      # seconds: static route census + report
tools/gauntlet/run.sh --mode full        # unavailable; refuses before any build
tools/gauntlet/hot-cu/run-hot-cu.sh      # the Hot tail's compute, swept over 20 seeds
```

**Start with `DESIGN.md`.** It states why this exists and what makes an
assertion admissible here. `TIERS.md` is the mechanics of adding a tier.

`hot-cu/` is the one entry above that is a MEASUREMENT rather than a campaign:
it admits no evidence and observes no route, and its answer is a pass count
plus a mean, never a margin. **It does not answer "does the public Hot trade
fit under 1,400,000 CU."** Its witness drives the Registry Hot continuation,
which the 2026-08-30 packet demoted to harness-only, and which measures a
constant +35,127 CU above the production top-level route on every comparable
seed (`docs/evidence/CONTINUATION_ROUTE_FIX_OR_RETIRE_2026_08_30.md`,
`8bf6ad40`; `docs/decisions/DECISION_PACKET_2026_08_30.md` §4). The public
figure comes from `direct_hot_top_level_margin_gate.rs`. Read
`hot-cu/README.md` before quoting a number out of it; ledger item M-61 is the
other reason, and neither is optional advice.

## What it is

Campaign tiers drive real transactions built by the chain-derived operators and
submitted to a real validator running real ELFs at real limits. No native
processors or mock programs qualify as campaign evidence.

Alongside the campaign it maintains an **execution census**: a static
enumeration of every program's public dispatch surface and refusal taxonomy,
joined to a ledger of what has actually been driven on a validator, rendered as
EXECUTED / NEVER-EXECUTED per route.

That census is the point. A route that is never executed produces silence, and
silence reads as success in every test report ever written. Found31 was over the
packet limit for months while ~2,300 tests stayed green, and no report anywhere
said "this has never been submitted".

## Outputs

`run.sh --mode census` writes the first three outputs below under `--work`
(default `/private/tmp/dclutch-gauntlet`). Named family campaigns document
their own run, ledger, and ELF paths. The shared checkout's `target/` is never
used, because parallel lanes share this working tree.

| path | what |
|---|---|
| `out/inventory.json` | the statically enumerated route + refusal surface |
| `out/ledger.json` | append-only chain-corroborated execution observations |
| `out/CENSUS.md` | the EXECUTED / NEVER-EXECUTED report |
| `runs/<stamp>/evidence.json` | the campaign's finalized transaction evidence |
| `runs/<stamp>/plan.json` | the bootstrap's hash-pinned genesis plan |
| `runs/<stamp>/ledger/` | the validator ledger, kept as evidence |
| `elf/*.so` | the seven SBF artifacts under test, digest-pinned |

## Supported top-level mode

`run.sh --mode census` is supported. It needs no chain or port and may run
concurrently.

`run.sh --mode full` is not a campaign at HEAD. Tier 1's localhost Market
producer was `demo-market`, which is deliberately retired because a standalone
Registry address cannot authenticate the current Direct facts. The available
`devnet-market` and `graduation-market` planners require acknowledged inputs and
fee-policy choices that this runner does not own. Reusing either silently would
invent a different campaign.

The `full` spelling remains an explicit refusal so old invocations do not fall
through to another mode. It exits 1 before resolving a revision, creating the
work root, checking build tools, or compiling ELFs. Use the family runners under
`tools/gauntlet/` for their named campaigns; use `run.sh --mode census` to
render the accumulated execution report.

`tools/gauntlet/test-run-cli.sh` guards that boundary with hostile build-tool
stubs and proves the refusal exits 1 without creating its work root.

## Ownership

The gauntlet owns `tools/gauntlet/**` and nothing else. It is read-only toward
every protocol source, toward `tools/local-validator/**` (the transaction-only
bootstrap it drives as a subprocess), and toward `apps/dclutch-web` and
`formal/`. A suite that can edit the thing it tests is one refactor away from
being a mirror again.

## Evidence boundary

Local-validator execution is **local-validator evidence**. It is not devnet
evidence and it is not mainnet evidence (`AGENTS.md` names these as distinct
levels). A green gauntlet is not verification and discharges no theorem; it
establishes that the named routes executed on a real validator at real limits
and that the named refusals refused.
