# Reader guide

What dClutch is, what works today, and how to tell a demonstrated protocol
step from a button you can use. Read this first if you are deciding whether the
idea makes sense before you trade or run a market.

## What dClutch is

dClutch is a Solana protocol for markets on real-world numbers. A market asks
one question and splits the possible answers into a finite set of **cells**.
You can hold claims on those cells. A claim on the cell that wins pays its
stated collateral unit; claims on the other cells pay zero.

Every claim is fully backed by collateral deposited before the claim exists.
There is no leveraged position that needs a margin call or liquidation. That
limits the protocol's payout liability; it does not guarantee a good price, a
buyer before resolution, or that an outside data source is correct.

### One bounded-price example

Suppose a market asks:

> What will the pinned SOL/USD source report at 12:00 UTC on Friday?

The market fixes these cells before it opens:

| cell | report that belongs in it | payout per winning claim |
| --- | --- | ---: |
| Low | below $90 | 1 collateral unit |
| Middle | $90 through under $110 | 1 collateral unit |
| High | $110 or higher | 1 collateral unit |

The boundaries are part of the product. A report of exactly $90 is Middle and
a report of exactly $110 is High. The cells cover every report without overlap,
so there is always one winner and never two.

One claim on every cell is a **complete set**. It always pays one collateral
unit, because exactly one member wins. The protocol can therefore lock one unit
in the market's **Hoard** and mint one complete set; while the market is open,
returning a complete set releases one unit. This is the whole backing rule:

```text
one collateral unit -> Hoard principal -> Low + Middle + High complete set
pinned source report -> exactly one cell -> that cell's claims redeem
```

Before resolution, people may value the example cells at 0.20, 0.55, and 0.25
units. Those example prices add to one unit, like the complete set; they are
not a forecast or a quote from dClutch. A person seeking protection below $90
would hold Low claims. A range or tail is just a bundle of the relevant cells,
whose price is the sum of their individual prices. It needs no separate
liquidation engine.

### What the collateral can and cannot pay for

The Hoard's **principal** is reserved for claim payouts. It is not a general
purpose balance. Custody keeps separate compartments for trading principal,
realized fees, liveness work, recovery reserve, and Series escrow; a transfer
from Hoard principal to a fee vault is refused.

This is why a fee or a source-work bounty cannot quietly reduce the collateral
promised to winning claims. A founder has to fund each named obligation before
opening the market.

### Resolution is a stated procedure, not a vote

Every market pins a source procedure before it opens: the exact source and
program release, what statistic is read, and the time window in which it may
answer. The first valid observation inside that window maps through the fixed
cells and settles the market. Later observations cannot replace it. There is no
committee or founder discretion after opening.

Some markets can also preselect funded recovery depth. The source state has
primary, recovery, resolved, exhausted, and failure states, and a funded
recovery ladder has been exercised on real program bytes. Its participant-facing
meaning is simple: a recovery route is chosen and funded before the market
opens, never improvised after an inconvenient result.

If no usable source answer arrives by the disclosed deadline and selected
recovery is exhausted, anyone can take the permissionless **failure walk**.
It commits the already-published failure outcome and can pay the market's
pre-funded bounty to the person doing the work. This is the planned answer to
unavailable source data, not a vote or a founder override.

### Why use a bounded claim market?

Fixed-payout claims make a clearly worded future number into small exposures
that can be held, exchanged, and combined. That can support a view on a future
number, a bounded range hedge, or a market price for an outcome. The useful
properties are concrete: the outcomes, payout rule, and source procedure are
stated before participation, and the claim collateral is reserved for claims.

They do not make every question well specified, create liquidity, guarantee a
profit, or certify the truth of the source. A participant still needs to decide
whether the question, terms, source, and available price are worth accepting.

### A few terms

| term | meaning |
| --- | --- |
| **Cell / outcome** | One member of the market's exhaustive, non-overlapping answer partition. |
| **Claim** | A holding on one cell, with its fixed terminal payout. |
| **Complete set** | One claim on every cell; worth one collateral unit at resolution. |
| **Hoard** | The custody compartment holding principal for claim payouts. |
| **Position** | A participant's market-specific claim and collateral state. |
| **Venue** | The mechanism selected for exchange, such as Direct, General, or Dealer. |
| **Recovery ladder** | A preselected, funded sequence of source-recovery steps. |

## What works today

The recorded cohort 17 deployment contains eight protocol programs — a full
redeploy with fresh ids each time, so the addresses are not permanent — and the
live cohort's markets are read off the chain by the public app, which labels
anything it cannot authenticate instead of filling in missing facts. Every
market's collateral is a devnet test token, so there is nothing to buy with
money.

