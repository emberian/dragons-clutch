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
