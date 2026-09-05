# Cohort-13: the founding and the seal finally name the same release set — 2026-09-02

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized.

Tree root `/Users/ember/dev/dclutch`. Every deployed byte is built from
`315f1931f4d6bb01510a3b78ccd056149e87367f`.

**The machine-readable witness is the authority for every number below.**
`docs/evidence/witnesses/cohort-13-discovered.json` and `cohort-13-founding.json` carries the
signatures, slots, accounts and compute units read back from devnet, and the
job directory `~/jobs/dclutch-cohort13-20260902/` holds the plans, journals and
poststates they were folded from. The prose copies numbers from those files
by hand and is kept for its findings; where a number here and the witness
disagree, the witness is right and this sentence is the correction.

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
| Direct FOUNDING-PERMIT root | `2dGxuxe5LGdckG9r3co9u57MbMzoT5xJJTipUysgA261` | vacant — **permanently, by design; see the correction below** |

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
   which cohort-13 predates by nineteen minutes. (This item also read "there is
   no devnet General market compiler at all" — true when written, **no longer
   true**; see the correction at the end of this file.)

Devnet evidence. Not mainnet evidence.

## Addendum: the census can now be told what it moved, and there is still nothing to tell it

`aeb316d4` (census flags) and `bf59126d` (the simulator's declarations) landed at
07:59 and 08:00, before this cohort's fill would have run. So the §6b finding is
closed as tooling: **`ledger-census` can now judge L8.**

**It is not closed as evidence, and the reason is §6c and not the census.** The
fill never ran, because Direct capability entry 0 is not activated, because the
campaign report has no `execution` block. So `49c8fa92` / `be67416e` remain
unjudged by a real fill — now for exactly one reason instead of two.

The recipe is recorded here so whoever lands the recovery-to-complete step does
not have to rediscover it. On the census taken **after** the fill, with the
buyer's collateral source, the seller's Direct token PDA and the venue fee PDA
all named by `--token`:

```
--declared-collateral-delta 0 \
--declared-hoard-delta 0 \
--declared-class-delta unclassified=0 \
--declared-class-delta HoardPrincipal=0
```

Three things about that which are easy to get wrong:

- **From the first declared class, every unnamed class is a declaration of
  zero.** The flag is not additive commentary; supplying one makes the whole
  vector explicit. An unknown label refuses, naming the census's ten.
- **If the buyer's collateral source is NOT named by `--token`, both
  `unclassified` and the collateral delta are `gross + fee`, not 0** — the atoms
  are real and left a set the ledger does not enumerate. That is the same shape
  as cohort-12's `VIOLATED L1`, one law over.
- **L7 becomes applicable only with `--declared-fees-lamports`**, the sum of that
  cycle's own transaction fees, which the simulator does not supply because its
  census payer and its trade payer may differ. Pass it by hand from the
  signatures if they can be named; otherwise report L7 inapplicable **by
  construction**, and never as a pass.

The simulator computes its declarations from `direct-trade-finalized.json`, not
from the public manifest, because the public manifest lacks `priceScale`.

**Report every law by name, L1 through L8, with its actual verdict.** An
INAPPLICABLE is not a pass, and a run that prints six greens and omits the two it
could not judge has reported a number it did not earn.

## Addendum: the recovery step is not disabled, it was never written — and this is the third time

Checked before declaring the wall, because "no path exists" and "I did not look"
read identically in a report.

`recovered_finalized_founding` is a real field on the execution schema
(`campaign.rs:163`, `4332`), it is serialized on two surfaces (`3823`, `4276`),
and a refusal guards it (`376`). **And it is assigned `false` at both of the only
two sites that set it** — `campaign.rs:3818` and `campaign.rs:4571`. Nothing in
the tree ever sets it true.

So the reader, the schema and the refusal all exist, and the WRITER does not.
There is no recovery path to enable, no flag to pass, no disabled branch: the
field is a placeholder for a step that was anticipated and never built, and
cohort-13 is the first cohort to need it.

**This is the third instance of this exact shape in this project**, and it is
worth naming as a pattern rather than a coincidence:

- Cohort-12's **Wall D**: `AlreadyCurrent` was validated, audited live and
  projected into the plan by the tree, and written by nothing in it. Closed by
  `28ff0823` giving the disposition a writer.
- `PERMANENT_DEVNET_UPGRADE_TARGETS_V1`: the journal re-read every row against
  the chain, and the constant it compared them to named a substrate that had been
  closed for cohorts. Closed by `8e1f9850` making the set an authenticated input.
- Now `recovered_finalized_founding`: a refusal that names its own repair —
  *"a separate recovery-to-complete step must reconstruct and authenticate
  execution.market"* — with no such step in the tree.

The pattern: **a consumer, a schema and a refusal can all be built and reviewed
without anyone noticing that the producer is missing**, because every one of them
is exercised by the failure path and none of them by the success path. A field
whose only assignments are a literal `false` is the cheap detector, and it is
greppable.

Both earlier instances were closed within a day of being named, which is the
reason to name this one precisely rather than to describe it as "the founding
failed".

## Correction: `2dGxuxe5…` is the founding-permit root, not the activation root

This file first named `2dGxuxe5LGdckG9r3co9u57MbMzoT5xJJTipUysgA261` as "the Direct
capability root, vacant — correct, activation creates it". **That is wrong, and
the error is worth recording because the vacancy it reported is uninformative.**

The web lane, deriving from the Market's own capability manifest entry 0 under
the Trading program, got a different address — `4GzDzNxj248uBkNLxKN2ffVzZ6cFZy158mVCeLec6ufz`
— and refused to let two authorities disagree about a derived address. Both are
vacant today, so nothing on any surface is currently false; but exactly one is
what activation creates, and shipping the other means a wall that turns FALSE the
moment activation lands, which is worse than the wall we have.

The code answers it directly, `direct_capability_activation.rs:296`:

> Not to be confused with the founding checkpoint's `direct_capability_root`,
> which is the **FOUNDING-PERMIT namespace address and at which no account can
> ever exist**. This is the address activation creates and the address the
> terminal sequence means.

So `2dGxuxe5…` is cohort-13's founding-permit root. It is vacant permanently, by
construction, and its vacancy is evidence of nothing.

**Positive control, from the one cohort that actually activated.** Cohort-12:

| | address |
| --- | --- |
| founding checkpoint `direct_capability_root` | `8TfFY4236bjxW8N17jqfP1rcU4eNyNNBSUc69awgmEyL` |
| the root activation actually created | `88jJTMmUGr4tB92SwAVpNnQ5CYnWYsg19cu3ULgrZmd4` |

Different addresses — and cohort-12's own activation report names them separately,
`foundingPermitRoot` and `root`. Three independent sources agree.

The activation root is `direct_execution_root_v1(trading, release_set, market,
generation, entry_index, manifest_body)`: `find_program_address` over a
`CapabilityRootHeaderV1` built from the release set, the Market, the generation
and a `CapabilityExecutionSelectionV1` carrying the entry index,
`sha256(manifest_body)` and the entry's kind, release and config ids — under the
**Trading** program. The browser's spine uses the same authors, which is why the
two must land on one address; the remaining way to differ is an input, and
**generation is the trap**: this Market is generation **2**, and the founding
record beside it is generation 1.

**The lesson, which is AGENTS.md's probe rule one level up.** *"A probe measures
what it touches, not what you meant."* I had the right instrument — a chain read
of an account — pointed at the wrong account, and the reading was confidently
reported. What caught it was not a better probe but a SECOND AUTHOR deriving the
same fact independently and refusing to reconcile by preference. When activation
lands, the account at the manifest-derived address becomes occupied, and that is
the cross-check that closes this for free.

### The settled activation root, with the inputs that decide it

Confirmed by the web lane off the live spine, and the generation confirmed a
second time from the Market's raw bytes rather than from the spine's own answer:

| input | value |
| --- | --- |
| trading program | `HkNhMJrERGko9mFXKq6UaL8qu2QnzqJx1hwJ5U8AVUHZ` |
| generation | **2** (`generation@272`, read twice by two instruments) |
| release set | `82a969ddbcd1782aab65016632742e4dd956978dc5e3a8f0ba0f853e0c13c62c` (`releaseSet@208`) |
| entry index | 0 |
| manifest record | `2hN3F4vsarGvDgjdmwfrtjDZrEwxeB6xN83VSFNwgdU5` |
| program set id | `c6d185d8e675c5bc62f27084e08dcaa8237338be7d158606ead819c86f92f9f2` |
| config id | `456733e9fc75fe614df2f3d689431c6422bbe0ea6678b0b3d3762efec01a19ce` |
| **activation root** | **`4GzDzNxj248uBkNLxKN2ffVzZ6cFZy158mVCeLec6ufz`** |

Every one of these is read off the Market itself or the manifest that Market
names — none is taken from a campaign report.

**The pending cross-check, stated before it is run so it cannot be reinterpreted
afterwards:** when the recovery step lands and activation executes, the account
at `4GzDzNxj…` must become **occupied**. If it is still vacant, the two
derivations disagreed for a reason we have not found, and the correct action is
to stop rather than to report a successful activation.

### Why the first check looked fine, which is the reusable part

Verifying that `2dGxuxe5…` was vacant was a TRUE observation that meant nothing,
and from outside there is no way to tell a vacancy that is guaranteed by
construction from a vacancy that reports work not yet done. They log identically.

What made it decidable was not a better read of that account but **an activated
cohort to compare against** — cohort-12, whose checkpoint field and whose actual
activation root are different addresses, and whose own report names them
separately. That is the same lesson as *absent signal needs a positive control*,
sharpened: the control has to be a case where the thing DID fire, or a
permanently-dead instrument and a correctly-quiet one are indistinguishable.

### A third author, and the route is no longer the unknown

The Direct lane landed `canonical_activation_creates_the_direct_root_through_real_core_and_trading`
(`5b2565ad`, `programs/dclutch-core-sbf/tests/capability_close_alias_program_test.rs`):
the same frame `devnet-direct-capability-activation-v1` builds —
`CapabilityRouteLayoutV1::new(1, 18)`, 35 accounts, the selection taken from the
manifest entry, **the root derived from that selection's header seeds** — run on
real Core, Trading and Registry ELFs from a vacant root and a Pending ledger.
329,736 CU. The root is created Trading-owned, its bytes exactly
`CapabilityRootHeaderV1 || DirectRootStateV1::new()`, the ledger reaches Active
with zero rent remaining, and `outstanding_capabilities` goes 0 → 1.

So three independent authors now agree on where the activation root comes from —
the activation driver, the browser's trade spine, and a program test on real
ELFs — and all three take it from the manifest selection, none from the founding
checkpoint's permit field.

**What that changes for cohort-13:** the route is no longer an unknown, and the
remaining blocker is narrowed to exactly one thing — the campaign report's
missing `execution` block. When the recovery-to-complete step supplies it, the
instruction the activation then builds is one this repository has executed end to
end on real bytes.

## The five routing tables, read back by address — the `dc07c73a` freeze holds

Recorded as pending earlier in this file because `routing-readback.py` reads
`evidence["execution"]`, which the report lacks. It does not have to: the
addresses are recoverable from the founding's own ALT transactions, which is the
discipline `8fda79bf` established anyway — **read by address, never by scan**,
because `getProgramAccounts` over the ALT program answers an *absence* on devnet
rather than a refusal.

Re-read at observation slot **491,972,453**, every one owned by the Address
Lookup Table program, not executable, authority **None**, `deactivation_slot`
`18446744073709551615` (`u64::MAX`), last extended strictly before the
observation:

| table | label (by creation order and width) | last extended | addresses |
| --- | --- | ---: | ---: |
| `GgCA8HGhfArdD6KubXgYmvc1F6vs5NLHSHdmYZePoMKQ` | Found37 | 491,960,315 | 35 |
| `7TctGEa6EBQAeAHZhBtMGdbEhar4UnxaJFwGWnY84aF5` | DCLTCFQ1 | 491,961,265 | 45 |
| `7Xqw7YBnedX4wYUBkiZeKBpNuMZvYjjnmk653MLuBpSq` | DCLTCF1A | 491,961,512 | 15 |
| `3439rxEAeXQ4U9DZKDCD1s7ys4BwF6AcWZ9qRLu6iygL` | DCLTPCB2 | 491,961,772 | 56 |
| `8DjFdk2J5BQVjVw76xYkHsjL8ACWJD3A7BmdTwKJS72w` | DCLTGMF3 | 491,962,871 | 62 |

**All five frozen.** The address counts are 35 / 45 / 15 / 56 / 62 — cohort-12's
five tables to the address, which is an independent check that the same five
tables were built for the same five frames.

### The instrument was broken and reported an absence

Worth writing down because it nearly became a finding. The first scan reported
**zero** ALT accounts, and it had a clean bill of health: 192 `getTransaction`
calls, 0 failures. The failure was in the PARSE, not the fetch — under
`jsonParsed` encoding a recognised instruction carries `parsed` and **no**
`accounts` array, so `ix.get("accounts")` was empty for every ALT instruction and
the loop skipped all of them. A count of program ids across the same
transactions showed 23 ALT instructions in a 60-transaction sample, which is what
exposed it; re-reading with raw `json` encoding, where instructions carry
`accounts` as indices, found all five immediately.

**"0 failed" was true and reassuring and measured the wrong stage.** The rule
this earns: when an instrument reports an absence, the health check must cover
the step that could produce a false absence — here parsing — and not just the
step that is easiest to count. A positive control (does this scan find ANY of the
thing, anywhere?) would have caught it in one run.

## Correction: the devnet General path exists now, and only the Trading bytes still block OpenBatch

This file recorded, and this lane reported, that General could not run on devnet
because there was **no devnet General market compiler at all** and because
`general_capability_activation.rs` refused every non-loopback origin. Both were
true when measured. **Both are now closed**, by the General lane, within hours —
and closed by exactly the work this lane's report named as owed.

| what was named as missing | what exists now |
| --- | --- |
| a devnet General market compiler | `general_devnet_market.rs::devnet_general_market_input` and `attach_devnet_general_capability_v1`, reached from the `devnet-general-market` command (`main.rs:1043`) |
| a devnet arm for capability activation | `general_capability_activation.rs::run_devnet` → `run_with_cluster_v1(_, ExpectedClusterV1::Devnet)` |
| the four deployment identities read rather than projected | the accelerator is deployed on devnet, and its release id is derived through `plan::release_facts` from the program, slot and digest the chain carries, not transcribed |

So the claim to carry forward is narrower and should be stated exactly:
**cohort-13 cannot run OpenBatch because its TRADING BYTES predate `a517d27c`**,
whose inline input transport lives in `programs/dclutch-trading-sbf`. That
blocker is a property of this cohort's deployed artifacts and no tooling closes
it. A cohort deployed from a commit at or after `a517d27c` now has both halves.

One correction the General lane made to itself is worth reading beside this,
because it is the same shape as this file's activation-root error: it expected
that swapping the accelerator would move an entry's `release_id` and leave
`config_id` alone, since `GeneralConfigV3` carries no deployment field. It does
not — the config binds `program_set_id`, the program set is downstream of the
certificate naming the accelerator, so one flipped bit of the artifact release
moves the whole entry. **The run corrected the test.** An expectation written
from the struct's fields rather than from the derivation's closure was wrong in
the direction that would have looked like a passing test.

## Addendum: the recovery step exists now, gets six stages further, and stops on the same shape

`00793136` — *"recovery: the founding that landed can finally say so, and the
flag that could not"* — built the producer this file recorded as missing, and it
works. Run against cohort-13's own report and chain, all six stages read
`already complete`, the poststate re-authentication passes, and the run reaches
the Open acknowledgement. **The flag is no longer written at all: it is DEFINED
as `recovery_to_complete.is_some()`, so a recovery that cannot say what it read
cannot present itself as a normal founding** — which is a better repair than the
one this file asked for.

It then refuses:

```
Error: Error("Open changed a Pending controller funding ledger while consuming its checkpoint")
```

`market.rs:12468`, the loop closing `authenticate_open_market_poststate_v1`. It
reads the funding ledgers **live** and requires them to still equal their
**Pending** bytes. This founding's own journals, in finalized-slot order:

| # | operation | finalized slot |
| --- | --- | ---: |
| 2 | `dcltgmf3` — the Open | 491,963,072 |
| 3 | `core-funding-create-v1` | 491,963,194 |
| 4 | `resolution-funding-activate-v1` | 491,963,281 |
| 5 | `core-funding-accept-v1` | 491,963,396 |

**Three finalized stages run after Open and exist precisely to move those ledgers
off Pending.** So the verifier reads live state for a fact three later stages of
the same founding superseded by design — which is `00793136`'s own point 2
verbatim, one verifier over and not swept.

The enclosing function was swept so the fix can be scoped in one pass: of its
eight checks, seven are facts no later stage can move (Market state, permit
vacancy, Position and admission allocation, Hoard principal, source closure,
replay revision, checkpoint consumption). **The funding-ledger loop is the whole
defect in that function.**

### The class, and why nothing caught it

`00793136` added a detector for booleans with no varying producer. **A boundary
invariant evaluated against LIVE state has no detector**, and it has now bitten
twice in one file within one commit.

The reason is structural and worth keeping: **the reconstruction path is the only
caller that runs these authenticators long after the boundary they describe.**
Every other caller runs them while live state and boundary state still coincide,
so the defect is invisible until something recovers. Nothing was wrong with the
tests; there was no caller that could fail.

Note also what the check is right about. Open must not change a Pending ledger
*while consuming its checkpoint* — a real invariant about ONE transaction. The
defect is the evaluation point, not the sentence, so the repair is to compare
against what the journal recorded and require every live difference to have a
named later owner, exactly as `00793136` did for poststates. Deleting the check
would be the wrong fix.

### And a hazard found while proving the refusal spent nothing

It spent nothing — balances unmoved to the lamport. But three copies of
`campaign-open.json` now differ, and **the copy written by the first refused
resume has `founding_targets` NULL**. A refused run rewrites the evidence file.

That is AGENTS.md's own rule under Project conduct: *"A failed generator must
leave the last accepted output byte-for-byte intact."* The campaign driver
writes its evidence before the authentication that can refuse. The current file
is healthy only because the recovery run restored the field before refusing —
luck, not design, and `founding_targets` names the Open Market the whole
recovery is for. The repair is the pattern this repository already documents and
its cut tool already follows: temporary file on the same filesystem, producer
exits zero, validate the shape, replace atomically.

## RESOLVED: the recovery landed, the market activated, and all eight conservation laws hold

Written 2026-09-02 by the COHORT-13 RESUME lane, at finalized slot 492,094,435.
**Devnet evidence. Not mainnet evidence.**

Two commits, and every act below is one of them reading the other's output:

| | |
| --- | --- |
| deployed cohort (unchanged, no byte moved) | `315f1931f4d6bb01510a3b78ccd056149e87367f` |
| host tools that read and repaired its evidence | `4d9b8d3fd8d8ac0e85dae3ea43861e214f24d0ae` |
| activation root, occupied | `4GzDzNxj248uBkNLxKN2ffVzZ6cFZy158mVCeLec6ufz` |
| the fill, finalized | `3FpQ2fSEph8WXovyYeoSG36ZwNGqenpVi3pEW4t2Xn64PQyCNhP5Cv4NCkuPxPcd4DxXqKyRtbtuc6HChYB1P2eJ` |

The deployer `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` is at
**32.47385185 SOL, unmoved to the lamport across this entire lane.** Everything
below was paid by the campaign payer, which went **1.663405281 → 1.524416622**:
**0.138988659 SOL** for the activation, the participants, two admissions, a
nine-transaction Direct session and the fee settlement.

### The third wall, and the producer gap behind it

`be012a46` fixed the Open verifier's clock and the recovery then ran to
completion — six stages, all `already complete`, **zero SOL**, balances unmoved.
But the report it wrote was still refused, one consumer over:

    recovery-to-complete named a DCLTCFQ1 signature its own transaction
    projection does not carry

This one was NOT another clock defect. `authenticate_recovery_to_complete_v1`
corroborates all six journal signatures against the report's own
`execution.transactions`, and it is right to: an `execution` block naming
journals no transaction row backs is assertion, not evidence. The gap was in the
PRODUCER. `recover_completed_market_from_checkpoint` republishes nothing by
design — `reconstruct_founding_checkpoint_v1` refuses its own republication by
count — so DCLTGMF3 had a projecting owner in
`finalize_existing_founding_submission_v1`, the three funding stages had one in
`execute_funding_readiness_suffix_v1`, and **the two stages before Open had none
at all. Six journals, four rows.**

`4d9b8d3f` supplies the missing owner. Both rows are read back off chain and
reauthenticated by `authenticate_historical_founding_transaction_v1` — the same
helper the readiness suffix already uses, which reparses the journal signature,
refetches the finalized packet and compares slot, packet digest, fee and compute
units against what the journal recorded. Its test asserts the PARTITION rather
than the fix: the stages before Open, Open itself, and the post-Open suffix must
together be exactly `ORDER`, so a seventh stage added anywhere goes red naming
the half whose owner is missing. Proved red before green by swapping the array's
two entries.

**The prediction in the resume brief was wrong, and usefully so.** The named next
candidate wall — `execute_funding_readiness_suffix_v1`'s unconditional
`authenticate_funding_readiness_route_v1(…, "accept")` — never fired. The live
plan short-circuited at `ConsumedByFounding` and the suffix reported
*"completed the post-Open V7 funding readiness suffix in exact order"*. The real
next wall was a consumer one function further on, in a different file, of a
different class.

Result, on the third run: **six transactions in the projection**, in canonical
order, every journal signature corroborated.

| # | operation | finalized slot | signature |
| --- | --- | ---: | --- |
| 1 | DCLTCFQ1 | 491,961,396 | `31Cb2kwwKq6xTaJC7DiptWT8hPvon3efibC1RN3XNk7VUTSA1FLcqkjbmgvWQEb8M6iRTXsaz53amd92MMPZEW68` |
| 2 | DCLTPCB2 | 491,962,044 | `2SLSaUPmp8VFG7fFmVST7ZifZoFgcnDdKDuGcGFeMPjoXm2jUSijHXaK1ZThBTjpSZimUHqY976p6nCXNQxCpppX` |
| 3 | DCLTGMF3 | 491,963,072 | `5Ji1babqGguizfSafscVnSpeXTjXE3GqpCteUF7PQJ1r6eAs2oizENwPoK3XooisHkgVgZTxeHHNk3qTcSM47jtj` |
| 4 | core-funding-create-v1 | 491,963,194 | `4ECUUXmKGe3gVoLdeUuE68KwLavN94enWQNeJE9vanHeX4J5Vbkh96jr3CxS6uYsUGBh8CCoXEXbGeLaee3QEpNo` |
| 5 | resolution-funding-activate-v1 | 491,963,281 | `2fowWQzpp1utLjjTo35GNXncAzEnxpUzkpLDye1CHmFhYQ4NeTGtwBi4HDbdD688cY4X9f5qbmqmQYySaiX65x4o` |
| 6 | core-funding-accept-v1 | 491,963,396 | `4gaisSBgnbccWHeAJYBVw74r2tVX4CDXWdFhzS41RWHY6SWuzuV5x94ZKcJZ2svsDP9EtLDTLFitKGmqbHs9qKKo` |

Balances after the recovery: deployer 32.47385185, payer 1.663405281 — **unmoved
to the lamport**, and `be012a46`'s promise held: the two refused runs left
`campaign-open.json` byte-identical (sha256 `70ac7a0d…` before and after).

### THE ACTIVATION DEADLINE WAS NOT 492,460,566 — read it from the command

`HOLD_STATE.md` carried **492,460,566**, about 42 hours at spawn. The activation
command's own report says:

    "activationDeadlineSlot": 492169598

At the preflight's observed slot 492,086,455 that was **83,143 slots, about 9.2
hours** — not 42. The same discipline the brief applies to the root applies to
the deadline: **read it from the command's own report, never from a remembered
number.** Had the lane trusted the remembered figure and taken the fallback's
leisurely path, the window would have closed while it waited.

### The activation, and its pre-registered cross-check

`devnet-direct-capability-activation-v1`, payer the campaign payer, four
transactions, `"verdict": "ACTIVATED"`.

| | slot |
| --- | ---: |
| `5SY8RPKi3mSivRUA5gLbQ8vNhEge7yFNk9swMJ9cCEozTERmMv15Qa9soL5xJEDHWenCasxkUo5ygkVm7uaa6SPF` | 492,086,895 |
| `5fJ9N1CyFVV3z5KyKEmm3fYYXyh856Ag3EoGvt6gZ4qppfTgZFECitMs7zfyW9AhjV5QRk85S4R8J9prjh5ZkiEU` | 492,086,936 |
| `PrQrxpsFbB7xTQkRmjGQnEw2zmqkh6gafdptAoLGxBwxoBUKqUGxC2rZdMYRuMUX119te8ZdLZbiYoyYmpHfLxM` | 492,086,975 |
| `12192UWhZdUAveWQiACXCPunmx7ouEMgit74rbZBuH1ZZcUuhs2kbJWjHGX3uMMJEbDrhrYVqTffDE3hQ3qv7cq1` | 492,087,015 |

Payer **1.663405281 → 1.656393297**, cost 0.007011984 SOL.

**The cross-check registered before the act is the one that closed it.** The
report's own `facts.root` is `4GzDzNxj248uBkNLxKN2ffVzZ6cFZy158mVCeLec6ufz` —
the same address two independent derivations named in advance. Read off chain
before the activation it was `AccountNotFound`; after, it is a **256-byte
`DCLTCRT1` record owned by Trading `HkNhMJrERGko9mFXKq6UaL8qu2QnzqJx1hwJ5U8AVUHZ`,
phase 0x01, 0.002431872 SOL rent**. Three authors, one address, and a vacancy
that became an occupancy across the one transaction set that should have caused
it.

`2dGxuxe5LGdckG9r3co9u57MbMzoT5xJJTipUysgA261` is still `AccountNotFound`, as it
will be forever. It remains evidence of nothing.

**A silent success nearly ate this step.** The first `--execute` printed
`"verdict": "planned"` and exited zero. `activate.sh` built its `--execute` flag
into an `EXTRA` array and **never passed it to the binary**, so `--execute` ran a
preflight and reported success having done nothing. That is AGENTS.md's
"Distrust silent success" exactly; the verdict string, not the exit code, is what
caught it.

### The owed readbacks

`market-readback.py` now runs, and both Core Market accounts read correctly —
including the trap:

| label | address | phase | readiness | generation | derived Claims aggregate |
| --- | --- | --- | --- | ---: | --- |
| `market` | `C4sCA56dDCuqoonU7spBS3j21tskD9DL754KiXPDmTXV` | 0x00 Founding | 0x00 Prepaid | 1 | `DddbwL8t…` VACANT |
| `founding_market` | `6t3ZnmRuxVKsB4NGrpiQurEwK52xSKVyNqY3tF1ner15` | **0x01 Open** | 0x02 Consumed | **2** | `HCnz8YXLnQdLgEBb8RAPjJg7R3Eh3qQx4oQiWmqGUhsc` EXISTS, 288 bytes |

`routing-readback.py` reports *"no routing table create transaction found in this
founding"*, and that is a correct output rather than a failure: it scans the
report's own transactions for ALT-program creates, and this founding created no
table — it CONSUMES three that the ladder froze earlier.

| stage | frozen table |
| --- | --- |
| DCLTCFQ1 | `7TctGEa6EBQAeAHZhBtMGdbEhar4UnxaJFwGWnY84aF5` |
| DCLTPCB2 | `3439rxEAeXQ4U9DZKDCD1s7ys4BwF6AcWZ9qRLu6iygL` |
| DCLTGMF3 and all three funding stages | `8DjFdk2J5BQVjVw76xYkHsjL8ACWJD3A7BmdTwKJS72w` |

The `dc07c73a` freeze proof for this cohort still reads against the earlier
section's five-table readback, not against this founding's report.

### The participants, and the admissions

Two 0.05 SOL transfers from the campaign payer, never the deployer:

| | signature |
| --- | --- |
| participant-1 `H1cYAJL3aNjLda7az96r13pDixnpUc2a7XYTE3dyWg4` | `3TzhgDpidBKxXGQEc46Yu3zxseRa466H5ozunHejeUfYsUoYxm7tCPPkJx4yxe7izGjUUkpnhq5hLxBPsBnsx58` |
| participant-2 `BVBriJDjsN7ZhGsJoJ3PET5FdkSKbcn7iDMAjA5tB6ZV` | `5HBw3au2KFXz4JSuJozYEenooxsfu5vFLvNDwMVJGHDHZgiEYRB5aLL21m7h3QTX9csUVZuM28mrrdQERWFdyjzx` |

Payer **1.656393297 → 1.556383297**.

Then both admissions, executed directly rather than preflighted — a preflight is
not free on this driver:

| | slot | CU | fee |
| --- | ---: | ---: | ---: |
| participant-1 `3crBKWVQszbx6eB1brWcyHF7wnjjnScJNzF6dBAbjxSFiv61sY2Gnp8GmzsHfq7ndCJ1XBYYwzJSed5h9JktkT2R` | 492,089,325 | 218,562 | 80,000 |
| participant-2 `2hZtmJwpdyCM9t3d9uAGpZRnmjMNTUXCTzH14PdY53xnDnhLZVPokKhdyWRJX176SuDdQbHDE9BVbS4Vv87wKXGv` | 492,089,485 | 200,562 | 80,000 |
| participant-2 collateral delegation `3dwjjekkT9QJqVcy3HVcHsu32vPES8RCK9xy1KpD1NcziBGXx7VicuuaYKh5vHpDRWa4DhpVbLS4aiTr24vmANCF` | 492,089,556 | 4,953 | 10,000 |

Payer **1.556383297 → 1.554337728**. The delegation put exactly **201 atoms** —
`required_buyer_collateral` — into `HJBvqz8qoUPemqDBwucnK7UgLYKsF978YNUxhqrNKkku`.

**The admission refused first for a reason worth recording**: the config named
`plan.json`, but cohort-13 is the first cohort founded from the SEALED plan and
the admission authenticates the report's own `plan_sha256`
(`760df09d…` = `plan-seal.json`). The refusal it earns is
*"campaign evidence schema, plan digest, or completed execution refused"* — one
coarse code over three conjuncts, which is why a path problem reads as a report
problem. This is the `map_err` cost AGENTS.md names, met in the wild.

### THE FILL: 1,286,187 CU, and the drift went the other way

`FILL_CU_RISK.md` predicted the ceiling might be hit, since cohort-13's Trading
ELF is 11,832 bytes larger than the build the 1,317,129 figure was measured on.
**It was not.** Recorded beside it, as asked:

| | CU | margin under 1,400,000 |
| --- | ---: | ---: |
| Direct lane's measurement, cohort-12 Trading `b0cff55a…` | 1,317,129 | 82,871 (5.9%) |
| **cohort-13, Trading `1b41f552…`, measured here** | **1,286,187** | **113,813 (8.1%)** |

**The drift is −30,942 CU across twenty-plus commits and a larger ELF.** A bigger
binary bought a cheaper crossing, so the next cohort inherits a measured number
rather than a lane's memory — and the honest reading is that this margin is still
thin enough to re-measure every cohort, not that the hazard is retired.

The whole nine-transaction session, one durable mutation per invocation:

| stage | slot | CU | fee | signature |
| --- | ---: | ---: | ---: | --- |
| replay-setup | 492,091,905 | 158,662 | 75,000 | `4gBoKbbQE2CFHVgM228DBC1RQHKmFQGT4Tnk64s26phRg13dudTR8SPB847xLF26mcCmfZT4xgRf1CFRnRfsgR1u` |
| token-setup | 492,092,002 | 108,800 | 75,000 | `4ekfpH6tuonx7wnfwQqy7ZjMqJw77AAwoVMB1HFrQGfxLAZn89Y9Lz2MHz8cezjAmYh9LqzAfX8GPRgVKroi9SD2` |
| lookup-create | 492,092,120 | 10,508 | 5,000 | `Yw4QP62va8YQ1XttquRsJpavv1DaMTWxAHgxjsTKWrC8usnMQ5oUM5aB9cDvHQwDoEsAYEAp3Z94mJthnqYxSyF` |
| lookup-extend | 492,092,232 | 11,657 | 5,000 | `5pB6yGEni6m4veTQC1vnma2ehQqhijQRyhpWU7EdreaJJ6SsqtAUxmLnv41zcPxd2cLpdJqw3a8JyQFJxWqckhD8` |
| lookup-extend | 492,092,355 | 11,660 | 5,000 | `3EemkrmwuwuVwRtpU32CjAMosWRAW2Ye22dzh1sVcvjSpvWUetEHcB6j9A5yDz7aazFnxyx4KLKcJTYrrJgsyvGo` |
| lookup-extend | 492,092,476 | 10,780 | 5,000 | `4EpvmiiS7UkMWeKvLLG5vQZesfyxcarxeFc5suuboSmCT8XpwWmxyaorn2T4LnM9k7weVAKkEqUKxyfuuPTxnNg3` |
| lookup-freeze | 492,092,595 | 1,517 | 5,000 | `43bNZSWK8EoAGx1aswvSvK8Nb4x8rqyjy2deLZg61YgpYZD5b9TbhagAp61fHo1LWhRv5T7LotKnNyYbnVPubMFx` |
| lookup-activation | 492,092,667 | — | — | (no transaction; a wait on activation) |
| capability-seal | 492,092,785 | 738,892 | 5,000 | `4Yi8YHmYd7MceNAZPqGyGJ9SgjYsdZJwnpLf4go9yGAaj8VgdgeuTKcG1gWfBQhU55pAxzp8hyMGNDgyodr7cqN8` |
| **hot (THE FILL)** | **492,092,896** | **1,286,187** | 15,000 | `3FpQ2fSEph8WXovyYeoSG36ZwNGqenpVi3pEW4t2Xn64PQyCNhP5Cv4NCkuPxPcd4DxXqKyRtbtuc6HChYB1P2eJ` |

**2,338,663 CU and 195,000 lamports across the session**, payer
**1.554337728 → 1.524491622**. The Hot packet is 1,167 wire bytes, 61 unique
message accounts, 57 loaded through lookup table `DR6YzUoEXQsPEPn9dMZsgsY3wg2paDmGv4Qg4Cgz6cz7`.

The terms, from the session's own finalized evidence: fill **200** at price
1,000,000 over scale 1,000,000 — gross **200** — outcome **0**, fee **50 bps per
side**, seller the founder `FBYW95Fo…`, buyer participant-2. Exactly the
fee-bearing crossing at the smallest gross whose fee does not floor to zero, and
nothing was retried at gross 199.

**One host-tool defect, after the fill was already finalized.** The session's
terminal step publishes `direct-trade-finalized.json` through
`write_create_only_json_v1`, which publishes by `fs::hard_link` precisely so an
existing evidence file can never be overwritten — then publishes the same path a
second time in the same invocation and trips its own guard:

    publish Direct evidence: File exists (os error 17)

The three files share one mtime (13:49:13) and the failing process left its
`.direct-trade-finalized.json.direct-evidence-55564.tmp` behind, which is what
identifies it as a double publish rather than a collision with an earlier run.
The evidence written was complete and authentic, so rerunning the simulator found
the session already complete and continued. **The guard is right; publishing
twice is the defect.** It is the same shape as the two walls above — a correct
invariant evaluated at a moment its author did not intend — and it is the reason
a fill that had fully landed reported as a refusal.

### The fee settlement

`devnet-direct-fee-settlement-v1`, permissionless, no party to the trade signing.
The obligation was read off chain and matched the prediction to the atom:
debtor participant-2, maker replay `86XRnuxX7ZN64eqchJ8oAYw2xLib5VFiq6hxqtzGAo51`,
**fee_owed 2**, standing allowance 2, destination the venue fee PDA
`2o8RqauePumkdm8yuEgd3aBm81xyr2XLBxarMgn1BVCs` owned by
`GCeAKFmCXgkCa5ebDGXQ1q8VEaS5z8oNe9JRWgGtTa76`, custody revision 2 → 3.

    signature      ChTAyLg6LtLWK1uQLE65SKifwqq9eKnZXPSJWmv4bFQRqUGKuQ4Qx2VVy6WKRQRAamV1TzfudUbiV36FVJ6MC2x
    slot           492,094,058
    compute units  151,913        fee 75,000 lamports
    fee_owed after 0 (read back from chain)

Payer **1.524491622 → 1.524416622**. `fee_owed after 0`, read back, is the only
thing that distinguishes a settled fee from a sent transaction.

**`settle-fee.sh` had never been run once, under any arguments.** An apostrophe
in `producer's` inside a `${2:?…}` message inside a double-quoted string is a
PARSE error in bash 3.2 — *"line 28: unexpected EOF while looking for matching
'"* — so the script could not be executed at all. A script written and never
executed looks identical to one that works until the moment it is needed.

### THE CENSUS: L1 through L8, every one by name, none inapplicable

At stage `cohort13-post-fee-settlement`, finalized slot 492,094,312, chained
through `--prior` to the post-fill boundary, from the observer build at
`4d9b8d3f`:

| law | verdict |
| --- | --- |
| **L1** | HOLDS — tracked 1,000,000,000 atoms across 5 accounts == Mint supply 1,000,000,000 |
| **L2** | HOLDS — the Hoard moved 0 atoms, exactly as declared; it holds 500,000,000 |
| **L3** | HOLDS — 3 Positions sum to the aggregate supply vector [500000000, 500000000, 500000000, 500000000] |
| **L4** | HOLDS — Hoard 500,000,000 >= worst outcome 500,000,000 x unit 1 |
| **L5** | HOLDS — tracked collateral moved 0 atoms, exactly as declared |
| **L6** | HOLDS — no watched account closed at this boundary |
| **L7** | HOLDS — the payer moved −75,000 lamports, its transactions paid 75,000 in fees, watched accounts gained 0, 0 went to nothing unwatched; debit == credit + fee |
| **L8** | HOLDS — every compartment moved exactly as declared: unclassified +0 |

Declarations: `--declared-collateral-delta 0`, `--declared-hoard-delta 0`,
`--declared-class-delta unclassified=0`, `--declared-class-delta HoardPrincipal=0`,
`--declared-fees-lamports 75000`.

**`CENSUS_L8_FINDING.md`'s premise is now obsolete, and it should be read as
history.** It said L8 is INAPPLICABLE by construction because `ledger-census`
hard-codes `ClassClaimV1::inapplicable` and the simulator passes no declarations.
**`bf59126d` landed both halves** — the `--declared-class-delta LABEL=ATOMS` flag
and the simulator wiring that computes the terms from the session's own finalized
evidence. So this is the first cohort in which L8 judges a claim across a real
fill instead of sitting out, and the first with **no INAPPLICABLE anywhere in
L1..L8**. An INAPPLICABLE is not a pass, and this sweep has none to excuse.

The state the laws are about, at that boundary:

| account | atoms |
| --- | ---: |
| founder collateral wallet `AWWxWQ2xUm86FkKcT8gXkSDiezwrAw9rJm6ED3xdeArq` | 499,999,799 |
| Hoard `8PMHP6cweSPjqpQmQurstNXKcBB855t6hXELePzdibY3` | 500,000,000 |
| seller Direct token PDA `3ir66Yi6LsLdoJD68msEeBh7xaVJ5zWUPMMRgmcRVqFU` | **199** |
| venue fee Direct token PDA `2o8RqauePumkdm8yuEgd3aBm81xyr2XLBxarMgn1BVCs` | **2** |
| buyer delegated collateral `HJBvqz8qoUPemqDBwucnK7UgLYKsF978YNUxhqrNKkku` | **0** |

199 + 2 = 201, the buyer's allowance spent exactly to zero; 199 is gross 200 less
the seller's fee of 1, and the 2 in the venue account is both sides' fee. The
claims moved with it: the founder's outcome-0 balance went 500,000,000 →
**499,999,800** and participant-2's went 0 → **200**, while outcomes 1, 2 and 3
did not move at all.

### The census bindings, and the two halts that were the instrument working

The first census VIOLATED L1 by **exactly the delegated amount**:

    tracked 999999799 atoms across 2 accounts != Mint supply 1000000000;
    201 atoms are in accounts this ledger does not name

999,999,799 + 201 = 1,000,000,000 to the atom. `build-sim-config.py` tried to read
the buyer's delegated account and the participants' Positions out of the
FOUNDING's accounts map, where neither can ever appear — the admission is what
creates them — so both lookups could only ever `KeyError` and both bindings were
silently absent. The builder now reads the landed admission reports, which is the
only author of those two facts.

The second census then VIOLATED L5 by the same 201, and this one is not a defect
at all: the tracked SET grew between the two boundaries, so a chained delta
compares two different apertures. `bf59126d`'s own message predicts it — *"a
buyer whose collateral source is unbound makes the tracked set grow by the atoms
that left it, and declaring zero there would red L5 against a claim that was
never true."* The clean baseline is therefore the complete-bindings census, and
the fill was judged from it.

Both halts were archived under `halts/`, never deleted. **Neither was a
conservation breach; both were the ledger correctly refusing to certify a set it
could not account for.**

### Cost, against the budget

| step | payer before | payer after | cost |
| --- | ---: | ---: | ---: |
| recovery (two refusals + the run) | 1.663405281 | 1.663405281 | **0** |
| activation | 1.663405281 | 1.656393297 | 0.007011984 |
| fund participants | 1.656393297 | 1.556383297 | 0.100010000 |
| admissions + delegation | 1.556383297 | 1.554337728 | 0.002045569 |
| Direct session (9 transactions) | 1.554337728 | 1.524491622 | 0.029846106 |
| fee settlement | 1.524491622 | 1.524416622 | 0.000075000 |
| | | **total** | **0.138988659** |

Against a ceiling of 2 SOL beyond what `HOLD_STATE.md` priced. The deployer never
paid for any of it and never moved.

### What this cohort now has that no previous cohort had

- A founding whose seal and whose founded plan carry the same release-set
  identity, **and** whose evidence file is whole enough for every consumer.
- An activated Direct capability at a root three independent authors named
  before it existed.
- A **fee-bearing** Direct crossing that landed, with its compute measured
  against the prior build rather than remembered.
- A fee settled to `fee_owed 0`, read back from chain.
- **L1 through L8, all HOLDS, no INAPPLICABLE**, across that fill.

## Addendum: RESOLUTION — the window closed unobserved, and the only reachable terminal is the failure walk

The resolution lane opened at 18:00 UTC on 2026-09-02 and read the window off
chain before anything else. **The window had already closed, nine minutes
earlier.** Everything below follows from that one fact, and the useful part of
this section is not the outcome but the four conjuncts that decide it.

### The window, read from its own account

`window_spec_record` `4CM5e6Eq7nXcAnYbEnhP9Jqfkp6BrsYdokks5dLwQCEu`, 112 bytes,
owned by Registry `8XsxVn35…`, magic `DCLTWIN1`:

| field | offset | value |
| --- | ---: | --- |
| kind | 10 | `1` = `WindowKind::Terminal` |
| source_spec_id | 16 | `09d17c26…6287d` (= the Source spec record's own digest) |
| start_unix_seconds | 48 | **1788369759** = 2026-09-02 **17:22:39 UTC** / 13:22:39 EDT |
| end_unix_seconds | 56 | **1788371559** = 2026-09-02 **17:52:39 UTC** / 13:52:39 EDT |
| max_age_seconds | 64 | **7200** |
| max_future_skew_seconds | 68 | 1 |
| schedule_id | 72 | `e7b6b794…919c` |
| cadence_tolerance_seconds | 104 | **0** |

Width exactly 1,800 s, as founded. The lane's briefing carried "around
15:00–16:00 EDT"; the account says 13:22:39–13:52:39 EDT. **Read the window from
the account, never from a handoff** — this is the third time in this cohort that
a remembered number and a read one disagreed, after the activation deadline and
the activation root.

### Whether the protocol admits a post-window observation: it does, and it cannot help

`crates/dclutch-source-contract/src/lib.rs:2146-2158`, `NormalizedEvidenceV1::validate`,
carries **two different clocks**, and only one of them is about lateness:

- `window.contains_observation(self.observation_unix_seconds)` — at
  `cadence_tolerance_seconds == 0` this is exactly the closed interval
  `[1788369759, 1788371559]`; else `Error::InvalidObservationSchedule`.
- `self.publication_unix_seconds` within `[clock − max_age, clock + max_future_skew]`
  = `[clock − 7200, clock + 1]`; else `Error::InvalidPublicationTime`.

So a submission *after* the window is admissible — the protocol never demanded
that the transaction land inside the window. What it demands is an observation
**whose own timestamp** is inside it. The relay is a snapshot of one fixed
mutable account, and that account had moved on.

`PythSponsoredPushReleaseV1` pins the **exact** `price_account`
(`crates/dclutch-pyth-svm/src/sponsored_push.rs`, field `price_account`), and the
market's copy of that release — record `5YdUbRHreVcLzBP9D6WAU8jZc7ncHD4iXPyjL8iCua9i`,
592 bytes — carries `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` at byte 240.
Read at 18:02:55 UTC that account held `publish_time` **1788372175**, 616 seconds
past the window's end, at **$99.337** (price 9,933,701,838, expo −8, conf
±$0.0212, posted slot 491,921,765). There is no second account the market would
accept and no way to make the sponsored one publish backwards. **The honest
observation for this window is permanently unavailable** — not late, not
missing-but-recoverable: unreachable.

Every one of those six Pyth addresses was read back out of the on-chain release
record rather than typed from the runbook.

### What the protocol does instead, and where its deadline is

`programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs` states the whole
lifecycle in its own module doc: *"Candidate admission closes at
`window.end + max_age`; settlement and head-vacant funded failure begin only
after that same strict deadline."* The two conjuncts sit eleven hundred lines
apart and point opposite ways:

| action | line | conjunct | refusal |
| --- | ---: | --- | --- |
| `Capture` | 131 | `clock.unix_timestamp > primary_deadline` | `ProviderFreshness` |
| `Settle` / `CommitFailure` | 875 | `clock.unix_timestamp <= deadline` | `ProviderFreshness` |

`primary_deadline = end + max_age = 1788371559 + 7200 =` **1788378759**
= 19:52:39 UTC / **15:52:39 EDT**. The last second a candidate could be admitted
and the first second a terminal could be committed are adjacent and disjoint.

`SourceResolutionStateV2::exhaust_after_primary_deadline` refuses a market whose
material carries a recovery policy (`RecoveryNotExhausted`) — *"a policy means
the market bought named alternative sources, and skipping them would take an
outcome away from the holders who paid for them."* This market bought none:
`source_material_record` `2nomjpYBmbJu37apysDdpuhACa955yACNvz3bW2fgGAq`,
`DCLTSMV3`, has `RECOVERY_PRESENT` byte 10 = **0** and a zero recovery-policy
digest. So the walk is open, and it is the only thing that is.

### The griefing vector this exposes, which is the finding worth keeping

Between a window closing and its primary deadline — here a **two-hour** gap, and
in general exactly `max_age_seconds` — capture is still legal, permissionless,
and **does not check window admissibility**. `process_capture` builds the
candidate from whatever `publish_time` the account holds, then
`initialize_head_account` runs on any vacant head (lines 175-212). Nothing in
that path consults `contains_observation`.

One such transaction is enough to strand the market permanently:

| route | conjunct | refusal |
| --- | --- | --- |
| `Settle` | candidate's `observation_unix_seconds` outside the window | `ProviderWindow` (mapped from `InvalidObservationSchedule` at `sponsored_push_v1.rs:1150`) |
| `CommitFailure` | head must be system-owned and `data_is_empty` (`:1662-1669`) | `SponsoredPush` |
| `CloseHead` | `terminal_source_for_cleanup` admits only `Resolved`/`FailureCommitted`/`Retired` (`:1445-1450`) | `Transition` |

Settle cannot consume it, failure cannot step around it, and cleanup cannot
remove it because cleanup requires the terminality that neither route can now
reach. **A market whose window closes unobserved has a `max_age_seconds`-wide
interval in which one permissionless capture makes it unresolvable forever.**
The lane's first operational decision was therefore *not* to capture, and the
head `FzUpoPVQdMFgxvkMbLVoTuJK5Nvsa33y82FyTmsBRcPs` was confirmed vacant before
and after every step.

This is not a defect in the failure walk; it is a missing conjunct in capture.
The cheap fix is to refuse a capture whose `publish_time` fails
`window.contains_observation`, which costs one call in a function that already
holds the window. It is queued as owed, not as done.

### The exterior input has no producer, so it was authored from chain

`devnet-sponsored-push-v1` consumes `dclutch-sponsored-push-exterior-input-v1`.
A sweep for that string finds the consumer (`sponsored_push.rs:55`), a validator
that checks a field of it (`tools/release/devnet-sponsored-keeper.py:45`), a
README example, and one doc — **and no producer anywhere in the tree.** This is
the producer-missing shape again: a reader, a schema and a refusal all built,
with nothing that writes the thing.

So the document was assembled from authenticated on-chain facts, and every field
carries its own check rather than a memory:

- Each of the eleven `raw`/`staging` pairs is
  `find_program_address([RAW_RECORD_PDA_SEED_V1 | STAGING_CURSOR_PDA_SEED_V1, schema, digest], registry)`
  (`provider_instruction_v3.rs:1037-1040`). The schema for each record was not
  looked up — it was **solved for**, by trying every 32-byte schema constant in
  `crates/` until one reproduced the record's known address from its own
  on-chain digest. All eleven solved uniquely, which makes each raw address a
  reproduction rather than a transcription, and all eleven staging accounts read
  back vacant.
- `activationCache` `HETBPPJFC7HbxmPdCnU4f1bmZEDABXG98b1KV1YfJnQg` is
  `find_program_address(["dclutch:release-activation:v1", release_set], registry)`
  (`relay_transport_v1.rs:294-297`), occupied, 1,288 bytes, registry-owned.
- `resolutionProgramdata` derived from the loader came out
  `D7nZhwBe81dN2RgnGinxz2YDYh9vFBWLos5Yd7aCBZDR`, **which is the exact string
  `observed-ids.tsv` recorded at deploy time** — an independent join between a
  derivation made now and a fact written seven hours earlier.

The routing table is routing and not authority: `authenticate_frozen_routing_table`
requires only ALT ownership, `authority == None`, `deactivation_slot == u64::MAX`,
`last_extended_slot < observation.slot`, and nonempty. Cohort-13's five frozen
founding tables all qualify; `3439rxEAeXQ4U9DZKDCD1s7ys4BwF6AcWZ9qRLu6iygL`
(DCLTPCB2, 56 addresses) was chosen because it covers 18 of the 29 accounts in
the failure frame, which is the smallest packet of the five.

The read-only preflight **planned**, which is what proves the hand-authored
document: 29 account metas resolved, instruction data
`DCLTSPI1` + action `5` + generation `2` + terminal sequence `1`, digest
`1049d3c4d1f5302baa9a02c30248302b9aaa7127ab8e54e88b17f5633145a955`, certificate
seat `7S9tCjXTHMPjtAoRq3uUPGDRfy2hG4rtQtAN5Uybd8Za` vacant, head vacant.

### Who owns what, before anything moved

The failure selector is `ResultDomainV2::failure_selector() = region_count`. With
cuts `9600, 10000` over denominator 100 there are three ordinary regions, four
outcomes, and **the failure outcome is selector 3**. All three Positions are
`DCLLBP02`, 160 bytes, Claims-owned, four claim slots at offset 128:

| holder | key | 0: < $96.00 | 1: $96.00–$100.00 | 2: > $100.00 | 3: FAILURE |
| --- | --- | ---: | ---: | ---: | ---: |
| founder | `FBYW95Fo…` | 499,999,800 | 500,000,000 | 500,000,000 | **500,000,000** |
| participant-1 | `H1cYAJL3…` | 0 | 0 | 0 | 0 |
| participant-2 | `BVBriJDj…` | **200** | 0 | 0 | 0 |
| aggregate `HCnz8YXL…` | — | 500,000,000 | 500,000,000 | 500,000,000 | 500,000,000 |

Every column sums to the aggregate exactly, which is the fill's conservation
re-read from the positions rather than from the trade's own report: the Direct
crossing sold participant-2 **200 atoms of outcome 0** and the founder is short
exactly that.

**So the failure walk pays the founder and pays the two strangers nothing.**
Participant-2 bought outcome 0 (SOL/USD below $96.00) and holds no failure
claims; participant-1 holds nothing at all. That is not the walk misbehaving — it
is precisely the property `exhaust_after_primary_deadline` documents, that *"a
silent provider cannot make a market unresolvable, only drive it to a
pre-disclosed outcome"* — but it should be said plainly, because the outcome a
buyer gets when the oracle goes quiet is the one the founder minted and kept.

For the record of what the honest outcome *would* have been: the sponsored feed
read $99.337 twelve minutes after the window closed, with a 5-minute EMA of
$98.60, both inside the $96.00–$100.00 cut — **outcome 1**, not the outcome
participant-2 bought. Hermes' historical endpoint now requires authorization
(HTTP 401), so this is a bracket from the live account either side of the
window, not a reading of the window itself, and it is labelled as one. It is
narrative, not evidence: nothing on chain will ever carry it.

### THE WALL: the failure certificate seat has no funder on the devnet path

The walk was armed to fire the instant it became legal and it did, at
19:52:52 UTC — thirteen seconds after the deadline. **It refused, `0x8002`,
after 305,522 CU.** Band 8 is Resolution, and `0x8002` is
`ResolutionError::OutputState` (`programs/dclutch-resolution-proof-sbf/src/lib.rs:49`).
Nothing moved: payer unchanged to the lamport, Source state still `Primary`,
certificate still vacant, funding ledger byte-identical.

`OutputState` is a coarse code — the exact idiom AGENTS.md names as this tree's
most expensive — but it localized in one step rather than by bisection, because
of what it *rules out*. Planning maps its failures through
`map_funded_walk_error` (`relay_transport_v1.rs:1291-1299`), and that function
cannot produce `OutputState` at all: `Request→Instruction`,
`Source→SourceMaterial`, `Product→ProductDomain`, `Transition→Transition`,
`Funding→Funding`. **So every walk semantic had already passed** — deadline,
material, absent recovery policy, product domain, escrow authentication, the
`Primary → Exhausted → FailureCommitted` transition, and the certificate's own
shape validation. Only the write could have refused, and the write is
`commit_deadline_failure` (`:1465`), which reaches `initialize_certificate_at_kind`
(`:1918`) first.

There the conjunct is `:1951-1957`:

```rust
if certificate.owner != &system_program::ID
    || certificate.data_len() != 0
    || certificate.lamports() < minimum      // minimum = rent.minimum_balance(312)
    || certificate.executable
{ return Err(ResolutionError::OutputState.into()); }
```

**The route allocates and assigns; it never funds.** The certificate seat must
already hold its own rent before the instruction runs. `7S9tCjXTHMPjtAoRq3uUPGDRfy2hG4rtQtAN5Uybd8Za`
held **zero**, and `getMinimumBalanceForRentExemption(312)` on devnet is
**2,786,520** lamports.

This is a caller obligation by design, and the tree says so in the one place it
is discharged: `tools/gauntlet/relayed-vertical/src/relayworld.rs:282` —
*"Prepay one certificate destination: the route allocates and assigns"* — and
`vertical.rs:792` prepays **both** destinations because success and failure are
different addresses. The harness fixture prepays it too
(`resolution_core_v3_lifecycle.rs:3978`).

**Nothing on the devnet path does.** `devnet-sponsored-push-v1` has five actions
and none of them is a prepay; the README's sponsored-push section documents the
routing table, the journal ladder, and the cleanup payer rule, and never mentions
that `settle` and `commit-failure` both land on an account the caller must fund
first. So the second producer gap in this lane sits directly behind the first:
the exterior input has no producer, and the seat that input's terminal actions
write to has no funder.

That it appeared on `commit-failure` is an accident of which terminal we
reached. **`settle` allocates its certificate through the same
`initialize_certificate_at_kind`**, so an honest resolution on this cohort would
have hit the identical refusal at the identical line — this is not a
failure-walk defect, it is the terminal seat, and it was in front of both doors.

Owed, in order of cost: `devnet-sponsored-push-v1` should either prepay the seat
in the terminal action's own transaction or refuse in **preflight** with a
message naming the account and the lamports. The preflight had the certificate
address and its `null` prestate in hand and reported `planned` anyway — a
preflight that plans an action the chain will certainly refuse is the same defect
class as a green build with a red umbrella.

### The walk, executed

The seat was prepaid by hand — exactly what `relayworld::prepay_certificate`
does — 2,786,520 lamports from the campaign payer, signature
`2xhst2Ud2A5Ck8MowhgY7MoqvxZbzS9vDUkRLGMTyFPPqFviwuHBb33tY1NifmPKsqS9Nq4XYSrC6gLHwYcaXXQw`.
Then the same command, unchanged, under a fresh output path:

| | |
| --- | --- |
| signature | `37Ye9gafsCMSJEjG99bJ5kcdC4ACpQYnTfEyFAbHaxoXNQavDJUZiVuWS52mBWafu9LJPik2bnD1eGwgGW3uasac` |
| slot | 492,139,257 |
| compute units | **311,799** (program 311,499) |
| fee | 75,000 lamports |
| inner instructions | two System invokes — the allocate and assign the seat was funded for |

**This is the first resolution of any kind on devnet.**

Poststates, each read back from chain by address:

- **Source state** `A71ZfJCq…`: phase `0 → 4` = **`FailureCommitted`**
  (`lib.rs:3135`); `terminal_route` byte 3; **`selector = 3`**, which is exactly
  `ResultDomainV2::failure_selector() = region_count`; `terminal_sequence = 1`;
  `resolved_at = 1788379028` (19:57:08 UTC); `resolution_evidence_id`
  `1cde2ee3…` = the Source material id, since a deadline failure is attributable
  to no provider.
- **Certificate** `7S9tCjXT…`: created, 312 bytes, Resolution-owned, magic
  `DCSRCER2`, **kind byte 4 = `ResolutionFailure`**. `route` and
  `provider_evidence` are **all zero** — *"this terminal is not attributable to
  any provider, which is the whole content of the claim 'the relayer went
  silent'"* (`funded.rs:356-361`). `funding_allocation` = the material id;
  `receipt_account` decodes to `7S9tCjXTHMPjtAoRq3uUPGDRfy2hG4rtQtAN5Uybd8Za`,
  its own address; generation 2; selector 3; sequence 1.
- **Funding ledger** `CP2F4fcy…`: 2,482,539 → **2,482,538**. Exactly **one
  lamport** left the escrow — the Failure row's bounty — and that row's status
  word went to zero while the other two rows are untouched.
- **Head** `FzUpoPVQ…`: still **vacant**. The walk never creates one, so the
  head-vacancy conjunct it depends on is still true afterwards.
- **Market** `6t3Znm…`: **unchanged**, phase byte 1 / readiness 2. Core has not
  consumed the certificate; that is the terminal sequence's job and it is the
  next owed step, not something the walk does.

**The lamports close exactly.** Campaign payer 1,524,416,622 → 1,521,550,103, a
delta of **2,866,519** = 2,786,520 prepay + 5,000 transfer fee + 75,000 walk fee
**− 1 lamport of bounty paid to the walker**. The escrow debit and the payer
credit are the same lamport, observed from both ends.

### What the outcome means, said plainly

Selector 3 won. The founder `FBYW95Fo…` holds 500,000,000 failure claims and is
owed the collateral; participant-2 holds 200 claims on outcome 0 and is owed
**nothing**; participant-1 holds nothing at all. The oracle went quiet inside a
thirty-minute window and the pre-disclosed consequence is that the market's
founder keeps the pot. The protocol did what it says it does. Whether a venue
should *sell* that shape to strangers is a product question this cohort has now
made concrete rather than hypothetical, and it belongs to whoever owns the
failure policy — the honest reading is that a failure outcome wholly owned by the
founder converts an oracle outage into founder revenue.

### Owed after this

1. **Prepay, or refuse in preflight** — the seat gap above. Cheapest real fix.
2. **A capture conjunct** — refuse an out-of-window `publish_time` at capture,
   closing the strand-by-grief interval.
3. **A producer for `dclutch-sponsored-push-exterior-input-v1`.** The document
   this lane hand-authored is checkable (every address reproduced from chain) but
   it should not have to be hand-authored.
4. **Terminal sequence and payout.** The Market is still Open with a committed
   failure certificate beside it; `devnet-terminal-sequence-v1` then
   `build_wallet_terminal_payout_v3` for the founder's 500,000,000 outcome-3
   claims. Not run here.
5. **Retirement is still owned-loopback only** — `AggregateRetirement`'s only
   command is `local-private-validator-aggregate-retirement-v1`, unchanged since
   cohort-12 named it. The market can now be resolved on devnet; it still cannot
   be retired there.

Devnet evidence. Not mainnet evidence.

### After the walk: two more walls, one of them fixed here

The certificate exists and the Source is `FailureCommitted`, but the **Market is
still Open**. Core has not been told. Two things stand between the two, and they
were found in order.

**Wall 1 — the terminal sequence cannot see the activation, and the refresh that
would show it refused a resolved world.** `devnet-terminal-sequence-v1` refuses
the founding evidence outright:

```
terminal sequence is blocked: the Direct first-use accounts are created together
by the first trade, so campaign evidence must carry all of them or none. It
carries direct_trading_funding_ledger and omits direct_capability_root.
```

That is correct and expected — the campaign report was written before activation,
and `docs/design/EVIDENCE_REFRESH_V1.md` §3 exists precisely because
`direct_capability_root` names two different addresses and only a refresh emits
the execution one. So: produce a refresh. It refused too:

```
REFUSED evidence refresh [refresh/immutable-moved]: immutable founding record
resolution_funding_ledger differs on chain from the founding campaign's sealed
row; this world is not that world
```

**It differs because the walk debited it, by design.** One lamport of bounty.
`evidence_refresh.rs` classified `resolution_funding_ledger` in
`IMMUTABLE_FOUNDING_RECORD_LABELS_V1` — the list whose contract is *"a refresh
MUST carry each of these byte-identical to its founding row"* — while
`ADVANCEABLE_FOUNDING_LABELS_V1`, whose own doc comment says it is *"deliberately
about a CLASS, not about one cause … every one of these is an account the
protocol is designed to advance between founding and resolution"*, listed
`direct_trading_funding_ledger` and not this one.

That is a **deadlock, not an ordering mistake**. Producing the refresh earlier
would not have helped: the same comment says being advanceable *"never buys
permission to differ from the chain: each row is still pinned byte-exact against
the live finalized account by the consumer that reads it"* — so a pre-walk
refresh would carry a pre-walk ledger row and fail the same equality against a
post-walk chain. Every terminal action writes this ledger; the deadline walk
debits the Failure row and settlement debits its own. A resolved market could
produce no refresh, and without a refresh no terminal sequence.

Fixed at its owner in `68f0b3da`: the label moved from immutable to advanceable,
with the cause written beside it, plus two tests —
`resolution_funding_ledger_is_advanceable_and_not_immutable` (whose first
assertion is exactly what was false before) and
`the_two_founding_label_classes_are_disjoint`, because the two lists are read as
a partition and an overlap would silently grant an immutable record permission to
move. **All 19 `evidence_refresh` tests pass**, including the three pre-existing
generic ones over both lists — none had hardcoded a count, which is why a
one-label move stayed a one-label move.

With the fix the refresh produces: 16 rows, `direct_capability_root` →
`4GzDzNxj248uBkNLxKN2ffVzZ6cFZy158mVCeLec6ufz`, the same execution root three
authors named before activation existed.

**Wall 2 — nothing on devnet projects a sponsored-push certificate to Core.**
With the refresh accepted, `devnet-terminal-sequence-v1` moves to its next
refusal:

```
REFUSED terminal sequence: terminal ALT projection requires the exact
terminal/retiring founding Market generation
```

The terminal sequence is the **post**-terminal ladder — it wants a Market already
Terminal or Retiring, and `terminal_sequence.rs` never mentions `AdmitTerminal`.
The step that would move it is `ResolutionCoreActionV1::AdmitTerminal`, *"project
an authenticated terminal Resolution certificate to Core"*. On devnet that call
is built in exactly one place — `flagship_resolution.rs:5166`,
`core_terminal_accept_report`, the `accept` stage of **`flagship-resolution-v1`**,
whose `PlanInputV1` demands `postUpdateBodyBase64`, `encodedVaa` and
`updateAccount`: the relayed-VAA provider coordinates, produced by
`--produce-input` from `--pyth-facts`. A market resolved through the
sponsored-push snapshot route has none of those and never will.

The capability is not missing — `tools/gauntlet/relayed-vertical/src/vertical.rs:1366`
does exactly this and its own assertion text reads *"AdmitTerminal, chain-derived
from the FailureCommitted Source, executed against the live validator. The Market
is Terminal, committed to the failure certificate's own …"*. It is the **devnet
arm** that is missing, and it is missing in the same shape as the other two gaps
this lane found: the reader exists, the semantics exist, the local path exercises
them, and no devnet producer reaches them.

Stated at its true strength: I did not attempt to coax `flagship-resolution-v1
--through accept` with a hand-built input that leaves the VAA fields unused, so
"impossible" is not proven — "there is no route that does not go through the
relayed-VAA producer" is. Whether `accept` is separable from its ladder is the
first thing the next owner should test, and it is cheap.

### Where cohort-13 now stands

| | |
| --- | --- |
| Source `A71ZfJCq…` | **FailureCommitted**, selector 3, sequence 1 |
| Failure certificate `7S9tCjXT…` | **written**, `DCSRCER2` kind 4, 312 bytes |
| Market `6t3Znm…` | **still Open** — Core has not admitted the certificate |
| Payout | not reachable until the Market is Terminal |
| Retirement | still owned-loopback only |

Total spent by this lane: **2,866,519 lamports** (0.00287 SOL) — the certificate
seat, one transfer fee, one walk fee, less one lamport of bounty received.
Campaign payer 1.524416622 → **1.521550103**. The deployer was never touched.

Devnet evidence. Not mainnet evidence.

## Addendum: TERMINAL AND PAID — the certificate reaches Core, the founder redeems, and one finding is retracted

The successor resolution lane opened at 20:15 UTC on 2026-09-02 against a Market
that was resolved and still Open. It closes with the Market **Terminal**, the
founder's winning failure position **paid**, and the previous addendum's
griefing finding **withdrawn as incorrect**. Four producer gaps were closed as
code; a fifth, in the browser, is measured and named rather than repaired.

### 1. Was `accept` separable from the relayed-VAA ladder? The builder yes, the command no

The previous addendum left this as the first thing to test, and it is settled in
both directions.

**The builder is separable.** `build_resolution_admit_terminal_v3`
(`crates/dclutch-resolution-core-v3-operator`) names no relayed-VAA coordinate
at all: its snapshot is the Market, the activation cache, the four deployment
programs, SourceMaterial, the capability manifest, the Source state, the funding
ledger, the certificate, Rent, and three product record pairs. Twenty-two
accounts, no signer meta, no CPI. `flagship-resolution-v1`'s `accept` stage
calls exactly this.

**The command is not.** Four independent refusals stand in front of that stage,
and `--through` reaches past none of them, because `--through` caps the LAST
stage run while the ENTRY stage always comes from `classify`:

| refusal | site |
| --- | --- |
| `encodedVaa` must be a nonzero pubkey | `SelectedInputV1::parse` |
| `updateAccount` must be a nonzero pubkey | `SelectedInputV1::parse` |
| `postUpdateBodyBase64` must parse as `PostUpdateParamsView` | `SelectedInputV1::parse` |
| `Accept` is reachable only from a **Consumed** provider lifecycle and a **Submitted** Receiver update | `classify` |

The last one is executable and now executed:
`sponsored_push_failure_state_is_not_an_accept_stage` feeds cohort-13's exact
facts — Market `Open`/`Consumed`, Source `FailureCommitted`, lifecycle vacant,
update vacant, certificate submitted — and asserts the canonical-stage refusal
by its text. Its positive control is in the same test: the **same Source phase**
with the relayed ladder's own lifecycle and update accounts still classifies as
`Accept`, so the refusal is about the missing VAA route and not about failure
resolution being unrepresentable.

So the arm went where the market's other five actions already live:
`devnet-sponsored-push-v1 --action admit-terminal`, committed at `f56b1d2d8`,
building the Core instruction through the same builder from the same finalized
snapshot the command already authenticates.

### 2. The Market is Terminal

Preflight planned 22 metas, one writable (the Market), no signers, program =
Core `HZsbUHHw…`, caller authority `FurcUgNomZopN5p97SF29WV2nkgzNRp87tdPYhmLMVXb`
at index 0, certificate `7S9tCjXT…` at index 14, 592 instruction bytes.

Executed **20:36 UTC**, signature
`29oFp7aru4qW52952cqdYLePooUcuoFWH4nQBkxvanDo4LyDgtNkNDAvZKET9FL9rNPSAtcLF3Z5hFiJQXeErGQV`,
slot **492,149,710**, **95,854 CU**, fee 75,000 lamports, `fee_only_balance_change`
true — the Market's rent did not move, only its body.

Read back from chain at slot 492,149,793, off the account rather than off the
report:

| field | offset | before | after |
| --- | ---: | --- | --- |
| `phase` | 10 | `1` = **Open** | `2` = **Terminal** |
| `readiness` | 11 | `2` = Consumed | `2` = Consumed |
| `terminal_winner` | 12 | `0` | **`3`** — the failure selector |
| `terminal_receipt` | 328 | all zero | **`7S9tCjXTHMPjtAoRq3uUPGDRfy2hG4rtQtAN5Uybd8Za`** |

The Market's body digest went `2564c367…` → `aa41381b…`. Core committed to the
failure certificate's own address, and to selector 3.

### 3. The payout, and the two walls in front of it

**Wall 3 — the Claims-role Custody replay had no public producer.** The payout
input producer refused: *"wallet payout snapshot is missing Claims Custody
replay `8iXLcqN5NAV3N1xHkeu1jDtxKUxqJ8qFQsNTdvNKrNue`"*. The route that creates
it exists and is complete — `claims_custody_replay.rs`, whose own module doc
says terminal payout *"decodes the Claims-role replay and deliberately does not
create it"* — and it admitted **only a validator this runner owns**. The same
producer-missing shape a fourth time. Fixed at `31d09aed2` by giving it the
public arm; the instruction is byte-identical between clusters, and what the
cluster decides is which origin is admitted, which campaign report is consumable
and which label the evidence carries.

Created: signature
`5JKdUXJurc4jjESgrTxZaPwA6zo9f3BKtCYzM2DkjE7RGPeHL7Z6qhP3WpF9v8b7SXeGxAjvb499ytxMu8UtkBhC`,
slot **492,151,322**, **174,589 CU**, rent 2,634,528 lamports, read back at
`next_revision 1` in the Claims role over custody context
`35afb096da2cbf6b2ea79d97f2aecf7c8f23a299dc7a07a34daeb5e556ed13dc` — which is
the same context the evidence refresh independently recorded as
`chain_persisted_custody_context`. Two derivations, one digest.

**Wall 4 — the conventional payout destination is unpayable under Token-2022.**
`dclutch-wallet-terminal-input-operator` documents the wallet's associated token
account as *"the conventional destination for a payout"* and derives it. Under
Token-2022 the ATA program always adds the `ImmutableOwner` extension, so that
account is **170 bytes**; `TokenAccount::parse` is documented as *"Parse exactly
165 bytes, refusing truncation and every extension suffix"*, and refuses. The
founder's ATA `4BENW7YgFnAbagoRr8rAdD8p2kqhwiyvtyDzNKj8ef6W` was created, read
back at 170 bytes, and the payout builder refused it as
`WalletTerminalPayoutErrorV3::Custody` — one code over a twenty-four-conjunct
disjunction, which is why localizing it took a byte-width comparison rather than
a reading. The refusal is not host-side only: `programs/dclutch-claims-sbf`'s
`rational_terminal_v3.rs:625` parses the recipient with the same function, so
the chain refuses it too.

Worked around, not repaired: a **165-byte auxiliary** Token-2022 account
`1JpBu46CZF37RtuiipZfmBQiuPU5voceu8NeqVDFd9T` was created for the founder
(`spl-token create-account` with an explicit keypair, not the ATA program). The
contradiction between "the conventional destination" and "every extension suffix
refuses" is left standing and named; it is the browser's default and therefore
every stranger's default.

**The payout.** Input produced by `wallet-terminal-payout-input`, quantity
**500,000,000** at claim index **3**, terminal certificate `7S9tCjXT…`. The
operator's own ladder then ran: one ALT created, extended twice and frozen
(`hH77XUWB5BGTN35JjKtmesPB6QS1hJoYv1QZLT2oZEA`, 33 addresses), then the payout.

Signature
`etUrsvhwZkueSdHam8dJ2v3AMKseXXCn3WgtNisbVXixkXP4HMXtU9BFHefyjgLmBCM3Kpkxo8JpLNuedhRArN2`,
slot **492,154,205**, **353,233 CU**, fee 10,000 lamports.

**Custody's move, read back from chain at slot 492,154,390:**

| account | before | after |
| --- | ---: | ---: |
| Hoard vault `8PMHP6cwe…` | 500,000,000 | **0** |
| founder token `1JpBu46…` | 0 | **500,000,000** |
| founder Position `HKw5Uzh…` claim 3 | 500,000,000 | **0** |
| founder Position claims 0/1/2 | 499,999,800 / 500,000,000 / 500,000,000 | unchanged |

The three losing columns are untouched on purpose: they are claims on outcomes
this market did not reach, and nothing retires them.

### 4. Participant-2 is paid zero, asserted from chain

Not skipped and not inferred. Both statements come from the same producer that
paid the founder, run read-only against the same finalized chain:

- At the **failure** selector: `wallet-terminal-payout-input --claim-index 3`
  for owner `BVBriJDj…` refuses *"payout quantity must be within 1..=0 atoms at
  claim index 3"*. Their failure balance is zero, so there is no quantity to
  name.
- At the outcome they actually bought: `--claim-index 0` produces an input at
  quantity **200**, and the payout preflight over it reports **`"payout": "0"`**,
  `keysOpened: false`, nothing sent. Two hundred atoms of a losing outcome are
  worth exactly nothing, computed by the operator rather than by this document.

Participant-1's Position `EeE6FETM…` reads `[0, 0, 0, 0]` — they hold nothing at
all, which was already true before resolution.

### 5. The browser's gated redemption test: it fails, and the byte is named

`apps/dclutch-web/lib/walletTerminalRedemption.live.test.ts` was written to be
run the day a market resolved. Run against this one —
`DCLUTCH_RESOLVED_MARKET=6t3Znm…`, owner the founder, endpoint Helius devnet —
its verdict is **red**, and not for the reason anyone expected:

```
Error: RPC response exceeds the browser byte bound
  at boundedJson lib/rpc.ts:332
  ... acquireFinalizedAccountsInChunksV1 lib/coreFound.ts:264
  ... deriveWalletTerminalPayoutInputV1 lib/walletTerminalInputSnapshot.ts:126
```

Localized exactly, by replaying the four rounds against the shipped wasm with
the byte cap removed:

| round | addresses | first chunk | verdict |
| --- | ---: | ---: | --- |
| one | 3 | 1,560 B | fine |
| two | 3 | 648 B | fine |
| three | 2 | 684 B | fine |
| **frame** | **38** | **5,269,020 B** | **exceeds `MAX_RPC_RESPONSE_BYTES` = 4,194,304** |

The frame round's first `getMultipleAccounts` chunk takes 32 of its 38
addresses, and those include the deployment's ProgramData accounts — the largest
single account in the frame is **1,369,758 bytes**. Three of them in one chunk
is over the cap. The chunker respects the RPC's 32-KEY bound and has no BYTE
bound of its own, so the split is by count where the constraint is by size.

Behind that, with the cap lifted, the same derivation reaches the **same
Token-2022 width refusal** as §3 when the destination is left to default — and
completes end to end, emitting a well-formed
`dclutch-wallet-terminal-payout-plan-input-v1`, when handed the 165-byte
recipient. So the browser path has exactly two blockers, both measured, neither
guessed: a chunker that splits by count instead of bytes, and a default
destination the protocol's own token decoder refuses.

Neither is repaired here. `apps/dclutch-web` and `crates/dclutch-token-svm` have
their own owners and both fixes are design decisions, not adjustments: chunking
by cumulative size (or fetching ProgramData with `dataSlice`), and whether the
protocol admits extension-suffixed token accounts at all.

### 6. The producers, closed as code

| gap | before | after |
| --- | --- | --- |
| Core `AdmitTerminal` on a sponsored-push market | only inside `flagship-resolution-v1`'s VAA ladder | `--action admit-terminal` (`f56b1d2d8`) |
| the Claims-role Custody replay | owned-loopback only | `devnet-claims-custody-replay-v1` (`31d09aed2`) |
| `dclutch-sponsored-push-exterior-input-v1` | **no producer anywhere** | `devnet-sponsored-push-input-v1` (`62a0b7fb5`) |
| the certificate seat's rent | gauntlet-local `prepay_certificate` only | `--action prepay-certificate --prepay-for …` (`62a0b7fb5`) |

Two checks on the last two are worth recording because they are the ones that
could have been faked:

- The **input producer reproduces the hand-authored document in all 42 fields**,
  including the `coreProgramdata` the admission arm needed. The previous lane
  solved each record's schema against its own digest until the known address
  reproduced; the command derives each pair from the sealed report's own
  `data_sha256` and refuses when the derived raw address is not the persisted
  one. Two independent authors, one document, zero differences. The emitted
  document is also handed back through the consumer's own `InputKeysV1::parse`
  before it is written, and producer and consumer share one `SponsoredInputV1`.
- The **prepay arm's arithmetic matches what the walk actually needed.** For the
  settle seat `BScEeMiRo5oggZNJAA3sPxCrxM4fGSg9nvF3UwjSJAei`, never taken, it
  plans a System transfer of **2,786,520 lamports** — to the lamport what the
  failure seat `7S9tCjXT…` holds on chain, which the previous lane had to
  transfer by hand after `0x8002`. For the failure seat itself it refuses:
  *"already occupied by `J33AaXnV…`; a prepay funds a seat the walk has not
  taken yet"*.

### 7. The griefing window is retracted

The first resolution addendum's stranding vector — one late permissionless
capture makes a market unresolvable forever — **does not exist**, and
`docs/design/SPONSORED_WINDOW_ADMISSION_2026_09_02.md` carries the retraction
with the call chain. In one line: `process_capture` reaches
`PythProviderAdapterObligationV2::normalize_authenticated_update`, which refuses
a `publish_time` outside `[window.start, window.end]` as `ProviderWindow` before
any candidate is built. The addendum's sentence *"nothing in that path consults
`contains_observation`"* is true as text and false as inference — the conjunct is
spelled inline, so a sweep for the symbol finds nothing, correctly.

Both repairs that addendum queued are withdrawn. What the retraction uncovered
and left owed is smaller and real: `cadence_tolerance_seconds` widens the window
on the multi-observation routes and is **inert** on the two single-snapshot Pyth
routes, while the narrow site's own doc comment claims to match the widened one.
Cohort-13 carried tolerance 0, where the two coincide. Queued to the lane that
owns `dclutch-source-contract`, with its gate named.

### 8. The ledger

Every transaction this lane signed, in order. Payer is the campaign payer
`7j8RHenZ…` throughout; the deployer was never opened.

| # | act | signature | slot | CU | fee | rent placed |
| --: | --- | --- | ---: | ---: | ---: | ---: |
| 1 | Core `AdmitTerminal` | `29oFp7ar…rGQV` | 492,149,710 | 95,854 | 75,000 | — |
| 2 | Claims-role Custody replay create | `5JKdUXJu…kBhC` | 492,151,322 | 174,589 | 75,000 | 2,634,528 |
| 3 | founder ATA create (170 B, unusable) | `5XZZoKNZ…J2gZ` | 492,152,448 | 18,014 | 5,000 | 1,887,234 |
| 4 | founder auxiliary token account (165 B) | `35BiumGH…XfvD` | 492,153,197 | 2,202 | 10,000 | 1,855,569 |
| 5 | payout ALT create | `2v72ET9r…BA7u` | 492,153,578 | 10,508 | 5,000 | 1,165,272 |
| 6 | payout ALT extend | `3BADkJxR…ZYXz` | 492,153,651 | 11,657 | 5,000 | 4,053,120 |
| 7 | payout ALT extend | `5vSbWPCQ…H2Cw` | 492,153,741 | 9,601 | 5,000 | 2,634,528 |
| 8 | payout ALT freeze | `GeCqKF44…yhh` | 492,153,871 | 1,517 | 5,000 | — |
| 9 | wallet terminal payout | `etUrsvhw…rN2` | 492,154,205 | 353,233 | 10,000 | — |

Balances:

| | |
| --- | --- |
| campaign payer before | **1,521,550,103** (1.521550103 SOL) |
| campaign payer after | **1,507,124,852** (1.507124852 SOL) |
| spent by this lane | **14,425,251** lamports = **0.014425251 SOL** |
| of which rent placed in accounts | 14,230,251 |
| of which transaction fees | 195,000 |
| deployer `4zrxtw5c…` | **32,473,851,850** — unmoved to the lamport |

The two figures close exactly: 14,230,251 + 195,000 = 14,425,251, and the fee
column sums to the same 195,000 (row 4 pays two signatures, because an auxiliary
token account signs with its own fresh keypair as well as the payer). Of the rent,
7,852,920 sits in the payout ALT and 1,887,234 in the unusable 170-byte ATA;
both are recoverable and neither is spent.

Where cohort-13 now stands:

| | |
| --- | --- |
| Market `6t3Znm…` | **Terminal**, `terminal_winner` 3, receipt `7S9tCjXT…` |
| Source `A71ZfJCq…` | FailureCommitted, selector 3, sequence 1 |
| Founder's failure position | **paid**, 500,000,000 atoms, Hoard vault emptied |
| Participant-2 | **paid zero**, asserted from chain at both claim indices |
| Browser redemption | red at a measured byte, with the exact chunk named |
| Retirement | still owned-loopback only |

Devnet evidence. Not mainnet evidence.

## Addendum: THE BROWSER'S TWO RED BYTES — one closed, one convicted, and the ATA is cohort-14's

The redemption-UX lane opened against the previous addendum's §5, which left the
browser's live redemption red at two measured bytes and repaired neither. One is
closed. The other turned out to have a third refusing site nobody had looked at,
and that site cannot be edited — only released again.

Tree root `/Users/ember/dev/dclutch`, commit `e0594084`.

### 1. The chunker split by the bound that did not bind

`acquireFinalizedAccountsInChunksV1` respected `getMultipleAccounts`'s 32-KEY
limit and had no byte bound of its own, so the frame round's first chunk was
**5,272,883 bytes** against `MAX_RPC_RESPONSE_BYTES` — reproduced exactly, to
within a slot's drift of the 5,269,020 the previous addendum recorded.

It now learns every address's length first, in one `dataSlice` round, and plans
chunks under both bounds. Two facts made that cheap, and both are measured
against devnet rather than assumed:

| | |
| --- | --- |
| `space` under a `dataSlice` | the account's FULL data length — a one-byte slice over the seven cohort-13 ProgramData accounts returned `2,320,197` for the largest |
| the whole frame's sizing round | **5,035 bytes** (3,947 + 1,088), against the 5.27 MB body chunk it plans |

Cohort-13's 38-address frame plans as **24 + 14 keys**:

| chunk | keys | predicted | measured | under 4 MiB |
| --- | ---: | ---: | ---: | --- |
| frame 0 | 24 | 2,685,808 | 2,681,669 | yes |
| frame 1 | 14 | 2,597,904 | 2,595,186 | yes |

The estimator is conservative on both, which is the direction that matters: a
planner that underestimates plans a chunk the node refuses. The measured frame —
every address and the length the node reported for it — is committed as
`fixtures/cohort13-terminal-frame-sizes-v1.json` in both trees, and the unit
test plans it.

### 2. The check that fix uncovered could not be satisfied by any node

Behind the byte bound stood `chunked finalized acquisition returned different
context slots`, which read as the strongest possible statement about a composite
read and was in fact **unsatisfiable**. `getMultipleAccounts` answers from the
node's current finalized bank, and devnet's advances while a round is in flight:

| attempt | chunk 0 slot | chunk 1 slot | elapsed |
| --- | ---: | ---: | ---: |
| 0 | 492,178,904 | 492,178,906 | 715 ms |
| 1 | 492,178,908 | 492,178,910 | 528 ms |
| 2 | 492,178,912 | 492,178,914 | 653 ms |
| 3 | 492,178,915 | 492,178,917 | 563 ms |

Two slots apart, four times out of four. It had never fired because every round
this client made until now fit in a single chunk, and the byte bound stopped the
first round that did not before its second chunk was ever requested.

The composite is now **proved** to be one picture instead of asserted to be:
every chunk but the last is read again after the last, at the greatest slot
observed, and must return byte for byte identical. A single-chunk round pays
nothing for this, which is every round but the frame.

### 3. The conventional destination: admitted twice, refused once

`TokenAccount::parse_base_or_immutable_owner` admits exactly the extension the
ATA program forces and nothing else, with a hostile that six other extension
types still refuse at the identical 170-byte width. Its test vector is the
founder's own ATA `4BENW7Yg…`, read off devnet: 170 bytes, account-type byte `2`,
extension `7` at length `0`.

| site | now |
| --- | --- |
| Claims `rational_terminal_v3.rs:625` | admits — and `terminal_settlement_v3` calls this one function instead of carrying a second copy of it |
| the host-side builder `wallet_terminal_payout_v3` | admits, and its poststate projection copies the suffix rather than dropping it, because the chain hashes the whole account |
| both browser wasm derivations | inherit the admission |
| **Custody's `ExactTransferProfileV1`** | **still refuses, and must** |

**Why Custody is not an edit.** Its
`ExtensionStoragePolicy::ExactBaseWidthsOnly` byte sits inside the
`CollateralAdapterReleaseV1` preimage; a realm stores the SHA-256 of that
preimage on chain as `collateral_adapter_release_id`; Custody selects a profile
by matching it. Widening the release under an unchanged id would make the tree
and the chain disagree about what one identity means, and would break every
operator read against every market founded under it — this one included. The
repair is a **third** adapter release beside the two that exist, selected by
realms a later cohort founds;
`docs/design/TOKEN_2022_IMMUTABLE_OWNER_DESTINATION_2026_09_02.md` carries its
shape.

Measured on real Claims, Custody and Token-2022 ELFs, in the Claims
program-test: with a 170-byte ATA-shaped destination the payout **builds,
submits, reaches Custody, spends 345,149 CU and refuses `0x6006`
(`CustodySbfError::TokenState`)** — with the Hoard, the destination balance, the
Position and the Custody cursor all asserted unmoved. Its control is the same
payout into a 165-byte destination, which commits. Before this lane the builder
refused *offline*, so no transaction existed and the chain was never asked.

**So cohort-13's auxiliary account was never a workaround.** Its deployed
Custody carries the old rule in shipped bytes; the 165-byte account created by
hand is the only destination that cohort could ever have paid, under any version
of this tree. **Cohort-14 founds its realm on the new release and pays a
wallet's own ATA** — the destination the input operator has documented all
along, and the one the browser now derives when a reader supplies nothing.

### 4. The browser's live redemption test is green

`walletTerminalRedemption.live.test.ts` against cohort-13, Helius devnet:
**4 of 4**. It now tests a market that is resolved AND PAID, which has two honest
readings and makes both:

- a holder who still holds something (the founder's 499,999,800 atoms at the
  losing outcome 0) derives a complete input across all four rounds, over
  `recipient` **`4BENW7Yg…` — the derived ATA, filled in because nothing was
  supplied** — and stops only at a lookup table, which is an account somebody
  must create in a signed transaction;
- a holder already paid (the founder at claim 3) is refused *"payout quantity
  must be within 1..=0 atoms at claim index 3"* — at their balance, by name,
  after the same four rounds.

Both cases assert the byte bound is not what refuses them, so a regression in
the chunk planner fails here by name rather than as an unexplained red.

### 5. The page now says what the answer means

`Resolved — The source failed to report` named a claim and never said what it
was. The market page carries three sentences above the exact values: the data
source never reported, the market did not get stuck, it settled on the fallback
outcome it named and paid for before it opened; anyone holding that claim cashes
in at one collateral unit per claim atom; every other claim is worth exactly
nothing and is not waiting for anything. The first draft rendered inside a
COLLAPSED drawer — the capture caught it, which is why it now sits in the page's
own flow.

The portfolio hero said *"Nothing has resolved yet"*. That is a census, it went
false on the day this market was paid, and it is the second census in that hero
to rot; both are refused by name in the tests now. A holder with zero at the
winning claim is told so where the number is, instead of after two clicks of a
redemption they cannot make.

| capture | words | `$` | hex ids | scrollWidth | page errors |
| --- | ---: | ---: | ---: | ---: | ---: |
| market page, 1280×900 | 1,005 | 18 | 0 | 1,280 | 0 |
| market page, 390×844 | 1,263 | 19 | 0 | 390 | 0 |

Accessibility: **0 open failures on all four arms**. The unresolved-contrast wall
moves 194 → 196 for the new block and its eyebrow, which are the same
translucent-tint pair `.phase-meaning` already contributes and which the
instrument cannot resolve either.

Devnet evidence. Not mainnet evidence.
