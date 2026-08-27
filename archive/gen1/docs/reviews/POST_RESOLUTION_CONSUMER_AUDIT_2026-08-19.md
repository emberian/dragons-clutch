# Post-resolution consumer audit — 2026-08-19

Verdict: **CLEAN — zero suspect findings.** Read-only sweep over every
resolution-fact consumer in the sealed runtime (`programs/clutch-sbf/program/
src/**` at the frozen ancestry) and the public adapter
(`programs/solana-reference`), evidencing the V1_BACKLOG "post-resolution
preset-index-zero consumer audit" item and the audit half of STOP 2. This is
a review result over named source, not a proof of absence.

## What was checked

Fifteen consumers enumerated and traced to their authentication chains:
Resolve entry and all three arms (legacy v2, native v3 point, occupation v4),
RedeemInternal, RedeemExternal/bearer, the Split family seam, supply
synchronization (`claim_truth`), WithdrawCash, ResolutionWork
Begin/Finalize/Abort, CreateMarket founding validation, the Direct/settlement
plane, and the adapter's `apply_with_evidence`/`redeem_from_evidence`/
`kernel_market`/`derive_payout` surfaces.

## Findings

1. **No path infers basis mode** from `resolved_payout == 0`, preset
   membership, or vector-equals-preset. Mode always arrives from the
   immutable KernelAccount v2 byte (v1 refuses at decode), cross-checked
   against digest-bound Terms degree. Where native paths require
   `resolved_payout == 0`, it is a downstream consistency refusal, never a
   branch selector.
2. **Cross-ABI presentation is impossible without refusal**: resolution
   account length is selected from Terms (degree + statistic) and enforced
   three deep — exact-length account roles, `resolution_binds`/
   `bound_native_resolution` re-refusal, and the per-version mode
   discriminator constants.
3. **Hostile prestate**: every resolution-fact read sits behind
   owner/non-executable/exact-length/PDA/stored-bump/market-terms-feed
   binding plus lifecycle/phase/kernel joins; the one non-address-bound
   account (the Resolve evidence buffer) is transport only, forced into
   per-record byte equality with the independently verified sealed archive.

## Recorded asymmetries (not violations)

1. RedeemInternal re-derives `expected_window_id`; RedeemExternal relies on
   the program-written record under its binding authority — different depth,
   both closed inductively.
2. `native_kernel_invariants` substitutes `PayoutVector::ZERO` on a record
   mode-constant mismatch instead of refusing in place; refusal is
   guaranteed but non-local (zero denominator refuses downstream).
3. `apply_resumable_occupation_candidate` trusts its caller's PDA checks on
   the Terms/Resolution slots; the indices are verified aligned today, and
   drift between the two files would be silent. A shared index constant
   would make this structural.
4. `SettleDirectV2` reads no market lifecycle: a preauthorized receipt could
   in principle be consumed after resolution (moves already-encumbered
   internal claims; closure preserved). Receipt creation is not publicly
   reachable; this is STOP-5 clearing-lifecycle territory.
