# Cohort-14: the seal, the founding, the fill — and a market that can pay a wallet its own account — 2026-09-03

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized.

Tree root `/Users/ember/dev/dclutch`. Every deployed byte is built from
`8e96ec3f8cd4438040d9287d2489ea84587ebd5c`.

## Headline: the two digests a reader looks for first

| | |
| --- | --- |
| founded plan `plan.json` release set | `398e51c008cc5f592f3252f0c1f2246e019ace000b04b74766a41cb45a8a3e09` |
| sealed plan `plan-seal.json` release set | `398e51c008cc5f592f3252f0c1f2246e019ace000b04b74766a41cb45a8a3e09` |
| the Open Market's own byte 208, **read off chain** | `398e51c008cc5f592f3252f0c1f2246e019ace000b04b74766a41cb45a8a3e09` |
| sealed deployment set final digest | `f53575526c0a46309e0ea476264d46e905145fdf011e256a06e5e656a2e26b20` |
| checked release gate | `afee5f070c7388d8622203873d6c550f32ea5188a2f3d2912ad6a06d01a3ba61` |

Cohort-13 was the first cohort whose founding and whose seal named one release
set. Cohort-14 is the second, and it adds the third statement in that table:
the identity is also what the **Market account itself** carries, read at
`STATE_SELECTED_RELEASE_SET_OFFSET = 208` rather than pasted out of `prepare`.

**And cohort-14 can do the thing cohort-13 could not do under any version of
this tree.** Its Realm `8YfBJ2xZV9HGjE9UTdcXow5DMrAEeBr5vUZzxcH5YPRh` carries
collateral adapter release `430369ce72f5e1dcfa19dcee63d5e15f9fbf2d6c9950c5caab53d5c028ae0a2d`
at **offset 80**, read off the account — the third release, whose
`ExtensionStoragePolicy` byte admits the 170-byte `ImmutableOwner` account the
ATA program actually writes. Cohort-13's `228c14f9…` is absent from that record,
as it must be. Whether a market can pay a wallet its own associated token
account is fixed at founding, inside a released identity, and no later commit
grants it.

## 1. The gate certifies the bytes this cohort runs

`checked-release-candidate.sh --genesis-cohort` at `8e96ec3f`, run from a
detached worktree at that exact commit.

```
source_revision=8e96ec3f8cd4438040d9287d2489ea84587ebd5c
sbf_build_freshness=passed          sbf_build_freshness_links=12
sbf_build_diagnostics_total=0       infrastructure_lineage=genesis
trading_elf_sha256=29b6da226fb47bbb551358b12ecd4eb3a829b5b510943322f7890baffbf50bd2
trading_profiled_elf_sha256=223734be9f53ef3ae9ff85e72246d36b001738d069895350c4576fd7bbc3f6e5
trading_admitted_artifact=shipped
node_version=v26.4.0  node_archive_sha256=bef4c7e7…
CANDIDATE_EXIT=0
```

**Two independent builds agree**, and a third instrument reads the chain.

| role | bytes | SHA-256 | second worktree | on-chain dump |
| --- | ---: | --- | --- | --- |
| registry | 238,000 | `94f6cf9b3f4b7ace784e1752decd1b7c2d59820eee8efe3e873667fdd205c865` | identical | IDENTICAL |
| rent | 141,680 | `b9128748d972b5e5afdfdb76a5dc363fe62c3b0ac3a4fbc167fe968156d0da8b` | identical | IDENTICAL |
| custody | 576,552 | `13484668c77d29dcd153ecefd6af7a77ab86eb6afac24c95cc3d737373e51d25` | identical | IDENTICAL |
| resolution | 820,248 | `0691ba844fd10cbb631d77c149541e7e1660864b38f0aa6432b46eba27bfc1f8` | identical | IDENTICAL |
| claims | 1,374,040 | `845d37f57afbd1bc770e2cea8283cdd0235ea2b2a1692fafc502a6d94b9289fb` | identical | IDENTICAL |
| trading | 2,326,128 | `29b6da226fb47bbb551358b12ecd4eb3a829b5b510943322f7890baffbf50bd2` | identical | IDENTICAL |
| core | 1,186,424 | `864394530f37c04e53d10f918c8fab0c265187549895bf5a9207ae91f2a7d02f` | identical | IDENTICAL |
| | **6,663,072** | | | |

Rent's ELF is byte-identical to cohort-13's: nothing under
`programs/dclutch-rent-sbf` changed between `315f1931` and `8e96ec3f`. The
Registry's digest `94f6cf9b…` independently reproduces the one the General
accelerator lane built on 2026-09-02 for its own frame control — two lanes,
two worktrees, one artifact.

## 2. Cohort-13 closed

Ids derived from cohort-13's own keypair files, never transcribed. Its market
`6t3Znm…` was **Terminal and paid**: the founder's failure position was
redeemed, participant-2 was asserted paid zero at both claim indices, and
retirement is owned-loopback only. Nothing reachable was lost.

| role | cohort-13 program id | rent reclaimed (SOL) |
| --- | --- | ---: |
| trading | `HkNhMJrERGko9mFXKq6UaL8qu2QnzqJx1hwJ5U8AVUHZ` | 14.694618225 |
| core | `HZsbUHHwJLUqXdUhjDc4vhnmtgqr65VkU36G8hTijWiy` | 7.513148217 |
| claims | `3XHt6sRpdFxeAa1J23T8TKKFgA78ioJQAdeqJ3HZ5zMv` | 8.675481705 |
| registry | `8XsxVn35gtemD9PuWC9pHYX1rxBAC4T8xV4xdrkdfCBV` | 1.486412097 |
| rent | `CUDLPLjjiLNAQ6hPczhL3AoDux3u77zaJyTERbAbs7Am` | 0.898355049 |
| custody | `G7xAFLpJzdCnc7FXjj5uE3qWSk3ZgCgL684YCQEdCah4` | 3.625294185 |
| resolution | `J33AaXnVDFGYJXYhDxFCPGk4MSM2Ssc69ZcU5PbkYfbb` | 5.189443857 |
| | **total** | **42.082753335** |

Deployer `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`: **32.473851850 →
74.556570185 SOL**. The observed delta 42.082718335 is the table's total less
seven 5,000-lamport fees, which closes exactly.

**Verified by asking the account that can answer.** A closed program keeps its
36-byte Program account, its executable flag and the ProgramData address it
names at offset 4 — so the Program stub cannot tell a live cohort from a dead
one. All seven Program stubs still read 36 bytes, kind 2, `executable=true`, and
still name their ProgramData; all seven **ProgramData accounts are vacant**.

## 3. The redeploy

Seven programs, fresh identities, each verified by dumping the on-chain image
back **before the next one started**.

| role | program id | ProgramData | deployment slot |
| --- | --- | --- | ---: |
| registry | `ySYoUvUw7Z5AtDNqxQAo93vJXD1enNoK8Bf5uLRSyRm` | `9TZNB3AuGZh9XfpP8t8NE8KieinmDRGTyeQ9GctGsEVN` | 492,225,646 |
| rent | `4oQLFDM9TbGBdb2q6QZCxRKZ3u5sqhTycb9MeHt9k41r` | `9RYt8ePJncr4bftaiB1BFo4xeo8DL2B46AdX6Kp1ciTt` | 492,225,697 |
| custody | `8mWrLG2sjfzSKA3fEVBfY3RkGLTLuZZjKqDXWDuTpLbk` | `GHD79BJhR8xB2T2TCUccpL7CUbiy9AoK6CBfnvbeTmto` | 492,225,768 |
| resolution | `5ML5pbUfCaDwokNtmLyTgDEb7eHrfDRrW4PmktXAmphs` | `AXnQbjYTqFD25qQ4BsY9urYpSRhhHfJH2HGPs59v1SUw` | 492,225,857 |
| claims | `H8ANKXECwkntr8Cczo6gZX5d9PWN6uwrqCyeohYsZVhV` | `DkkGjSpV7X5enzpUM5GB4qPwnidXmquwimGLpFVXp9cC` | 492,225,979 |
| trading | `DcsWHSjPTTpYzXScmB5xYh3iEsM9fx4YFC1BPvQggEtu` | `FMgGM3THeeqML5ALMG3XKee4k8eh8QaqXGjE2uiUUAxH` | 492,226,154 |
| core | `9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB` | `CC39Q4RstSZBniSZZASYoZXMyQtTari3WHZs9Zscgt2t` | 492,226,262 |

**Three instruments, one claim.** The dump comparison, the byte count, and a
hostile decode of each live ProgramData account — all seven `MATCH`. All seven
carry an authority tag of 1 equal to the retained deployer, are Loader-owned,
and their ProgramData accounts are non-executable. The ProgramData address is
**read**, not derived: it is the 32 bytes the Program account names at offset 4.

