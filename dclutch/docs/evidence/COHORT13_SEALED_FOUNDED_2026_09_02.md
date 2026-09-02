# Cohort-13: the founding and the seal finally name the same release set — 2026-09-02

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized.

Tree root `/Users/ember/dev/dclutch`. Every deployed byte is built from
`315f1931f4d6bb01510a3b78ccd056149e87367f`.

## Headline, and it is one number said twice

| | |
| --- | --- |
| founded plan `plan.json` release set | `82a969ddbcd1782aab65016632742e4dd956978dc5e3a8f0ba0f853e0c13c62c` |
| sealed plan `plan-seal.json` release set | `82a969ddbcd1782aab65016632742e4dd956978dc5e3a8f0ba0f853e0c13c62c` |
| sealed deployment set final digest | `e6829ff9adbe74d80bb7cf6c701570b89146d130168d606ca49b735ea3ce72a4` |
| checked release gate | `f360ef2f0bc254b0d550dc58d5e1dfe0267bbc5ce45179e893d74e93ec69128c` |

**Cohort-13 is the first cohort in this project's history whose founding and
whose checked seal carry the same release-set identity**, and the seal cost
**zero SOL**: no upgrade ran, no deployment slot moved, no key was opened.

Cohort-12 could not have this. Its founded plan embedded semantic ids that were
hashes of the git revision `e39efbb0`, the release-tool repair cannot be applied
retroactively to that commit, and so its seal necessarily minted a different set
and stranded the market it was for. Since `0785bd52` a role's semantic id is a
function of its shipped ELF, and since `2da012cd` `validate_prepare` re-derives
every artifact-derived id from the artifact it is supplied beside and refuses a
mismatch by name. The two identities cannot diverge here; they are not equal by
luck.

## 1. The gate certifies the bytes this cohort runs

`checked-release-candidate.sh --genesis-cohort` at `315f1931`, run from a
detached worktree at that exact commit because the candidate refuses to run as
any commit but its own.

```
source_revision=315f1931f4d6bb01510a3b78ccd056149e87367f
sbf_build_freshness=passed          sbf_build_freshness_links=12
sbf_build_diagnostics_total=0       infrastructure_lineage=genesis
trading_elf_sha256=1b41f55254c6b2bac198570958c9557616069e34176bc4774f47f1f29204f670
trading_profiled_elf_sha256=70edb010b7ee9deea652592f551ecc51853aeb4969663a49a835d2b1b758ce86
trading_admitted_artifact=shipped
CANDIDATE_EXIT=0
```

`trading_admitted_artifact=shipped` is `28ff0823` holding: `provenance/trading.json`
carries the feature in its FRAME command and not in its plain one, so the gate's
Trading link is the ordinary artifact a cohort deploys and not the `hot-cu-profile`
measurement. The profiled build is 6,240 bytes larger and is never admitted.

**Two independent builds agree.** The candidate's own shipped ELFs and a second
`cargo build-sbf` run in a SEPARATE detached worktree at the same commit produce
byte-identical artifacts for all seven roles.

| role | bytes | SHA-256 | second worktree |
| --- | ---: | --- | --- |
| registry | 234,536 | `ed70f8bda12b77d663126218ad05f36dd77c5bf3100642879cef1441a845afe7` | identical |
| rent | 141,680 | `b9128748d972b5e5afdfdb76a5dc363fe62c3b0ac3a4fbc167fe968156d0da8b` | identical |
| custody | 572,272 | `9a77f24cf9d0c039221ab6fd55a8943a35a47ebb1d6b13688cce325c55023f35` | identical |
| resolution | 819,256 | `d31df7c1ab7b4d7dde07373137b2e74489148e299d400f2ac18c7f8a835a21aa` | identical |
| claims | 1,369,712 | `6d719e62b4088ed348aedde99b9fd36d7e9247f79443af65159feb60d58b9fb5` | identical |
| trading | 2,320,152 | `1b41f55254c6b2bac198570958c9557616069e34176bc4774f47f1f29204f670` | identical |
| core | 1,186,176 | `d28da71b9d966e23f6d092641b799e453973d9de80f3c385e09d516ee7910ff1` | identical |

Registry's ELF is byte-identical to cohort-12's: nothing under
`programs/dclutch-registry-sbf` changed between `e39efbb0` and `315f1931`.

## 2. Cohort-12 closed

Ids derived from cohort-12's own keypair files, never transcribed.

