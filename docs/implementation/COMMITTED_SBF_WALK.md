# Signed committed SBF walk

Status: **green at clean source commit `c05fe84`; signed, sequential, and
same-market on a loopback validator; still genesis-assisted and not an
end-to-end venue lifecycle.**

The ordinary differential harness uses non-committing simulations and gives
conceptual steps separate prestates. This gate closes that evidence gap. Fresh
test-only keypairs sign real legacy transactions; `solana-test-validator`
commits them in order against one market; and the runner reloads exact account
bytes after every accepted step. Expected refusals are submitted past
preflight, confirmed, and required to leave all watched accounts byte-identical.

Run from the repository root:

```sh
programs/clutch-sbf/scripts/run_committed.sh
```

The runner and script refuse non-loopback operation. They use seven ephemeral
keys, pass only their public keys into fixture generation, do not read Solana
CLI wallet configuration, and unlink the private key files on exit.

## Clean runtime evidence

The 2026-08-18 clean reproduction used:

- source commit `c05fe84`;
- SBF ELF SHA-256
  `70c33c1cd44b475745b0562a79d9107f1d2101cbf698ebd6c233ca167ebab2e6`;
- `solana-cli 4.0.2` (`src:549805f3`, `feat:6ff76655`, Agave client);
- `cargo-build-sbf 4.0.0`, platform-tools `v1.53`, and SBF Rust `1.89.0`;
- 22 signed, confirmed transactions, including two expected refusals;
- 18 unique watched accounts; and
- 11 program-owned genesis-assisted prerequisites.

The complete disposable signatures, generated plan, expected bytes, validator
logs, and negative-run log are in the local evidence directory
`/tmp/clutch-committed-root-20260818-v3`. They are local-validator receipts,
not cluster transactions or deployment evidence.

## Exact transaction sequence

| # | transaction | committed result |
| ---: | --- | --- |
| 1 | CreateMarket | creates the absent market state, Hoard token account, and outcome mints |
| 2 | create founder Egg account | accepted |
| 3 | create independent holder Egg account | accepted |
| 4 | create holder collateral account | accepted |
| 5 | backed founder Endow | deposits 64 collateral atoms and credits Position cash |
| 6 | create second-owner collateral account | accepted |
| 7 | fund second owner | ordinary Token-2022 transfer of 6 atoms |
| 8 | second-owner first Endow | creates that owner's absent Position and Replay and deposits 6 atoms |
| 9 | Split | locks founder cash into complete sets |
| 10 | Materialize | mints a real winning Egg |
| 11 | Dematerialize | burns part back into internal supply |
| 12 | transfer Egg to bearer | ordinary Token-2022 transfer to a holder with no Position or Replay |
| 13 | Merge | unlocks the remaining pre-resolution complete set |
| 14 | FeedAdvance | commits three contiguous pages in one signed transaction |
| 15 | Resolve | records the evidence-derived market-global resolution without owner replay |
| 16 | late Merge | refuses with `Custom(0x0016)` and changes no watched byte |
| 17 | winning RedeemInternal | converts winning claims into founder Position cash |
| 18 | losing RedeemInternal | burns losing claims for zero payout |
| 19 | duplicate RedeemExternal pair | refuses with `Custom(0x001c)`; transaction rollback restores all watched state |
| 20 | RedeemExternal | burns the bearer's Egg and pays 3 collateral atoms |
| 21 | founder WithdrawCash | transfers all 61 free atoms from pooled custody back to the founder |
| 22 | second-owner WithdrawCash | transfers the remaining 6 free atoms and drains the Hoard token account to zero |

The 70 deposited atoms therefore close exactly:

```text
70 deposited = 3 bearer payout + 61 founder withdrawal + 6 second-owner withdrawal
terminal Hoard locked backing = 0
terminal Hoard Token-2022 balance = 0
terminal founder cash = 0
terminal second-owner cash = 0
```

Split, Merge, internal redemption, and resolution move no pooled Token-2022
atoms. Only backed Endow, external bearer redemption, and WithdrawCash cross
the custody boundary.

## Falsifiability

After the green run, the script changes one byte in step 22's expected
`committed-market.hoard-token` image, starts a fresh validator, and replays the
sequence. The second run fails specifically with:

```text
committed-22-withdraw-second-owner-cash / committed-market.hoard-token:
committed bytes differ
```

This demonstrates that the byte oracle can go red; transaction acceptance
alone cannot make the gate pass.

The SBF build still emits the repository's known 4,096-byte frame diagnostics
for offline layout/reference functions. The production program built and ran,
but local execution does not discharge the separate stack-safety review.

## Why this is still genesis-assisted

The validator injects 11 program-owned prerequisites:

1. Realm;
2. Profile;
3. immutable Terms;
4. the matured resolution Feed head;
5. a separate Feed head exercised by `FeedAdvance`;
6. collateral-policy evidence;
7. a resolution evidence buffer;
8. a redemption evidence buffer; and
9. observation page zero;
10. observation page one; and
11. observation page two.

`CreateMarket` itself is no longer assisted: it creates its state PDAs, Hoard
token account, and outcome mints from absent addresses. A later owner's first
Endow creates that owner's absent Position and Replay. Ordinary holder token
accounts are also created through public System and Token-2022 instructions.

The split feed identities remain an explicit test limitation. Static injected
buffers and pages do not prove authenticated source history, operatorless data
availability, or that the feed advanced in step 14 is the exact source consumed
by step 15.

The remaining blank-bank surface includes public construction and lifecycle
for Realm/Profile/Terms/policy artifacts, authenticated feed archive and page
production, and the batch epoch/candidate/checkpoint/pot/receipt plane.
`SettlePage` remains unimplemented. This walk exercises no order-book clearing,
candidate selection, or receipt settlement.

The supported claim is therefore **signed, committed, same-market execution
from a genesis-assisted local prestate**. It is not a blank-bank lifecycle, an
operatorless venue, devnet evidence, or mainnet evidence.