Deployer **74.556570185 → 32.311320662 SOL**; the redeploy cost
**42.245249523**. The runbook priced it at 42.2453 from cohort-13's measured
6,340.21 lamports/byte over 6,663,072 bytes. **The projection and the
measurement agree to five decimal places**, which is the model earning its keep
rather than a coincidence: the affine `890,880 + 6,960·n` ceiling would have
said 47.28 and over-predicted by 12%.

## 4. The ladder, and the TENTH record

`campaign --through activation`, deployer as Core upgrade authority, campaign
payer `4Fbg977j7wK5y7hsV3LcRPXL4RVYyVUxafvHmXk2VmR7` funded with 2 SOL first.

**36 transactions, zero errors, 7,586,576 CU, 2,700,000 lamports of fee, and
`publication: 10 record bodies finalized`.** Cohort-13's was 33 transactions and
nine records; the tenth is the General accelerator's `ArtifactRelease`.

```
campaign stage succession: nothing to execute -- this cohort is born at V2
and carries no ceremony; observed absent
```

**Re-observed from chain** by a second preflight that reads the cluster rather
than the driver's exit code — substrate, publication, initialize, succession and
activation all `complete`.

Deployer **30.311315662 → 30.271732270**: 0.039583392 SOL of fees and record
rent, against cohort-13's 0.037179840 for nine records.

## 5. The accelerator's release record IS the deployment observation

`prepare` carried the `--general-accelerator-*` group, which publishes an eighth
`ArtifactRelease` beside the seven roles'. Under `90a8563f` its **finalization is
the observation**: `observe_artifact_release_deployment_v1` derives the Program
and ProgramData metas from the record's own content and compares them against
the chain before the staging cursor closes. A finalized record is the check, not
a receipt of one.

Read back off the account, not out of the plan —
`HHhYpY5f73L695Ak3wQeo4SQp7nnQzBEEBCjfUmjRAua`, 216 bytes, magic `DCLTARF1`,
owned by cohort-14's Registry, **staging cursor vacant**:

| field | offset | value |
| --- | ---: | --- |
| accelerator Program | 16 | `8pgnyNvgdue7Jc8aw75BGWoghsKGevWJvFom8omUWvQY` |
| accelerator ProgramData | 80 | `HcxFzWKaFzrVVnvgx6BWuNbo278pgpYY5CrxyVe67Sxb` |
| ELF digest | 144 | `61b2d73d44f2470051b40e39cda1d31a5f67679429eacd5448d5e5ac583b74ae` |
| deployment slot | 176 | **491,959,038** |
| upgrade authority | 184 | `4zrxtw5c…` — `ExactAuthority`, never `immutable` |

Its semantic release id `932bc2ca37b976061ed4e0b91ab9fec0c801fbd42fb4ede1ac517dc208008760`
is **operator-stated**, because `checked_semantic_release_preimage_v1` refuses
any role outside the seven. It is nonetheless reproducible rather than random:
it is the ordinary artifact derivation
`sha256("dclutch/checked-semantic-release/artifact/v2\n" ‖ "general-accelerator" ‖ 0x00 ‖ elf_sha256_hex)`
under a label no role uses, so it is a function of the shipped accelerator ELF
and cannot collide with a role's by construction. That is stronger than what the
tool checks (nonzero, no collision) and it is **still not a check**. The fix
remains a `SourceSemanticRoleV1::GeneralAccelerator` label.

### THE ACCELERATOR'S SOURCE MOVED AFTER ITS DEPLOY, and this cohort pins the chain

The accelerator was deployed from `324528a4`. A build at **this cohort's own
revision** produces
`4bdbfc886fb64c030099badd82cb836105c43643f4ac7f80fc5a7902b6a74f96`, not the
deployed `61b2d73d…`, and the cause is exactly one commit: **`fd6cd0603`**
(the clippy census) edited
`programs/dclutch-general-accelerator-sbf/src/lib.rs` after the deploy.

**The chain did not move; the source did.** So every record cohort-14 published
observes the DEPLOYED artifact — the 302,256-byte ELF whose digest reproduces
the on-chain dump byte for byte — and not the one this tree would build today.
That is the only correct choice, because the record's whole purpose is to be
compared against the chain, and the guard that would have caught the mistake did
fire: the capture step asserts the live ProgramData tail equals the supplied
ELF's digest for all eight artifacts and refuses otherwise.

It is also debt, stated as debt. A future redeploy of the accelerator supersedes
this market's certificates, and the tree can no longer rebuild the artifact those
certificates pin without checking out `324528a4`.

## 6. THE SEAL, before the founding and at zero cost

Key-free and read-only, run **before** any founding, so the founding is only
attempted once the identity it will pin is proved reachable.

All five owned roles preflight `equal: true` against a **fresh finalized
observation**:

| role | live ELF = checked candidate | observed slot |
| --- | --- | ---: |
| custody | `13484668…` | 492,230,266 |
| resolution | `0691ba84…` | 492,230,276 |
| claims | `845d37f5…` | 492,230,286 |
| trading | `29b6da22…` | 492,230,296 |
| core | `86439453…` | 492,230,306 |

```
completed_role_count 7      next_role null
final_set_sha256 f53575526c0a46309e0ea476264d46e905145fdf011e256a06e5e656a2e26b20
registry carry-forward   rent carry-forward
custody / resolution / claims / trading / core: already-current
```

`prepare --deployment-set-journal` then produced `plan-seal.json` whose
`checked_upgrade_set_final_sha256` is that same digest, with `release_set_id`
**unchanged**, and `plan.checked_upgrade_set` is `Some`.

**Cost: 0.000000000 SOL.** Deployer 30.271732270 and payer 2.000000000, both
unmoved across the whole seal. Nothing was signed.

## 7. The cut fragment, emitted rather than typed

```json
{
  "checkedReleases": {
    "398e51c008cc5f592f3252f0c1f2246e019ace000b04b74766a41cb45a8a3e09": {
      "gateDigest": "afee5f070c7388d8622203873d6c550f32ea5188a2f3d2912ad6a06d01a3ba61",
      "sealedSet": "f53575526c0a46309e0ea476264d46e905145fdf011e256a06e5e656a2e26b20"
    }
  },
  "schema": "dclutch-public-cut-checked-releases-fragment-v1"
}
```

`prepare` emitted this beside `plan-seal.json`; nothing here was typed. The
cut's `--release-set` argument must be read **off the chain** from byte 208,
never pasted out of `prepare`'s result JSON — that file is the fragment's own
source, so the comparison would be a value against itself.

## 8. The market IS founded and open, and this time the driver said so

`SCRIPT_EXIT=0`, **186 campaign transactions** — cohort-13's count exactly, and
without the `getBlockTime` RPC death that cost cohort-13 its `execution` block
and two more lanes. The report is whole: `execution.completed = true`,
`recoveredFinalizedFounding = false`, **192 transactions in the projection**, 62
market accounts, `founding_targets` present.

| campaign label | address | bytes | phase | readiness | generation | selected_release_set |
| --- | --- | ---: | --- | --- | ---: | --- |
| `open_market` | **`FgzbVSWVp36R5AVPchneyz9AKCMzdjpRfsekUsY4tT6i`** | 368 | `0x01` **Open** | `0x02` Consumed | **2** | `398e51c0…` |
| `found31_market` | `2iMUkPVU3B4GBbj6be6PdKY4gdmHg91LCPiCgZ2s2yA7` | 368 | `0x00` Founding | `0x00` Prepaid | 1 | `398e51c0…` |
| `abort_market` | `AP7AzsLY4dQp5hsV9w7XMVtD7TYYM6cuU13bLMgGuvUW` | — | — | — | — | vacant, as it should be |

Both Core-owned `DCLTCOR3`, owner `9JW1qqJVeFo9ZRvzzVzNvqrwzt7QvyHpGafTJmj2hBFB`.

**All five routing tables frozen**, read back by address: address counts
**35 / 45 / 15 / 56 / 62** — cohort-13's and cohort-12's counts to the address,
which is an independent check that the same five frames were built.

| table | label | addresses |
| --- | --- | ---: |
| `DvWkAZtUNSA5EtYJeCymC3quLMz4FLskQa5hxd7ahUKP` | Found37 | 35 |
| `7BVwyhoAL3HAxQq3k77E4mrZdk1RW5K5DxGgrChsSRBC` | DCLTCFQ1 | 45 |
| `HKubAgtDLdFHEHTNmrUdRz2A6FZuG1UE2V1xGiunpJxw` | DCLTCF1A | 15 |
| `6bckhUU3WLBfem3ff7x6RbDbzQungXhjQbVcetcemC2z` | DCLTPCB2 | 56 |
| `5Pw2FPhwdrzCaA7XPkqYSKSrPJQWtM6tTGaffqGNPhyK` | DCLTGMF3 | 62 |

Each is ALT-program owned, non-executable, `authority = None`,
`deactivation_slot = u64::MAX`, last extended strictly before the observation.

### The rate, admitted before a lamport was spent

```
directFeeBasisPointsPerSide                       50
directTokenSetupAdmitsThisRate                    true
feeRateIsIrreversible                             true
maximumGrossCollateralAtomsWhoseFeeFloorsToZero   199
```

