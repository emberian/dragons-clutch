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
- [Trencher guide](trencher.md) — the same protocol in trench terms: what
  you'd actually hold, why the payout can't be walked back, and the
  standing bounty.

Two generated reference pages back these guides up:
[every error code with its meaning](../reference/refusals.md) and
[the exact byte layouts](../reference/abi/README.md). The rest of the
generated reference — routes, costs, decisions — lives in the
[repository](../reference/README.md).

The seven dClutch programs are deployed on Solana devnet, and one market on it
is open for trading. That market's collateral is a devnet test token, so there
is nothing to buy with money and no value at risk. These guides describe the
devnet preview and the local test-chain workflows; neither is mainnet evidence.
