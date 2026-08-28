# DEPLOY-1 — the durable devnet deploy

**Date: 2026-08-27/28. Lane: DEPLOY-1, keyed by ember (keyDEPLOY-1).**

**Current-state note (2026-08-28).** This is the historical record of the
first durable deployment and its `DCLTGMF1`/`DCLTPCB1` founding attempts. Those
attempts and addresses remain evidence, but decision 0013 supersedes that
founding route for the next generation. The replacement source-tree routes are
`DCLTPCB2` and `DCLTGMF2`; their compiled messages measure 62 and 59 complete
keys respectively. They have not opened a market on devnet, and the seven
permanent program addresses below still ran the generation recorded here at
the pre-write reads in [DEVNET_ITERATION_2](DEVNET_ITERATION_2.md) §1. A later
generation is not installed until a separately evidenced `Upgrade` completes.

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

**Status: STOPPED AT A NAMED WALL — the market is NOT founded on devnet.**
Ten attempts; each earlier one found and fixed a real driver defect (§7), the
tenth reached the atomic founding itself and received the chain's own answer:
`TooManyAccountLocks`. The DCLTGMF1 atomic founding — Lock, Found, Realize,
Claims, Open; five stages, one transaction, which IS the design — locks more
than 64 unique accounts; devnet's `increase_tx_account_lock_limit` feature
account is **absent** (verified on chain), so devnet's limit is 64, while the
local validator activates the feature (128), which is why the identical
transaction executed there at 1,199,741 CU. A protocol-frame versus
cluster-feature wall: no driver plumbing can clear it, and per the charter a
stopped deploy with a named wall is a success. The derived targets below are
what this input's founding lands at whenever the wall lifts; everything
through DCLTPCB1 executed on devnet with real signatures.