### The cuts, re-centred on a spot measured three ways

Read immediately before staging off the sponsored PriceUpdateV2
`7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` — the account this market itself
resolves against — SOL/USD was **$100.363041**, conf ±$0.009853, EMA $99.846,
`publish_time` **nine seconds old**, with **Coinbase $100.36** and
**Kraken $100.38** agreeing. So `--cuts 9800,10200 --cut-denominator 100
--band-anchor 10036`: $98.00 and $102.00 straddling a measured $100.36, the
anchor stating spot rather than a round number.

Cohort-13's `9600,10000` sat entirely below this price; inherited cuts are how
cohort-12 founded a market whose upper cell's answer was already known.

The founding drew **0.336594719 SOL** from the campaign payer — cohort-13's and
cohort-12's figure **to the lamport**, which is itself a check that the same work
happened.

## 9. Activation: the verdict string, and a root registered in advance

`devnet-direct-capability-activation-v1 --execute`, payer the campaign payer.

```
verdict                 ACTIVATED
facts.root              8hRZFfmRsRu7BRYPyHv66C4BNKfCK4majQP4DKVE3RtG
activationDeadlineSlot  492447083
```

Both the root and the deadline come from **the command's own report**. The
deadline was 205,301 slots away at the observed slot — about 22.8 hours — and
`HOLD_STATE.md`'s remembered number is not what was used, which is the discipline
cohort-13 paid for.

**The cross-check was registered before the act, and it closed.**

| address | before | after |
| --- | --- | --- |
| activation root `8hRZFfmRsRu7BRYPyHv66C4BNKfCK4majQP4DKVE3RtG` | `AccountNotFound` | **256 bytes `DCLTCRT1`**, Trading-owned, 2,431,872 lamports |
| founding-permit root `AqB6mcEmGu3KSkTXPvHTXZWnLGRVpeRQxuDZP4PQfoax` | `AccountNotFound` | `AccountNotFound`, **as it will be forever** |

The permit root's vacancy is evidence of nothing — it is the FOUNDING-PERMIT
namespace address at which no account can ever exist. Reading it to judge an
activation reports a permanent vacancy as a failure. Payer cost
**0.007011984 SOL**, cohort-13's figure to the lamport.

## 10. THE PREPAY: the arm that had only ever been planned, and the four sites it broke on

Cohort-13's failure walk refused `0x8002 ResolutionError::OutputState` after
305,522 CU because the certificate seat had no funder on the devnet path.
`62a0b7fb5` built that funder and checked its arithmetic. **Cohort-14 is the
first cohort to EXECUTE it**, and it refused:

```
sponsored-push-PrepayCertificate: v0 message: NoLookupUsed
```

**The prepay's frame has nothing a routing table can resolve, by construction.**
It is a bare System transfer: the signer is never table-resolvable, the
certificate seat is a PDA minted long after the founding froze its five tables,
and the System program is a program id, which a v0 message requires in its
static keys. All five frozen tables were checked against the frame and every one
covers exactly the System program and nothing else. So
`dclutch-versioned-message-operator` compiles a message that resolves ZERO
addresses through the table and refuses — correctly. Paying the v0 lookup cost
for nothing is the thing that refusal exists to catch.

Fixed at its owner in **`ab0322d50`**: the prepay travels as a LEGACY packet,
selected by `ExteriorActionV1::uses_routing_table()`, branched at sign, submit
and confirm. The test asserts the **partition** rather than the fix — it checks
the predicate against an independent property, *does this action invoke a dClutch
program at all* — so an eighth action goes red naming the half whose membership
it did not state. Proved red before green: under the pre-fix behaviour it fails
on `PrepayCertificate` and on no other action.

Then it landed on chain and refused **while writing its own evidence**:

```
sponsored report: canonical v0 message: NoLookupUsed
```

`authenticate_report` was the fourth v0 site. **`3e5e0b0be`** branches it too,
and adds `authenticate_signed_legacy_packet_against`, because the legacy
authenticator that already existed checks only the packet digest and signature —
reusing it would have made the prepay's report the only one in this command
whose packet is not pinned to its instruction, and nothing would have gone red.

This is the shape cohort-13's evidence named a fourth time: a reader, a schema
and a refusal all built and reviewed with the success path never run. Here the
producer existed and only its LAST step was unexercised, which is the same gap
one stage later.

**Result**, read back off chain:

| | |
| --- | --- |
| certificate seat | `6tjPnH8XdPmmq5H2rB5icEUendutVgZ5xP1tPsAw36GA` |
| lamports | **2,786,520** = `rent.minimum_balance(312)` on this cluster, to the lamport |
| owner / bytes | System, 0 — a seat the walk has not taken yet |
| signature | `2KjkMhtagVzBm6sc6cQzFq1roFgPDa6VWBqh9ZbDrZUMGKXekYeywTfH3DVgGAScLPbanmTNeKcswkUtHpGQKUXE` |
| slot / CU / fee | 492,245,070 / 450 / 75,000 |

Payer −2,861,520 lamports = 2,786,520 rent + 75,000 fee. Exact.

Both terminal input documents were produced at founding time, not resolution
time: `terminalSequence 0` for the capture and `1` for the settle, because the
consumer refuses a mismatch and the producer never overwrites an output path.

## 11. The strangers, the delegation, and THE FILL

Two 0.05 SOL transfers from the **campaign payer, never the deployer**, then two
user-position admissions, both authenticating `plan_sha256` = `ef378b70…` =
`plan-seal.json`.

| | signature |
| --- | --- |
| participant-1 `5Sqp8onyzFSKWQ2kje235ns2UfNQRwSpBZuGYGBMmoJj` | `dC2PMkaDkYSp9BWYYmvBMKH3nqMXmJbp9Xq3e7E2dPvBLvWPUuwuic533whGPaKrbMzz4Tj1kLX9V9Sb8a5G3yK` |
| participant-2 `92RrSCo9B3yEka47ZWV7dyJ2hpw11GdsduAhA21qh6dd` | `2D189ytM3gvSt2KtZUQjwVepESuyyPkHRFLfeVgWQW5k4G6Fa8Qaj7cnZZMfstX4rdwMXaktc1ezMBEiPY93XwJ9` |

The collateral delegation put exactly **201 atoms** —
`required_buyer_collateral` = gross 200 + buyer fee 1 — into
`BDqobdrDVCkbNTEnnqhiW6gpEipNy7sy3Ptc1MEtHyg5`, read back off chain at 165 bytes
under Token-2022. Signature
`5EL6AuauA3pMedVVZFQFGwbALouM8L3k5FJW8W2XwzSmT9raX9xprXfWsV1JdSrjnZLLWtc9jvWSxqYVXzfJB7aS`,
slot 492,247,636, **4,953 CU** — cohort-13's figure exactly.

### THE FILL: 1,284,573 CU, and the drift went the same way again

| stage | slot | CU | fee | signature |
| --- | ---: | ---: | ---: | --- |
| replay-setup | 492,248,347 | 157,483 | 75,000 | `2EWzfZA6…rWfxtV` |
| token-setup | 492,248,441 | 107,918 | 75,000 | `2GKkqPnG…UTjDtu` |
| lookup-create | 492,248,552 | 10,430 | 5,000 | `yhGyz9RQ…CaFj2E` |
| lookup-extend | 492,248,666 | 11,657 | 5,000 | `4nEusTgE…M7wnVH` |
| lookup-extend | 492,248,780 | 11,660 | 5,000 | `2ysrLDJg…Swoyks` |
| lookup-extend | 492,248,891 | 10,780 | 5,000 | `zz2AZEG8…MGVzqU` |
| lookup-freeze | 492,249,004 | 1,517 | 5,000 | `5KJkhbWK…uiVTeA` |
| capability-seal | 492,249,180 | 737,283 | 5,000 | `5v6YLs34…32unVg` |
| **hot (THE FILL)** | **492,249,302** | **1,284,573** | 15,000 | `56KTyPF913E7QEstS5bUVha1RvMqUk7EtFFG8yEHpb5Agvpx8fWPesNEhH5P2g2jf2VHvXkMkT36dozXmikz2HNn` |

**2,333,301 CU and 195,000 lamports across the session.**

| | CU | margin under 1,400,000 |
| --- | ---: | ---: |
| cohort-13, Trading `1b41f552…`, 2,320,152 bytes | 1,286,187 | 113,813 (8.1%) |
| **cohort-14, Trading `29b6da22…`, 2,326,128 bytes** | **1,284,573** | **115,427 (8.2%)** |

**The drift is −1,614 CU across a Trading ELF 5,976 bytes larger** — the same
direction as cohort-12 → cohort-13's −30,942, and an order of magnitude smaller.
Two cohorts in a row have bought a slightly cheaper crossing with a bigger
binary. The honest reading is unchanged: 8.2% is still thin enough to re-measure
every cohort, and the number is inherited as a measurement, never as a memory.

