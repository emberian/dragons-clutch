# Cohort-9 devnet deploy — 2026-09-01

**Devnet evidence. Not mainnet evidence.** Nothing here says anything about
mainnet, and no mainnet act is authorized. This records a public-test
deployment of exact committed sources under ember's standing 2026-09-01 devnet
grant and disposability ruling.

## Source

- Commit `5ba7f3873e3e073811a390d3915e030ced24b261`, built from a clean
  detached worktree. Not the ambient dirty tree.
- Toolchain: `cargo-build-sbf` 4.0.0, platform-tools v1.53, rustc 1.89.0,
  target `sbpf-solana-solana`. Repository toolchain pin 1.97.1.
- Deployer / upgrade authority: `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`.

## Determinism control

Every ELF was built TWICE, in two independent detached worktrees at the same
commit, and compared. All seven byte-identical. The digests below are that
agreed value.

## Live program set

Deployed in dependency order, each verified by dumping the on-chain image back
and comparing it to the built ELF. All seven `IDENTICAL`.

| role | program id | bytes | ELF SHA-256 | deploy signature |
| --- | --- | ---: | --- | --- |
| registry | `Gaap8HNik9Qyc9oDo9fEwShJojwhWwtL1ddQUVH1wzP4` | 234,536 | `ed70f8bda12b77d663126218ad05f36dd77c5bf3100642879cef1441a845afe7` | `3hQz9HTFwLUre4vHi2fmwB5BygD7btcAwefGLdY9X7wisVV2Twhe2rsfjjEecGJL5VaJTJ4BHj6mCrNXHWYdDdw2` |
| rent | `4TG6qpDFxDd8cdQNeygzTjj2i2nkvnBGzJojkg7T1nLt` | 142,320 | `d46e5f0a64fd7d5e296118c2e7a62a3b67aed2c2ac4420e85069fb8dca632837` | `5nQMBrsFQVpgwMw78N6ikN1Qcc3jh32p8WggaoerRkP4E6czcvr5hJXgJ2gDZJmJQ8nhTaEcGniy8C2qNcxJ5ykW` |
| custody | `9kM3W6YzL5K6S3fwUo6EkhF6VDS9T1Ge9Fz1LNXdMwWR` | 571,432 | `2823c82351638566e295d7f7acc2e559ab61b3ea43750759e84f73bc0f80d567` | `52NnUK3ydpKrPQEbddGbd8RSAfE6ZV9C2fdFMzYd3rTV3oYCt8848B1XqGPSkhdgcRtnYtEMJWs3KceoCdvZMqjB` |
| resolution | `H9A5BFW2xqvveyYpeMZyPKrxM9YEGaGeGT6RNjovXpN3` | 820,864 | `93f6908bf4f3b5ddbc50dbf839d056a5a7638b82e267aed1388979ee98038a33` | `3tkTZ9JFrswxgHGQUPu7TCaiNzafLL2exYr2XfNQHPVRhn1YrGjWTskVbTcBjysWr7aiFmhUDYnLW6nSq8Z3VeME` |
| claims | `A9wmJggh3deVyQF3dJ5YxF7vgh95NeWd9uap1gLc9Kxg` | 1,363,672 | `002dd1133ef0cdb876411066b1fa8462ab6899958bd8b909f73020cf8b10a6f6` | `58pnfoNnNhz5ZvYoFka5iBNt67qVuEHGijXPyHabBHsa422Z5uQH4yc4yt82dLgfHMRvBEzbfRxo1ddL6zbivxsS` |
| trading | `7JZKAonrsHeQyb7idmJdScV9sEDzi8EkRAqHFj6GGkmX` | 2,285,728 | `50d57606d8a7c9bbf71a827694ef3048c3613e362dd46b461c1b1494e488836e` | `3M5gPqhVPRCzrKL5Zob2XJFnrwyLRhDqRD67rhfvGcVHNXLqhpFuMXjT3tPirekb7X7D5stpAAe9B6u3n3Q2ZVaf` |
| core | `AXCKJ2rYXvF95esfczvVfdnzbqypLWA59HjRDCt4LeEA` | 1,187,000 | `2d7fe4ecb07a77cd4b2dbb5344ce85d1e981d9063a14b15ce1b0696000016793` | `3etaPozwzjDymjBcAM6W5DtWvTgWYCqvKhydrHBPxCESHKv8VsZP286Ng5PKAF1PbKYWgeBzbTvwt6ZzCfaigQa4` |

