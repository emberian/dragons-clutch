# Cohort-12: a fillable market at last, and the wall standing behind the one we knew — 2026-09-02

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized.

Tree root `/Users/ember/dev/dclutch`. Every deployed byte is built from
`e39efbb0b31afeb7a03b10a71b6e2e5d6da0e040`, the commit
`COHORT12_RELEASE_GREEN_2026_09_02.md` recorded the first green
`checked-release-candidate.sh --genesis-cohort` at. That file staged this work
and stopped deliberately before the first irreversible step; this is the sequel,
and it spends.

**Headline.** Cohort-11 is closed, cohort-12 is deployed, laddered, and founded
at exactly 50 basis points on cuts centred on a measured spot; two strangers hold
Positions and the buyer holds an exactly-delegated allowance; five routing tables
are the first on-chain proof that `dc07c73a` froze them. **The fee-bearing trade
did NOT execute.** It is blocked by a wall nobody had measured, and the wall is
structural rather than incidental: a full-redeploy cohort can never produce the
checked execution release a Direct fill demands. Section 8 is the finding.

## 1. Cohort-11 closed

Irreversible, and the reason the redeploy could be afforded: the deployer held
38.738044775 SOL and the seven new programs cost 42.03. Program ids derived from
cohort-11's own keypair files, never transcribed from its evidence doc.

| role | cohort-11 program id | rent reclaimed (SOL) |
| --- | --- | ---: |
| trading | `4fhQyBPgvaZw6jEWwT3U64tHfgTNRPuWuH5MjPLrxjzk` | 14.487909105 |
| core | `FinXxc9drpmCYA7Cy4aGWSa1jYY87K6pNPfY9qFWzJCF` | 7.556972577 |
| claims | `HQYqqdzn5s6tEM6ywgeCr7Bd56tEuhpoop3ruvHRfAq6` | 8.654608137 |
| registry | `ADB72ar6ZSstXEg76Q1bPb5UY2EGmH6mrVfwr8K2fzom` | 1.486412097 |
| rent | `HA31aDmTnLFjBYQoCXeyRBsfdddxne1apyfzfq4tSp8e` | 0.902408169 |
| custody | `Cdh8Vv7DRyk7rhLcee574potYfaiVEsYR5HUPCrNPzCB` | 3.619974465 |
| resolution | `3WqTxq6uKMK2d9f6uRujh8hCZvVB78KjGo9AYxvPQNVM` | 5.183820153 |
| | **total** | **41.892104703** |

Deployer `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`: **38.738044775 →
80.630114478 SOL**. The 41.892069703 that actually arrived is the table's total
less seven 5,000-lamport fees, which is the arithmetic closing exactly.

Cohort-11's market is now unreachable. It could never have taken a fill anyway —
founded at 30 basis points, where `direct_token_setup_v1` admits only 50.

## 2. The redeploy

Seven programs, fresh identities, each verified by dumping the on-chain image
back **before the next one started**. Every dump is byte-identical to the ELF:
the prefix compares equal and the on-chain tail is all zero.

| role | program id | bytes | ELF SHA-256 | deploy signature |
| --- | --- | ---: | --- | --- |
| registry | `5c4CfHXHaLoJRtVSZFURp6Qhub8P4x8Hk4yZ3KJNrK53` | 234,536 | `ed70f8bda12b77d663126218ad05f36dd77c5bf3100642879cef1441a845afe7` | `2ShFZNRHJfqtqK7sKYmkv8b5S25wNhhUUD2ExEUqvtTPqJZZXvv9nz4VZoq4GfanurnJVAfT1w2tswKXW4sfhQ8T` |
| rent | `HD72aKvtRzBrVdmDGn8UrcocVA6g4NuG9Bt94GRLMYcW` | 142,320 | `d46e5f0a64fd7d5e296118c2e7a62a3b67aed2c2ac4420e85069fb8dca632837` | `5qn1XVjNSWvd5avBZLjQbRsNFSUroLCA57GnpCuwaUdJeU7djtxy8yJR9Hh5kvBKs3i27s7EozjCYCGNXuTZHwCm` |
| custody | `2MHNgYoCtDzqRryjgAxzFwLVPztSN6NTUr7RmjiMrcLc` | 571,432 | `2823c82351638566e295d7f7acc2e559ab61b3ea43750759e84f73bc0f80d567` | `41vXQzNG8fxytGEtsMz7M8mmj1ACgEjUDsf5GqSB3AY97H7ZAE7FPRm6hFzB6NtReUcuxGs5gf6EqL8wsCVJuqpp` |
| resolution | `9vs7atqDTAZTMo2a9iMZXD6Nf39jQZ7sZFf2X4pGDDvs` | 818,368 | `307bc81c604f1a3c52a0dc5ff1b66b094f12faf6be8f8d66b7d337e08c8873e0` | `2WzwBeremdsUA2s2PanpvUP5jDU6iChEmef3XDrLLrxLm35geQM9Gix1424t84LLxyX9b124r5c58ijB4QUu8JYh` |
| claims | `GwduZB13AgqLxsoxi8wZEQndYBsQERea35dhuYKJzCvc` | 1,366,416 | `268d527e600706b9062921e0a35f0ea2ba13f5bc7790a4351b6b7f0fff5e910f` | `3fA5YCCwFMzc3aEKtDehTbhgYhzAw3uMZw8kQn3Sx3wtMrYy8dsYDkFmj5yLwBzSvjUhreoCu31YJ57vvrRAvsG7` |
| trading | `Ahzug4zYhG8sc4t6tXjaSjnqbv7bTkgNYRc4kWUxYGJe` | 2,308,320 | `b0cff55ab0ef162d7e427b8cb894f1468b1804d997ab35c52710df3268a8e3ed` | `4UhtrTbaYgKf65NGUyrUG5kGm7GsGtuW78bJyj7y65DrQo97cScRjiB1a4kfstkuLG5HFVsi9XX8RWLSYT3AZYuL` |
| core | `G4Wz4fj4zqBPFWYFF9CeYeJtTK5UqSZUu2fyCr9ANjYG` | 1,187,432 | `9ef7df559565effb780db6b26bf9fd3c89cefb2b86ae5205d37c688d1a5ea58b` | `2fjKzdinW7XjDiGoBQdWjtbWLjYVn2tX1SSLYijc6g4tak5XbnsXHNwhuTHdecxFWhWy1SnUnm9umNhQ21AokDaX` |