The terms, from the session's own finalized evidence: fill **200** at price
1,000,000 over scale 1,000,000 — gross **200** — outcome **0**, fee **50 bps per
side**, seller the founder, buyer participant-2. The smallest gross whose fee
does not floor to zero, and nothing was retried at 199.

**The double-publish defect cohort-13 recorded is still there**, and it is the
reason a fill that fully landed reported as a refusal:

```
publish Direct evidence: File exists (os error 17)
```

`direct-trade-finalized.json` is published through `write_create_only_json_v1`,
which publishes by `fs::hard_link` precisely so evidence can never be
overwritten — then publishes the same path a second time in the same invocation
and trips its own guard, leaving
`.direct-trade-finalized.json.direct-evidence-35778.tmp` behind, which is what
identifies it as a double publish rather than a collision. The evidence written
was complete and authentic. **The guard is right; publishing twice is the
defect**, and it is now the second cohort to meet it.

## 12. The fee, settled and read back

`devnet-direct-fee-settlement-v1`, permissionless, no party to the trade signing.
The obligation matched the prediction to the atom: debtor participant-2, maker
replay `Ab997sZbMvUCd4oMFNzjvuxFJvziRKnpA1LpcEBCPUvE`, **fee_owed 2**, standing
allowance 2, destination the venue fee PDA
`GjEPE5TFovJrfkvTbPDhecBfjEya915jqFRgXvfj2a2C` owned by
`AMEonMv4TwWjgwCDTAYwxs7V1CDyoktvteBNeJzDFhMU`, custody revision 2 → 3.

```
signature      5TsBX6xpKvxoBnPaKB6ZEYyMEDP7yfjybhzJYAi5kqe9kxxjeSx9VZdJNgnBAaDJQNJo7KUoR4CBWHu46wVJJyNp
slot           492,249,852     compute units 155,689     fee 75,000
```

**Read back from chain afterwards**, the command's own preflight now refuses:

> `92RrSCo9…` owes nothing on `Ab997sZb…`; the route would refuse as `FeeNotOwed`

which is a stronger statement than `fee_owed 0`: the obligation does not exist,
asserted by the same authority that would have to honour it.

## 13. THE CENSUS: seven laws HOLD, and L7 is INAPPLICABLE by a cause this lane owns

At stage `cohort14-post-fee-settlement`, finalized slot **492,250,832**, chained
through `--prior` to the pre-fill boundary, with the buyer's collateral source,
the seller's Direct token PDA and the venue fee PDA all named by `--token`.

| law | verdict |
| --- | --- |
| **L1** | HOLDS — tracked 1,000,000,000 atoms across 5 accounts == Mint supply 1,000,000,000 |
| **L2** | HOLDS — the Hoard moved 0 atoms, exactly as declared; it holds 500,000,000 |
| **L3** | HOLDS — 3 Positions sum to the aggregate supply vector [500000000, 500000000, 500000000, 500000000] |
| **L4** | HOLDS — Hoard 500,000,000 >= worst outcome 500,000,000 × unit 1 |
| **L5** | HOLDS — tracked collateral moved 0 atoms, exactly as declared |
| **L6** | HOLDS — no watched account closed at this boundary |
| **L7** | **INAPPLICABLE — not a pass. See below.** |
| **L8** | HOLDS — every compartment moved exactly as declared: unclassified +0 |

Declarations: `--declared-collateral-delta 0`, `--declared-hoard-delta 0`,
`--declared-class-delta unclassified=0`, `--declared-class-delta HoardPrincipal=0`,
`--declared-fees-lamports 280000`.

**L7's own words, quoted rather than paraphrased:**

> this boundary admitted 5 account(s) the previous census did not watch
> (buyer_delegated_collateral, participant-1, participant-2, seller_direct_token,
> venue_fee_direct_token), so their balances have no predecessor to difference
> against; L7 resumes at the next boundary

**The cause is this lane's and it is worth writing down, because cohort-13 got
L7 for free and did not have to learn it.** The complete-bindings census must be
taken **after the collateral delegation and before the fill**. This lane's prior
boundary was taken before the delegation, so the aperture grew by five accounts
across the fill and a chained delta compares two different apertures. Cohort-13's
prior already carried the full aperture, which is why its L7 judged and this
one's cannot.

`census/post-settlement.json` is therefore left as a **stable-aperture
baseline**, and L7 is owed at the next real boundary — the relay capture — with
the same four `--token` and three `--position` bindings and that transaction's
own fee. Manufacturing a boundary with nothing between it and this one would
make L7 read `0 == 0 + 0`, which is a number nobody earned; this file does not
do that.

Cohort-13 had no INAPPLICABLE anywhere. **Cohort-14 has exactly one, it is named
by name with its cause, and it is not reported as a pass.**

The state the laws are about, at that boundary:

| account | atoms |
| --- | ---: |
| founder collateral wallet `4uYpgsxpFjyPQFKqoJwfJYriKGbZwWEv6G18VKj9MWn2` | 499,999,799 |
| Hoard `4m81M8LHy46pv7ZMqYK4WQ48yK58Tu2rnz6c4RVqaEGy` | 500,000,000 |
| seller Direct token PDA `3ZeDjX9Nefcsw73VqPts5iVHaVcc6QyXS8fJCdedz2BK` | **199** |
| venue fee Direct token PDA `GjEPE5TFovJrfkvTbPDhecBfjEya915jqFRgXvfj2a2C` | **2** |
| buyer delegated collateral `BDqobdrDVCkbNTEnnqhiW6gpEipNy7sy3Ptc1MEtHyg5` | **0** |

199 + 2 = 201, the buyer's allowance spent exactly to zero; 199 is gross 200 less
the seller's fee of 1, and the 2 in the venue account is both sides' fee. The
claims moved with it: the founder's outcome-0 balance went 500,000,000 →
**499,999,800** and participant-2's went 0 → **200**, while outcomes 1, 2 and 3
did not move at all.

## 14. The money, end to end

| stage | deployer | campaign payer |
| --- | ---: | ---: |
| before anything | 32.473851850 | — |
| after closing cohort-13 | **74.556570185** | — |
| after the seven-program redeploy | 32.311320662 | — |
| after funding the payer | 30.311315662 | 2.000000000 |
| after the ladder | **30.271732270** | 2.000000000 |
| after the seal | 30.271732270 | 2.000000000 |
| after the founding | 30.271732270 | 1.663405281 |
| after the activation | 30.271732270 | 1.656393297 |
| after the certificate prepay | 30.271732270 | 1.653531777 |
| after funding two participants | 30.271732270 | 1.553521777 |
| after the admissions and the delegation | 30.271732270 | 1.551476208 |
| after the nine-transaction fill | 30.271732270 | 1.521630102 |
| after the fee settlement | **30.271732270** | **1.521555102** |

| step | cost | against the 2 SOL bound |
| --- | ---: | --- |
| redeploy (the deploy itself, exempt) | 42.245249523 | — |
| ladder: 2,700,000 lamports fee + record rent | 0.039583392 | within |
| campaign payer capitalization | 2.000000000 | at the bound, by design |
| **the seal** | **0.000000000** | key-free, read-only, nothing signed |
| founding | 0.336594719 | within |
| activation | 0.007011984 | within |
| certificate prepay (rent, recoverable) | 0.002861520 | within |
| participants + admissions + delegation | 0.102045569 | within |
| the fill | 0.029846106 | within |
| fee settlement | 0.000075000 | within |

**THE DEPLOYER MOVED FOR ITS OWN DEPLOY AND ITS OWN LADDER AND NOTHING ELSE.**
It has been at 30.271732270 SOL since the ladder finished, unmoved to the
lamport through the seal, the founding, the activation, the prepay, the
admissions, the fill and the settlement.

### The open number cohort-13 carried is CLOSED

Cohort-13 recorded −1.917836469 SOL leaving the deployer during its founding
window as unattributed, refusing to explain it away. It is the **General
accelerator's deploy** — 1,916,321,469 lamports of rent plus 1,515,000 of fees =
**1,917,836,469 to the lamport** — signature
`3TtiaVkrubvTjhMu4GTD1369AwGYiksSG5WRBn6Sz3SbBB8SY4SZcvMbTay6pcGKVweE766BCxu7sGt3e4aYKnPR`,
recorded in `docs/evidence/GENERAL_ACCELERATOR_DEVNET_2026_09_02.md`. Two lanes
sharing one deployer keypair path is the cause, and the discipline that closed it
is the one that found it: state an unattributed movement as an open number rather
than a story.

## 15. What cohort-14 has that no previous cohort had

- A founding whose **seal, whose founded plan, and whose Market account** all
  carry one release-set identity — the third statement is new.
- A **Realm founded on the third collateral adapter release**, so a 170-byte
  `ImmutableOwner` associated token account is a payable destination. Cohort-13's
  165-byte auxiliary account was never a workaround; it was the only destination
  that cohort could ever have paid.
- The General accelerator's **`ArtifactRelease` finalized in a cohort's own
  Registry** — the deployment observation, on chain, at slot 491,959,038.
