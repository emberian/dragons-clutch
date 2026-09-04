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

---

# ADDENDUM C — COHORT-15C, 2026-09-04

**Devnet evidence. Not mainnet evidence.** Written by the COHORT-15C lane. Every
address, digest, signature and lamport figure below was read off the chain or
off a file this lane produced; the tree root for every source claim is
`/Users/ember/dev/dclutch`.

## C1. THE RE-ADMISSION, and the second wall it found

Addendum B recorded that cohort-15 could settle its market and could not trade
it: `plan-seal.json` pinned `checked_release_gate_sha256 = 1e614d92…`, the
digest of a `CHECKED_UPGRADE_GATE.json` in a scratch directory that no longer
exists, over 48 files carrying per-run identity that no rebuild reproduces.

A fresh candidate was built at the deploy commit `1cae26fd6` from a detached
worktree. **All seven role ELFs came back byte-identical** to the deployed
table, and `elf[45:45+n]` of every live ProgramData account hashes to the same
seven digests — the second verifier of `tools/cohort15/README.md`'s step 05,
`equal ×7`, read off chain.

| | |
| --- | --- |
| candidate | `CANDIDATE_EXIT=0`, `sbf_build_diagnostics_total=0`, `sbf_build_freshness_links=12` |
| checked Upgrade gate | `bb0b8f87b417864e09c6a6c33c66c30412296153a9c1e5af9396cec833dc2f4b` — **moved**, as the reproducibility finding predicts |
| **reproducible gate** | **`52dd6b66f849e78d7cbdd10ed92007886235d6a6f922d3c2a09f97457734f064`** |
| run record | `70f7fed1e23fd6ed2d11d5887342d3760a809c832158890132edfedcf800f9cb` — this run's, not the deployed one's |

**The gate digest was PREDICTED before the run.** The deployed cohort's own
`CHECKED_UPGRADE_GATE.json` (`1e614d92…`, a different run in a deleted
directory) carries every field the reproducible gate keeps, so projecting it
into the reproducible shape gives what a rebuild must produce. That projection
hashed to `52dd6b66f849e78d7cbdd10ed92007886235d6a6f922d3c2a09f97457734f064`
and the rebuilt candidate's `RELEASE_GATE.json` is **byte-identical to it**.
That is the reproducibility claim measured against the ACTUAL deploy run rather
than against a second local build.

