# Trader guide

Choose a market, read its payout rules and trade claims on the outcomes you
want to hold. The public app runs on Solana devnet with test tokens.

## What you hold

In a price-range market, each claim pays one collateral unit if its range wins
and zero otherwise. The market fixes the ranges, price source, observation
window and fallback before trading begins.

For example, buying 100 claims on “SOL below $100” at 0.20 units each costs
20 collateral units before fees. If that outcome wins, the claims pay 100
units. Otherwise they pay zero. Your purchase is fully paid: holding the
claims creates no margin calls, funding payments or liquidation risk.

Some markets use more detailed payout curves. Read the market’s payout chart
for the amount each claim pays at each possible result.

## Before you trade

Check these terms on the market page:

- **Collateral:** the token used to buy claims and pay redemptions.
- **Outcomes and payouts:** including which range contains an exact boundary.
- **Source and timing:** which observation can settle the market and when.
- **Fallback:** which payout applies if the source and funded recovery steps fail.
- **Fees:** the cost added to a purchase or deducted from a sale.

One claim on every outcome forms a **complete set**. In a range market, a
complete set pays one collateral unit. Depositing collateral can create a
complete set; returning one while the market is open releases its backing.
Individual offers can be priced differently, so check the total cost of the
claims you are buying.

## Join and trade

Open **Markets**, choose a market and connect your devnet wallet. Joining
creates your **Position**, which holds your claim balances, and the associated
collateral account. The page shows whether joining is available for that
market.

For a **Direct** trade:

1. Choose the outcome and an available signed offer.
2. Enter the quantity and review the price, fees and resulting balances.
3. Sign your trade terms, then sign the transaction. If a different wallet
   pays the transaction fee, that wallet must also sign.
4. Submit the transaction and wait for confirmation. The page then shows your
   updated balances.

You can reload to check a submitted transaction’s progress. Selling requires
a buyer who accepts your terms; posting an offer does not itself move claims.

The Console’s **Direct trade** page (`/trade`) previews trade calculations.
Use a market’s detail page for wallet trading. Other markets may use
**General** batch orders or **Dealer** liquidity; their pages show the actions
available for the selected mechanism.

## Direct trading fees

The market’s creator fixes its Direct fee rate and recipient when founding the
market. The rate applies to each side. At 50 basis points (0.5%) on a trade
worth 100 collateral units, the buyer pays 100.50 and the seller receives
99.50; the fee recipient receives 1 unit in total. Token-atom rounding applies
to the amounts shown in the transaction preview.

Fees go to the market’s designated recipient and are held separately from
claim collateral. The market page and signed offer show the applicable rate.

## Settlement and redemption

The first valid source observation in the market’s window selects the result.
If the source is unavailable, the market follows its funded recovery steps
and, if those are exhausted, its preselected failure outcome.

After settlement, open **Portfolio** to see the payout for your holdings and
redeem paying claims into your collateral account. Before settlement, you can
exit by selling to another participant or returning complete sets through an
available market route.

## Transfer a bearer claim

Markets with bearer representations can issue claims as Token-2022 tokens.
The Console’s **Representation** page transfers an existing bearer claim
between compatible token accounts when the selected market supports it.
Enter the amount in token atoms and provide the destination account. The
source owner authorizes the transfer; a separate transaction payer can pay the
Solana fee. After submission, the page checks the resulting balances.

## If a transaction fails

Read the displayed reason before retrying. Expired offers need fresh terms;
changed balances need a new preview; a closed market cannot accept a trade.
A failed Solana transaction rolls back its protocol changes but can still
charge a transaction fee. Program error codes are listed in the
[refusal reference](../reference/refusals.md).

Continue with the [reader guide](reader.md) for a worked market example or the
[operator guide](operator.md) to create a market.