- A founding driver that **completed**, so no recovery step, no reconstructed
  `execution`, no consumer refusing the evidence.
- The **certificate seat prepaid before it is needed**, by the public arm, which
  had never been executed until this cohort executed it and broke it twice.

## 16. Owed, in priority order

1. **The relay capture and settle.** Armed, timed and handed off: the window is
   1788415866 → 1788417666 (2026-09-03 06:11:06 → 06:41:06 UTC), the capture
   fires at 06:12:06 and the settle at 08:41:36, and the seat is already funded.
   `~/jobs/dclutch-cohort14-20260902/HOLD_STATE.md` carries the resume order.
   **The verifier that matters is the certificate's kind byte: 1, not 4.** Kind 4
   is cohort-13's outcome and shipping it twice would make an oracle outage into
   founder revenue a second time.
2. **L7**, per §13 — at the capture boundary, chained to the stable baseline.
3. **The General market**: its six inputs are built and verified, the compile is
   green (`deployment_slot 491959038`), the founding is running as this file is
   written; the activation and OpenBatch N=2 follow.
4. **The accelerator's semantic release id** is still operator-stated, and its
   **source has drifted from the deployed artifact** (§5).
5. **The double publish** in the Direct session's terminal step (§11), met twice.
6. **Retirement is still owned-loopback only.** Cohort-14 can be resolved on
   devnet. It still cannot be retired there.

## 17. Provenance

Job directory `~/jobs/dclutch-cohort14-20260902` (mode 700). The endpoint key
exists in no file under it: the generated founding script reads it at run time,
and the sweep that redacts evidence skips live inputs by name — a lesson bought
twice, because redacting a live input broke a founding and an admission with
`getGenesisHash returned HTTP 401 Unauthorized`. Both refusals cost zero
lamports and both are recorded in `HOLD_STATE.md`.

Deployed bytes come from `8e96ec3f8cd4438040d9287d2489ea84587ebd5c` and nothing
else. The two host-tool commits `ab0322d50` and `3e5e0b0be` change no deployed
byte, no release set and no plan; they are the prepay's send path and its report
authenticator, and they are named wherever their output is used.

Devnet evidence. Not mainnet evidence.

---

## Addendum: THE GENERAL MARKET IS STRANDED, and the cause is one shared keypair

Written the same night, after §16 item 3 was attempted. **Devnet evidence. Not
mainnet evidence.**

### What worked

The compiler is green and its output is checkable. All six inputs were built
rather than borrowed:

| input | bytes | sha256 |
| --- | ---: | --- |
| `policy.json` | 613 | `6b3b64118a017e32b63b89c12030d191e3f586fc7e6f46b12a7dc4a7acd67d88` |
| `selection-policy.txt` | 621 | `397123cc49f5a2ca565886610a723d9afc5dcd13ec0c1faac1b4a23f9e4a3e71` |
| `compiler-release.txt` | 1,214 | `e3797097f3fc3b8f801f3c13099038d0aa152054c4ff95a2ae4075f3e2f55a7a` |
| `toolchain.txt` | 1,206 | `83c5b639c8c38437ca56f28642f2da82c0cdfec244fe6544685784d5a93f8437` |
| `translation-validation.bin` | **688** | `aa97c0c10a98248f9ada4dccc96a5a4e969073cb57ded04340efe21b6996f4f8` |
| `price-update.bin` | 134 | `0d39fbbb1d1d0556b0fb2ed08fdbe9305dca0b8e63cba09595a177ac17338edb` |

The translation validation is a real one, not a placeholder: `lake build` (134
jobs) then `tools/direct-translation-validator/check.sh` ran green, and
`dclutch-release-tool create-translation` minted the canonical 688-byte
`CheckedTranslationValidationV1` over its evidence directory. It remains
**Direct-shaped** — there is no General translation-validation corpus — which is
a claim about a different program and is named again here rather than quietly
inherited.

`devnet-general-market` compiled a 231,095-byte `market.json`, and
`accelerator-observation.txt` reports **`deployment_slot 491959038`**, the
runbook's verifier, with `elf_digest 61b2d73d…`. Its
`artifact_release_id dcce810097111f696e4888ef89385fc0e1e1c24d89818ca040d950d1daec5b94`
is **byte-identical to the `content_sha256` of the record the ladder finalized**
in §5 — two independent authors, one digest, neither reading the other.

### THE WALL: the founding source funder is a PER-MARKET identity, and nothing says so

The founding ran 530 transactions over 68 minutes and refused:

```
fund the founding principal supplier and its rent-capacity witness:
  Error processing Instruction 3: custom program error: 0x0
  Create Account: account G2dfBofaGFRtTSsWiwNeibD7niwoMS9XsLUAFC1c93uJ already in use
```

`Custom(0)` is below `0x1000`, so by decision 0007 it is **not ours** — it is the
System program refusing to create an account that exists. And
`G2dfBofaGFRtTSsWiwNeibD7niwoMS9XsLUAFC1c93uJ` is this lane's
`founding-source-funder`, read back off chain as a **165-byte Token-2022 account
holding 1,855,569 lamports of rent — created by the DIRECT founding four hours
earlier.**

So the founding principal supplier's keypair is not a campaign credential that
signs; it is an identity the founding **creates a token account at**. A second
market founded with the same keypair collides on `CreateAccount` at the first
funding instruction after every record has been published.

**Nothing in this tree says so.** The runbook's step 3 reads *"Found through the
ordinary founding campaign with `market.json` as the market input… nothing about
the founding driver changes"*, which is true of the driver and false of the keys.
The collateral mint and wallet are obviously per-market and were freshly
generated; the source funder and the projection witness look like campaign
identities and are not. The projection witness `AkSjTgMKq94r6ssds2Fgj4rAsExgACxAa3uU6QPmqHzJ`
also carries 3,141,168 lamports from the Direct founding and would have been the
next collision.

### And the collision lands where NO resume exists

This is the part worth keeping. The refusal is not merely expensive, it is
terminal for that market, because of exactly where in the ladder it falls.

A fresh evidence path refuses, correctly:

```
this founding has STARTED on this chain (the Open Market does not exist at
DL675bt1dmQQ87UeGBCnmV1zQTWsvd7jbwLmVqT2tsW9 but this founding has started:
collateral mint 2si3RLx8…, collateral wallet FuRBL5tx…,
realm record Bji6ADSm…, Found31 Market JEG8H7qS…)
```

and resuming the original evidence path refuses, also correctly:

```
…but no compatible durable DCLTPCB2 checkpoint authenticates a safe suffix resume
```

**The collision falls after the collateral mint, the collateral wallet, the Realm
record and the Found31 Market are on chain, and before the DCLTPCB2 checkpoint
that would authenticate a suffix resume.** Both doors are shut and both are shut
for good reasons. Read back off chain:

| account | state |
| --- | --- |
| General Open Market `DL675bt1dmQQ87UeGBCnmV1zQTWsvd7jbwLmVqT2tsW9` | **AccountNotFound** — never created |
| Found31 Market `JEG8H7qSJGxiCSKpXr2pg6oSPrTofoiHjLjX7RCiDdWK` | 368 B `DCLTCOR3`, Core-owned |
| Realm record `Bji6ADSmRReUu8B9Fw6h7PdEZgSJB7AfXpcuvzD6B1Rg` | 112 B `DCLTRLM1`, Registry-owned |
| collateral Mint `2si3RLx8qxJPfsw2kStZGQoxuAkKuw2nJwFASf5M96kS` | 82 B, Token-2022 |

**No third attempt was made.** A third founding would need a fresh mint, wallet
and realm against a Market PDA whose partial state is already on chain, and that
is improvising past an irreversible boundary. This lane stops here and says so,
which is the same call cohort-13 made at its own wall.

### What it cost, and what it did not

**Cost: 0.812936971 SOL**, all from the campaign payer, against a 2 SOL
per-step bound. It is the most expensive step after the deploy and it bought a
finding rather than a market.

**One deliberate movement of the deployer, recorded rather than hidden.** §14's
sentence *"the deployer moved for its own deploy and its own ladder and nothing
else"* held until this step and no longer does. With the payer at 0.790 SOL and
the founding still running, the payer was topped up by **1.000000000 SOL**
(signature `1iN4spgqNpJJddLdMNajrrL5LhigpUEfg6CECZVcNqvHXFjYtMhf8qrt9LMFhT9duUzjP9Wb8zjCchcMkKvxtku`),
deployer **30.271732270 → 29.271727270**. That was a judgement call: a founding
that dies for want of lamports is the failure this project has paid for
repeatedly, and 1 SOL is cheap against it. The §14 table is correct up to and
including the fee settlement; this is the movement after it.

**It cost the Direct market nothing.** Re-read after the wall: Market
`FgzbVSWVp36R…` still `0x01` Open / `0x02` Consumed / generation 2 / release set
`398e51c0…`; activation root `8hRZFfmR…` still a 256-byte `DCLTCRT1`; the
certificate seat still holds 2,786,520 lamports. The relay is unaffected.

### Owed, precisely