The story: protection against SOL/USD leaving the 120.00–180.00 band at a
real terminal window, resolved by the real devnet Pyth receiver. Founded
under kappa as a founder-side discipline (the on-chain cap is the
RECORDS-MIGRATE row; SMOKE-0 §6.8's framing stands).

| fact | derived target (founding stopped at the named wall) |
|---|---|
| **Open Market** (not yet live) | `Csa2B4ZKRZYD889njds8GyN6Lmdt4SUZHJUdLieXoG1u` |
| Found31 Market (generation 1) | `HYjpq3dgqVvbEVqJJHyyAkpihYpzw7HuyZQvFBM21Eso` |
| abort-lane Market (generation 3, staged and unwound) | `4R8RirZavRF6Dsp9xRUL3iLiSbwtgZ2w3wLTbbr6o5uA` |
| collateral mint (Token-2022, 6 decimals) | `5xZQoY3dHzuxWHyXudgx73Jevi7BR8WHo9u7iQxwCgM8` |
| realm record | `6Lvs7x2hQCvVsKvWkrsYTRkZzUWUhiN8cdt6u8wUt8fP` |
| band | cuts 12,000 / 18,000 at denominator 100 (USD cents); coefficients 1,0,1,0 — either tail pays |
| terminal window | 2026-08-28 01:05:58Z → 01:35:58Z (1,800 s = ~5.75 measured cadences) |
| `max_age_seconds` | 86,400 — a deliberate submission-latency budget so the resolution tooling that follows this deploy can still submit an in-window publication; the window bounds WHAT resolves, the budget only bounds how long after publication a submission may land |
| provider | the committed devnet Pyth row (`devnet_release_v1`), SOL/USD `PriceUpdateV2` at `7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE` |

What DID execute on devnet for this lineage, with real finalized signatures
(attempt 10, priority-fee send path): the realm record + collateral, the
Market-scoped RentV2 credit, the Found31 routing table, **Found31 with both
hostile probes refused-and-rolled-back and the Market created (223,385 CU)**,
and **DCLTPCB1 — the four-stage projected-Custody founding prestate — with
both of its hostile probes refused (719,995 CU)**. The substituted-Claims
hostile probe was undeliverable (it shares the over-limit frame) and is
recorded as an evidence gap; its refusal is executed evidence on the local
proof chain.

### 6.2 The abandoned market (relayed, relayer deliberately silent)

**Status: NOT FOUNDED — input compiled and ready (real venue facts,
failure-shaped window, the 250,000-lamport walk bounty disclosed in the
manifest quote), but its founding shares the flagship's DCLTGMF1 frame and
therefore the same `TooManyAccountLocks` wall. One command once the wall
lifts.**

### 6.3 The mainnet-observer graduation market (relayed, operated daemon)

**Status: NOT FOUNDED — doubly gated.** The founding pins an
`account_set_id` this lane derived as `63918468…` over the real watched set;
the operated daemon's own `show-config` must independently print the same
value before anything is founded against it (two authors, one number) — and
the founding itself shares the flagship's `TooManyAccountLocks` wall. The
operated relayer is live and disarmed on the box, its fee payer funded, its
attestation key pinned by the compiled input.

## 7. Frictions and findings

- **THE WALL, named by the chain itself: `TooManyAccountLocks`.** The
  DCLTGMF1 atomic founding locks more than 64 unique accounts;
  `increase_tx_account_lock_limit`'s feature account is absent on devnet
  (limit 64) and active on the local validator (128), which is why the
  identical transaction executed locally and can never execute on today's
  devnet. Ten founding attempts hid this behind one property: every hostile
  probe is sent `skipPreflight` (an expected-failure transaction must not be
  vetoed by simulation), so the over-limit transactions produced no error —
  leaders silently declined to schedule them, which presented as eternal
  transaction drops and consumed four real diagnosis cycles (the probe
  addresses, the read floor, the resubmit, the priority fee — each a genuine
  fix for a genuine defect, none of them this). The tenth attempt's
  evidence-gap machinery let the REAL founding send with preflight ON, and
  the simulator named the wall in one second. Lesson for every campaign
  after this one: **send one preflighted probe of a new frame shape before
  spending a ladder on it.**
- **The founding detector consumed the forge key it was looking at.**
  `KeyForge::keypair` issues a fresh index per call; the detector's
  pre-execution derivation drew `collateral-mint[0]`, the executor founded on
  `[1]`, and the post-execution verifier peeked `[2]` — so the local proof
  opened its Market at 1,199,823-class CU (measured 1,199,741) and then
  reported it absent. Fixed: `KeyForge::peek_pubkey` (non-consuming, refused
  on a random forge whose future key does not exist until drawn); the
  campaign peeks once and threads one value through detector, report, and
  verify. Found by the first driven founding; invisible to every unit test
  that had ever passed.
- **`Pubkey::new_unique()` is a test counter, and its addresses exist on
  devnet.** Three consecutive founding attempts died at the Found31 rollback
  check. The first diagnosis — a load-balanced endpoint's replica lag
  breaking read-your-writes — was WRONG for this bug (it produced the
  read-floor hardening below, which stays as correct discipline), and the
  compound `if a || b || c || d` refusal with one sentence hid the truth for
  two paid cycles. The instrumented refusal finally named it:
  `recipient_exists=true` — the "fresh" rollback recipient and substituted
  market key come from `Pubkey::new_unique()`, a **deterministic global
  counter meant for unit tests**, so every process draws the same
  low-counter addresses, and on a public cluster with years of history those
  addresses EXIST. A fresh local ledger — where every such address is empty
  by construction — can never catch this. The chain's own arithmetic was
  exact on every attempt (payer −5,000, nothing else moved). Fixed: all
  seven live probe sites draw a random keypair address
  (`seed::fresh_probe_address`), and the four rollback proofs now read the
  refused transaction's **own** `preBalances`/`postBalances` — one atomic
  record the chain wrote, covering every account the transaction touched —
  with every refusal printing its component values so a compound condition
  can never again cost a diagnosis cycle.
- **Devnet drops transactions, and the founding met it at its last hostile
  probe.** The fifth attempt reached the DCLTGMF1 stage — Found31 (224,735 CU)
  and both DCLTPCB1 lanes (700,292 CU) landed and verified — and then died on
  the substituted-Claims hostile probe: devnet dropped that transaction (its
  blockhash expired before it landed), no status ever appeared, and `confirm`
  hit its 300 s deadline and hard-errored. No principal moved — the Lock lives
  inside the atomic DCLTGMF1, which never ran. This is the "devnet dies
  mid-ladder" case reaching a transaction the driver did not resubmit, and it
  is a genuine network property, not a defect. Fixed: `confirm` now resubmits
  the same signed bytes every ~30 s (idempotent by signature — a duplicate
  lands as the same signature and the chain deduplicates) until a status
  appears or the deadline passes.
- **A compute-heavy transaction at priority zero is what a leader drops
  first — and the drop verdict itself was being spoofed.** Attempt 8
  completed the diagnosis: every small transaction landed in seconds while
  the 1.4M-CU DCLTGMF1-route transactions were left behind for full
  blockhash lifetimes, repeatedly, at priority zero. Every campaign
  transaction now carries `SetComputeUnitPrice` at 50,000 microlamports/CU
  (~70,000 lamports of priority on a 1.4M-CU transaction). And the
  height-vs-lastValidBlockHeight expiry check was declaring "dropped" ~20 s
  into a measured 148-block (~60 s) margin, because the load-balanced
  endpoint served the blockhash from one replica and the height from a
  fresher one; expiry is now believed only from this process's own wall
  clock (≥75 s since submit) plus a finalization-depth margin, so a false
  Dropped can never rebuild a transaction that was still in flight.
- **Read-your-writes, made structural anyway.** The driver's `Rpc` now
  carries a per-connection read floor — every confirmed transaction raises
  it to its finalized slot, and every single-account read passes it as
  `minContextSlot`, retrying the node's `-32016` not-yet-reached answer
  inside the confirmation deadline. Reasoned hardening for a load-balanced
  endpoint, kept on its own merits; explicitly **not** the bug above.
- **The second attempt found the probe the first had not needed**: attempt 1
  had also created `collateral-wallet[0]` (initialized against the index-1
  mint), and attempt 2 — peek fixed, mint back at index 0 — collided on that
  wallet mid-transaction ("account already in use") because the detector's
  partial probe list covered the mint but not the wallet. The wallet is now
  peeked and probed the same way, so a half-founding refuses with the account
  named. Attempt 3 runs on a fresh collateral key set (`sol2-*`), which is
  what moved §6.1's addresses.
- **Strands from the paid lessons**, on devnet, never recycled per the
  charter: attempts 1–4 each left a collateral lineage (a mint + wallet, its
  realm record, a Market-scoped RentV2 credit, a Found31 routing table —
  attempt 1's wallet is `8Lng2CshnKa8fGUBac1svdGzSvif1HSt9U4DE7uys5o5`,
  holding that lineage's atoms), ≈ 0.02 SOL in total across the four. The
  mint-independent records — product graph, source specs, manifest, basis —
  are content-addressed and were **reused verbatim** by every retry, which
  is why each successive attempt was cheaper and faster than the last.
- The relayed-vertical build was red at HEAD: DRIVER's `cluster.rs` split made
  the successor's `rpc.rs`/`seed.rs` import `crate::cluster`, and the
  vertical's `#[path]` module list never gained it (the journey's did). One
  module include fixes it; found because this lane builds what it ships.
- The twin's "synthetic" DBC deployment slot (423,941,138) is the **real**
  mainnet value — tonight's read of `HUfnSSiJ…` confirms slot, and adds the
  real authority `JADaUV8kvDpDbJr55wxXJHVaBS3VCj8thZZHjfeuCVLd` and ELF-tail
  digest `c7613080…` (2,326,622-byte ProgramData). 1,586,969 real
  `VirtualPool`s enumerated (766,251 pre-graduation).

## 8. What SMOKE-1 still needed at this checkpoint

**Superseded plan.** The list below records DEPLOY-1's conclusion on
2026-08-28. The first item has since been answered in source by decision 0013
and the compact `DCLTPCB2`/`DCLTGMF2` routes; see
[the compact-founding record](FOUND_COMPACT_2026_08_28.md). Those replacement
routes are offline evidence until a checked Upgrade and fresh devnet lifecycle
are recorded. The other items remain historical inputs, not claims that a
current bounty, market, or browser trade exists.

The durable substrate is up, activated, permanent, and detector-confirmed
(§2–§3). The founding seam is wired, proven end-to-end on the local harness
(DCLTGMF1 at 1,199,741 CU), and driven on devnet through Found31 and DCLTPCB1
with real signatures; the atomic founding itself is stopped at one named,
chain-confirmed wall. In order of consequence:

1. **THE WALL (authority decision, recommended answer attached).** The
   DCLTGMF1 frame must lock **at most 64 unique accounts** to execute on
   devnet — `TooManyAccountLocks`, `increase_tx_account_lock_limit` absent
   there, active locally. Recommended: measure the frame's exact unique-lock
   census, then take the frame-narrowing rows already queued
   (RECORDS-MIGRATE: the Found-frame +6 accounts, stored bumps, funding-state
   footprint) with 64 as the design bound; treat a devnet feature activation
   as weather, not a plan; re-deciding the five-stage atomicity is the
   fallback authority question if narrowing cannot reach 64. Every other
   market (abandoned, graduation, BTC/ETH extras) founds with one command
   each once this lifts — their inputs are compiled and committed tooling.
2. **The graduation market's `account_set_id` cross-check** with the operated
   daemon's `show-config` (`63918468…` from this lane; two authors, one
   number), then INFRA-RELAY's arming with MARKET + GENERATION + vacancy.
   The relayer is live disarmed on the box, fee payer funded, attestation key
   pinned by the compiled input.
