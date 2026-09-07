# LADDER — a market's funded ordered recovery ladder, on ONE live validator

```sh
tools/gauntlet/run.sh --mode census          # once, for the inventory
# BOTH arms, one at a time, in one invocation: the walk where both legs expire
# unobserved and the ladder exhausts, then the walk where the rung is ANSWERED
tools/gauntlet/ladder/run-ladder.sh --checked-release-gate ABS/CHECKED_UPGRADE_GATE.json
# one arm only, when you are iterating on it
tools/gauntlet/ladder/run-ladder.sh --walk capture \
    --checked-release-gate ABS/CHECKED_UPGRADE_GATE.json
```

Two walks, and both are the tier, which is why `--walk both` is the default —
the same default `run-relayed-vertical.sh` already carries for the same reason.
The exhaust arm is where nothing answers; the capture arm is where the market's
funded alternative does. **Neither arm's witnesses are evaluated against the
other's transcript.** They used to be: the two rung witnesses lived in
`witnesses.json` disjoined over `.walk` and returned their expected value on the
exhaust walk, so a run of the default arm went green having never asked their
question — and a green that cannot tell a silent instrument from a silent chain
is not evidence. They now live in `witnesses-capture.json`, without the
disjunct, behind a witness that pins the arm. `witnesses-exhaust.json` holds the
two the exhaust arm had never had at all, plus the failure walk's own three,
which the failure arm had written to be red on the capture walk ON PURPOSE — a
red nobody is meant to read is a row that trains a reader to ignore reds.
`witnesses.json` keeps only the ones that are about either arm. Running one arm
covers one arm, and the other arm's witnesses are then simply not evaluated
rather than reported green or red.

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
  through activation, and keep the child;
- **`../journey/src/{provider,resolution,stages,ledger}.rs`** — the Pyth
  transport that answers a leg, the terminal admission that moves the Market's
  phase byte, and the conservation ledger that closes L1–L8 at every boundary.
  A tier that wrote a second Pyth transport would be measuring a second author;
- **`successor/src/pyth_lab_publication.rs`**, the producer that mints the
  publication this walk resolves against. See below.

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

## The publication is minted at the run's own hour

**The exhaust walk no longer stops at `Exhausted`.** Decision 0027 says the
ladder exhausts INTO the failure selector and decision 0025 says what that pays,
and since the failure-arm build the tier drives both past the exhausting crank:
`local-private-validator-commit-deadline-failure-v1` commits the Product's own
failure cell from `Exhausted` (22 accounts, the same stranger paid the bounty),
`local-private-validator-admit-terminal-v1` reads the certificate kind off the
Source and moves the phase byte, and `local-private-validator-wallet-terminal-payout-v1`
refunds the founder at every ordinary index into an account the tier opens for
the founder key -- one atom per ordinary claim on the refunding scale, the
Hoard read before and after. The escrow's own payout is recorded as the
producer's refusal ("is this Market's own failure escrow"), never sent. The
transcript's `refund` object carries all of it; on the capture walk it is null.

**A rung capture used to be unreachable here, and the obstruction was the
fixture rather than the wiring.** One `WindowSpecV1.max_age_seconds` governs
*both* the crank's admissibility (`window.end + max_age` is the primary leg's
deadline) *and* the publication's freshness (an observation is admissible only
inside `[now - max_age, now + skew]`), and `window.end` *was* the captured
publication instant. So a shelf life short enough for the primary leg to close
inside a lab run was one under which the frozen capture was already stale for
every rung after it. No value of one field satisfies both legs against a frozen
publication — a field cannot fix a fixture.

So this tier **mints one**. `tools/local-validator/bootstrap/successor/src/pyth_lab_publication.rs`
signs a fresh Pyth publication about any instant a caller names, and the
mechanism is public test material rather than a secret:

- the lab's Wormhole guardian set is the **nineteen dummy guardians** the pinned
  upstream test utilities derive from `secret_i = [i + 1, 0, …, 0]`, and the
  fixture's own `guardian-set-0.account.hex` was checked against exactly that
  derivation, nineteen of nineteen;
- the campaign signs **thirteen of nineteen** — the router's own strict
  two-thirds quorum — over the double keccak of the VAA body;
- the accumulator tree is **single-leaf**, so the root the VAA carries is
  `keccak256(0x00 ‖ message)[..20]` and the receiver's Merkle proof is empty.

The real Wormhole router and the real Pyth receiver ELFs then verify it exactly
as they verify the capture: thirteen recovered secp256k1 signatures, then one
proof against the root. That verification happens **on chain, in this run** —
`journey: the real router cryptographically verifies the signed VAA` — so a
producer that had drifted from the upstream wire fails there rather than in an
assertion it wrote itself.

The instant is the **cluster's own block time**, not the host's wall clock, for
the same reason nothing here warps a clock: a publication stamped against a
clock the chain does not keep would be as dishonest as moving the one it does.
The walk mints two publications about that one instant under two Wormhole
sequences — the primary leg is offered the first, the rung answers with the
second — because a rung answered by a *re-post* of the publication its own
market already declined would answer nothing.

**What remains a lab shape, stated plainly.** The publication is authored by the
lab and signed by keys anyone can derive. It proves the **wire** — the router's
verification, the receiver's posting, Resolution's admission, Core's execution —
and nothing whatever about a price. A guardian set whose keys are derivable is a
guardian set no release catalog can name, and that is the line between this and
any evidence about a real feed.

**The shelf life is this TIER's parameter, never the market's.**
`--publication-shelf-life-seconds` (default 1,200) is how old the campaign will
let the publication *it just minted* be before its own transport refuses it, and
therefore how far past the mint instant the primary leg falls due. A market's
staleness policy is a thing a founder authors and prepays for; this is a
tolerance a lab holds about an artifact it made twenty minutes ago. The window
ends at the mint instant, the primary leg is due one shelf life later, and the
rung `--recovery-rungs` buys is due its own committed interval after that — all
inside the hour one run occupies.

**The recovery leg's admission rule is not the primary's, and that is written
down where it is enforced.** `normalize_authenticated_recovery_update` drops the
age floor rather than widening it: a market stands on a rung only *because*
`now - max_age` expired, so re-applying the floor would make every rung
structurally unanswerable. The rung's bound is instead its own committed
deadline, which the market prepaid at founding. The future-skew ceiling stays on
both legs.

**One consumer still cannot verify such a terminal**, and it is named here so
the next lane does not go looking for a missing producer: the successor's
flagship command pins `route == Primary` and `attempt_index == 0` in two places
(`flagship_resolution.rs:7373-7376`, `:8216`/`:8237`). That is a verifier that
has not been told the ladder exists. This tier does not use it — it asserts the
poststate through `dclutch-provider-transport-v3-operator` and the Source's own
terminal projection — and the pin is a real gap in a different command.

**What the walk now asserts.** Before the crank advances anything, the
chain-derived operator is asked to build the rung's capture and must **refuse**,
off chain, with no key open. After the advance, the same builder handed the
*primary*-shaped request against the same Source must refuse again — and only
then is the real capture sent. The Source's terminal projection reads back
`Recovery` and the certificate reads back attempt index 1, and the journey's
conservation ledger closes L1–L8 at every boundary of the walk.

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
