# DEPLOY-1 — the durable devnet deploy

**Date: 2026-08-27/28. Lane: DEPLOY-1, keyed by ember (keyDEPLOY-1).**

Charter: `WAVE.md`'s DEPLOY-1 queue over decision 0012's substrate — mutable,
slot-pinned, iterated by `Upgrade`, nothing recycled. Every number below is
measured, not predicted, unless it says otherwise.

**Status, by section.** The deployment record — the gate (§1), the substrate
(§2), the record layer and five-role activation (§3), the wallet arithmetic
through activation (§4), and the founding wiring (§5) — is **complete and
final**: every act in those sections is executed, byte-verified, and
detector-confirmed on Solana devnet, and their contents will not change. The
market foundings (§6), the life (§7-adjacent items), and the closing ledgers
carry their own status lines and grow as each act lands; a section that is
still in progress says so in its own words.

## 1. The gate

Checked-release candidate at **`555209aa`** (= `d2111cb4`'s programs plus two
DEPLOY-1 driver commits, tools-only): `sbf_build_diagnostics_total=0`,
`accepted=false`, **thirteen links at zero** (ten role/accelerator artifacts +
three frame-gate-only programs). Toolchain: rustc 1.89.0 (solana
platform-tools v1.53), solana-cli 4.0.2, cargo-build-sbf 4.0.0.

| role | ELF bytes | sha256 (prefix) |
|---|---:|---|
| registry | 207,072 | `e1f4a20f` |
| rent | 137,608 | `3b857b22` |
| custody | 360,328 | `d171cf74` |
| resolution | 588,336 | `03842494` |
| claims | 1,010,496 | `51967830` |
| trading | 1,325,848 | `7facb8e5` |
| core | 934,088 | `e0cc7109` |
| **seven roles** | **4,563,776** | |

## 2. The substrate — PERMANENT ADDRESSES

Deployed 2026-08-27 ~23:57Z, MUTABLE, upgrade authority
`4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` (the deployer, retained per
decision 0012), TPU transport, one role at a time, byte-verified **buffer-side
before paying and dump-side after**. Whole seven-role ladder: ~2.5 minutes of
wall time — SMOKE-0's TPU measurement, reproduced at scale.

| role | program id | ProgramData | deployment slot |
|---|---|---|---:|
| registry | `Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj` | `ENRSwrUEymWaXyrNtyD4QXXXk3tsTmcTGPTUFvnpsRVz` | 489,100,383 |
| rent | `DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3` | `78MW6W4iPzBVLceAwTL51CtyLcpcFM2iGVMDbzZtUFmy` | 489,100,242 |
| custody | `34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH` | `EhB7hHJ7vsCW3nCeqbxbJrn5Jsi6gbqwpVhoLMPZ8ENf` | 489,100,460 |
| resolution | `2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd` | `2QFBQJdLBXAnJWTVK8KeeUtWZEFhQqqN2CbkrWjMjY6f` | 489,100,560 |
| claims | `85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN` | `4La2511ddSxUcAQfdhKvEeGEasih3TStbQWVFEQKd34j` | 489,100,803 |
| trading | `5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk` | `AE1cWbCvXedE23XH3otSxvDQ7xVx7WLNMYDc8y8rqkrn` | 489,100,942 |
| core | `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N` | `AD6mb5SP6yqc5GFexf3xhpr1wKaZQhS7Hrt41iZhKxaN` | 489,100,672 |

Rehearsal-scoped nothing: **these are the durable protocol addresses.**
Program keypairs live with the job at
`~/jobs/dclutch-deploy1-20260827/keys/` (deploy secrets; the addresses above
are the public facts).

Transport: buffer writes rode TPU; RPC reads (blockhash/confirm/verify) rode a
keyed devnet endpoint whose key lives in `~/.helius-key` and appears in **no**
record, log, or evidence file (the driver's `redacted_url` discipline; the
ladder script redacts the same way).

## 3. The record layer and the five-role activation

Plan minted by `prepare` from the seven **real ProgramData observations** (the
account bodies read back off devnet), so every deployment slot the release
binds was decoded from the chain by the same hostile parse the on-chain
authenticator runs. All seven minted `ExactAuthority` under the deployer —
decision 0012's shape; `requires_revoke: false`.

- release set id: `e68f73651f97993110262bf5177029d7c31387b4cbcd67f4d96115db398a063b`
- activation cache: `Hz6BXyxyf66teABb6Pr6ev9jCZBJJpP5Q9p4sYJwJSkj`
- semantic release ids: resolution = SHA-256 of the protocol preimage
  `dclutch/release/source-resolution-controller-core-effects-source-closure-v4`
  (the bootstrap refuses any other); the six others =
  `SHA-256("dclutch/deploy-1/semantic-release/v1\nrole=<role>\ncommit=555209aa…")`.

Campaign (`--execute --through activation`, the DRIVER driver, paced,
sequential): **33 transactions** — 9 infrastructure record bodies finalized
through Registry `Begin → Append → Finalize` (27 tx), Core infrastructure
profile initialized at **214,285 CU**, and the five-role activation **under
the slot-pin admission on the mutable substrate**:

| role | activation CU |
|---|---:|
| Core | 517,975 |
| Claims | 539,072 |
| Trading | 697,109 |
| Resolution | 328,723 |
| Custody | 230,261 |

Every activation under half the 1,400,000 ceiling. Decision 0012's
activation-side claim is measured on the public cluster, not argued; the
Hot-path side is POST-0012-EXACTAUTH's +73-CU parity measurement (M-63/M-65),
which closed DEPLOY-1's inherited 20-seed debt before this lane reached it.

