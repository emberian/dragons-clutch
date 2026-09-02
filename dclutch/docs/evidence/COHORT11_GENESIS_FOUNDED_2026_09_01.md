# Cohort-11: a genesis cohort, founded on the day it deployed — 2026-09-01

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized.

`c60b25e8` made a genesis cohort foundable *in the program*: one instruction
commits the sealed V1 and the genesis V2 every consumer reads, over a frame that
widened 14 → 15 accounts. This file records the host half — one schema
migration, end to end — and the cohort-10 redeploy that carries both to chain.

Source commits, tree root `/Users/ember/dev/dclutch`:

- `c5efdc4d` — the plan schema, the Found dispatch, the manifest, the stage
  arithmetic.
- `062b62fa` — the succession stage stops claiming a ceremony ran, and every
  live-instruction account moves to the V2 domain.

**Cohort-10 is deployed from `062b62fa`**, so the driver and the programs it
deploys are one commit. That matters here rather than being a formality:
`c5efdc4d..062b62fa` carries other lanes' changes to `hot_v3.rs`,
`admitted_composition_v3.rs` and `dealer/v3_trade_profile.rs`, all of them
inside a role this cohort deploys, so a driver at one commit against ELFs from
the other is a mismatch waiting for the Direct path to find.

## The migration, and what each piece had to do

### 1. The plan pins both profiles

`SuccessorPlan` gains `genesis_infrastructure_profile` and the schema moves to
`dclutch-local-successor-infrastructure-plan-v3`.

The V2 body is **derived from the V1's own two bindings**, never supplied:
`ProtocolInfrastructureProfileV2::genesis(registry, rent)` is the whole body and
the two sentinels are the only difference. So a plan cannot pin two profiles
that disagree, and `authenticate_infrastructure_profile_projection` rebuilds it
and refuses a substitution *before* a live transaction reaches the fifteenth
account.

A v2 plan refuses by name and says to re-run `prepare` — in the Rust driver
(`model.rs::authenticate_successor_plan_schema_v3`), in the jq validator, in
`reconcile.py`, and in the SDK document importer a browser pastes plans into.

Everything that names the profile as a **live instruction account** now names
the V2 domain, because Core authenticates the V2 and nothing else since
`2951b226`: the founding path reaches it through Found's own selection,
`terminal_sequence.rs` and `flagship_resolution.rs` read it from the plan. The
sealed V1 stays where it is used as **lineage or observation evidence** — the
carry-forward snapshot, the succession ceremony's predecessor, the `run`
supervisor's pre-init vacancy check. A market founded on a born-at-V2 cohort
that could not be retired would have been the same defect one rung later.

### 2. Found stops offering a V1 it cannot use

`checked_found_infrastructure_selection_v1` loses its `Predecessor` arm and
gains `Genesis`.

After `2951b226` the `Predecessor` arm could not produce a foundable projection
at all. It was not dead code: it was the arm a genesis cohort *always* took, and
it always failed sixty transactions deep with a coarse `AccountAuthority`.
Measured on cohort-9 at the cost of two stranded collateral mints. AGENTS.md
forbids keeping a superseded authority path beside its successor, so it goes in
the same change as the arm that replaces it.

A plan with no succession now **requires** its genesis V2 on chain. Absence is
not a fallback; it is the initialize stage not having run, and the refusal says
so. The `Genesis` arm authenticates the observed account against the plan's own
pin — exact bytes, `born_at_v2()`, the V2 PDA under this plan's Core — so a
*succeeded* profile can never quietly found without its ceremony.

### 3. The stage machine learned to count two profiles

- `initialize_state` is Complete only when **both** are committed. A cohort
  holding the V1 alone is exactly the pre-`c60b25e8` shape that nothing can
  found; reporting it Complete would report an unfoundable cohort as ready.
- `wallet_arithmetic` adds the genesis V2's rent. An estimate that under-states
  the requirement is the one that strands a run.
