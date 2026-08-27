# Cluster-observed upgraded Pyth generation (2026-08-26 cutover)

These bytes are **bounded public JSON-RPC observations** of the Pyth Core
"upgraded" generation as it stood on Solana `mainnet-beta` and `devnet` on
**2026-08-27T02:00Z–02:10Z**. They are cluster-observation evidence: they say
what those clusters carried at that moment. They are *not* a production release
row, a provider-availability claim, a liveness guarantee, or executable
campaign input on their own.

The sibling directory `../local-upgraded-2026-08-22/` remains the **executable
lab** fixture and is unchanged historical evidence. This directory is the
**observation** that dates it, binds it per cluster, and adds the one program
the lab fixture never held.

All reads were read-only (`getGenesisHash`, `getAccountInfo`,
`getMultipleAccounts`, `getBlockTime`, `getSignaturesForAddress`,
`getTransaction`). No writes, no signing, no keypairs, no airdrops. The
complete read log is `RPC_READS.md`.

## Cluster identity is an explicit bound fact

A Pyth release is pinned **per cluster**, and the cluster is named by its
genesis hash — never inferred by the adapter.

| cluster | `getGenesisHash` |
| --- | --- |
| `mainnet-beta` | `5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d` |
| `devnet` | `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG` |

## The three programs of the generation

Program accounts are Loader V3 `tag = 2`, 36 bytes, and are **byte-identical on
both clusters**.

| role | program id | ProgramData |
| --- | --- | --- |
| Wormhole receiver ("router") | `HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL` | `9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x` |
| Pyth Solana receiver | `rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp` | `3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX` |
| Pyth push oracle | `pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou` | `9nxngQjxBGUZ3ajfqoTrpiuDBVfztXCQVDuWDAw52Gew` |

## Measured fact 1 — the ELFs are identical across clusters

The ProgramData ELF tail (bytes `[45..]`) was fetched in full from both
clusters and hashed. All three programs are byte-for-byte identical across
`mainnet-beta` and `devnet` (measured 2026-08-27):

| role | ELF bytes | ELF SHA-256 (both clusters) |
| --- | --- | --- |
| router | 655,960 | `f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb` |
| receiver | 416,864 | `c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64` |
| push oracle | 234,952 | `a0318a87b80cebf9633e2b16e81984af5633e9a72ab491960ca16fbfd0d7d916` |

Only `push-oracle.so` is stored here. The router and receiver ELFs are **not
duplicated**: they are byte-identical to the already-committed lab fixture, and
that equality is itself the recorded fact (see "Measured fact 3"). One semantic
owner per persisted fact — `../local-upgraded-2026-08-22/{router,receiver}.so`
owns those bytes.

## Measured fact 2 — the ProgramData still differs per cluster

Equal ELFs do **not** make a shared release. The 45-byte Loader V3 metadata
header differs on both clusters, so the complete ProgramData body digest — the
thing an on-chain deployment observation actually binds — is per cluster.

| | `mainnet-beta` | `devnet` |
| --- | --- | --- |
| upgrade authority (all three) | `6oXTdojyfDS8m5VtTaYB9xRCxpKGSvKJFndLUPV3V3wT` | `upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr` |
| router deployment slot | 417,825,233 | 460,336,290 |
| receiver deployment slot | 417,825,260 | 460,336,311 |
| push-oracle deployment slot | 417,825,281 | 460,336,332 |
| slot wall clock (`getBlockTime`, router) | 2026-05-05T20:55:18Z | 2026-05-05T20:53:31Z |
| ProgramData sizes (router/receiver/push) | 656,005 / 416,909 / 234,997 | identical |

Complete ProgramData body SHA-256 (45-byte header + ELF, the full account):

| ProgramData | `mainnet-beta` | `devnet` |
| --- | --- | --- |
| router `9hLWdeVh…` | `3b911964d5c74335cf81838f46903abd04ffd3fe7ed7bc2661add50fbf90d4b3` | `f26f4b53b0f980455886116f500fa74ba475e51b1acb7f486b18afa9d73d948f` |
| receiver `3UV7w2yT…` | `292d187cfc879f5b0f9dd061f76ea96ea4f8193a83d3de654652309769a57ecf` | `7122abc6b5e78d30bf88c869cb5d8783adaf897369d04eca827d3af8ffe18e5d` |
| push oracle `9nxngQjx…` | `0238fa7b6724e2dde966c96a84131d4c244c0a896555ebdf04e900902c072d84` | `95c4f5d726073d533c1509eb79260d914a8c1ca939e91f18de062f98328b5e97` |

