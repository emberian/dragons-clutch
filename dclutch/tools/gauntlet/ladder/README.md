# LADDER — a market's funded ordered recovery ladder, on ONE live validator

```sh
tools/gauntlet/run.sh --mode census          # once, for the inventory
tools/gauntlet/ladder/run-ladder.sh --checked-release-gate ABS/CHECKED_UPGRADE_GATE.json
```

Decision 0027 built the funded ordered ladder. `6a3079454` taught the
successor's market compiler to found a market that buys one
(`--recovery-rungs BPS:SECONDS_AFTER_PREVIOUS`). `61706bc9a` gave the
permissionless crank its driver (`advance-recovery`). All three were tested
offline or in `solana-program-test`, and none of them had ever met a chain
together, because the sequence they form — found, crank, answer or exhaust — is
**three commands against one live cluster** and tier 1 founds and resolves
inside a single process whose `runtime::found_through_open` owns the validator
child.

This tier is that cluster.

## What it is made of, and what it does not copy

Every line of the founding, the crank and the substrate bring-up is compiled in
by `#[path]` from somewhere else:

- the tier-1 producer's modules, from
  `tools/local-validator/bootstrap/successor/src/`, exactly as the journey and
  the relayed vertical link them;
- **`recovery_crank.rs`**, the shipped
  `local-private-validator-advance-recovery-v1` driver. This campaign calls its
  entry point with an argument vector. A tier that built its own 18-account
  frame would be measuring a second author instead of the driver a host runs;
- **`../relayed-vertical/src/substrate.rs`**, the one bring-up in this tree
  that leaves a validator RUNNING for a caller to drive more than one command
  against — prepare the checked-mutable substrate, spawn a
  `solana-test-validator` over the prepared account directory, administer
  through activation, and keep the child.

If any of them moves, this build breaks. That is the intended tripwire; the
rule is the journey's rule — **extend the module set, never fork a file.**

## No clock is warped, and that is load-bearing

`SourceResolutionStateV2::crank_recovery_ladder` refuses while
`current_unix_seconds <= due`. The last second an honest observation may land
and the first second a crank may run are **different seconds**, and that single
conjunct is what stops the funded failure walk from being a shortcut around the
legs a market's holders paid for. A campaign that moved its validator's clock
to make its own hostile pass would be measuring a market it had edited, and the
hostile would become unfalsifiable.

So this campaign reads `due` off the market's own published `WindowSpecV1` and
`RecoveryPolicyV2` against the cluster's own clock, and when a leg is not yet
due it **records the two seconds and stops**. `--max-wait-seconds` is the whole
budget a walk may spend waiting; a leg further away than that is reported, not
slept for. A transcript that says `not-yet-due` is an honest account of a walk
that stopped, which is worth more than a green one that did not happen.

## The build stage is the gate

`TIERS.md` asks a tier's build stage to refuse artifacts the SBF backend calls
potentially-undefined, because `cargo build-sbf` exits zero when it reports that
a call overwrites its own stack frame. This tier does not count diagnostics
itself: it **requires a checked release gate**, and
`tools/release/checked-release-candidate.sh` emits `CHECKED_UPGRADE_GATE.json`
only in strict mode, which refuses a nonzero count. A gate that exists is that
proof, made by the one stage in a position to make it.

It also names the revision. The campaign binary is built from `git archive` of
the gate's own `source_revision`, so the host code and the ELFs it drives come
from one commit rather than two.

## What it does not reach, and why

**A rung CAPTURE cannot be driven on this fixture.** Not for want of a builder:
`dclutch-provider-transport-v3-operator` derives the execute request's
`source_index` and its source-spec identity from the Source's own phase and
active attempt, so the capture that answers a rung is buildable today. The
obstruction is the fixture's arithmetic. One `WindowSpecV1.max_age_seconds`
governs **both** the crank's admissibility (`window.end + max_age` is the
primary leg's deadline) **and** the publication's freshness (an observation is
admissible only inside `[now - max_age, now + skew]`). `window.end` IS the
captured publication instant. So a shelf life short enough for the primary leg
to close inside a lab run is one under which the frozen publication is already
stale for every rung after it. A rung answered on a loopback needs a
publication the lab can refresh — a fixture question, not a wiring one.

The successor's flagship command is separately unable to *verify* such a
terminal: `flagship_resolution.rs` pins `route == Primary` and
`attempt_index == 0` in two places (`:7373-7376`, `:8216`/`:8237`). That is a
verifier that has not been told the ladder exists, and it is named here so the
next lane does not go looking for a missing producer.

## What this is NOT

1. **Not devnet evidence and not mainnet evidence.** One loopback validator.
2. **Not a fast lane.** There is no `solana-program-test` here and there will
   not be one: the founding fails all four of `TIERS.md`'s conditions, and the
   crank's whole subject is a wall-clock deadline a bank with a settable
   sysvar cannot honestly measure.
3. **Not a CU budget.** The figures the transcript carries are single draws on
   a run with fresh keypairs, so `find_program_address` bump-search noise is in
   them. They are the first loopback numbers for these routes and they are
   quoted as first numbers.