- `succession_state` for a plan with no ceremony **reads the sentinels** instead
  of returning `Complete` without reading anything. That old silence is why the
  cohort-9 evidence had to carry a paragraph saying "`succession complete` is
  not a claim the ceremony ran". Now `Complete` means the chain says there is
  nothing to execute; `Absent` means initialize has not run; and a V2 that is
  **not** born at V2 is a succession this plan does not describe — a conflict a
  resumed campaign must never write over, which the old unconditional `Complete`
  reported as done.
- The executor **skips** the succession stage for a born-at-V2 plan before
  looking at any observed state, and says "nothing to execute" rather than
  "already complete". Without that skip the honest new `Absent` would have sent
  a fresh cohort into `execute_succession_stage_v1`, which has no pin to run.

### 4. The genesis manifest carries both bodies

Schema 3 pinned only the 144-byte V1, which since `c60b25e8` describes half a
chain act. Schema 4 embeds both bodies and both PDAs, the same mechanism
`61817d7a` used to add schema 3 — a reader built for one refuses the other by
name instead of misreading it. The V2 half is derived inside the builder, so the
two halves cannot disagree. Schema 3's width is kept **only** so a stale
manifest refuses by name instead of dying on `InvalidLength` in the succession
decoder. `derive-genesis-infrastructure-profile` now writes `profile.bin` and
`profile.v2.bin`, so the candidate pack carries the bytes the chain will hold.

## Controls

### Red proofs — one conjunct weakened, exactly one test red

| weakened | test that went red |
| --- | --- |
| the vacant-V2 refusal (accept a vacant V2 as `Genesis`) | `found_infrastructure_selection_is_genesis_or_planned_successor_and_never_v1` |
| the manifest's derived-V2 equality | `a_genesis_manifest_carries_both_profiles_and_neither_can_be_substituted` |
| the plan projection's derived-V2 equality | `a_prepared_plan_pins_the_genesis_v2_beside_its_sealed_v1` |

The third needed a **sharper hostile than the first draft had**. A flipped byte
in the trailing predecessor id was already caught by the sentinel check, so
weakening the equality changed nothing. The hostile that convicts it is a
well-formed genesis V2 — both sentinels intact, `born_at_v2()` true — built for
a *different* Registry binding. Only the derived-from-the-V1 equality can refuse
that, and it does.

### The genesis release candidate — the one control not yet closed

`checked-release-candidate.sh --genesis-cohort` at `8ae2c9c9` built all thirteen
SBF links, passed the freshness gate, produced all thirteen frame reports and
provenance descriptors and the host tool, and then **died on the very stale
`Cargo.lock` this lane fixed in `b2ac8a79`** — because it archives the source of
the commit it is given, and `8ae2c9c9` predates that fix. Re-running it needs a
commit that carries the lock, which is a different commit from the one whose
bytes cohort-11 runs; the candidate is a source→artifact provenance artifact
with synthetic program ids, so that is legitimate, but it means this control is
**queued, not closed**.

Re-run at `659d6f26`, a commit that does carry the lock, it died again — and
this time not on anything in the tree. The registry SBF link stopped mid
`Compiling dclutch-registry-sbf` with no diagnostic, which is a kill, on a
machine sitting at load average 127 across 12 CPUs with the data volume 98%
full. Nine lanes were building concurrently. Recorded as an environment wall
rather than retried a third time, because a third hour-long thirteen-link build
on that machine is a cost the other lanes pay.

(This lane's own share of that pressure has been returned: three dead candidate
work directories and its abandoned cohort-10 build worktrees removed, ~12 GB,
keeping only the two worktrees at `8ae2c9c9` that are cohort-11's named
provenance.)

What it would add is end-to-end confirmation in the real pipeline. The substance
it checks is already proven by unit test: schema 4 carries both profiles and
neither can be substituted (`a_genesis_manifest_carries_both_profiles_and_neither_can_be_substituted`,
red-proved), and `derive-genesis-infrastructure-profile` now writes both
`profile.bin` and `profile.v2.bin`.

### Suites

- 660 successor tests green, 22 of them `plan::tests`, and 35 release-tool tests.
- One successor test is red: `general_capability_activation::tests::
  the_addresses_this_driver_borrows_are_the_ones_the_founding_finalizes`. It is
  **red at pristine HEAD** in a clean worktree with none of this lane's files —
  proved by reverting them and re-running that name alone. It belongs to another
  lane.
