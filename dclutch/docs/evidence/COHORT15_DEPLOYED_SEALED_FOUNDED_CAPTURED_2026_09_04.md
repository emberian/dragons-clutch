# Cohort-15: the deploy that could not be built, the digest written before the money moved, and a capture inside its window — 2026-09-04

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized.

Tree root `/Users/ember/dev/dclutch`. Every deployed byte is built from
`1cae26fd61defbefd20bcd52acc449b6e94e64ed`.

## Headline: the digests a reader looks for first

| | |
| --- | --- |
| deploy commit | `1cae26fd61defbefd20bcd52acc449b6e94e64ed` |
| checked release gate | `1e614d92c1ec19ed2fcfba4cc255e3d2bbd3c6a955d44a0d3ebdb3ce6f47bbbe` |
| prepared plan release set | `9895faee8f7f6a1926df18302f1b003afcf4b6c56518ba7bba2614c86eea8a22` |
| sealed plan release set | `9895faee8f7f6a1926df18302f1b003afcf4b6c56518ba7bba2614c86eea8a22` |
| the Open Market's own byte 208, **read off chain** | `9895faee8f7f6a1926df18302f1b003afcf4b6c56518ba7bba2614c86eea8a22` |
| sealed deployment set final digest | `824ae36f3da8a031026e8e4e76dd7320f4f2ccd5aa745a6231cab8f3ebff76c3` |

Cohort-13 was the first cohort whose founding and whose seal named one release
set; cohort-14 added the third statement, the Market account's own byte 208.
Cohort-15 is the third cohort to say all three, and it adds two things no
previous cohort had: **a Core whose Product-graph projection was recorded before
the deploy spent a lamport**, and **a statistic that carries the source-to-result
scale factor**, so its cuts mean what they say.

## 0. THE DEPLOY COMMIT MOVED, AND THE GATE ITSELF IS WHY

The lane pinned `8599cfc69`, built a checked release candidate against it, and
got this and nothing else, fifty minutes in:

```
SBF build freshness PASS links=12
```

No `CANDIDATE_EXIT`. No error. The reason was one line in
`<work>/product-handoff/build.log`:

```
error: cannot update the lock file .../successor/Cargo.lock because --locked
was passed to prevent this
```

**`8599cfc69` cannot build a checked release candidate at all.** Its
Product-handoff phase builds the successor host producer with `--locked`, and
`1c49ecac2` — which added `dclutch-hot-bump-miner-v1` to `dclutch-operator`'s
manifest — updated seven lockfiles and missed two of the workspaces that build
`dclutch-operator`. Measured in a detached worktree at HEAD, which is the only
place the question can be asked: a shared checkout already carries the
regenerated file, so the tree everyone works in is green and the tree everyone
else clones is not.

`1cae26fd6` supplies both rows, and **the lock commit moved no shipped byte** —
measured, not assumed: two detached worktrees, one at each commit, produced all
seven role ELFs byte-identically.

The instrument that hid it is fixed at its author in `8f9e53b19`. Five steps of
the gate redirect their whole output to a log and abort silently under `set -e`,
and each has a guard written underneath it that `set -e` makes unreachable —
dead code that reads like a check. `step_refused LABEL LOG STATUS` names the
step, names the log, and prints its tail. Proven red on the exact failure shape:
before, the previous PASS line and `exit=101`; after, `REFUSED: source-pinned
Product producer build exited 101` with the `--locked` line quoted.

**It has now cost two lanes with two different causes.** A cold machine met the
same silence on 2026-09-03 over four crates a `cargo fetch` would have supplied
(`COLD_MACHINE_2026_09_03.md` defect 3). Both diagnoses were sitting in the log;
neither reader was told the log existed.

## 1. The gate certifies the bytes this cohort runs

`checked-release-candidate.sh --genesis-cohort` at `1cae26fd6`, from a detached
worktree at that exact commit.

```
source_revision=1cae26fd61defbefd20bcd52acc449b6e94e64ed
sbf_build_freshness=passed          sbf_build_freshness_links=12
sbf_build_diagnostics_total=0       sbf_build_diagnostics_accepted=false
infrastructure_lineage=genesis      cargo_lock_immutability=passed
cargo_lock_count=70                 node_version=v26.4.0
spline_product_handoff=passed
checked_upgrade_gate_sha256=1e614d92c1ec19ed2fcfba4cc255e3d2bbd3c6a955d44a0d3ebdb3ce6f47bbbe
CANDIDATE_EXIT=0
```

**Two independent builds agree, and a third instrument reads the chain.**

| role | bytes | SHA-256 | second worktree | on-chain dump |
| --- | ---: | --- | --- | --- |
| registry | 240,440 | `3a6d615de8cf51fbb59dc8ebe121dba0aabbd4cd6aab7e5924317bf30f4e9ec4` | identical | IDENTICAL |
| rent | 143,736 | `738d847981f60815deffdb62beb834b7c634767a86560b2128a6834fad65ad60` | identical | IDENTICAL |
| custody | 574,336 | `15a421ae0928be1017cf553991f8eaee222c4669c6e2ffeb48325ce2be8400bb` | identical | IDENTICAL |
| resolution | 829,888 | `24af85048c086c28b529960f2b785b58a794026c73a7e7799aa6da6df340ac9d` | identical | IDENTICAL |
| claims | 1,387,496 | `f6ab44acb90423440af02d312466305b3a4dea3afa793999d06f1ae1c3850d70` | identical | IDENTICAL |
| trading | 2,341,296 | `7e581f12c89a56cfb9fa34da065db30ea02c17ee9e768439d10e6a13b358de24` | identical | IDENTICAL |
| core | 1,193,400 | `a5385ba240b6eeae2cb3279d1014690cc44bf87e593dc9b0dc51b974142c73b6` | identical | IDENTICAL |
| | **6,710,592** | | | |

The reproduction is same-host, which is not a formality: `platform-tools` ships
a standard library carrying its own CI build path, so a Linux and a macOS build
of one commit differ in nine of ten ELFs for a reason that has nothing to do
with this tree (`COLD_MACHINE_2026_09_03.md` §3).

**An independent cross-check fell out of it.** The cold-machine campaign
recorded the laptop's registry ELF at `fe70f0769` as `3a6d615de8cf51fb…` and its
trading as `7e581f12c89a56cf…`. This cohort's candidate, built twenty-five
commits later by a different lane, reproduces both digests exactly — two lanes,
two work roots, one artifact.

## 2. Cohort-14 closed

Ids derived from cohort-14's own keypair files, never transcribed. Its markets A
(unresolvable-honestly), B and C (Terminal and paid) hold nothing that needed
preserving.

| role | cohort-14 program id | rent reclaimed (SOL) |
| --- | --- | ---: |
| registry | `ySYoUvUw7Z5AtDNqxQAo93vJXD1enNoK8Bf5uLRSyRm` | 1.508349609 |
| rent | `4oQLFDM9TbGBdb2q6QZCxRKZ3u5sqhTycb9MeHt9k41r` | 0.898355049 |
| custody | `8mWrLG2sjfzSKA3fEVBfY3RkGLTLuZZjKqDXWDuTpLbk` | 3.652399425 |
| resolution | `5ML5pbUfCaDwokNtmLyTgDEb7eHrfDRrW4PmktXAmphs` | 5.195726193 |
| claims | `H8ANKXECwkntr8Cczo6gZX5d9PWN6uwrqCyeohYsZVhV` | 8.702890929 |
| trading | `DcsWHSjPTTpYzXScmB5xYh3iEsM9fx4YFC1BPvQggEtu` | 14.732464233 |
| core | `9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB` | 7.514718801 |
| | **total** | **42.204904239** |