The deploy commit predates `9a5332884`, so its own
`checked-release-candidate.sh` emits no reproducible gate and refuses to run
under a newer script (it `cmp`s five tool files against the archived
revision). The gate was therefore emitted by HEAD's `artifact_provenance.py
emit-gate` over the deploy-commit-built root. **The positive control for that**:
the re-emitted `CHECKED_UPGRADE_GATE.json` is byte-identical (`bb0b8f87…` both
ways) to the one the deploy commit's own tool wrote over the same root, and
`git diff 1cae26fd6..HEAD -- tools/release/artifact_provenance.py` removes no
line. The newer tool introduces nothing.

`verify-reproducible-gate` and `select-reproducible-role --role trading` both
pass from `$JOB/candidate-readmit/`, a directory inside the job dir naming no
path under the deleted scratch; a wrong digest refuses with *"reproducible gate
SHA-256 differs"*. `checked-execution-release.bin` was regenerated by its real
producer (`devnet-checked-execution-release-v1`) from the re-admitted
candidate's own five `checked.bin` files and came out **byte-identical** to the
copy addendum B reconstructed by hand — `183652fb8ecde03ef931345ad2342f6623accc430301cfdd90642e9b3aff49b9`.
Addendum B's reconstruction is confirmed by an independent author.

### The second wall, and the fix

`prepare --deployment-set-journal` against the re-pinned deployment set
produced a new plan with the **same `release_set_id` `9895faee…`** the founded
Market carries at byte 208. The produce command then refused:

    REFUSED: Direct campaign and checked plan/Market input digests differ

The founding campaign binds its plan by whole-file digest, and re-admission
necessarily changes that digest. Diffing the two plans leaf by leaf: **exactly
twenty leaves moved, and every one is a candidate path, a gate digest or a
deployment-set journal digest.** Not one program id, release id, record body or
`checked_candidate_elf_sha256`.

`13eda4e48` makes the producer MEASURE that instead of assuming it.
`--founding-plan` and `--expected-founding-plan-sha256` are supplied together
or not at all; the document must be the one the campaign names; and the two
plans must be structurally identical with every leaf equal except a closed
seven-key provenance set, whose members must additionally be absolute paths or
64 lowercase hex on both sides. Three controls, all measured:

* the re-admitted plan offered as its own founding plan → *"supplied founding
  plan is not the one the Direct campaign names"*;
* one half of the pair → *"…are supplied together or not at all"*;
* the founding plan → **the provenance check passes.**

## C2. THE FILL ON THE TERMINAL MARKET IS REFUSED BY PHASE, and that is the result

With the admission repaired, the produce reached the Market itself and refused:

    REFUSED: Direct producer requires the exact finalized Open founding Market
    (owner 7hGerMC6… vs core 7hGerMC6…, executable false, phase Terminal,
     market id 3QytL1bB… vs 3QytL1bB…, release set 9895faee… vs 9895faee…)

Every other conjunct AGREES — owner, market id, release set, non-executable.
The one that differs is `phase Terminal`. The settle at 03:42:47 UTC put the
market past trading, so a fill after Terminal is refused by phase, by name.
**That refusal IS the correct result**, and it doubles as verifier (3) of step
05: reaching it required getting through
`reauthenticate_checked_deployment_set_pin` from a job directory in which no
path under the deleted candidate scratch exists.

## C3. THE THIRD MARKET, AND THE FIRST FEE-BEARING FILL ON A PUBLIC CHAIN

A third cohort-15 Direct market was founded from the **re-admitted** plan, so
its own founding campaign names that plan and needs no re-admission binding.

| | |
| --- | --- |
| Open Market | **`C9dLhWj7yi76RtQhhHV13gKuudAbV8qio8TZVEn3CjAT`** — 368 B `DCLTCOR3`, phase `0x01` Open, readiness `0x02` |
| `selected_release_set` @208 | `9895faee8f7f6a1926df18302f1b003afcf4b6c56518ba7bba2614c86eea8a22` |
| collateral mint / wallet | `GZE37P8vKK8kvQMq6AKAUEvUhLoug13ejq3eHU2RYvY` / `6JSZSmfoq2dSTBc3YmWTvJ9RyR73CRVNK87MzmKM66YB` |
| Claims aggregate / Hoard | `7idrFHE2q6Mcm775tnqK3JptAAt5Cp4TreAtWBA7cJNe` / `DL88yhc1KHJ5ioqw6TFupMWYFpXoGxLNSCiELcaGexEC` |
| founding | **83 campaign transactions**, one run, no resume |
| DCLTGMF3 routing table | `8mcPQNt5eCid5wr8f34yofWty2sx55q5kZoeg6fF6iQ8` (recovered from the ALT-create transaction `5xo8y7AV…`, account key index 1) |
| activation | verdict **`ACTIVATED`**, root `Ap9eCmjuqDzuFqPTgL8ApJKuzUVPqTsZf96HQooMso3m`, generation 2, entry 0, deadline slot 493,072,207 |
| window | start 1788499895, width 1,800 s, max_age 7,200 s — **the capture and the settle are OWED** |
| cuts | `10200,10600` over `100`, anchor **10397**, from SOL/USD `$103.972224` (conf ±`$0.009593`, 67 s old) read off `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` immediately before staging |

**The stranger admission**, participant-2 `92RrSCo9…`:

| | |
| --- | --- |
| Position | `FSUxksBxHWAk2dkPp8JHfNnembYjQmQdn5VXexiRuQ9y`, 160 B `DCLLBP02` |
| admission | `2CabHYSLTdKeaAqqagPB42QkH7PGX8UmYE1yL3o19EgGZkssACMBAvuKBM9PdJU6eFgHnQ5zm1EcXMydfUx7SazG`, slot 492,863,447 |
| collateral, 201 atoms | `62dXCX7GJb8i9ui94sRVR9apS9cNgSgPho9rwJ8hJdPgJTBqvVG3ezm91oTzTaiFjzN9ARN8Eh3PMRiRkKJbxgPs`, slot 492,863,516 |

**`0x4003` AT 12,231 CU WITH NO CPI, A THIRD TIME.** The first `--execute`
refused exactly as `admit.sh`'s own header warns: a plan-then-execute pair
RESUMES the durable `Planned` journal, whose System top-ups are inline, so the
Position owner is debited, a debited account is writable, and
`UserPositionAdmissionFrameV1` requires the owner to sign READONLY. Cohort-12
measured 12,233 CU; this is 12,231. `2613e7d6` was believed to have repaired
it. **It is not repaired for the bare command** — only for the simulator path
that carries the warning. Re-running with no prior plan file admitted on the
first attempt.

### The tickets, and the seller's Direct token PDA derived with a control

Outcome **1**, gross 200, 50 bps/side, generation 2, valid 492,863,000 →
493,063,000. Seller is the founder `5wjmqJAL…`; buyer is the stranger.

The seller's collateral is a Trading PDA over
`["dclutch:direct-token:v1", market, generation_le, owner, role]`. The
derivation was **checked against market 1 first**: it reproduces
`F7kfTc16ftcG8TQ6g4CbUiJjwBwqbq7YvpLV5QNJJd2t` at bump 254, the address
addendum B recorded. For this market it gives
`7imXqzwBBGQq6bbZAf8Y6g5nRV9hT4bG59SXYmynXdYV` at bump 255.

    seller ticket sha256  6bfa97c5ab55af4377cf65d33ab63a7406ddb334c5aadfa1f534d6f70e491932
    buyer  ticket sha256  181c56bea8ad10999ef0ee2862caaa61357cc2c40d48de85c8a36f06d6a53ed6

### THE FILL

The Direct ladder ran replay-setup → token-setup → lookup create/extend×3/
freeze/activation → capability-seal → **Hot**, ten durable stages, no refusal.

| stage | signature | slot | CU |
| --- | --- | ---: | ---: |
| lookup | `3aRzrSMy5c9tN72AQHTUqLhyZSNRHtQ1SGwpwJXv9fChCnMCe5ZnFHgYBvSgd1qxJstz8nFCNQaFAhe7YkHPBP4M` | 492,864,928 | 1,517 |
| capability seal | `5wPF5SbftvtBotLRP34Lupp2WfC8dLcHMCSssHi2FAeR94vDg5fZUmBfMoHLqivW3D1MQSMN3KEgYgPG2cKGFVeL` | 492,865,103 | 722,044 |
| **the fill** | **`3eKAiD9T13wgqeQBiVBpnGgrTo65N7Ck2gcyNGmdz41HEcKVFP6xTTK7LLsKxo7rR6z51GVgfdXAWWxcSukMe5qw`** | 492,865,197 | **1,137,522** |

**1,137,522 CU against cohort-14C's 1,281,582** — 144,060 lower, and against
the 1,400,000 ceiling that cohort-11 believed a fee-bearing fill could not fit
under. Fee 15,000 lamports.

### THE PERMISSIONLESS `DCLTDFS1` SETTLEMENT, on a public chain

`devnet-direct-fee-settlement-v1` — *"the only caller of the DCLTDFS1 route
outside a program test"* — executed:

| | |
| --- | --- |
| signature | `5yVEK542AE4oStiHD3yuRSnBgDtkrvn16GTmQweEipF2a9MSjJF2AkoTmJzK3wqJMAvUw3frWwjf4wGGukhjpu11` |
| slot / CU / fee | 492,865,496 / **95,583** / 75,000 |
| custody revision | 2 → 3 |
| **`fee_owed` after** | **0, read back from chain** |

The gauntlet's program-test measured 173,721 CU for this route; on the real
chain it is 95,583.

### The atoms close, exactly

    buyer collateral   78BT2zDigNyGegoLsYC54Zix8n1sALsqVAYtF4yPKSJe   201 -> 0
    seller token PDA   7imXqzwBBGQq6bbZAf8Y6g5nRV9hT4bG59SXYmynXdYV     0 -> 199   (gross 200 less seller fee 1)
    fee destination    GoEvhmG4K2dRUufppzrLjNiaNf1UHGPFA1i2h8npdjfp     0 -> 2     (seller fee 1 + buyer fee 1)

199 + 2 = 201, to the atom.

## C4. RETIREMENT ADVANCED FROM ITS EVIDENCE WALL TO ITS SEQUENCE WALL

Addendum B recorded retirement refusing on evidence it expected wrongly. The
chain says both halves of that expectation are wrong, and the transactions say
so by name:

* `direct_trading_funding_ledger` `J4nR8mhJ…` was created by the **FOUNDING**
  campaign at slot 492,763,282 — `3brogqWJ…`, top-level program Trading, its
  lamports 0 → 4,002,456. The first trade did not create it; this market has
  never traded.
* the **ACTIVATION** at slot 492,765,919 — `zrLHfHLX…`, top-level program Core
  — debited that ledger to 1,570,584 and created the execution capability root
  `FUJ9pNuk…` in the same transaction.

And `direct_capability_root` names two addresses: the founding checkpoint's
founding-permit root, at which no account can ever exist, and the execution
root the terminal sequence means. Only an evidence refresh emits the second
under that label. `f86b1df78` gives `devnet-aggregate-retirement-v1` the
`--refreshed-evidence` input `terminal_sequence.rs` has taken for exactly that
reason, with the same `effective_accounts_v1` merge against a finalized slot.

`devnet-refresh-evidence-v1` produced a refresh at finalized slot 492,859,008
carrying `direct_execution_capability_root FUJ9pNuk…` and
`founding_permit_capability_root 8Ya1hee…` as separate fields. Retirement then
walked past the evidence gate and refused further in:

    Error("checkpoint AggregateRetirement: Market")

`MarketRetirementOperatorErrorV1::Market`, from
`crates/dclutch-market-retirement-v1-operator/src/lib.rs:751`. It is ONE code
over ten conjuncts; nine of them hold and the tenth is
`market.phase != Phase::Retiring` — the market reads `0x02` Terminal. **That is
a sequence precondition, not a defect**: `terminal_sequence`'s CoreBeginRetiring
and DirectBeginRetiring stages are what move a Terminal market into Retiring,
and they have not been run. Retirement's evidence expectations are corrected;
what it now waits on is the stage before it.

## C5. THE CENSUS ROWS: three of four, and the fourth is a different defect

`4b2519c3a`. The four magics were never missing ROUTES — they were routes
carrying no BYTES, because a guard written `if is_x(data)` produced a
`Predicate` selector naming the recogniser and nothing else. Reading the
discriminant out of the predicate's body, one hop deep:

| | before | after |
| --- | ---: | ---: |
| magic selectors | 46 | **76** |
| distinct magic values | 40 | **64** |
| length selectors | 34 | **39** |
| routes | 162 | 162 |
| refusal codes | 350 | 350 |

`DCLTPUA1` → `trading/user_position_admission_v1::process_user_position_admission_v1`,
`DCLTSPI1` → `resolution/sponsored_push_v1::process_sponsored_push_v1`,
`DCLTDFS1` → `trading/direct_fee_settlement_v1::process_direct_fee_settlement_v1`,
and as a side effect `DCLTHOT3` reaches `trading/hot_v3::process_hot_execution_v3`
— Trading previously reported zero magics across its whole top-level surface.
`inventory --check-unique` stays green; 81 census tests pass.

**`DCLTCRQ2` is NOT fixed, and it is a different defect.** It is declared as a
bare `REQUEST_MAGIC` in three crates at once — `dclutch-market-core-codec`
(`DCLTCRQ2`), `dclutch-dealer-codec` (`DCDREQ01`), `dclutch-general-codec`
(`DCGREQ01`) — so `ConstantIndex::resolve` refuses it as a collision. Core
never names the constant anyway: it checks the bytes inside `Request::decode`,
which the walk treats as terminal. The fix is a rename of a Lean-emitted
constant (`formal/dclutch-semantics/EmitMarketCoreRust.lean:1594`) plus hoisting
the check into the dispatch guard, in the shape
`programs/dclutch-core-sbf/src/lib.rs:495` already uses for `DCLTCSR1`. Owed.

## C6. THE CREDENTIAL SENTENCE IS TRUE AGAIN

§16 said *"The endpoint credential exists in no file under it"*; addendum B
found that false. The key was then scrubbed to a `<HELIUS_KEY_REDACTED>`
placeholder, which left `sim-config.json` both credential-free and
**unloadable**, because `3b31e6f7b` made the simulator refuse any stored
endpoint carrying an `api-key` parameter and resolve the key at use time.
Closed at the source: both configs now store `https://devnet.helius-rpc.com/`
with no query string, and `build-sim-config.py` gained `credential_free()`, so
a rebuild cannot reintroduce the key whatever URL the caller passes.

## C7. BALANCES AND COSTS

    deployer  24.890564434 -> 23.890559434   (one 1 SOL transfer to the campaign
                                              payer, 5,000 lamports of fee; it has
                                              still moved for nothing else)
    payer      0.624384429 -> 1.399802515    (funded 1.0; spent 0.224581914 on the
                                              third market's founding, activation,
                                              admission, ten-stage fill and fee
                                              settlement)

The whole third market — founding, activation, one stranger admission, the
fee-bearing fill and its settlement — cost **0.224581914 SOL**, inside the
0.5 SOL bound.

## C8. STILL OWED after this lane

1. **Market 3's capture, settle and payout.** Window start 1788499895; the
   capture must fire inside `[1788499895, 1788501695]`; the settle is legal
   strictly after **1788508895** (= end + max_age). Then admit-terminal,
   custody replay, the ATA and the winning stranger's payout. The buyer holds
   outcome 1 and the cuts straddle the reading, so this is the first market
   whose stranger stands to be paid.
2. **Market 1's terminal sequence** (CoreBeginRetiring → DirectBeginRetiring →
   payout), then retirement, which is now blocked only by the phase.
