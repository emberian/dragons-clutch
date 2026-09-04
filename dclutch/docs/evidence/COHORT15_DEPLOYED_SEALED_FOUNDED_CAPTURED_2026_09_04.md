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
