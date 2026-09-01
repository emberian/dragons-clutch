# Operator walkthroughs

A walkthrough is one complete act, start to finish: you begin with nothing,
you run real commands, and you end holding the thing the act was for. The
[guides](../guides/README.md) tell you what a market is and what you decide
when you make one. These tell you what to type.

## What a walkthrough promises you

**Every command was run.** Not composed from a help page — run, against this
tree, and the output below it is what came back. Where output is long it is
cut, and the cut is marked. Two substitutions are made and no others: an
absolute path that was particular to the machine it ran on appears as
`~/work/…` or `<PLACEHOLDER>`, in the command and in the output alike, and
digests are shown in full or truncated with an ellipsis. Nothing inside the
fences is edited to look better than it was, and no output is composed.

**Refusals appear where they happen.** dClutch refuses a great deal, on
purpose, and an operator meeting a refusal for the first time wants the way
out before the explanation. So a refusal you can actually hit en route is
printed at the step that produces it, with its remedy on the line after it
and its reason after that. The meanings come from
[the refusal reference](../reference/refusals.md), which is generated from
the code that raises them.

**Arcana gets one sentence of why.** Some steps ask for things no other
protocol asks for: an acknowledgement naming the cluster you think you are
on, a keypair passed by path rather than read from your Solana config, a
clean commit before an offline gate will look at your source. Each is there
because something is not safe to guess. The walkthrough says which thing, in
one sentence, at the step that asks.

**A wall is written down as a wall.** Where an act cannot be completed today,
the walkthrough runs up to the wall, shows the exact command that stops, and
says what would have to exist. It does not route you around the gap without
telling you a gap is there.

## The walkthroughs

- **[Found a market on your own validator](found-a-market.md)** — the local
  chain, the source contract gate, and how far founding gets from a cold
  machine today.
- **[Author a ticket, take a trade](author-a-ticket.md)** — the ticket
  author, the verifier, a board running on your own loopback, and the
  taking side.

Planned, not yet written:

- **Settle a fee** — the realized-fee path from a filled trade to the
  recipient, and the compartment that keeps it separate.
- **Run a publication cut** — the checked release, the three-layer verify,
  and what the site is allowed to say afterwards.
- **Close and retire** — ending a market's life: the close, the seal, and
  the accounts that come back.
- **Run the board** — operating a ticket board for other people: what you
  are custodian of, and what you are not.

## What these are not

None of this is mainnet. The commands here run against a local chain you
start and tear down, or against no chain at all. Seven programs are deployed
on Solana devnet and one market on it is open for trading, with a devnet
test token as collateral, so there is nothing to buy with money and no value
at risk. Nothing here is an audit, and nothing here is a release.
