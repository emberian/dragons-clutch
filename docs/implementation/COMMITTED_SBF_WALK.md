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
  `aadc0cd5ad562f2cc144a3a9cddbd2f1c87fd959`;
- SBF ELF SHA-256
  `98cac8a1e48f629f15d0efbf6295b2c96df5296f6acf6cec28ca76491da4b391`;
- `solana-cli 4.0.2` (`src:549805f3`, `feat:6ff76655`, Agave client);
- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, and SBF Rust
  `1.89.0`; and
- a fresh loopback ledger under
  `/tmp/clutch-committed-final-20260818-c`.

There were 20 signed transactions, 18 unique watched accounts, 94 declared
post-state reload comparisons, 27 declared unchanged-account comparisons, and
two refusal transactions that each snapshotted and reloaded all 18 watched
accounts.  Thus the runner made 157 after-step byte-equality checks in total.
Every signature reached `confirmed` commitment:

| # | step | local signature |
|---:|---|---|
| 1 | create market | `3g85ojqwbkmCQy3yu52mgLUKeeXPw9v35gq61c9TxNPmxFEcxQp19MLr5Wy85E7pUv1McQBj6KQGwXyfMK2psaRL` |
| 2 | create actor Egg account | `2f9RcYf7RZUakpkGLH7mbuQPcvb3TLvb2NhV71sVeMDMoGFNvfMFCWT6Lq27dpeiQ5goAeWHXs7JuiRPDA8MK5N2` |
| 3 | create holder Egg account | `4UG6jgKHsxorW2Q6nf74z7Q666NVdXZAAERknWQqrJG625zjfM5iWC2Nun1QrPZ4VbgKcLr9hY3BrsGjhxcNhSZN` |
| 4 | create holder collateral account | `3nPiyufrmPpK1cgTEmybGFjMZrowecHbShf1zUBsBQ7jDiLL1SqvJg8wXjLnfQieao4ztLeNpTKEuZ13b2JtD48S` |
| 5 | backed founder Endow | `46MiBAijw7wyU4ad6xk3XcifNmWC8tUPCdoT31sWK7VPqMP8sepjJssvfN3qysJwJdCcTxUvThtTP54GYvxyPrYG` |
| 6 | create second-owner collateral account | `4kqEWko8vmcsW6qRJ7wBfE8cZJKLmsjrn59wbrnwxaSP3S3x5SRGRUWCBcCk1Xzs1ouuvSYM9LxCUUrmnnjTzFRR` |
| 7 | fund second owner | `3zx8xmVhJ9B8dBUuBC68RzpW1jgeYBvdopEYxZ4WFbahZVTn3kp1Uo5T7bYGxvZPScPtFnidLqeb2Pb7DhZ5osjA` |
| 8 | create second owner with Endow | `4a28AWMth4a74QK8kxZ3mTGkcknfYk1b2tyHbarp5sWyGT2z9jPLkFqWrvAeQHsnaFT8wC5Saq2E7uz4oYQKmnRg` |
| 9 | Split | `3GuAbeBx2EWDrPVSiXshHFiTV7THuYmsPA7AfqFjBsDB1w7KEJakC763au5G2wTEV2smMs43PbUyGqmoSMkVEiEq` |
| 10 | Materialize | `4JPvawidw7BJti3GQ9WBoSbknex5ZZGM5zBV4FWWwU8Q3SvXXTpmbiMV8sUsczak3nFqxFvRRJu4rG3FM7YVh6Qx` |
| 11 | Dematerialize | `4c1yRtUD8k4tQ7Kp82wsKQXaucegx8i1J5QE8yZdsnpeQqdZx3aT7ps5daKBbDaVehn1bpjtD2auWjb3xR1y6rLL` |
| 12 | transfer Egg to bearer | `Jdwkj4N9bsGfubD3nMit2n8jvudFu8ariDgWuJfZMi5CYshpQwa9TDD2MzXSdnb9FWUR83t7vk8VmWWHEzSSt7o` |
| 13 | Merge | `3ZhTB2YNEJSefDvsYodfbB1oq9t8NJnkfFe63vjP7pWP9JLiHgvFaU4EK971Yiww9rgNjKNfuoVu5395JxFMujfY` |
| 14 | FeedAdvance | `2wZbKG6pJaZa211XAjsjwPJYALqkpvHUigiUcYHrxRSXppTaoZ4NKWN8uWSrmk7DhRrdWS5FXEFk7MPLtnG5sBBo` |
| 15 | Resolve | `49cSLkrD42BrGYFSJWWybrACHTrnkpHHWaiXEgwMZcrcqUhXM31wFuaLYCCMVcXejYYNKCXDLq5AcJEohosbmRNY` |
| 16 | late Merge refusal | `3KjTGh7ZXswvEoopK44zpLSsexi8yNM1YaFezSvKn4fEB4LaUPRJKosr3YGjyyzy5UT5gdgWFnCpuZz92nd21Aa1` |
| 17 | winning internal redemption | `4NC1LJuDfrNpQwgU2daSN14i5nCbzRGobN45fp61bwPpGCQhiGRG4rTzQxTeLJ6PBMXXk6LpEZE8BL5prthes74W` |
| 18 | losing internal redemption | `44kEGbfbkppPCfv1rfLKikwJzNuLKa6YidDep84j3k7bvpRKATSwC431RfKbbRYjXdfGSZLMb92pufr6zQ5L3arH` |
| 19 | external-exit rollback | `5LbVEZpcBiic4m9cvtFYNx2uak1XXThqv8rUBPjDsGrZiC4B8rwgrF2Cf2hHmWyqq98YRYniJq4jQX2hHK3aZtvG` |
| 20 | external bearer redemption | `zNBMcTkPhyFiYzEVhA6GH8Z8kjABjhumBahKestpt8NesAohE4jz477cJoyd2dDXfwGk6tQhwgnYrchipQJt9Aq` |

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
