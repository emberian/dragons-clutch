# dclutch-economic-slice-kernel

This standalone successor removes two provisional physical restrictions from
the first Economic adapter:

- aggregate Market state no longer freezes two participant identities; and
- outcome vectors are borrowed runtime-width tails, not `[u64; 16]` arrays.

One Market account owns aggregate supply, native/materialized partition, Hoard,
phase, and revision. Each Position account owns one holder’s native and
materialized claims. A transition receives whichever source and destination
Positions actually participate. This supports claimant→wrapper issuance,
wrapper→current-holder redemption, and transferred wrapper receipts without a
parallel economic truth.

The crate is safe Rust, `no_std`, `no_alloc`, fixed-scalar, and total. Successful
execution mutates authenticated account bytes only after all decoding,
optimistic concurrency, phase, balance, overflow, and post-invariant checks
have succeeded. Solana ownership, PDA derivation, Registry receipt, CPI,
Token-2022, collateral custody, and transaction rollback remain adapter work.
