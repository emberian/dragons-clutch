# Signed committed SBF walk

Status: **green for a signed, sequential, same-market local-validator slice;
still genesis-assisted and not an end-to-end venue lifecycle.**

The ordinary SBF differential harness calls `simulateTransaction` with
`sigVerify: false` and gives each conceptual step a separate prestate.  This
lane closes that evidence gap.  Fresh test-only keypairs sign the actual legacy
transaction messages, `solana-test-validator` commits them in order against one
market identity, and the runner reloads confirmed account bytes after every
step.

The dedicated runner in `programs/clutch-sbf/committed-harness` refuses every
non-loopback URL.  It accepts only explicitly supplied ephemeral keypairs whose
public keys occupy required message signer slots, fetches a fresh blockhash,
signs the serialized message, submits with `sendTransaction`, waits for
`confirmed` or `finalized`, and compares account data with the offline
expectation.  Expected refusals are submitted past preflight; the runner
snapshots every watched account before them and requires exact equality after
the bank records the declared custom error.

Run the gate from the repository root with:

```sh
programs/clutch-sbf/scripts/run_committed.sh
```

The script creates a private temporary directory for seven fresh keys: payer,
founding actor, independent bearer, and four ordinary Token-2022 account
identities.  Only their public keys enter fixture generation.  Cleanup unlinks
the key files, and neither the script nor runner reads Solana CLI wallet
configuration or contacts a non-loopback RPC.

## What the green slice proves

One market address passes through these 20 signed and confirmed transactions:

1. `CreateMarket` creates seven absent Clutch state PDAs, the absent Hoard
   Token-2022 account, and two absent outcome mints;
2. ordinary System and Token-2022 instructions create the actor's winning-Egg
   account;
3. the same public path creates an independent holder's Egg account;
4. the same public path creates that holder's collateral destination;
5. backed `Endow` debits the actor and credits the Hoard exactly, then credits
   founding-position cash and advances replay;
6. ordinary instructions create a collateral account for the second wallet;
7. ordinary `TransferChecked` funds it from the founder;
8. the second wallet's first backed `Endow` atomically creates its absent
   generation-zero Position and Replay PDAs;
9. `Split` reclassifies pooled backing into a complete set;
10. `Materialize` mints a winning Egg into the actor's actual token account;
11. `Dematerialize` burns part of that balance back into internal supply;
12. ordinary Token-2022 `TransferChecked` sends three winning Eggs to the
    positionless holder;
13. `Merge` cancels the remaining internal complete set;
14. `FeedAdvance` commits an observation into its injected feed head;
15. `Resolve` fixes the market outcome from the separate matured evidence
    fixture;
16. a late `Merge` refuses with `Custom(0x0016)` and changes no watched byte;
17. `RedeemInternal` redeems the founder's winning internal claims into cash;
18. `RedeemInternal` drains the losing internal claims without a payout;
19. two identical `RedeemExternal` instructions share one transaction: the
    first burns and pays, the duplicate sees an empty source and refuses with
    `Custom(0x001c)`, and transaction atomicity restores all 18 watched
    accounts; and
20. one `RedeemExternal` then burns the positionless holder's Egg and pays
    three collateral atoms directly from the Hoard.

The 70 endowed collateral atoms close exactly as 61 founder cash, 6 second
owner cash, and 3 collateral atoms paid to the bearer.  Split, Merge, and
internal redemption do not double-debit or double-pay the pooled Token-2022
custody account.  The two cash balances remain in the Hoard because no
`WithdrawCash` instruction exists.

## Exact evidence, 2026-08-18

The clean-source reproduction used:

- repository source commit
  `882204f14b0383f05d851b39a43ebb41d420ad17`;
- SBF ELF SHA-256
  `98cac8a1e48f629f15d0efbf6295b2c96df5296f6acf6cec28ca76491da4b391`;
- `solana-cli 4.0.2` (`src:549805f3`, `feat:6ff76655`, Agave client);
- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, and SBF Rust
  `1.89.0`; and
