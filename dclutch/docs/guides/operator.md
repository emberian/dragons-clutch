# Operator guide

Creating a market means choosing its terms, funding its obligations and
publishing it on chain. The outcomes, source rules and payout terms are fixed
before trading begins.

Use **Design a market** in the app to explore the terms, and the **Console**
for market operations. The [founding walkthrough](../operators/found-a-market.md)
covers the command-line workflow. Rehearse your configuration on a local
validator before creating a devnet market.

## Choose the market terms

| Choice | What to decide |
| --- | --- |
| Collateral | The token backing claims and its admission rules. |
| Outcomes | The complete set of possible results, their boundaries and payouts. |
| Source | The feed or other source that supplies the result. |
| Observation window | When an observation is eligible and how old it may be when submitted. |
| Recovery and fallback | Funded recovery steps and the payout used if they are exhausted. |
| Trading mechanism | Direct signed offers, General batches or Dealer liquidity. |
| Fees | The selected mechanism’s rate and recipient. |
| Program releases | The deployed programs and capabilities used by this market. |

For a price-range market, include every possible price and assign each boundary
to exactly one range. The payout chart should make the result clear at the
boundaries as well as between them.

## Fund the obligations

Claim collateral goes into the market’s **Hoard**. Source work, recovery,
rent and fees have separate accounts and funding requirements.

| Funds | Purpose |
| --- | --- |
| Claim principal | Pay claim holders. |
| Trading and settlement principal | Fund the selected trading mechanism. |
| Source-work funding | Pay for resolution work. |
| Recovery reserve | Fund the selected recovery steps. |
| Failure bounty | Pay the caller completing the failure procedure. |
| Account rent | Create and maintain the required Solana accounts. |
| Realized fees | Pay the market’s designated fee recipient. |

Native SOL and collateral tokens are separate amounts. Review both before
founding. Claim principal cannot be used to cover another obligation.

## Open the market

The founding workflow has three stages:

1. **Prepare the market.** Publish and check its outcomes, source, capabilities
   and selected program releases.
2. **Fund custody and operations.** Create the required accounts and fund their
   named obligations.
3. **Found and open.** Commit the market terms, lock collateral, initialize
   claims and enable trading.

The final stage can use one transaction or a funded two-step permit, depending
on the selected market. A two-step founding commits the opening terms first;
anyone can then complete that exact opening. The permit also defines how an
incomplete opening is refunded.

Solana allows at most 1,400,000 compute units in one transaction. Simulate the
selected route before submitting it. The [transaction budgets reference](../reference/budgets.md)
and the founding walkthrough cover route sizes and measured costs.

## Choose a usable resolution window

Allow enough time for the source to publish and for a caller to submit its
observation. A narrow window can send a market to fallback even when the source
is generally working.

Measure publication intervals for the feed you intend to use. Choose a window
covering several intervals, with room for delays. The `max_age_seconds`
parameter separately limits how old an observation may be when submitted;
a wider observation window does not remove that limit.

The first valid observation settles the market. Later observations cannot
replace the result. Check the selected source’s deadline rules in the
[reference](../reference/README.md) when scheduling resolution and recovery.

## Recovery and failure

Choose recovery sources, activation conditions and funding before opening.
If a recovery source supplies the required statistic, the result uses the same
market payout rules. If all selected routes are exhausted, the published
failure outcome becomes available.

Anyone can complete the failure procedure once its conditions hold. A funded
bounty pays the caller; replaying the completion cannot collect it again.
Choose a fallback outcome your participants can understand and show it beside
the normal outcomes.

## Follow the market

Use the market page and Explorer to inspect its state, collateral, claims and
source progress. Keep the required source or keeper process running through
the observation window. After settlement, complete the selected cleanup and
retirement steps so account rent and remaining operational funds reach their
specified recipients.

The [client developers guide](client-developers.md) covers integrations. Program
instructions, account layouts and error codes are in the
[reference](../reference/README.md).