The seven ELFs were built **twice, in two independent detached worktrees at
`e39efbb0`**, on the ordinary release invocation rather than the candidate's
`--features hot-cu-profile` trading link, and are byte-identical across both
builds. Registry, rent and custody are byte-identical to cohort-11's; the other
four carry everything between `8ae2c9c9` and `e39efbb0`, Direct's frame repair
`58b077f8` among them.

**All seven are mutable under the deployer's ExactAuthority**, as cohort-11's
were, read back off chain rather than assumed:

| role | ProgramData | authority | last deployed slot | data length |
| --- | --- | --- | ---: | ---: |
| registry | `FbfJYjjyULLJYX8wzXqv5M5U2fxWLhCwwhvWaxCfWAsu` | `4zrxtw5c…` | 491,871,867 | 234,536 |
| rent | `7kuFuNDjMGTeYqodG3Aq8ynSBLdN6rLQsSWDyF5mvMGp` | `4zrxtw5c…` | 491,871,910 | 142,320 |
| custody | `HCiJ7DfkqLagqQPfGQ5sXUXPyMuttds8EvtSQGEaVH4f` | `4zrxtw5c…` | 491,871,984 | 571,432 |
| resolution | `2XiYgQfRAKivMxWaTZLpRZbXtvN14WUKGv7acriTWhcW` | `4zrxtw5c…` | 491,872,069 | 818,368 |
| claims | `9gA7cT9mhFwDLjFzWYprzE44Vzu8U9e8tgArrSAL3uJL` | `4zrxtw5c…` | 491,872,186 | 1,366,416 |
| trading | `Gx2b9xPmUcyLN79DHy9ExncTnAmedXX3pVmRpG2zVqqA` | `4zrxtw5c…` | 491,872,374 | 2,308,320 |
| core | `6wzSk1ip5uuiiqQoCDUuJPNjDZdHtdaJZL5DSQHG4kcy` | `4zrxtw5c…` | 491,872,487 | 1,187,432 |

And a third, independent agreement: `capture-and-prepare.sh` hostile-decodes
each live ProgramData account and hashes the ELF tail it actually carries. For
all seven roles `live_elf_tail_sha256 == built_elf_sha256`. The dump comparison,
the byte count and the tail digest are three different instruments reading the
same claim.

Deployer **80.630114478 → 38.601927539 SOL**; the redeploy cost
**42.028186939 SOL**.

## 3. The ladder, re-observed from chain

`campaign --through activation`, executed with the deployer as Core upgrade
authority. The runbook's ladder script refused first, and correctly:

```
Error("campaign omitted required keypair paths: campaign-payer")
```

`administration_required_roles_v1` adds the campaign payer whenever the
succession stage is `Absent` or `Partial`, and it is `Absent` here. So the payer
was funded first and the flag added — the refusal is the tool declining to open
a signer for a stage it cannot pay for, before any key was read.

33 transactions, **zero errors**, 4,084,967 CU, 2,475,000 lamports of fee: 27
record publications in Begin/Append/Finalize triples for nine records, one
`initialize Core infrastructure profile` (both profiles, one instruction, 254,739
CU), and five `activate immutable release-set role` — Core 635,845, Claims
714,192, Trading 1,184,004, Resolution 449,898, Custody 338,997 CU.

Succession executed nothing, and said so rather than being assumed:

```
campaign stage succession: nothing to execute -- this cohort is born at V2
and carries no ceremony; observed complete
```

**Re-observed after the run**, by a second preflight that reads the chain rather
than the driver's own exit code — the discipline cohort-10 paid for with 2 SOL of
landed work its tool had reported as failed:

| stage | state |
| --- | --- |
| substrate | complete |
| publication | complete |
| initialize | complete |
| succession | complete |
| activation | complete |

Deployer `38.601927539 → 36.564742699 SOL`, which reconciles exactly: 2 SOL to
the campaign payer, 5,000 lamports for that transfer, 2,475,000 lamports of
ladder fees, and 34,704,840 lamports of rent now sitting in the nine published
records.

## 4. The market, founded at exactly 50 basis points

### The cuts were re-centred, because the runbook's were three months stale

Cohort-11's `--cuts 14800,15200 --band-anchor 15000` centred a **$150** SOL. Read
on 2026-09-02 from three independent venues — Coinbase $100.005, Kraken $99.970,
CoinGecko $100.04 — and then from the account the market itself resolves against,
the sponsored devnet PriceUpdateV2 `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE`:

```
price 10003917148  expo -8  conf 1082854   =>  $100.03917148
```

Founding on the runbook's numbers would have asked whether SOL ends between $148
and $152 when it is worth $100 — a market whose answer is already known, which
is precisely the defect `centred_cuts_v1` and the founding band exist to refuse.

So: **`--cuts 9800,10200 --cut-denominator 100 --band-anchor 10004`** — $98.00
and $102.00 straddling a measured $100.04, with the anchor stating spot rather
than a round number. Under the triangular model the band implies (volatility 200
bps over a 10,000-slot window, three plausible half-widths: a characteristic
displacement of $2 and a plausible range of $94.04–$106.04) the three ordinary
cells carry roughly 22% / 56% / 22%, well inside the 9,000-bps ceiling
`MAX_CELL_EX_ANTE_SHARE_BPS_V1` enforces.