- `apps/dclutch-web` `lib/deployments.test.ts` 12/12, including a new case that
  a retired v2 plan is named as retired rather than called a foreign document.

### `prepare` against the live cohort-9 observation

The control that the observation path is unchanged, run **before** cohort-9 was
closed and spending nothing. All seven ProgramData accounts were dumped off
devnet, the seven ELFs recovered with `solana program dump`, and the v3
`prepare` run against them.

Every ELF digest and byte count matched the cohort-9 evidence file exactly
(registry 234,536 / `ed70f8bd…` through trading 2,285,728 / `50d57606…`), and
the resulting v3 plan reproduced cohort-9's plan **field for field** on
everything it already carried:

- all seven program ids, programdata ids and artifact-release ids;
- all seven deployment slots — *decoded out of the observed account image, never
  supplied* — 491482785 (registry) through 491484013 (core);
- the retained authority `4zrxtw5c…` on all seven;
- the V1 profile address **and body**, the release-set id, and the activation
  PDA `EGY1DPNCmbTTFX4uAbpJuuW9YBmhXpSzXFF6MtxyHVxU`.

The only difference is the added pin, and it lands where it must: the V2 PDA
`G5M2jgBQXypkUgoMc2jswH8WZx61ykDcSeQm3Bfw5tTF`, 224 bytes — **byte-for-byte the
address the previous lane read off chain as a System-owned vacancy.** The
derivation is therefore confirmed against a live observation rather than
asserted.

## Cohort-10: the redeploy

Every ELF is built **twice, in two independent detached worktrees at
`062b62fa`** (`/private/tmp/dclutch-c10-build2` and `…-build3`), on the ordinary
release invocation:

```
CARGO_TARGET_DIR=<per-role> cargo build-sbf \
    --manifest-path programs/<package>/Cargo.toml -- --locked
```

Deliberately **not** the checked-release candidate's trading link, which builds
`dclutch-trading-sbf` with `--features hot-cu-profile`. That is a diagnostic
profile and not what a cohort should run; the candidate keeps it so the command
it records is the command that ran.

The runner deploys one role at a time and **verifies each before the next
starts**, by dumping the on-chain image back and comparing it to the built ELF —
prefix equal and the Loader's allocation tail all zero. Verify-after-each rather
than verify-at-end is the rule for any sequence whose steps spend money:
cohort-9's first attempt died on role one, with nothing spent, because
`--upgrade-authority` takes a **signer** and had been handed a pubkey. A runner
that deployed all seven and only then checked would have attempted the whole
6.6 MB against an unsatisfiable signature and burned the reclaim.

All seven are deployed **mutable** under `4zrxtw5c…`. That is the sanctioned
iteration substrate, and since `6155219a` it is also a wall rather than a
landmine: `prepare` refuses a plan that would commit an infrastructure profile
with both Registry and RentCredit already immutable, because such a cohort can
never run the succession ceremony and is therefore permanently unfoundable —
silently, with no diagnostic until a founding fails sixty transactions in.

### Cohort-10, deployed and then superseded — and what it proved

Cohort-10 went live from `062b62fa`, all seven roles, each verified by dumping
the on-chain image back:

| role | program id | bytes | ELF SHA-256 | deploy signature |
| --- | --- | ---: | --- | --- |
| registry | `DB8xTh5XCLFyaXCr86rbpN4Wq47WCvSErTZ2z7suJ7gR` | 234,536 | `ed70f8bda12b77d663126218ad05f36dd77c5bf3100642879cef1441a845afe7` | `4KTa2PFn2ZERetLuYNTbLwF1pL298amWo52csRUdXWgbsMNkkwcG2hB6J6DUJot7J3AzFeVER7Xy5TmmmVLPmSLV` |
| rent | `9Mh4mFUnYDo68vCq1DFW3GsmZBhQ75mY4mM5RxD9Kkap` | 142,320 | `d46e5f0a64fd7d5e296118c2e7a62a3b67aed2c2ac4420e85069fb8dca632837` | `5aGDFsic5DP5VuZWtNwR7sjX6oCFdtR41AuEiViD6qmn2hfbpu8fWMdAw7MACeqwMyq1L5mbLdhZ9c9UCsZpprbk` |
| custody | `2vEJxiBq66x62sV11ZHi4c3kA83YWzQCCSDarB5ECH1P` | 571,432 | `2823c82351638566e295d7f7acc2e559ab61b3ea43750759e84f73bc0f80d567` | `5NFf2Bub2M9KFqC6JuPDpgg7f96ZkiEDg8XSnRYws3nqHGfL72gBx4B7bto5Lmf2WeC11TuEymoNwJAFAPU586mZ` |
| resolution | `C7q5F3Sd23gm3LREu4RsTgisgyEt9Mk4tC6BiYssz3Aa` | 818,368 | `5caf0be15dcde186df2be8eeaabc490efd817d4add528d51760949a404aa6217` | `kH6zhw5oDUhb7jCDursddAidtV95zoxhNkA7sNv4TsGk3aU7NA21J4c9WF9jeCMKzccUxfSAD3hV81dNdtehZtA` |
| claims | `CxcRJ2cFQMFJYjxSPfuVC2uVoXFRNsmtgoEwRokFC3c1` | 1,366,416 | `67d9d97fdac0abb7e67f8adf1e3f92e3d022b8d530442dd9351f69bda75f463f` | `47sv1uy63Jn3Vekx8Uw65upNaqHWjXygrKLMfLhPijdKMa4CKzNrHha41amM5wAAE3Q5geHjpNFXy4ReVRAJgs23` |
| trading | `A9pA1N16PmpUHTmDxn1hk8oECQ6M1aUd5oFHg8kff84Z` | 2,289,648 | `2c11f352f94ce2f97a0a52749045631bc41be54f85fed9033d966f9ba800bba9` | `59ebhjoFjvERSxU9WK8ArzX4J4ipu2WzMBPia3yy93e9HXRyBB6cgC46bvv8r3E44EFDTGde23Eeov6HmEfbQWFC` |
| core | `C6JbUfwk3V9Cve5nTvKRWvvfELzSFU26gzTXNc1YwXMG` | 1,193,096 | `b31579a546b4caf1d394d80f552f1e504cfc28915bc4927395ab24643393f33b` | `5VM32js9Vg2pwNRA51wk5HfbUYViepEU5jQJ62cL6Nb5v6jL3UrEYS5KDifP5a45CbP1sf9KG598M6KmmAjTihq7` |

All seven byte-identical across the two independent worktrees, all seven
`IDENTICAL` on read-back, all seven mutable under `4zrxtw5c…`. Cohort-9 was
closed first and returned **41.840595079 SOL** (42.945709919 → 84.786304998);
the deploy spent **41.945742275** (→ 42.840562723).

Its ladder then reached, in one run:

```
substrate    complete    (the seven deployed roles match the plan's pins,
                          verified against chain rather than the deploy tool)
publication  complete    9 of 9 record bodies finalized
initialize   REFUSED     custom program error 0x3001, 3,388 CU
succession   absent      the V2 PDA is vacant -- the new detector reading the
                         sentinels, where the old code said "complete"
```

**`0x3001` is `CoreSbfError::AccountFrame`, and the refusal was right.**
`c60b25e8` widened the initialization frame 14 → 15 by inserting
`genesis_profile` at index 2, which moved `upgrade_authority` from index 3 to 4;
`require_distinct_except_payer_authority` went on exempting the literal pair
`(0, 3)`. So the one aliasing the frame exists to admit — a payer that is also
the upgrade authority, which is exactly how the campaign drives this stage — was
refused, and the pair it then named is one no caller can construct. Thirty-three
core-sbf tests stayed green through that commit because the exemption had no
test. It has one now (`8ae2c9c9`), proved red by putting the index back.

**Cohort-10 is therefore abandoned in place and superseded by cohort-11.** A
one-line core fix reaches chain only through a full redeploy; condition (a) of
the standing grant forbids a partial single-program deploy, and this is the
second time in two days that rule has been the thing that kept a cohort honest.
Stranded with it: nine finalized Registry record bodies and the 2 SOL that
capitalized its campaign payer.

## Cohort-11: the cohort that carries the repair