Detector re-read (a second, read-only invocation whose detectors read the
chain — no state file exists): substrate / publication / initialize /
activation all **complete**.

## 4. Wallet arithmetic (running)

| checkpoint | lamports | SOL |
|---|---:|---:|
| start (= SMOKE-0's final) | 64,990,412,120 | 64.990412120 |
| seven roles deployed | 33,187,407,520 | 33.187407520 |
| record layer + activation | 33,151,551,640 | 33.151551640 |
| INFRA-RELAY fee payer funded (0.05) | 33,101,546,640 | 33.101546640 |

Deploy delta 31,803,004,600 = ProgramData rent 31,772,309,520 (parked, never
burned; iterable by `Upgrade` at fee cost) + seven 36-byte Program residues
7,990,080 + ~22.7M lamports of ladder fees (~4,540 signatures). Record layer
delta 35,855,880 = nine record bodies + profile + activation cache rent +
165,000 fees. Peak exposure stayed under 32 SOL against the 40 SOL cap.

## 5. The founding wiring (the lane's engineering item)

DRIVER's named seam — "the market input lives in the SPEC, the driver takes a
PLAN" — closed at `1e5581f5`: the campaign takes `--market ABSOLUTE_JSON`, the
founding stage gained a chain-reading detector whose Complete is the
executor's own Open-market poststate core, Partial **refuses** with the
existing accounts named (a founding that fails midway has real principal
behind it; never resumed blind), and the record compilation is factored to one
author so the detector and the publisher cannot drift. Proven end-to-end on a
local harness cluster (seven mutable roles from this candidate's exact ELFs,
plan minted from real observations, campaign driven through founding) before
any devnet founding principal moved.

Producers: `devnet-market` (`555209aa` + follow-up) compiles the Pyth
range-protection input against the committed devnet Pyth row and a live
`PriceUpdateV2` read, refusing any window narrower than the measured
1,252-second cadence floor; `graduation-market` compiles the relayed
graduation input from ONE author shared with the relayed-vertical rehearsal,
over venue facts read off real mainnet.

## 6. The markets

### 6.1 The SOL/USD range-protection flagship

**Status: founding IN PROGRESS on devnet (the campaign is executing as this
revision is committed); the addresses below are the derived targets the
founding detector reported before execution and are final either way.**

The story: protection against SOL/USD leaving the 120.00–180.00 band at a
real terminal window, resolved by the real devnet Pyth receiver. Founded
under kappa as a founder-side discipline (the on-chain cap is the
RECORDS-MIGRATE row; SMOKE-0 §6.8's framing stands).

| fact | value |
|---|---|
| **Open Market** | `HEzCuDvrKP9ScVK8dZqULRzw78U3922yk5q7cu3riqK` |
| Found31 Market (generation 1) | `9oww4URBNczg83g6ZzRD8zvcFfVeKTimAutwKm6qAuZa` |
| abort-lane Market (generation 3, staged and unwound) | `5ZtTvhPLeP1HjLfHBWwnfZ2B5wrNj8AamzZoGof8uGHk` |
| collateral mint (Token-2022, 6 decimals) | `YyQtrWnqAFqx4D2MZdnS3eK8TgM1Ewabx9waExH99aD` |
| realm record | `2D4qAzXUyDK5qwVa5HxDvvRtPQR5iojXUAF4Am5dAAeu` |
| band | cuts 12,000 / 18,000 at denominator 100 (USD cents); coefficients 1,0,1,0 — either tail pays |
| terminal window | 2026-08-28 01:05:58Z → 01:35:58Z (1,800 s = ~5.75 measured cadences) |
| `max_age_seconds` | 86,400 — a deliberate submission-latency budget so the resolution tooling that follows this deploy can still submit an in-window publication; the window bounds WHAT resolves, the budget only bounds how long after publication a submission may land |
| provider | the committed devnet Pyth row (`devnet_release_v1`), SOL/USD `PriceUpdateV2` at `7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE` |

*(the founding transcript's facts — transaction counts, DCLTGMF1 CU, the
Open poststate — land here when the campaign completes)*

### 6.2 The abandoned market (relayed, relayer deliberately silent)

**Status: input compiled (real venue facts, failure-shaped window, 250,000
lamport walk bounty disclosed in the manifest quote); founding queued behind
the flagship.**

### 6.3 The mainnet-observer graduation market (relayed, operated daemon)

**Status: gated, deliberately.** The founding pins an `account_set_id` this
lane derived as `63918468…` over the real watched set; the operated daemon's
own `show-config` must independently print the same value before anything is
founded against it — two authors, one number. Founding proceeds on that
cross-check.

## 7. Frictions and findings

- The relayed-vertical build was red at HEAD: DRIVER's `cluster.rs` split made
  the successor's `rpc.rs`/`seed.rs` import `crate::cluster`, and the
  vertical's `#[path]` module list never gained it (the journey's did). One
  module include fixes it; found because this lane builds what it ships.
- The twin's "synthetic" DBC deployment slot (423,941,138) is the **real**
  mainnet value — tonight's read of `HUfnSSiJ…` confirms slot, and adds the
  real authority `JADaUV8kvDpDbJr55wxXJHVaBS3VCj8thZZHjfeuCVLd` and ELF-tail
  digest `c7613080…` (2,326,622-byte ProgramData). 1,586,969 real
  `VirtualPool`s enumerated (766,251 pre-graduation).

## 8. What SMOKE-1 still needs

**Status: collected during the run; closed at the lane's yield.**