3. **OpenBatch on a real chain.** The seam is now exactly located and is one
   missing document: the only host path to
   `build_general_successor_instruction_v5` is
   `parse_route_v1` → `snapshot_addresses` → one finalized `getMultipleAccounts`
   → `acquire_route_v1` (which builds the `GeneralHotStateV3`) →
   `compile_general_successor_v0` → `serialize_plan_v5`, exposed as
   `general-successor-plan-v5`. **Nothing in the tree writes the
   `GeneralSuccessorRouteV1` that command consumes.** `devnet-general-session`
   derives and reports the very same frame — DELIVERABLE, walls `[]`, at all 55
   coordinates — but emits a producer report rather than a route. Emitting a
   route from that session is the whole of the missing producer.
4. **The General capability seal** through `capability_seal_instruction_v1`
   (`crates/dclutch-operator/src/capability_seal_v1.rs:100`, from `bdce0dc8e`).
   Its only caller tree-wide is the program-test at
   `programs/dclutch-trading-sbf/program-test/general-hot/tests/open_batch.rs:1056`;
   it has no host caller and no devnet driver. One transaction plus rent.
5. `DCLTCRQ2`'s census row, per C5.

## C9. MARKET 3'S CAPTURE LANDED, INSIDE ITS WINDOW, ON ATTEMPT ONE

| | |
| --- | --- |
| signature | `2p5urmVAjSDrfjZUz9jL7yChRVFd3tYaHTTejvs4bH7jLNjBe5h5vKXbM86sbXjpUT2gW4HPNXzfZkv5zZ7AKxtX` |
| slot / CU / fee | 492,868,986 / **103,810** / 75,000 |
| fired at | 05:32:44 UTC, **1,731 s of window left** |
| candidate | `Aeh8S9PuMtZG2tTjbBDtAMNx3GKpA1sYySDXGQWfQ8Q9` — 432 B `DCLTSPC1`, 3,546,480 lamports |
| head | `ByGGsDxatVMwLcsegUqo2oACkMhfhytpRquNoiXGAo3T` — 2,938,512 lamports |
| **the verifier** | the candidate's own snapshot seconds, read off the account: **1788499916, 1788499917, 1788499969**, all inside `[1788499895, 1788501695]` |

The window was read off the market's own 112-byte `DCLTWIN1` record
`APAqeQu3dZ22F1pwbs1ddtLcfrCdSF8DrbbeV3uABNdb` — start 1788499895, end
1788501695, max_age 7,200 — never from a handoff table.

**ONE ATTEMPT, against market 1's five.** The difference is not luck and not
margin: market 1's `input-capture.json` was assembled by hand, and this one was
authored by `devnet-sponsored-push-input-v1` — the producer that `sponsored_push.rs`
records as having been written precisely because *"the consumer was written,
shipped, exercised and used to resolve a devnet market, and until this function
nothing in the tree WROTE its input."* Its first devnet use is this capture, and
the retry ladder it fed did not need a second pass.

**The settle for market 3 is legal strictly after 1788501695 + 7200 =
`1788508895` = 2026-09-04 06:01:35 UTC**, and the market stays settleable after
that. The verifier is the certificate's kind byte at offset 10: **1**.

## C10. ROUTE WITNESSES: 22 → 26

`corroborate.py --discover` over this document resolves **nine** records where
cohort-15's committed witness document had zero, because C5 put the bytes on
the predicate-selected rows: both stranger admissions and market 3's
(`DCLTPUA1`), the two captures and the settle (`DCLTSPI1`), the capability
seal, the fee-bearing fill (`DCLTHOT3`) and the permissionless fee settlement
(`DCLTDFS1`). `corroborate.py --check` re-reads every signature in all four
witness documents from devnet: **42 distinct routes, 0 problems**.

A `genref --converge` pass in a detached worktree at the commit that landed the
witness document moved `docs/reference/route-witnesses.md` from **devnet 22 to
devnet 26**, and *"a real Agave runtime drives 54 of the 162"* to **58**. That
regeneration is deliberately not committed by this lane: HEAD moved five
commits while it ran, another lane was regenerating the same references, and
`AGENTS.md` gives `genref` to the convergence owner. The witness document is in
the tree and the next convergence picks the rows up. The number, measured, is
**22 → 26**.

The one signature still dropped is `DCLTCRQ2` to core, per C5.

---

# ADDENDUM D — OpenBatch's missing documents, written; and the field that stops it

Devnet evidence. Not mainnet evidence. Written by the COHORT-15D lane,
2026-09-04. Every measurement below was taken against tree root
`/Users/ember/dev/dclutch`.

## D1. The seam was three missing producers, not one

Addendum C recorded OpenBatch's seam as *"one missing DOCUMENT"*: nothing wrote
the `GeneralSuccessorRouteV1` that `general-successor-plan-v5` consumes. That was
true and it was the first of three. Running the route through to a chain found
the other two, and each was invisible for the same reason: a reader, a schema
and a refusal all built, and only the failure path ever exercised.

| owed | what it was | now |
| --- | --- | --- |
| the route | `devnet-general-session` derived the frame and emitted a report | `--emit-route` |
| the table | `compile_general_hot_v0` requires an exact address set nothing could compute | `devnet-general-lookup-table-v1` |
| the signer | two commands produce a plan document; nothing signed one | `devnet-general-successor-execute-v1` |

## D2. The route, and why its three flags are flags

`--emit-route` serializes the derivation the frame report is already made of, so
the report and the route cannot disagree about a frame. Three coordinates the
frame cannot observe are arguments, all-or-nothing, with a refusal that names
the missing one:

* `--lookup-table` — a caller-owned account whose address set is a function of a
  compiled instruction, and this command compiles nothing;
* `--rent-credit` — the frame report's own row for it reads *"nothing on chain
  names it"*;
* `--checked-release` — and this one is authenticated rather than transcribed.
  `build_general_successor_instruction_v5` never re-derives the checked-manifest
  digest, so a route could carry any nonzero 32 bytes and the producer would copy
  them into its plan. What the chain does state is the release set the Market
  selected, so the manifest's own `execution_release_set_id` must equal the
  Market's `selected_release_set` before the digest is stated at all.

The batch state PDA is derived, never supplied: `GeneralBatchOccurrenceTermsV1`
over the root's own sequence, generation and Market, the config record's price
scale and order bound, and the Product graph's outcome count and record identity,
then `GeneralStateAddressSeedsV3::batch` for the seed order — the same two types
the operator calls. The runtime suffix's privileges are read off the published
AccountProfile through `physical_account_geometry_with_dynamic_spans`, which is
the function `validate_runtime_geometry` compares them against.

The route is emitted only on the DELIVERABLE path. A gate that also emits its
subject on the refusing path is not a gate. And the producer closes its own loop
before writing: the document is fed back through `parse_route_v1`.

**The first `GeneralSuccessorRouteV1` ever written**, market
`6aqy89GhhXFtDbawC5ors4HLkGvzdHC4R26TXTaaXRKj`:

    format          dclutch/general-successor-route/v1
    action          open-batch
    accounts        39 fixed + 12 strategy + 4 runtime suffix, 56 to reacquire
    batch state     6Bai5tHS5enG1BN91k3ntQFcvgtJkkFXHXswg5t42kTR  (derived, bump 254)
    runtime suffix  6Bai5tHS…  signer=false writable=true
                    D5qe7ZoQ…  signer=true  writable=true
                    64mXYRdx…  signer=false writable=true
                    1111…1111  signer=false writable=false

## D3. Two walls the first production run found, and neither was findable without one

**The route grammar could not state the System program.** `parse_route_v1`
refused every account address equal to the all-zero key, and on Solana the System
program IS the all-zero key. Every General AccountProfile declares a
System-program runtime coordinate, so no General route of any action could be
parsed. The producer's self-check caught it before a file existed:
`runtimeSuffixAccounts[3].address is not a nonzero canonical public key`. The
guard that matters for an ACCOUNT is the canonical base58 round trip; nonzero is
a guard for content IDENTITIES, where the zero value is reserved. The runtime
suffix admits it now; the fixed frame, the strategy accounts, the payer and the
lookup table still do not.

**A real route always carries nineteen vacant staging cursors, and the plan's
snapshot refused the first.** `finalized_observed_accounts` treats a null
`getMultipleAccounts` answer as a missing observation — right for its twenty
other callers, wrong for a frame that deliberately names what a closed
publication ladder leaves behind. Measured: `finalized observation missing
MUKgLFXeGK8tCCRzjTZMiEXS5WSvwzZFW8XjQD7X6qz`, the manifest staging cursor, three
accounts into a fifty-six account snapshot. `general_session::finalized_frame_v1`
already knew this, in one function for one command; that synthesis is now
`observed_or_vacant_v1` with one author and both callers.

## D4. THE GENERAL CAPABILITY SEAL, on a real chain

Fixed coordinate 38's author row has read *"PRODUCIBLE, AND THE PRODUCER EXISTS
… Still unproduced ON THIS CHAIN"* since 2026-09-03.
`capability_seal_instruction_v1`'s only caller tree-wide was one program-test.

