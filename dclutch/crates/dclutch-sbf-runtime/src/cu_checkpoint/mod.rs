#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! The one diagnostic compute-meter checkpoint, shared by every SBF program
//! that carries a profiling feature.
//!
//! ## Why this is a crate and not a fourth copy
//!
//! `dclutch-claims-sbf`'s `claims_cu_checkpoint!` says, in its own doc:
//!
//! > If a third program needs one, that is the moment to extract the pair, not
//! > before.
//!
//! `dclutch-custody-sbf` is the third program. This is that extraction, and it
//! is deliberately only the half that was genuinely identical:
//! [`cu_checkpoint`] and the reason it carries `#[inline(never)]`. Each program
//! keeps its OWN feature and its OWN macro, because the feature name is what a
//! build line names and the domain prefix is what a log reader greps -- and
//! `dclutch-trading-sbf`'s `hot_checkpoint` keeps its own body outright,
//! because it reports its bump allocator's outstanding heap at every mark and
//! the other two have no such allocator. Sharing that would mean either a
//! second parameter nobody passes or a heap figure that is always zero, and
//! both are worse than two spellings of two different instruments.
//!
//! ## The reading rule, stated once
//!
//! `sol_log_compute_units` reports the **transaction** meter, not a
//! per-invocation one. A child reached by CPI therefore continues its caller's
//! sequence, and the two are directly subtractable -- which is the whole reason
//! a child's phases can be compared with its parent's at all. Verified from
//! inside a run rather than assumed: the span from the Dealer accelerator's
//! return to Trading's `candidate` checkpoint was 6,339 CU in two independent
//! runs, which only arithmetic on one shared meter produces.
//!
//! ## What this instrument is for
//!
//! A child program reached by CPI is ONE number in its caller's log, and one
//! number cannot say whether it was spent on work only that program can do or
//! on re-deriving what its caller already authenticated. Twice now the answer
//! has been the second: the Claims SignedDelta child spent 149,107 CU of
//! 173,676 re-authenticating its caller and 662 applying the deltas it exists
//! to apply. See `docs/design/DEALER_PARTIAL_REMOVE_COMPUTE_2026_09_02.md`.

/// Log one phase label and the transaction meter, as two syscalls.
///
/// `#[inline(never)]` is not decoration. A route near the 4 KiB SBF frame limit
/// that expands two syscalls inline at a dozen phases spills enough frame to
/// overwrite its own caller's, which silently invalidates every number it
/// prints -- and the frameguard ratchet cannot catch it, because the ratchet
/// measures the build WITHOUT the profiling feature. The attribute is the whole
/// guard.
#[inline(never)]
pub fn cu_checkpoint(phase: &str) {
    solana_program::log::sol_log(phase);
    solana_program::log::sol_log_compute_units();
}
