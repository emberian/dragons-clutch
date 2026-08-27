# Trader guide

What you are holding when you hold a dClutch claim, what it can and cannot
do to you, and what the protocol's words mean when it talks to you.

Nothing here is deployed or tradeable today; this describes the protocol as
implemented and executed locally. See the [reference](../reference/README.md)
for the exact numbers this guide links to.

## What a claim is

A market names a bounded objective question — say, where SOL/USD publishes at
a stated time — and partitions the answer space into an **exhaustive,
disjoint, ordered, canonical** set of cells. That partition is fixed at
founding and is the market's whole ontology: every claim is a claim on one
cell.

A claim on a cell pays exactly one collateral unit if the resolved answer
lands in that cell, and zero otherwise. One claim on *every* cell — a
**complete set** — therefore pays exactly one collateral unit no matter what
happens, and the protocol treats it that way: complete sets are minted
against collateral and redeemed for collateral at par, exactly.

**Fully collateralized** means the collateral is already there. Claims exist
only because someone deposited a collateral unit into the market's **Hoard**
and took the complete set for it. The Hoard's principal pays claimants and
does nothing else — by standing invariant it is never fees, rent, bounty,
insurance, work funding, reserve, or treasury capital. There is no leverage
anywhere in the design, so there is no liquidation, no margin call, no
funding rate, and no path where the market owes more than it holds. What you
can lose is what you paid for the claims you hold, at most, ever.

Because a complete set is always worth exactly one unit, cell prices are
prices on a simplex: they are exact scaled integers that sum to one unit,
with one named rounding boundary. When this project says a clearing result is
the "**best valid submitted candidate**," that is the whole claim — it is
deliberately not called "optimal" without a checked optimality certificate.

## How range protection works

Range and tail products are not a separate machine. A digital ("below X"),
a range ("between X and Y"), or a tail ("above Z") is a *set of cells*, and
buying protection is buying one claim on each cell in that set. The payoff
compiles exactly onto the categorical basis: the cells are disjoint, so if
the resolved answer lands in your range, precisely one of your cells wins,
and it pays one unit per claim on it. The price of a range is the sum of its
cell prices, exactly, because the cell prices live on the simplex.

So "protection against SOL falling below X" is: buy claims on every cell
below X. If it does, the resolved cell is one of yours and pays par. If it
does not, your claims expire worthless and the collateral you paid stays
with whoever sold you the range. No oracle-triggered liquidation sits in the
middle; the only event that matters is resolution.

## What resolution is

Resolution consumes an authenticated observation from the market's
**release-bound source** — a source identity pinned at founding, down to the
program deployment it trusts — inside a terminal window that has real width.
The **first admissible observation terminalizes the market**; every later
one refuses without being inspected. That single-answer property is
machine-checked, not implied (see the resolution material in
[`docs/design/MAINNET_STATE_RELAY.md`](../design/MAINNET_STATE_RELAY.md) §12.3).

If the source stays silent through the window and its grace, the market
walks to its **pre-disclosed failure outcome** — funded in advance, taken
permissionlessly. You know before you trade exactly what silence produces.
There is no discretionary resolver to appeal to, which also means there is
no discretionary resolver to be surprised by.

## What a refusal means

dClutch fails closed. A transaction that does not authenticate exactly —
wrong account shape, wrong authority, stale state, a window that has not
opened, a replay — is **refused**: the whole transaction rolls back, no
partial effect survives, and your collateral is exactly where it was. A
refusal costs you a transaction fee and nothing else.

Every refusal carries a code that names the program that refused and why.
`band = code >> 12`: `0x5…` is Claims, `0x3…` is Core, and a code below
`0x1000` is not dClutch's at all — it came from some other program in the
transaction. The complete tables, with meanings, are in
[the refusal reference](../reference/refusals.md).

Read refusals as the protocol working. The alternative to a refused
transaction is not a successful one; it is a market whose invariants bent.

## The vocabulary, kept honest

- **Fixtures, ProgramTest, local validator, devnet, mainnet** are distinct
  evidence levels, and none of them promotes to the next by analogy.
  Everything in this guide is at the local levels today.
- **"Formally verified"** is never said here without naming the theorem, the
  digest, and the unverified boundary. Today's machine-checked results cover
  named models and per-case corpora — the cases, and nothing else.
- **Route coverage** is stated per route, in
  [the routes reference](../reference/routes.md): executed under a named
  campaign, blocked with a stated reason, or never-executed. The uncomfortable
  rows are printed, not hidden.