| role | cohort-12 program id | rent reclaimed (SOL) |
| --- | --- | ---: |
| trading | `Ahzug4zYhG8sc4t6tXjaSjnqbv7bTkgNYRc4kWUxYGJe` | 14.619686169 |
| core | `G4Wz4fj4zqBPFWYFF9CeYeJtTK5UqSZUu2fyCr9ANjYG` | 7.521102465 |
| claims | `GwduZB13AgqLxsoxi8wZEQndYBsQERea35dhuYKJzCvc` | 8.654608137 |
| registry | `5c4CfHXHaLoJRtVSZFURp6Qhub8P4x8Hk4yZ3KJNrK53` | 1.633574640 |
| rent | `HD72aKvtRzBrVdmDGn8UrcocVA6g4NuG9Bt94GRLMYcW` | 0.991751280 |
| custody | `2MHNgYoCtDzqRryjgAxzFwLVPztSN6NTUr7RmjiMrcLc` | 3.619974465 |
| resolution | `9vs7atqDTAZTMo2a9iMZXD6Nf39jQZ7sZFf2X4pGDDvs` | 5.183820153 |
| | **total** | **42.224517309** |

Deployer `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`: **36.327394469 →
78.551876778 SOL**. The observed delta 42.224482309 is the table's total less
seven 5,000-lamport fees, which closes exactly.

Cohort-12's market `EQnYCUMkz…` was already terminal-in-place — reachable by no
fill and no resolution — so nothing reachable was lost.

## 3. The redeploy

Seven programs, fresh identities, each verified by dumping the on-chain image
back **before the next one started**. Every dump is byte-identical to the ELF.

| role | program id | ProgramData | deployment slot |
| --- | --- | --- | ---: |
| registry | `8XsxVn35gtemD9PuWC9pHYX1rxBAC4T8xV4xdrkdfCBV` | `CAg7K8jM7JUNEVUsatiQVdhsVLFuhLXbGKixCa93uygE` | 491,947,648 |
| rent | `CUDLPLjjiLNAQ6hPczhL3AoDux3u77zaJyTERbAbs7Am` | `7puqpbRCHq62NgmSkbHdpRbAkprdZW3EKZKe4fTcgfSR` | 491,947,697 |
| custody | `G7xAFLpJzdCnc7FXjj5uE3qWSk3ZgCgL684YCQEdCah4` | `GuQLkd2HfhAeRG7jTfHVGs4P4shxxesZR93bmqGEauUs` | 491,947,778 |
| resolution | `J33AaXnVDFGYJXYhDxFCPGk4MSM2Ssc69ZcU5PbkYfbb` | `D7nZhwBe81dN2RgnGinxz2YDYh9vFBWLos5Yd7aCBZDR` | 491,947,871 |
| claims | `3XHt6sRpdFxeAa1J23T8TKKFgA78ioJQAdeqJ3HZ5zMv` | `E64zDwMKVti1KqoHW2WKWNGnYL29ZjfbLDBZdL1pz2xW` | 491,948,001 |
| trading | `HkNhMJrERGko9mFXKq6UaL8qu2QnzqJx1hwJ5U8AVUHZ` | `HL9zDRHXfU9eqrTxXTq9nTdpPtW5mahsUWY4LrnjUobS` | 491,948,188 |
| core | `HZsbUHHwJLUqXdUhjDc4vhnmtgqr65VkU36G8hTijWiy` | `4omgGnaiYEPEomH8k7mRwjrog8Ayg7mBeQJjsQh7D1dv` | 491,948,308 |

**Three instruments, one claim.** The dump comparison, the byte count, and a
hostile decode of each live ProgramData account whose payload digest is compared
to the built ELF — all seven `MATCH`. All seven carry an authority tag of 1 equal
to the retained deployer, are Loader-owned, and their ProgramData accounts are
non-executable.

The ProgramData address is **read, not derived**: it is the 32 bytes the Program
account itself names at offset 4. A closed program keeps its 36-byte Program
account, its executable flag and the ProgramData address it names — only the
account holding the code is gone — so asking the Program account is the question
that cannot distinguish a live cohort from a dead one.

Deployer **78.551876778 → 36.428873159 SOL**; the redeploy cost **42.123003619**.

## 4. The ladder

`campaign --through activation`, deployer as Core upgrade authority, campaign
payer `7j8RHenZxcdrfGkqxei3RsJqJUgdwYuB16LbX8d7uUzw` funded with 2 SOL first.

