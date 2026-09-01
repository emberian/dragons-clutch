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
