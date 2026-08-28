# Trenchers

You've been rugged before. Dev pulled the LP, oracle "paused", the exchange
socialized your win, the perp venue ADL'd you at the top. Every one of those
is the same failure: your money was somewhere a stranger could touch it.

dClutch is a claims protocol built so there is no such place. This page
tells you what you'd actually be holding, why the payout can't be walked
back, and how there's a standing bounty you can collect just for paying
attention.

First, the disclaimer you actually care about: **seven protocol programs
are deployed on Solana devnet, but there is no open market, no token, and
nothing to buy today.** The complete market and trading rehearsals still run
on local test validators while the first public test market is prepared.
You're early — this page is so you know what it is before that market opens.

## What a claim is

A market here is a bet on a stated question with the answer split into
outcomes — say SOL/USD at the deadline: *under $120*, *$120–$180*, *over
$180*, and one more outcome for "resolution failed" (more on that one
below, it's the good part).

When the market is founded, **the full payout for every outcome is already
in the vault.** That's the whole trick. You are not holding a token someone
can print more of. You are not counting on a counterparty to be solvent
later. The collateral that pays the winning side is deposited before the
first trade exists, and the payout per winning claim is fixed arithmetic
written into the market — not a number an admin types in afterward.

So the worst case is known when you enter, not discovered when you exit.
There's no team allocation. There's no LP to pull. There is nothing behind
the curtain because there is no curtain — every account the market is made
of sits on chain where your own tools can read it.

## Range protection beats getting rugged

Longing spot means unlimited ways to be wrong: wicks, funding, exit
liquidity games. Buying a range means one way to be wrong, priced up front.

You think SOL holds $120–$180 through Friday? Buy that range. Land inside
it, you redeem at the fixed rate. Land outside it, you lose what you paid
and nothing else — no liquidation cascade, no margin call at 4am, no
counterparty deciding your win was too big.

And your fill can't be sandwiched into something you didn't sign. A trade
here is two **signed intents** — yours and the other side's — with your
limit price and expiry inside the signature. The chain checks the execution
against both signatures and refuses anything that doesn't match. Price
outside your limit? The transaction doesn't execute badly; it doesn't
execute.

```sh
dclutch markets ls
dclutch markets show <market>
dclutch intent buy --route route.json --outcome 1 --fill 5 \
    --price 400000 --collateral <acct> --keypair me.json --out my-bid.json
dclutch buy --route route.json --take their-ask.json --fill 5 --price 400000 \
    --collateral <acct> --keypair me.json
dclutch portfolio
```

When the chain says no, you get told who said no and why — the actual
program and the actual reason, not a hex number and a shrug:

```sh
$ dclutch refusal 0x5000
  claims refused: ClaimsSbfError::Instruction (0x5000) — Instruction bytes
  were hostile or selected no supported family.
```

## The failure walk: get paid for watching

Here's the outcome nobody else's protocol has: the oracle ghosting is **a
priced outcome with a bounty on it.**

Every market has a resolution deadline. If the deadline passes and no
resolution landed — relayer died, oracle stopped, team got bored — the
market doesn't limp along as stuck TVL. Anyone, meaning you, can send one
transaction that flips the market to its explicit failure outcome. Everyone
redeems their collateral back out. And **you get paid the bounty for
sending it** — escrowed by the market at founding, so it's already there,
not a promise.

```sh
dclutch walk --book walk-book.json --generation 1 --terminal-sequence 1 \
    --keypair anyone.json
```

Any wallet can be the walker. You pay one transaction fee; the market pays
you the quoted bounty (the current demo market escrows 250,000 lamports).
Too early? The program refuses and tells you so, and you're out one fee.
It's a race worth scripting: watch deadlines, be first, collect. Free money
for insomniacs, and it's not a bug — it's the mechanism that makes "the
oracle ghosted" cost the market instead of costing you.

## The honest part

- Seven programs are live on devnet. No open market and no token exist
  today.
- A winning-position payout has not completed end to end from a user's
  wallet on devnet yet. [The CLI](../../packages/dclutch-cli/README.md)
  tells you exactly which checks and submission steps it can perform.
- Where something isn't finished, the tools say so to your face instead of
  spinning. That's the house style: the chain refuses loudly, and no
  partial state survives a refused transaction.

When the first market opens, the promise will be the same one this page just
made: the money is where you can see it, the math is fixed before you enter,
and even the disaster case pays somebody — might as well be you.

The numbers behind everything here — payouts, fees, every refusal code —
are in the [reference](../reference/README.md), and the
[trader guide](trader.md) walks the same ground with the slang off.
