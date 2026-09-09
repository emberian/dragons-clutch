# Claims, trades and payouts

A dClutch range market lets you buy a fixed payout on a price outcome. You
choose the range, quantity and price you will pay. The market already holds
the collateral needed to pay its claims.

## A trade in numbers

Suppose a market settles on Friday’s SOL/USD price. You buy 100 claims on
“$120 to less than $180” for 0.30 collateral units each.

- **You spend:** 30 units, plus fees.
- **If that range wins:** you redeem 100 units.
- **If another range wins:** those claims pay zero.

The source, observation window, range boundaries and failure outcome are
fixed before the market opens. There are no margin calls or funding payments
on these fully paid claims. To sell before settlement, you need a buyer.

## Try a market

The public app uses Solana devnet and test tokens. Open **Markets**, select a
market and read its terms. Connect a devnet wallet to join, review an available
Direct offer and trade. **Portfolio** shows your claims and available payouts;
**Activity** shows your transactions.

A signed offer contains the quantity, price limit and expiry. Review the fees
and resulting balances before signing the matched transaction.

## When the source stops answering

A market can fund recovery steps and a completion bounty when it is created.
If no valid observation arrives and its recovery steps are exhausted, anyone
can finish the published failure procedure. The market then uses its
preselected failure outcome, and the caller can collect the funded bounty.

The failure outcome determines which claims pay. Read it before taking a
position.

The [trader guide](trader.md) covers joining, trading, fees and redemption.
The [reader guide](reader.md) explains complete sets and collateral backing.