Markets have been founded and opened on devnet; strangers have been admitted;
a stranger's fee-bearing Direct fill has crossed and its fee has been settled by
a third party; a Pyth SOL/USD observation has been captured inside a market's
window; the market has settled; and winning claims, including a stranger's,
have been paid into ordinary wallet token accounts. A market whose source went
silent has taken its disclosed failure path, and a devnet market has completed
the checkpointed retirement route. A later local-validator diagnostic also
completed a Direct payout-and-retirement tail and reached terminal recovery
exhaustion. Those diagnostics exercise named program bytes; they are not a
fresh release campaign. [`GOAL.md`](../../GOAL.md) indexes the dated record;
signatures and poststates are in the [cohort-17 evidence](../evidence/COHORT17_SEATED_FILLED_RETIRING_2026_09_06.md).

The site reads its selected devnet cohort and must be treated as a projection:
the authenticated on-chain accounts and checked release information are the
authority. Its current entrances have different powers:

| goal | current entrance | what it means |
| --- | --- | --- |
| Watch or inspect | Watch, Markets, Activity, and Explorer | Read market and account facts from the selected devnet cohort. |
| Join and trade Direct | A detailed Market page (`/markets/<address>` or `/market`) | The page can check the market, join an eligible wallet, use a signed maker offer, persist the exact transaction before signing/sending, and verify the finalized poststate. It offers no trade when the market or route fails its checks. |
| Inspect Direct arithmetic | Console `/trade` | A deliberately read-only route and fill-arithmetic inspector; it does not request a wallet signature or submit. |
| Inspect or redeem a Position | Portfolio / redeem surfaces | They read the wallet's market state and expose a wallet operation only when the checked market and payout route admit it. |
| Design a market | `/create` | A browser planning and input-checking surface, not a self-service founding button. |
| Run lifecycle operations | Local-validator runbooks and operator tooling | These are the paths that have founded, resolved, and paid campaign markets; they are not an arbitrary public creator or source-submission service. |

The exchange and claim families are at different stages:

| family | what it provides | recorded execution and remaining work |
| --- | --- | --- |
| Direct | A maker and taker agree to a bilateral exchange. | Nonempty trades, resolution and payout have run on devnet; the detailed Market page is the wallet entrance. |
| General | Orders participate in a bounded batch. | Founding and OpenBatch have executed; a complete accepted nonempty lifecycle remains owed. |
| Dealer | A funded participant quotes under a bounded-loss scoring rule. | A [local-validator market](../evidence/DEALER_ACCEPTED_LOCAL_VALIDATOR_2026_09_08.md) accepted Quote, a nonzero Fill and Withdraw with collateral checks. Terminal settlement and a public Dealer market remain owed. |
| Fractional | Native claims back transferable Token-2022 shards. | [Wrap, transfer to another holder and WholeUnwrap](../evidence/claims-fractional-validator-2026-09-08/README.md) executed on a local validator. That run did not establish the terminal lifecycle. |
| Structured | Claims compose exposures across market coordinates. | Component and program tests exist; the complete composed local-validator lifecycle remains work in progress. |
| Series | One precommitted schedule creates recurring markets. | [Native recurrence tests](../evidence/SERIES_NATIVE_RECURRENCE_2026_09_08.md) cover successive occurrences; the complete local-validator lifecycle remains owed. |

These results name specific sources and runtimes. Current development must
complete its own execution tests before a fresh devnet deployment and running
public simulator. A Console workspace named after a family does not establish
that an arbitrary wallet can use it today.

The TypeScript SDK and two command-line clients build and check related flows.
`dclutch` reads and authors tickets but never submits. `dclutch-terminal` has
durable-journal support for founding, joining, and redeeming, while its `buy`,
`sell`, and failure-walk submission paths deliberately refuse. A refusal is not
an alternate entrance.

## The plan

Pyth's devnet feeds carry the major prices used in this preview; everything
else can arrive through a relayer that publishes signed copies of mainnet
account data, checked and decoded on chain
([`docs/design/MAINNET_STATE_RELAY.md`](../design/MAINNET_STATE_RELAY.md)).
The mechanism agenda ([decision 0031](../decisions/0031-the-mechanism-agenda.md))
is the design layer under construction. This remains an unaudited devnet
demonstration, not a place to put money at risk.

## See it run

```sh
# build the programs and enumerate every route they accept (no chain):
tools/gate census

# found a market on a throwaway local validator and walk it: the `ladder`
# gauntlet tier stands the validator up, founds, opens and completes (the
# private-validator lifecycle runner it replaced was deleted on 2026-09-04;
# SIMPLIFY_DRIVERS.md §3 says why). It needs a checked release gate to bind to:
tools/gauntlet/ladder/run-ladder.sh --checked-release-gate /absolute/checked/CHECKED_UPGRADE_GATE.json

# the web app's test suite:
cd apps/dclutch-web && npm test
```

The lifecycle run builds its own local chain, founds one market, admits one
participant, and tears the chain down when it finishes; `--through
participant` requires exactly one seed. The operator walkthrough
[found-a-market](../operators/found-a-market.md) shows the same command with
its measured output.

Every instruction, error code, and measured cost is in the
[reference](../reference/README.md), generated from the code itself.
