# Bounded public-RPC read log

Every read performed to produce this directory. All are read-only JSON-RPC
against the default public endpoints `https://api.mainnet-beta.solana.com` and
`https://api.devnet.solana.com`. **No writes, no transaction submission, no
signing, no keypairs, no airdrops, no API keys.** Window:
**2026-08-27T02:02:16Z – 02:09:54Z**, 80 calls total.

| method | mainnet-beta | devnet | purpose |
| --- | --- | --- | --- |
| `getGenesisHash` | 1 | 1 | cluster identity as an explicit bound fact |
| `getMultipleAccounts` | 3 | 3 | the three Program accounts; the three ProgramData headers; `Config` + `GuardianSet[0]` + bridge config + SOL/USD price |
| `getAccountInfo` | 15 | 15 | paged `dataSlice` fetch of the three complete ProgramData bodies (128 KiB pages) |
| `getBlockTime` | 1 | 1 | router deployment slot to wall clock |
| `getSignaturesForAddress` | 11 | 5 | SOL/USD posting cadence, paged at `limit = 1000` |
| `getTransaction` | 12 | 12 | fee payer of the most recent 12 postings |
| **total** | **43** | **37** | |

## Targets

Programs (`getMultipleAccounts`, `dataSlice(0,45)` then full paged reads):

```text
HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL   Wormhole receiver / router
rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp   Pyth Solana receiver
pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou   Pyth push oracle
```

ProgramData (paged `getAccountInfo` with `dataSlice`, `commitment = finalized`):

```text
9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x   656,005 B  (6 pages)
3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX   416,909 B  (5 pages)
9nxngQjxBGUZ3ajfqoTrpiuDBVfztXCQVDuWDAw52Gew   234,997 B  (3 pages)
```

…plus one zero-length probe per account to learn `space` before paging
(3 + 14 = 15 calls per cluster).

Program-owned accounts (`getMultipleAccounts`, full bodies):

```text
7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE   SOL/USD PriceUpdateV2, 134 B
H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye   receiver Config PDA, 370 B
CJHmJw4FuvLTUfPsYepyVCQkUR8qv1AtZbkwsS36hEcd   GuardianSet[0] PDA, 124 B
GPhDjebMkciFeemuNGaUn5RsmxauQL7UZArqRDjCSZSW   bridge config PDA, 24 B
```

Cadence sampling (`getSignaturesForAddress` on `7AviUf9nL…`): devnet 2 pages of
1000 plus 3 probe calls; mainnet-beta 8 pages of 1000 plus 3 probe calls. Fee
payers via `getTransaction` on the most recent 12 signatures per cluster,
`encoding = json`, `maxSupportedTransactionVersion = 0`.

## Derivations performed offline

No RPC involved. Reproduced in this lane from first principles
(`find_program_address` with an Ed25519 on-curve check):

```text
["config"]                      under rec2HH…  -> H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye  bump 255
["GuardianSet", 0u32 BE]        under HDw2E7…  -> CJHmJw4FuvLTUfPsYepyVCQkUR8qv1AtZbkwsS36hEcd  bump 255
["Bridge"]                      under HDw2E7…  -> GPhDjebMkciFeemuNGaUn5RsmxauQL7UZArqRDjCSZSW  bump 249
[0u16 LE, SOL/USD feed id]      under pyt2F4…  -> 7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE  bump 253
[0u16 LE, SOL/USD feed id]      under pythWS…  -> 7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE  bump 252
```

The SOL/USD feed id `0xef0d8b6fda2ceba41da15d4095d1da392a0d2f8ed0c6c7bc0f4cfac8c280b56d`
was read out of the live account body, not taken from documentation.