3. **The founding-prestate unwinds**: the sol6/sol7/sol9 staged DCLTPCB1
   prestates (each ~500M demo atoms of its own lineage's mint + ~0.01 SOL of
   rents) unwind through DCLTPCA1 after their 500,000-slot expiries (~2.3
   days) — three cleanup rows, no real principal.
4. **The N-trader life + conservation ledger** (`ledger-census` is built and
   documented; four traders funded and waiting) and **the redemption ALTs**
   (PAYOUT's step-2 rider) — both per market, once one is Open.
5. **The web smoke pages** flip live via the one-record `lib/smokeMarkets.ts`
   edit once markets exist; the endpoint default already points at public
   devnet and names the cluster from its genesis hash.

## 9. The wallet ledger, final for this run

| checkpoint | lamports | SOL |
|---|---:|---:|
| start (= SMOKE-0's final) | 64,990,412,120 | 64.990412120 |
| seven roles deployed (rent parked) | 33,187,407,520 | 33.187407520 |
| record layer + five-role activation | 33,151,551,640 | 33.151551640 |
| INFRA-RELAY fee payer funded (0.05) | 33,101,546,640 | 33.101546640 |
| four traders funded (4 × 0.02) | 33,021,526,640 | ~33.021 |
| after ten founding attempts (final) | 32,185,584,146 | 32.185584146 |

Founding-attempt spend ≈ 0.836 SOL total: market-record rents (content-
addressed, reused by every retry and by every future founding of these
inputs), per-lineage collateral/credit/ALT rents across the paid-lesson
strands (§7), ~1,000+ transaction signatures, and priority fees at 50,000
µlam/CU on the final attempts. Peak exposure never exceeded 32.9 SOL against
the 40 SOL cap; the ~31.77 SOL of program rent is parked, never burned, and
iterable by `Upgrade` at fee cost per decision 0012. Nothing was recycled;
the substrate is permanent.
