# Devnet Upgrade cohort — 2026-08-29

Status: complete at `2026-08-29 03:33 EDT`  
Cluster: Solana devnet only  
Decision: 0012; permanent Program IDs retained

This is the execution record for the first five-role Upgrade cohort of devnet
iteration 2. Registry and Rent were reauthenticated and carried forward. The
five mutable protocol roles kept their existing Program and ProgramData
addresses, retained upgrade authority
`4zrxtw5c4oPLpuTQbLYjRCXFUudvFCNNjzR9LqVQvEwP`, and advanced only through
Loader `Upgrade` instructions.

The admitted mixed release gate is SHA-256
`9713d52e370bc11adbb7e5a11624ef93aa045c9931a95bcdfbc1c8100cf53fcb`.
It binds candidate source `d3cf6bbf9ff51de2ede9ceb6e1319866c27b2136`
and candidate tree
`92888191a7144ec0623f6863fd33a14fd95ccd3b3f45d68d8fc3f32b6ba6ae61`.
Every role used a previously uploaded, full-byte-readback-authenticated Loader
Buffer. Each Upgrade was finalized before the next role began, then the
operator reauthenticated the exact Loader instruction, fee, slot, authority,
ProgramData rent, deployed payload, Buffer refund, and wallet bridge.

| role | Program | finalized slot | payload SHA-256 | transaction signature |
|---|---|---:|---|---|
| Custody | `34dhZkSUUhhFPL98KpWXaoG9aMs3EinZo5xN5epJEgGH` | 489,777,952 | `013e1ad7377a81702486293a6d3955122a475bfc5dbbedde8c7191348d3c1ac8` | `3K7gRRnVihnW8XfDZ5o7cXrPNKZk9RoUFweV4XqxueRHjLf8G6qWS4yCBZrUPsroUk3CWdoi1DAH6FHjZRQe8fyq` |
| Resolution | `2GHmxBawHTmwDRzqXuqdeC9A9Gj2HzucRd29wGpfgzmd` | 489,782,168 | `61c1548a822e4935ca10b27b88989582c1ba6c874fb808e7de3eaeacb57f6f6c` | `9q1VVusEQg8FMr7Aq3o1HnNp2JKcXfAPzyKaKU79K5XuV57eBZ6tCMvvn3XrvRU741LtZcZsYRiKAKTuXiFtiZ6` |
| Claims | `85hwTeQGabwFRs71Hafvngb1UmHb6dQoumBv3VV4epNN` | 489,782,885 | `0561eefabb64305e747887add0f5b9f4642984f5f95cce00feaf751fbb874719` | `214yLH9bhmnhbrLmXwFBeJzaaXW8NKmKSEquHSfGxAGCBYoGk7RtgExKrLzz2ooZsmSEZenB8WLz9uPtEETfCYes` |
| Trading | `5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk` | 489,783,425 | `93fbb6a4493ec4d51edc6e4d3c76d5fbb0083931ddfb72e3933bcfc97e5d58bf` | `3btfjfDFXRL2aDoQ4rv71aY2Cg7pnz8MudkpaDwRyLwtE4Xxu2vXUbbuJiScVpVCErhPSm2uYFA8P3JEochVqjEm` |
| Core | `HezRkcMGTZ5EY2LZk3i4uJbrAjUSDcamAw9B5v68z33N` | 489,783,964 | `0591bb399771a2fcf0df7fde56f9ffa97f916075242c398f64945af7073f1259` | `3CGbyDoMphUqg3Uqb58NfXsBwDEb6pyKBfXFQMfqu2HVKym6AsNJaDcMGPMXyTqYcKynywPuY55ee1oh2iS9GcXU` |

Claims' admitted raw ELF was 1,155,112 bytes with SHA-256
`dee036994ec1630c3bcee1349110553cd00b2c7ae475bc2bb2d81104ba8411b4`.
Its live image has 8,280 authenticated zero-padding bytes because the Loader's
minimum extension left a 1,163,392-byte payload capacity. The table records the
full live payload digest. The other four live payloads equal their raw ELF
bytes and hashes.

Each Loader Upgrade paid exactly 5,000 lamports: **25,000 lamports total**.
The five resident Buffers returned exactly **38,793,599,280 lamports** of rent.
The deployer moved from **3,883,781,946** to **42,677,356,226 lamports**, which
is exactly the five refunds less the five fees. Every ProgramData lamport
balance was unchanged by its Upgrade. Buffer-upload fees and earlier
ProgramData-extension rent are outside this Upgrade-only bridge and are
recorded separately on the coordination board.

The final key-free set audit completed all seven ordered roles with no next
role. Its report SHA-256 is
`f05224982abda0d2a9d14874979e141642a1f7295fe9cb7c2e73be847c56e3a3`;
its final set SHA-256 is
`7e15f244c27a877258fa7761f12373eed1db1140c92a47d251c752d67306a2ee`.
This is devnet execution evidence, not mainnet evidence.