- a fresh loopback ledger under
  `/tmp/clutch-committed-final-20260818-e`.

There were 20 signed transactions, 18 unique watched accounts, 94 declared
post-state reload comparisons, 27 declared unchanged-account comparisons, and
two refusal transactions that each snapshotted and reloaded all 18 watched
accounts.  Thus the runner made 157 after-step byte-equality checks in total.
Every signature reached `confirmed` commitment:

| # | step | local signature |
|---:|---|---|
| 1 | create market | `5ow9Yoax8n3UbiWGacFkgofkD2m88AfJHVzDeJEwD29L8K77ynYgMSQx9C3v2Wgno5beBytHoJMtswuEe5kRdZ1B` |
| 2 | create actor Egg account | `2b95QGTu4rjYWbc5AX727KDLY9QwrLXFHUJQAkWjHfWAe8HorJDYTuJKeTTa9jVEdqgiGwQnDtqcfRGe76uTnHBR` |
| 3 | create holder Egg account | `528EYfTZT3P56WDWHXvSMzqvxCDVcBVSU3AzLZ6XawQvcAXHvxtWJ4qxKFjCUTNagG74p7W5xyhyWaumwH5ruVZm` |
| 4 | create holder collateral account | `H43ZpXegGvpUH1EzTL85nWEMwKGKns8wxNymG4QnMWZF7eKQAPDMNtqUhmBdENozXeKiiUD1dJQunXnCkJnGdc8` |
| 5 | backed founder Endow | `3uMN4DVYNQj55GGqzLLRFAmpAy8E9i4GTdyMAVxEDtG8YJnCFsMqUznCDDGHJ86ajmEPTqWQszSwGKjSgSsY6XvV` |
| 6 | create second-owner collateral account | `p8RM6iEM3XzQuY2c4Z4Zg7bjmLGnvGuYbUeVorqiK74UrWfqrvHx1DsV5PwNXKXHfkrqbhGTCG2MkhgubAWn4jV` |
| 7 | fund second owner | `AJckMnHNGqxMTxjsgQFiexUPx2woqcANKYU47uByEib4trFQdQwmktnuTE54g86HHLjM2QR19vHKqyVFwnvEPgf` |
| 8 | create second owner with Endow | `4w8sLi228Jyb849CLdaDd9sziM8HRNnnKe4hyMCKQ7opcGgzFAJ4eQP8R3dgGLKRekCHxuTGXQFjf7wJyV5d2KaY` |
| 9 | Split | `3ooadFVwGtQNzP1Khw99eRQPPaXs6Kt2VvcZLsHF8VaFB3n61X3oza29yAvZUTrGubJ5RVun73EcNDvczgNtuL9` |
| 10 | Materialize | `sqETiCWi8hHTPEaqBxraM5Vu2eejbGjHnAXVz4ey6y4zBrxiuVGZLergTZ6YYDbPu1UQF6JDq5nFA8n9qgp6Pg3` |
| 11 | Dematerialize | `4V1G7bDXCqYPK4pRgpgc97BwMJC3yEceVw6Zx36FybuuyrUqGPW4TrLU74Jc3reLAbkDqcobgPDbRQ5fHoYsYZ8Y` |
| 12 | transfer Egg to bearer | `3tyHeGyJehp6qNhqtkFe7aNpya5t96cAh43cpSa2PB1Rq7oQJDfuV41ikYzXbW9q76kc8bbWZJzgaYmL2kRNT4Nw` |
| 13 | Merge | `47iaJfRv4Jzy849WByP3B2xxWqfFdCNZV3Wxqw1EgebTFX6N5rrZScA5DvZy5HUBTL9QoFW1Qx7n9T5WuEA2grK1` |
| 14 | FeedAdvance | `msZ6ko7Yza8Nb6CKDdf1u2HJ5NUWaAWPA3z2EUmYX6wY4U8RQ8Er3DcwRxmKuksnRsFMKdJAEtx99tery9DTqcJ` |
| 15 | Resolve | `4ZRJP5xrNYi99TtrfcRzD5shHHprV8q25t7jAN5hMQ51LK9mDGV5ahBVEcsjRUYffg1xpVyjHnLamQ4p2fQVVpqK` |
| 16 | late Merge refusal | `4oUf54s9B4nwrNqeqce2vzAXFRXpcQHA7XsXPdJ4SFZPZ1Uoth6dJU99riEuwWvWpKd2hPboHiwPN5HpuusF5FnS` |
| 17 | winning internal redemption | `66cxYydJHXhgtpPjUPHkqd31D5F23BGYbsVjXjQ7QdWx9MNVB933m7wQF4EVzSoDuwuSiFb5kj3Ydu24EAbYZwRG` |
| 18 | losing internal redemption | `5RUXMckghSkk7hczUgop1enqo5e9N5xYwCqZmzmixcXZpnf2xzWyZFiUQhfumUEv118jqPUay1e8SmUQdwbcF7vM` |
| 19 | external-exit rollback | `LKf8R6A9UHPi1Phb5ZqvPRhdHwtogggmYSi3hRBFvSgY2maPCqYTReSbdmNKHPvk4aw8vKB4CrDYWmy8SH6MWbA` |
| 20 | external bearer redemption | `42P6GS43mbhkoFf16H2dzmTnLKWvUobgh2FYuwB8hy2V3W1p9jvHQ4GDKrJiYi3PhUd8VzoDtM7Yd3LxetHDp7kZ` |