Deployed from `8ae2c9c9` (the frame fix, on top of everything cohort-10 carried),
built twice in `/private/tmp/dclutch-c11-build2` and `…-build3`, all seven
byte-identical and all seven `IDENTICAL` on read-back:

| role | program id | bytes | ELF SHA-256 | deploy signature |
| --- | --- | ---: | --- | --- |
| registry | `ADB72ar6ZSstXEg76Q1bPb5UY2EGmH6mrVfwr8K2fzom` | 234,536 | `ed70f8bda12b77d663126218ad05f36dd77c5bf3100642879cef1441a845afe7` | `34on8aiehZ1qvN1ekWbZ3dH7xS35QCacUSYjHgrJXAqH9zV9gNwPMCwDA17eRX5gWP8fn8T5HCF9mYdPg9UvH6vT` |
| rent | `HA31aDmTnLFjBYQoCXeyRBsfdddxne1apyfzfq4tSp8e` | 142,320 | `d46e5f0a64fd7d5e296118c2e7a62a3b67aed2c2ac4420e85069fb8dca632837` | `2JZ4bfEJcJj39Rz2krDLbYr297eHVLBo1ormePm4dCW3yCqiDQJyP3vc5B1VUKekNr4fTUKnGi8aAxJjpmEtDcsN` |
| custody | `Cdh8Vv7DRyk7rhLcee574potYfaiVEsYR5HUPCrNPzCB` | 571,432 | `2823c82351638566e295d7f7acc2e559ab61b3ea43750759e84f73bc0f80d567` | `5K8z2pLujhw1eFn6him5nCk9B6pQc8vrxHCMdjRnPcJr5BwqqQAZGv5xM8Ea89dWyZnWTi7kWhgZVYa69AG2N2TF` |
| resolution | `3WqTxq6uKMK2d9f6uRujh8hCZvVB78KjGo9AYxvPQNVM` | 818,368 | `5caf0be15dcde186df2be8eeaabc490efd817d4add528d51760949a404aa6217` | `2hK8noeVJhNLqpiHyMHWwwsqy8BRYRnMXBr3QXRUEQoAmJNdA1BYR7Wygi7X8wKBFf5R7uxXPYiZWjF2BXqYhWKu` |
| claims | `HQYqqdzn5s6tEM6ywgeCr7Bd56tEuhpoop3ruvHRfAq6` | 1,366,416 | `67d9d97fdac0abb7e67f8adf1e3f92e3d022b8d530442dd9351f69bda75f463f` | `4X8HSYKpzbHoTDbQBc24nwreRqnpYxke2Y3KkaEXmXuNwUSyjZ72WYamKCC28W3QAEtdZ9B3inMh1xWoVj8zb9ct` |
| trading | `4fhQyBPgvaZw6jEWwT3U64tHfgTNRPuWuH5MjPLrxjzk` | 2,287,512 | `c2ddacc96a37d04bcdf3f3e5158f11b0fee28bd9ad1e05d6f38ddf9b8b1b67b1` | `4nFMAyDk4w7UwtMYVGWwLNvZzFvHheP1vwWEPQKdAsuj5qecttHcKZ3qgn1Xqd9MjD7CYejTM6orSQrPMbSgBVP1` |
| core | `FinXxc9drpmCYA7Cy4aGWSa1jYY87K6pNPfY9qFWzJCF` | 1,193,096 | `77c14336687d0a1622c3be66a568ee35c1607fe32a571dae160dd85cb8a70cc5` | `2e7bNG1YvDeSgHbsmwL483vnRmtsFqtXYWTUct2w1UEw4LdLCEKpJHMrJRhpMftfEnQAeoN6S7sCY1TAFteM3m7L` |

Cohort-10 was closed first and returned 39.881781471 SOL (42.840562723 →
82.722344194); the deploy spent 41.932204987 (→ 40.790139207). Everything but
core and trading is byte-identical to cohort-10's, which is the frame fix and
another lane's `entrypoint_adapter` work and nothing else.

### The ladder, re-observed from chain after it ran

```
substrate    complete
publication  complete    9 of 9
initialize   complete    BOTH profiles, one instruction
succession   complete    nothing to execute; the sentinels say so
activation   complete    5 of 5
```