Deployer `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`: **27.271722270 →
69.476591509 SOL**. The observed delta **42.204869239** is the table total less
seven 5,000-lamport fees, which closes exactly.

**Verified by asking the account that can answer.** A closed program keeps its
36-byte Program account, its executable flag and the ProgramData address it
names at offset 4, so the stub cannot tell a live cohort from a dead one. All
seven stubs still read 36 bytes, kind 2, `executable=true`, still naming their
ProgramData; **all seven ProgramData accounts are vacant.**

## 3. The redeploy

Seven programs, fresh identities, each verified by dumping the on-chain image
back **before the next one started**.

| role | program id | ProgramData | deployment slot |
| --- | --- | --- | ---: |
| registry | `8ci2LojrUPtVZoByaAmoNCj8244DTdxkUkHRVA9JiHdV` | `2Xvmiz5kBseLAeCCgJsuJmK7UHQ1fzzbaSm7FqHF8Eat` | 492,745,516 |
| rent | `EQ61nrN71xaTsDMSrrH9FzMeqc27ghRex9abyjKu8vC4` | `6jqwvw8jPSqomLLir3rNohLHAm5awrkcx4b3WvjxVmSm` | 492,745,574 |
| custody | `4sCbUV6f8iZaaVDkp3qAJtxiLY1cfBcKrVc5wpAFjpzM` | `D7X4Wximtu9BwHXdrWgWpXkjMCFS7xPtV3UevgMVXhKB` | 492,745,658 |
| resolution | `24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn` | `B8N72QJgwKypTfgr9TsLDtKWjrCFeJ9n7FjsjV1xgB71` | 492,745,773 |
| claims | `595PExHje1GnbryQtXvpxSfxK72snpMcrG1Sj8VpPLEe` | `7RLeuS5vDph1VpgG3NGYwZnjnTxvwmwJE2FfgWNUDWPG` | 492,745,957 |
| trading | `3gBSSjYwSC4phutpGKRkMhrnCDVzHu6kfQ3L4jLf2UmG` | `CnER99tNi7gz24g7sFEzoUoxa9Q3cZe8FMJTdu9iEfxG` | 492,746,142 |
| core | `7hGerMC6Wj742FVTyiF9PhRnGSBzbee7TMZ6sUytsmFr` | `DrT1XF1qDgbTs8WCkhk1yjxFyehPSBM1VQrrowipmDyt` | 492,746,271 |

**Three instruments, one claim.** The dump comparison per role, the byte count,
and a hostile decode of each live ProgramData — all seven `MATCH`. All seven
carry an authority tag of 1 equal to the retained deployer, are Loader-owned,
and their ProgramData accounts are non-executable. The ProgramData address is
**read**, not derived: it is the 32 bytes the Program account names at offset 4.

Deployer **69.476591509 → 26.930152826**; the redeploy cost **42.546438683**.
The runbook priced it at **42.546563** from cohort-13's measured 6,340.21
lamports/byte over 6,710,592 bytes. **The projection and the measurement agree
to 0.000124 SOL** — 0.0003% — which is the model earning its keep for a third
cohort. The affine `890,880 + 6,960·n` ceiling would have said 46.71 and
over-predicted by 9.8%.

## 4. THE RECORDED CORE DIGEST, written before the deploy spent

`075098e5f` made the founding predictor a model of the DEPLOYED Core and left
`RECORDED_PRODUCT_GRAPH_CORE_ELF_SHA256_V1` empty. `tools/cohort15/README.md`
calls filling it *"the step whose omission is invisible until money has moved"*:
the refusal is correct, fail-closed, and arrives at the FIRST founding — after
the redeploy has been paid for.

**Cohort-15 wrote the row before any transaction**, in `9c943d4f2`, from the
candidate's own output:

```
a5385ba240b6eeae2cb3279d1014690cc44bf87e593dc9b0dc51b974142c73b6
```

A row keyed by BYTES can be written the moment the candidate is built, and the
claim it makes is checkable with no chain at all:
`git merge-base --is-ancestor b312ce3c4 1cae26fd6` is TRUE, so this Core fills
all eight Product-graph nibbles where cohort-14's `864394530f…` writes zeros.
`fc6ed37a6` later corrected the comment to name the commit the cohort is
actually deployed from.

**The disjointness test was half a test, and that is the finding beside the
row.** `no_core_digest_appears_in_both_lists` iterated `UNRECORDED` only, so its
three assertions applied to whichever list happened to be non-empty — which,
with `RECORDED` empty, was every row in the file. The first row ever written to
`RECORDED` would have been the first unchecked one, and `RECORDED` is the list
that grows once per cohort forever. **PROVEN RED on this commit's own row**:
with one character of the digest uppercased, the pre-fix loop passes (1 passed)
and the fixed loop fails naming the row — *"A5385ba…c73b6 is not lowercase hex"*.

The proof that the row was needed and correct is negative and total: **no
founding in this cohort refused on a bump tail**, where cohort-14c paid 0.139
SOL and an orphaned Found37 Market to discover the same question unanswered.

## 5. The ladder, and the TENTH record

`campaign --through activation`, deployer as Core upgrade authority, campaign
payer `D5qe7ZoQ6LimRfk9FG5zy29kULz8ykYmt8Hsb4r5oWjx` funded with 2 SOL first.

**36 transactions, zero errors, `publication: 10 record bodies finalized`** —
the tenth is the General accelerator's `ArtifactRelease`, published by the
`--general-accelerator-*` group, whose FINALIZATION is the deployment
observation under `90a8563f`.

```
campaign stage succession: nothing to execute -- this cohort is born at V2
and carries no ceremony; observed absent
```

**Re-observed from the CLUSTER** by a second preflight that reads the chain
rather than the driver's exit code — substrate, publication, initialize,
succession and activation all `complete`.

Deployer **24.930147826 → 24.890564434**: **0.039583392 SOL**, which is
cohort-14's ladder figure to the lamport across a different release set.

## 6. THE SEAL, before the founding and at zero cost

All five owned roles preflight `equal: true` against a fresh finalized
observation; registry and rent carry-forward.

```
completed_role_count 7      next_role null
final_set_sha256 824ae36f3da8a031026e8e4e76dd7320f4f2ccd5aa745a6231cab8f3ebff76c3
```

`prepare --deployment-set-journal` then produced `plan-seal.json` whose
`checked_upgrade_set_final_sha256` is that digest with `release_set_id`
**unchanged**.

**Cost: 0.000000000 SOL.** Deployer 24.890564434 and payer 2.000000000, both
unmoved across the whole seal. Nothing was signed.

## 7. THE FIRST FOUNDING DIED AT TRANSACTION 146, AND NEITHER DOOR OPENED

145 campaign transactions in, past the funding of the founding principal
supplier and past the lifecycle RentCreditV2, the driver refused:

```
publish record: Begin 9QwZ6ard…: sendTransaction RPC error: code -32002
Transaction simulation failed: Blockhash not found  err BlockhashNotFound
```

A transport failure that spent nothing on that transaction. Rerunning refused
both ways at once:

```
this founding has STARTED on this chain (the Open Market does not exist at
AQgrWny3ghhvQ85uSb8NG2pfMwJcrjCMTDUP6DHVwRrC but this founding has started:
collateral mint GjZdUCnbgw4Jt94cbA87wWtegbFnzrw3rHfgKRYkANbT, collateral wallet
DVnDR13mGXNwnaNjxQDPoyjBLymKzp7LYuRsD4sScEaH, realm record
668UXmS1VvDjEtANUGD5sgfFcUt5iqcKKyQ61gqDefcT, Found31 Market
JCVGcYkyxav2LMDx4H5m8bcrDafaesSNt5Cm5mgWpgQp), but no compatible durable
DCLTPCB2 checkpoint authenticates a safe suffix resume
```