The rate is the one thing that could not be got wrong twice:

```
directFeeBasisPointsPerSide        50
directTokenSetupAdmitsThisRate     true
feeRateIsIrreversible              true
maximumGrossCollateralAtomsWhoseFeeFloorsToZero  199
```

Four devnet markets before this one were founded unfillable — three at 0 and
cohort-11's at 30 — because every stage before the fill is indifferent to the
rate. This one is the first that `direct_token_setup_v1` will admit.

## 5. The routing tables, and the first on-chain proof of the freeze

Since `dc07c73a` — *"routing: every routing table is frozen, because a mutable
one is both halves of the route"* — `publish_routing_table` refuses unless every
table it publishes reads back owned by the Address Lookup Table program, with
**authority `None`**, **`deactivation_slot == u64::MAX`**, **last extended
strictly before the observation slot**, and byte-exact addresses. `dc07c73a` is
an ancestor of `e39efbb0` and is **not** an ancestor of `8ae2c9c9`, the commit
cohort-11 was built from. No cohort has founded since it landed, so cohort-12's
founding is the first on-chain proof, and cohort-11 is the control.

Read **by address, never by scan** — `8fda79bf` established that
`getProgramAccounts` over the ALT program answers an *absence* on devnet rather
than a refusal, so the addresses come from each founding's own
`CreateLookupTable` transactions, where the table sits at account index 0 of its
own instruction.

**The control, cohort-11 at `8ae2c9c9`:** five tables published, and only the one
its own label calls frozen is authority-less.

| table | label | authority | addresses |
| --- | --- | --- | ---: |
| `6in3trBkcQYGVhq2pRkgiwq4YYYvmm5PX7ExDB7QuKW5` | Found37 | `3CZxcpGp…` **mutable** | 36 |
| `7a6vYnjrz1F4fwcj8tUhXEMJcGRHWRUnTpdfuRRf9TVS` | DCLTCFQ1 | `3CZxcpGp…` **mutable** | 46 |
| `EMtgeNZMvcgY5xfbC7k2XXEYz1AM2L92keDqH6AXV8Dd` | DCLTCF1A | `3CZxcpGp…` **mutable** | 16 |
| `6Q5Gvq21kQRp5rcEccv1xvAqUtokSfCAtHsH1ALph2Nf` | DCLTPCB2 | `3CZxcpGp…` **mutable** | 57 |
| `6Pwb16HHphgvDbr6RW4p7k82qTGDccQHizJzk3LDXZwk` | DCLTGMF3 frozen | **None** | 64 |

Four of cohort-11's five routing tables are still held by its campaign payer
today. That is exactly the shape `dc07c73a` closed: a table whose authority
survives its publication is both the route and the power to change the route.

**Cohort-12 publishes five, and all five read back frozen.** Re-read by address
at observation slot 491,885,451, every one owned by the ALT program, not
executable, `authority None`, `deactivation_slot 18446744073709551615`
(`u64::MAX`), last extended strictly before the observation slot:

| table | label | authority | deactivation_slot | last extended | addresses |
| --- | --- | --- | --- | ---: | ---: |
| `83ZLegT7FhPJkSRSV66T7LfwTHzu9wL26WqFqns6cU7b` | Found37 frozen | **None** | `u64::MAX` | 491,882,176 | 35 |
| `8qpF6HVeEoceSgfY3LBqqmQRoxnpfnh5sz3SLcpU5rdj` | DCLTCFQ1 frozen | **None** | `u64::MAX` | 491,883,111 | 45 |
| `9gDFww7wUbAzdT3ywudT2moCNQXACdCzdYkL95A3FZXw` | DCLTCF1A frozen | **None** | `u64::MAX` | 491,883,360 | 15 |
| `7HWmjshupa7hEWigbAG2FHJjqqfhqSN2PLGEH9DdoYzH` | DCLTPCB2 frozen | **None** | `u64::MAX` | 491,883,612 | 56 |
| `HvZktruUHznQNC2CY93BR7bARaZrHLM2zmCkQQ9eNdoE` | DCLTGMF3 frozen | **None** | `u64::MAX` | 491,884,712 | 62 |

The transaction labels changed with the code: cohort-11 emitted *"create Found37
routing address lookup table"* and no freeze for four of the five; cohort-12
emits *"create Found37 **frozen** routing address lookup table"* and then
*"freeze Found37 routing table after its one complete extension plan"* for every
one. **`dc07c73a` is proved on chain.**

**It has a cost nobody had measured, and it is the founding's new dominant
term.** A frozen table is usable only strictly after the slot that last extended
it, so each freeze must finalize before the next table is created. Cohort-12's
founding took **186 transactions over roughly 40 minutes**, against cohort-11's
188 over about 26 — the extra quarter-hour is four additional
freeze-and-await-finalization barriers, not extra work.

## 6. Which address is which, read off chain

A founding produces **two** Core-owned `DCLTCOR3` accounts of 368 bytes and the
campaign labels do not say which is live. Read at byte 10 (phase), byte 11
(readiness) and the `u64` at 272 (identity generation), with each one's Claims
aggregate derived rather than inferred:

| campaign label | address | phase | readiness | generation | derived aggregate |
| --- | --- | --- | --- | ---: | --- |
| `market` | `9jJqs6UA1KhCspJZ3ACW9yq9cn9Gp13zbkhTrLj1FdH2` | `0x00` Founding | `0x00` Prepaid | 1 | `2J2vfzg2…` bump 254 — **VACANT** |
| `founding_market` | **`EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1`** | `0x01` **Open** | `0x02` Consumed | **2** | `GZzSimhqU5o7XDoUCSMq1szM7xt29Ae9zwExMbpa48WU` — exists, 288 B |