`devnet-capability-seal-v1` consumes the frame report rather than deriving a
second frame, and the transcription is safe for one specific reason: the builder
DERIVES the seal address from the four seeds and refuses `SealCoordinate` when
the frame names a different one. It refused twice before it landed, and both
refusals were the caller's:

1. `legacy transaction is 1566 bytes, above the 1,232-byte packet ceiling`. The
   seal frame is 41 accounts — the whole common Hot fixed frame plus a payer, by
   construction, for every family — so this route has never been reachable
   without v0 routing.
2. `0x4008 TradingSbfError::HeapFrame`, 24,612 CU of the 1,399,700 it asked for
   and none of the heap it had not. `declares_extended_heap_profile_v1` lists
   `DCLTSEL1` so a grant is ADMISSIBLE and never automatic, and the adapter's own
   comment names the shape: *"the right shape for a caller who forgot, rather
   than an unnamed abort."*

| | |
| --- | --- |
| seal | **`F8U3JsvigjFGX1Pynx1bBao2i1k7nAnJDWc7b7gwZUmr`**, bump 255 |
| signature | `2rXCJ2ieKZuimXqn9eREfNSNwhUx5K2owZ91mTywKnrB2LQvHjMVX84ZS1pdG3Gfyqkr7eqPVeKjiLDwCc5GppQs` |
| slot / CU | 492,886,343 / **225,141** |
| read back | 968 bytes, owner `3gBSSjYwSC4phutpGKRkMhrnCDVzHu6kfQ3L4jLf2UmG`, 6,940,968 lamports |
| routing table | `FEdxo6sMN4gHVSTzdiGdsttnStP9nYP1QMC5UeC9QYAQ`, 4 transactions |

The builder derived that address from the four seeds; the frame report had been
stating it at coordinate 38 since the day before. Two authors, one address,
neither told by the other.

## D5. The first General Hot routing table, and the first plan document

    GENERAL-HOT   GTZD8BonxAiUFx8D3cx7pnJbFG4UpUWRT7WJhxHDP5YV
                  53 addresses, create + 3 extends + freeze, frozen and read back
                  slots 492,887,555 / 595 / 634 / 675 / 715

`compile_general_hot_v0` requires `table.addresses ==
canonical_general_lookup_addresses_v3(instruction, payer)` byte for byte. Nothing
in this tree creates a table over a General Hot frame — `publish_routing_table`
serves foundings, activations and Direct fills, and the General family's only
table was `GENERAL-ACT`, whose set is the ACTIVATION instruction's.
`canonical_lookup_addresses_v1` computes the set by doing everything
`produce_plan_v5` does except the compilation that would refuse.

The first `dclutch/general-successor-plan/v5` produced from a real chain:

    observedSlot              492,887,872
    outcomeCount              4
    admittedInvocationCount   4
    heapFrameBytes            65,536
    requiredSigners           1  (D5qe7ZoQ…)
    lifecycle.primary         6Bai5tHS…  coordinate 5, bump 254, isMaterialized false
    childRoutes               0
    familyRequestDigest       e6c1f43d5d935244567496edcacb861dc4492c3421de5404480e9d849b077332
    rootPrestateDigest        b1809a57df13d64c9dcfe1e54758aae2887a09dabe45bfd53f561831f6338ad0

## D6. THE FINDING: one field, and it is a founding input

The first General OpenBatch simulation on a real chain:

    units 128,724, err {"InstructionError":[1,{"Custom":16405}]}

`16405` is `0x4015`, `TradingSbfError::DescriptorManifestEntry`. That code covers
five conjuncts of `CapabilityProgramV4::validate_selection`, and
`devnet-general-session` held every input that function takes and had never asked
it — so a market whose descriptor does not bind its manifest entry reported
DELIVERABLE at all 55 coordinates. It asks now, through the same function, and
names each side rather than restating the code:

| conjunct | descriptor | entry / selection | |
| --- | --- | --- | --- |
| entry release vs root selection | — | `7c0457ce…` = `7c0457ce…` | equal |
| entry config vs root selection | — | `2438edf4…` = `2438edf4…` | equal |
| capacityProfile | `99cb433a…` | `99cb433a…` | equal |
| rootSchema | `b94537e8…` | `b94537e8…` | equal |
| **derivationPolicy** | **`68b513717dce78482c1fd6a56e81f5f07e8d41b5c60bea7750718e55379850f6`** | **`7fe9b22d9897e44ab02cdd9f5aaf85dbca59a02c27143350b243d66535c184c1`** | **NOT EQUAL** |

Cohort-15's General OpenBatch descriptor's `derivation_policy` is not its
manifest entry's `child_derivation_id`. The ACTIVATION descriptor's was, which is
exactly why this market activated and why this action cannot execute. It is a
compile-time property of the founding, fixed by re-founding and by no producer —
which is why the gate now says so before it emits a route, rather than 128,724 CU
into a simulation.

**OpenBatch N=2 on a real chain is therefore still owed, and its remaining
blocker is a market, not a route.** Every producer between the frame and the
chain now exists and every one of them has been exercised against devnet.

## D7. The terminal lifecycle, driven; and two coarse codes that are one wall

Market 1 (`3QytL1bB…`) was Terminal and had never been retired. Driving it named
its own order, one refusal at a time:

    custody replay -> ATA -> a wallet payout for EVERY claim index
      -> core-begin-retiring -> direct-begin-retiring
      -> resolution-receipt-prepay -> Resolution CloseFund

`BeginRetiring is blocked: Claims supply at index N is 500000000; produce and
execute wallet terminal payouts first` names each index in turn; market 1 needed
four. Three paid **zero** and one paid **500,000,000** — the certificate selected
cell 1, and the payouts agree with it:

| index | signature | payout | CU |
| ---: | --- | ---: | ---: |
| 0 | `2R3e5YoZa44HhgekfRA3aiAe3VAnKLxiPfsFguvbo58mDYbd46r6TJqwSezxPSdPG8DSrVtSiapkbgNMidgL9cDT` | **0** | 165,591 |
| **1** | `5ktVXiodjkHt5VkJKycTr9c4wqTCfMKyHwxyPym8Br9hhmbx5b7MQMyMQjdCRBaTrbMGqRzKAU9LmVLoHDynxLue` | **500,000,000** | 235,003 |
| 2 | `3LLhLeiuPRX3W74j7FMowXtZ4w6vQyCFoE68aMwrzGVumQuoKZmrXX7KcMzykFHUM39fBGrPUaebYAYwuA23ZGDq` | 0 | — |
| 3 | `MNFkHGPeXoijgYD6dxz3RRjxLqNmo6JaWJ19RCkk6tQDezCzDmepJQAJMtgWomU2UAZtkfcKaAdJrCZhn8YdPk7` | 0 | — |

`core-begin-retiring` and `direct-begin-retiring` then landed and the Market's
phase byte at offset 10 reads **3, Retiring**, off the chain. That is
`direct_begin_retiring_v1` driven on a public cluster for the first time.

### The Clock is not a prestate

`resolution-receipt-prepay` refused eight consecutive plan-then-execute passes
with `terminal finalized prestate changed after durable planning`. Diffing the
eight preserved plans account by account: exactly one moved across all of them,
`SysvarC1ock11111111111111111111111111111111`, observation slot 492,898,149 to
492,900,125, everything else byte-identical. The Clock is in that stage's frame,
so it lands in the durable prestate map, and its bytes change every slot by
construction — no plan can bind it, and the stage was structurally unreachable.
Presence is still required; the equality is not. The stage advanced immediately
after.

### `Resolution CloseFund: Funding`, and why retirement is not a second wall

The sequence then stops at `ResolutionCoreOperatorErrorV3::Funding` — a code
whose own documentation covers *"funding state, manifest binding, or physical
custody"* and which is raised from two `_ =>` catch-alls
(`resolution-core-v3-operator/src/lib.rs:2558`, `:3456`). It is the idiom
`AGENTS.md` names as the most expensive in this tree, and localizing it needs the
inner distinction surfaced, not a retry.

`devnet-aggregate-retirement-v1` refuses separately with `checkpoint
AggregateRetirement: Market`, which is another ten-site coarse code. **They are
one wall.** Read off the chain rather than inferred: market 1's
`outstanding_capabilities` at CoreState offset 280 is **1**, and the conjunction
at `market-retirement-v1-operator/src/lib.rs:751` requires **0**. The capability
is retired by the rest of the terminal sequence. Retirement is downstream of
CloseFund, and CloseFund is the whole remedy.

## D8. The OpenBatch execute path, and where the refusal sits

`devnet-general-successor-execute-v1 --execute` was run against the same route.
It produces the plan, signs the exact bytes the plan published, and submits with
preflight on — so it never became a block entry, and the runtime's answer is the
same one the simulator gave:

    Error processing Instruction 1: custom program error: 0x4015
    Program 3gBSSjYwSC4phutpGKRkMhrnCDVzHu6kfQ3L4jLf2UmG
      consumed 128,566 of 202,850 compute units
    unitsConsumed 128,724

