# SourcePlane V3 core promotion

## Decision

Promote the recurring-product model through a new pure core before changing the
live SBF dispatcher. The implementation is
`crates/clutch-source-plane-v3`. Its identity and codec vectors are frozen only
for the sound subset implemented now; policies without executable bytes refuse.

## Why the first model could not be copied directly

The host research compiler established the right ownership graph, but several
parts were not a production ABI:

- `source_plane_version >= 3` would have treated unknown future formats as
  compatible. V3 now requires exact schema, page, window, result, and capability
  values.
- The old `statistic_result_id` omitted result bytes. It is now named
  `StatisticKey`; a separate result content digest binds WindowSeal, status, and
  canonical payload.
- A transient `Sha256` value cannot resume across transactions. `WindowWorkV3`
  persists a domain-separated rolling root plus exact cursors and counts.
- Current V2 source sequences may repeat or jump and need publication time.
  V3 retains sequence/slot/time and does not invent `sequence + 1` semantics.
- Whole historical pages cannot safely be accepted from caller bytes in one
  transaction. `OpenRawPageV3` persists the real single-boundary ingestion
  path, then seals an immutable prefix.
- A mutable tail commitment would invalidate old overlapping windows. Every
  seal binds immutable page content, and later observations start another page.
- Checking only a Series' final start missed overflow in its final end/maturity.
  V3 validates final maturity before activation.
- The legacy failure-policy name did not prove a uniform payout. V3 joins the
  actual payout table and validates the selected vector explicitly.

## Identity graph

```text
reviewed SourcePlaneProgram ─┐
existing SourceSpec ─────────┼─> source-only Head -> OpenPage -> immutable RawPage chain
                             └─> WindowSpec/WindowKey

WindowKey + exact RawPages -> resumable WindowWork
WindowWork + maturity-page ClosureReceipt -> WindowSeal/WindowSealId

WindowKey + SummaryProgram + statistic -> predictable StatisticKey
StatisticKey + WindowSealId + status/payload -> StatisticResultId

SourcePlane + SourceSpec + SummaryProgram + Partition + PayoutTable
   -> reusable ProductTemplate

Template + Realm/Profile/Grid/Fee + Work/Liquidity refs + absolute start/cap
   -> compact InstanceDescriptor/InstanceId

finite SeriesPlan + exact segregated prepayment -> permissionless exact-next Instances
```

WindowKey contains neither Realm nor statistic, so terminal and drawdown can
reuse the same source/window evidence. StatisticKey is predictable before the
result exists. ResultId is not.

Repair generation is not decorative: it is checked across Head, OpenPage,
RawPage, WindowSpec, maturity closure, Template, and Instance lowering. A
different repaired generation produces different Window semantics and refuses
an exact-generation Template until that Template is intentionally superseded.

## Narrow typed refusals

- Bounded gaps refuse until a source adapter can create authenticated absence
  evidence rather than accepting caller-manufactured gaps.
- `FAIL_EXTENDED_WINDOW_02` refuses until Template bytes carry the complete
  immutable successor-window/extension semantics.
- This core does not lower into legacy Terms/Market accounts. The existing
  runtime's uniform-refund gap is documented rather than silently retrofitted.
- No adapter accepts caller-owned content bytes as proof of SourceSpec, page,
  seal, or result authenticity.

## Next adapter sequence

1. Add Solana account wrappers around the exact core bytes without placing PDA
   trivia in content preimages.
2. Implement verified SourceSpec projection and source-specific per-boundary
   admission into `OpenRawPageV3`.
3. Commit page seal and SourceHead advance atomically; test rollback after every
   failed mutation.
4. Add WindowWork and maturity-closure handlers, then independent SVM vectors.
5. Add closed terminal/drawdown evaluator constructors and result accounts.
6. Only then wire Template/Series/Instance construction and prepaid funding to
   live handlers.

This sequence intentionally leaves the current handlers unchanged until the
new core has an authenticated adapter and differential evidence.
