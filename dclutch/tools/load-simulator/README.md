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