**This is the load-bearing per-cluster reason.** The earlier statement that the
upgraded generation has "identical `ProgramData` sizes on both clusters" is true
but understates the situation in one direction and overstates it in another:
the *binaries* are not merely the same size, they are identical; and the
*accounts* are not interchangeable, because deployment slot and upgrade
authority differ. A release pinned to a complete ProgramData digest, a
deployment slot, or an upgrade authority is cluster-specific by construction.

## Measured fact 3 — the lab fixture is on the *upgraded* side of the cutover

`../local-upgraded-2026-08-22/receiver.so` and `router.so` are byte-identical
to the live upgraded receiver and router on **both** clusters, verified today:

```sh
# after re-fetching, tail -c +46 of each ProgramData account
cmp devnet-receiver.so  fixtures/pyth/local-upgraded-2026-08-22/receiver.so   # equal
cmp mainnet-receiver.so fixtures/pyth/local-upgraded-2026-08-22/receiver.so   # equal
cmp devnet-router.so    fixtures/pyth/local-upgraded-2026-08-22/router.so     # equal
cmp mainnet-router.so   fixtures/pyth/local-upgraded-2026-08-22/router.so     # equal
```

Its devnet ProgramData complete-body hashes (`f26f4b53…`, `7122abc6…`) and its
recorded deployment slots (460,336,290 / 460,336,311) also still reproduce the
live devnet accounts exactly. The 2026-08-22 capture named the upgraded program
IDs and took the upgraded binaries; the cutover made that generation *canonical*
without changing those bytes.

Consequence: the receiver ABI identity
`c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64` pinned by
`docs/evidence/PYTH_SYNTHETIC_RELEASE_V1.md` is **the upgraded generation's**
receiver, not a legacy one. The local campaign has been executing the new
generation's real ABI all along. Nothing in the ABI moved.

## Measured fact 4 — per-feed addresses are PDAs of the push oracle

Seeds are `[shard_id: u16 little-endian, feed_id: [u8; 32]]`. Reproduced
independently in this lane for SOL/USD, feed id
`0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d`, read out
of the live account body:

| program | shard 0 PDA | bump |
| --- | --- | --- |
| upgraded `pyt2F414…` | `7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE` | 253 |
| legacy `pythWSns…` | `7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE` | 252 |

Both match the documented addresses. Every per-feed account address moved
because the push-oracle program id moved; the feed id itself did not change.

The resulting account is a 134-byte `PriceUpdateV2` (discriminator
`22f123639d7ef4cd`) **owned by the receiver** `rec2HH…`, not by the push
oracle. A consumer therefore authenticates the receiver program, and uses the
push-oracle id only for address derivation.

## Measured fact 5 — guardian set and receiver config

`GuardianSet[0]` PDA under the upgraded router, seeds `["GuardianSet", 0u32
big-endian]`, derived independently in this lane:
`CJHmJw4FuvLTUfPsYepyVCQkUR8qv1AtZbkwsS36hEcd`.

124 bytes, **index 0, five 20-byte keys, `expiration_time = 0`** on both
clusters. The five key bodies are identical across clusters:

```text
[0] 0x41534bb176e461a3fb30479400f210549ecce638
[1] 0x6502987b62f21cab7eb5ccd8f0173084b60d5b41
[2] 0x44a3e8f6a382412cf6bb90a3f8106e68977476c9
[3] 0xd9d7d4529577864352c9a6539a48238fcd447052
[4] 0x1663a5a822336ece48559b1dfb1e93a017a7dac3
```

**The accounts are nevertheless not byte-identical**: `creation_time` is
`1778014551` (2026-05-05T20:55:51Z) on `mainnet-beta` and `1778014447`
(2026-05-05T20:54:07Z) on `devnet`, a 104-second difference, giving different
account digests (`97d00e13…` vs `8f11fb97…`). Any adapter that binds a
*complete guardian-set account digest* is per cluster; only the key material is
shared. This is a correction: the guardian set is identical in **key material**,
not in **bytes**.

Bridge config PDA `GPhDjebMkciFeemuNGaUn5RsmxauQL7UZArqRDjCSZSW`, 24 bytes, IS
byte-identical across clusters (`e1fc7570…`): `guardian_set_index = 0`,
`guardian_set_expiry = 86400`, `fee = 0`.

