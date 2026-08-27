# SBF frame budget — findings and plan

Status: **ASSESSMENT + ORDERED PLAN.** Every size below was measured (probe
crate printing `size_of`, build-log diffs, `nm` residency), not derived by
hand — hand-derived layout was wrong twice during the analysis. Full detail
in the lane report; this records what decisions follow.

## 1. The headline

**The frame blocker for the general batch relation is already solved in this
repository, and the solution is unconsumed.**
`crates/clutch-batch/src/relation_v1_stream.rs` (2,378 lines, 17 tests)
reproduces the batch verdict one order at a time: `push_order` frame
**1,280 bytes**, `push_slice` 64, `end_pass` 832 — zero frame diagnostics,
measured in the same build where the monolithic `verify_inner` reproduces
its 39,104-byte diagnostic as a positive control. It handles Portfolio
orders. Equivalence is gated by 19,520 verdict comparisons over an
enumerated 2,592-book domain. The companion on-chain checkpoint codecs exist
too (`clearing.rs`: `ClearWorkAccount` 48,750 B / 0.3402 SOL rent,
`CandidateFeedAccount` 6,266 B / 0.0445 SOL) and say of themselves that
nothing consumes them yet.

Portfolio clearing is therefore **join-blocked, not frame-blocked**: the
eight lifecycle joins listed at `orders_batch.rs:271-290` (reservation-set
commitment, tombstone counting, policy preimage, id domain mapping, a stable
`ClearWorkV1` codec — `repr(C)`+`Pod` or a serializer, staged creation of a
48,750-byte account past the 10,240 per-CPI allocation cap, candidate-set
closure/selection transitions, entitlement objects) plus the multi-pass
compute envelope — which the SHA-syscall win just made affordable.

## 2. What is arithmetically impossible, so nobody retries it

- `verify_inner` / `canonical_candidate` as monolithic in-frame functions:
  the stage sequence keeps `NormalizedBookV1` (11,912 B) and
  `ParticipationV1` (16,384 B) simultaneously live — 28 KB of irreducibly
  co-live state, 7x the frame. Frame as a function of bounds is
  `~32NK + 67N + 96K + 728`; at the committed K=16 the book fits only at
  N<=3. Streaming or off-frame storage are the only escapes; streaming is
  written.
- `propose_best_valid` on-chain: it enumerates the price simplex
  (`10,000^15` coordinates at K=16). It is a host-side solver by
  construction; its frame is a red herring. The vocabulary already says so:
  "best valid *submitted* candidate."
- The V5 participation matrix is a passes-versus-memory trade, recorded so
  the alternative survives: `K` passes with a 1 KiB working set, or 1 pass
  with the 16 KiB checkpoint. Streaming chose the checkpoint; the K-pass
  form is what to want if rent ever dominates. At post-SHA compute (~180k
  CU/route) both are affordable; neither was this morning.

## 3. The inversion nobody expected

**Binary size is gated by the frame ceiling, not the reverse.**
`opt-level="z"` shrinks the ELF 1,420,608 → 1,090,872 (−23%, worth ~2.3 SOL
of deploy rent) and fails tests — because exactly **ten reachable
functions** go 64–896 bytes over the 4,096 line at `z` (place_order 4,992,
settle_page 4,608, recorded_redeem 4,736, resolve_global 4,352, both legacy/
native resolution arms, two direct-selection preparers,
commit_observed_supplies, prepare_direct_v4_economics). The failure
signatures match causally: padding-canonicality refusals are the signature
of a corrupted stack buffer. Meanwhile ~12 resident handlers sit at exactly
4,096 — the recorded "one more dispatcher arm" watch item.

## 4. The ordered plan

- **Tier 0 (dispatched):** bring the ten `opt-z` overflowers under 4,096
  using the codebase's own mature idiom (307 existing `#[inline(never)]`
  annotations; `Box`-one-decode-per-helper per
  `direct_selection_v3/common.rs:139`). Payoff: headroom on the
  exactly-4,096 handlers now, and the *option* of a 23% smaller ELF. The
  committed profile keeps `opt-level` 3: at `z` some rows cost +60–220% CU,
  which is affordable post-SHA but is a per-deployment economics choice, not
  a default.
- **Tier 1 (dispatched):** `portfolio_settlement::{prepare,apply}_full_pair`
  to `&mut` out-params (4,224 → ~1,900; 6,784 → ~400). Zero callers, zero
  regression risk, and it removes the frame excuse from the
  coefficient-vector path.
- **Tier 2 (next-cycle, the real unblock):** consume `relation_v1_stream` +
  `clearing.rs`: discharge the ClearWork codec soundness obligation, stage
  the account creation past the 10,240 cap (ResolutionWork shape), build the
  page→order projection, then the eight joins. This is the actual road to
  portfolio clearing and none of it is stack work.
- **Tier 3 (rides along when convenient):** host-only hygiene — the
  `participation_from_fills` in-place zero (24,704 → ~200), `normalize_into`,
  the reference crate's one-character `&decoded` fix worth 2,960 B, and
  `TermsAccount::decode_into` adoption. Removes real host-build UB hazards;
  changes nothing in the deployed ELF.
- **Never:** "fix" `propose_best_valid`.

## 5. Runtime facts pinned during the analysis

Frame 4,096; call depth 64 (256 KB total stack); CPI nesting 5; default
heap 32,768 with `request_heap_frame` to 262,144 at 8 CU per extra 32 KiB;
CPI invoke 1,000 CU (0.5% of a post-SHA route — noise); per-CPI allocation
cap 10,240 bytes; rent `(128+bytes)*6960`. A CPI callee gets a fresh 4 KiB
frame, never a larger one — program splitting is deployment economics, not
a frame tool. Heap is per-instruction and non-persistent — right for large
locals, wrong for the checkpoint. `clutch-batch` is inside the no_alloc
kernel boundary as AGENTS.md draws it; the program crate is outside it and
already uses boxed decodes deliberately.