**This is cohort-14's owed item 2, met a second time from a different cause, and
that is what makes it worth writing down.** Cohort-14's General founding died at
the FIRST funding instruction on a shared-keypair collision — a bug, since
fixed. This one died 145 transactions later on a blockhash race that no fix
prevents, and earned the identical refusal. The gap is therefore not "a rare
collision": it is **every founding that dies anywhere in the long stretch before
DCLTPCB2**, and on a public endpoint that is one transient RPC error away. Both
refusals are correct; what is missing is a resume for the prefix they describe.

Attempt 1 cost the campaign payer **0.215699220 SOL** and left four per-market
accounts abandoned in place. Attempt 2 used FRESH `collateral-mint` and
`collateral-wallet` keys — and deliberately **did not** regenerate
`founding-source-funder` or `founding-projection-witness`, because `3ba991025`
made those per-market coordinates derived from the secret AND the Open Market's
address. That is `3ba991025` exercised a second time, by a lane that needed it
rather than a lane testing it: **one key file, three foundings, three funders.**

Attempt 2 ran **83 campaign transactions against attempt 1's 145**, and the
difference is entirely record reuse — a record body is content-addressed, so the
re-founding paid for nothing attempt 1 had already finalized.

## 8. The market IS founded and open, and its cuts mean what they say

| | |
| --- | --- |
| Open Market | **`3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2`** — 368 B `DCLTCOR3`, Core-owned, phase `0x01` Open, readiness `0x02` Consumed |
| `selected_release_set` @208, read off chain | `9895faee8f7f6a1926df18302f1b003afcf4b6c56518ba7bba2614c86eea8a22` |
| collateral mint / wallet | `nWANBKurv2STKgkAdGuk2ZSUV3rQpJqU43hVv845CpP` / `2388f4ha6J1da74z53LbVJH1haWqV2SQrpK7s3cZmL6g` |
| Claims aggregate / Hoard | `GRzD2Mkb89k8fsMXPdvrQsuvMFhg41pHJT6tvsHmruyK` / `B8dmPQGkqoFsgXmCpRTY3LxqVyARvJaBJxt2LAf2FWbj` |
| DCLTGMF3 frozen table | `4a3S6qTKmXAGF5qPcWpDMsgbQiDCHjZdmhExA262NpMT` |
| window record | `7nPCcGCWfPA5vH7xwyDA6LgmUEhKwMbgP9jJQ8B5YjXf` — 112 B `DCLTWIN1` |
| activation | verdict **`ACTIVATED`**, root `FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG`, deadline slot 492,976,232 |

### The cuts, re-centred on a spot measured three ways

Read immediately before staging off the sponsored PriceUpdateV2
`7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` — the account this market itself
resolves against — SOL/USD was **$103.951853**, conf ±$0.011853, `publish_time`
281 s old against a ~313 s cadence, with **Coinbase $103.79** and **Kraken
$103.78** agreeing to 0.17%. So `--cuts 10200,10600 --cut-denominator 100
--band-anchor 10395`: $102.00 and $106.00 straddling a measured $103.95.

**AND THIS IS THE FIRST COHORT WHOSE STATISTIC CARRIES THE FACTOR.** Cohort-14's
markets B and C compared a raw Pyth mantissa at exponent −8 against cuts in
dollars, and each paid the cell the deployed program chose rather than the cell
its reading falls in — market C across a position a stranger had paid for.
`4cd2b9cb5` gave `StatisticSpecV1` a `source_scale_exponent` in four bytes that
were already reserved and enforced zero, and the devnet founding writes it from
the observed publication's own exponent. A market founded after that commit
which declares a conversion and omits the number cannot resolve at all: it
refuses `ResolutionError::ProviderScale 0x801C`.

## 9. The strangers, and the aperture fixed before the boundary

Both participants are **cohort-14's own keys, reused deliberately**: they were
already funded at 0.082368928 SOL each, and a stranger is an identity rather
than a per-cohort artifact. No participant funding transaction was needed.

| | position | signature |
| --- | --- | --- |
| participant-1 `5Sqp8ony…` | `NKdAYFXgMi4PquGMwiZJWb2wgCTuYmimWs517GpkJDm` | `2wqT3rjHAvUxXpJh76rK7uLNGnPmY4NUQCX152aUWRhHGVfizAhRR6cL8hL8pJbqM6uvtkqL3wVjHPMcQfhSsJFT` |
| participant-2 `92RrSCo9…` | `C2qSFaZb1qRRGueEtmHdBscuBCyPLfuKkF1NBoCCx72G` | `2yRSDsSs3oRCcjzb4LhyjhWVr9XcuKZJgeQ792Rq2CecUdsiit41AepmUs8ujz7WHkYwdfjqNhPesZXHxofdBp4u` |

The collateral delegation put exactly **201 atoms** — `required_buyer_collateral`
= gross 200 + buyer fee 1 — into
`HkZpRaZEtvRw7UqxZFVxXhfja6wM8UHfhMr7NWz5uM1k`, 165 bytes under Token-2022,
read back off chain.

### THE SIMULATOR HALTED, AND IT WAS RIGHT

```
VIOLATED L1: tracked 999999799 atoms across 2 accounts != Mint supply
1000000000; 201 atoms are in accounts this ledger does not name
HALTED: ledger-census violated a conservation law at cycle 1
```

`build-sim-config.py` binds only the founder's collateral wallet, and the
delegation had just moved 201 atoms into an account outside the aperture.
**This is cohort-14 §13's lesson arriving one law earlier**: there the
incomplete aperture cost L7 at the fill boundary and was reported as
INAPPLICABLE; here it costs L1 at the admission boundary and is reported as a
VIOLATION, because L1 is stated about a total rather than about a difference.
Same cause, louder instrument.

The bindings now name the delegated account, all three Positions and both
participant owners, and `cohort15-pre-capture` was taken with that complete
aperture **before** the first boundary:

```
HOLDS L1: tracked 1000000000 atoms across 3 accounts == Mint supply 1000000000
HOLDS L3: 3 Positions sum to the aggregate supply vector [500000000 x4]
HOLDS L4: Hoard 500000000 >= worst outcome 500000000 x unit 1
INAPPLICABLE L2 / L5 / L6 / L7 / L8 — first boundary, each in its own words
```

## 10. The prepay, and the armed relay

Both terminal input documents were produced at founding time, not resolution
time — `terminalSequence 0` for the capture and `1` for the settle, because the
consumer refuses a mismatch and the producer never overwrites an output path.

| | |
| --- | --- |
| certificate seat | `84YxYUrEYxYyRHAWSnZrNn7D1GeA6teGfvC3hdBfNh9K` |
| lamports | **2,786,520** = `rent.minimum_balance(312)` on this cluster, to the lamport |
| owner / bytes | System, 0 — a seat the walk has not taken yet |
| signature | `2tV9iqraGgq7AYsAb9tAnWMLpX896tVQD4ksHgrCpGWMoWEZP5hwytSgbQR3UYpqJbKr17LavbtbxBb95VSF6Muj` |
| slot / CU / fee | 492,766,541 / 450 / 75,000 |

`ab0322d50`'s legacy send path and `3e5e0b0be`'s legacy report authenticator both
ran with no wall in front of them, for the second cohort running.

## 11. THE CAPTURE LANDED, INSIDE ITS WINDOW, ON ATTEMPT 5

