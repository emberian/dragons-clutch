# DEVNET-ACTIVITY-001 — disposable devnet activity wrapper

Source: Dragon's Clutch commit `9546db35`, paths
`dclutch/tools/release/devnet-activity.sh` and
`dclutch/tools/release/test-devnet-activity.sh`.

The retained operational invariant is narrow: a public-cluster campaign must
name devnet's genesis hash, an explicit payer, and a dedicated state directory;
it may read only its newly generated participant keys, and must prove resumed
work from chain-visible balance or recorded-signature checks.  It must never
infer a wallet, a token-source account, or permission to replay a trade.

This is a user-directed byte-level port after the wrapper was authored in the
wrong sibling worktree.  Both repositories are Ember's private AGPL-3.0-or-later
dClutch worktrees; no third-party source, protocol implementation, account
codec, or generated DTO is involved.  The new semantic owner is
`tools/release/`: it orchestrates existing `solana`, `spl-token`, and dClutch
CLI surfaces and owns no on-chain authority.

The port uses current top-level paths.  It deliberately rejects the earlier
worktree layout and does not add any dependency.  Its adversarial local mock
test covers wrong external transport only through a mocked devnet genesis,
fresh-key creation, explicit SOL/token funding, a recorded Direct-trade
signature, replay invocation, and public reconciliation.  The test does not
claim devnet or mainnet execution.