**The genesis V2 is on chain and it is born at V2.** At
`DpHNSCLurBaJaNrQAH4T2S9D25UdWAnwWYJFiYF5zBoR`: 224 bytes, owner
`FinXxc9dr…` (Core), 2,229,216 lamports — devnet's exact rent floor for 224
bytes — byte-identical to the plan's pin, and carrying
`sha256("dclutch/genesis/protocol-infrastructure-predecessor-registry-v2")` at
offset 144 and the `…-rent-v2` sentinel at offset 176. The sealed V1 stands
beside it at `6D3pdh1jZQgHVsQc2Rh4mzgQ2VHpTGRwzLJaWrTvVqs5`, 144 bytes,
1,722,576 lamports. That is `c60b25e8`'s whole claim, on a real chain.

And the driver said the right thing while doing nothing:

```
campaign stage succession: nothing to execute -- this cohort is born at V2
                           and carries no ceremony; observed complete
```

### Two more walls the run found, both host-side and both real

- **The successor workspace's `Cargo.lock` was stale.** A lane added
  `dclutch-sbf-bump-heap` to `dclutch-trading-sbf` without it; the root
  workspace's lock carries it, so every SBF build stayed green and the failure
  waited for something to run `--locked` in the successor workspace — which the
  plan step does, *after* 41.9 SOL had been spent. Fixed in `b2ac8a79`.
- **`stage-devnet-sponsored-market-open.sh` could not state a founding band.**
  `26179076` made the gated product entrance measure a partition against a
  declared belief, and the compiler grew five `--band-*` flags; the stager never
  learned them, so from that commit it could not stage a Pyth market at all.
  Fixed in `96c0a58b`, all-five-or-none checked before a socket opens.

### The founding: a genesis cohort founded a market on the day it deployed

`campaign --founding-only`, 188 transactions, `completed: true`. The gate it had
to pass first is the one this migration strengthened: `--founding-only` requires
all five earlier stages Complete, and Succession now means *the genesis V2 is on
chain, born at V2, and matches the plan's pin* rather than *this plan has no
ceremony*.

| what | address |
| --- | --- |
| Market (`DCLTCOR3`, 368 B, owner `FinXxc9dr…`) | `ARuPAuyJbJoLdMWGDzSqvcV9py25EkmMj8ABnfKP56s` |
| Claims liability aggregate | `5wdhigoUUNDaQFjqBmVUTmyh5ihqjxUNV6sdaNt6izxE` |
| founder Position | `AB6HppHWFsMMobJxin6GgVh9qD5xJcNchUm4qnWndq7` |
| Claims admission | `3HyBinfqDZ9WBEdyUEfB6Mz3TSfSJBemT2dJHoVyRNRj` |
| collateral Mint (Token-2022) | `H5zmg8nVY9JPccjYeB1t4d7AuLPJhVpjDnZMD718gGFk` |
| collateral wallet | `EZstrdjqrapd99GDZaQ4s7FnXM9L3qWWEBgqVijNAuWK` |
| capability manifest record | `DHWJSTRQXCSQv5PyBXo5Gg9pgJf3oBCWsQdqCsyYpqem` |
| founder identity — **key held** | `BmDp2LRfAUxPw6qhQr9ceGMoitMtkQf3H547iTS631rv` |

The driver's own last words: *"executed DCLTGMF3: the Market is OPEN, with the
Claims liability aggregate, the founder Position, the admission record, and a
Hoard holding the exact collateral"*, followed by the post-Open V7 funding
readiness suffix in order (`core-funding-create-v1`,
`resolution-funding-activate-v1`, `core-funding-accept-v1`).

**The founder key is held.** The wrapper derives the founder identity from a
keypair file rather than accepting a bare pubkey, because burning the founder's
complete set is the only route to retirement and to the collateral. Three live
devnet markets already share a founder nobody holds and can never be retired
(decision 0015 §8). This is not a fourth: `keys/founder.json` under a mode-700
job directory derives exactly `BmDp2LRf…`.