**The OPEN Core Market is `EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1`**, and
the founding record still in `Founding`/`Prepaid` is `9jJqs6UA…`. The same
instrument reproduces cohort-11's addendum exactly — `ARuPAuyJ…` Founding/gen 1
with a vacant `ERd4rTg9…` at bump 253, `3rBfDBpa…` Open/gen 2 with `5wdhigoU…`
at bump 250 — which is the positive control for it.

Note the generation: the Open market is **generation 2**, not 1. Cohort-11's
prepared trade terms table says `--generation 1`, "the Open Market identity
generation". That is wrong in both cohorts, and the producer reads the
generation off chain and refuses a ticket that disagrees.

## 7. Two strangers admit, and the buyer arms itself for the fill

Through the load simulator, against the live Open market, fee payer the campaign
payer and never the deployer.

| what | address | bytes | owner |
| --- | --- | ---: | --- |
| participant-1 owner | `Frvzdn6QupyCGRQEbXo7kCgkuxYLWYFfwiJzbLSdtF9Q` | | |
| participant-1 Position | `AKYMkQD4BZKZytdGAh9v2BkfC8jY2tdDqqkun4o1EFRn` | 160 `DCLLBP02` | Claims `GwduZB13…` |
| participant-1 admission | `FLpPZvTCTFBgJsKA7UTt9Yw8GYZYkKVSQNxxwwEvMFfa` | 512 `DCLPPS02` | Claims `GwduZB13…` |
| participant-2 owner | `DNX9dDu4YyGgGGGAfaCqw6ed5EEZWsu33AYg55tSPL8Y` | | |
| participant-2 Position | `GLgV7gJJdaLbtJ6bx1dPNDvbqZsCW7UYYcfq4AgDgBB6` | 160 `DCLLBP02` | Claims `GwduZB13…` |
| participant-2 admission | `8ARZMtT9EYM9m7saymDcZ9yafwuz6s6vFwmsrM9GZYUu` | 512 `DCLPPS02` | Claims `GwduZB13…` |

| leg | signature | slot |
| --- | --- | ---: |
| participant-1 admission | `25JqtNCRjVHeCEEnFiiMSbLtPF1JYjfFZPQCirMFaWUtKcUrsczJYqLJiQursiTm7Y5MAKDhXvHZNNv2yuyXJ2dq` | 491,887,654 |
| participant-2 admission | `4RoGqjMDfB3ZsUGcPyCjBnmEZpDJP7U4Df95Cjx8ukz3oqdnnPtcDUdE7gczdxots7efpVxSm1Fwq959N7HtHc2m` | 491,887,968 |
| participant-2 collateral | `5aegz3ChQSKmVgxtsBPmwdHYwoMZrrW3QxixDKnbDTEvdkjfbBfm4kGGr5dooVuExRB7JRAkQVExjSvpqVfU5FzW` | 491,888,047 |

Cohort-11 stopped short of the collateral leg on purpose, because its market
could never trade. This one ran it, and the buyer's account reads back exactly
what the producer demands — **`amount 201`, `delegated_amount 201`**, an equality
and not a floor, because the allowance authorizes one trade and is spent to zero:

```
9GgaqNyk4prm2zwoGvzfFZddPLi1qkGQMtNBSr5VzWdY  165 B  owner Token-2022
   amount 201   delegate present   delegated_amount 201
```

201 is `gross + buyer_fee` for the fee-bearing trade this cohort exists to
demonstrate: fill 200 at limit price 1,000,000 against a price scale of
1,000,000 gives gross 200, and `floor(200 x 50 / 10_000)` is **1** — the
smallest fill at 50 bps whose fee does not floor to zero.

### The preflight run poisoned the execute run, and the chain caught it

`simulator.py run` without `--execute` writes a durable `Planned` admission
journal. The subsequent `--execute` run **resumes that journal** rather than
replanning — and the prefund branch is taken only under `--execute`, so the
resumed plan still carried its two System top-up transfers inline. The Position
owner is debited by them, a debited account is writable, and
`UserPositionAdmissionFrameV1` requires the owner to sign **readonly**. The
program refused:

```
Error processing Instruction 4: custom program error: 0x4003   (TradingSbfError::Content)
consumed 12233 of 1399400 compute units, no CPI
positionTopUpLamports 1823904 + admissionTopUpLamports 4053120 = 5877024
instruction 0: System, Frvzdn6Q… signer WRITABLE
instruction 1: System, Frvzdn6Q… signer WRITABLE
```

**12,233 CU and no CPI is cohort-11's Wall 3 signature exactly**, and the cause is
the same message-level owner promotion — reached by a different route.
Cohort-11's repair (`require_fee_payer_never_declared_readonly_v1`) closes the
fee-payer half; this is the *rent* half, and the driver already closes it too,
by prefunding in a separate finalized transfer. What is not closed is that a
**preflight journal survives into the execute run and skips that branch.**

Nothing landed: the signature `7fc5cLEJ…` was confirmed **absent** on chain and
all four PDAs **vacant** before the journals were archived (to
`sim-archive-01-preflight-planned/`, never deleted) and replanned. The replanned
admissions carried zero top-ups and both finalized first try.

**The lesson is narrow and worth writing down: on this driver, a preflight is not
free.** It leaves a plan the execute run will adopt, built under different
assumptions than the one that will spend.

### The census caught the collateral leg, and was right to

With the two Positions admitted and 201 atoms moved, the first census **halted
loudly**:

```
VIOLATED L1: tracked 999999799 atoms across 2 accounts != Mint supply 1000000000;
             201 atoms are in accounts this ledger does not name
```

Exactly the 201 atoms the collateral leg had just moved, into an account nothing
had named. Not a protocol divergence — a law asked to check a total over a set
missing a member. Three bindings were added: the buyer's delegated collateral
account in `census.tokens`, and **both** admitted Positions in
`census.positions` (the buyer's before the fill, or L3 falls short by the traded
atoms at the traded outcome). With them supplied:

```
HOLDS L1: tracked 1000000000 atoms across 3 accounts == Mint supply 1000000000
HOLDS L3: 3 Positions sum to the aggregate supply vector [500000000, 500000000, 500000000, 500000000]
HOLDS L4: Hoard 500000000 >= worst outcome 500000000 x unit 1 = 500000000
INAPPLICABLE L2, L5, L6: the first census has no predecessor
INAPPLICABLE L7, L8: external census -- the transactions between boundaries were
                     not driven by this ledger
```

That is the **pre-fill baseline**, and it holds.

## 8. THE TRADE IS BLOCKED, and the blocker is structural

Everything the fee-bearing trade needs is in place except one artifact, and that
artifact **cannot be produced for any full-redeploy cohort**.

`devnet-checked-execution-release-v1`, given cohort-12's plan and the five
`checked.bin` from the green candidate, refuses:

```
REFUSED: [checked-execution/plan-unsealed]
         devnet plan omitted its authenticated permanent checked deployment set
```

The conjunct is one line — `direct_hot_route_manifest.rs:375`:

```rust
let set_pin = plan.checked_upgrade_set.as_ref().ok_or_else(|| { ... })?;
```

`plan.checked_upgrade_set` is populated **only** by `prepare
--deployment-set-journal` (`main.rs:1411`), and that journal is audited by
`devnet-deployment-set-journal-v2`, whose own documentation states the shape it
demands:

> Registry and Rent must be **CarryForward** under one exact finalized
> nine-account snapshot and exact live dumps; Custody, Resolution, Claims,
> Trading, and Core must be **receipt-backed Upgrades**. … only two fresh
> carries plus five fresh receipts produce the v2 final digest.

**A full redeploy performs no upgrade, so it can produce no Upgrade receipts, so
it can have no deployment-set journal, so its plan can never be sealed, so it can
never yield a checked execution release, so it can never take a Direct fill.**
The command is an auditor, not a producer — handed a path that does not exist it
answers `set journal … cannot be inspected: No such file or directory`.

Measured across every cohort on this machine, and it is not a cohort-12 accident:

| cohort | lineage | deployment-set journal | `plan.checked_upgrade_set` |
| --- | --- | --- | --- |
| 7 | upgrade | `upgrade/deployment-set.json` | **SEALED** |
| 8 | upgrade | `upgrade/deployment-set.json` | **SEALED** |
| 10 | full redeploy | absent | None |
| 11 | full redeploy | absent | None |
| 12 | full redeploy | absent | None |

**The two sealed plans are exactly the two upgrade-lineage cohorts.** Since
cohort-9 every cohort has been a full redeploy, because that is condition (a) of
the standing devnet grant — *"full redeploy only … fresh identities, the old
cohort abandoned in place; no partial or incremental program deploys."*

So the standing grant's condition (a) and the checked-execution requirement are
in **direct structural conflict**, and have been since cohort-9. This is the real
reason no Direct fill has ever executed on devnet. Cohort-11's file names the
blocker as the red release candidate and lists the checked release as item 2 of
three; the candidate is green now, its five `checked.bin` exist and hash
correctly, and the next wall was standing behind it unmeasured.

**This lane stops here rather than improvising past it.** The two routes onward
are both outside what this lane may decide:

1. **Seal a redeploy cohort without an upgrade lineage** — teach `prepare` to
   accept a checked *deployment* set (seven fresh receipt-backed deploys) as it
   accepts a checked *upgrade* set. This is the honest fix: the artifact the fill
   needs is a statement about which bytes are live, and a fresh deploy can prove
   that at least as well as an upgrade can. It is a change to the plan schema and
   its authenticator, and it needs an owner.
2. **Run one cohort as an upgrade of cohort-12 rather than a redeploy.** Five
   `devnet-upgrade-v1` runs would produce the five receipts and the journal. It
   costs another full upload of the five large ELFs (the deployer holds 36.56 SOL
   against a ~42 SOL redeploy) and it is explicitly *not* what condition (a)
   authorizes.

Fabricating the journal was never an option: `authenticate_complete_deployment_set_for_prepare_live`
revalidates every row against the chain.

### What is nevertheless proved about the trade

- **The market accepts the rate.** Founded at exactly 50 bps, and the stager's
  own gate agrees `directTokenSetupAdmitsThisRate: true` — the first of five
  devnet SOL/USD markets that `direct_token_setup_v1` would admit.
- **The buyer is armed exactly.** `amount 201`, `delegated_amount 201`, the
  equality `refusing_buyer_collateral_clauses_v1` demands.
- **Both Direct token PDAs are derived and vacant**, so token setup has not half
  run: seller `9DZpNMChJLKqpv14jLWHiRx1m5x1MuBVyhJz8GCUficJ` (bump 254) and venue
  fee `BpVxBzA1JZSLzorzeRsPCb87SBsUkLBEbZ7z9xXuuedq` (bump 254), both derived
  from the five seeds `DirectTokenAccountSeedsV1` declares.
- **The census bindings the fill needs are already in place** — the buyer
  Position, and the two token PDAs the simulator adopts from the producer's
  public manifest.

