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
  journal/cycle-NNNNNN/cycle.json     planned -> executing -> finalized
  sessions/cycle-NNNNNN/    one Direct session per cycle (driver-owned files:
                            direct-trade-{public,session,producer,finalized}.json + journal/)
  census/cycle-NNNNNN.json  ledger-census observations, chained via --prior
  logs/                     one log per child invocation
  admissions/<name>.done    admission markers
```

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
 "halted":false,"halt_reason":null,"stopping":false}
```
