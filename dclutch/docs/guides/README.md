# Guides

- [Trader guide](trader.md) — what a claim is, what protection costs, and
  what can and cannot happen to your money.
- [Operator guide](operator.md) — how to create a market: what you fix up
  front, how to fund it, and how to pick a resolution window that works.
- [Reader guide](reader.md) — what dClutch is, what works today, and how
  to run the whole thing yourself.
- [Client developers guide](client-developers.md) — building a bot,
  dashboard, or integration: the SDK, the CLI, and a working example of
  each core flow.
- [Two clients](two-clients.md) — this repository ships two command-line
  programs. Which one you have, which one a runbook means, and how to
  build either.
- [Trencher guide](trencher.md) — the same protocol in trench terms: what
  you'd actually hold, why the payout can't be walked back, and the
  standing bounty.

Two generated reference pages back these guides up:
[every error code with its meaning](../reference/refusals.md) and
[the exact byte layouts](../reference/abi/README.md). The rest of the
generated reference — routes, costs, decisions — lives in the
[repository](../reference/README.md).

The seven dClutch programs are deployed on Solana devnet as a cohort — a full
redeploy with fresh ids each time, the previous cohort abandoned in place — and
the live cohort's markets are read off the chain by the site. Every market's
collateral is a devnet test token, so there is nothing to buy with money and
no value at risk. These guides describe the devnet preview and the local
test-chain workflows; neither is mainnet evidence.
