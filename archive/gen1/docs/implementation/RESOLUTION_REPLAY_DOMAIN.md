# Resolution replay domain

Status: implemented in the offline reference and SBF adapter, and exercised in
the clean signed committed-bank baseline at source commit `c05fe84`.

## One market fact, not one owner's action

`Resolve` records a fact about a Market. It does not spend a Position and does
not belong to any owner's replay sequence. Its exact production account plane
is:

| index | role | access |
| ---: | --- | --- |
| 0 | fee payer | signer, read-only to Clutch |
| 1 | Market | writable |
| 2 | Hoard accounting | read-only |
| 3 | kernel aggregate | writable |
| 4 | SupplyLedger | writable |
| 5 | immutable Terms | read-only |
| 6 | canonical Resolution | writable |
| 7 | admitted Feed head | read-only |
| 8 | hostile evidence buffer | read-only |
| 9.. | complete canonical outcome-mint vector | read-only |

There is deliberately no Position, external-owner shadow, or owner Replay
account. `Request::sequence` has a resolution-specific meaning: it must equal
both immutable `Terms.repair_generation` and the sealed window generation. It
is not incremented.

`RedeemInternal` remains an owner transition. It retains Position and Replay,
requires the owner's signature, consumes that owner's next sequence, and reads
the Resolution record without re-folding evidence.

## Exact retry semantics

The canonical Resolution account is the persisted replay fact.

- First resolution: fold and bind the evidence, derive the payout, transition
  Market/kernel to resolved, and write the record.
- Exact retry: revalidate the same evidence-derived fact and return every byte
  unchanged. A later retry cannot rewrite the original cursor or recorded slot.
- Conflicting retry: refuse. A different generation, payout, window identity,
  terms binding, or inconsistent resolved kernel cannot replace the fact.
- A two-instruction transaction containing an accepted resolve followed by a
  conflicting resolve must roll the first instruction back atomically.

This makes retries safe without a wallet nonce and avoids the liveness bug in
which one owner's unrelated transition could make market resolution stale.

## Evidence boundary

Removing owner replay does not weaken the resolution gate. Before any first
resolution can write:

1. all runtime owners, exact keys, PDAs, bumps, aliases, and mutability roles
   are checked;
2. Terms bind the Market, feed, outcome count, payout/basis parameters, and
   repair generation;
3. the complete outcome-mint vector is admitted and reconciled with aggregate
   supply;
4. hostile observation bytes drive the accumulator through its real coverage,
   maturity, cursor, and sealing checks;
5. the sealed domain must equal the immutable domain in Terms; and
6. the requested categorical payout must equal the evidence derivation.

“Evidence-gated” is still narrower than “source-authenticated.” The source
adapter and archive plane must establish where accepted observations came from;
neither an opaque caller buffer nor this replay refactor supplies that fact.

## Native shaped-resolution boundary

The reference exports one native derived-basis seam:

```text
derive_payout_vector(ResolutionTerms, WindowResult) -> PayoutVectorBytes
```

Degrees 1–3 use that seam and `MarketState::resolve_with_vector`; they must not
be lowered to a preset lookup or an arbitrary interval midpoint. Degree 2/3
currently requires point evidence after edge handling, and non-point evidence
refuses as `R-15 NonPointEvidence`.

The current persisted reference/SBF `KernelAccount` and `ResolutionAccount`
still encode only a finite-preset payout index. Therefore the categorical
account path and the native vector seam remain separate until a reviewed
account revision can persist `BasisMode` and the resolved vector. This is a
named ABI residue, not permission to silently turn a spline into portfolio
sugar.

## Executable checks

- `solana-reference`: exact first resolve, exact idempotent retry, conflicting
  retry, wrong generation, aliasing, and source/window-domain refusals.
- SBF harness: the Resolve transaction omits owner Position/Replay, two exact
  resolves execute sequentially in one transaction, and a late conflicting
  sibling proves rollback expectations.
- Committed plan: Resolve leaves the actor replay at sequence 5; the subsequent
  two redemptions consume 5 and 6, and terminal `WithdrawCash` consumes 7.

The clean loopback runtime gate used ELF SHA-256
`70c33c1cd44b475745b0562a79d9107f1d2101cbf698ebd6c233ca167ebab2e6`.
All 22 signed transactions reached `confirmed`; the two declared refusals
preserved all 18 watched accounts; and step 15 resolved without consuming an
owner replay sequence. The terminal custody sequence paid the positionless
bearer 3 atoms, withdrew 61 atoms to the founder and 6 to the second owner, and
left the Hoard Token-2022 balance at zero.

The gate remains genesis-assisted by 11 program-owned prerequisites. In
particular, the resolution feed and evidence pages are injected rather than
produced by an authenticated public archive lifecycle. This is runtime evidence
for replay-domain separation, not source authentication, blank-bank operation,
or batch settlement. A narrow `SettlePage` slice is implemented separately in
`COUPLED_SETTLEMENT_V1.md`, but it is not part of this replay walk.