**33 transactions, zero errors, 4,062,457 CU, 2,475,000 lamports of fee.**

| role | activate immutable release-set role | CU |
| --- | ---: | ---: |
| Core | 491,951,052 | 636,717 |
| Claims | 491,951,092 | 723,340 |
| Trading | 491,951,138 | 1,192,922 |
| Resolution | 491,951,178 | 441,343 |

Succession said so rather than being assumed:

```
campaign stage succession: nothing to execute -- this cohort is born at V2
and carries no ceremony; observed absent
```

**Re-observed from chain** by a second preflight that reads the cluster rather
than the driver's exit code — substrate, publication, initialize, succession and
activation all `complete`.

## 5. THE SEAL, before the founding and at zero cost

The seal was run **before** the founding rather than after it. It is key-free and
read-only, so a refusal costs nothing; running it first means the founding is
only attempted once the identity it will pin is already proved reachable. The
market is then founded from the SEALED plan, which is what cohort-12 could not
do and is why cohort-12 is reachable by neither the fill nor the resolution.

**Carry-forward capture** (registry + rent closure) succeeded at 506,580 bytes
with **no rent top-up**: schema v2 records the live Rent rate its own finalized
context quoted, so `require_rent_exempt` judges against the cluster's actual rate
rather than `Rent::default()`. Cohort-12 bought 0.2373 SOL of over-funding to
paper over exactly this.

Two operational facts worth writing down, because both cost a cycle:

- **The release-capture family admits only `https://api.devnet.solana.com`** —
  *"release capture requires the canonical public devnet endpoint"*. The private
  endpoint the write path needs is refused here, deliberately: the carry-forward
  snapshot is the authority the whole seal rests on.
- **The checked gate resolves its siblings relative to ITSELF.** `provenance/`,
  `elf/`, `evidence/` and the twelve `frame-target-*/…/*.o` frame objects are all
  named as paths relative to the gate file. A copy of the gate sitting alone in a
  directory fails at the first lookup with *"source tree manifest … cannot be
  inspected"*. Only the twelve named objects were copied — 5,700,608 bytes, not
  the candidate's 2.8 GB.

All five owned roles preflight `equal: true` against a **fresh finalized
observation**, not against the journal's own claim:

| role | live ELF = checked candidate | observed slot |
| --- | --- | ---: |
| custody | `9a77f24c…` | 491,952,057 |
| resolution | `d31df7c1…` | 491,952,068 |
| claims | `6d719e62…` | 491,952,075 |
| trading | `1b41f552…` | 491,952,084 |
| core | `d28da71b…` | 491,952,099 |

Journaled `already-current`, then audited:

```
completed_role_count 7      next_role null
final_set_sha256 e6829ff9adbe74d80bb7cf6c701570b89146d130168d606ca49b735ea3ce72a4
registry carry-forward   rent carry-forward
custody / resolution / claims / trading / core: already-current
```

and `prepare --deployment-set-journal` produced a plan whose
`checked_upgrade_set_final_sha256` is that same digest. **`plan.checked_upgrade_set`
is `Some`, and `release_set_id` is unchanged from the founding plan.**

The seven semantic ids, derived from what they identify:

| role | source | semantic release id |
| --- | --- | --- |
| registry | ELF `ed70f8bd…` | `451cb6966d177f38dae6354b2b40d65945802f87b4e2225b3f6567fa2409edc6` |
| rent-credit | ELF `b9128748…` | `e089f9683333e23cad619fd6a308484b64a8026840469f53a563a46c5c3bb7c9` |
| custody | ELF `9a77f24c…` | `00636189f22e5e18135deb32b69adcea4ce3e6e0dc9091f31b048ca967d803ee` |
| claims | ELF `6d719e62…` | `4a1ca487db34da910bf443502023e44ec10b47d36a4389780dae40a1ebde7267` |
| core | ELF `d28da71b…` | `e2bcff463160a5fc9922c3629dc14bb13a0a34a7fcc876566c0dfdc43b1da6d4` |
| trading | code-owned `COMPILED_DIRECT_RELEASE_ID_V1` | `79fad2f04f8d9ce07d76c809fe116db8ef9374adbeb15e62f603235c3a2b96b9` |
| resolution | code-owned `RESOLUTION_CONTROLLER_RELEASE_ID_V7` | `6e4b9a545277cf68731108fe1729ff047affe72e16d79c3930acadc8016f554a` |

