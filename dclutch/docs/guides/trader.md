# Trader guide

What you hold when you hold a dClutch claim, what it can and cannot do to
you, and how to read what the protocol tells you.

Seven protocol programs are deployed on Solana devnet, and one devnet market is
open for trading — the first one on any dClutch deployment whose trading was
switched on before its deadline. Its collateral is a devnet test token, so
nothing here is bought with money. This guide describes the trading path that
market runs, which is the same path exercised on local test chains and in test
harnesses. No trade has been made on it yet.

## What a claim is

A market asks one question with a bounded, checkable answer — say, where
SOL/USD is at noon on Friday. The possible answers are split into buckets
called **cells**, fixed when the market is created. Every claim is a claim
on one cell.

A claim pays **one collateral unit** if the answer lands in its cell, and
**zero** if it doesn't. That is the whole product.

One claim on every cell — a **complete set** — pays exactly one unit no
matter what happens. So the protocol treats a complete set and a
collateral unit as the same thing: deposit a unit and you mint a complete
set; return a complete set and you get the unit back. That deposit is
where every claim comes from. The collateral sits in the market's vault
(its **Hoard**) before any claim exists, and it does nothing but pay claim
holders.

What this means for you:

- The most you can lose is what you paid for your claims. Ever.
- There is no leverage, so there is no liquidation, no margin call, and
  no funding rate. Nothing can force-close your position.
- Because a complete set is always worth exactly one unit, cell prices
  always sum to exactly one unit. A cell priced at 0.07 units is the
  market pricing that outcome at seven cents on the dollar.

## Buying protection

"Protection against SOL below $100" is not a special product. It is
claims on every cell below $100. If SOL resolves below $100, exactly one
of your cells wins and pays you one unit per claim. If not, your claims
expire worthless and the seller keeps what you paid — like an insurance
premium.

The same shape covers a range ("between X and Y") or a tail ("above Z"):
pick the cells, buy claims on each. The price of the bundle is the sum of
the cell prices, exactly.

No price feed watches your position along the way, because there is no
position to liquidate. The only moment that matters is resolution.

## Getting into a market

Before you can hold claims in a market you need a **Position** in it. A
Position is an account that belongs to you and holds your claim balances,
alongside a collateral account that funds them. Both live at addresses
worked out from the market and your own wallet, so nobody assigns you one
and nobody can hand you someone else's — the addresses are yours before
either account exists. Joining is what creates them.

Today you join from the command line:

```sh
dclutch join --plan <market plan> --campaign-evidence <market evidence> \
  --position-owner <your address> --position-owner-keypair <your key> \
  --output <where to write the report>
```

Two things worth knowing before you run it.

**It does not send anything unless you tell it to.** Without `--execute` it
preflights: it reads the market, works out your addresses, builds the exact
transaction, and writes a report — then stops. Run it, read what it says it
will do, and only then run it again with `--execute`.

**You need the market's own documents.** The plan and campaign evidence
describe the market you are joining; they are published alongside a public
market's evidence, or written by your own local run. You cannot join a
market by address alone, and that is deliberate: what you sign should be
checkable against something the market published, not assembled from a
name.

Against a devnet market you also have to say so out loud, by passing
`--i-mean-devnet` with that cluster's genesis hash. A local chain needs no
such acknowledgement, and passing one anyway is refused rather than
guessed at.

You can fund the Position as you join, with
`--collateral-source-owner-keypair`, `--collateral-source-account` and
`--collateral-quantity-atoms`. Give all three or none: a half-specified
funding is refused instead of being interpreted. Leave them off and you
join with a Position holding nothing, which is a perfectly good place to
start.

By default the fee payer is you. Name a different one with
`--fee-payer-keypair` if somebody else is paying.

The web app can show you where you stand in a market — whether you have a
Position, what it holds, what joining would create — but it cannot build
the admission transaction for you yet. That is a gap we intend to close,
not a rule.

## How the market resolves

Every market pins its source when it is created — a specific price feed,
down to the exact program deployment it trusts — and names a time window
with real width. The first valid observation from that source inside the
window settles the market; every later one is rejected. No committee, no
vote, nobody to appeal to — and nobody to be surprised by.

If the source publishes nothing through the whole window, the market
takes a fallback outcome that was disclosed and funded before it opened.
You know before you trade exactly what silence produces.

## When the protocol says no

dClutch refuses any transaction that doesn't check out exactly: wrong
account, wrong signer, stale state, a window that hasn't opened, a
replay. A refused transaction rolls back completely — your collateral
stays exactly where it was, and you're out a transaction fee and nothing
else.

Every refusal carries a code naming the program that refused and why. The
full list, with meanings, is in
[the refusal reference](../reference/refusals.md). A refusal isn't a
malfunction; it's the protocol keeping the market's rules.
