# Structured-claim Solana adapter seam

Status: **isolated production-bound library and bank model; not wired into the
live dispatcher and not deployed** (2026-08-23).

This crate takes the exact algebra in `clutch-structured-claim` across the
Solana trust boundary without creating another backing or supply ledger. It is
`no_std`, allocation-free, safe Rust and fixed-capacity. The central dispatcher
is deliberately untouched.

## Semantic ownership

| fact | sole authority | adapter treatment |
| --- | --- | --- |
| wrapper supply | actual Token-2022 mint | authenticated `MintProjection`; never persisted in the descriptor |
| wrapper holder balance | actual Token-2022 account | authenticated pre/post projection |
| cash and residual-Egg backing | wrapper-owned base `PositionAccount` | reconstructed as `BackingVault` for each call |
| native internal/external supply | market `SupplyLedgerAccount` | exact pre/post closure against kernel total supply |
| Hoard collateral and payout liability | base Hoard plus Eggcrate/kernel state | reconstructed `MarketLedger`; never copied into wrapper state |
| payoff identity | live `NativePortfolioClaimV1` plus the exact core preimage | recomputed from Market, self-certifying Terms and primitive coefficients |
| wrapper fungibility | deployment-bound wrapper product digest | recomputed; descriptor PDA, mint PDA, vault-owner PDA and actor replay PDA verified |

The 384-byte descriptor contains only deployments, Market/Terms identity,
primitive coefficients, lifecycle, and three bumps. Complete-set floor,
residual vector, native-claim id, product id, mint address, backing totals and
supply are all derived.

## Planning and execution contract

`plan_route_into` performs all account, deployment, PDA, Token-2022 profile,
generation, replay, reservation, closure, cap and core-semantic checks before
the first CPI. It writes a `RoutePlan` into caller-owned `RouteScratch`:

1. an ordered prefix of at most three exact `CpiStep` values;
2. canonical empty step padding;
3. every expected final Position, SupplyLedger, Hoard/kernel aggregate,
   Token-2022, descriptor, and replay field.

The SBF dispatcher must allocate `RouteScratch` on its requested heap (5,256
bytes on the measured host ABI), not in a 4,096-byte SBF frame. It then invokes
the staged programs in order, records exact successful `CpiReceipt` values,
re-reads authoritative accounts, and calls both `reconcile_receipts` and
`reconcile_post_state`. A successful CPI is not accepted as evidence of the
right mutation by itself. Solana transaction rollback remains the atomicity
mechanism if any CPI, receipt, or post-state check fails.

| route | outer CPIs |
| --- | ---: |
| canonical wrap / unwind | 2 |
| full-vector wrap / unwind with positive floor | 3 |
| zero-floor full wrap / unwind | 2 |
| surplus compaction | 0–2 |
| direct exact-vector redemption | 2 |
| retirement | 0 |

Ordinary wrapper transfer and direct holder burn remain ordinary Token-2022
operations. Direct burn releases no backing. Permissionless compaction donates
all resulting cash and residual-Egg surplus, credits no caller, and is cap
gated.

## Reservation and replay rules

Buy reservations are the `Position.cash_atoms - reserved_cash_atoms`
decomposition, so only `free_cash_atoms()` may leave a holder Position. Sell
reservations already removed their Eggs from `Position.internal`, so the
visible vector is exactly the free vector and no order-page scan is needed.
The vault must have zero reserved cash. The wrapper program never exposes an
order-placement path signed by its vault-owner PDA; any hypothetical seller
reservation would remove backing and make the exact coverage check fail.

Each wrapper instruction consumes one descriptor/actor replay sequence. Each
base Position mutation carries exact source and/or vault generation and replay
sequences. Writable roles are pairwise nonaliased. Wrapper replay accounts must
not be closed and recreated while a descriptor is live.

## Remaining SBF seam

The live dispatcher still needs small adapters that:

- decode upgradeable-loader Program and ProgramData accounts into
  `RuntimeDeployments`, including the linked ProgramData identities and slots;
- decode the exact extension-free Token-2022 mint and ordinary holder account,
  rejecting every mint extension and every holder extension other than
  `ImmutableOwner`;
- authenticate the existing base Market, Terms, Hoard, kernel, SupplyLedger,
  Position and Replay PDAs/owners before building projections;
- expose the general base instructions staged here:
  `AtomicPositionAssetTransferV1`, beneficiary-free collateral donation,
  beneficiary-free internal-vector donation, and exact aggregate-vector
  redemption; and
- translate each `CpiStep` into frozen instruction bytes, call only the bound
  base or Token-2022 deployment, then re-decode and reconcile post-state.

Base Position/Replay PDA derivation stays with the base program, which must
authenticate it again inside every CPI. The wrapper crate intentionally does
not copy the base program's still-proposed seed module and create a second seed
authority.

## Rent estimate

Using the default historical rent arithmetic already used by the repository,
`890,880 + 6,960 × data_bytes` lamports, rather than a live-bank query:

| account | bytes | estimated rent-exempt lamports |
| --- | ---: | ---: |
| descriptor | 384 | 3,563,520 |
| extension-free mint | 82 | 1,461,600 |
| vault base Position | 220 | 2,422,080 |
| vault base Replay | 84 | 1,475,520 |
| one actor wrapper Replay | 80 | 1,447,680 |
| holder Token-2022 account with `ImmutableOwner` | 170 | 2,074,080 |

The product-level descriptor + mint + vault Position + base Replay estimate is
8,922,720 lamports. One actor replay raises it to 10,370,400 lamports. Existing
Market/Hoard/kernel/SupplyLedger accounts are not wrapper rent, and Hoard
principal or future fees fund none of these accounts. The local CLI was pointed
at an unfunded/no-Sysvar local endpoint during this pass, so these values are
arithmetic estimates, not bank observations.

See `SBF_EVIDENCE.md` for build and frame measurements and `tests/routes.rs`
for the atomic rollback model.