All seven retain upgrade authority `4zrxtw5c…` — **mutable / `ExactAuthority`**,
not immutable. That is a deliberate choice, not an omission: ember's grant is
standing ("whenever and as often as you feel ready"), and repeatable redeploys
only work if the rent comes back. `programs/dclutch-core-sbf/src/
infrastructure.rs:311-314` admits `ExactAuthority`, so this is the sanctioned
iteration substrate rather than a weakening.

## Cohort-8, abandoned in place then closed

All seven cohort-8 programs are closed and their ids can never be reused.

| role | closed program id | reclaimed SOL |
| --- | --- | ---: |
| trading | `5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk` | 14.38201176 |
| core | `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N` | 7.75965528 |
| claims | `85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN` | 8.91000408 |
| registry | `Hies39GBowHUMZw9rVCfaDTAXNorkQqMGKnukY2MD4Qj` | 1.44242520 |
| rent | `DgfYeuorJUmnktxgCmUXy65f6MFBGcc1aMQoauxoJCY3` | 0.95895576 |
| custody | `34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH` | 3.99101016 |
| resolution | `2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd` | 5.67449496 |
| | **total** | **43.11855720** |

## Cost, measured against the forecast

- Balance before: 43.742833706 SOL. Reclaimed: 43.118557200. Inflow 86.861390906.
- Balance after: **44.980665543 SOL**. **Spent: 41.880725363 SOL.**
- Forecast was 46.037939, so the estimate was **4.157 SOL high**. The rent model
  `890,880 + 6,960·(45 + elf_len)` over-predicts what `solana program deploy`
  actually allocates. The forecast was still load-bearing and correct in its
  conclusion: at 43.742834 on hand the deploy could not have proceeded without
  the reclaim, and it did not.

## The attempt that failed first, and why it cost nothing

The first deploy run died on the very first role with:

```
Error: Dynamic program error: missing signature for supplied pubkey:
4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP
```

`solana program deploy --upgrade-authority` takes a **signer** — a keypair path
— not a pubkey. Passing the authority's public key is accepted by the argument
parser and then refused at signing time, after the deploy has otherwise been
assembled.

**Nothing was spent.** The balance was 74.794489826 SOL before the attempt and
74.794489826 after it. The run stopped at role one because the runner verifies
each role — dumping the on-chain image back and comparing it to the built ELF —
and exits non-zero the moment a role fails either its deploy or its
verification. A runner that deployed all seven and only then checked would have
attempted the whole 6.6 MB against an unsatisfiable signature and burned the
budget on the first attempt, with the reclaim already spent and nothing live.

This is the argument for verify-after-each over verify-at-end on any sequence
whose steps spend money. Keep it.

## What is NOT done

The programs are live. The protocol is not yet founded on them:

- No infrastructure profile is initialized, no artifact records are published,
  no role is activated. **The succession ceremony has NOT executed** and
  nothing here should be read as saying it has.
- No market is founded. This is deliberate and it is a named wall, not an
  oversight — see below.

## Named wall: no market may be founded on this cohort yet

`tools/local-validator/bootstrap/successor/src/spline_product.rs:445` and
`market.rs:3138,12691` call `compile_product_records_v2`, the UNGATED product
entrance. `market.rs:12180-12182` (`demo_market_input`, via
`LocalMarketShapeV1::default`) still carries `cut_denominator: 100` and cuts
`[12_000, 18_000]` — byte-identical to the web wizard defect that makes a
SOL/USD market resolve into its top cell 100% of the time.

The gated entrance `compile_interesting_product_records_v2` exists and refuses
exactly that with `DegenerateOutcomePartition`, and its test admits a correctly
centred partition, so it is not a checker that refuses everything.

It cannot be called from this path yet. It requires a `FoundingBandV1` of
`{anchor, denominator, volatility_bps, window_slots}`. Spot and window are
available; **volatility is not**. `spline_product.rs:44-65` has twenty fields
and every one is geometry or identity — no spot, no window, no volatility. The
only nearby candidate, `max_confidence_bps` (`market.rs:12104`), is by its own
comment "the adapter's tolerance for the provider's own stated confidence
interval" — a refusal bound the caller states, not an observed volatility over
a window. Using it would manufacture precisely the false confidence the gate
exists to catch.

So markets are not founded, and the load simulator has not been run against
this cohort. Founding a cohort of markets that all resolve into one bucket
would satisfy the letter of "full redeploy including the load simulator" and
fail its point.


## The genesis checked release candidate, run for the first time

`checked-release-candidate.sh --genesis-cohort` had never been executed end to
end. It has now, four times, and each run moved the wall. The pinned Node
v26.4.0 archive was fetched and its SHA-256 matched the script's own pin
(`bef4c7e7…57dd`), which removes the "no Node archive" wall entirely.