**What is NOT proved, and must be said plainly: the fill-boundary conservation
laws landed at `49c8fa92` and `be67416e` still have never judged a real fill.**
They are exercised only by the faked fill in `tools/gauntlet/journey/src/ledger.rs`.
That remains the first real test, and it is still queued.

## 9. Cost, against the budget stated before spending

Stated in advance: at most 2 SOL per step beyond the deploy itself, and stop at
any step that would exceed it without an executing result.

| stage | deployer | campaign payer |
| --- | ---: | ---: |
| before anything | 38.738044775 | — |
| after closing cohort-11 | **80.630114478** | — |
| after the seven-program redeploy | 38.601927539 | — |
| after funding the payer and the ladder | 36.564742699 | 2.000000000 |
| after the founding | 36.564742699 | 1.663405281 |
| after funding two participants | 36.564742699 | 1.563395281 |
| **after the admissions and collateral leg** | **36.564742699** | **1.561349712** |

| step | cost | against the 2 SOL bound |
| --- | ---: | --- |
| redeploy (the deploy itself, exempt) | 42.028186939 | — |
| ladder: 2,475,000 lamports fee + 34,704,840 record rent | 0.037179840 | within |
| campaign payer capitalization | 2.000000000 | at the bound, by design |
| founding | 0.336594719 | within |
| two participants | 0.100000000 | within |
| admissions + collateral (fees + participant PDA rent) | 0.013799767 | within |

**The deployer has not moved since the ladder.** Every population lamport came
from the campaign payer, never the deployer.

## 10. What remains, and who owns it

1. **The checked execution release for a redeploy cohort** — section 8. Until an
   owner takes one of the two routes, no full-redeploy cohort can take a Direct
   fill, and the fill-boundary laws stay untested.
2. **Resolving this market.** Three things stand between `EQnYCUMkz…` and a
   terminal payout, and all three exist:
   - **the relay** — `devnet-sponsored-push-v1`, which reads the sponsored
     PriceUpdateV2 `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE`, derives the
     immutable candidate and canonical head from the authenticated release, and
     binds the executing Resolution deployment through the active release set;
   - **the observable** — SOL/USD in USD cents over denominator 100, against cuts
     $98.00 and $102.00, resolving into one of four outcomes at the window this
     founding stamped;
   - **the resolution** — `flagship-resolution-v1`, devnet-capable in three
     phases: `--produce-input` (key-free, needs `--pyth-facts` from the relay),
     `--provision-tables` (three exact typed tables, one journaled create,
     ordered extension or freeze per invocation), then the executor
     `--through submit|execute|reclaim|complete`.
   - **the retirement path is the gap.** Terminal settlement has
     `devnet-terminal-sequence-v1` and `devnet-wallet-terminal-payout-v1`, but
     **`AggregateRetirement` is owned-loopback only** — `COMMAND_V1` is
     `local-private-validator-aggregate-retirement-v1` and there is no devnet
     arm. So this market can be resolved and paid out on devnet, and cannot yet
     be *retired* there, which is the only route to an empty aggregate and the
     collateral. The founder key is held (`5YzYwcwio7wm88CXdh6ir8w3xstfwRcwU7ZFRtJ3aLRq`),
     so unlike the three markets decision 0015 section 8 names, this one is not
     stranded by a lost identity — only by a missing driver.

   That is a well-shaped lane: relay, resolve, pay out, and name what retirement
   needs.
3. **The web lane's redemption test** — handed off with the market and both
   participants.

Devnet evidence. Not mainnet evidence.

## Addendum, 2026-09-02: the sealing route was tried, and there are three more walls behind the first

The coordinator's reading of the standing grant is right — condition (a) forbids a
*partial* deploy, not an upgrade of the whole set, and cohorts 7–8 sealed exactly
that way. So the route was attempted. **Every step below is a measurement, taken
key-free with `--preflight`; nothing was signed, sent, or changed. The deployer
sits at 36.564742699 SOL, unmoved, and every live program is byte-for-byte what
section 2 recorded.**

### First, two facts that reshape the goal

**Any upgrade mints a new release set, even to identical bytes.** `release_facts`
binds `(elf_sha256, deployment_slot, upgrade_authority)` (`plan.rs:1080-1117`), so
the deployment slot alone moves `release_set_id`.

**And the founded Market pins its release set immutably.** Read off chain at
`STATE_SELECTED_RELEASE_SET_OFFSET = 208`:

```
EQnYCUMkzSG2pHnzkdEC7vxqYgabPgBserq9oS4VmGs1
  selected_release_set = 797e83ac0522787898b24a963182b846f61f96c6968e4bfdbfbb8dc5bcf7e9a1
  plan.release_set_id  = 797e83ac0522787898b24a963182b846f61f96c6968e4bfdbfbb8dc5bcf7e9a1   MATCH
```

So upgrading to seal would strand the market this file just founded and handed to
the web lane: the fill would need a **second founding** under the new set. The
activation account is per-release-set (`find_program_address([ACTIVATION_PDA_DOMAIN_V1,
release_set_id], registry)`), so the old activation survives — the market stays
Open and resolvable. It simply becomes unfillable, exactly as cohort-11's did, for
a different reason.

### Wall B: an upgrade may not re-upgrade to bytes already live

Measured, as a positive control, by placing the gate's own trading ELF and
re-running the custody preflight:

```
Error("current live payload already equals candidate without a bound receipt")
```

`upgrade.rs:4543`. So "upgrade the five to the same bytes to mint receipts
cheaply" is refused by design. A receipt requires a payload that actually
changes, which requires a **new checked gate at a new commit**.

### Wall C: the checked gate can never authorize the trading bytes a cohort runs

This is the deepest one, and it has nothing to do with redeploy-versus-upgrade.

