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

## Concurrent runs, on disjoint port blocks

This used to say "one run at a time, machine-wide", and it was true: the
launcher was pinned to `http://127.0.0.1:20890/` and refused to start while
anything else listened there, so two lanes could not run `--mode full`
concurrently whatever `--work` roots they passed.

The origin is a parameter now. It is in no authenticated material — not in the
keypair derivation, not in a program address, not in a semantic release ID, not
in an artifact attestation, not in the genesis plan — so moving it moves nothing
a budget row or a witness reads, and it is deliberately not in the campaign
stamp: changing it must not cost a 13-minute re-run.

```sh
tools/gauntlet/run.sh --mode full                    # 20890, as it always was
tools/gauntlet/run.sh --mode full --rpc-port auto    # a free 42-port block
tools/gauntlet/run.sh --mode full --rpc-port 31890   # a base you chose
```

The launcher derives its whole block from that base — `rpc BASE`,
`faucet BASE+2`, `gossip BASE+3`, `dynamic BASE+10..BASE+41` — and BASE 20890
reproduces the historical `20890-20931` block byte for byte, so nothing that
never asked for a port notices.

`auto` is resolved at the **campaign** stage, not at argument parse: a base
chosen at parse time is six minutes of SBF builds away from being used, and it
scans a band below the kernel's ephemeral range, because the ephemeral range is
the one the kernel also hands to every ordinary outbound connection. Both halves
are measured — the first parallel attempt drew ephemeral `49952` at parse time
and found it occupied when it got there.

Two campaigns sharing a `--work` root still collide on everything else in it, so
give each run its own. And `census observe` is a read-modify-write of one
`ledger.json` that every family runner defaults to; both runners take an atomic
lock around the fold, so concurrent campaigns serialise there rather than losing
each other's observations.

If a port you asked for is occupied, `run.sh` refuses before the launcher's
sixty-second timeout with

    gauntlet: 127.0.0.1:20890 is occupied. Pass --rpc-port auto to take a free base instead.

Still never kill a `solana-test-validator` whose `--ledger` is not under your
own `--work` root. A validator started by a campaign is now bound to its
supervisor's lifetime and dies with it even if the supervisor is SIGKILLed, so
a leaked one should no longer be something you find.

`--mode census` needs no chain and no port; run it freely and concurrently.

A corollary that cost this lane an hour: **never edit `run.sh` while a run is in
flight.** Bash reads a script incrementally by byte offset, so an edit mid-run
shifts what it reads next and it will re-execute or skip a block. Wait for the
run, or copy the tree.

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