**The accelerator is never invoked.** The logs carry ComputeBudget and Trading
and nothing else: Trading refuses in its own prologue, in
`reauthenticate_top_level_root_roles_v3`, before any CPI. So the accelerator's
single ack, the chunk count and the `DCLTGBT01` Batch account are all downstream
of a conjunct that is not about them — and reporting them as "owed" without
saying that would put the reader two hops from the cause.

Landing this refusal as a block entry rather than a preflight rejection would
require `skipPreflight`, which is the deliberate posture for hostile evidence and
not the default for an honest act. It is not taken here: the same code, the same
instruction index and the same compute figure come back from both the simulator
and the sender, and a third statement of one refusal is not a third fact.

## D9. MARKET 3'S SETTLE: refused six times, convicted to one absent account, then landed

The ladder fired exactly when the market's own record said it could — chain clock
1788508927 against `legal_from` 1788508896, verdict `due`, read off `DCLTWIN1`
`APAqeQu3…` as the last statement before the action. Then six attempts refused
identically:

    Error processing Instruction 2: custom program error: 0x8002
    Program 24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn
      consumed 135,522 of 1,399,700 compute units

`0x8002` is `ResolutionError::OutputState` — *"A writable Source state or
certificate account was not canonical."* Two accounts can carry that accusation,
so both were read rather than guessed. The settle's writable frame past the payer
is three accounts, and the plan the ladder itself wrote names them:

| | address | observed |
| --- | --- | --- |
| Source state | `68oH466LZ7cdnwKBLg6M24LmJ75oytCW77WS86Vyq6B3` | 224 B `DCLTSRS2`, Resolution-owned — canonical |
| **certificate seat** | **`9BnUF5rKx2WNbvvexQd3f7CgJzUBEBEwBn8ZvwK62gSu`** | **ABSENT** |
| settle receipt | `EGTGB7Vtu2Y49im4HNMfXAmjCsqhD16qbsyi2bAQrMHV` | absent, and correctly so — the settle creates it |

**Market 3's founding never prepaid its certificate seat.** Market 1's was, at
founding, with 2,786,520 lamports and 450 CU (`2tV9iqra…`) — a plain System
transfer — and this document records it under "THE DIRECT MARKET". Market 3 was
founded from the re-admitted plan in 83 transactions and that transfer is not
among them. The seat is not derivable-into-existence by the settle: it must
arrive prepaid, System-owned and unallocated, and an absent account is the one
shape `OutputState` names.

Prepaying it to exactly market 1's figure (`2w2DdvCVWcPoKdkoApr7EZsGYyxtWWhogbY5tjoB6NC4FynqdfC96NySHo8mn1NFsX9DMFR75Yi2TM7kcfqh8dQn`)
and re-running the same ladder landed the settle on **attempt one**:

| | |
| --- | --- |
| signature | `25Cxq4WmJKZtj4pKyRU96qZKaXPnMtrR9wfA7Yd2o13cwNZyR4Qr1CSjECdMqkCWtYH4kBd1rjVChRZYTEune2ec` |
| slot / CU / fee | 492,925,112 / **146,902** / 75,000 |
| certificate | 0 B System-owned → **312 B `DCSRCER2`**, Resolution-owned, same 2,786,520 lamports |
| **kind byte @10** | **1** — not the 4 of `CERTIFICATE_RESOLUTION_FAILURE_KIND` |

One attempt against market 1's one, and the difference between six refusals and
one success was a single 5,000-lamport transfer. Note also that 2,786,520 is
*above* this cluster's current rent minimum for 312 bytes — the Rent sysvar reads
`lamports_per_byte_year 5080`, `exemption_threshold 1.0`, so `(128+312)×5080 =
2,235,200`. The conjunct is rent-exemption, not an exact figure; what failed was
absence.

### THE TWO SELECTORS AGREE, AND THIS TIME THE STRANGER STANDS TO BE PAID

Committed selector at certificate offset 256: **1**. The observation staged with
this market was SOL/USD `$103.972224`, which at the statistic's own
`source_scale_exponent = -8` is 10,397.2224 on the cuts' ×100 scale, against cuts
10,200 / 10,600 — **cell 1**. Same cell both ways. The buyer holds outcome 1.

## D10. Admit-terminal refuses, and the two markets' ledgers are equivalent

    Core terminal admission builder: Funding
    active-funding-ledger refused: native custody arithmetic

The diagnostic is the operator's own, printed beside the coarse code:
`authenticate_active_funding_ledger` in
`resolution-core-v3-operator/src/lib.rs:3470`, refusing at its last conjunct,
`authenticated.validate_native_custody(account.lamports,
rent.minimum_balance(account.data.len()), allow_lamport_surplus)` (`:3513-3517`).

Market 1's admit-terminal passed that same check at 03:42 UTC, so the two ledgers
were measured against each other rather than reasoned about:

| | market 1 `9xQHh4n6…` | market 3 `5Sa5WXPp…` |
| --- | ---: | ---: |
| width | 264 B `DCLTFL02` | 264 B `DCLTFL02` |
| lamports | 2,482,539 | 2,482,539 |
| rent minimum | 1,991,360 | 1,991,360 |
| surplus | 491,179 | 491,179 |

Byte-diffing them: they differ in exactly four ranges — the 32-byte identity at
`[16:48]` and three 3-byte fields at 56, 128 and 200, one per funding entry.
Every quantity this conjunct reads is equal. So the refusal is not explained by
the ledger's own lamports, and the next question is whether
`allow_lamport_surplus` differs between the settle path and the admission path,
or whether market 1 passed while its surplus was still zero. That is one
instrumented re-run away and is not guessed here.

---

# ADDENDUM E — COHORT-15E, 2026-09-04

**Devnet evidence. Not mainnet evidence.** Tree root `/Users/ember/dev/dclutch`,
HEAD `5f9cd1ca3` at the start, `08fe86470` at the stop. **No transaction was
signed or submitted by this lane, no program was changed, and no market was
founded.** Deployer `23.890559434` and campaign payer `1.304204301` at both ends
of it.

Two walls arrived here named by code and unexplained by cause. Both are convicted
to one field. In both the field belongs to a deployed program, so under this
cohort's standing rule — no program change under the live cohort — the decision
is recorded and not made.

## E1. THE FIELD THAT MOVED IS NOT IN EITHER LEDGER: DEVNET CHANGED ITS RENT

D10 left `active-funding-ledger refused: native custody arithmetic` with market
1's and market 3's funding ledgers byte-diffed and equal in everything the
conjunct reads, and asked whether `allow_lamport_surplus` differed between the
paths or whether market 1 passed while its surplus was still zero. **Neither.**
The two accounts are equivalent, the flag is `false` on both paths, and the
answer was never in the accounts.

Read off the chain, in this order:

1. **Market 1's ledger was READONLY in its own admission transaction**
   (`64jEDVT6…`, slot 492,829,917) — it appears in that transaction's lookup-table
   readonly span — so nothing rewrote it afterwards, and its bytes at 03:44:45 UTC
   are its bytes now. Its whole lamport history is two entries: `0 → 2,482,545`
   at the CreateFund (slot 492,763,282) and `2,482,545 → 2,482,539` at the
   activation (slot 492,765,121). Market 3's is the same two entries at slots
   492,859,368 and 492,861,217.
2. **Both ledgers authenticate and report identically.** Selected mask `0x000e`,
   three rows at manifest entries 1/2/3, each `Active`, each carrying one lamport
   of Bounty principal and two released, against a quote of three:
   `remaining_native_lamports_total = 3` for both.
3. **Every account cohort-15 created reads back at 6,333 lamports per byte over
   the 128-byte overhead** — both certificate seats at 312 B (2,786,520), both
   payout ATAs at 170 B (1,887,234), both Markets at 368 B (3,141,168), both
   capability manifests at 2,128 B (14,287,248), both funding ledgers' rent
   component at 264 B (2,482,536).
4. **The cluster now says 5,080.** `getMinimumBalanceForRentExemption` returns
   1,991,360 for 264 B and 650,240 for 0 B; the Rent sysvar reads
   `lamports_per_byte_year 5080, exemption_threshold 1.0` — the shape SIMD-0194
   prescribes. An account created at finalized slot 492,933,968 holds exactly
   650,240.

`validate_native_custody(account.lamports, rent.minimum_balance(len),
allow_lamport_surplus = false)` asks for **exact** equality between what the
account holds and `rent.minimum_balance(len) + remaining principal`. The first
term is read from the Rent sysvar AT THE MOMENT OF THE CHECK; the account was
funded at whatever that term was when it was created; and nothing in the ledger
records which. So:

| | market 1, 03:44:45 UTC | market 3, after 08:07:58 UTC |
| --- | ---: | ---: |
| ledger lamports | 2,482,539 | 2,482,539 |
| `rent.minimum_balance(264)` | **2,482,536** | **1,991,360** |
| remaining native principal | 3 | 3 |
| expected | 2,482,539 | 1,991,363 |
| verdict | **exact** | **PresentNativeLamportsMismatch** |

