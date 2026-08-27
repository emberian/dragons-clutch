# tools/gauntlet — the standing outside-in functional suite

```sh
tools/gauntlet/run.sh --mode census   # seconds: static route census + report
tools/gauntlet/run.sh --mode full     # the tier-1 campaign on a real validator
```

**Start with `DESIGN.md`.** It states why this exists and what makes an
assertion admissible here. `TIERS.md` is the mechanics of adding a tier.

## What it is

Every interaction is a real transaction, built by the chain-derived operators,
submitted to a real `solana-test-validator` running the real ELFs at real
limits. No genesis-injected protocol state beyond what the transaction-only
bootstrap legitimately deploys; no native processors; no mock programs.

Alongside the campaign it maintains an **execution census**: a static
enumeration of every program's public dispatch surface and refusal taxonomy,
joined to a ledger of what has actually been driven on a validator, rendered as
EXECUTED / NEVER-EXECUTED per route.

That census is the point. A route that is never executed produces silence, and
silence reads as success in every test report ever written. Found31 was over the
packet limit for months while ~2,300 tests stayed green, and no report anywhere
said "this has never been submitted".

## Outputs

Everything lands under `--work` (default `/private/tmp/dclutch-gauntlet`); the
shared checkout's `target/` is never used, because parallel lanes share this
working tree.

| path | what |
|---|---|
| `out/inventory.json` | the statically enumerated route + refusal surface |
| `out/ledger.json` | append-only chain-corroborated execution observations |
| `out/CENSUS.md` | the EXECUTED / NEVER-EXECUTED report |
| `runs/<stamp>/evidence.json` | the campaign's finalized transaction evidence |
| `runs/<stamp>/plan.json` | the bootstrap's hash-pinned genesis plan |
| `runs/<stamp>/ledger/` | the validator ledger, kept as evidence |
| `elf/*.so` | the seven SBF artifacts under test, digest-pinned |

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
