# Restart after the usage interruption — 2026-09-09

User has a constrained Team account now. Use one lead agent; no standing swarm.
The prior work-until-9am goal stopped at the usage limit. Protocol completion,
fresh devnet redeployment and a running devnet aquarium are unfinished.

## Where things stand

Live tree: `/Users/ember/dev/dclutch`; publish through `tools/cut.sh` into
`/Users/ember/dev/dragons-clutch`. Read AGENTS.md. The shared dirty and untracked
files contain interrupted native/client work; do not discard or blanket-commit.
Copy-only formatting noise also reappeared after an earlier cleanup.

The website, explorer, participant screens and reader/trader/operator guides
have extensive plain-language changes committed. GitHub Pages has not been
redeployed with this batch. Cohort 17 remains the previous public deployment.
Native CLI/WASM/browser work for General, Dealer and Structured is partly
committed and partly unfinished. Do not enable incomplete flows merely because
the UI exists.

Local execution has real results: a Direct simulator completed three nonzero
fills and fee settlements and resumed without sending again; older selected
Dealer/Ensemble lifecycles reached payouts and cleanup. These are separate
source revisions, not a final integrated release.

## Immediate operational issue

At 15:43 UTC, hbox reported `/tank` with zero available space. Five surviving
dClutch validators were suspended with SIGSTOP and confirmed in T state at
15:44 UTC. Their ledgers were not deleted. Resume with SIGCONT only after
recovering space and choosing the continuation to run:

| PID | Run | RPC |
| --- | --- | --- |
| 440761 | fd7 Direct simulator floor | 46024 |
| 443622 | a8 Structured runtime | 30400 |
| 751542 | a3dd Series diagnostic runtime | 31100 |
| 4106046 | older 691 Direct runtime | 43144 |
| 4166530 | older 691 Structured runtime | 28783 |

Pause record: `/private/tmp/dclutch-validators-paused-20260909.json` on Mac.
Earlier approved incremental-cache cleanup records are under
`/tank/dregg-build/dclutch-cache-cleanup-20260909/`. Preserve evidence, frame
objects, deployed ELFs and ledgers. Do not recreate all build targets.

## The shortest path to a usable aquarium

1. Preserve interrupted changes and identify one coherent source revision.
2. Fix existing Direct token setup for a second seller on a used market.
   The first seller initializes the shared fee destination. A later seller has
   a vacant seller account but an initialized fee account; current setup rejects
   that combination. This prevents genuine A-to-B then B-to-A activity.
   Start in successor `direct_trade_token_setup.rs` and Trading
   `token_setup_v1`; the simulator participant rotation changes are unfinished.
3. Execute first seller, second seller, reciprocal trades, fees and restart on
   one compatible local validator. Complete the existing public Direct client
   path. Terminal buy work is unfinished; immediate sell also needs the native
   SDK preparation generalized beyond a buyer taker.
4. Freeze the source, build all eight programs on hbox, update frame baselines,
   finish generated artifacts, deploy fresh identities, and run the simulator.
   Then publish the deployment bindings and existing Pages app.

This delivers a first usable live Direct aquarium. General, Dealer liquidity,
Structured and Series still need completion sequentially; that larger goal
must not be described as finished by a Direct launch.

## Remaining family blockers at interruption

- General: Buy Place/Submit accepted. Sell passed the first composition check
  but another wire-sizing reader still decoded unnormalized zero-transfer rows.
  Verify reached an Effect comparison: Result rent scalar150 lacked a producer,
  despite correct current-Rent scalar147 and corrected Credit beneficiary.
  Fixes and matched-order helper are in the dirty tree, not established as a
  completed Buy/Sell settlement lifecycle. Larger widths have real CU/bank bounds.
- Structured: fresh Market `CPbaUQsiedRBCr5CpkzqPaoquqF2beAoy5ajoisrbC5d`
  on preserved a8 bank opened and activated coordinates0/1 (544206/556206CU).
  Next operation refused a readonly account also used as fee payer; use the
  correct separate payer and resume this market. Log:
  `/tank/dregg-build/structured-a8e3-v2-runtime-20260909/fresh-selection-vacancy.log`.
  Four-action native planner, CLI/browser wiring and automatic lookup-table
  provisioning remain unfinished.
- Dealer: multi-LP V4/native discovery topology and full typed wallet flow are
  still being integrated. Existing older Dealer execution is not proof of this
  new complete flow.
- Series: shared corpus and explicit occurrence/finalized replay authoring
  landed in393f260aa/ae6c56170. Later action source producers, selected all-eight
  build and full current-source validator recurrence remain outstanding.

## Funding

Last finalized devnet balance read at08:43 UTC: deployer
`4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` held26.572399090 devnet SOL.
An earlier fd7 eight-ELF rent quote was about41 SOL before fees/market funding.
Requote final binaries. Faucet permission was requested previously but remains
unanswered; no faucet action was taken. Old cohorts must remain abandoned in
place under standing authority.

## Last small native completion

c7398d06f adds the shared closeable-Mint encoder for native poststate planners.
Custody check and two focused encoder tests passed on hbox; the writer was
compared with the Token-2022 SDK. Frame capture remains owed. Separate uncommitted
Token state projection helpers belong to the interrupted Dealer work.