Every artifact-derived id was reproduced **offline, outside the tool**, from
`(role label, shipped ELF digest)` and the domain
`dclutch/checked-semantic-release/artifact/v2\n` alone. Unlike cohort-12's, all
five reproduce: cohort-12's registry and rent were CarryForward rows keeping ids
the chain already held from the retired revision-hash derivation, and this cohort
mints all seven fresh under the artifact derivation.

**A second, independent instrument agrees on the release set.** The SDK's
`derive-activation-hint.mjs` asks the Registry through the browser client's own
`discoverCurrentActivationCacheV1` and answers activation cache
`HETBPPJFC7HbxmPdCnU4f1bmZEDABXG98b1KV1YfJnQg` for release set `82a969dd…` —
the same identity the sealed deployment set carries, reached by code that never
reads the plan.

## 6. The market IS founded and open — and the driver said it failed

`SCRIPT_EXIT=1`. The last line before it:

```
Error: Error("getBlockTime RPC error: code -32004 message Block not available for slot 491963417")
```

**That is an RPC condition, not a protocol refusal**, and the whole founding had
already landed behind it. Re-observed from chain rather than believed from the
exit code — the discipline cohort-10 paid 2 SOL to learn:

| campaign label | address | bytes | phase | readiness | generation | selected_release_set |
| --- | --- | ---: | --- | --- | ---: | --- |
| `open_market` | **`6t3ZnmRuxVKsB4NGrpiQurEwK52xSKVyNqY3tF1ner15`** | 368 | `0x01` **Open** | `0x02` Consumed | **2** | `82a969dd…` |
| `found31_market` | `C4sCA56dDCuqoonU7spBS3j21tskD9DL754KiXPDmTXV` | 368 | `0x00` Founding | `0x00` Prepaid | 1 | `82a969dd…` |
| `abort_market` | `Fs2SXRURNAx1soB1hH1bdTVwXKW8gN1C3Z6kuZoyG4kM` | — | — | — | — | vacant, as it should be |

Both Core-owned `DCLTCOR3`, owner `HZsbUHHwJLUqXdUhjDc4vhnmtgqr65VkU36G8hTijWiy`.