1. **Per-market founding identities, stated where a caller reads.** The campaign
   should either derive the source funder and the projection witness per market,
   or refuse at plan time when the supplied keypair already holds an account —
   a check it can make offline, for free, before publishing 530 transactions
   worth of records. The refusal it earns today is a System `Custom(0)` seven
   hundred lines from anything that names a market.
2. **A resume for a founding that dies before DCLTPCB2.** Cohort-13's recovery
   step closed the case where a founding dies *after* Open; this is the case
   where it dies *before the first checkpoint*, and the same argument applies:
   the journal records what landed, and re-reading live accounts a later stage
   was going to create is not the check it looks like.
3. The General market itself — compile inputs are built and verified and are
   reusable as they stand; only the founding needs fresh per-market keys and a
   Market PDA whose partial state has been dealt with.

Devnet evidence. Not mainnet evidence.

---

## Addendum: THE CAPTURE CANNOT SUCCEED, and it never could have — Pyth's Receiver was redeployed before cohort-13 was founded

**Devnet evidence. Not mainnet evidence.** This is the most important thing this
lane found, and it retroactively corrects cohort-13's resolution narrative.

### What happened

The relay runner was armed to wait for the window and fire. Its bounded wait
refused (see below), so the capture ran immediately — **three hours and
forty-five minutes before the window opens** — and the deployed Resolution
program refused it in simulation, spending nothing:

```
Program 5ML5pbUfCaDwokNtmLyTgDEb7eHrfDRrW4PmktXAmphs
  consumed 101787 of 1399700 compute units
  failed: custom program error: 0x8014
```

**`0x8014` is not a window refusal.** Band 8 is Resolution and `0x8014` is
`ResolutionError::ReleaseSuperseded` (`lib.rs:120`) — read from the enum, not
guessed from the number. An out-of-window capture would have been
`ProviderWindow`, and a late one `ProviderFreshness`. This is neither.

### Where it comes from, in one function

`sponsored_push_v1.rs::authenticate_provider_program_pin` pins the Pyth
**Receiver** and **push oracle** programs by exact
`(ProgramData address, deployment_slot, upgrade_authority)` equality, and its own
doc comment says why: rehashing 1.64 MiB of ProgramData on every capture is not a
viable transaction path, so Loader-v3's monotonic deployment slot is used as the
proxy and *"any byte-changing upgrade fail[s] closed as `ReleaseSuperseded`"*.

Read off chain, and read off **this market's own release record** rather than
out of a constant:

| | pinned by the market | live on devnet |
| --- | ---: | ---: |
| Pyth **Receiver** deployment slot | **487,855,452** | **491,006,444** |
| Pyth push oracle deployment slot | 293,898,740 | 293,898,740 — **matches** |

The market's copy is `HnxCroTCtgkoaqA57kyyGrLmBxGM9ar3L6KidaRi7N8G`, 592 bytes,
carrying `receiver_deployment_slot` at **offset 560** and
`push_oracle_deployment_slot` at **offset 568**. Its source is the tree's
`PythSponsoredPushReleaseV1` constant at
`crates/dclutch-pyth-svm/src/sponsored_push.rs:500`.

**Pyth redeployed their Receiver on devnet.** The push oracle did not move,
which is what makes this a reading rather than an inference: one of the two pins
is exact and the other is 3,150,992 slots stale. The protocol failed closed,
correctly, and no timing could have avoided it.

### AND IT WAS ALREADY STALE WHEN COHORT-13 WAS FOUNDED

| | slot | relative to Pyth's redeploy |
| --- | ---: | --- |
| Pyth Receiver last deployed | 491,006,444 | — |
| **cohort-13's registry deployed** | 491,947,648 | **4.36 days later** |
| **cohort-14's registry deployed** | 492,225,646 | **5.64 days later** |

Cohort-13's resolution addendum concluded that its window closed unobserved and
that *"the honest observation for this window is permanently unavailable"*, and
attributed the outcome to timing: *"No code was wrong. Nothing ran at the right
time."* **That is true and it is not the whole cause.** Cohort-13's capture would
have refused `ReleaseSuperseded` at any second of its window, because the release
its market pinned was already four days out of date at founding. The lane that
followed built `devnet-sponsored-relay-schedule-v1` to fix the timing — good work
that this cohort used and that is still owed — and the wall in front of the
timing was never reached, because nobody ran a capture inside a window to meet
it. **Cohort-14 ran one outside the window, and met it early instead.**

That is the useful shape: an instrument aimed at the wrong conjunct reported
truthfully and left a nearer cause unmeasured. The thing that exposed it was
running the action at a time when it was *guaranteed* to refuse and then reading
**which** refusal came back.

### What this costs, and the one thing it does not authorize

- **The Direct market `FgzbVSWVp36R…` cannot be captured**, so it cannot reach
  `Resolved`, so its only reachable terminal is the funded failure walk.
- **The failure walk was NOT run.** It is reachable — cohort-13 proved that, and
  this cohort's certificate seat is already prepaid, so it would commit. It is
  not run because the runbook says so and the reason is a product reason, not a
  technical one: kind 4 means the honest observation was never captured, the
  founder keeps the pot, and *"shipping it twice would make an oracle outage into
  founder revenue a second time."* This lane stops at the wall rather than
  walking through the only door left open.
- **Nothing was spent.** The refusal was a simulation failure; the campaign payer
  is unmoved at 1.708618131 SOL across the whole attempt.
- **The fill, the settlement and the census are unaffected.** They never touch
  the provider pin.

### Owed, and none of it is repairable inside this market

1. **Re-mint `PythSponsoredPushReleaseV1` against the live Pyth deployment**, and
   give the constant a check that fails at PLAN time rather than at capture time.
   The staging tool reads the cluster already; comparing two `u64`s before
   founding a market against a superseded provider costs one RPC read and would
   have refused cohort-13's founding, cohort-14's founding, and this capture.
2. **A market pins its provider release at founding**, so this is not fixable for
   `FgzbVSWVp36R…` or for `6t3Znm…`. It is fixable for cohort-15, which should
   not be founded until (1) lands.
3. **A third-party deployment is an event this protocol has no watcher for.**
   The pin is right and the fail-closed behaviour is right; what is missing is
   anything that notices the supersession before a market is founded on top of it.
4. `devnet-sponsored-relay-schedule-v1 --wait` refused with
   `--max-wait-seconds 25000` and the runner then proceeded to act anyway. The
   wrapper's fault is real and worth naming — a wait that refuses must stop the
   sequence, not fall through it — and in this case falling through is the only
   reason the supersession was found tonight instead of at 06:12 UTC.

Devnet evidence. Not mainnet evidence.

---

## Addendum: THE PROVIDER WALL IS BROKEN — a second Direct market, founded on a release re-minted from the live chain

Written 2026-09-03 by the COHORT-14B lane. **Devnet evidence. Not mainnet
evidence.** No program was redeployed: every deployed byte is still
`8e96ec3f8cd4438040d9287d2489ea84587ebd5c`. Two host-tool commits carry the
whole fix and change no deployed byte — `12a9b13a5` and `3ba991025` — and a
third, `674a7873e`, closes a credential-at-rest defect this work exposed.

### THE `0x8014` CODE HAS TWO CONJUNCTS AND THE FIRST ADDENDUM NAMED ONE

The addendum above reads the Receiver's deployment slot off this market's own
release record and off the chain, and stops there. Reading the *other* branch
that returns the same code:

```
crates/dclutch-pyth-svm/src/sponsored_push.rs:500       receiver_config_digest
programs/dclutch-resolution-proof-sbf/src/sponsored_push_v1.rs:526
    if hash(&config_data).to_bytes() != release.receiver_config_digest() {
        return Err(ResolutionError::ReleaseSuperseded.into());
    }
```

| | pinned by market A | live on devnet |
| --- | ---: | ---: |
| `receiver_deployment_slot` @560 | 487,855,452 | **491,006,444** |
| `receiver_config_digest` @528 | `bbbc324e70a436d70522595f477a3104488f2b02417e207483880337cd383592` | **`f8aca67e2ab8d31e9be38bb0434d6e59e06871c80b744e9e0668a16f254c83a4`** |
| `push_oracle_deployment_slot` @568 | 293,898,740 | 293,898,740 — matches |

The Receiver `Config` `DaWUKXCyXsnzcvLUyeJRWou8KTn7XtadgTsdhJ6RHS7b` is still a
canonical 370-byte V2 body — discriminator `9b0caae01efacc82`, one Pythnet data
source, `fee 0`, `minimum_signatures 3`, canonical zero tail — so
`ProviderRelease` passes and the digest branch is reached. Its digest matches
neither the pinned constant nor either fixture in the tree
(`fixtures/pyth/upgraded-2026-08-26/devnet/receiver-config.account` hashes
`23a7a19c…`). **A re-mint that fixed only the slot would have founded a market
that refuses with the identical code**, which is the shape `AGENTS.md` names —
one undifferentiated code over many conjuncts — met from the reader's side.