| | |
| --- | --- |
| signature | `3VsB7dNppiq6uM7QGCpd4G4WGJjJSGaDxyvyct5UZFJN9Fik67NgUswbzNhebDSeCb5tNxyKTsiGhxLnyKbD6AeM` |
| slot / CU / fee | 492,775,238 / **106,810** / 75,000 |
| fired at | 01:13:37 UTC, **1,711 s of window left** |
| candidate | `EsdcBPpSQycmvB3AH5kkE5J62XLF7Y3WXwNzKgUjKX5s` — 432 B `DCLTSPC1`, 3,546,480 lamports |
| head | `1419PF9Bn9p6QnpfAoSExTzcox1SU8UEevLs56yoJaQm` — 336 B `DCLTSPH1`, 2,938,512 lamports |
| **the verifier** | the candidate's own snapshot seconds, read off the account: **1788484405**, **1788484406**, **1788484422**, all inside `[1788484328, 1788486128]` |

The window was read off the market's own `DCLTWIN1` record at offsets 48, 56 and
64 — start 1788484328, end 1788486128, max_age 7200 — never from a handoff
table. The guard is the last statement before the action and it EXITS.

**FIVE ATTEMPTS, AND THE LADDER IS WHY IT LANDED.** Attempts 1–3 refused
`0x8015 ResolutionError::SponsoredPush` in simulation, spending nothing; attempt
4 refused *"derived sponsored coordinates changed after preflight; use a new
output path"* — the candidate PDA is seeded by the observation's own
`publish_time` and `posted_slot`, so a plan one provider push old names an
address the signer will not reach. Attempt 5 planned and signed back to back.
Cohort-14b's *"the retry ladder, not the margin, is what makes the capture
land"* is measured here a second time, needing four passes where cohort-14b and
RELAY-C each needed one.

The seat read back **0 bytes, System-owned, still exactly 2,786,520 lamports**
after the capture: prepaid and unallocated.

### THE CENSUS: L7 VIOLATED FIRST AND LOCATED WHAT NOTHING ELSE COULD

Declared with only the capture's own fee:

```
VIOLATED L7: the payer moved -6559992 lamports since cohort15-pre-capture and
its transactions paid 75000 in fees, but watched accounts gained 0; -6484992
lamports are unaccounted for.
```

**6,484,992 = candidate 3,546,480 + head 2,938,512, to the lamport.** Neither is
a token account, neither is a Position, and neither closed, so L1–L6 are blind
to both by construction. Re-declared with both by name and by lamport,
`cohort15-post-capture-complete` reads **ALL EIGHT LAWS HOLD**. Both readings
are kept; the violation is the finding, not a draft.

This is the third cohort in a row where L7 has earned its pass by failing first,
and the second where the residue it named was exactly two rent-bearing accounts
a capture creates.

## 12. The General market: compiled with DERIVED widths, and re-minted against the live chain

`tools/cohort15/steps.tsv` row `02 refound-general` exists because cohort-14's
General market publishes an `Exact(48)` RentCredit coordinate — 48 being the
width in `account_rules_v3.rs`'s own `#[cfg(test)]` fixture, transcribed four
times, while the only RentCredit this protocol produces is
`LIFECYCLE_RENT_CREDIT_BYTES_V2` = 128. An `Exact` prestate is a refusal, not a
preference, and the width is inside a **founded** `AccountProfile`.

Cohort-15's policy document is `dclutch-general-devnet-policy-v2` — windows and
`token_account_bytes`, and **no `external_widths` block at all**. `3a8ac205d`
made `general_external_account_widths_v3` the one author: nine of the eleven are
protocol constants and two are functions of the run spec's own Product graph, so
none was ever a policy choice. A stale v1 document refuses by naming the field
that stopped being a question, rather than by being read with its widths
dropped.

```
market.json compiled
deployment_slot        491959038      <- the runbook's verifier, matched
elf_digest             61b2d73d44f2470051b40e39cda1d31a5f67679429eacd5448d5e5ac583b74ae
artifact_release_id    dcce810097111f696e4888ef89385fc0e1e1c24d89818ca040d950d1daec5b94
```

The sponsored release was **RE-MINTED against the live chain** at finalized slot
492,768,891, and both conjuncts of `0x8014` moved together — which is
cohort-14b's fix exercised by a lane that needed it:

```
receiver_deployment_slot:  declared 487855452 -> observed 491006444
receiver_config_digest:    declared bbbc324e… -> observed f8aca67e…
```

The push oracle did not move, which is the positive control that makes this a
reading rather than an inference.

**The founding was deliberately not started while the capture runner held the
campaign payer.** Two concurrent signers on one payer is a blockhash race, and
this cohort had already lost one founding to `BlockhashNotFound`. It was started
immediately after the capture landed and was still running when this document
was written; its result, the activation, and the width verifier are §15 item 3.

## 13. The money, so far

| stage | deployer | campaign payer |
| --- | ---: | ---: |
| before anything | 27.271722270 | — |
| after closing cohort-14 | **69.476591509** | — |
| after the seven-program redeploy | 26.930152826 | — |
| after funding the payer | 24.930147826 | 2.000000000 |
| after the ladder | **24.890564434** | 2.000000000 |
| after the seal | 24.890564434 | 2.000000000 |
| after founding attempt 1 (died) | 24.890564434 | 1.784300780 |
| after the re-staging and founding attempt 2 | 24.890564434 | 1.588734021 |
| after the activation, the prepay and the admissions | 24.890564434 | ~1.53 |
| after the capture | 24.890564434 | — |

**THE DEPLOYER HAS MOVED FOR ITS OWN DEPLOY AND ITS OWN LADDER AND NOTHING
ELSE.** It has been at 24.890564434 SOL since the ladder finished, unmoved to
the lamport through the seal, two foundings, the activation, the prepay, the
admissions and the capture. Cohort-14 could say that sentence only up to its
General founding; cohort-15 can still say it.

| step | cost | against the 2 SOL bound |
| --- | ---: | --- |
| redeploy (exempt) | 42.546438683 | — |
| ladder | 0.039583392 | within |
| campaign payer capitalization | 2.000000000 | at the bound, by design |
| **the seal** | **0.000000000** | key-free, read-only, nothing signed |
| founding attempt 1 (bought a finding) | 0.215699220 | within |
| founding attempt 2 + activation + prepay + admissions | ~0.42 | within |

## 14. What cohort-15 has that no previous cohort had

- **A Core whose Product-graph projection was recorded before the deploy spent a
  lamport.** Cohort-14c paid 0.139 SOL and an orphaned Found37 Market to learn
  the question; cohort-15 answered it from the candidate's own bytes, and no
  founding in this cohort refused on a bump tail.
- **A statistic that carries its scale factor**, so the cuts a founder authored
  in dollars are compared against a reading on the same scale. Cohort-14's two
  settled markets could not say that, and one of them paid the wrong cell across
  a stranger's position.
- **A General market compiled with DERIVED external widths** and a policy
  document that no longer states a single account width.
- **A capture that landed inside its window on a cohort whose provider release
  was re-minted from the live chain at plan time**, both conjuncts.
- **A checked release gate that names the step that killed it** — and a deploy
  commit chosen because it is the first commit at which the gate can run at all.

## 15. Owed, in priority order

1. **The settle**, legal strictly after `window.end + max_age` = **1788493328 =
   2026-09-04 03:42:08 UTC**. The seat is funded, the `terminalSequence 1` input
   exists, and the verifier is the certificate's kind byte at offset 10: **1, not
   4**. Then admit-terminal, the Claims custody replay, the ATA and the payout.
2. **The fill.** `sim-config.json`'s trade section is still `mode: none`; the
   figure to record beside it is cohort-14C's **1,281,582 CU**.
