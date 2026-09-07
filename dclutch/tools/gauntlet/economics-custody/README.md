# Custody economics local-validator campaign

Run this on hbox through `swarm-build`, from a clean committed host:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=6 swarm-build tools/gauntlet/economics-custody/run.sh \
  --checked-release-gate /absolute/CHECKED_UPGRADE_GATE.json \
  --work /absolute/new/campaign-directory --rpc-port 47386
```

The campaign links the checked-mutable substrate used by the Dealer campaign.
It drives the canonical `parameters-found` command with the Custody ProgramData
retained authority, then `upkeep-found` with a separate funded campaign payer
and a nonzero voluntary Deposit Credit. It reads both Custody records back,
captures the finalized accepted transactions, and proves no Hoard principal or
Donation credit was created.

Its controls are finalized on-chain transactions: a hostile parameters founder
must refuse `ProtocolParametersSbfErrorV1::FoundingAuthority`, while duplicate
parameters and upkeep Found calls must refuse `Record` and `Vault` respectively
without changing their accepted poststates. The source report records the
current host revision separately from the checked runtime revision, so this is
diagnostic host-current/runtime-checked evidence rather than a release claim.
