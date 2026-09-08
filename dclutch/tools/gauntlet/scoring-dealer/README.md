# Scoring Dealer local-validator campaign

Run on hbox through `swarm-build`, from a clean committed checkout:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=6 swarm-build tools/gauntlet/scoring-dealer/run.sh \
  --checked-release-gate /absolute/release/CHECKED_UPGRADE_GATE.json \
  --work /absolute/new/campaign-directory --rpc-port 27176 --census
```

The shared Journey binary owns this campaign's `dealer` command. It brings up
`relayed-vertical/src/substrate.rs`, compiles and founds a market, admits and
funds a separate taker, then calls the shipped Dealer found, quote, fill and
withdraw drivers. It checks a real deposit, a nonzero minted fill's collateral
conservation and exact taker outcome balances, and an exact withdrawal. It
creates no protocol prestate by direct account installation.

The supplied port is the base of the launcher's 42-port block. The validator
is a child guarded by the campaign and stops on every return path. The work
directory must be new; logs, each driver's evidence, and the successful prefix
survive a failure in the transcript. The runner records host and release
source revisions separately and certifies no release. A final release needs
a rerun against its final source and artifacts.

This is an execution campaign, not a declaration that every stage already
passes. Its three native poststate-verifier controls are orchestration
controls and contribute no protocol execution evidence. With `--census`, first
run `tools/gate census` in the same checkout. The runner then selects the four
terminal Dealer transactions from the completed evidence, derives the run's
exact program-id map, and folds
`tools/gauntlet/scoring-dealer/bindings.json` into that census work directory.
It refuses if any of the four transactions is missing. Complete Dealer
settlement and resource retirement remain additional lifecycle work.
