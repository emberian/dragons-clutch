# The Upkeep Vault — design sketch V0

Status: SKETCH for adversarial review. Nothing here is chartered. Born
2026-08-31 from the opener-shortfall discussion (WAVE b0e81f7c §3):
three separate rulings landed on "nowhere / zero / TBD" for the same
reason — the protocol has no honest sink — and the estates question
(who funds mainnet upkeep) has no answer that isn't a person's wallet.

## 1. What it is

One protocol-owned lamport vault (a PDA with no authority) that gives
homeless value a principled home and upkeep work a permissionless
financier. It is not a treasury: nothing is taken, nobody decides,
nothing accrues to anyone except through published prices for
verifiable work.

## 2. The three invariants (constitutional; each needs a red-proof)

**I1 — No involuntary inflow.** The vault is never funded from trades,
fees, or holder entitlements. Admissible inflows are exactly: residues
a ruling would otherwise send nowhere; donation slices explicitly
ruled; voluntary deposits. "The protocol takes nothing" remains true
by construction: nothing that was someone's becomes the vault's.

**I2 — No discretionary outflow.** Lamports leave only through
permissionless work routes with fixed, published prices. There is no
spend instruction, no authority, no vote. The vault cannot be spent;
it can only be earned from. A route pays only on a receipted, real
state change (the crank discipline: no vacuous work, idempotent per
state transition).

**I3 — Full legibility.** Every inflow and outflow is a receipted act;
the balance is derivable from the ledger. The vault must be
conservation-table-able exactly like a market's life.

## 3. Inflow inventory (from today's open rulings)

| source | today's ruling | under the vault |
|---|---|---|
| Compaction dust residues | "goes NOWHERE" | → vault (I1: would otherwise strand) |
| CloseMakerReplay donation slice | provisionally 0, pending ember | → vault (a principled recipient now exists) |
| Escrow-close residue after opener_outlay serviced | unruled | → vault (composes with the receivable completion) |
| Voluntary deposits | n/a | open to anyone, receipted |

Explicitly NOT inflows: trade fees (fee_recipient's, ADR 0014 D1),
rent principals (rent_owner's), recorded receivables (fee_owed,
opener_outlay — obligations to named parties are serviced first,
always).

## 4. Outflow inventory (candidate work routes)

- Crank rewards where swept rent cannot cover them (today: the crank
  simply doesn't happen — safe but untidy).
- Ceremony/upkeep reimbursement at cohort cuts (lineage declarations,
  evidence refresh, profile succession) — the estates answer: upkeep
  becomes something anyone is paid to do from a pool nobody controls.
- Oracle observation posting for resolution, if ever needed.
- Future: ZeroBump-class recovery bounties where the recovered rent is
  insufficient to motivate the work.

Each route: fixed price, published on chain, paid only on the
receipted completion of the named act.

## 5. Prior art and the failure modes it teaches

The outflow side is well-precedented: MakerDAO's liquidation tip+chip
(work-priced permissionless keeper pay), Gelato/Chainlink Automation/
Clockwork (keeper markets), EIP-4337 paymasters (prefunded pools,
priced permissionless execution), Solana closer-keeps dust bounties.
The inflow side is where the package diverges: precedented systems
fund keepers from protocol revenue (fees — refused here), the punished
party (liquidations — no such party exists here), or job-owner
prepayment (an operator float, not a commons). The nearest kin to I1
is EIP-1559's burn — residual value provably extracted by no one —
and the vault is the untaken third option: neither taken nor burned,
but housed. The no-discretion triple (I1+I2+no governance) appears
undeployed anywhere as a package; governed treasuries are the honeypot
story every time (Uniswap fee-switch, Nouns raids, KeeperDAO's
reflexive collapse).

Failure modes to design against, from precedent:
- **Serum crank decay**: underfunded upkeep silently stops. The
  empty-vault story must be honest: acts wait, degradation is safe,
  top-up is always open, and the site shows the balance.
- **Stale prices**: hardcoded bounties rot as rent reality moves.
  Prices should DERIVE from named on-chain quantities (rent
  minimums, published constants) — per the canonical-generation
  mandate, never hand-pinned.
- **Work-spam drains**: a route that pays on a repeatable no-op
  drains to a spammer. Pay only on real state transitions, receipted
  (the existing crank discipline).
- **MEV races**: priced bounties create competition; on Solana that
  is priority-fee competition and the work still happens. Acceptable.

## 6. What it deliberately refuses to be

- Not an insurance fund (full collateralization means no bad debt to
  backstop — the classic treasury reason does not exist here).
- Not protocol-owned liquidity, not a market participant.
- Not governed, upgradeable-in-policy, or spendable. A price change
  is a release-content change riding a cohort cut with its own
  review, never a parameter someone turns.
- Not a fee switch, and never fundable by one.

## 7. Naming

Not "treasury" (extraction connotation the protocol defines itself
against). Candidates: the upkeep vault, the commons, the WorkReward
pool. The copy must be able to say, truthfully: "nobody can spend
this; anybody can earn from it; nothing in it was taken from anyone."

## 8. Sequencing

Cohort-10 material, paired with the opener-receivable completion
(escrow close services opener_outlay first, residue → vault). Both
need the C9-REVIEW treatment before charter: the adversary should
attack I1 (can any involuntary flow be laundered in?), I2 (can any
route be made to pay twice for one state change?), and the empty-vault
degradation story.