What the genesis path now does, from a clean archive of a named commit:

1. builds all thirteen SBF links and passes the freshness gate, zero diagnostics;
2. emits thirteen artifact-provenance descriptors;
3. passes the public spline Product compiler/SDK handoff gate;
4. checks all ten role artifacts and the five-role execution release set;
5. derives the genesis infrastructure profile — `infrastructure/profile.bin`,
   **exactly 144 bytes**, the write-once V1 a cohort that succeeds nothing
   commits.

Three walls were found by running it, each invisible to every existing suite:

- **the sixteenth stale lockfile.** The successor `Cargo.lock` could not resolve
  under `--locked` at committed HEAD, so the candidate died with no message at
  all. Fixed.
- **cargo package-cache contention.** Concurrent cargo work in the same session
  put `Blocking waiting for file lock on package cache` into a build log, and
  the provenance gate correctly refused a log it could not attribute. Not a
  defect: a release build must be exclusive.
- **an exact-key set that had gone stale.**
  `verify-spline-product-handoff.mjs` validated the SDK inspection by exact key
  equality, and the compiler had grown `partition_quality`. Fixed, and the
  field is now checked rather than tolerated.

### The remaining wall, and it is structural

`create-infrastructure` refuses the genesis profile:

```
dclutch-release-tool: infrastructure profile refused: InvalidLength
```

`CheckedInfrastructureV1` structurally embeds a `ProtocolInfrastructureProfileV2`
— `build_checked_infrastructure_v1(execution, profile: ProtocolInfrastructureProfileV2, …)`
— and V2 exists precisely to pin the two predecessor artifact-release ids a
succession copies forward. A cohort that succeeds nothing has none, and commits
a V1.

So the genesis half of the release candidate is complete through the execution
release set and stops at the infrastructure manifest. Closing it means either a
genesis manifest variant or a version-polymorphic profile field in
`dclutch-release-set-contract`. That is a change to a release-identity-bearing
structure, which is a release event and an Ember-scheduled one — not a
lane-local fix, and not something to improvise underneath a live cohort.

`bf5499da` closed the profile-derivation half of this gap. This is the manifest
half, and it is named rather than worked around.

## The submission driver: it exists, and it is `campaign`

Traced, not guessed. `tools/release/stage-devnet-sponsored-market-open.sh`
prepares a market and then WRITES OUT the invocation it deliberately never
runs, into `<work>/open-market.execute.sh`:

```
dclutch-local-successor-bootstrap campaign --founding-only \
  --rpc-url <devnet> --i-mean-devnet <genesis> \
  --plan <plan> --market <market.json> --evidence <campaign-open.json> \
  --keypair-campaign-payer … --keypair-collateral-mint … \
  --keypair-collateral-wallet … --keypair-founding-beneficiary … \
  --keypair-founding-projection-witness … --keypair-founding-source-funder … \
  --founding-founder <derived> --substituted-founder <distinct> --execute
```

It is gated behind `DCLUTCH_AUTHORIZE_MARKET_OPEN=YES` and six keypair paths,
and it proves founder-key CUSTODY before it will run — because on 2026-08-30
all three live devnet markets were found sharing a founder nobody holds, and
none of them can ever be retired (decision 0015 §8).

So the answer is **"the driver is elsewhere", not "the driver is missing"**.

`campaign` owns the whole ladder as ordered stages: Substrate, Publication (the
nine records), Initialize (the profile), Succession, Activation, Founding.
`--founding-only` refuses unless the five before it are Complete, which is
exactly the ordering the chain requires.

### Cohort-9's observed ladder

```
substrate    complete
publication  absent
initialize   absent
succession   complete
activation   absent
```

`substrate complete` is independent confirmation that the seven deployed roles
match what the plan pinned, verified against chain state rather than against
the deploy tool's own report.

`succession complete` is NOT a claim that the ceremony ran. `succession_state`
returns Complete when `plan.infrastructure_succession` is None: it reports that
this plan HAS no ceremony. The ceremony has still never executed.

### Correction: publication is PARTIAL, not absent

The ladder above is the state BEFORE the write attempts. Re-observed
afterwards, two of the nine records had finalized on chain despite every
attempt returning 429:

```
substrate    complete
publication  partial    2 of 9 finalized; still missing or in flight:
                        execution_release_set, pyth_release,
                        registry_artifact_release, rent_artifact_release,
                        resolution_artifact_release, trading_artifact_release,
                        custody_artifact_release (in flight)
initialize   absent
succession   complete
activation   absent
```

