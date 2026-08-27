# R4 model boundary and SBF STOPs

This directory is an isolated research artifact. Nothing here is an Anchor or
SBF instruction, account definition, CPI implementation, authority grant,
migration, or claim that a live terminal walk is ready.

## What the host model authenticates abstractly

- immutable `(market, generation, terminal_authority, neutral_sink)` identity;
- monotonically increasing market and post-terminal CreditVault nonces;
- exact internal, registered external, and authoritative mint supply equality;
- one canonical credit identity per owner in one resolution domain;
- exact collateral, rights, rent, donation, refund, keeper, and vault equations;
- creation-time R4 mint version and close authority;
- close order and replay tombstone content;
- atomic refusal by copy/validate/commit transitions.

IDs passed to these methods stand for already authenticated principals and
accounts. The Rust value comparison is not signature verification.

## Required live adapter work

1. Freeze concrete PDA seeds, discriminators, owners, sizes, alignment,
   generation semantics, and rent-exempt principal for Policy, SupplyLedger,
   CreditRoot, CreditVault, claimant Credit, rent records, keeper escrow, and
   LifecycleReceipt.
2. Define canonical Token-2022 program/mint/account identities and the permitted
   extension set. New mints must carry the terminal MintCloseAuthority at
   creation. A legacy mint without it remains `PERMANENT_INFRA` or
   `UNCLASSIFIED_STOP`; it is never silently repaired.
3. Replace the bounded bearer array with a persistent aggregate supply ledger.
   For materialization, redemption, and third-party burn reconciliation,
   authenticate exact pre/post mint supply and token-account amount after CPI.
   Refuse `A_i > E_i`, stale observation sequence, an unregistered delta, or a
   partial vector that cannot prove aggregate equality. Multiple burns observed
   between reconciliations require one complete canonical per-outcome vector.
4. Bind external redemption credit to the source token-account owner, or to an
   explicit owner-signed redirect. A delegate or generic burn authority is not
   the claimant. Cross-owner credit transfer requires source authorization and
   destination acceptance in the same credit domain.
5. Make burn, collateral transfer, credit update, SupplyLedger update, and
   observed Token-2022 deltas one atomic transaction. Specify rollback/error
   behavior for every CPI ordering and perform real-bank adversarial tests.
6. Capitalize CreditVault only with segregated collateral transferred from
   Hoard. Keeper, fee, rent, future volume, and unsolicited lamports are not
   credit backing. Freeze token-vault authority and exact balance mirrors.
   Reconcile unsolicited collateral tokens into a separate donation compartment
   and route them only to the neutral sink; they are neither backing nor slack.
7. Implement a program-controlled lamport close router. It must split recorded
   principal to the immutable refund owner and excess donations to the neutral
   sink using observed pre/post balances. Direct close-to-refund aliases are
   invalid when donations may exist.
8. Prepay keeper escrow and every refundable or permanent account at creation
   or explicit owner-funded initialization. Predictable-PDA prefunding is a
   donation; it does not reduce the named payer's principal obligation.
9. Create, fund, and atomically reserve Replay/LifecycleReceipt before the market
   identity is consumable. It must self-authenticate without reading closed Market bytes,
   commit the final ledger snapshot and CreditRoot/Vault reference, and reject
   recreation forever.
10. Specify staged close batches, account locks, compute limits, and terminal
    authority expiry. The host model's single transition is not evidence that
    all Token-2022 closes and lamport routes fit one SBF transaction.
11. Pin source and artifact hashes, generated ABI fixtures, feature set, and
    deploy provenance. This model intentionally has none.

## Irreducible STOP

For `D>1` and distinct owners holding positive residues `a,b` with `a+b=D`, no
integer payout vector gives both owners their exact `a/D` and `b/D`. Therefore
permissionless tombstone-only closure is impossible under all of:

- arbitrary raw bearer quantities;
- indivisible native collateral atoms;
- no external subsidy;
- no confiscation or forced owner merge.

R4 stops with persistent CreditRoot/CreditVault/credit accounts. A future
profile may instead choose exact-lot bearer encoding, finer collateral, an
explicit separately funded rounding policy, or claimant-authorized forfeiture.
None is inferred from donations, burn slack, rent, fees, keeper funds, or hoped
future activity.

## Codec boundary

`src/wire.rs` rejects wrong length, magic, version, padding, boolean, enum, and
shape values. It does not define Solana account ownership, PDA derivation,
Token-2022 TLV parsing, hashing, signature checks, upgrade compatibility, or a
released ABI. Those remain STOPs until a real adapter and fixtures land.
