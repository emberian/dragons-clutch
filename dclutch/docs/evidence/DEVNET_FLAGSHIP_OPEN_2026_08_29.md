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
| collateral mint (attempt 6, the live one) | `GtmpRvSL9y6RpqMth73VSdb9h1XRe7zqQZkhJkfgxKrA` |
| collateral raw-atom wallet | `EybzHgWfAbc7yW1HkgmDcrUPhWtYywEvTP2EaNBpE4LX` |
| founding beneficiary | `5QnMv6S3uiWGWgiVyiDs9Ai3QN4cqS2i7EsJwnZ3FJej` |
| founding projection witness | `9vdQnsz5LyhLnjru3ycVgqGVWKYGPk5L4VFMbpg1vcGn` |
| founding source funder (fresh per attempt; the ladder CREATES it as the Token-2022 principal account) | `8p2yvHUEwyRdtgWjBsmfizV1XNPYxMF51PTjQVScyHkJ` |
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

## Cohort-2: the Upgrade that makes devnet foundable

The definitive finding of this session: the live first cohort (source
`d3cf6bbf`) could never found a Market — its own Trading forwards the DCLTCFQ1
funding source as `found[0]` (a signer) into the Resolution CPI, while its own
Resolution refuses any signer in the found window. `5ca145e8` (landed 78
minutes after that cohort was cut) is the fix. Every founding attempt in
recorded history refused at exactly this CPI (`0x8000 AccountFrame`, ~8.2k CU
into Resolution), reproduced here with three progressively-coherent clients
before reading both programs settled it.

Cohort-2 source: `5e78c3ed` (= `e4aa2bbd^`) — carries `5ca145e8` (CFQ1),
`da5460b3` (provider under Core caller PDA), `4953bada` (terminal settlement
on a resolved Market — the dead-terminal fix), `f30cf078` (PCB2 completion
set), and predates the `emit_series_consume_artifacts_v4` frame regression, so
the all-13 checked gate emits with **zero** SBF frame diagnostics:
gate `4f5d5d8b6b9115ff2b5a9826f48de517bbafe95a6442c41ffb1614c369fe1a19`,
built on hbox at `/tank/dregg-build/dclutch-cohort2-gate`.

Upgrade mechanics (permanent IDs retained, retained authority signing):
hand-authored v3 deployment-set journal (auditor-green), per-role key-free
baseline, CLI `program extend` where capacity demanded it (the loader enforces
a 10,240-byte minimum extension), CLI `write-buffer` on a keyed RPC origin,
driver arm with `--adopt-existing-buffer`, CLI `program deploy --buffer`,
driver attach via `--adopt-finalized-cli-upgrade-signature` completing the
digest-bound receipt. Extensions: resolution +10,240 B, trading +10,240 B,
core +57,240 B.

| role | upgrade signature |
|---|---|
| custody | `3K6ik9Ah7xzBtYgvm6ZuaNs7C3GCNnPiwP5XX1b9gDG1EyjbU9AEN7ei8kYk4umPt3dXCXqiFwLEecBjunFVKtwF` |
| resolution | `D1BVSBR79UscDbvpUYSmsnoPpbYiYUymUYhaMUc5rv4bfEDwZCb9ZXQza4X3Hr5Yrt1Hb81W8nBF7tmtbYXpmFM` |
| claims | `3hGhX2VeDQPTdk6tHyJhBPie2xHrni7tnTRUba6vYmyCbD3eKBhGPuMEvJqHCNW2SxaiWGWbVkDGaHnR29GerYSf` |
| trading | TBD |
| core | TBD |