**Epoch 1141 began at slot 492,912,000 = 07:31:40 UTC**, between market 1's
admission (03:44:45 UTC, epoch 1140) and market 3's settle (slot 492,925,112,
08:07:58 UTC). A rent-exempt minimum is a per-epoch cluster parameter; that
boundary is the only point at which it can move, and it is inside the bracket.

### The instrument now says the arithmetic, not its name

`authenticate_active_funding_ledger` prints the six numbers on the refusing path.
Verified on the live refusal, against a prediction made from the account bytes
before the instrument existed:

    active-funding-ledger custody: lamports 2482539 against rent minimum 1991360
      + remaining native principal 3 = 1991363 over 264 bytes
      (surplus 491176, allow_lamport_surplus false)

**491,176 is the whole of the rent difference** — `2,482,536 − 1,991,360` — to
the lamport. Not one lamport left the account; the cluster reclassified it from
rent into surplus.

### The author is a program

`programs/dclutch-core-sbf/src/resolution.rs:1270` runs the same call with the
same `false`. A host relaxation would build a transaction the deployed Core
refuses with `CoreSbfError::Funding`, so nothing host-side lands the admission.

**Decided by reading, not made.** The close path already anticipates a surplus
and classifies it (`allow_lamport_surplus = true`, then `ledger_lamport_surplus`
folded into the refund); the admission paths do not. The parsimonious repair is
to **persist the rent the account was funded at and compare against that**, which
keeps the refusal exact rather than widening it to `>=`. Widening admits a real
donation as custody; the recorded figure does not. The ledger header's four
reserved bytes cannot hold a `u64`, so this is a v3 schema question, not an edit.

**So the first stranger payout on an honest selector is not blocked by anything
in cohort-15.** The ATA `EorpstZuhLHkXXraUHm32zN8kzP8YedEfnVuXG4it9ew` is created
and waiting, market 3's certificate reads kind 1 with selector 1, and the buyer
holds outcome 1. What stands between them is one conjunct in a deployed program
and a cluster parameter that changed under a live cohort.

### Three tests, over the two ledgers' real bytes

