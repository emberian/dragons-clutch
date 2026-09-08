# Trader guide

What you hold when you hold a dClutch claim, what it can and cannot do to
you, and how to read what the protocol tells you.

This guide describes the current-source trading path exercised on local test
chains and devnet. Devnet assets and executions are public-test evidence, not
mainnet evidence. Treat a live deployment as a dClutch deployment only when its
checked release manifest authenticates the programs and profile it names.

## Which browser page can trade?

The public site has two Direct surfaces with intentionally different jobs.
Console `/trade` reacquires a route and previews exact fill arithmetic; it is
read-only and asks neither for a wallet signature nor for submission. The
detailed Market pages (`/markets/<address>` and `/market`) carry the Direct
participant and execution flow when the market authenticates as open and has a
checked route.

On an eligible detailed Market page, the browser checks your Position and
collateral account, can admit your wallet where the checked first-admission
binding permits it, lets you select an outcome and a maker's signed offer, and
then prepares the exact transaction. It saves the unsigned packet before the
wallet opens, saves the signed packet before its single send, and checks the
finalized poststate before reporting completion. A market that fails any of the
market, route, phase, or prestate checks does not expose a trade.

That path is a way to take an existing signed offer. It is not a promise of an
order book, a buyer, a price, or an executable route for every market. The
[reader guide](reader.md) explains what the market is backing; this guide
explains what the Direct terms mean once one is available.

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

A browser wallet admission is available only for a market whose public Market
page carries a checked first-admission binding. Connect the wallet that will
own the Position, open that Market page, and choose the admission action. The
browser asks the Rust planner to reauthenticate the finalized Market and
linked-basis record, saves the exact unsigned request before the wallet opens,
submits the signed bytes once, and reports success only after the signature and
the Position's finalized poststate agree.

If a Market has no checked first-admission binding, the public entrance stays
closed. A market address, an aquarium observation, or an old cohort report is
not a substitute: wait for the market's checked founding report to be bound and
published. This is a launch gate, not a wallet error to retry.

The CLI remains useful for an operator's or local run's checked plan and
campaign evidence. Its key file is the Position identity, but an operator
artifact never authorizes a stranger's public wallet transaction. Against
devnet, confirm the cluster identity; an owned validator uses its own
credential-free loopback endpoint. The client does not guess which chain you
meant.

## Who gets the trading fee

Every Direct market has one venue rate, set when it is created, immutable
after, and charged **per side**: a rate of 50 basis points takes 50 from the
seller and 50 from the buyer, so a fill at that rate moves 1% of the gross.
The seller nets the gross less their side; the buyer is debited the gross plus
theirs. Rounding goes toward the makers, never toward the venue.

**The protocol takes none of it.** There is no protocol treasury, no protocol
beneficiary, and no instruction anywhere that lets the protocol sweep a market's
fees. The whole fee goes to the market's own `fee_recipient` — a pubkey the
founder fixed at creation — and it gets there by an ordinary token transfer that
anybody may submit. That transfer is a second transaction: permissionless,
unsigned by the venue, unrewarded, and with no deadline.

The rate can be anything from zero up to **500 basis points a side**, and no
higher: the protocol refuses a market founded above that, and the ceiling lives
in the deployed program rather than in a setting. Inside that band the rate is
the founder's choice and it is shown to you before you trade — on the market
page, on the ticket you sign, and copied into the signed terms where the trade
form cannot change it. A market's rate is a fact about that market, disclosed;
it is not a number the protocol collects.

## How the market resolves

Every market pins its source when it is created — a specific price feed,
down to the exact program deployment it trusts — and names a time window
with real width. The first valid observation from that source inside the
window settles the market; every later one is rejected. No committee, no
vote, nobody to appeal to — and nobody to be surprised by.

If the source publishes nothing through the whole window, the market can take
the disclosed recovery path it selected and funded before opening. If that
path is exhausted, the permissionless failure walk commits the published
failure outcome. You know before you trade what silence produces; neither the
founder nor a later committee gets to invent a different answer.

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
