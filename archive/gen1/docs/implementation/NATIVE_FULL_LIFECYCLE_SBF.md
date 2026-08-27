# Native degree-1--3 blank-bank lifecycle evidence

Status: implementation evidence, not deployment evidence. The campaign is local,
offline `solana-program-test` execution of the `cargo-build-sbf` ELF and the real
Token-2022 program. It does not use an RPC endpoint, a deployed program, a
production oracle, a wallet file, or external funds.

## Claim

`programs/clutch-sbf/svm-tests/tests/native_full_lifecycle.rs` joins the pieces
that earlier focused campaigns tested separately. For each B-spline degree 1,
2, and 3, it starts with no Market, Hoard, Position, Kernel, Replay,
SupplyLedger, Resolution, Hoard token account, or outcome mint. Ordinary local
keypairs then execute:

1. create a real extension-free Token-2022 collateral mint and two holder
   accounts, mint exactly 64 atoms, and permanently remove mint authority;
2. upload and seal the 266-byte collateral policy, PriceGrid, and native Terms
   through `BeginArtifact` / `WriteArtifact` / `SealArtifact`;
3. create Realm and frozen Profile through their public instructions;
4. call prefund-safe `CreateMarket`, which creates all seven program-state
   PDAs, the immutable-owner Hoard token account, and four outcome mints;
5. `Endow(64)`, `Split(64)`, and `Materialize` the minimal exact outcome-0 lot;
6. transfer that outcome token with an ordinary Token-2022 `TransferChecked`
   from the position owner to a second wallet, without a Clutch instruction;
7. resolve through the canonical SourceSpec and sealed SourceArchive to a
   persisted 319-byte v3 native Resolution vector;
8. redeem the transferred token as a positionless bearer and redeem every
   remaining internal claim at exact fractional lots;
9. withdraw the resulting position cash and reload a zero-collateral Hoard,
   zero Token-2022 Hoard balance, zero internal/external ledger, zero kernel
   supply, zero position cash, and zero outcome-mint supply.

This is a native basis lifecycle, not a search for a matching member of the
finite payout table. The active v2 Kernel stores `DerivedBasis`; the v3
Resolution is the only persisted owner of the resolved coefficient vector.

## Shapes and exact arithmetic

All three markets use denominator 64 and four one-hot primitive Eggs in the
immutable Terms artifact. Those primitive Eggs are the settlement liabilities;
the resolved B-spline coefficient vector selects their exact fractional
redemption rate.

| Degree | Resolved point | Point class | Persisted weights | External outcome-0 lot | External payout |
| ---: | ---: | --- | --- | ---: | ---: |
| 1 | 8 | closed left boundary | `[64, 0, 0, 0] / 64` | 1 | 1 |
| 2 | 4 | interior | `[16, 40, 8, 0] / 64` | 4 | 1 |
| 3 | 4 | interior | `[8, 24, 24, 8] / 64` | 8 | 1 |

The initial 64 complete sets make every final internal redemption integral.
For each degree, the external payout plus the four internal payouts is exactly
64 atoms. There is no floating point, midpoint rule, dust forgiveness, or
rounding at redemption.

## Adversarial joins

The same end-to-end campaign includes three rollback/refusal checks before the
successful terminal walk:

- it fault-injects `FinitePreset` into the active native Kernel, proves Resolve
  refuses the Terms/Kernel mode mismatch without changing Market, Kernel,
  SupplyLedger, or Resolution, and restores the exact original bytes;
- it presents byte-identical sealed archive data from a different program-owned
  address and proves canonical source-archive address binding refuses it without
  writes;
- after Token-2022 bearer transfer, it sends external redemption to a valid
  collateral token account whose balance is `u64::MAX`. The program reaches a
  late Token-2022 transfer overflow after the earlier burn CPI, and bank
  atomicity restores the source token, mint supply, Hoard custody/accounting,
  Kernel, ledger, Resolution, and destination byte-for-byte.

The hostile `u64::MAX` token account and Kernel-mode mutation are explicit
test-only genesis/fault-injection fixtures. They are not claimed to be reachable
through the protocol's public construction instructions.

