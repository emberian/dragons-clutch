# dClutch load simulator

Sustained, rate-controlled, multi-wallet activity against a live cluster —
the thing that makes a market look and be alive.  Participants join, run
Direct trade sessions, churn, and a reconciliation loop proves conservation
as it runs.  It is an ORCHESTRATION layer in Python over accepted drivers
that own their own signed journals; it builds no transaction and reads no
key bytes itself.

## What it drives

| step | driver |
|---|---|
| admission (join) | `dclutch-local-successor-bootstrap {local-private-validator,devnet}-user-position-admission-v1` |
| Direct session production | `...-direct-trade-produce-v1` (loopback) / `devnet-direct-trade-produce-v1` (sha-pinned inputs) |
| Direct session execution | `...-direct-trade-v1 --session S --execute` — one invocation per durable mutation (replay-setup, token-setup, lookup-*, capability-seal, hot), pulsed until `direct-trade-finalized.json` exists |
| wallet minting + funding | `tools/release/devnet-activity.sh` (the activity harness's keygen + exact-target envelope funding + signature markers; devnet only; NEVER the deployer as a participant) |
| reconciliation | `dclutch-local-successor-bootstrap ledger-census` — read-only, evaluates the conservation-ledger laws, exits nonzero on any violation; `--prior` chains the delta laws across cycles |

## Files

- `simulator.py` — the sustain loop.  `run --config C [--cycles N | --sustain] [--execute]`.
  Default is a one-cycle PREFLIGHT that signs nothing; `--execute` opts into
  mutation exactly like the house drivers.  `mint-wallets` mints + funds
  extra devnet participants through the activity harness.
- `simcore.py` — cluster-free primitives: per-cycle write-ahead journal
  (resume-never-resend), rate control with jitter + refusal-aware backoff,
  atomic `status.json` writer, durable halt.
- `build_config_from_probe.py` — builds a config from a HELD local
  private-validator probe (see below).
- `test_simcore.py`, `test_simulator.py` — unit + fake-driver tests
  (`python3 test_simcore.py && python3 test_simulator.py`).  The fake driver
  honors the real artifact contract, so the loop, resume, backpressure, halt
  and SIGTERM paths are all proven without a validator.

## Work-dir layout

```
<work>/
  status.json               rewritten atomically every cycle (see schema below)
  HALT.json                 present only after a conservation violation; its
                            presence refuses restart until removed by a human
  EXIT.json                 how the run ended, when it was able to say; never
                            refuses a restart.  Cleared at startup, so its
                            ABSENCE is a live claim (see "Death, honestly")
  journal/cycle-NNNNNN/cycle.json     planned -> executing -> finalized
  sessions/cycle-NNNNNN/    one Direct session per cycle (driver-owned files:
                            direct-trade-{public,session,producer,finalized}.json + journal/)
  census/cycle-NNNNNN.json  ledger-census observations, chained via --prior;
                            bounded, see "Storage" -- only the newest few
                            survive, and the newest is still the whole series
  logs/                     one log per child invocation
  admissions/<name>.done    admission markers
```

## Storage, as a number

A census file is not one observation: `ledger-census --prior P` reloads P's
array, appends what it just observed, and re-serializes the chain.  The newest
file is therefore the WHOLE series, which is the property
`apps/dclutch-web/scripts/simulator-series.mjs` draws the run from.  It is
also why the directory used to cost the SUM of its files.

Measured on the market18 devnet run: **3,871 bytes per observation**,
cycle-000001.json 3,793 B, cycle-000123.json 476,055 B, **28 MB of directory
at 123 cycles** -- `b·N(N+1)/2`, so **1.94 GB by cycle 1000**.  On 2026-08-30
that filled the machine's data volume and every lane on the box lost its shell
to ENOSPC, including this run, which was killed mid-cycle 124.

Two bounds now hold it (`simcore.CensusRetention`, config block
`census_retention`):

| knob | default | what it does |
|---|---|---|
| `keep_files` | 2 | every file older than the newest is a strict PREFIX of it, so superseded files are redundancy, not record.  O(N²) → O(N). |
| `window` | 480 | the newest file is truncated to its last `window` observations before it becomes the next `--prior`.  O(N) → O(1). |
| `disk_floor_bytes` | 2 GiB | the run stops BETWEEN cycles when the volume drops below this, while it still has room to record that it stopped. |

**Worst case on disk is `keep_files × window × bytes-per-observation`, constant
in the number of cycles run** — at the defaults and market18's measured
per-observation size, **2 × 480 × 3,871 = 3,716,160 bytes, about 3.7 MB**.
That size depends on how many accounts a config tracks, so the run MEASURES it
every cycle and publishes the ceiling it actually implies in
`status.json`'s `census_retention` block, rounded up.  A bound you cannot read
off the artifact is not a bound.

Truncation is **lossless for every conservation law**, and that is a property
of the ledger rather than a hope: each delta law reads exactly one
predecessor, `self.observations.last()` — L2 at
`tools/gauntlet/journey/src/ledger.rs:463`, L5 at `:551`, L6 at `:577`, L7 at
`:653` — and the census's own verdicts and exit code come from
`observations.last()` alone
(`tools/local-validator/bootstrap/successor/src/main.rs:502-522`).  Nothing
reads the prefix.  Truncation drops whole array ELEMENTS and never edits one,
which `test_simcore.py` proves by round-tripping a real market18 observation
to the census tool's own bytes.

## Death, honestly

The run that died on 2026-08-30 left `halted: false`, `stopping: false` and no
halt record: its own artifact still claimed health while the process was gone.
Three things changed, and the third is the honest one.

1. **`status.json` carries its own expiry.**  Every write stamps
   `heartbeat.expected_next_update_by` — the instant by which a LIVING run
   must have written again.  A reader compares it to their own clock and needs
   to know nothing about the simulator.  The deadline is derived, not picked:
   one jittered period + the backoff currently in force + `grace_seconds` for
   one cycle of child processes (default 300 s).  At the devnet cadence that
   is 20·1.25 + 0 + 300 ≈ 5.4 minutes, widening to 7.4 under a capped 429
   backoff.
2. **`EXIT.json` records how the run ended** — completed, signalled, halted,
   low-disk or crashed — written from a `finally`, so a crash gets a record as
   readily as a clean stop.  It never refuses a restart: how a process ended
   is a fact about the process, and only a conservation divergence
   (`HALT.json`) is a fact about the ledger that a human must clear.
3. **Some deaths cannot be recorded, and this does not pretend otherwise.**  A
   SIGKILL runs no handler; an ENOSPC fails the write that would describe it.
   Those leave no halt record and no exit record, BY CONSTRUCTION.  What makes
   them legible is absence: the deadline in (1) passes with no record from
   (2) beside it.  `EXIT.json` is cleared at startup precisely so that its
   absence is a claim about this run rather than a leftover.

Rerunning over finalized cycle journals is a byte-identical no-op and
re-invokes no driver.  A changed plan under an existing journal refuses
(`JournalConflict`) rather than resuming somebody else's run.

## Rate control

`cadence.period_seconds` (jittered ±25%) between cycles, plus
`trade.step_pause_seconds` between session pulses.  Any child surfacing
rate-limit markers (429 / Too Many Requests / blockhash not found) triggers
exponential backoff (5s → 120s cap) and the SAME cycle retries — the
drivers' durable journals make the retry a resume, never a resend.  Devnet
runs stay strictly sequential: the successor already paces itself at 250 ms
per RPC call and one busy writer starves the whole per-IP budget.

## Stopping

SIGTERM/SIGINT finish the in-flight cycle, seal its journal, write a final
`status.json`, and exit 0.  A census violation instead writes `HALT.json`
and exits 3; the work dir then refuses restart until the file is removed
deliberately.

## Local proof (held validator)

1. Stand up the held probe (~16–18 min; work dir must not exist):
   ```
   cd <clean clone at the gate commit> && python3 tools/release/private-validator-lifecycle/run.py \
     --repo <that clone> --release-root <gate dir> \
     --expected-release-gate-sha256 <gate sha> \
     --validator <solana-test-validator> --solana <solana> \
     --work /private/tmp/dclutch-sim-hold-NN \
     --through participant --seeds 1 --hold-after-participant
   ```
   At the hold it writes `runs/seed-01/participant-handoff.json` and SIGSTOPs
   itself; the validator stays alive.
2. Build the config and run:
   ```
   python3 tools/load-simulator/build_config_from_probe.py \
     --probe-work /private/tmp/dclutch-sim-hold-NN \
     --sim-work /private/tmp/dclutch-sim-run-NN \
     --output /private/tmp/dclutch-sim-run-NN.config.json
   python3 tools/load-simulator/simulator.py run \
     --config /private/tmp/dclutch-sim-run-NN.config.json --cycles 3 --execute
   ```
3. Teardown: `kill -CONT <run.py pid>` (the supervisor is in state T); it
   authenticates the handoff and tears the validator group down itself.
   Never kill the validator directly.

## Devnet flip

Same loop, config swapped: `cluster.label = "devnet"`, an https RPC URL,
`cluster.devnet_genesis` spelling the genesis hash in full, and the
`trade.devnet` block naming the sha-pinned producer inputs (plan,
market-input, campaign report, buyer participant, checked execution release,
per-pair seller/buyer tickets) from the campaign-open facts.  Mainnet is
refused unconditionally.  Start gentle: `cadence.period_seconds` of a few
seconds minimum, one process, no concurrency.

## status.json schema (`dclutch-load-simulator-status-v1`)

```json
{"schema":"dclutch-load-simulator-status-v1",
 "cluster":{"label":"local|devnet","rpc_url":"..."},
 "market":{"address":"..."},
 "mode":"finite|sustain","started_at":"iso","updated_at":"iso",
 "cycles":{"run":0,"target":3},
 "trades":{"landed":0,"signatures":["... last 50"]},
 "wallets":[{"address":"...","role":"participant","sol_lamports":0,"source":"staged|minted"}],
 "last_reconciliation":{"ok":true,"checked_at":"iso","output":"census path"},
 "halted":false,"halt_reason":null,"stopping":false,
 "heartbeat":{"cadence_seconds":20.0,"jitter_fraction":0.25,"grace_seconds":300.0,
              "backoff_seconds":0.0,"budget_seconds":325.0,
              "expected_next_update_by":"iso","note":"..."},
 "census_retention":{"window":480,"keep_files":2,"files":2,"removed_files":0,
                     "observations":480,"dropped_observations":1,
                     "bytes_on_disk":0,"bytes_per_observation":0,"bytes_bound":0}}
```

`heartbeat` and `census_retention` are additive; the /pulse decoder
(`apps/dclutch-web/lib/simulatorStatus.ts`) pins the fields it renders and
tolerates the rest.

## Secrets

The live endpoint is `https://devnet.helius-rpc.com/?api-key=<the key in
~/.helius-key>`, and it is passed to every driver as `--rpc-url`.  Redaction
happens where a value is **stored**, never where it is passed, so no caller
can forget:

| written file | redacted by |
|---|---|
| `status.json` | `StatusWriter.__post_init__` — the writer never holds the key at all |
| `journal/*/cycle.json` | `cycle_plan`, at the point the plan is recorded |
| `HALT.json` | `halt_loudly`, over its reason and every string in its details — it quotes the failing command, and the command is `--rpc-url <url>` |
| `EXIT.json` | `record_exit`, over the crash detail |

`logs/` is the exception and deliberately so: it is a child's raw stdout, kept
verbatim for diagnosis, never committed and never published.  The published
artifacts are only `status.json` and the series derived from the census;
`simulator-series.mjs` re-checks both against `~/.helius-key` before writing,
and `publish.sh` refuses a subtree carrying it.

---

# simlife: many markets, each with a personality

`simulator.py` watches ONE market and runs one canned walk. `simlife.py` draws
a POPULATION — markets of different archetypes, widths and fuses, with
participants who are not all the same person — and interleaves their whole
lifecycles into one ordered event schedule.

```
python3 simlife_drive.py plan   --config C [--out world.json]   # draws a world, touches nothing
python3 simlife_drive.py run    --config C [--execute]          # walks it against the substrate
python3 simlife_drive.py routes                                 # which driver owns each route
```

## The line the engine will not cross

> The engine decides **what to attempt and when**. The census decides **what is
> true**.

No number `simlife.py` invents ever reaches a series point. Every market
quantity comes from `ledger-census` observing accounts on a chain, exactly as
before. What the engine contributes is a plan and a record of what happened to
it — and an event has four possible endings, three of which are not "it worked":

| ending | means |
|---|---|
| `executed` | a transaction landed, or an account was read |
| `refused` | the route exists and the chain said no |
| `unattempted` | this substrate has no such route |
| `blocked` | a prerequisite of this event never executed |

Folding the last three together would turn one wall into a hundred failures.

## Market archetypes

Each is a bundle of **distributions**, not a market: two markets of the same
archetype differ in every number.

| archetype | cells | basis | deadline (slots) | destinies |
|---|---|---|---|---|
| `coin-flip` | 2 | categorical | log-uniform 2,000–20,000 | resolves 8 / fails 1 / sleepy 1 |
| `short-fuse` | 2–4 | categorical | log-uniform 120–1,200 | fails 6 / resolves 3 / sleepy 1 |
| `ladder` | 4–8 | **ramp, degree 1** | log-uniform 8,000–60,000 | resolves 7 / sleepy 2 / fails 1 |
| `tent-band` | 3–6 | **tent, degree 1** | log-uniform 4,000–30,000 | resolves 6 / fails 2 / sleepy 2 |
| `wide-field` | 6–12 | categorical | log-uniform 10,000–80,000 | resolves 5 / sleepy 4 / fails 1 |
| `quiet-corner` | 2 | categorical | log-uniform 40,000–200,000 | sleepy always |

Every archetype is **zero-fee** and the draw refuses a nonzero rate where it is
drawn: fee-bearing founding does not fit in one transaction on today's wire
(`docs/evidence/FEE_SECOND_TRANSACTION_FOUNDATION_2026_08_30.md`), so a world
that drew a rate would be a world whose markets cannot be founded at all.

**Only `CategoricalQ1` is FOUNDABLE today.** Degree-0 and degree-1 graded bases
— `Constant`, `RampUp`, `RampDown`, `Tent` — decode, evaluate and settle on the
wire, but `compile_linked_basis_v3`
(`tools/local-validator/bootstrap/successor/src/market.rs:1683`) hard-wires
`CategoricalQ1` and founding refuses any other kind at `market.rs:3487`. The
ladder and the tent are kept anyway: an archetype table that only contains what
today's compiler emits cannot say what is missing. A substrate declares which
kinds it can **express**, and a founding it cannot express is `unattempted` with
that sentence — never a failure, and never quietly redrawn as a categorical
market wearing a ladder's name. Use `archetype_mix: "foundable-today"` for a
world a real substrate can drive end to end.

## Participant personas

| persona | admits | active | redeems | compacts strangers | cranks |
|---|---|---|---|---|---|
| `eager-maker` | +0–1 ticks | 6–10 | 95% | 20% | 30% |
| `patient-maker` | +2–8 | 2–5 | 90% | 10% | 15% |
| `prompt-redeemer` | +1–4 | 1–3 | 100%, same tick | 5% | 5% |
| `sleeper` | +0–6 | 1–4 | **never** | never | never |
| `crank` | +0–3 | 0 | never | 60% | always |
| `compactor` | +1–5 | 0–2 | 80% | always | 70% |

`sleeper` is the one that earns its keep. A holder who never returns is not an
inactive account — their claim check sits on the chain occupying rent a stranger
can recover by compacting it, and that compaction is permissionless. A world
with no sleepers never exercises it; a world that models sleepers as "did
nothing" never notices that somebody else did something to them.

Compaction is scheduled against every **dormant** holder, not only `sleeper`s: a
crank who took a position and only ever cared about the permissionless steps
leaves exactly the same account behind. The persona is recorded so a reader can
tell them apart.

## Reproducibility

Everything is drawn from one recorded seed **preimage** — a sentence, so a run
can be named and re-run by typing its name — through **independent named
streams**: each draw site derives its own generator from
`sha256(preimage ‖ domain ‖ index)`. That is the property worth paying for.
Market 3 of an eight-market world and market 3 of an eighty-market world are the
same market; with one shared `Random` they would not be, and two runs of a
slightly edited world would never be comparable again.

A plan records the **distribution** a value came from, not only the value.
`deadline 4096` says nothing about whether that was inevitable; `log-uniform
over 512..32768 → 4096` says what the world was actually like.

## Two clocks, and the number that reconciles them

A market's deadline is in **chain slots**. A run's horizon is
`ticks × slots_per_tick`. A world can easily be drawn where no market reaches
its deadline before the run ends — and then every resolution, failure walk,
redemption and retirement falls outside the horizon and the run watches twelve
markets hold still.

Neither clock is adjusted to flatter the other:

- `plan` prints how many markets reach a terminal boundary inside the horizon,
  and says out loud when the answer is none;
- `cadence.period_seconds` paces the conductor between ticks (jittered, seeded
  from the run's own seed, so the pauses replay too). Without it a sixty-tick
  run finishes in eight seconds and the finalized slot moves by two;
- the run **measures** what a tick was actually worth. `measured_pace` reads
  each market's census chain back and reports the slots that really advanced per
  tick beside the number the plan assumed.

## Config (`dclutch-simlife-config-v1`)

```json
{"schema":"dclutch-simlife-config-v1",
 "cluster":{"label":"local","rpc_url":"http://127.0.0.1:PORT/"},
 "bootstrap_bin":"/abs/dclutch-local-successor-bootstrap",
 "work_dir":"/abs/work",
 "substrate_label":"what this chain is, in one sentence a reader will see",
 "source_revision":"HEX40",
 "cadence":{"period_seconds":20.0,"jitter_fraction":0.25},
 "substrate":"ledger-census|lifecycle",
 "world":{"seed":"dclutch/simlife/DATE/name","markets":12,"ticks":45,
          "archetype_mix":"design-space|foundable-today","slots_per_tick":176},
 "bindings":{"m06":{"mint":"...","payer":"...","hoard":"...","aggregate":"...",
                    "claim_unit_atoms":1,"outcome_count":2,"basis":"categorical-degree-0",
                    "positions":{"label":"PUBKEY"},"tokens":{"label":"PUBKEY"}}}}
```

A **binding** attaches a planned market to a market that ALREADY EXISTS on the
chain. Three refusals guard it, all before any census runs: a binding naming no
planned market; a binding whose `outcome_count` disagrees with the plan's; and,
when the operator states one, a `basis` that disagrees with the plan's. That
join is the only place in the pipeline where a caption could come apart from
its chart — a two-cell market filed under an eleven-cell archetype draws two
bars under a promise of eleven, and a categorical market filed under a ladder is
captioned with a payout shape it does not have.

## Work-dir layout

```
<work>/
  world.json                  the whole plan, with every draw and its distribution
  ledger.json                 every planned event and what became of it
  status.json                 simcore's status artifact, plus a `simlife` block
  census/<market_id>/cycle-NNNNNN.json   one chain PER MARKET
  logs/<market_id>/           one log per driver invocation, named for its route
  wallets/<participant>.json  one funded local wallet per participant  (lifecycle)
  markets/<market_id>/attempt-NN/         one founding attempt: keys, the compiled
                                          MarketRunInput, and the campaign report
  HALT.json / EXIT.json       simcore's halt and exit discipline, unchanged
```

The census chain is per market because the conservation ledger is per market:
one Hoard, one aggregate, one Mint, and a delta law that reads exactly one
predecessor. Two markets sharing a chain would compare one's Hoard against the
other's, and L2 would be arithmetic about nothing.

## Capturing it for the site

```sh
node apps/dclutch-web/scripts/simlife-series.mjs --work /abs/work [--out F] [--check]
```

writes `dclutch-simulator-series-v4`: v3's single-market document at the top
level (so every existing surface keeps drawing), plus `world` — the seed, the
substrate, every planned market observed or not, and route by route what the run
could not do — and `markets`, one sub-series per observed market. A planned
market that was never observed appears in `world.planned` and never in
`markets`, because a market with no points must not be drawn as a market whose
line is flat at zero.

## Two substrates, and a config chooses

`substrate` names one, and the default is the read-only one, because a run that
MUTATES a chain should have said so in a file somebody reviewed rather than
acquired the ability by upgrading a module.

| substrate | what it does |
|---|---|
| `ledger-census` (default) | observation and nothing else, against markets that already exist. Every mutation is `unattempted` with the driver that would perform it named. |
| `lifecycle` | founding, admission, fills, resolution, the failure walk, redemption, retirement and the census over all of it — each through the SHIPPED driver that owns the route. |

A `lifecycle` config adds one block:

```json
 "substrate":"lifecycle",
 "lifecycle":{"plan":"/abs/plan.json",
              "campaign_payer_keypair":"/abs/campaign-payer.json",
              "founding_founder":"PUBKEY","substituted_founder":"PUBKEY",
              "solana_keygen":"/abs/solana-keygen",
              "driver_timeout_seconds":1800}
```

and needs no `bindings` at all: a market this run founds is bound from the
FOUNDING's own evidence, so the census observes exactly the accounts the chain
gave it rather than anything a config typed by hand.

`simlife_drivers.py` is one function per route and each is a subprocess. Nothing
in it builds a transaction, derives a PDA or copies a constructor — that is
FOUND-5182 stated as a module boundary, where a hand-written copy of a kernel
constructor drifted three bytes and walled every local founding for a day while
the "independent" control passed.

### Four things this substrate learned by running

- **The admission packet does not fit a legacy message.** It routes through the
  founding's OWN frozen DCLTGMF3 lookup table; passing all five founding tables
  refuses `DuplicateAddress`. The founding evidence does not record its address,
  and does not have to: a frozen table is one whose authority is `None`, and the
  founding's own is the frozen table whose address list contains the market.
- **The owner wallet must already exist and be funded**, or the driver refuses
  `snapshot missing required account` before it compiles anything, and at
  FINALIZED commitment rather than confirmed.
- **A partially consumed founding key set is unresumable.** A retry is a whole
  new key set in its own `attempt-NN/` directory; the abandoned attempt stays on
  disk under its own name.
- **A market's terminal window ENDS at the captured fixture publication**, which
  is in the past on every local chain — so a local market is past its terminal
  boundary the instant it is founded, and `--terminal-window-width-seconds` is
  how far back the window reaches rather than how long anybody waits.

## The one route with no driver anywhere

**Claim-check compaction by a stranger.** It is implemented and green in
ProgramTest (`programs/dclutch-claims-sbf/tests/claim_check/mod.rs`, sixteen
tests including
`a_market_retires_a_sleeping_holders_position_and_the_holder_is_still_paid`) and
no gauntlet binding names it, so it is covered and census-unbound.

It is **sized rather than built**, because a compaction this module wrote by
hand would be exactly the mirror the driver layer exists to avoid: one new
`local-private-validator-claim-check-compaction-v1` subcommand shaped like
`…-wallet-terminal-payout-v1` — read the holder's claim check and the market's
terminal receipt, build the one Claims instruction the ProgramTest already
calls, journal the packet before the send, verify the poststate. **6–10 hours**
for the subcommand, its argument parser, its journal domain and one hostile test
that a holder cannot compact their own check; **plus 1–2 hours** for the gauntlet
binding, since an unbound transaction fails the census. Driver tier, not
protocol: no program changes.

## Tests

`python3 test_simlife.py` — no validator. The engine is proven by
assertion (determinism, stream independence, exact conservation through a
Dirichlet split, total ordering, every destiny and persona reachable, a sleeper
never redeeming, a compactor never compacting themselves); the driver layer is
proven against a fake bootstrap that honours the real `ledger-census` contract,
including the halt on a violated law and the refusal to restart afterwards.