The deployer balance moved 44.980665543 -> 44.968159503 SOL, and that 0.0125
SOL is those two records' rent. So the campaign is not merely refusing, it is
RESUMING: each attempt lands what it can before the endpoint cuts it off, and
the next one skips what finalized.

Worth stating plainly because the earlier reading would have been wrong. A
driver that reports a stage `absent`, fails, and is then assumed to have
written nothing is exactly how a partial external mutation goes unnoticed --
the state has to be re-observed after the attempt, not inferred from the
attempt's exit code.

### Where it stops, and it is not code

Publication cannot proceed on `https://api.devnet.solana.com`, which is
rate-limiting `getSignatureStatuses`:

- six consecutive campaign attempts, spaced 90 s apart, each failing at the
  FIRST signature poll of the write path with `HTTP 429 Too Many Requests`;
- reproduced independently of this tree with plain `curl` — five calls, five
  429s — while `getHealth`, `getSlot` and `getLatestBlockhash` all returned
  200 in the same seconds, and the same method returned 200 minutes later.

So it is bursty, per-method throttling on the free public endpoint, not a
defect in the driver and not something a retry fixes: the read-only observation
pass above completed on the same endpoint, and only the write path's polling
rate exceeds the limit. The driver's own 250 ms pacing is a measured-profile
constant (SMOKE-0) and was NOT weakened to get past this.

What it needs is a devnet endpoint that is not the free public one. That is an
operator input this lane does not hold, and hunting for credentials would be
the wrong way to obtain it.

## Re-observation after the driver died — 2026-09-01 18:11-18:15

The previous lane's last written report said *2 of 9 records finalized,
44.968159503 SOL*. The balance was then read at 42.945709919 — 2.022449584 SOL
gone after its last observation, far more than seven records' rent. The rule
this file already carries says a driver that fails and is assumed to have
written nothing is how a partial external mutation goes unnoticed. So: observed
first, acted second.

### The ladder, read live off devnet at slot 491667150

`campaign --founding-only` in its preflight (reads only, enforced) mode, run
from tree root `/Users/ember/dev/dclutch` at HEAD `64cc3436`:

```
substrate    complete
publication  complete    9 of 9
initialize   complete
succession   complete    (reports the plan has NO ceremony; it has never run)
activation   complete    5 of 5
founding     partial     the Open Market does not exist at
                         6URVxwyXUfXFxwEFhvGysx89fx4vQTCKxUiBwXvwCoK9 but this
                         founding has started: collateral mint DAjjZfKR…,
                         collateral wallet AftBLbo2…, realm record 6uwcZDqL…
```

The five stages the previous lane was driving all completed. `publication
complete` is nine of nine because `publication_state` returns Complete only
when `missing` and `partial` are both empty over `plan.records`, and this plan
has nine. `activation complete` is five of five because
`activation_state_from_progress` returns Complete only on
`progress.is_complete()` against `ACTIVATION_ROLE_COUNT_V1 = 5` (Core, Claims,
Trading, Resolution, Custody). Positive control on chain: the activation cache
at `EGY1DPNCmbTTFX4uAbpJuuW9YBmhXpSzXFF6MtxyHVxU` is 1,288 bytes owned by the
Registry program `Gaap8HNi…`.

**`succession complete` still is not a claim that the ceremony ran, and this
time that is provable rather than merely restated.** The V2 profile PDA
`G5M2jgBQXypkUgoMc2jswH8WZx61ykDcSeQm3Bfw5tTF` — `find_program_address`
`["dclutch:infrastructure:v2"]` under core `AXCKJ2rY…` — reads back as an
absent account: System-owned, zero data, zero lamports. The one-per-domain
vacancy `InitializeProtocolInfrastructureV2` demands is untouched. The
derivation is not asserted: the same script derives the V1 domain to
`FYzxwAjEqRwzYA727p3K3bCyEioesJoCQzx6oKVeVmNe`, which is byte-for-byte the
address the plan already carries. That is the positive control on the
instrument.

### The 2.022449584 SOL, fully accounted

Three transactions, no residue:

| what | lamports |
| --- | ---: |
| the remaining seven records, the profile, the five activations (deployer-paid) | 22,369,584 |
| the last ladder transaction's fee | 75,000 |
| `23sCw5Vz…` slot 491600580: **System transfer of exactly 2.000000000 SOL to the campaign payer `3gDQDzsh…`**, plus its 5,000 fee | 2,000,005,000 |
| | **2,022,449,584** |

Read off `getTransaction` pre/post balances, not inferred. The 2 SOL did not
leave the cohort: it capitalized the founding payer, which is a distinct
keypair from the deployer and holds 1.805402034 SOL now. The 0.194597966 SOL
difference is the two founding attempts described below. **Nothing is
unaccounted for.**

