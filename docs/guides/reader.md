# Reader guide

What the dClutch demo will be when it lands, what it will refuse to claim,
and how to run the whole thing yourself today. This is the guide to read
first if you are neither trading nor operating — just deciding whether the
project is what it says it is.

## The demo, when it lands

**None of this is live.** Devnet deployment is deferred by explicit decision
and requires named authorization; this section describes the intended shape
so the work is legible, and it stays labeled this way until the day it is
not.

The demo is the completed protocol live on **devnet**, resolving markets
about the state of **Solana mainnet** — pool prices, token graduations, the
majors. The cross-cluster truth transport is disclosed, not hidden:

- Majors' prices need no trust bridge at all: Pyth's devnet deployment
  already carries mainnet-derived prices under the existing adapter.
- Everything else rides `RelayedMainnetStateV1`: an off-chain daemon reads
  finalized mainnet account state and signs attestations of **raw bytes** +
  slot + owner (+ the owner program's ProgramData digest, so program-identity
  checks work across clusters). The relayer attests observations, never
  interpretations — all decoding happens on-chain under pinned decoding
  rules, so replacing the trust root never moves semantics. v1 accepts this
  proof-of-authority relayer as the disclosed cost of doing this at all;
  the candidate permissionless upgrade (Wormhole Queries) and the hardening
  path (relayer quorum, TEE signer) are written down in
  [`docs/design/MAINNET_STATE_RELAY.md`](../design/MAINNET_STATE_RELAY.md).

In the browser you will see market discovery, a market's cells and prices,
range/tail products over real feeds, a portfolio derived straight from chain
state without an indexer, and — where the protocol refuses — the refusal,
with its meaning, because the browser's decoders enforce the same grammars
the chain enforces.

What the demo will **not** claim, even when live: it is devnet, not mainnet;
it is unaudited; its evidence stays labeled at exactly the level it reaches;
and nothing about it is an invitation to put value at risk.

## Running it today, locally

Everything the demo will do on devnet already executes locally, and you can
watch it:

```sh
# the tier-1 campaign: builds the seven programs, boots a local validator,
# founds a Market atomically, opens it, checks witnesses and CU budgets:
tools/gauntlet/run.sh --mode full

# static census + report only (seconds, no chain):
tools/gauntlet/run.sh --mode census

# the frontend suite, including every generated-ABI byte-compare gate:
cd apps/dclutch-web && npm test
```

The campaign's validator is killed when the run returns; the frontend can be
pointed at the retained ledger by resuming it
(`tools/gauntlet/frontend/resume-validator.sh`) — that is exactly how the
frontend met the first live open Market
([evidence](../evidence/FRONTEND_LIVE_OPEN_MARKET_2026_08_27.md)).

## Deciding whether to believe it

The project's epistemics are the product as much as the protocol is:

- [The routes reference](../reference/routes.md) states, for every
  instruction route, whether an in-tree campaign executed it, why it is
  blocked if not, and prints the routes with neither — the doctrine is that
  a route ships with the campaign row that executes it or ships marked
  never-executed.
- [The refusal reference](../reference/refusals.md) names every way the
  protocol says no, per program, with meanings from the enums' own doc
  comments.
- [The budgets reference](../reference/budgets.md) pins what the golden
  transactions cost and refuses its own file when a transaction stops
  fitting under Solana's compute ceiling.
- [The decisions index](../reference/decisions.md) records why the
  architecture is what it is, including the decisions that were reopened.
- Evidence levels are nontransitive and say so: fixtures, ProgramTest, local
  validator, devnet, mainnet. Lean theorems cover named models and per-case
  corpora — the cases, and nothing else; universal refinement is named as
  parked debt, not assumed.

If a claim you find anywhere in this repository is not labeled with its
evidence level, that is a bug — the same kind as a route with no census row.
