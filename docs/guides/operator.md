# Operator guide

Running a dClutch market means making all the decisions up front, funding
them, and then getting out of the way. After your market opens you hold no
admin keys, you cannot change its rules, and you are not the referee:
everything that happens next is either open to everyone or refused for
everyone. This guide walks through the decisions that are yours.

Seven protocol programs are deployed at permanent addresses on Solana
devnet, but the first open market is not live yet. Today you rehearse market
creation against a local test chain and use the public app to inspect the
devnet deployment. Exact costs, routes, and codes live in the
[reference](../reference/README.md).

## What you fix at creation, forever

- **The collateral** — which token backs the market (its **Realm**: the
  collateral and admission namespace).
- **The question and its cells** — how the answer space is split into
  buckets.
- **The source** — the exact feed whose observation resolves the market,
  pinned down to the program deployment it trusts.
- **The window** — when the market can resolve, and how stale an
  observation may be when it lands.
- **The fallback outcome** — what happens if the source stays silent.
- **The program releases** — the exact on-chain code your market runs,
  named by content, selected from the Registry.

All of it is published on chain before the market opens, and none of it
can change afterwards. There is nothing to govern, so there is no
governance to capture — and no creator backdoor to worry about, because
none exists to defend.

## Opening the market

Creating a market — the protocol calls it **founding** — now has three
ordered stages:

1. **Project the market.** The Core program authenticates the complete
   Registry graph and authorizes the exact projection for this future market.
2. **Stage custody and controller funding.** The `DCLTPCB2` transaction opens
   the empty Hoard, creates the one-shot projected custody state, funds the
   named obligations, and records the exact controller-owned funding ledgers.
   It does not create claims or open the market.
3. **Found.** The `DCLTGMF3` transaction locks custody, creates the market,
   makes it real, sets up claims, and opens trading last in one all-or-nothing
   rollback domain. When that composed transaction does not fit under the
   compute limit, the same stages run as two transactions instead: `DCLTGFP1`
   commits the market and escrows a one-shot Core permit, and the
   permissionless `DCLTGMO1` consumes the permit to open the market last.
   Founding through the split is atomic economically rather than
   transactionally: the permit pins every coordinate of the open, so the
   market can only open on the terms the first stage committed, and the escrow
   carries a refund path so nothing strands between the two transactions.

This is the current source-tree route targeted for the next devnet program
update. It has passed the compiled-message lock census, but it has not opened a
market on devnet yet. Its predecessor exceeded 90% of Solana's per-transaction
compute limit; the replacement's compute cost must be remeasured before it is
installed. The current measurements and their evidence level are in
[the compact-founding record](../evidence/FOUND_COMPACT_2026_08_28.md) and
[the budgets reference](../reference/budgets.md).

## Funding named obligations

The money you put in has names. Custody tags every token account with
what it is for, and the tags never mix: collateral can never be spent as
fees, the fallback bounty can never be spent as rent, and so on. The
compartments (`CompartmentV1`, `crates/dclutch-custody-contract`):

| compartment | holds |
|---|---|
| `HoardPrincipal` | the market's collateral — pays claim holders, never anything else |
| `TradingPrincipal` | Direct/Dealer trading principal |
| `Settlement` | general settlement inventory |
| `FeeVault` | realized fees, kept physically separate |
| `LivenessVault` | the funding for the fallback (the walk bounty) |
| `RecoveryReserve` | recovery-reserve capital |
| `SeriesEscrow` | Series ticket principal before its market exists |
| `External` | accounts owned by depositors, recipients, beneficiaries |

When you fund a market, you are funding specific named obligations — the
fallback bounty, the resolution work, the rent — not topping up one
pooled balance. Native SOL and the market's collateral token are counted
separately and never converted into each other. An escrow one lamport
short of its named amount is refused.

## Choosing a resolution window

Your window is a time range `[start, end]`, and it needs real width: a
market that can only be answered in one exact second is answered
essentially never, and takes its fallback instead. Match the width to how
often your source actually publishes.

For Pyth's devnet SOL/USD feed (measured: a new price roughly every 313
seconds), the chance the window contains at least one publication is
about `1 − exp(−W/313)`:

| width `W` | shape | chance of ≥ 1 publication |
| --- | --- | ---: |
| 1 s | a single instant | ~0.3% |
| 300 s | one publication interval | ~62% |
| 600 s | two intervals | ~85% |
| 1,250 s | four intervals | ~98% |
| 1,800 s | 30 minutes | ~99.7% |

Practical rule: **make the window at least four publication intervals
(about 21 minutes), and 30 minutes for a market that should not fall back
just because the feed was slow.** The derivation is in
[`docs/design/MAINNET_STATE_RELAY.md`](../design/MAINNET_STATE_RELAY.md)
§12.3.

`max_age_seconds` is a separate knob: it caps how old an observation may
be when it lands on chain, and it sets the market's final deadline at
`end + max_age`. A wide window doesn't help if nobody can land the
observation within `max_age` of its publication.

Two guarantees you get for free: the first valid observation settles the
market and every later one is rejected; and there is no dead gap — the
last moment an observation can resolve the market and the first moment
the fallback can take it are the same moment, `end + max_age`.

## The fallback

If the source stays silent through the window and its grace period, your
market takes the fallback outcome you disclosed — the protocol calls this
the **failure walk** — and anyone may trigger it and collect the bounty
you funded. A walk before the deadline is refused, a second walk cannot
collect twice, and an underfunded bounty is refused down to the lamport.

The walk isn't a defect; it's the planned answer to a source that never
showed up. Your job is to make it rare (a wide-enough window, a
reasonable `max_age`, a source that really publishes) and survivable (a
fallback outcome you'd be willing to live with, funded for real). It
should happen because nothing published — never because your market asked
a question nothing could answer.