3. **The General founding, activation, and the width verifier** —
   `devnet-general-session`'s two-encoding offset probe must recover
   `Exact(128)` at the RentCredit coordinate, against cohort-14's `Exact(48)`,
   and 128 must equal the width of this market's own lifecycle RentCredit as the
   chain holds it.
4. **OpenBatch on devnet has no driver, and that is a build.**
   `devnet-general-session` is read-only by construction and builds no
   `GeneralHotStateV3`; the seam is `build_general_successor_instruction_v5` then
   `compile_general_successor_v0` over a frozen table. `0948b0224` reports the
   General-hot suite GREEN, so the wall is a producer and not the protocol.
   Separately, `general_session.rs` still pushes the caller-authority
   slot-binding wall **unconditionally** and its module doc still describes the
   pre-`3a8ac205d` seed: that commit moved the derivation to
   `accelerator_caller_authority_digest_v1` and did not touch this file, so the
   command would report a wall that no longer exists.
5. **Retirement on devnet is still unreachable, and the reason is two lines.**
   `local-private-validator-aggregate-retirement-v1` hard-codes
   `ExpectedClusterV1::OwnedLoopback` (`aggregate_retirement_exterior.rs:115`)
   and passes `None` as the acknowledgment to `ClusterOriginV1::parse` (`:892`).
   The dual-cluster idiom to copy is `claims_custody_replay.rs`: a second
   `COMMAND_DEVNET_V1`, a `command(expected)` selector, a `devnet_usage()`, and
   the expected cluster threaded from the origin rather than fixed.
6. **A resume for a founding that dies before DCLTPCB2** — §7, now met twice
   from two different causes.
7. **`corroborate.py --discover`** over this document, then regenerate
   `docs/reference/route-witnesses.md`. Its devnet class stood at **22 routes**
   before this cohort.

## 16. Provenance

Job directory `~/jobs/dclutch-cohort15-20260903` (mode 700). The endpoint
credential exists in no file under it: every script reads `~/.helius-key` at run
time, the staging generator's emitted `open-market.execute.sh` takes
`--rpc-url "$DCLUTCH_RPC_URL"` from the environment (`674a7873e`, `a217c3fe2`),
and no `cargo run` was used anywhere in this lane — every driver invocation
calls a built binary, because `cargo run` echoes its exec line before it execs.

Deployed bytes come from `1cae26fd61defbefd20bcd52acc449b6e94e64ed` and nothing
else. Four host-tool commits carry this lane's fixes and change no deployed
byte: `9c943d4f2` (the recorded Core digest and its half-a-test),
`1cae26fd6` (the two missing lockfile rows — also the deploy commit),
`8f9e53b19` (the release gate's located refusal) and `fc6ed37a6` (the row's
commit reference). The driver used for the founding was built at `1cae26fd6`,
one commit ahead of nothing — it IS the deploy commit — and it is the build that
knows what the deployed Core writes.

Devnet evidence. Not mainnet evidence.

---

## Addendum: THE GENERAL MARKET IS FOUNDED AND ACTIVATED, AND THE WIDTH THAT MADE COHORT-15 NECESSARY IS 128

Written the same night, after the capture. **Devnet evidence. Not mainnet
evidence.**

`FOUND_GENERAL_EXIT=0`, **576 campaign transactions**.

| | |
| --- | --- |
| General Open Market | **`6aqy89GhhXFtDbawC5ors4HLkGvzdHC4R26TXTaaXRKj`** |
| Found31 / abort Market | `3ydLR6LHDSqoAcpYZkLdH9QT5BMDerYJueamZtXQZ9mr` / `BwLRGqjgWwXN1hx7Ve9hznuTGZjDZjLgdtiiNvN5wmY4` |
| realm record | `HFybHWZPGRfmuD2ZYhWgVWCugZzhSajJWmZye65dmBDD` |
| collateral mint | `6pzxjdrt7pWKUoFNVizWo4UH11CyaiAXVDSg78UTrpuU` |
| Product / ResultDomain / Portfolio records | `A8228FSecWKd3bXUFxEk6UYyksUMxLFDsRKceDSpxaVn` / `8aqkKLNGqzaBkr1rU6WfEEEskpM5eYr2upb3F7a98AYA` / `2yGx2PJn2H8m8iqFhuifF8pvUPFoyF5AHNnjg4PxqDx4` |
| graded liability-basis record | `D7CBjC9f2m7GpTWGZF7UwbZbKv3idzE26JBFNXFnnptT` |
| activation | verdict **`ACTIVATED`**, root `96FQ72xmJKN2RY1jbZJVmQfo8fBaWahef2awVqMWE6qr`, entry index 3, generation 2, deadline slot 492,984,927 |

It was founded with the SAME `founding-source-funder.json` and
`founding-projection-witness.json` files the two Direct foundings used: **one key
file, three foundings, three funders** — `3ba991025` exercised by a lane that
needed it.

### THE VERIFIER, AND BOTH ITS CONJUNCTS

`tools/cohort15/steps.tsv` row `02` asks for two things, and requires the width
to be recovered **off the chain** rather than read out of the file that produced
it. `devnet-general-session` encodes the `OpenBatch` `AccountProfile` twice
differing in exactly that one width, locates its little-endian offset, reads the
value out of the finalized record, and refuses unless the recovered set
re-encodes to those exact bytes.

```
schema      dclutch-devnet-general-session-frame-report-v1
rentCredit  128
```

**128, against cohort-14's `Exact(48)`.** And the second conjunct, read off the
chain independently: this market's own lifecycle RentCredit
`64mXYRdxPFUCjCVBXgsdopQ2rZZwwPWw5Ne3hqEWVucD` is **128 bytes, `DCLRNTL2`** —
and its founding sibling `4BA8NTaEBrij2MXu4Si1zF9aw8AFtUntchL3rGxqCvNz` is 128
bytes too. Two independent authors agree, and the policy file is no longer
either of them: `dclutch-general-devnet-policy-v2` states no account width at
all.

**Cohort-14's first wall is closed.**

### AND THE COMMAND STILL REPORTS THE SECOND WALL, WHICH NO LONGER EXISTS

```
REFUSED: [session/caller-authority-slot-binding] OpenBatch is not deliverable
against market 6aqy89GhhXFtDbawC5ors4HLkGvzdHC4R26TXTaaXRKj:
1 unsatisfiable conjunct(s)
```

Exactly one, and the rent-credit wall is not it — so the report is the width
verifier passing and the caller-authority row is the only thing left. But that
row is **stale, not observed**. `general_session.rs` pushes it unconditionally:

```rust
walls.push(json!({ "code": "session/caller-authority-slot-binding", … }));
```

and its `detail` still describes the pre-`3a8ac205d` preimage —
*"role_request_digest is sha256(accelerator request header ‖ inline bank) …
each address is therefore a function of the slot"*. `3a8ac205d` moved the
derivation to `accelerator_caller_authority_digest_v1(kind,
parent_request_digest, index)`, proved it with
`one_signed_account_list_opens_the_same_batch_at_two_execution_slots` (one
signed frame, two banks 47 slots apart, 55-entry account list byte-identical),
and **did not touch this file** — its diff names 23 paths and
`general_session.rs` is not among them.

So the command reports a wall the deployed bytes no longer have, and it reports
it with a `detail` describing bytes that are not on the chain. **This is the
mirror shape `AGENTS.md` warns about**, met from the other side: a hex constant
typed into a checker agrees right up until the tree changes, and then it agrees
with what the tree USED to say. Here it is not a constant but a hardcoded
verdict, which is worse, because it cannot go red.

