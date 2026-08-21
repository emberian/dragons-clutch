# PartialFillLedger — implementation-ready design

Status: **DESIGN / APPROVED FOR BUILD** (2026-08-21, the Phase-S1 design
lane; every cited seam read, not recalled). Implementation follows the
fee-plumbing lane's merge (shared files in entitlement.rs/settlement.rs);
the design's own sequencing note about the V3 campaign is already
satisfied (it sealed before TerminalClosure and re-derived at cycle E).

## The thesis

Partial fills are already complete and frozen at the model plane — the
runtime's entitlement/consumption seams are the only refusal sites — so
this wave is NOT a policy fork or a new account family: it is a
reservation schema v2 (cumulative per-order consumption counters + the
fee-carry landing zone) plus the generalization of EntitleSlice/SettlePage
from "full one-to-one fills only" to "per-slice consumption with per-order
completion," under the unchanged frozen digest 7a9e...065b.

## Key decisions

1. **The ledger lives in ReservationAccount v2**, not a sidecar family
   (rejected: new rent + new close route + a two-account coherence
   invariant, no capability gained). v2 appends after remaining_internal
   (570 -> 610 bytes): entitled_units u64 (stamped once at first
   entitlement from the digest-verified feed, re-derived and required
   equal on every later touch), consumed_units u64 (monotone; completion
   == entitled_units flips CONSUMED and releases the remainder),
   fee_debited_atoms u64 and fee_carry_numerator u128 (RESERVED for the
   adopted composite base, validated == 0 in v2 semantics). v1 bytes
   refuse at the decoder (KernelAccount-v2 precedent; no live value, no
   migration owed). Receipts and FinalPot get NO fee fields, deliberately
   (G is owner-level; the destination is the treasury Position).
2. **No sibling profile.** GENERAL_CLEARING_POLICY_V1 already admits
   partial fills: PricePriorityMarginalProRata produces them,
   AssignCanonical was frozen to keep remainder books clearable,
   UniqueSliceReceipts is the per-slice receipt family. aon/rounding/
   portfolio_lots stay. Partial PORTFOLIO fills remain structurally
   impossible under StrictWholeOrder — a sibling-profile decision if ever
   wanted, not this wave.
3. **No new intents, no wire changes.** EntitleSlice (59) already carries
   slice_index; SettlePage's 7-account shape becomes the universal
   per-slice consumer; the atomic portfolio full-pair route is KEPT
   verbatim (moving it per-slice would narrow admission).

## The conservation identity, generalized

Per consumed receipt (outcome o, quantity q, price p, atoms a = q*p/S):
buyer.cash -= a, seller.cash += a; buyer reserved/remaining_cash -= a;
buyer.internal[o] += q; seller remaining_internal[o] -= q; both
consumed_units += q. Per-order completion (independently per end): buy end
releases the exact remaining_cash (price-improvement + unfilled refund in
one number); sell end returns the remaining_internal vector. Invariant
pinned at every transaction boundary:
initial = consumed-so-far + remaining + released(0 until completion),
per cash and per outcome; CONSUMED iff consumed_units == entitled_units
iff remaining zero. Whole-plane closure: per-owner cash deltas equal the
verified summary's debit/credit atoms exactly; seller remainders equal
summary.unfilled_refund_egg; H = L + P + S untouched. TerminalClosure
unchanged: tag 60 keeps fill == 0 on CLEARED; filled orders' remainders
return only through completion; consumption is permissionless so any
keeper can drive a partial book to CONSUMED.

## Exactness: the divisibility discipline, kept and named

Per-slice rule kept: q*p = 0 mod S before minting a receipt. The theorem
(new host property test): under TerminalOwnerFloor, when every slice is
exact, per-owner sums are multiples of S, both model conversions are
identities, rounding_pot == 0, and per-slice atom sums equal the model's
per-owner atoms — the per-slice path realizes the frozen rounding
boundary exactly. The honest residue (inexact slices with cancelling
fractions; nonzero pots) stays refused and re-files under the
VirtualPot/rounding family, with a deliberately-inexact fixture keeping
the refusal red-tested.

## Seam changes

EntitleSlice: route selection by a per-order-totaling coverage scan; four
shapes — (a) single-single partial/fragmented -> per-slice (new),
(b) exclusive portfolio pair -> atomic (unchanged), (c) mixed -> per-slice
(new), (d) non-exclusive portfolio -> per-slice (new). Per-slice route:
existing plane loads; slice validity (no virtual legs — VirtualPot
stands); divisibility; the per-order stamp (total == fill_at(rank), or
fill * sum(coefficients) for portfolios; first touch flips ACTIVE ->
ENTITLED and stamps entitled_units; later touches require equality —
forged stamps cannot survive recomputation); receipt at the canonical
(epoch, candidate, slice_index) PDA. Retires entitlement.rs:559, :567,
:717-726, :759, :798-801, :816-821; retains :708, :836/:1056, :337-342.
SettlePage: EntitledSliceConsumptionPlan {outcome, quantity,
consideration_atoms, buyer_completes, buyer_release_atoms,
seller_completes, seller_remainder} — prepare write-free/fallible-first
(receipt latch, bindings, ENTITLED + max_fee_atoms == 0 kept at this
seam, envelope bounds, consumed + q <= entitled both ends); apply
infallible per the identity above. The current full-fill case is the
one-slice special case of the generalized seam.

## Test gates

Gate 0 FIRST (merged red): a bank test driving a marginal-pro-rata book
to SELECTED and asserting EntitleSlice refuses NotYetImplemented — the
executable gap exhibit whose expectation the wave flips. Layout hostile
codec battery (counters, v1 refusal, 610 pin). The relation-side
exactness theorem + partial fixture family with verdict identity. Program
unit tests incl. out-of-order consumption-before-later-entitlement and
the forged-stamp refusal. Bank walks: fragmentation lifecycle (sell 10 vs
buys 6+4 at p=5000); partial lifecycle end-to-end (sell 12, fills 10,
remainder 2 returns) with the FULL TerminalClosure sweep joined to the
hostile terminal walk; the divisibility-strand sibling (red-tested);
mixed shape; hostile battery (replay, sibling-candidate receipt, page
close mid-consumption refuses/after-completion succeeds, prefunded PDAs).
Mutation falsifiers: dropped increment, skipped release, widened
invariant, skipped divisibility — each must go red. Last: the blocker
ledger flips to [VirtualPot], PartialFillLedger joins the retired list,
and the deferred PROPOSED->FROZEN doc-comment edits ride this wave's
reseal (ADOPTED item 1).

## Increments (lane-sized)

0 gap exhibit (SBF red-direction) · 1 reservation v2 codec + compile
sites (HOST) · 2 relation exactness theorem + fixtures (HOST) · 3
EntitleSlice generalization (HOST) · 4 consumption generalization (HOST,
parallel with 3 at unit level) · 5 bank campaigns both profiles (SBF) ·
6 ledger flip + docs + ONE reseal (cycle G). Whole-wave claim plane:
SBF-EXECUTED (bank), UNPROMOTED, fees forced zero, frozen digest
unchanged.

## Out-of-scope pins

VirtualPot; rounding-pot realization (re-filed there); fee rates and
machinery (v2 fields validated zero; the five-plus-one zero gates stand);
partial portfolio fills (sibling-profile decision); AON/minimum-fill
widening; promotion; cancellation/continuous claims; deployment/value.
