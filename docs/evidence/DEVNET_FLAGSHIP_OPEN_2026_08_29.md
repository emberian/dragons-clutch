# Devnet flagship Market open — 2026-08-29

Status: IN FLIGHT (this file finalizes with the campaign-open facts)
Cluster: Solana devnet only
Substrate: the live first Upgrade cohort (source `d3cf6bbf`, gate `9713d52e…`),
seven permanent program IDs unchanged.

## What this cohort adds on top of the first Upgrade cohort

The first cohort upgraded five ProgramData payloads and stopped. The chain had
no publication layer: 3 of 9 Registry records finalized, no ExecutionReleaseSet
activation, and therefore nothing foundable. This session:

1. completed the administration campaign on the live cohort — publication of
   the seven ArtifactRelease records and the singleton infrastructure profile,
   Core profile initialize, and the five-role ExecutionReleaseSet activation —
   21 finalized transactions signed by the retained authority;
2. funded the founding actors and four disposable participants;
3. compiled the sponsored SOL/USD flagship MarketRunInput against the live
   plan (50 bp per side, Direct fee recipient = the devnet development wallet,
   four outcomes, cuts 12000/18000 usd-cents);
4. executed `campaign --founding-only` to found the Market.

## Client/chain frame coherence (the finding that shaped the session)

The live cohort is source `d3cf6bbf`. Two later commits changed on-chain
frames: `5ca145e8` inserted a separate DCLTCFQ1 funding-source account at
index 11 (shifting the found window), and `da5460b3` moved provider invocation
under the Core caller PDA. Any HEAD-era client therefore speaks frames the
live programs refuse. The founding/admission/Direct client for this open is
built from `58bb7684` (= `5ca145e8^`): its Trading/Claims/Core/Custody sources
are byte-identical to `d3cf6bbf` and it reads the v3 deployment-set journal.
Post-d3cf program commits stay quarantined for the next Upgrade cohort.

Also learned and used: the Upgrade receipts bind the redacted RPC origin, so
`prepare` and the market compiler run against the public origin the receipts
recorded; execution campaigns run on a keyed endpoint with fresh evidence.

## Identities

| role | address |
|---|---|
| deployer / retained authority / Direct fee recipient | `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` |
| campaign payer | `GZQoAjVBaNh7KcGDSjjMaFBcTaJPbYxhkDYHudYb88ic` |
| collateral mint (attempt 3) | `EUztpHQNUyi7X19yYa5BJEpNmCiVwS2L8CFVDyLSaGZc` |
| collateral raw-atom wallet | `J16VAi5orcpTYJNYcFGYRBypJ5KjS21bY7c2Rii8JeBU` |
| founding beneficiary | `4JbuXbcAnVi95itMiZFu6sAhb7AgvYsv34hwpuweVhFQ` |
| founding projection witness | `4sDPhUBKLCbFxBc45XWBuRwcNG8tw78tct9MgxeVZFTY` |
| founding source funder | `DXCBPpfxhJfLrXEuhUwNt8TgM3atocHomrKLUa17rzvp` |
| founding founder (identity) | `6LkyGdwJcCWaGRZPc9DKYqtabvABgXuyPLTHeGJvRdoS` |
| substituted founder (identity) | `9s9ZmJxmc6G2GVZZfU5iqVbQLgJgBppqfcTeX5X8F5ox` |
| participant 1 | `5oGySWQAKZ3fLmAwUbG6WifP7dCF6FRtriawtgxoCZXf` |
| participant 2 | `GcE6LWbduoATDgK8jsGyj2i8ywV37fcAYABKCmKgttDz` |
| participant 3 | `E9buaTm2SAovWXsaRBMPyfk5uhdryDVA744CfAogpoRR` |
| participant 4 | `E71d4qisbiQbt8UGb2PPmmJanqUeUxyPcjLjXX14ooZX` |

Market address, founding signatures/slots: TBD from `campaign-open.json`.

## Wallet arithmetic (devnet development wallet, ember's)

All lamports, finalized reads:

- at first-cohort stop: `42,677,356,226`
- administration campaign (publication rent + initialize + activation + fees):
  `−26,781,000`
- campaign payer funding: `−2,000,000,000` (+ `5,000` fee)
- founding source funder funding: `−2,000,000,000` (+ `5,000` fee)
- observed after the above: `38,650,565,226` (exact)
- four participant fundings at the activity envelope (20,000,000 each):
  `−80,000,000` (+ `20,000` fees)

Founding attempts 1–2 spent from the *campaign payer* (records/mint/wallet
rents for two abandoned prefixes, ~`216,000,000` lamports total); the orphaned
mint/wallet/realm accounts of those prefixes are named debt, not hidden:
attempt 1 mint `57i7c6zwEEzySrt7a94FAbY6AWnEdK4jDZEYkTej4PrP`, attempt 2 mint
`7w9HaLRRqQ2Qnrpg6BkT38uuRVDnxFPDW8mcFsXCBzwT`. Their rent is recoverable only
by a future cleanup route; it is written off here.

Final balances: TBD at close.

## Safety rails held

- devnet only; the genesis hash is asserted by every driver invocation;
- no keyed RPC URL reaches a commit, an evidence JSON (origins are redacted by
  the drivers), or the frontend;
- mainnet untouched.