`checked-release-candidate.sh` builds `dclutch-trading-sbf` **with
`--features hot-cu-profile`**, a diagnostic profile. `build-elfs.sh` deliberately
does not — "a diagnostic profile and not what a cohort should run". So the gate's
admitted trading artifact and the cohort's deployable trading artifact are
different by construction:

| | trading ELF SHA-256 |
| --- | --- |
| gate's `provenance/trading.json` `shipped_elf` | `5354b4cd78aa0bfd2e6ab838c2b52c3f6baee663be303108755a1ce3cb209336` |
| deployed, and what cohort-12 runs | `b0cff55ab0ef162d7e427b8cb894f1468b1804d997ab35c52710df3268a8e3ed` |

Six of the seven role ELFs are **identical** between candidate and cohort;
trading alone differs, and only because of that feature flag.

**The gate admits the release as a SET of twelve links, so this blocks every
role, not only trading.** The custody preflight — custody's own bytes being
identical to the candidate's — refuses:

```
Error("trading ELF bytes or SHA-256 changed after checked-release admission")
```

### Wall D: `AlreadyCurrent` is consumed by the tree and written by nothing in it

There is an escape hatch that would solve all of this without a single upgrade.
The deployment-set journal admits three dispositions, and the third is
`already-current` — "admitted on **byte equality alone**" (`upgrade.rs:2091`),
carrying a baseline and a dump, `receipt.sha256: null`, no transaction at all.
Cohort-8's journal uses it for its resolution role.

**Cohort-12's custody, resolution, claims and core all qualify today** — their
live payload already equals the checked candidate. If they could be journaled
`already-current`, no upgrade would run, no deployment slot would move,
`release_set_id` would stay `797e83ac…`, and **the market already founded would
become fillable in place.**

The tree validates such a row, audits it live on every invocation, and builds the
plan pin from it (`upgrade.rs:3457-3520`). What it does not have is anything that
**writes** one. Cohort-8 produced its single `already-current` row with an
out-of-tree patched binary — `~/jobs/dclutch-cohort8-20260831/bin/bootstrap-alreadycurrent`,
built 2026-08-31 — driven by a hand-written `refresh-journal.py` that maintains
the journal's digests by hand. That capability never landed in the tree; only its
reader did.

And even it would not save trading, whose live bytes can never equal a candidate
built with a different feature set.

### So the fix is two release-tool changes, and they are smaller than a cohort

Named for an owner, because this is release tooling and not a cohort lane's work:

1. **The checked candidate must ship the ORDINARY trading link.** The
   `hot-cu-profile` build is a *measurement*, and measuring it is right; admitting
   it as the shipped artifact is what makes the gate unable to authorize any
   cohort's real bytes. Until this changes, **no cohort — redeployed or upgraded —
   can be sealed against the bytes it actually runs.**
2. **A writer for the `already-current` disposition.** The reader, the auditor and
   the plan projection all exist. With a writer, a genesis cohort whose live
   payload already equals the checked candidate seals with **no upgrade, no new
   release set, and no second founding** — which is strictly better than the
   upgrade route and leaves the market in this file fillable.

With both, cohort-12 seals in place and the trade in section 8 runs against
`EQnYCUMkz…` with nothing stranded. The earlier suggestion — teach `prepare` to
accept a genesis deployment's receipts — is subsumed by (2), which the tree is
already most of the way to.

**The fee-bearing trade, its `DCLTDFS1` settlement, and `ledger-census` across the
fill therefore remain undone, and the fill-boundary laws `49c8fa92` / `be67416e`
remain unjudged by a real fill.** Everything else the trade needs is in place and
recorded above: the 50-bps market, the exactly-delegated 201 atoms, both Direct
token PDAs derived and vacant, and the census bindings.

### The shape wall C shares with two other findings tonight

Worth naming because it is the third instance in one night, in three subsystems,
found by two lanes independently. The Redemption/web lane reports two of its own
(its measurements, not re-verified here): a liveness test that asked the Program
stub rather than the account holding the code, and a header check that had never
followed the Registry's bump offset. Wall C is the same shape one level up —
**a gate that certifies something adjacent to the thing it claims to certify.**
The checked release gate does measure a real artifact, built from the real
sources, with a real frame report; it simply measures the `hot-cu-profile`
build, and no cohort runs that.

AGENTS.md already carries the probe-level version — *"a probe measures what it
touches, not what you meant"* — learned from a heap probe that measured
`entrypoint!`'s hardcoded `HEAP_LENGTH` instead of the granted frame. This is
that lesson at the **gate** level, and it is more expensive there, because a
probe misleads one investigation while a gate silently authorizes nothing at all
for as long as it stands. The cheap defence is the same one that caught it here:
name the artifact the gate certifies and the artifact production runs, put the
two digests side by side, and require them to be equal rather than merely both
present.

## Addendum, 2026-09-02: the market is ACTIVATED, and the deadline was not attached to what it looked attached to

The UX reading lane found, and was right about, a vacancy with a clock on it:
`EQnYCUMkz…` had Direct trading **founded but not activated**, no activation root
at `88jJTMmU…`, and capability entry 0 had to be activated by **slot 492,091,890**
or the market could never trade — the fate of the four earlier devnet markets.

**It is activated.** Read back off chain:

| | |
| --- | --- |
| activation signature | `2hr4RJJTS12XmszvLUFysFq9k27E5DKT67sSa1wdv44hvdnTMzCmVbYW6oiG5YSkCiHxwMeUiCAvm1rFZguMbcQJ` |
| activation root `88jJTMmUGr4tB92SwAVpNnQ5CYnWYsg19cu3ULgrZmd4` | **exists**, 256 B, `DCLTCRT1`, owner Trading `Ahzug4zY…`, phase Open |
| entry / generation | 0 / 2 |
| deadline slot | 492,091,890 |
| activated near slot | 491,906,637 — about **185,000 slots** to spare |
| instruction | 35 accounts, 528 bytes, fee 75,000 lamports from the campaign payer |