**Owed, and it is the last thing between this cohort and OpenBatch:** delete the
unconditional push, derive the four caller authorities from the new preimage,
and let the row appear only when a derivation actually disagrees. Then the
executor — `build_general_successor_instruction_v5` into
`compile_general_successor_v0` over a frozen table — is a producer with nothing
in front of it, and `0948b0224` already reports the General-hot suite GREEN
through the real ELFs.

Devnet evidence. Not mainnet evidence.

---

## Addendum B: OPENBATCH REPORTS DELIVERABLE, AND THE FILL IS BLOCKED BY A PROVENANCE ENVELOPE THAT DOES NOT REPRODUCE

Written by the COHORT-15B lane, 2026-09-04, resuming from `HOLD_STATE.md`.
**Devnet evidence. Not mainnet evidence.**

Tree root `/Users/ember/dev/dclutch`.

### B0. The resumption began by discovering its own tools were gone

cohort-15's scratch (`cohort15-1788476653`) no longer exists, and **every script
in the job directory hardcodes a driver path inside it**. The driver was rebuilt
and symlinked back to that path so the scripts run unedited.

It is built at **`e754c6999`**, not at the deploy commit `1cae26fd6`, and the
difference is measured rather than waved at: `git diff 1cae26fd6..e754c6999`
touches four files in the successor tool — `campaign.rs`,
`core_bump_projection.rs`, `infrastructure_succession.rs`, `local_mutable.rs` —
and three in `dclutch-operator`. **None is on the settle, fill, census or
General-session path**, and `general_session.rs` is byte-identical at both
commits.

**A correction to §16 of this document.** It says *"The endpoint credential
exists in no file under it"*. That is false: `sim-config.json` carries the
Helius key in cleartext in `cluster.rpc_url`. The file is mode 600, but the
sentence is wrong, and `build-sim-config.py` is the author — it takes the URL as
`sys.argv[1]` and writes it straight through.

### B1. THE CALLER-AUTHORITY WALL WAS A HARDCODED VERDICT, AND IS NOW A DERIVATION

`e1ae00c81`. §15 item 4 owed this and named it exactly: `general_session.rs`
pushed `session/caller-authority-slot-binding` unconditionally with a `detail`
describing the pre-`3a8ac205d` seed.

The fix is a deletion plus a derivation, through the same two authors
`invoke_admitted_accelerator_v3` calls and in the order it calls them:
`accelerator_caller_authority_digest_v1(Admitted, parent_request_digest, index)`
mints the `role_request_digest`, `CallerAuthoritySeedsV1` places it in the sole
universal release-pinned seed order, and `find_program_address` over Trading
closes it. The wall row now appears **only when a derivation fails**, carrying
the failure's own words.

`admitted_caller_authority_span_v1` takes no slot argument and no bank argument,
because the preimage has neither. If one returns, the file stops compiling
rather than quietly agreeing.

**And the command gained a green it never had.** Its only two exits were a
refusal and a refusal — it was written when the honest answer for four accounts
was *nobody can*. With the walls gone it printed
`[session/unreachable] … 0 unsatisfiable conjunct(s)` and exited non-zero. A
gate whose green is spelled as a refusal is a gate nobody can put in front of a
producer.

#### Measured on cohort-15's General market

```
DELIVERABLE: OpenBatch names no unsatisfiable conjunct at any of the 55
top-level coordinates of market 6aqy89GhhXFtDbawC5ors4HLkGvzdHC4R26TXTaaXRKj
```

| | |
| --- | --- |
| frame report | `$JOB/general/session-derived.json` |
| `walls` | `[]` |
| `rentCredit` | **128** |
| top-level accounts | 55 |
| caller-authority span, derived | `BN91dspGEEdNVfBV2Q3F7CXg8VXXNeAm9Wx33MkLhi14`, `atxHNGRrhTACswHAsVBNtHtHqhMei5HUrNSvrm5JnH8`, `AAfrdEPUepRbkrUmnSFd8RqQruChRrNfUdoYnyjWXcrn`, `D5dmyKB6RRtaFBLfDBttZuWKAjRaFztcRqhVKUS6Hc39` |

Those four are derived against a **stated probe** family-request digest
(`5bc43870c202fcc7d1fa5a27e8afb2e21f41a022f3920faee1a0c2e8a40a581e`, the sha256
of a literal a reader can recompute), because this command signs nothing and the
span is a function of the SIGNED instruction. `--parent-request-digest` takes the
real one when a producer has it; the frame row and the report both say which was
used, so a probe is never presented as a caller's span.

**Both of cohort-14's walls are now closed on one market**: the rent-credit
width by cohort-15's re-founding, the caller authority by `3a8ac205d` plus this.

**What DELIVERABLE does and does not say.** It says every one of the 55
top-level coordinates has a producer and none of them names an unsatisfiable
conjunct. It does NOT say OpenBatch has run, and it does not say every account
in the frame EXISTS — the `capability seal` row still reads *"Still unproduced
ON THIS CHAIN"*, which is a coordinate whose producer exists (`bdce0dc8e`) and
whose account does not. `tools/cohort15/steps.tsv` row `03` asks for three
things and this is the first: the report naming no unsatisfiable conjunct. The
transaction committing, and a `DCLTGBT01` Batch account appearing at the
address the request's own seeds derive, are still owed.

Four tests, and the substantive one is **proven red**: dropping the invocation
ordinal from the preimage — the exact regression that collides one execution's
four authorities — fails
`the_caller_authority_span_moves_with_every_coordinate_that_must_move_it` on
*"each invocation ordinal names its own authority"*; restored, all four pass.
`the_old_slot_bound_seed_moves_between_slots_and_the_new_preimage_does_not`
MODELS the old seed rather than describing it, and asserts the two banks DIFFER
before comparing the addresses they derive.

### B2. RETIREMENT'S CLUSTER STOPS BEING A LITERAL

`e5a42c632`. §15 item 5 named two lines and both are gone:
`ExpectedClusterV1::OwnedLoopback` was a literal in `run()`, and the
acknowledgment handed to `ClusterOriginV1::parse` was a literal `None`.

The idiom is `claims_custody_replay.rs`'s, copied whole: a second
`COMMAND_DEVNET_V1` (`devnet-aggregate-retirement-v1`), a `command(expected)`
selector, a `devnet_usage()`, and the expected cluster threaded. The packets,
the journal, the vault close and the rent arithmetic were always cluster-blind,
so this is a second entry point, not a second implementation.

Four tests; `the_public_retirement_arm_refuses_a_loopback_origin_before_any_key_or_read`
is **proven red** by restoring the literal `None`, and green when the
acknowledgment is threaded again.

**This unblocks retirement on devnet. It does not perform one** — cohort-15's
Direct market is not Terminal at the time of writing.

### B3. THE HONEST SELECTOR, READ OFF THE CHAIN BEFORE THE SETTLE

This is the number §14 says cohort-15 exists to be able to state. It is read
from the captured candidate account's own bytes, not from a plan:

| | |
| --- | --- |
| candidate | `EsdcBPpSQycmvB3AH5kkE5J62XLF7Y3WXwNzKgUjKX5s`, 432 B `DCLTSPC1` |
| snapshot slot / time | 492,775,238 / 1788484422 |
| `publish_time` | **1788484406**, inside `[1788484328, 1788486128]` |
| price mantissa | **10,373,844,866** |
| `source_scale_exponent` | **−8** |
| **SOL/USD** | **$103.738449**, conf ±$0.011551 |
| cuts | `10200`, `10600` over denominator `100` = **$102.00** and **$106.00** |
| **the cell the reading falls in** | **1** — inside `[102.00, 106.00)` |

