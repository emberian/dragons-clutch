# Structured local lifecycle

Run `run.sh` from a clean committed checkout through `swarm-build`, with a new
work directory and a checked eight-link release gate. Use a checkout-owned Cargo
target and keep the validator ledger under the authorized build filesystem.

Set `DCLUTCH_TOKEN_2022_ELF` to the fetched devnet Token-2022 ELF matching
`token-2022-devnet-20260909.json`. That observation pins the program identity,
deployment slot, upgrade authority, and code hash; the launcher authenticates
the hash before genesis and the loaded code before publication. The file is a
local runtime dependency, outside the first-party release gate. It is installed
with upgrades disabled only in the fresh local genesis. No public Token program
is deployed or modified.

For the separate reproducible fixture control, set `TOKEN_2022_V11_ELF` instead;
its digest must match the Claims fixture provenance. Supplying both variables
refuses. The run records its selection in `substrate/external-token-runtime.json`.

Use `DCLUTCH_TICKS_PER_SLOT=64` on the current shared hbox storage profile so
durable journal flushes fit the recent-blockhash window. This changes local
validator cadence, not a protocol deadline or transaction acceptance check.

```sh
tools/gauntlet/structured-claims/run.sh \
  --checked-release-gate /ABS/CHECKED_UPGRADE_GATE.json \
  --work /ABS/NEW_WORK_DIR --rpc-port 28783
```

The transcript marks completion only after representation retirement and the
canonical capability-root close. It does not imply public devnet execution or
whole-Market retirement. Preserve failed transcripts and finalized progress;
restarting a fresh immutable cohort is distinct from resuming a retained one.