`crates/dclutch-resolution-core-v3-operator/tests/funding_ledger_rent_parameter_v1.rs`,
with both ledgers and both manifests committed as fixtures read off devnet at
finalized commitment (each manifest's sha256 is the manifest id its ledger's
header binds, which is the fixtures' own self-check):

* everything the conjunct reads is equal across the two markets;
* the verdict flips on the rate alone, identically for both;
* the stranded amount is exactly the difference between the two minimum balances.

Proven red by declaring the post-change rate to be the creation rate: two of the
three fail, one on `Ok(())` where `Err(PresentNativeLamportsMismatch)` was
expected.

## E2. THE GENERAL FIELD IS NOT A COMPILER WRITING ONE ID TWO WAYS

D6 convicted OpenBatch's `0x4015` to `derivation_policy 68b51371… !=
child_derivation_id 7fe9b22d…` and called it a founding input one compiler wrote
inconsistently. **One author writes both, and it writes them consistently.**

    selected_capability.rs:112   entry.child_derivation_id = descriptor.derivation_policy()
    general_market.rs:79         descriptor = release.bundles.first()
    release_v3.rs:99             GENERAL_ACTIONS_V5[0] = Action::Consider
    general_selected_release_v1.rs:1042  the activation bundle takes the same first descriptor
    general_selected_release_v1.rs:1129  descriptor.derivation_policy = digest(lifecycle_policy)
    general_selected_release_v1.rs:1194  encode_lifecycle compiles PER ACTION

So the manifest entry binds `Consider`'s policy, the activation descriptor
carries `Consider`'s policy — which is exactly why the market activated — and
OpenBatch's descriptor carries its own.

**The inconsistency is structural, and both halves are required.** The General
family's `validate_descriptor` pins `derivation_policy ==
lifecycle().program()` (`dclutch-general-adapter-contract/src/artifacts_v3.rs:522`),
and the lifecycle is per action because `GeneralChildRentWidthsV5` is per action.
Fifteen actions therefore mint fifteen policies by requirement. A Market's
capability manifest carries ONE entry per capability root and therefore one
`child_derivation_id`, and `CapabilityProgramV4::validate_selection` compares
them. **A General market can execute exactly one action**, and re-founding
chooses which one rather than repairing anything — which is why this lane did
NOT re-found it. Binding OpenBatch would strand the other fourteen and buy one
transaction with a wall moved sideways.

The Direct family does not have this shape: its non-ordinary bundles carry
`ordinary.derivation_policy()` (`begin_retiring_bundle_v1.rs:131`,
`native_close_bundle_v1.rs:187`, `close_maker_bundle_v1.rs:137`,
`activation_bundle_v1.rs:231`), so one entry binds every Direct action. That is
the shape General owes. Moving to it changes
`dclutch-general-adapter-contract`, which is linked into the accelerator —
**a program is the author again.**

`every_action_descriptor_carries_its_own_derivation_policy` pins it: fifteen
pairwise-distinct policies, with the Consider/OpenBatch pair named rather than
indexed. Proven red by making `encode_lifecycle` ignore its action.

## E3. MARKET 1'S RETIREMENT IS BEHIND THE SAME CLUSTER PARAMETER

`Resolution CloseFund: Funding` publishes one code from twelve sites in
`authenticate_close_funding`, six of them `map_err(|_| …)` and one a `_ =>` arm
that discarded which side of a pair disagreed. Every site now names its conjunct
and the four that compare numbers print both numbers.

**They are not exercised on a live refusal, and the reason is E1.** Market 1's
sequence no longer reaches CloseFund. Run plan-only against a COPY of the durable
journal in this lane's own scratch — nothing signed, no other lane's session
touched:

    dclutch-local-successor-bootstrap devnet-terminal-sequence-v1 \
      --plan $JOB/backups/cohort15c/plan-seal.json.nonce-bound \
      --market-input $JOB/market/market.json \
      --evidence $JOB/market/campaign-open.json \
      --refreshed-evidence $JOB/retirement/refresh.json \
      --market 3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2 \
      --session <copy>/session.json --journal-dir <copy>/journal

    Error: terminal session receipt rent no longer rederived from the canonical
           Rent sysvar

`authenticate_terminal_receipt_funding_v1`
(`tools/local-validator/bootstrap/successor/src/terminal_sequence.rs:8115`) holds
the session's `receipt_rent_lamports` — **3,445,152**, the figure its prepay
finalized with at 07:01 UTC, pre-flip — against `rent.minimum_balance(416)`,
which now rederives **2,763,520**. With that one guard opened in an UNCOMMITTED
probe build, the next refusal is `terminal ALT data width, rent, or canonical
prefix refused`: the lookup table `4a3S6qTKmXAGF5qPcWpDMsgbQiDCHjZdmhExA262NpMT`
holds 13,729,944 lamports and was funded at the old rate too.

Driven to the end, market 1's remaining path is **five rent-exactness guards
deep**, each opened in turn in an UNCOMMITTED probe build to see the next:

| | guard | the two numbers |
| ---: | --- | --- |
| 1 | terminal session receipt rent | 3,445,152 against a rederived 2,763,520 |
| 2 | live terminal ALT rent | 11,703,384 against 9,387,840 |
| 3 | frozen terminal ALT final rent | the plan's, rederived today |
| 4 | supplied frozen-union ALT rent | the same, on the supplied table |
| 5 | `Resolution closure receipt was not the exact vacant at-most-rent destination` | the seat holds its prepaid 3,445,152; today's bound is 2,763,520 |

Guards 1–4 now split into one sentence per conjunct and print both numbers
(`69e0de7f4`); the live refusal reads *"terminal session receipt rent 3445152 is
no longer what the canonical Rent sysvar rederives (2763520) for 416 bytes"*.

**Not one verdict moved, and the reason is a test.** The two ALT rent comparisons
were first made floors, on the reasoning that the program they mirror,
`require_prepaid_output`, asks `lamports < minimum`, so a table holding more than
today's minimum is still exempt. The tree refused it:
`terminal_alt_refuses_divergence_partial_freeze_surplus_and_wrong_boundary` is
written about ONE extra lamport on the table, so the exactness is deliberate and
it is what refuses a table carrying lamports nobody can account for. The floors
were reverted; only the sentences were kept. **A guard a test defends is not a
guard to loosen on the way to a wall behind it** — which is also why guard 5,
an upper bound on custody rather than a mirror of a program floor, was left
exactly as it is.

Every one of the five makes the same mistake as the program conjunct in E1: it
compares an account funded at one reading of a cluster parameter against another
reading of it. The fact each of them wants is the rent the account was funded at.
The session already records that figure; the ALT plan does not; the ledger
cannot.

Market 1 reads phase byte 3 `Retiring` and `outstanding_capabilities` 1 at
CoreState offset 280; market 3 reads phase 1 `Open` and outstanding 1. Both are
correct for where their sequences stopped.

**The runbook's `retire` row stays owed**, and this lane deliberately did not add
one: `tools/cohort/steps.tsv` has 25 rows and no retirement row, and a row
asserting a step that has never completed on any chain would be a runbook
claiming what the evidence does not.

## E4. WHAT COHORT-15 NOW KNOWS THAT IT DID NOT

A cohort is funded at the rent-exempt rate in force when it is founded, and every
later check that re-derives that rate is a check against a number the cohort
never agreed to. Devnet moved the rate mid-cohort and stranded three walls in one
epoch. **The lesson is not "devnet is unstable"** — it is that a fixed bound this
tree would label *chain-derived* has been treated everywhere as *mathematical*,
and AGENTS.md already requires a bound to be labeled. The account's own funded
rent is the fact; the sysvar is a reading of it at a moment.

Two acts remain owed and neither is blocked by anything cohort-15 built: the
first stranger payout on an honest selector, and the first General OpenBatch on a
real chain. Both wait on one rule inside a deployed program.

## E5. COMMITS FROM THIS LANE

    260684fad  funding custody: the conjunct that reads a chain parameter, and
               the fifteen policies one entry cannot hold
    08fe86470  close-funding: twelve sites, one code, and now twelve sentences
    fce9b7b76  evidence: addendum E, the field that moved was the cluster's rent
    69e0de7f4  terminal sequence: four rent guards that said "refused" and now
               say which number

All carry `Lane: COHORT-15E`. Every new test was proven red against a real
regression and green when restored. No crate in an SBF link was changed, so no
frameguard rows are owed.

---

# ADDENDUM F — COHORT-15F, 2026-09-04

**Devnet evidence. Not mainnet evidence.**

Addendum E stopped with two walls convicted and nothing signed, and named the
one fact a resumer needed: devnet dropped its rent-exempt rate from 6,333 to
5,080 lamports per byte at the epoch-1141 boundary with this cohort live on it.
PROGRAMS-16 then gave the funding ledger a field for the rate its founding paid
(`c0a1586b1`) and corrected addendum E on who refuses (`4137ec0d3`): Core does
not run the native-custody conjunct on `AdmitTerminal`, so the wall belonged to
`dclutch-resolution-core-v3-operator`'s planner, a host.

This addendum is what the host had to learn to price a cohort that was founded
before the field existed, and what happened when it could.

## THE FIRST STRANGER PAYOUT ON AN HONEST SELECTOR, ON ANY CHAIN

| act | signature | facts |
| --- | --- | --- |
| market 3 admit-terminal | `UNQQiM29eLGJy6LHkXo5Dt7aVwsD29SyXaECQ3T8JLu7sd3Xotv94QQdZUeXuJk5EComD7JsmpmCG3MPpYW3uo8` | slot 492,976,283; Market phase byte @10 goes 1 Open to **2 Terminal**, read back |
| market 3 custody replay | `2qrSV6jRhnsFYLQawvGAAjyUzBr5Saw6Yzfp7xPyH2RZCfQsSTKtXBPKYY8SMqdzQZg8V7ab9QRaZHnmc2fW96Wv` | slot 492,976,487, 91,911 CU, replay `6hAgMbNWkpnzkf3T2sEX8UXzMuHg9zSvA4FzQqaq3xPU` next_revision 1 |
| **the winning stranger's payout** | `5K5Tqf1NnjdaQk2L8XNBmqtG3vUQQaeEuF1vJPu3Sf5gDF9YdtM8MW5fDpMzCskfxsTVMg3YUjrXxSJz62jpSbwe` | **payout 200 atoms** to `EorpstZuhLHkXXraUHm32zN8kzP8YedEfnVuXG4it9ew`, six passes (ALT create, two extends, freeze, activation, payout) |
| the loser's zero | `dfUtUcZshtRZ5qyRb3AVqFFd7iQP9JcEqLgpNtGLG4CaFKk2sZy1i4o7EJbLpSSB2wQ6bKK2n8EeQWNSZvmoNj7` | **payout 0**, founder's claim index 0, quantity 500,000,000 |

The stranger is participant-2, a key admitted as an ordinary user with no
relationship to the founding, and the outcome that paid is **1**. The selector
is stated both ways and they are the same cell: the certificate at
`9BnUF5rKx2WNbvvexQd3f7CgJzUBEBEwBn8ZvwK62gSu` (312 B `DCSRCER2`, kind byte @10
= 1) commits selector **1** at offset 256; and the statistic, SOL/USD
`$103.972224` at exponent −8, is 10,397.2224 on the cuts' ×100 scale, which
falls between the cuts 10,200 and 10,600 — **cell 1**. Read off chain after the
payout: the stranger's ATA holds **200** atoms of a 6-decimal mint and the
founder's payout ATA `5ieuXMUGy472Fx97nhnquQUXD4iYrnj1VjdVsVKeLYLi` holds **0**.

The loser's zero was asked twice and answered twice, which is worth separating.
The founder's claim at index 0 — an outcome that did not occur, on a position
that holds 500,000,000 of it — executed and paid **0**. The stranger's claim at
index 0 does not exist at all, and its producer says so rather than building a
zero-atom transfer: `payout quantity must be within 1..=0 atoms at claim index
0`. A holder of one outcome has no claim on another; a holder of every outcome
has a claim worth nothing on three of them.

### The census at the boundary, with L4 retiring by name

`$JOB/census-c.sh cohort15f-market3-post-payout`:

    HOLDS L1: tracked 1000000000 atoms across 7 accounts == Mint supply 1000000000
    HOLDS L3: 2 Positions sum to the aggregate supply vector [0, 499999800, 500000000, 500000000]
    INAPPLICABLE L4: the Market is terminal: settlement DISCHARGED the liability
      this law is stated about ... L4 is a PRE-TERMINAL invariant and retires by
      name here rather than reading VIOLATED against a protocol that did exactly
      what it should. L1, L3 and L7 go on watching this boundary unweakened.

The aperture cost one wrong reading first, and the instrument was right about it
for the third time in this cohort. Six 160-byte `DCLLBP02` Positions exist on
this cluster; three are market 1's, one is the second market's untouched founder
position carrying `[500000000 x4]`, and two are market 3's. Binding the extra
one made L3 read VIOLATED by exactly one position's supply at every index.
`$JOB/census-c.sh` now names the two and says how they were enumerated.

## UNIT 1 — THE PLANNER RECOVERS A RATE NOBODY RECORDED

A cohort-15 funding ledger carries zero in the span a cohort-16 founding writes
its rate into, and zero prices every account at nothing, so
`FundingLedgerV2::decode` refuses it. That is right for a program and wrong for
a host that must plan against a cohort already on chain. `afab02c25` adds
`dclutch-resolution-core-v3-operator::funded_rent_recovery_v1`:

    rate = (lamports − remaining native principal) / (ACCOUNT_STORAGE_OVERHEAD + len)

which is `the_rate_is_recoverable_from_the_zero_length_minimum` and
`one_rate_prices_every_length` read backwards. Three things make it a recovery
rather than a guess.

- **The division is exact or nothing is recovered.** A donated lamport puts the
  balance off the affine line and no rate reproduces it. That is the same
  hostile PROGRAMS-16 wrote for the recorded-rate path, asked one layer earlier,
  and it refuses under its own name: `FundedRentUnrecoverable`, split out of
  `Funding` under decision 0007 because "funding state, manifest binding, or
  physical custody" would send a reader to the wrong place.
- **One founding is one rate** — `the_whole_cohort_is_one_rate`. Sibling
  readings are folded into one rate and any disagreement refuses. The siblings
  are not invented: market 3's founding created a second funding ledger,
  Trading-owned, at `Gm5WFhDCa7CryyLkfPWBvcDQhAEcwszjDVAhJvdmq1tx` — 120 bytes
  holding 1,570,584 lamports with no principal outstanding. Two widths pin the
  affine function, which is the shape `derive_funded_rent_rate_v2` requires of a
  founding that records its rate rather than recovering it.
- **A recorded rate is never second-guessed.** A header that speaks for itself
  is returned unchanged, even when the balance would derive another — otherwise
  the record stops being the authority and a lying record becomes unfalsifiable.

The header span is located by probe rather than by an offset this module would
be a second author of: the span is the one that is currently all zero and,
filled with a probe rate, decodes to THAT rate with the manifest identity and
selected mask unchanged. Exactly one span in a legacy header does that. The
recovered rate is spliced into a COPY; the account on chain still holds the
zeros it was created with, and the ledger's PDA derivation binds only the
manifest identity and the mask, so the splice cannot move an address.

The tests are on the four `.bin` fixtures PROGRAMS-16 committed — cohort-15's
own account bytes at finalized commitment, unedited — and the red half is
permanent rather than a moment in a session:
`a_cohort_fifteen_ledger_records_no_rate_and_does_not_decode` pins the wall so
that a recovery which did nothing could not look like one that worked.

Five accounts of this cohort at five widths, read off chain 2026-09-04, all
derive 6,333:

| account | bytes | lamports | (128 + len) × 6,333 |
| --- | ---: | ---: | ---: |
| Resolution funding ledger ×2 | 264 | 2,482,539 | 2,482,536 + 3 principal |
| Trading funding ledger | 120 | 1,570,584 | 1,570,584 |
| Core Market ×2 | 368 | 3,141,168 | 3,141,168 |
| certificate seat | 312 | 2,786,520 | 2,786,520 |
| payout ATA | 170 | 1,887,234 | 1,887,234 |
| Position ×6 | 288 | 1,823,904 | 1,823,904 |

On chain the recovery prints one line per planned ledger and it is in every log
this lane produced:

    funded-rent recovery: ledger of 264 bytes holding 2482539 lamports over 3
    native principal was funded at 6333 lamports per byte (0 siblings agreed)

`ec373d90d` then gives the terminal session the same treatment. It derived its
rate from the Rent sysvar, which answers what an account created NOW would cost
while every account the sequence prices was created by a founding that may long
predate the reading — the reading that refused market 1 five guards deep. It now
recovers from the Market (which holds exactly its funded minimum and parks no
principal) cross-checked against a closure receipt an earlier session already
prepaid, at a second width. Market 1's session records `fundedRentRate 6333` and
`receiptRentLamports 3445152`, which is 544 × 6,333 and is exactly what its seat
has held since 03:42 UTC.

## UNIT 3 — MARKET 1 MOVED FROM FIVE WALLS TO ONE, AND THE ONE IS NOT A REFUSAL

Four host defects stood between market 1 and `Resolution CloseFund`, each found
by driving the sequence and reading what it said. None was repaired by loosening
a check.

**1. A journal outlives the session that wrote it** (`291e3e277`, `713a0f012`).
The funded-rate schema superseded the v1 session that had prepaid market 1's
seat, and a successor session then refused the pair — prepay journal beside a
receipt that needed no prepay — categorically, at two sites. The replacement is
stricter about the case that matters: a journal accounts for a seat only when it
is FINALIZED, because a planned or submitted prepay puts no lamports anywhere.
The prepay stays inside the durable prefix so a later action after a missing
earlier one goes on refusing.

**2. The mask says which three entries; the material says what each is for**
(`270f23a13`), and this one was building a wrong instruction, not only refusing
one. `funding_entries_from_mask` walks the selected mask's ascending bits;
`select_resolution_funding_entries` is the one author of the roles and returns
`[recovery, exhaustion, failure]`. `authenticate_close_funding` read the first
list as the second twice. Convicted by printing the four identities rather than
by inspection:

    close-funding no-recovery compartments: entries [1, 2, 3],
      material fdea108fda41f8ac74a0072e6a31ae080e3f49529c3af69aca6a5b5ddffbd2b4,
      configs [3a4c18d1…, fdea108fda41f8ac…, a8b3245e…]

Exactly one config is the material's own and it is the **middle** entry, so the
structural fact the check means to assert holds and only the positional binding
failed. Core folds entries into a mask at `CreateFund`, so nothing on chain ever
required failure to be last, and this market was founded and activated with it
in the middle. The second reading of the same list filled
`recovery_entry_index` / `exhaustion_entry_index` / `failure_entry_index` in the
retirement facts — same mistake, no refusal, wrong compartment in two of three
slots. The positional config check was the thing stopping that, which is why the
repair is the author and not the check. Third instance of this exact defect;
`0c26bba0` fixed it at verify-fund-ready and the activation-receipt arm was
fixed after it, both by comparing MEMBERSHIP.

**3. A readonly account may be declared unmoved, and may not be moved**
(`16dd0e917`). Two sites carried one refusal over two accusations — a writable
account with no poststate, and a lamport delta on an account the frame cannot
write. The second is a contradiction only when the delta is NONZERO. A zero
delta says the account's lamports do not move, which is what a readonly account
supports and which is checked against its own recorded pre/post balances.
`ResolutionCloseFund` says exactly that about the Market, correctly, and was
undrivable for it. Convicted the same way, by naming the key:

    terminal semantic report: writable with no poststate [],
      lamport deltas on readonly accounts ["3QytL1bBMtCvRoXWR5h7MgutRBZqtv7emUVubEo5a4T2"]

**4. And then the transaction was built, signed, and could not run.**

    Program 24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn invoke [1]
    Program 24AkUjtXg61La45u7KTge8u4dKpVqkzirmzycVyckFgn consumed 200000 of
      200000 compute units
    Program … failed: exceeded CUs meter at BPF instruction

**`ResolutionCloseFund` exceeds the default compute budget, and the terminal
sequence declares no `ComputeBudget` prefix at all.** It is the only driver in
this tree that does not: `direct_trade.rs` and `aggregate_retirement_journal.rs`
both bind two exact ComputeBudget prefixes, and the wallet payout that landed
above spent 235,003 CU, which is not possible under the default. The prefix is
not a flag: `authenticate_terminal_message_decompilation_v1` pins the durable
message at exactly ONE instruction, so it is a change to the durability schema,
the v0 placement authenticator, the completion expectations and the fee
arithmetic together. **This lane stopped here rather than start it**, and this is
why retirement has never completed on any chain — not the five rent guards,
which are now passed, and not the retirement packets, which are drivable.

The signed packet is durable at
`$JOB/terminal-1/journal/13-resolution-close-fund.json`, phase `submitted`,
expected signature
`JGLMWwRMmASsszK3ciYjWpa1RYPD4BzC5xv4ZSongFrLDY6dveAprxeTYhfgsipHuiguGxKUcK6bZmFs2JsVdRi`.
`getSignatureStatuses` with `searchTransactionHistory` returns `null`: it never
landed and, exceeding the meter, never can. **It was not re-signed and must not
be** — it is left for its blockhash to expire, which is what the ambiguous phase
is for.

## THE RETIREMENT PACKETS ARE DRIVABLE, AND WITNESS-3'S CONCERN IS ANSWERED

WITNESS-3 asked whether market 1's retirement finish needs
`core/retire_v1::process#Retire`, whose 2,152-byte instruction
(`RETIREMENT_INSTRUCTION_BYTES_V1`) exceeds Solana's 1,232-byte packet and which
no CPI caller builds — and, if so, whether the row belongs in `blocked.json` as
`structural`. **It does not, and no such row is owed.**

`build_market_retirement_v1` is the legacy aggregate builder and is where the
2,152 bytes come from: 72 + 480 + 256 + 2 × 672. Nothing in the retirement
sequence submits it. `build_checkpoint_market_retirement_v1` builds four
packet-bounded instructions from the same finalized snapshot, all four to the
Core program:

| packet | data bytes | route in the DEPLOYED Core (`1cae26fd6`) |
| --- | ---: | --- |
| prepare | 72 + 480 + 256 = **808** | `Action::Retire` at `RETIREMENT_CHECKPOINT_PREPARE_INSTRUCTION_BYTES_V1`, `lib.rs:627` |
| close-vault | 192 + 672 = **864** | `AGGREGATE_RETIREMENT_CLOSE_VAULT_MAGIC_V1`, `lib.rs:392` |
| close-replay | 192 + 672 = **864** | `AGGREGATE_RETIREMENT_CLOSE_REPLAY_MAGIC_V1`, `lib.rs:392` |
| finish | 192 + 72 + 480 = **744** | `AGGREGATE_RETIREMENT_FINISH_MAGIC_V1`, `lib.rs:392` |

The 808 figure is the `core/process_instruction#Retire` route WITNESS-3 named.
All four are inside the packet, and all four dispatch arms are in the bytes
actually deployed — checked by reading `1cae26fd6`'s `dclutch-core-sbf/src/lib.rs`
rather than HEAD's. What blocks the row is upstream of it and is item 4 above.

## THE RUNBOOK

`tools/cohort/steps.tsv`'s `retire` row named the phase byte at CoreState offset
280. It is at offset **10**; 280 is `outstanding_capabilities`. Corrected, with
both figures now read off chain in this addendum. The README's `retire` section
records the compute-budget wall as the open one, and its `funded-rent-recorded`
section now says what a reader who finds a zero should do — a zero is not a dead
end for a cohort already on chain, and `funded_rent_recovery_v1` is the host that
answers it.

## BALANCES AND STATE AT THE STOP

    deployer  4zrxtw5c…  23.890559434   unmoved by this lane
    payer     D5qe7ZoQ…   1.289445181   was 1.304204301; this lane spent 0.014759120
    market 1  3QytL1bB…  phase @10 = 3 Retiring, outstanding_capabilities @280 = 1
    market 3  C9dLhWj7…  phase @10 = 2 Terminal, outstanding_capabilities @280 = 1

## COMMITS FROM THIS LANE

    afab02c25  funded rent: a cohort already on chain records no rate, and its
               own bytes still name one
    ec373d90d  terminal session: the rate is recovered from the founding, not
               read off todays cluster
    291e3e277  terminal sequence: a journal outlives the session that wrote it,
               and can account for what it started from
    270f23a13  close funding: the mask says which three entries, the material
               says what each one is for
    713a0f012  terminal sequence: the same inherited prepay, for the order of the
               journals as well as their arithmetic
    16dd0e917  terminal sequence: a readonly account may be declared unmoved, and
               may not be moved

Every one carries `Lane: COHORT-15F`. The driver in the job directory is built at
the last of them and its digest is beside it in `bin/DRIVER_PROVENANCE.txt`; it
was built from a DETACHED WORKTREE at that commit, because the shared live tree
was mid-edit by another lane and would not compile.