## Source boundary

This campaign does **not** establish production source ingestion. The Feed,
canonical SourceSpec, and sealed SourceArchive are genesis-assisted local mock
accounts produced by the reviewed fixture parser/deployment authenticator. The
program currently has no live instruction that authenticates a production
provider deployment and initializes/appends/seals those accounts.

What the real Resolve instruction does establish is narrower: given those
program-owned canonical accounts, it authenticates the SourceSpec and sealed
archive receipt, binds the exact feed/window/cursor/domain, requires the legacy
buffer to be a byte-exact redundant projection, rejects a same-domain archive
at the wrong address, evaluates the native basis, and persists the v3 record.
The routed source-construction intents have real-bank evidence only under the
explicitly different `non-production-mock-source` ELF. The default registry
contains no production release and refuses source construction and Endow;
production provider/parser/archive creation remains a release STOP.

## Reproduction

From the repository root, after the coherent Kernel-v2 sources are committed:

```sh
programs/clutch-sbf/svm-tests/run_svm_tests.sh \
  --non-production-mock-source \
  blank_bank_native_degrees_one_through_three_reach_zero_hoard
```

The script builds the current program with `cargo-build-sbf`, prints the ELF
SHA-256, and then drives that ELF with the Rust 1.93.1 SVM workspace under a
single test thread. Exact artifact digest, size, compute-unit rows, test count,
and source commit are recorded below only after a clean attributed run.

## Attributed result

Run date: 2026-08-19. The program source boundary was the coherent immutable
Kernel-v2 commit
`3a81b38f62c172da588166dcc8b9a8f4d62cd6a3`. The exact campaign source later
committed as `86e0195c5540537db81f47d30fc25e8861866b10`; its file SHA-256 was
`f88cd3d2585cb96b29103d917af32c62a2c66bc3390429953c24946fd6acfb19`
both at execution and from `git show` after commit.

`cargo-build-sbf` produced:

- ELF SHA-256:
  `c8ff4ac7286004cb5d897cc92b05f7a9e386107d295cb1441adcd227e0b35138`
- ELF size: 809,824 bytes
- SVM result: 1 test passed, 0 failed; the test ran three independent banks,
  one for each degree, in 3.58 seconds

The bank-reported compute units include the explicit compute-budget
instruction in each measured transaction:

| Transition | d1, point 8 | d2, point 4 | d3, point 4 |
| --- | ---: | ---: | ---: |
| `CreateMarket` | 1,081,504 | 1,072,104 | 1,074,689 |
| `Endow(64)` | 294,841 | 294,841 | 294,841 |
| `Split(64)` | 276,413 | 276,413 | 276,413 |
| `Materialize` | 154,683 | 154,683 | 154,683 |
| native `Resolve` | 1,076,536 | 1,107,963 | 1,144,736 |
| bearer `RedeemExternal` | 773,047 | 762,847 | 763,552 |
| each of four `RedeemInternal` calls | 697,790 | 687,790 | 688,965 |
| terminal `WithdrawCash` | 294,703 | 294,703 | 294,703 |

The build also emitted backend stack diagnostics in dependency symbols. This
campaign records execution, state, and CU evidence; it does not supersede the
separate final-LTO survivor audit or turn the artifact into a deployability
claim.

## What this still does not prove

- no production provider adapter, deployment authenticator, parser, or live
  archive construction route is exercised;
- no deployment, upgrade authority, mainnet loader account, cluster liveness,
  transaction inclusion, or fee-market property is established;
- no generic theorem connects every future Rust source to the formal model;
- the scenario is one position plus a freely transferred bearer token, not an
  exhaustive multi-position/concurrent-order proof;
- zero Hoard balance is a terminal economic state reload, not account closure
  or rent reclamation.

Accordingly the defensible statement is: the named source commit's exact SBF
artifact completed this finite degree-1--3 lifecycle campaign under local SVM
with the stated mock-source and runtime assumptions. It is not a claim that a
production market is deployable or formally verified end to end.