**The Open Market's `selected_release_set` at `STATE_SELECTED_RELEASE_SET_OFFSET
= 208` is `82a969ddbcd1782aab65016632742e4dd956978dc5e3a8f0ba0f853e0c13c62c` —
the same string the founded plan and the SEALED plan both carry.** That is the
sentence cohort-12 could not write, and it is the whole point of this cohort.

Three further corroborations, all read off chain:

| what | address | state |
| --- | --- | --- |
| Trading funding ledger | `9VDSCch4JXG3oL3CCFMVX4iypuDeY8fsW5yVAKxEPyCV` | 120 B `DCLTFL02`, Trading-owned |
| found record | `5W9QNVVXLK9grA2NzhCsuzsPNFvcwdPCAMxN5QvDkAJc` | 400 B `DCLTGFQ1`, Registry-owned |
| lock record | `AvEEKXTscWkjzwgvK8umFQu3Sbn56NaVenVEi42W3n9u` | 768 B `DCLPCQ01`, Registry-owned |
| collateral Mint | `Ejswx4ypMm1SohutytuJrvzCHhD2VqGwZMRewYvSB1qu` | 82 B, Token-2022 |
| Direct capability root | `2dGxuxe5LGdckG9r3co9u57MbMzoT5xJJTipUysgA261` | **vacant — correct, activation creates it** |

And all six founding submission journals read `phase = finalized`, `dcltgmf3`
among them, plus `core-funding-create-v1`, `resolution-funding-activate-v1` and
`core-funding-accept-v1`. 186 campaign transactions.

The founding was staged from the SEALED plan (`plan-seal.json`,
sha256 `760df09de06f7887142ccb5829c155a0c340dd1dff4d60f7c9b2cece93a0813e`), so
unlike cohort-12 the market's own plan carries `checked_upgrade_set`.

### The rate, admitted before a lamport was spent

```
directFeeBasisPointsPerSide                       50
directTokenSetupAdmitsThisRate                    true
feeRateIsIrreversible                             true
maximumGrossCollateralAtomsWhoseFeeFloorsToZero   199
```

### The cuts, re-centred on a spot measured twice

Cohort-12's `9800,10200` centred a $100.04 SOL. Read immediately before staging
off the sponsored PriceUpdateV2 `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE`
— the account this market itself resolves against — **$97.702271, conf ±$0.0127**,
with Coinbase $97.705 and Kraken $97.71 agreeing. So `--cuts 9600,10000
--cut-denominator 100 --band-anchor 9770`: $96.00 and $100.00 straddling a
measured $97.70, the anchor stating spot rather than a round number. Under the
declared band (200 bps over 10,000 slots, three plausible half-widths) the three
ordinary cells carry about 25% / 56% / 18%, inside the 9,000-bps ceiling.

The window opens six hours after staging and is 1,800 s wide — the vetted
default, ~5.75 measured provider cadences. Cohort-12 started its window
immediately and handed on a market whose observation had already closed.

## 6c. THE WALL: the founding's evidence has no `execution`, and the recovery step the code names does not exist

Everything after the founding is blocked, and the blocker is one missing block in
one file.

`devnet-direct-capability-activation-v1` refuses:

```
Error: Error("campaign report omitted execution")
```

`campaign.rs:368`. The driver writes `execution` only when it finishes the
founding stage cleanly, and it died on the `getBlockTime` RPC error *after* the
founding landed but *before* sealing that block.

**The resume cannot repair it, and the reason is structural.** Re-running the
driver refuses at:

```
Error: Error("missing founding finalized poststate account Q9zc5g4fqVt84215uXg1XqkZ6kYxRwKkyTRwPiuZsBp")
```

`market.rs:1126`, in `capture_founding_poststates_v1`, which calls
`rpc.required_account` over **every completion account of every finalized
journal**. `Q9zc5g4f…` is DCLTCFQ1's 768-byte Trading-owned Pending ledger, and
it is **vacant — because a later stage of the same founding consumed it.**
Verified on chain. So the resume path demands a prestate the run it is resuming
destroyed by design: it can only re-validate a founding that has NOT finished.

**And the tree already knows the shape of this.** `campaign.rs:377`:

```rust
if execution.recovered_finalized_founding {
    return Err(Error::new(
        "crash-recovered founding evidence is non-consumable; a separate
         recovery-to-complete step must reconstruct and authenticate
         execution.market before terminal use",
    ));
}
```

The refusal names the owed repair in its own words — **"a separate
recovery-to-complete step"** — and that step does not exist. This cohort is the
first to need it.

**Nothing was fabricated to get past this.** Hand-writing an `execution` block is
the same act the cohort-12 file refused for the deployment-set journal: the
consumer authenticates every row, and a lane that types one has replaced evidence
with assertion. So this lane stops here.

### What that costs, and what it does not

- **Costs:** Direct capability entry 0 is not activated, so no fill, no
  settlement, no post-fill census. `49c8fa92` / `be67416e` remain unjudged by a
  real fill — and see §6b: `ledger-census` could not have judged L8 anyway.
- **Does not cost:** the market is Open, generation 2, sealed, and selects a
  release set the checked deployment set carries. It is not stranded the way
  cohort-12 is. The activation deadline is slot **492,460,566**, which at the
  finalized slot 491,965,646 leaves **494,920 slots, about 55 hours.** A
  recovery-to-complete step landed inside that window activates this market and
  the fill runs against it — no redeploy, no refounding, no new release set.

### Two instrument defects this exposed, both worth their own owner

1. **`capture_founding_poststates_v1` re-validates consumed accounts.** A
   completed founding can never be resumed, because its own later stages close
   earlier stages' completion accounts. The check should compare against what the
   journal recorded at the time (it already stores `account_sha256`, `data_len`,
   `lamports` and `owner` per poststate), not re-read a live account whose absence
   is the expected outcome.
2. **A `getBlockTime` failure is fatal to a founding that has already landed.**
   The block-time read is bookkeeping — the founding's authority is its finalized
   signatures and poststates, not a wall-clock lookup. A transient
   `-32004 Block not available` on a public devnet endpoint should not be able to
   cost a two-hour founding its evidence.

## 6b. What the census can and cannot judge, said before it is run

`ledger-census` hard-codes its class and lamport claims as **inapplicable** on
every invocation (`main.rs:1008-1017`):

> external census: the transactions between boundaries were not driven by this
> ledger, and it refuses to guess which compartments they crossed

That is the honest answer for an external observer — `unchanged()` would be an
unearned claim — and it means **L7 and L8 are inapplicable from this command no
matter what happens on chain.** So the fill-boundary law landed at `49c8fa92` /
`be67416e` is NOT judged here, and an inapplicable must never be reported as a
pass: that is the "absent signal needs a positive control" failure at the law
level.

What the fill's census does newly judge: cohort-12's was a single pre-fill
observation, so L2, L5 and L6 were inapplicable for want of a predecessor.
Chained through `--prior`, those three become applicable across a real Direct
crossing for the first time, and L1, L3 and L4 are re-judged at the post-fill
boundary.

Closing L8 needs a repeatable `--declared-class-delta LABEL=I128` on the census
and the simulator populating it from the producer's own manifest on the filling
cycle — the simulator does drive the fill, so it has the standing the external
command lacks. A lane owns it.

## 7. The cut fragment, emitted rather than typed

```json
{
  "checkedReleases": {
    "82a969ddbcd1782aab65016632742e4dd956978dc5e3a8f0ba0f853e0c13c62c": {
      "gateDigest": "f360ef2f0bc254b0d550dc58d5e1dfe0267bbc5ce45179e893d74e93ec69128c",
      "sealedSet": "e6829ff9adbe74d80bb7cf6c701570b89146d130168d606ca49b735ea3ce72a4"
    }
  },
  "schema": "dclutch-public-cut-checked-releases-fragment-v1"
}
```

This row is for the release set the founded market actually selects, so unlike
cohort-12's it is a row to ship. **The cut's `--release-set` argument must be read
OFF THE CHAIN** from the market's own `STATE_SELECTED_RELEASE_SET_OFFSET = 208`,
never pasted out of `prepare`'s result JSON: that file is the fragment's source,
so passing it would compare a value with itself and the refusal could never fire.

## 8. Cost, against the budget stated before spending

At most 2 SOL per step beyond the deploy, and stop at any step that would exceed
it without an executing result.

| stage | deployer | campaign payer |
| --- | ---: | ---: |
| before anything | 36.327394469 | — |
| after closing cohort-12 | **78.551876778** | — |
| after the seven-program redeploy | 36.428873159 | — |
| after funding the payer | 34.428868159 | 2.000000000 |
| after the ladder | 34.391688319 | 2.000000000 |
| after the seal | 34.391688319 | 2.000000000 |
| after the founding | 32.473851850 | **1.663405281** |

| step | cost | against the 2 SOL bound |
| --- | ---: | --- |
| redeploy (the deploy itself, exempt) | 42.123003619 | — |
| ladder: 2,475,000 lamports fee + record rent | 0.037179840 | within |
| campaign payer capitalization | 2.000000000 | at the bound, by design |
| **the seal** | **0.000000000** | key-free, read-only, nothing signed |
| founding (from the campaign payer) | 0.336594719 | within |

The founding drew **0.336594719 SOL** from the campaign payer — cohort-12's
figure to the lamport, which is itself a check that the same work happened.

**One movement is NOT attributed to this lane and is recorded as unattributed
rather than explained away.** The deployer went `34.391688319 → 32.473851850`
during the founding window, −1.917836469 SOL. The founding's fee payer was the
campaign payer, not the deployer; none of this cohort's sixteen keys received it
(all read back at 0 or dust); and the devnet deployer keypair is a SHARED path
that other lanes' live tests also use. Attempting to attribute it by transaction
failed because the same RPC instability that killed the founding also refused
`getTransaction`. Stated as an open number, because a lane that invents a cause
for a balance it cannot trace has stopped measuring.

## 9. What cohort-13 hands on

**The market `6t3ZnmRuxVKsB4NGrpiQurEwK52xSKVyNqY3tF1ner15`** — Open, generation
2, 50 bps, founded from the sealed plan, selecting release set `82a969dd…`, with
55 hours before its activation deadline at slot 492,460,566.

Owned, in priority order:

1. **The recovery-to-complete step**, which `campaign.rs:377` already names. It
   must reconstruct and authenticate `execution.market` from the six finalized
   founding journals and the chain, without re-reading completion accounts a
   later stage consumed. Landing it inside 55 hours activates this market and the
   fee-bearing fill runs against it.
2. **`capture_founding_poststates_v1` must stop requiring consumed accounts** —
   compare against the journal's own recorded digest, width, lamports and owner.
3. **A `getBlockTime` failure must not be fatal after a founding has landed.**
4. **`ledger-census` needs `--declared-class-delta`** so L8 can ever be judged
   outside the journey harness (§6b).
5. **Cohort-14 for General**: `a517d27c`'s inline input transport is in Trading,
   which cohort-13 predates by nineteen minutes — and separately, there is no
   devnet General market compiler at all.

Devnet evidence. Not mainnet evidence.