### Founder-key custody, proven before anything else

`/private/tmp/c9keys/founder.json` derives
`GrjLXgD2Pbd9Lqf2nAqes3wzDdPNswJwiWoJaapQXUJu`, which is exactly the
`founding_founder` in the campaign's execution intent. We hold it. Under
decision 0015 §8 that is the check that must pass before a fourth unretirable
market can exist, and it passes.

It was however sitting only in `/private/tmp`, which macOS clears. A founder
key that survives until the next reboot is the 2026-08-30 defect on a delay.
The six generated keypairs, the plan and the market input are now also at
`~/jobs/dclutch-cohort9-20260901/`, mode 700.

### Two stranded foundings, and why retrying was the wrong instrument

The founding stage ran twice and stranded twice:

- 15:07-15:15, collateral mint `GAwdVv3K…`, wallet `G1HKXpdb…`, realm record
  `9amZxeS4…`. Refused on the resume, correctly: *"this founding has STARTED on
  this chain … but no compatible durable DCLTPCB2 checkpoint authenticates a
  safe suffix resume."*
- 17:58-18:07, fresh mint `DAjjZfKR…`, wallet `AftBLbo2…`, realm record
  `6uwcZDqL…`, after regenerating the mint and wallet keypairs (the stranded
  pair is kept as `*.stranded.json`). Fifty-five transactions of market-record
  publication landed, and then:

```
Error: Error("chain-derived Found projection: AccountAuthority")
```

`AccountAuthority` has twenty-plus return sites in
`crates/dclutch-product-runtime-v2-operator/src/found.rs` — the coarse-refusal
shape AGENTS.md names — so a third attempt would have bought another stranded
mint and the same word. It was localized without spending anything instead,
because `project_found_v2` is a pure function over chain-read state.

### The wall, convicted at one line for zero lamports

`tools/local-validator/bootstrap/successor/src/market.rs:2003` wraps
`project_found_v2`. Its first conjunct block is
`authenticate_runtime_accounts`, and
`crates/dclutch-product-runtime-v2-operator/src/found.rs:548` reads:

```rust
|| state.infrastructure_profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
```

`PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2` is **224**
(`crates/dclutch-release-set-contract/src/generated_protocol_infrastructure.rs:22`).
The profile this cohort committed, read off chain at
`FYzxwAjEqRwzYA727p3K3bCyEioesJoCQzx6oKVeVmNe`, is **144 bytes**, owner
`AXCKJ2rY…` (core), lamports 1,722,576 — which is exactly devnet's live
`getMinimumBalanceForRentExemption(144)`, so it is a correctly-rented V1, not a
damaged V2. `PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1` is 144. 144 ≠ 224.

`git log -L 548,548` names the seam exactly. Commit `2951b226`, *"profile
succession: every consumer reads V2, and the predecessor is never an authority
again"*, changed that single line:

```
-        || state.infrastructure_profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V1
+        || state.infrastructure_profile.data.len() != PROTOCOL_INFRASTRUCTURE_PROFILE_BYTES_V2
```

That is why cohort-8 founded markets 21 and 22 on a profile that is also 144
bytes (`5pisnL8e…`, still on chain) and cohort-9 cannot: cohort-9 is the first
cohort deployed after the consumer moved to V2.

### It is not the driver picking the wrong account. The driver's V1 arm is now unreachable-good.

`market.rs:4290` `checked_found_infrastructure_selection_v1` dispatches on
`(plan.infrastructure_succession.is_some(), successor_profile_observed)`:

```rust
(false, false) => Ok(FoundInfrastructureSelectionV1::Predecessor),
```

Cohort-9 is exactly `(false, false)` — no succession in the plan, no V2 on
chain — so `authenticated_found_infrastructure_plan_v1` returns the plan
unchanged and `market.rs:4739-4740` feeds `plan.infrastructure_profile.address`,
the 144-byte V1, into the projection. **After `2951b226` the `Predecessor` arm
can no longer produce a foundable projection at all.** It is not dead code that
never runs; it is the arm a genesis cohort always takes, and it now always
fails, sixty transactions deep.

### It is two independent conjuncts, not one stale constant

Worth stating so nobody reaches for a one-line repair. `found.rs:548` is only
the first refusal on this path. `authenticate_infrastructure`
(`found.rs:676-686`) derives its expected profile from
`PROTOCOL_INFRASTRUCTURE_PROFILE_PDA_DOMAIN_V2` and compares it to
`state.infrastructure_profile.key` — which, on the `Predecessor` arm, is the V1
PDA `FYzxwAjE…`, a different address — and separately requires
`lamports >= rent.minimum_balance(224)`, which is 2,229,216 against the
account's 1,722,576. Relaxing the length check would walk into both.