The market is the compiled SOL/USD one — gated entrance, centred cuts
`[14800, 15200]` at denominator 100, coefficients `[1, 0, 1, 0]`, and the
declared band `{anchor 15000, volatility 200 bps, window 10000 slots, plausible
half-widths 3, max cell share 9000 bps}`. Not the stager's `12000,18000`
default, which at a ~$150 spot asks about $120 and $180 and puts essentially the
whole probability in one cell.

**A note on reading the campaign report.** Its `stages` array is the PRESTATE,
assembled before execution — so a completed founding run still shows
`founding absent` there, and `execution.completed` and
`execution.market.completed` are where the answer is. The run also exits 0
either way. That is the shape this file already warns about one cohort earlier:
a driver's exit code is not a poststate, and the chain was re-observed here
before anything was claimed.

### Cost

| | SOL |
| --- | ---: |
| deployer before cohort-10's deploy | 42.945709919 |
| cohort-9 closed, reclaimed | +41.840595079 |
| cohort-10 deployed | −41.945742275 |
| cohort-10 closed, reclaimed | +39.881781471 |
| cohort-11 deployed | −41.932204987 |
| cohort-11 ladder, market staging, founding | −2.049094432 |
| **deployer now** | **38.738044775** |

The campaign payer holds 1.662489345 of the 2 SOL it was given. Cohort-10 left
behind nine finalized Registry record bodies and its own 2 SOL payer — the price
of learning that the frame exemption had rotted, paid once.

Devnet evidence. Not mainnet evidence.

## The load simulator against cohort-11, and where it stops

Condition (b) of the standing grant. The config binds cohort-11's real facts —
market `ARuPAuyJ…`, mint `H5zmg8nV…`, Claims aggregate `5wdhigoU…`, Hoard
`ANJc9A1z…`, campaign payer as the funding wallet, never the deployer — and two
fresh participants funded 0.05 SOL each from that payer.

Pointing the sustain loop at a real founded market immediately produced two
defects it had carried unseen, each the next refusal in order, both fixed in
`18b9a21c`:

- **It could not admit anybody.** The admission packet does not fit a legacy
  message; it routes through the founding's own **frozen DCLTGMF3 address
  lookup table**, and the driver refuses `PacketTooLarge` without one.
  `simlife_drivers.py` has carried that fact since SEL-SEAM; `simulator.py`
  never learned it.
- **`simlife`'s own discovery helper does not work here.**
  `frozen_routing_table_for` scans `getProgramAccounts` over the entire
  AddressLookupTable program and answered `None` for a market whose frozen table
  demonstrably exists — devnet's ALT program is far too large for that scan
  through a real endpoint. The address is in the founding's own create/freeze
  transaction: `6Pwb16HHphgvDbr6RW4p7k82qTGDccQHizJzk3LDXZwk`, from
  `5iyBJssn…` / `2jF8ETgM…`.
- **The journal lock has no directory.** The driver writes a lock beside its
  `--output` before the output, so the admission died on the lock rather than on
  anything about the admission.

With those closed, both participants' admissions compile and preflight. **The
run then stops at a wall that is not the protocol's**: the admission transaction
refuses `BlockhashNotFound` at simulation, reproducibly, *after* its prefund
transfer has landed (`4qMCqn7f…`). That is a v0 lookup-table packet whose
blockhash is stale by the time a load-balanced endpoint simulates it, and the
driver correctly refuses to re-sign an expired packet — *"archive the journal
rather than re-signing"* — which is replay safety working, not a retry to
weaken.

So the market is founded and alive in the sense that matters for the genesis
claim, and its **population life is not yet demonstrated**. What remains is
narrow and named: a blockhash-freshness fix on the admission submission path,
then the Direct trade path, which additionally needs authored seller/buyer
tickets and a checked execution release the simulator config still carries as
placeholders.

**Continued in `COHORT11_POPULATION_2026_09_02.md`**, which closes the admission
half: two strangers hold Positions in this market and six conservation laws hold
across four census boundaries. The blockhash was one of three walls, and the
narrow estimate above was wrong about the other two — the frozen routing table
had to stop being *searched* for, and the Position owner could not be its own
fee payer, which no prefund can repair. The Direct trade remains open, and its
residue is three named artifacts.

Devnet evidence. Not mainnet evidence.