**And the premise attached to the deadline was wrong, which is the part worth
writing down.** The reading held that the already-current seal and the checked
execution release were what activation needed. They are not.
`devnet-direct-capability-activation-v1` takes `--plan`, `--market-input`,
`--campaign-report` and a payer — **all of which this cohort has had since the
founding finished.** Had the two been coupled, a nine-hour clock would have been
run against two release-tool changes that, as measured below, cannot be landed
for this cohort at all. Activation and fill are separate gates, and only the fill
needs the checked execution release.

The activation also published a **sixth** frozen routing table, which extends the
`dc07c73a` proof past the founding path: `create DIRECT-ACT frozen routing
address lookup table` (`7GrhHyWV…`), two extends, then `freeze DIRECT-ACT routing
table after its one complete extension plan` (`bKa52k8V…`).

## Addendum: the two release-tool changes, and why this cohort still cannot be sealed

Both fixes named in the previous addendum are landed (`28ff0823`).

**Wall C is closed at its cause.** `checked-release-candidate.sh` no longer admits
the profiled build. `sbf_feature_suffix` splits into a shipped suffix (empty for
every package) and a measurement suffix (Trading alone); the frame gate keeps the
profile, a new explicit profiled build keeps existing so its digest can be stated
beside the shipped one, and `metadata.txt`'s `build_command` describes the shipped
artifact. Two new refusals: the profiled measurement may not be byte-identical to
the shipped link — if it were, the feature has stopped doing anything and the
profile is a lie rather than a measurement — and Trading's provenance must carry
the feature in its **frame** command and not in its **plain** one. That second
gate was proved red against cohort-12's own real pre-fix descriptor before it was
trusted:

```
refusing: the shipped Trading command carries --features hot-cu-profile, so the
gate would admit a diagnostic build no cohort runs:
  … cargo build-sbf --manifest-path programs/dclutch-trading-sbf/Cargo.toml --features hot-cu-profile -- --locked
```

**Wall D is closed.** `devnet-deployment-set-already-current-v1` writes the
disposition the reader has always validated: key-free, read-only against the
cluster, admitting on byte equality against a fresh finalized observation and
refusing by name — with both digests — any role that differs. It also refuses a
role that already has a receipt, and one with a receipt *file* on disk, because
the journal loader rejects a receipt that exists while no digest is pinned and
this command will not delete evidence to get past it. Three fake-chain tests: the
admit path asserts the row it writes **reloads through the real loader**, the
refusal path asserts the journal is byte-identical afterwards, and preflight
writes nothing.

**And cohort-12 still cannot be sealed, measured twice.**

1. `checked-release-candidate.sh` refuses unless it *is* the script at
   `--commit`: `refusing: invoke the checked-release runner from the exact
   --commit source revision`. Correct, and it means **the fix cannot be applied
   retroactively to `e39efbb0`** — the commit whose bytes are live.
2. A gate at any commit carrying the fix therefore describes that commit's bytes.
   Built and compared rather than inferred: HEAD's **ordinary** Trading link is
   `1d92debe9a24d11cee73b3a8da3d6b01b935ada3c4b1df6429f0bbb4674e7319` at 2,308,328
   bytes, against the deployed `b0cff55ab0ef162d7e427b8cb894f1468b1804d997ab35c52710df3268a8e3ed`
   at 2,308,320. Twenty-one commits touched `programs/` and `crates/` since
   `e39efbb0`.

Trading can therefore never be `already-current` for this cohort, so the seal
fails, so the checked execution release cannot be built, so **the fee-bearing
trade and `ledger-census` across the fill remain undone on cohort-12** and
`49c8fa92` / `be67416e` remain unjudged by a real fill.

What the two fixes buy is the next cohort: one deployed from a commit that
contains them seals in place with no upgrade, no new release set and no second
founding — and can trade.

### The repaired candidate is green, and the gate now admits the bytes a cohort runs

Run from a worktree at `28ff0823` — the candidate refuses to run as any commit
but its own, so proving the fix means running the fixed script as itself:

```
source_revision=28ff0823b53c8c17b202cbe2432ca14c9fb2888c
sbf_build_freshness=passed            sbf_build_freshness_links=12
sbf_build_diagnostics_total=0         trading=0   (all twelve links zero)
checked Upgrade gate  sha256=91379329972a6881d5f9d7698046776f7aa6ce060369d4fd665ac4496ffa0af5
CANDIDATE_EXIT=0
```

The gate's Trading link is now the **ordinary** artifact, and the candidate states
both digests rather than leaving a reader to guess which one was admitted:

```
trading_elf_sha256=1d92debe9a24d11cee73b3a8da3d6b01b935ada3c4b1df6429f0bbb4674e7319
trading_elf_bytes=2308328
trading_profiled_elf_sha256=d2eb0013e3e0e1345d76b10d9197f11600815fd1355f15e805e9069ef4745d77
trading_profiled_elf_bytes=2314456
trading_admitted_artifact=shipped
```

`provenance/trading.json` carries the inverse of what it carried before the fix:
`shipped_elf` is `1d92debe…`, the feature is **absent** from `plain_build.invocation`
and **present** in `frame_measurement.invocation`. The profiled measurement is
6,128 bytes larger and is never admitted.

The admitted digest `1d92debe…` was independently reproduced by an ordinary
`cargo build-sbf` of `dclutch-trading-sbf` in a separate detached worktree at the
same commit — two builds, two roots, one digest. **That is the property the whole
repair exists for: the gate now certifies an artifact a cohort would actually
deploy.**

Devnet evidence. Not mainnet evidence.