These signatures name only the disposable local ledger; they are not cluster
transactions or deployment receipts.

The gate then changed one byte in the expected terminal Hoard image, started a
fresh validator, replayed the walk, and failed specifically with
`committed bytes differ`.  This establishes that the byte oracle can go red;
it does not merely observe transaction acceptance.

The SBF build completed but still emitted the repository's known 4,096-byte
stack-frame diagnostics for several layout/reference functions.  A green
local execution does not erase those diagnostics; SBF stack-safety review is a
separate release condition.

## Why this is still genesis-assisted

The validator injects 11 Clutch-owned prerequisites:

1. Realm;
2. Profile;
3. immutable Terms;
4. the matured resolution Feed head;
5. a separate Feed head exercised by `FeedAdvance`;
6. the 266-byte collateral-policy evidence account;
7. a resolution evidence buffer;
8. a redemption evidence buffer; and
9. three observation pages.

The split feed identities are deliberate test coverage, not a claim that the
walk advances the same source that resolves it.  Static injected buffers and
pages likewise do not prove authenticated source history or operatorless data
availability.

`CreateMarket` itself is no longer genesis-assisted: it creates the Market,
Hoard, founding Position, Kernel, founding Replay, Supply Ledger, Resolution,
Hoard token, and outcome mints from absent addresses.  A later owner's first
`Endow` creates that owner's absent Position and Replay.  Ordinary holder token
accounts are created through public System and Token-2022 instructions.

Removing the remaining assistance requires public, committed construction for
the prerequisites.  The exact missing protocol surface is:

- staged `BeginArtifact` / `WriteArtifact` / `SealArtifact` transport for the
  policy, grid, and oversized Terms bodies, followed by the existing
  `InitRealm`, `InitProfile`, `InitPriceGrid`, and `InitTerms` constructors;
- `InitFeed` plus authenticated archive/page creation and advancement, so
  resolution consumes state produced by the same admitted feed;
- `InitEpoch` and `FreezeEpoch`, then public order-page creation and the
  candidate/checkpoint/pot/receipt construction needed by clearing and
  settlement; and
- `WithdrawCash`, so authenticated owners can turn free Position cash into an
  exact Hoard-to-owner Token-2022 transfer.

`SettlePage` remains unimplemented, and this walk does not exercise an order
book, candidate selection, or receipt settlement.  Therefore the promoted
claim is **signed, committed, same-market execution from a genesis-assisted
local prestate**, not **blank-bank lifecycle**, **operatorless venue**, devnet
evidence, or mainnet evidence.