Cohort-14's markets B and C each paid the cell the deployed program chose rather
than the cell its reading falls in. Cohort-15's statistic carries the factor, so
the committed selector and the cuts-scale selector should be the same cell. Both
numbers are recorded either way — that is how the disagreement was found.

### B4. THE FILL IS BLOCKED, AND THE CAUSE IS THAT A PROVENANCE ENVELOPE DOES NOT REPRODUCE

Everything the fill needs was built. Two portable Direct intent tickets at
**outcome 1**, gross 200 at 50 bps per side, generation 2, `valid_from`
492,808,008 through 493,008,208:

| | |
| --- | --- |
| seller ticket | `b4845a8f860255cfca1de0b9afd782ced808a618700ab7944d2ea1715482b658` |
| buyer ticket | `ec30fbb909cfd4f89005a6e6d44f2a92a7ebf6a7e4898867d9adb3717da995c1` |
| seller Direct token PDA | `F7kfTc16ftcG8TQ6g4CbUiJjwBwqbq7YvpLV5QNJJd2t`, bump 254, VACANT |
| venue fee Direct token PDA | `4ypc38HfytpWRNFbrxWjeNf2Fben6cWPrVG76kp878wV`, bump 252, VACANT |
| `sim-config.json` | rebuilt `with-trade`, `mode: devnet` |

**Outcome 1 was chosen after the capture landed, knowing the reading**, and that
is said out loud rather than left for a reader to infer. Cohort-14 traded
outcome 0, drew cell 1, and paid zero, so the stranger-payout path has never
been exercised; buying the cell a published observation already fixes is what a
rational trader does and is the only way to exercise it.

#### The reconstruction that worked

`checked-execution-release.bin` died with the scratch. It is
`CheckedExecutionReleaseSetV1`: a 16-byte header, the 336-byte
`ExecutionReleaseSetV1`, then five (`ArtifactReleaseV1` ‖ `checked_release_id`)
pairs — 1,592 bytes — and **every input survived**. The release-set body and all
five artifact bodies are `records/*` in `plan-seal.json`, each with its own
`content_sha256`; each `checked_release_id` is the sha256 of that role's
`evidence/<role>/checked.bin`, which `CHECKED_UPGRADE_GATE.json` names in
`links[].checked_manifest.sha256`. The assembly self-checks:
`sha256(release_set_bytes)` = `9895faee8f7f…`, the same identity the plan, the
seal and the Market's own byte 208 state. Result:
`183652fb8ecde03ef931345ad2342f6623accc430301cfdd90642e9b3aff49b9`.

#### The reconstruction that must not be attempted

`devnet-direct-trade-produce-v1` authenticates the plan's whole checked-release
provenance chain, and refuses:

```
claims artifact provenance bytes or SHA-256 changed after checked-release admission
```

A **full rerun of `checked-release-candidate.sh` at `1cae26fd6`** —
`CANDIDATE_EXIT=0` — answers why. It reproduces:

| | |
| --- | ---: |
| role ELFs and checked manifests, byte-identical | **20 of 68 gate-named files** |
| `source-tree.txt`, `build-links.tsv`, `build-diagnostics.txt` | byte-identical |
| everything else | **differs** |

The other 48 — every `provenance/*.json`, `build-*.log`, `frame/*.txt` and
`frame-build-*.log` — carry per-run identity, and so does `build-run.txt`, whose
whole content is `dclutch-sbf-build-run-v1=<per-run nonce>`. That one file is
why the gate digest itself moved:
`195bac3cb0b4970244c34acd243edd0b260b57e46486eb36e437ca840279b09a` against the
deployed `1e614d92c1ec19ed2fcfba4cc255e3d2bbd3c6a955d44a0d3ebdb3ce6f47bbbe`.

**`build-run.txt` alone is recoverable, because the gate states its own
`build_run_id`**: writing
`dclutch-sbf-build-run-v1=4a156cf6a038f1cda1adc7bbcc7314a31648593ccf56558ef27a72ef59932492`
hashes to the pinned `36612048c70bb…`, exactly. The provenance JSONs are not —
substituting that run id into `provenance/claims.json` leaves
`b6728084e1aeb44d…` against the pinned `dd7c4f93a1d7b8bf…`, so more than the run
id varies.

**The 48 were deliberately NOT filled from the new run.** A gate from one build
run plus provenance from another is inconsistent evidence *even when every check
passes*, and the point of the envelope is that it is one run's. The rebuilt work
directory holds only files that hash to the deployed gate's own digests, with a
`README` saying which are missing and why.

So: **every byte this cohort certifies reproduces; the evidence that it was
certified does not.** A cohort whose scratch is deleted keeps the ability to
settle its market and loses the ability to trade it.

**Owed, and it is cheap: the job directory must archive the candidate's
`products/` and provenance envelope at deploy time**, or the gate must pin only
the bytes that reproduce. Nothing else in this document was at risk — the ELFs,
the manifests and the release identity all came back byte-identical from a
second build fifteen commits later.

Because the fill did not happen, no stranger holds a claim, and the
winning-stranger payout of §15 item 1 is downstream of this blocker rather than
of the settle.

### B5. THE SETTLE LANDED ON ATTEMPT ONE, AND THE TWO SELECTORS AGREE

The bounded waiter refused its own first pass — `2376s is past the stated
ceiling of 2000s` — which is the guard working: a forty-minute wait is a
scheduling decision, so it was made explicitly rather than slept through.

| | |
| --- | --- |
| signature | **`2Mqxghzc6Uwtpjndnvhw22QrcH3rbmuBxdSDbxV9ixa6hEwwxgbxbgvyd3qaoCyGjEe1cH6LLSAfWZ4DLyLGk4t7`** |
| slot / CU / fee | 492,829,232 / **140,902** / 75,000 |
| fired at | 03:42:47 UTC, chain clock 1788493361 against a legal-from of 1788493329 |
| attempts | **1** — against the capture's five |

The certificate seat `84YxYUrEYxYyRHAWSnZrNn7D1GeA6teGfvC3hdBfNh9K` went from
**0 bytes, System-owned** to **312 bytes `DCSRCER2`, Resolution-owned**, holding
the same **2,786,520 lamports** it was prepaid with at founding — the walk took
the seat it had been left.

**The verifier, read off the account: the kind byte at offset 10 is `1`**, not
the `4` of `CERTIFICATE_RESOLUTION_FAILURE_KIND`. Cohort-13's was 4.

#### THE TWO SELECTORS, AND THIS IS WHAT COHORT-15 WAS FOUNDED TO BE ABLE TO SAY

| | |
| --- | --- |
| certificate `result_numerator` / `result_denominator` | **10,373,844,866** / 1 |
| `source_scale_exponent`, from the observed publication | **−8** |
| the reading in dollars | **$103.738449** |
| on the cuts' scale (×100) | 10,373.8449 |
| cuts | 10,200 and 10,600 |
| **the cell the reading falls in** | **1** |
| **`CERTIFICATE_V2_SELECTOR_OFFSET` (256), committed** | **1** |
| | **AGREE** |

Cohort-14's markets B and C each paid the cell the deployed program chose rather
than the cell its reading falls in, and market C did it across a position a
stranger had paid for. **Cohort-15 is the first cohort whose committed selector
and whose cuts-scale selector are the same cell**, and `4cd2b9cb5` — the four
reserved bytes that became `source_scale_exponent` — is why. The certificate
carries the raw mantissa with denominator 1; the scale lives in the
`StatisticSpec`, where the founder put it.

### B6. TERMINAL, AND THE CENSUS RETIRES L4 BY NAME

`--action admit-terminal` on the same `terminalSequence 1` input:

