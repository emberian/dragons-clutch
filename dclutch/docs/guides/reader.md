# Reader guide

What dClutch is, what works today, and how to see it run. Read this first
if you're not trading or running a market yet — just working out what this
project is.

## What dClutch is

dClutch is a Solana protocol for markets on real-world numbers. A market
asks one question — where will SOL/USD be at noon on Friday? — and splits
the possible answers into buckets. You buy claims on buckets; claims on
the winning bucket pay out. Every claim is fully backed by collateral
deposited before the claim existed, so there is no leverage, no
liquidation, and no way to lose more than you paid.

A pinned price feed resolves each market — no committee, no discretionary
judge. If the feed goes silent, the market takes a fallback outcome that
was published and funded before it opened.

## What works today

Seven protocol programs are deployed on Solana devnet as a cohort — a full
redeploy with fresh ids each time, so the addresses are not permanent — and
the live cohort's markets are read off the chain by the public app, which
labels anything it cannot authenticate instead of filling in missing facts.
Every market's collateral is a devnet test token, so there is nothing to buy
with money.

Markets have lived whole lives on devnet: founded and opened, strangers
admitted, a stranger's fee-bearing fill crossed and its fee settled by a
third party, the Pyth SOL/USD feed captured inside a market's window, the
market settled on an honest certificate, and winning claims — a stranger's
included — paid into ordinary wallet token accounts. A market whose source
went silent took its disclosed fallback. The last step, retiring a market all
the way, has not completed on any chain. The dated record of each first is
[`GOAL.md`](../../GOAL.md); the signatures are in the cohort documents under
[`docs/evidence/`](../evidence/COHORT15_DEPLOYED_SEALED_FOUNDED_CAPTURED_2026_09_04.md).

What runs only on real program bytes in a test bank, not yet on a public
chain: the Dealer, Series and Structured families' lifecycles, and General's
first candidate batch.

The web app reads the live cohort, lists its markets, reads portfolio state
from chain, and signs and submits a Direct fill from a browser wallet. Static
browser data remains an untrusted view of the on-chain accounts. A TypeScript
SDK and two command-line clients build and check the same flows.

## The plan

Pyth's devnet feeds carry mainnet prices for the majors; everything else can
arrive through a relayer that publishes signed copies of mainnet account
data, checked and decoded on chain
([`docs/design/MAINNET_STATE_RELAY.md`](../design/MAINNET_STATE_RELAY.md)).
The mechanism agenda ([decision 0031](../decisions/0031-the-mechanism-agenda.md))
is the design layer under construction. It remains an unaudited devnet
demonstration, not a place to put money at risk.

## See it run

```sh
# build the programs and enumerate every route they accept (no chain):
tools/gate census

# found a market on a throwaway local validator and join it as a
# participant. It builds its own chain and tears it down afterwards, and it
# needs a clean committed checkout and a checked release root to bind to:
python3 tools/release/private-validator-lifecycle/run.py \
    --repo /absolute/clean/dclutch \
    --release-root /absolute/checked/release \
    --validator "$(command -v solana-test-validator)" \
    --solana "$(command -v solana)" \
    --work /absolute/scratch/outside/the/repo \
    --through participant --seeds 1

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
