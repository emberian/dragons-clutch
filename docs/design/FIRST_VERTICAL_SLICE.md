# First vertical slice

> **Supersession notice (2026-08-27):** this is the most orphaned document in
> the tree — no status header, no date, no evidence pointer, and none of the
> nine acceptance conditions below carries a status claim. The target
> lifecycle (compile -> create -> split -> authenticate Pyth -> resolve ->
> redeem -> retain terminal root) reaches step 3 in the live tree; treat
> everything below as a historical target statement, not current status. This
> banner is the only edit. See `WAVE.md` for where the lifecycle actually
> stands and `docs/ASPIRATION_LEDGER.md` (M-21) for the full staleness
> accounting.

The first end-to-end slice proves the smallest useful dClutch market without
introducing a venue, mock provider, or optional wrapper.

## Target lifecycle

```text
compile categorical terms
-> create compact Market Core
-> deposit collateral and split complete sets
-> authenticate a release-bound Pyth observation through the real provider ABI
-> produce the exact Product resolution receipt
-> resolve
-> redeem both winning and losing claims
-> retain the compact terminal root and reclaim every temporary child
```

The Pyth leg may initially use a synthetic observation only when the local
validator executes the captured real receiver/router ABI and cryptographic
checks. It remains labeled synthetic local evidence and cannot select a release
profile.

## Acceptance conditions

- The kernel is total, allocation-free, float-free, and explicit about every
  arithmetic refusal.
- The core has no General, Dealer, Fractional, Structured, or bearer-mint
  dependency.
- Exact collateral and supply ownership is visible after every transition.
- Every required rent and work debit comes from a present named balance.
- The operator derives all addresses and semantic IDs from a finalized snapshot.
- Wrong provider release, feed, units, confidence, staleness, window, result,
  owner, generation, and replay sequence refuse before mutation.
- A failed multi-program transaction rolls back the provider update and dClutch
  state atomically where the selected integration requires adjacency.
- The terminal root prevents recreation of the same Market generation.

Direct signed-intent trading is the second slice. It must reuse this core rather
than add a second liability owner.
