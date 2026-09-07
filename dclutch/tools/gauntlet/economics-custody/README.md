# Custody economics local-validator campaign

Run this on hbox through `swarm-build`, from a clean committed host:

```sh
SWARM_MEM_MAX=32G CARGO_BUILD_JOBS=6 swarm-build tools/gauntlet/economics-custody/run.sh \
  --checked-release-gate /absolute/CHECKED_UPGRADE_GATE.json \
  --work /absolute/new/campaign-directory --rpc-port 47386
```

The campaign links the checked-mutable substrate used by the Dealer campaign.
It drives the canonical `parameters-found`, `parameters-propose`,
`parameters-apply`, and `parameters-withdraw` commands with the Custody
ProgramData retained authority, then `upkeep-found` with a separate funded
campaign payer and a nonzero voluntary Deposit Credit. It reads the Custody
record and applied-change receipt back after each governed transition, captures
finalized accepted transactions, and proves no Hoard principal or Donation
credit was created.

The active record's chain-defined notice is 1,512,000 slots. The campaign first
proves an early Apply refuses, then stops and restarts the same local validator
ledger with Agave's `--warp-slot` at the record's stored earliest Apply slot. It
waits until the early-Apply transaction's slot is observable at finalized
commitment (up to a provisional 90-second local-validator persistence ceiling),
then re-reads the byte-identical standing record before submitting Apply. It
checks the receipt's prior/new digests, generation, proposal slot, activation
slot, and delay. The ceiling is lifted only after measuring a slower
checked-mutable validator root; its present failure is explicit rather than a
restart of an unpersisted ledger. This makes the bounded wall-clock run an
actual validator operation; it does not shorten the governed delay or replace
the ledger.

Its controls are finalized on-chain transactions: a hostile parameters founder
must refuse `ProtocolParametersSbfErrorV1::FoundingAuthority`; hostile Propose
must refuse `UnauthorizedGovernance`; duplicate parameters and upkeep Found
calls must refuse `Record` and `Vault`; early Apply must refuse
`ProposalNotMatured`; and Withdraw after Apply must refuse `NoPendingProposal`.
Each governed-parameter refusal carries byte-exact record rollback; duplicate
upkeep Found carries the same control over its vault. The source report records
the current host revision separately from the checked runtime revision, so this
is diagnostic host-current/runtime-checked evidence rather than a release
claim.