Receiver `Config` PDA `H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye`, 370 bytes,
same on both clusters in every field that decides trust:

```text
num_data_sources              = 1
  chain                       = 26  (Pythnet)
  emitter                     = 6R92oFT6UiP2xWZBjTbwAkHzFCLy5BhWnNh6m83ndhZR
wormhole                      = HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL
single_update_fee_in_lamports = 0
minimum_signatures            = 3
```

…but its `governance_authority` **differs**: `6oXTdojyfDS8m5VtTaYB9xRCxpKGSvKJFndLUPV3V3wT`
on `mainnet-beta` (the same key as the mainnet upgrade authority) versus
`7g4Los4WMQnpxYiBJpU1HejBiM6xCk5RDFGCABhWE9M6` on `devnet`. So the complete
`Config` digest is per cluster too (`e3abebb9…` vs `23a7a19c…`), and a release
that binds a config digest cannot be shared.

**Quorum.** The trust root is a **3-of-5 Pyth-controlled multisig**: guardian
set cardinality 5, `minimum_signatures = 3`. Note that 3 is exactly the strict
majority of 5, so under this generation the receiver's own policy value and the
`PythReleaseV1` strict-majority rule (`count / 2 + 1`) agree. Under the previous
19-key set they did not (policy 5 versus strict majority 10), which is what the
now-superseded "quorum distinction" paragraph was about.

## Measured fact 6 — staleness is measured, and the tail is the point

`getSignaturesForAddress` over the SOL/USD account `7AviUf9nL…`, successful
transactions only, distinct block times, measured 2026-08-27:

| | `devnet` | `mainnet-beta` |
| --- | --- | --- |
| observation window | 2026-08-19T23:40:18Z .. 2026-08-27T02:05:44Z | 2026-08-26T13:54:02Z .. 2026-08-27T02:06:38Z |
| span | 170.42 h (7.10 d) | 12.21 h |
| inter-update gaps (n) | 1,997 | 7,017 |
| min / p50 / p90 / p99 | 1 / 313 / 321 / 325 s | 1 / 7 / 9 / 13 s |
| **max observed gap** | **4,784 s (1 h 19 m 44 s)** | **21 s** |
| mean | 307.2 s | 6.3 s |

The single largest devnet gap ran **2026-08-25T08:42:02Z → 2026-08-25T10:01:46Z**.
The next largest was 354 s, so this was one discrete outage, not drift.

*measured-profile, 2026-08-27.* This supersedes the earlier n = 12 single-window
figure of 310–318 s, which captured the central tendency correctly (p50 = 313 s
here) and **missed the tail entirely**. A devnet staleness bound of 400 s —
the value that earlier measurement suggested — would have refused every read
for 79 minutes on 2026-08-25. A bound is the maximum, not the median. Neither
figure is a *guarantee*: both remain measured-profile with a finite window, and
the lifting plan is continuous observation, not a longer one-off.

Push identity, sampled over the most recent 12 postings on each cluster:

| cluster | fee payers |
| --- | --- |
| `devnet` | `4p16wya1Vw2u9w22oah4yXQgySb6eWKRRLMsEXCreish` (12/12) |
| `mainnet-beta` | `9F6ApEtzkHVdZXzsury6BYmyEh4pahDBxuhNLaGC6saC` (9/12), `9uFDvq24JQ8SzbFuQ5opBDfNy2NoCxUJCHdSapBxLufF` (3/12) |

Correction to the earlier "sole fee payer" reading: devnet is a single pusher in
this sample, **mainnet is not** — at least two distinct payers crank mainnet.
That is a liveness difference in mainnet's favour and it is a measured sample of
12, not an enumeration of the pusher set.

## Prices observed

Both clusters carry the same real series, `expo = -8`,
`verification_level = Full`:

| cluster | price | publish time | posted slot |
| --- | --- | --- | --- |
| `mainnet-beta` | 101.723959 | 1787796292 (2026-08-27T02:04:52Z) | 442,000,650 |
| `devnet` | 101.534737 | 1787796021 (2026-08-27T02:00:21Z) | 488,623,872 |

## Files

Raw account bodies exactly as returned, base64-decoded, no normalization.
`*.programdata-header` is the first 45 bytes of the corresponding ProgramData
account (Loader V3 `tag = 3`, 8-byte slot, 1-byte authority option, 32-byte
authority); the ELF tail is deliberately not duplicated per cluster because it
is identical.

See `SHA256SUMS` for the digest of every file in this directory.