### AND THE RECEIVER'S ELF DID NOT MOVE

`sha256` of the live ProgramData tail, offset 45 to the end:

| | live | pinned `abi_id` |
| --- | --- | --- |
| Receiver `96QrNCjmh32H9quY9DX4NEH81nECVsbkATBDZeoVbvLV` | `0d6bf9142c2eb1fdb8ead48054491369bed3a8d6846c5887e362844155599ab2` | **identical** |
| push oracle `8xAeURaAWExxyHUXJSgjsg5r96Ydr3G4cek2if7imQmz` | `845d84603cdef688e927521eb05be70996a0432c21f40f4cdeb3b7a59f44c4ec` | **identical** |

Pyth redeployed the **same bytes at a new slot**. That is exactly what
`authenticate_provider_program_pin`'s own docstring promises — Loader-v3's
monotonic slot is a *proxy*, and it fails closed on a supersession that changed
no code. The push oracle's digest matching is the positive control that makes
this a reading rather than an inference: the same convention, applied to a
program that did not move, reproduces its pin.

### THE FIX: THE RELEASE IS OBSERVED, AND THE CONSTANT IS THE DECLARATION

`12a9b13a5` adds
`tools/local-validator/bootstrap/successor/src/sponsored_release_observation.rs`.
Five accounts in one finalized `getMultipleAccounts`; nine facts are chain-owned
and re-minted, everything else comes from the constant because it says *which*
accounts the release is about. A moved ProgramData address refuses — Loader-v3
derives it from the program id, so a moved address is a different program.

**It was not fixed by retyping the constant.** Every consumer of
`devnet_sponsored_sol_usd_release_v1` is host-side, so editing it changes no
deployed byte — but the deployed Resolution ELF would then hash differently from
a build at its own revision, which is precisely the accelerator drift §5 records
as debt. The constant is left alone and the chain is asked.

Three consumers now ask the chain instead of the constant: the market compiler,
`authenticate_release` on the capture path, and the terminal input producer. All
three used to compare the Market's record to the constant, which agreed with the
constant and with nothing else — both stayed green while the chain refused.

**Supersession is not what this lane's brief believed.** There is no in-place
supersession of a provider release and the chain does not admit forward slot
movement: the pin is exact equality in both directions, and
`ArtifactReleaseV1::slot_pin_refusal`
(`crates/dclutch-registry-contract/src/artifact.rs:272`) turns a strictly later
slot into `ReleaseSupersededByUpgrade`. The one "forward movement admits" rule in
this tree is scoped to the Registry role under an infrastructure-succession plan.
A sponsored release supersedes by being a DIFFERENT RECORD: the 592 bytes are the
body, `sha256` of the body is its identity, and that identity flows through
`ProviderReleaseV1`, `SourceSpecV1` and `SourceMaterialV2` into the Market PDA's
own seeds. **Nor does the ladder publish it** — the publication stage covers
`plan.records`, the nine infrastructure bodies, and the sponsored release record
is published by `publish_market_records` during the FOUNDING. No ladder rerun and
no `prepare` rerun were owed.

### MARKET B, FOUNDED AND OPEN

Staged with cuts re-centred on a spot measured three ways: Pyth `$101.016809`
(conf ±`$0.008795`, EMA `$100.402605`, `publish_time` 50 s old), Coinbase
`$100.96`, Kraken `$100.96`. So `--cuts 9900,10300 --cut-denominator 100
--band-anchor 10102`.

```
the sponsored release is RE-MINTED against finalized slot 492293300; 2 chain-owned fact(s) moved:
  receiver_deployment_slot: declared 487855452 -> observed 491006444
  receiver_config_digest: declared bbbc324e… -> observed f8aca67e…
```

| | |
| --- | --- |
| Open Market | `DUVcCGfjXzp1fBktTCjsAomgrn9S6sxSDziQHoyRiu8A` — 368 B `DCLTCOR3`, Core-owned, phase `0x01` Open, readiness `0x02` Consumed |
| `selected_release_set` @208, read off chain | `398e51c008cc5f592f3252f0c1f2246e019ace000b04b74766a41cb45a8a3e09` — the SEALED plan's, a third statement of one identity for the second time |
| Found31 / abort Market | `Gd6hxzdA2BsWYscPzVrmGMKKnQuRJnEcudxMtG89694L` / `4sbU3jm6CKj8Mr3KWsPenFcadwk7mUGZDLd4KjePpDo6` |
| realm record | `CbwTTfijYeu98JzGARcvWqgMm6hPNgr7pp1TZDUP8xfy` |
| collateral mint / wallet | `3hgS9efP2BCoZUo8SY3UhHbS6RsKbtjzLeUywWukykyW` / `H3Ck3Nu35QzQxrUDtjQeif4su3xn5BXjxczUNXPZbtzH` |
| **provider release record** | **`BruLs2Hd3nWfJvFaznBoDDugZwRtR1gTDbHjDmPVVwv`**, 592 B, Registry-owned, `sha256 394795f32d4ec31bcbc76b38ebddd97334f70b34796d92c43ef16e3ca982f203` |
| its slot / config digest, read back | **491,006,444 / `f8aca67e…` — equal to the live chain** |
| `market_sha256` / `plan_sha256` | `2b0f5ae30099ace3dd96773aa3ee74dda81a2db260a727e9ca7539142619348a` / `ef378b70…` (the sealed plan, same as market A) |
| frozen tables | Found37 `2tzwxHBe…`, DCLTCFQ1 `5tvKBgfC…`, DCLTCF1A `41NK4qBR…`, DCLTPCB2 `3ojoxdL4…`, DCLTGMF3 `Hjav5EkU…` |

**110 transactions against market A's 192, and the difference is entirely record
reuse.** The non-record transaction count is **50 in both** — the founding ladder
ran identically, hostile probes included: both refused exactly 7 times, at
`Custom(4108)`, `Custom(12289)`, `Custom(12294)` and `Custom(16387)` twice.
Market A published 41 record bodies because it was first; market B published 18,
because **23 of A's records are content-addressed to the same bytes** and were
already finalized. 41 − 23 = 18, exactly. 5,210,638 CU and 8,285,000 lamports of
fee against A's 6,498,519 and 14,435,000.

Activation: verdict `ACTIVATED`, root `G7K2vSxq4acoTyVN3xitH2aMgccn69hCqUA15dedLryj`
registered in advance and read back `AccountNotFound` → **256 B `DCLTCRT1`**,
Trading-owned, 2,431,872 lamports. Deadline slot 492,509,337.

Certificate settle seat `6prkttkSfQG59MVUDKJ1VaAbdZiF2umYcCv2y5z1KQ37`, **2,786,520
lamports** = `rent.minimum_balance(312)`, signature
`3jmqg4758vVHvsh34TVF5xgES7iN1DHbnSDeBNRioEaGWBzySbmYHfbmXCTv91jN6wvZwQNq5i9Ce8kURysxU72B`,
slot 492,304,619, 450 CU, 75,000 fee; payer −2,861,520 exactly. The prepay ran
through `ab0322d50` and `3e5e0b0be` with no refusal — the second market is the
first time those two fixes were exercised without a wall in front of them.

### THE FOUNDING SOURCE FUNDER IS A PER-MARKET COORDINATE

`3ba991025`. `KeyOriginV1::Persisted` returns the operator's file key verbatim at
index 0, which is right for a role the operator FUNDS out of band and wrong for a
role the campaign CREATES AN ACCOUNT AT. `founding-source-funder` and
`founding-projection-witness` now derive from the secret AND the Open Market's own
address, so one key file founds any number of markets and a resume of the same
market lands on the same addresses.

**Market B was founded with market A's own funder key files** — the exact
collision the first addendum records — and the instruction that refused
`Create Account: account G2dfBofa… already in use` succeeded:

```
campaign transaction: slot=492300176 fee=80000 compute_units=3979
    fund the founding principal supplier and its rent-capacity witness
```

An unbound per-market draw is a hard stop rather than a fallback, because both
obvious fallbacks are worse than the bug: falling back to the file's key IS the
collision, and falling back to a fresh key SUCCEEDS — the campaign funds the
funder itself — so the founding would complete at an address whose private key
dies with the process and the market's principal supplier would be unrecoverable.

**The test for it was vacuous once, and that is the finding.** Written against
`PER_MARKET_ROLES`, emptying the constant makes every role "a campaign identity",
every equality holds, and the pre-fix behaviour passes. It now asserts against an
independent statement of which addresses cohort-14's founding was OBSERVED to
create accounts at; with the constant emptied it fails `the founding creates an
account at founding-projection-witness and two markets share its address`.

### THE GENERAL MARKET IS FOUNDED AND ACTIVATED, and it cost a quarter of the stranded attempt

Recompiled rather than reused: the stranded attempt's `market.json` pinned the
stale provider release, so it would have founded a second uncapturable market.
`231,095` bytes again, `accelerator-observation.txt` reporting `deployment_slot
491959038` -- the runbook's verifier -- with `elf_digest 61b2d73d…` and
`artifact_release_id dcce810097111f696e4888ef89385fc0e1e1c24d89818ca040d950d1daec5b94`,
byte-identical to the `content_sha256` of the record the ladder finalized in §5.
Two independent authors, one digest, for the second time.