The core program has no genesis V2 writer either:
`programs/dclutch-core-sbf/src/infrastructure.rs:67` constructs a
`ProtocolInfrastructureProfileV1` and `:587-616` commits 144 bytes at the V1
PDA. The only writer of a V2 anywhere is the succession ceremony.

### Why this is not fixable underneath the live cohort, and the refusal

The obvious repair — run the succession ceremony so a V2 profile exists — is
refused by the ceremony's own conjunct 4, and it should be.
`programs/dclutch-core-sbf/src/infrastructure_v2.rs:29-34` states it: a moved
binding's successor record must bind **a strictly later deployment slot** than
the predecessor it replaces, and *"a succession in which nothing moved selects
nothing and would only burn the one V2 vacancy — refused."*
`market.rs` restates it on the driver side at 4344:
`successor.registry().artifact_release() == predecessor.registry().artifact_release()`
is a refusal. Cohort-9 was deployed hours ago and nothing has moved.

The way to make something move would be to re-upload the Registry program's
identical bytes to bump its deployment slot, publish a new artifact release for
it, and then call the succession "forward". That manufactures exactly the
forward step conjunct 4 exists to require evidence of. **Refused. A slot bump
with no changed bytes is a ceremony performed on a check, not a succession**,
and it is also a partial single-program deploy, which condition (a) of the
standing grant forbids by name.

Nor can the genesis cohort simply write a V2 in the first place:
`ProtocolInfrastructureProfileV2::new`
(`crates/dclutch-release-set-contract/src/protocol_infrastructure.rs:266-277`)
refuses when `predecessor_registry_artifact == predecessor_rent_artifact` with
`AliasedInfrastructureBinding`, so a genesis encoding of "no predecessor" — two
absent ids, which are equal — is structurally impossible. V2 has no genesis
shape.

**So: cohort-9 is live, complete through activation, and cannot be founded.**
This is the same structural gap this file already named one rung higher —
`create-infrastructure` refusing the genesis profile with `InvalidLength`
because `CheckedInfrastructureV1` embeds a V2 — now measured a second time, on
chain, at the founding rung, having cost 0.19 SOL and two stranded collateral
mints to reach and nothing further to convict. Closing it is a change to a
release-identity-bearing structure. That is a release event and an
Ember-scheduled one. The two candidate shapes, so the decision is not restarted
from nothing:

1. **A genesis discriminant in V2**, so `predecessor_*` has a well-formed
   "none" that is not aliased, and the genesis cohort writes 224 bytes at the
   V2 PDA directly. Keeps every consumer V2-only, which is what `2951b226`
   wanted.
2. **A version-polymorphic profile field**, so `authenticate_runtime_accounts`
   and `CheckedInfrastructureV1` accept a V1 at the V1 PDA for a cohort with no
   predecessor and a V2 otherwise. Cheaper, but reintroduces the two-authority
   shape `2951b226` deleted on purpose.

Consequence for the standing grant: condition (b) — the load simulator running
against the new cohort — is blocked behind this, because there is no market on
cohort-9 to load. Naming that is better than founding a market that could not
have existed.

### `2951b226` knew. It says so in its own last paragraph.

This was not an oversight, which changes what the fix is for. The commit that
moved every consumer to V2 closes with:

> The bootstrap successor tool stays coherently on the predecessor profile and
> moves as one piece when it gains a ceremony stage. It has to: **the ceremony
> refuses a succession in which nothing moved, so a world whose Registry has
> never been upgraded cannot reach V2 at all**, and that tool builds exactly
> such a world.

So the gap was priced and accepted as a tooling-coherence problem. What was not
priced is that it is also a **founding** problem the moment a real genesis
cohort exists — which is what cohort-9 is, and what cohort-8 (founded before
the flip) never had to be. `PROFILE_UPGRADE_RULING_2026_08_31.md:259-266`
reasons carefully about the window *between the Registry upgrade and the
ceremony*; it never reasons about the window *before there has ever been an
upgrade*.

Note also that the ceremony has had a real devnet driver since `2a10fa4c`:
`devnet-infrastructure-succession-v1 --core --registry-artifact --rent-artifact
--evidence --i-mean-devnet [--execute …]`, whose own usage text says *"Run
ONCE, on cut day, AFTER the Registry upgrade and BEFORE the declarations."*
There is no missing tool. There is a missing upgrade, and no honest way to
manufacture one.

