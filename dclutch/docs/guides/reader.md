# Reader guide

dClutch is a Solana protocol for markets on prices and other measurable
outcomes. Each market defines the possible results and issues claims that pay
when a result wins. Collateral is deposited before claims are issued.

## A price market

Suppose a market asks what SOL/USD will be at 12:00 UTC on Friday. It names the
price source and divides the answer into three outcomes:

| Outcome | Reported price | Payout per claim |
| --- | --- | ---: |
| Low | Below $90 | 1 collateral unit |
| Middle | $90 to less than $110 | 1 collateral unit |
| High | $110 or more | 1 collateral unit |

A report of exactly $90 belongs to Middle; exactly $110 belongs to High.
The ranges cover every price without overlap, so one outcome wins.

If you buy 100 Low claims for 0.20 units each, you spend 20 units before fees.
A price below $90 pays you 100 units. Any other price pays you zero.
You can use claims to take a view on a price or offset some losses elsewhere.
Selling before resolution requires a buyer at a price you accept.

## How claims are backed

One claim on every outcome is a **complete set**. In the example, one Low, one
Middle and one High claim together pay one collateral unit at resolution.
Depositing one unit can
therefore create one complete set. While the market is open, returning a
complete set releases its collateral.

The market holds claim collateral in its **Hoard**. That principal pays claims;
fees, source work and recovery have separate funding. Trading changes who owns
the claims without removing their backing. Claim holders face no margin calls
or liquidations.

## How a market settles

Before opening, a market fixes its outcomes, payout rules, source and observation
window. The first valid source observation in that window determines the
winning outcome. Later observations cannot replace it.

A market can also fund recovery steps before opening. If the source remains
unavailable and those steps are exhausted, anyone can complete the published
failure procedure. It applies the market's preselected failure outcome and may
pay a funded bounty to the person completing the work.

Before participating, check the price source, observation time, range boundaries,
fees and failure outcome. The source determines which claims pay, so its terms
matter as much as the trade price.

## Try dClutch

The public app uses Solana devnet and test tokens. Open a market to see its
terms, current state and available actions.

| What you want to do | Where to go |
| --- | --- |
| Browse markets and compare terms | **Markets** |
| Join a market and trade a signed Direct offer | Open a market's detail page; joining and trading require an eligible wallet and an available offer. |
| View holdings and redeem payouts | **Portfolio**; redemption becomes available when the market has resolved and the wallet holds a paying claim. |
| Follow market activity or inspect an account | **Activity** or **Explorer** |
| Choose outcomes, a source and a trading mechanism | **Design a market** (`/create`); founding the market uses the operator tools. |
| Inspect trade calculations and operate a market | **Console** and the [operator guide](operator.md) |

Markets can use different exchange mechanisms: **Direct** matches a maker's
signed offer with a taker, **General** collects orders into batches, and
**Dealer** uses funded liquidity to quote prices. Check the selected market's
page for the actions it currently supports.

## Next steps

- [Trader guide](trader.md): choosing claims, trading and redeeming.
- [Operator guide](operator.md): choosing market terms and funding a market.
- [Client developers guide](client-developers.md): building a bot, dashboard or integration.
- [Reference](../reference/README.md): instructions, account layouts and error codes.