| | |
| --- | --- |
| General Open Market | **`8ExdC1RwbyuJweEqT1F6Gk9rgN87uuVaLwtaY2wmr5x`** -- 368 B `DCLTCOR3`, Core-owned, phase `0x01` Open, readiness `0x02` Consumed |
| `selected_release_set` @208 | `398e51c0…` -- the sealed plan's |
| Found31 / abort | `7o71Rsih9Vmk7z7f5WB43wLPWPuN3foJjkRhz584cCdx` / `H2YeZwfScZ8wLhri3CCGVcwKX1Y9oQE5pswKX9es22Kb` |
| realm record | `3hfUWGP8B76eNe2dffLdEMMmYtvGmgMnjQzzHvZNN4s7` |
| collateral mint / wallet | `Ha6ytM6o4zaHKKX8FnLjmj2GQ2L24uF57kXAx66Mbz1u` / `3AcLfkZh413ixoBbJyqWuC5ZcExJA1djz8VPwodPjvwb` |
| activation | verdict **`ACTIVATED`**, schema `dclutch-devnet-general-capability-activation-report-v1`, entry index 3, root `JDsNG7Tdj55pq81AVbcgymuXY763tDy7mHWsPyePJy9j` `AccountNotFound` → **360 B `DCLTCRT1`**, Trading-owned, 3,090,504 lamports |
| deadline slot | 492,513,869 |

**Cost 0.198318983 SOL against the stranded attempt's 0.812936971**, and the
ratio is the record-reuse arithmetic stated above rather than a cheaper founding:
this run paid for no record body the stranded attempt had already finalized.

**It was founded with the SAME `founding-source-funder.json` and
`founding-projection-witness.json` files the Direct foundings used.** That is the
whole proof of `3ba991025`: three markets, one key file, three funders.

### OPENBATCH N=2 IS NOT REACHABLE FROM THIS TREE, and the reason is not the market

The runbook's row 14 says "OpenBatch N=2 against the activated General root,
through the successor". The successor has no such caller, and this is a
producer-missing shape rather than a configuration problem:

* The only General hot driver is
  `local-private-validator-general-hot-campaign-v1`. Its flags are `--rpc-url
  --accelerator --caller --payer-keypair --account-dir --journal-dir --evidence
  --outcome-count`. **There is no `--market`.** Run against this devnet endpoint
  with `--outcome-count 2` it does not refuse -- it plans 11 General steps and
  writes a genesis ACCOUNT DIRECTORY, then says *"Start a validator with
  --account-dir pointed at the account directory"*. It is a local-validator
  harness that cannot address a founded Market at all.
* Its own module docstring says so: the accelerator half it drives is the
  read-only evaluation half, and *"the commit half -- Trading's
  `process_hot_execution_v3` writing the capability root and returning a
  `DCLTHAK3` ack -- additionally requires a founded Market whose capability
  manifest selects General, and no driver in this tree founds one."*
* `ASPIRATION_LEDGER.md` M-40 records `build_general_hot_instruction_v3` as
  having zero callers; its only exercise is
  `programs/dclutch-general-accelerator-sbf/program-test/tests/hot_instruction_v3.rs`.

**One half of that sentence is now false**: a driver in this tree founds one, and
`8ExdC1Rwb…` is Open and activated with a General capability root. What is owed
is a devnet General hot caller that takes a `--market` and routes the `DCLTHOT3`
envelope through a frozen table. That is a build, not a run, and this lane did
not do it -- the budget went to the resolution chain, which had never been walked
at all.

### THE ADMISSIONS, AND THE FILL THIS LANE DID NOT RUN

Two stranger admissions landed on market B, both authenticating the sealed plan's
`plan_sha256 ef378b70…`:

| | position | signature | slot | CU | fee |
| --- | --- | --- | ---: | ---: | ---: |
| participant-1 | `EXR2DrEE4BFtGuCMzWs8Ye446VWD2FRwubkaGxPPXQeW` | `2y4w3esS39SDi41DnUTyYKDtCBErT81uAnJes1fbrLoJW2P5jq7cuUJtjBXD8DzBrN6xXUfo3nkqkevEfjuyG1F1` | 492,315,744 | 206,486 | 80,000 |
| participant-2 | `9Sv8LTYhUavEsx6A6UHxfTDEDhHKuxZypPAMkmTv2dG6` | see `$JOB/sim-b/admissions/participant-2.json` | | | |

**The collateral delegation and the nine-transaction fill were NOT run**, and the
reason is a judgement rather than a wall: the trade chain needs `trade-facts.py`,
`author-tickets.sh` and a second simulator work dir re-pointed at market B, the
window was ninety minutes away, and a half-landed session would have grown the
census aperture across the capture boundary -- reproducing exactly the L7 defect
§13 records. The fill's own number, 1,284,573 CU, is cohort-14's and is not
re-measured here. It is owed.

### THE CENSUS APERTURE IS FIXED BEFORE THE BOUNDARY, WHICH IS §13's LESSON APPLIED

`census-b/cohort14b-pre-capture.json` is taken with the complete aperture already
in place -- mint, Hoard `BrLJBohX4W6sLe3N9z21KqRuiDKdyG7XWnRuv4sVNQFr`, aggregate
`7oUuHAVJsZtfwRwNLL8TLWBzQrve9tL4iay5k6iWNVbP`, the founder's collateral wallet,
**all three** Positions, and both participant owners as watches:

```
HOLDS L1: tracked 1000000000 atoms across 2 accounts == Mint supply 1000000000
HOLDS L3: 3 Positions sum to the aggregate supply vector [500000000 x4]
HOLDS L4: Hoard 500000000 >= worst outcome 500000000 x unit 1
INAPPLICABLE L2 / L5 / L6 / L7 / L8: the first census has no predecessor
```

Four INAPPLICABLE, every one of them because this is a FIRST boundary, each
saying so in its own words. The second boundary is the capture, over the same
bindings, with the capture's own fee declared -- which is where L7 becomes
applicable for the first time in this cohort.

### THE STAGING GENERATOR WROTE THE ENDPOINT CREDENTIAL TO DISK

`674a7873e`. `market-open-staging.json` learned on 2026-08-30 to redact its
origin. The executable the same function writes two blocks earlier baked
`--rpc-url` as a literal, so `market-b/open-market.execute.sh` held the key at
rest in a mode-700 job directory. Cohort-14 hand-edited its own copy afterwards,
which is exactly how its `HOLD_STATE` came to state that the file never held it —
true of the file, false of the generator, and nothing distinguished the two. A
credential-bearing endpoint is now not written at all, and the generator greps
its own output and deletes the file if it finds one. Both controls run.

### ORPHANED RENT: cohort-14's stranded General founding

The partial General founding is abandoned in place. Read back off chain, with the
exact lamports:

| account | bytes | lamports | state |
| --- | ---: | ---: | --- |
| Found31 Market `JEG8H7qSJGxiCSKpXr2pg6oSPrTofoiHjLjX7RCiDdWK` | 368 | 3,141,168 | `DCLTCOR3`, Core-owned |
| projection witness `AkSjTgMKq94r6ssds2Fgj4rAsExgACxAa3uU6QPmqHzJ` | 0 | 3,141,168 | System |
| collateral wallet `FuRBL5tx329YDuGxEKb3p1pFKnkfsUUaup6AmKKMYQZs` | 165 | 1,855,569 | Token-2022 |
| source funder `G2dfBofaGFRtTSsWiwNeibD7niwoMS9XsLUAFC1c93uJ` | 165 | 1,855,569 | Token-2022 — created by the DIRECT founding, which IS the collision |
| Realm record `Bji6ADSmRReUu8B9Fw6h7PdEZgSJB7AfXpcuvzD6B1Rg` | 112 | 1,519,920 | `DCLTRLM1`, Registry-owned |
| collateral Mint `2si3RLx8qxJPfsw2kStZGQoxuAkKuw2nJwFASf5M96kS` | 82 | 1,329,930 | Token-2022 |
| General Open Market `DL675bt1dmQQ87UeGBCnmV1zQTWsvd7jbwLmVqT2tsW9` | -- | 0 | **AccountNotFound -- never created** |
| | | **12,843,324** | **= 0.012843324 SOL** |

**Most of that founding's 0.812936971 SOL is NOT orphaned, and "the founding
bought a finding rather than a market" is the easy sentence rather than the exact
one.** Only 0.012843324 SOL sits in the six per-market accounts above. The
remaining 0.800093647 went to transaction fees and to RECORD rent -- and records
are content-addressed, so a re-founding of the same market graph pays for no
record body whose bytes are unchanged. Market B demonstrated exactly that
arithmetic on the Direct side: 23 of market A's 41 record bodies were already
finalized and cost it nothing.

Devnet evidence. Not mainnet evidence.