### A precondition nobody had written down, and cohort-9 satisfies it by luck

Conjunct 5 requires the **predecessor** release's bound upgrade authority to
sign (`infrastructure_v2.rs:285-294`), and an `Immutable` release binds none:

```rust
let bound = predecessor_release
    .upgrade_authority()
    .ok_or(CoreSbfError::InfrastructureConsentMissing)?;
```

**Therefore a cohort that revokes upgrade authority on both Registry and Rent
before committing its V1 profile can never reach V2, and is permanently
unfoundable — silently, at deploy time, with no diagnostic until sixty
transactions into a founding months later.** Cohort-9 retained
`ExactAuthority` on all seven roles, so it is not trapped. That was argued in
this file as an iteration-substrate convenience ("repeatable redeploys only
work if the rent comes back"). It turns out to have been load-bearing for a
reason nobody had stated. Whatever shape the fix takes, this precondition
should become a refusal at deploy-planning time rather than a property a cohort
happens to have.

### The sharpest form of the question, for whoever schedules the fix

Conjunct 3 of the ceremony says identity never moves: V2's Registry program
must be the same program id as V1's, only its artifact release may move
forward. So there *is* a path by which cohort-9 becomes foundable without any
code change at all — upgrade the Registry program in place, on the same id,
with genuinely different bytes; publish its new artifact release; run the
succession honestly on that real forward step; then found. Nothing is weakened
and every conjunct is satisfied on its merits.

That path is not available today (there is no Registry change to make, and
condition (a) of the grant forbids a partial single-program deploy), but naming
it exposes what the decision is actually about. If it is the intended
lifecycle, then **a freshly deployed cohort is never immediately foundable, and
the first market on any new protocol deployment can only exist after the
Registry has been upgraded at least once.** That is a strange property to have
acquired by moving one constant, and it is almost certainly not what `2951b226`
meant to buy.

So the question for ember is not "which of the two shapes above" so much as:
*is a genesis cohort supposed to be foundable on the day it is deployed?* If
yes, V2 needs a genesis shape. If no, the ceremony should say so out loud, and
the release candidate should refuse a genesis cohort at planning time rather
than sixty transactions into a founding.

### Ledger of what is on chain and stranded

| account | what | disposition |
| --- | --- | --- |
| `FYzxwAjEqRwzYA727p3K3bCyEioesJoCQzx6oKVeVmNe` | V1 infrastructure profile, 144 B | correct, permanent, sealed |
| `G5M2jgBQXypkUgoMc2jswH8WZx61ykDcSeQm3Bfw5tTF` | V2 profile PDA | vacant; succession never ran |
| `EGY1DPNCmbTTFX4uAbpJuuW9YBmhXpSzXFF6MtxyHVxU` | activation cache, 1,288 B | complete, five roles |
| `GAwdVv3K…` / `G1HKXpdb…` / `9amZxeS4…` | first founding's mint, wallet, realm | stranded |
| `DAjjZfKR…` / `AftBLbo2…` / `6uwcZDqL…` | second founding's mint, wallet, realm | stranded |
| `3gDQDzsh5ceKhrHArWFfVDhtyvVRV897y8ToLJFEY8by` | campaign payer | 1.805402034 SOL |
| `4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP` | deployer / upgrade authority | 42.945709919 SOL |

Devnet evidence. Not mainnet evidence.

## Correction: the wall is ON CHAIN, and the host refusal was only its mirror

Written 2026-09-01 19:0x, after the coordinator directed this lane to build the
genesis arm. Two facts found while scoping it change the build, and both were
found by reading rather than by spending, so they cost nothing but say the plan
as issued cannot work.

### 1. No host-side change can found cohort-9

`project_found_v2` lives in `dclutch-product-runtime-v2-operator`, which is a
**dev-dependency** of `programs/dclutch-core-sbf` — the projection is host-side
and never runs on chain. That was promising for a moment, and then:

`programs/dclutch-core-sbf/src/found.rs:289` and `:311` route both Found paths
through `authenticate_projected_found` / `authenticate_found`, which call
`authenticate_profile`, whose own doc comment
(`programs/dclutch-core-sbf/src/infrastructure.rs:124-135`) reads:

> **V2 only, and never a fallback.** Every route reaching here reads the
> succession profile at `dclutch:infrastructure:v2` and nothing else. … Before
> the ceremony this read refuses on vacancy (`CoreSbfError::Infrastructure`, the
> width check below, since a vacant PDA is System-owned and zero-length); the
> ceremony is what un-refuses it.

And it is in the **deployed** sources: `git merge-base --is-ancestor 2951b226
5ba7f387` returns true, and `git show 5ba7f387:…/infrastructure.rs` carries the
V2-only read at its lines 151-159. Cohort-9's live Core ELF was verified
byte-identical to that build.

**So the host-side `AccountAuthority` this lane convicted is a faithful mirror
of what the chain would do, not a driver defect.** A genesis arm in `found.rs`
and `market.rs` alone would move the refusal from the planner to the validator:
the founding would run the collateral mint, the fifty-five record publications,
and then refuse on chain with `CoreSbfError::Infrastructure` — stranding a third
mint to learn a fact already in hand. The red-then-green control as issued
("after: it founds") cannot be met on cohort-9 by any host change.

The fix is a **program** change, and it reaches chain only through a redeploy.
That is cohort-10, and condition (a) of the standing grant authorizes it
precisely — full redeploy, all seven roles, from a named commit.

### 2. The prescribed shape is refused by name in a written ruling

`docs/design/PROFILE_UPGRADE_RULING_2026_08_31.md` §6:

> **V2-only in redeployed consumers. No fallback.** A try-V2-then-V1 read was
> considered and refused: it creates two live authentication paths (an O-005
> "parallel authority path" smell), **its only benefit is founding during the
> mid-cut window that gate 7 forbids anyway**, and its failure mode (V2 creation
> forgotten, V1 silently still ruling) is exactly the silent divergence this
> codebase spends itself refusing.

"A genesis arm on the founding path that authenticates a V1 profile at the V1
PDA domain and the V1 rent floor" is try-V2-then-V1 with a genesis guard. The
ruling refused it, AGENTS.md refuses it generically ("Do not preserve parallel
legacy/current authority paths"), and the program says so in its own comment.

**But the ruling's benefit analysis is now stale, and that is the real finding.**
§6 says the fallback's *only* benefit is founding during the mid-cut window,
because §6 was reasoned for the cohort-8→9 plan of record: an **in-place
upgrade**, with markets 21/22 to drain and hop and refound (§6's own ordering
list, steps 1-6). That is not what happened. Cohort-8 was closed entirely and
cohort-9 was deployed fresh, with new program ids and no predecessor — so there
is no mid-cut window, and the fallback's benefit is not "founding during a
window gate 7 forbids." It is **founding at all, ever, on a genesis cohort.**

The premise of the refusal is dead. The failure mode it named — two live
authentication paths, the ceremony silently forgotten — is not.

### The shape that satisfies both, and the one open design unit

A **genesis-shaped V2, written at the V2 PDA by initialize.** Then:

- there is exactly one authentication path, V2 at `dclutch:infrastructure:v2`,
  224 bytes. §6 holds in full; O-005 holds; no fallback exists to forget;
- vacancy still refuses, and still means the ceremony is owed;
- a genesis cohort is foundable on the day it deploys, which is what the grant
  requires and what cohort-8 had;
- the succession ceremony is untouched for real upgrades, and a succession
  cohort is still refused a V1 profile by name, because nothing reads V1.

It is buildable **without a layout change**. The blocker named earlier in this
file — that "no predecessor" cannot be encoded — is real but narrow:
`ContentId::new` refuses all-zero (`ZeroContentId`, `dclutch-product-runtime-v2/src/lib.rs:73-78`)
so `ArtifactReleaseIdV1::new([0;32])` fails, and `ProtocolInfrastructureProfileV2::new`
refuses two equal predecessors as aliased. Two **distinct, domain-separated
sentinels** — `hash("dclutch:infrastructure:genesis:registry")` and
`…:rent` — are nonzero and unequal, so the existing constructor already accepts
them, and they are unforgeable as real artifact digests by construction.

**One design unit genuinely needs a decision before it is built**, and it is not
the encoding. Conjunct 6 of the ceremony is one V2 per domain, ever
(`InfrastructureAlreadySucceeded`, a vacancy that is burned once). If genesis
writes the V2, a genesis cohort has spent its single vacancy at birth and can
**never** succeed its Registry — which reintroduces P-008, the protocol-wide
brick the whole succession exists to repair, for exactly the cohorts that start
clean. So the genesis V2 needs either a generation counter, or a V2→V3 hop, or a
vacancy rule that distinguishes "succeeded from V1" from "born at V2". That is a
choice about the identity structure and it wants one paragraph of ruling, not a
lane's guess made underneath a cohort.

Named rather than improvised. The two workarounds declined earlier stay
declined, and this lane declines a third: shipping the V1 fallback because it is
the shape that was asked for, when a written ruling refuses it and the reason
the ruling gave has simply changed rather than disappeared.
