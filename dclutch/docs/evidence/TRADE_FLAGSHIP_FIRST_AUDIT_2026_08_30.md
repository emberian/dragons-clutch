# The flagship's first on-chain audit

2026-08-30, TRADE lane. The load simulator's conservation-reconciliation
engine (`dclutch-local-successor-bootstrap ledger-census`, the same subcommand
`tools/load-simulator/simulator.py` runs once per cycle) was run against the
live public-devnet flagship
`7Mcu1ZT9KZBnvLZ2vhSvLeQMRA1ejQWD93yyPF2k8WAC` at finalized slot 490437294.

This is the simulator's observable half running on live devnet today. The
trade-per-cycle half is gated behind the Direct capability-activation wall
(`docs/evidence/TRADE_DIRECT_ACTIVATION_WALL_2026_08_29.md`); the census half
needs no mutation and no activated root.

## Result: every applicable conservation law holds

- **L1 (token supply) HOLDS.** Tracked 1,000,000,000 atoms across four token
  accounts == Mint `6odqARs4…` supply: hoard `6aDbBXDY…` 500,000,000 +
  source-funder `3V9qcom…` 499,998,600 + participant-1 collateral `HkXcoXKw…`
  700 + participant-2 collateral `DN9dWWsh…` 700.
- **L3 (position ↔ aggregate) HOLDS.** The founder Position `FqT63Tkx…` carries
  the claim vector [500000000, 500000000, 500000000, 500000000], which sums
  exactly to the Claims aggregate `669xTVjB…` supply vector. This is the
  substantive market-integrity law and it is exact.
- **L4 (hoard collateralization) HOLDS.** Hoard 500,000,000 ≥ worst-outcome
  liability 500,000,000 claims × unit 1 = 500,000,000.
- **L2, L5, L6, L7 inapplicable.** This is the first census: there is no
  predecessor boundary to move deltas or fees from.

Census artifact: the finalized JSON is written beside this lane's work at
`census/flagship-baseline.json` in the orchestrator scratchpad.

## Two census-input facts, learned by running it

- **Claim unit is 1 atom per claim, not 10^6.** The hoard holds exactly one
  collateral atom per outstanding claim; with a per-claim unit of 1, the
  worst-outcome liability (500M claims) equals the hoard exactly. A unit of
  10^6 (the collateral's display decimals) makes L4 read a spurious
  under-collateralization.
- **A complete token census names the founding accounts, not only the
  participants.** The mint's supply lives mostly in the founding hoard and the
  source-funder wallet; a census that names only the two participant collateral
  accounts under-tracks the supply and trips L1. The corrected token set is
  recorded in the simulator's devnet census configuration.

## Standing devnet facts (re-verified this morning)

Market `7Mcu1ZT9…`: Core-owned, 360-byte state, Phase::Open. Founder Position:
four outcomes at 500,000,000 claims each. Participant-1/2 collateral: 700 atoms
each, finalized. All intact from the founding and admission of 2026-08-29.