| | |
| --- | --- |
| signature | `64jEDVT6ZbdUBxhHnUzeAktSfZY4TunWZcHWLxnxWNoUe8yxYXWVnPmtajBjFcWhLN6m9X2Y5G8d14M6ebuzp5PK` |
| slot / CU / fee | 492,829,917 / **89,942** / 75,000 |

The Core Market's own phase byte at offset 10 reads **`0x02` = Terminal**,
against the `0x01` Open it held before.

**L7 VIOLATED FIRST, AGAIN, AND AGAIN IT LOCATED THE CAUSE.** Declared with only
the two fees:

```
VIOLATED L7: the payer moved -3899136 lamports since `cohort15b-pre-settle` and
its transactions paid 150000 in fees, but watched accounts gained 0; -3749136
lamports are unaccounted for.
```

**3,749,136 is the settle receipt `BYbLHeWUQn5UBhbyPdfVmWWbrZTgqLmqWWeM8RvAm7j1`
— 464 bytes `DCLTSPR1`, Resolution-owned — to the lamport.** Re-declared with it
named, `cohort15b-post-settle-complete` exits zero. Both readings are kept; the
violation is the finding. This is the fourth cohort boundary running where L7
has earned its pass by failing first, and the third where the residue it named
was rent on an account the act itself created.

And **L4 retires by name at Terminal** rather than reading VIOLATED against a
protocol that did exactly what it should:

```
INAPPLICABLE L4: the Market is terminal: settlement DISCHARGED the liability
this law is stated about … L1, L3 and L7 go on watching this boundary
unweakened.
```

### B7. THE MONEY

| | deployer | campaign payer |
| --- | ---: | ---: |
| at the stop the previous lane left | 24.890564434 | 0.628283565 |
| after the settle, admit-terminal and four read-only commands | **24.890564434** | **0.624384429** |

The payer moved **3,899,136 lamports**, and it closes exactly:
**3,749,136** receipt rent **+ 75,000 + 75,000** two fees. Nothing else moved.

**THE DEPLOYER IS STILL AT 24.890564434.** It has moved for its own deploy and
its own ladder and nothing else — now through the seal, two foundings, the
activation, the prepay, the admissions, the capture, the settle and the
Terminal admission.

### B8. RETIREMENT REACHES ITS SUBJECT AND REFUSES BY NAME

With the market Terminal and `devnet-aggregate-retirement-v1` existing, the
first retirement on any chain was attempted. It got past the cluster gate — the
whole point of `e5a42c632` — and refused on evidence:

```
terminal sequence is blocked: the Direct first-use accounts are created together
by the first trade, so campaign evidence must carry all of them or none. It
carries direct_trading_funding_ledger and omits direct_capability_root.
```

`DIRECT_FIRST_USE_LABELS_V1` is `["direct_capability_root",
"direct_trading_funding_ledger"]`, and `evidence.accounts` is built from
`/execution/market/accounts` — 62 entries, which carries
`direct_trading_funding_ledger` and not `direct_capability_root`. **Read off the
chain, the refusal is right about the world and its comment is not:**

| label | address | on chain |
| --- | --- | --- |
| `direct_trading_funding_ledger` | `J4nR8mhJ1dArjXs7hV6D4B8vsHscd64eSfjpoe4hrrgJ` | **120 B `DCLTFL02`**, Trading-owned, 1,570,584 lamports |
| `direct_capability_root` (founding checkpoint) | `8Ya1hee1QrCnjVgmecqQxcYrV2q9jxVMR1aPEo6qtMkB` | **VACANT** |
| the LIVE capability root (activation) | `FUJ9pNukWRb5658ysiDP5gz9gF3Hx8c4cU2mUrvELAWG` | 256 B `DCLTCRT1`, Trading-owned |

**Neither was created by the first trade** — this market has never traded. The
funding ledger was created by the **activation**, which
`capability-activation.json` names as `fundingLedger` with
`ledgerLamportsAfter 1570584`; the live capability root was created by the same
activation, at an address the founding checkpoint's `direct_capability_root`
label does not name. So the all-or-none rule sees exactly one of its pair and
refuses correctly, while the sentence it refuses with describes a trade that
never happens on this route.

That is where this step stops. The question it opens — whether
`DIRECT_FIRST_USE_LABELS_V1` should pair an activation-era ledger with a
founding-checkpoint permit root at all, and which capability root a retirement
is owed — is an evidence-shape decision, not something to guess past on a live
market.

### B9. What is owed after this lane

1. **The fill**, blocked by §B4's envelope. The winning-stranger payout is
   downstream of it, not of the settle.
2. **OpenBatch N=2 on a real chain.** The route now reports DELIVERABLE and this
   command can finally be the gate in front of a producer; the producer itself —
   a driver that builds a `GeneralHotStateV3` through
   `build_general_successor_instruction_v5` into `compile_general_successor_v0`
   — is still unwritten.
3. **The General capability seal.** `bdce0dc8e` gave it a family-neutral
   producer; there is still **no devnet driver command** for it, and the frame's
   `capability seal` row still reads "Still unproduced ON THIS CHAIN".
4. **Retirement**, at §B8's refusal.
5. Route witnesses, and the `docs/reference/route-witnesses.md` devnet class,
   which stood at **22** before this cohort.

Devnet evidence. Not mainnet evidence.

### B10. THE DEVNET WITNESS CLASS STAYS AT 22, AND THE REASON IS A REGISTER GAP

`corroborate.py --discover` over this document, against a fresh
`dclutch-route-census inventory` (12 programs, **162 routes**, 350 refusal
codes) and cohort-15's own program map:

```
corroborate: discovered 0 record(s) from 6 signature(s); 5 carried no
resolvable first-party magic
```

The five it could not credit are not five failures to drive a route. The tool
read each transaction's outer instruction off the chain and resolved its magic
correctly — and then found the census has **no row for that magic at all**:

| signature | magic → program | reason |
| --- | --- | --- |
| `2wqT3rjH…`, `2yRSDsSs…` (the two admissions) | `DCLTPUA1` → trading | no trading route; **0 routes elsewhere** |
| `3VsB7dNp…` (the capture) | `DCLTSPI1` → resolution | no resolution route; **0 routes elsewhere** |
| `2Mqxghzc…` (the settle) | `DCLTSPI1` → resolution | no resolution route; **0 routes elsewhere** |
| `64jEDVT6…` (admit-terminal) | `DCLTCRQ2` → core | no core route; **0 routes elsewhere** |

Grepping the inventory confirms it: **`DCLTPUA1`, `DCLTSPI1`, `DCLTCRQ2` and
`DCLTDFS1` occur zero times in all 162 routes**, while `DCLTPUA1` and
`DCLTSPI1` are declared in
`crates/dclutch-user-position-admission-contract/src/lib.rs` and
`crates/dclutch-resolution-codec/src/sponsored_push_v1.rs`.

So `docs/reference/route-witnesses.md`'s devnet class goes **22 → 22**, and this
cohort contributes **0** — not because these routes were not driven on a public
chain, but because `dclutch-route-census inventory` does not enumerate them, so
the register has no row a devnet transaction could be credited against.

This is the same shape `corroborate.py`'s own header describes — *"a route driven
on a public chain for three cohorts running read as NEVER-EXECUTED, because the
register had no channel a devnet transaction could reach it through"* — one
level deeper. The channel now exists; **four of the magics that channel carries
are not in the register.** `docs/evidence/witnesses/cohort-15-discovered.json`
is kept with zero corroborated routes, because a witness document naming five
finalized signatures and the exact reason each was dropped is the evidence for
that claim.

Devnet evidence. Not mainnet evidence.
