//! The W1 compute-unit quote table, embedded with its source.
//!
//! Every limit below is the `selected_limit_cu` of a named route in the sealed
//! liveness policy profile:
//! `research/liveness-policy-profile/evidence.json`
//! → `projection.general_clearing_walk.w1.routes.<route>.selected_limit_cu`,
//! measured against the ELF whose `artifact_sha256` the same file records.
//! The route key is carried on every quote so a keeper log line names the row
//! it is spending against.
//!
//! The selection rule is `admission_math.quote_route`:
//! `selected = round_up(ceil(measured * 5 / 4), 10_000)`, refused when it
//! exceeds the 1,400,000-CU transaction ceiling.  [`selected_limit`]
//! reimplements it exactly so a keeper that meets a shape the profile has not
//! measured can still quote itself honestly, and say so.
//!
//! **The `TerminalClosure` family (tags 60-67) has no W1 row.** The profile
//! records `per_route_cu: "NOT_LABELLED_BY_SUITE_NO_ROW_DERIVED"` and
//! `per_route_cu_rows_derived: false` for it.  Those routes take
//! [`UNQUOTED_CLOSE_LIMIT`] and are logged `quote=UNQUOTED`, never as if a row
//! backed them.

/// Headroom numerator of the profile's admission policy.
pub const HEADROOM_NUMERATOR: u64 = 5;
/// Headroom denominator of the profile's admission policy.
pub const HEADROOM_DENOMINATOR: u64 = 4;
/// The profile's CU rounding quantum (blessed down from 50,000).
pub const ROUNDING_QUANTUM_CU: u64 = 10_000;
/// The runtime's per-transaction compute ceiling.
pub const TRANSACTION_CEILING_CU: u32 = 1_400_000;
/// The raw admission bound the 5/4 headroom implies under that ceiling.
pub const MAX_ADMITTED_RAW_CU: u64 = 1_120_000;

/// The limit the keeper spends on a route with no W1 row.
///
/// Chosen, not measured: it is above the largest close observed anywhere in
/// this repository's terminal walks and far below the ceiling, so an unquoted
/// close neither starves nor pretends to be priced.  Every use is logged
/// `quote=UNQUOTED` and the observed CU is reported so a row can be derived.
pub const UNQUOTED_CLOSE_LIMIT: u32 = 400_000;

/// Compute allowance for one funding-ledger sibling, per sibling.
///
/// **Stated, not measured.**  Every W1 creation row was captured against a
/// transaction that creates its machinery *without* the optional
/// `GeneralFundingLedgerV1` sibling.  A keeper must create that sibling --
/// an account made without one records no payer and can never be closed --
/// so it is running a shape the profile has not priced.  Rather than spend a
/// row it does not fit, such a route is reported UNQUOTED at the row's
/// measurement plus this allowance per ledger, and the observed CU is logged
/// so the missing row can be derived from a real bank.
///
/// The gap is not hypothetical: at the row's own selected limit of 60,000 the
/// ledgered `InitEpoch` exhausted its meter at 59,850 on a real validator.
pub const LEDGER_ALLOWANCE_CU: u32 = 60_000;

/// One quoted route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Quote {
    /// The profile's route key, for the log line.
    pub route: &'static str,
    /// `measured_cu` of that row, or `None` for a route with no row at all.
    pub measured_cu: Option<u32>,
    /// The compute-unit limit the keeper requests.
    pub limit_cu: u32,
    /// Funding-ledger siblings this shape creates that the row did not.
    pub ledgers: u32,
}

impl Quote {
    /// A route the profile has no row for.
    #[must_use]
    pub const fn unquoted(route: &'static str, limit_cu: u32) -> Self {
        Self {
            route,
            measured_cu: None,
            limit_cu,
            ledgers: 0,
        }
    }

    /// Whether a W1 row backs this quote *for the shape being sent*.
    ///
    /// A row plus an unmeasured ledger allowance is not a quoted route: the
    /// number it produces is partly stated, and saying otherwise would put a
    /// seal on an allowance nobody measured.
    #[must_use]
    pub const fn quoted(&self) -> bool {
        self.measured_cu.is_some() && self.ledgers == 0
    }
}

/// The same route, with `ledgers` funding-ledger siblings added.
///
/// # Panics
/// Panics when the allowance pushes the route past the transaction ceiling,
/// which would be a shape no keeper may send at all.
#[must_use]
pub fn ledgered(base: Quote, ledgers: u32) -> Quote {
    let measured = base.measured_cu.unwrap_or(base.limit_cu);
    let padded = u64::from(measured) + u64::from(LEDGER_ALLOWANCE_CU) * u64::from(ledgers);
    Quote {
        route: base.route,
        measured_cu: base.measured_cu,
        limit_cu: selected_limit(padded)
            .unwrap_or_else(|| panic!("{} with {ledgers} ledger(s) exceeds the ceiling", base.route)),
        ledgers,
    }
}

/// Reimplementation of `admission_math.quote_route`'s limit selection.
///
/// Returns `None` when the 5/4-headroom requirement exceeds the transaction
/// ceiling, which is the profile's `STOP_HEADROOM` — a route with no admitted
/// limit, not a route that gets the ceiling anyway.
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "the ceiling branch above bounds the value below u32::MAX"
)]
pub const fn selected_limit(measured_cu: u64) -> Option<u32> {
    let required = measured_cu
        .saturating_mul(HEADROOM_NUMERATOR)
        .div_ceil(HEADROOM_DENOMINATOR);
    let selected = required.div_ceil(ROUNDING_QUANTUM_CU) * ROUNDING_QUANTUM_CU;
    if selected > TRANSACTION_CEILING_CU as u64 {
        return None;
    }
    // Lossless: the branch above bounds `selected` by the ceiling.
    Some(selected as u32)
}

/// Build a quote from a W1 row's two numbers.
const fn row(route: &'static str, measured_cu: u32, limit_cu: u32) -> Quote {
    Quote {
        route,
        measured_cu: Some(measured_cu),
        limit_cu,
        ledgers: 0,
    }
}

// --- general_epoch -------------------------------------------------------

/// `init_epoch` -- measured on the UNLEDGERED shape; see [`ledgered`].
pub const INIT_EPOCH: Quote = row("init_epoch", 42_743, 60_000);
/// `InitOrderPage` has no W1 row in the profile at all.
pub const INIT_ORDER_PAGE: Quote = Quote::unquoted("general_epoch.init_order_page", 150_000);

/// `freeze_epoch_1page_4orders`.
pub const FREEZE_EPOCH_1PAGE: Quote = row("freeze_epoch_1page_4orders", 233_554, 300_000);
/// `freeze_epoch_2pages_17orders`.
pub const FREEZE_EPOCH_2PAGES: Quote = row("freeze_epoch_2pages_17orders", 477_995, 600_000);
/// `freeze_epoch_3pages_40orders` — the profile's worst measured route.
pub const FREEZE_EPOCH_3PAGES: Quote = row("freeze_epoch_3pages_40orders", 717_815, 900_000);

/// The freeze quote for a page set of `pages` pages.
///
/// Interpolating between measured shapes would be inventing a row, so a page
/// count past the largest measured shape takes the largest measured row and
/// is reported as an extrapolation by the caller.
#[must_use]
pub const fn freeze_epoch(pages: u16) -> Quote {
    match pages {
        0 | 1 => FREEZE_EPOCH_1PAGE,
        2 => FREEZE_EPOCH_2PAGES,
        _ => FREEZE_EPOCH_3PAGES,
    }
}

// --- clear_walk / clear-work creation ------------------------------------

/// `init_clear_work_exhibit_book` — the one measured creation row; it prices
/// the whole five-instruction staged-growth transaction.
pub const INIT_CLEAR_WORK: Quote = row("init_clear_work_exhibit_book", 70_613, 90_000);
/// `advance_clear_work_pass1_small_book`.
pub const ADVANCE_PASS1_SMALL: Quote = row("advance_clear_work_pass1_small_book", 288_078, 370_000);
/// `advance_clear_work_pass2_small_book`.
pub const ADVANCE_PASS2_SMALL: Quote = row("advance_clear_work_pass2_small_book", 286_935, 360_000);
/// `advance_clear_work_pass1_forty_order`.
pub const ADVANCE_PASS1_FORTY: Quote = row("advance_clear_work_pass1_forty_order", 393_207, 500_000);
/// `advance_clear_work_pass2_forty_order`.
pub const ADVANCE_PASS2_FORTY: Quote = row("advance_clear_work_pass2_forty_order", 305_527, 390_000);
/// `advance_clear_slices`.
pub const ADVANCE_SLICES: Quote = row("advance_clear_slices", 173_859, 220_000);
/// `complete_clear_work_selection` — the larger of the two complete rows.
pub const COMPLETE_CLEAR_WORK: Quote = row("complete_clear_work_selection", 126_213, 160_000);
/// The measured surcharge of the 256 KiB `RequestHeapFrame` rider.
pub const HEAP_FRAME_SURCHARGE_CU: u32 = 150;

/// The advance quote for a pass over `live_orders` orders.
///
/// A book at or under the measured small book takes the small-book row; a
/// larger one takes the forty-order row, which is the profile's measured
/// worst case for this route.
#[must_use]
pub const fn advance_clear_work(pass: u8, live_orders: u16) -> Quote {
    match (pass, live_orders) {
        (1, 0..=8) => ADVANCE_PASS1_SMALL,
        (1, _) => ADVANCE_PASS1_FORTY,
        (_, 0..=8) => ADVANCE_PASS2_SMALL,
        (_, _) => ADVANCE_PASS2_FORTY,
    }
}

// --- candidate_selection -------------------------------------------------

/// `finalize_selection_3_retained_winner` — the largest of the three shapes.
pub const FINALIZE_SELECTION: Quote = row("finalize_selection_3_retained_winner", 49_280, 70_000);
/// `finalize_selection_honest_lapse`.
pub const FINALIZE_SELECTION_LAPSE: Quote = row("finalize_selection_honest_lapse", 20_685, 30_000);

// --- entitled_clearing ---------------------------------------------------

/// `freeze_entitlement`.
pub const FREEZE_ENTITLEMENT: Quote = row("freeze_entitlement", 98_411, 130_000);
/// `entitle_slice_single_exhibit_book` — the larger of the two single rows.
pub const ENTITLE_SLICE_SINGLE: Quote = row("entitle_slice_single_exhibit_book", 224_645, 290_000);
/// `entitle_slice_portfolio_pair_exhibit_book` — the larger of the two pair rows.
pub const ENTITLE_SLICE_PAIR: Quote =
    row("entitle_slice_portfolio_pair_exhibit_book", 270_660, 340_000);
/// `settle_page_entitled_direct_slice`.
pub const SETTLE_SINGLE: Quote = row("settle_page_entitled_direct_slice", 47_328, 60_000);
/// `settle_page_entitled_portfolio_full_pair_exhibit_book` — the larger row.
pub const SETTLE_PAIR: Quote = row(
    "settle_page_entitled_portfolio_full_pair_exhibit_book",
    250_584,
    320_000,
);

// --- terminal_closure: no W1 row exists ----------------------------------

/// `ReleaseTerminalReservation` (tag 60) — unquoted.
pub const RELEASE_RESERVATION: Quote =
    Quote::unquoted("terminal_closure.release", UNQUOTED_CLOSE_LIMIT);
/// `CloseGeneralReceipt` (tag 61) — unquoted.
pub const CLOSE_RECEIPT: Quote = Quote::unquoted("terminal_closure.receipt", UNQUOTED_CLOSE_LIMIT);
/// `CloseGeneralReservation` (tag 62) — unquoted.
pub const CLOSE_RESERVATION: Quote =
    Quote::unquoted("terminal_closure.reservation", UNQUOTED_CLOSE_LIMIT);
/// `CloseGeneralPage` (tag 63) — unquoted.
pub const CLOSE_PAGE: Quote = Quote::unquoted("terminal_closure.page", UNQUOTED_CLOSE_LIMIT);
/// `CloseGeneralPot` (tag 64) — unquoted.
pub const CLOSE_POT: Quote = Quote::unquoted("terminal_closure.pot", UNQUOTED_CLOSE_LIMIT);
/// `CloseGeneralCandidate` (tag 65) — unquoted.
pub const CLOSE_CANDIDATE: Quote =
    Quote::unquoted("terminal_closure.candidate", UNQUOTED_CLOSE_LIMIT);
/// `CloseGeneralClearWork` (tag 66) — unquoted.
pub const CLOSE_CLEAR_WORK: Quote =
    Quote::unquoted("terminal_closure.clear_work", UNQUOTED_CLOSE_LIMIT);
/// `CloseGeneralEpoch` (tag 67) — unquoted.
pub const CLOSE_EPOCH: Quote = Quote::unquoted("terminal_closure.epoch", UNQUOTED_CLOSE_LIMIT);

// --- resolution_work batched folds ---------------------------------------

/// `projection.resolution_work_batched.routes.fold_batch_<n>.selected_limit_cu`,
/// indexed by the batch sizes the profile measured: 2, 4, 8, 12.
pub const FOLD_BATCH_ROWS: [(u8, u32, u32); 4] = [
    (2, 176_191, 230_000),
    (4, 331_897, 420_000),
    (8, 665_757, 840_000),
    (12, 929_429, 1_170_000),
];

/// `resolution_work.fold_1_cu`, worst observation, as the per-instruction cost
/// a batch of unmeasured width is priced from.
pub const RESOLUTION_FOLD_MEASURED_CU: u32 = 84_753;

/// `resolution_work.fold_4_cu`: one Fold instruction carrying the layout's
/// maximum `MAX_FOLD_RECORDS_V1` records.
///
/// The packet cost of a Fold instruction does not depend on its record count —
/// `record_count` is one fixed byte of the body — so a record-dense batch buys
/// four times the work for the same bytes.  That is what makes the packet
/// answer and the compute answer different questions.
pub const RESOLUTION_FOLD_4_MEASURED_CU: u32 = 95_710;

/// The total records one work item of the profile's projection carries.
pub const RESOLUTION_MAX_RECORDS: u32 = 32;

/// The quote for a batch of `n` singleton Fold instructions.
///
/// A measured batch size takes its own row.  Any other width is priced from
/// the per-instruction worst case and reported as a derived quote, because the
/// profile measured only 2, 4, 8, and 12 — the very reason this keeper has to
/// answer the packet question in the first place.
#[must_use]
pub fn fold_batch(n: u8) -> Quote {
    for (size, measured, limit) in FOLD_BATCH_ROWS {
        if size == n {
            return Quote {
                route: "fold_batch_measured",
                measured_cu: Some(measured),
                limit_cu: limit,
                ledgers: 0,
            };
        }
    }
    let derived = u64::from(RESOLUTION_FOLD_MEASURED_CU) * u64::from(n);
    match selected_limit(derived) {
        Some(limit) => Quote::unquoted("fold_batch_derived_from_fold_1_cu", limit),
        // Past the raw admission bound there is no admitted limit at all.
        // The profile calls this `STOP_HEADROOM`, and a STOP that quietly
        // takes the ceiling anyway would be the opposite of a quote.
        None => Quote::unquoted("fold_batch_STOP_HEADROOM", TRANSACTION_CEILING_CU),
    }
}

/// Whether a measured cost is inside the profile's raw admission bound.
///
/// The compute half of the batching question, kept next to the wire half:
/// a batch can be admissible on compute and still not frame into a packet.
#[must_use]
pub const fn admits_on_compute(measured_cu: u64) -> bool {
    measured_cu <= MAX_ADMITTED_RAW_CU
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_selection_rule_reproduces_every_embedded_row() {
        // If this ever fails, either the embedded table drifted from the
        // profile or the admission rule changed under it.
        for quote in [
            FREEZE_EPOCH_1PAGE,
            FREEZE_EPOCH_2PAGES,
            FREEZE_EPOCH_3PAGES,
            INIT_CLEAR_WORK,
            ADVANCE_PASS1_SMALL,
            ADVANCE_PASS2_SMALL,
            ADVANCE_PASS1_FORTY,
            ADVANCE_PASS2_FORTY,
            ADVANCE_SLICES,
            COMPLETE_CLEAR_WORK,
            FINALIZE_SELECTION,
            FINALIZE_SELECTION_LAPSE,
            FREEZE_ENTITLEMENT,
            ENTITLE_SLICE_SINGLE,
            ENTITLE_SLICE_PAIR,
            SETTLE_SINGLE,
            SETTLE_PAIR,
        ] {
            let measured = quote.measured_cu.expect("a W1 row carries its measurement");
            assert_eq!(
                selected_limit(u64::from(measured)),
                Some(quote.limit_cu),
                "route {} does not re-derive its own limit",
                quote.route
            );
        }
    }

    #[test]
    fn the_batched_fold_rows_re_derive_too() {
        for (size, measured, limit) in FOLD_BATCH_ROWS {
            assert_eq!(
                selected_limit(u64::from(measured)),
                Some(limit),
                "fold_batch_{size} does not re-derive its own limit"
            );
        }
    }

    #[test]
    fn the_headroom_bound_is_the_profiles() {
        assert_eq!(selected_limit(MAX_ADMITTED_RAW_CU), Some(1_400_000));
        assert_eq!(selected_limit(MAX_ADMITTED_RAW_CU + 1), None);
    }

    #[test]
    fn a_ledgered_creation_is_never_reported_as_a_quoted_route() {
        // The row's own limit was not enough on a real bank: at 60,000 the
        // ledgered InitEpoch exhausted its meter at 59,850.
        let bare = INIT_EPOCH;
        assert!(bare.quoted());
        assert_eq!(bare.limit_cu, 60_000);
        let with_ledger = ledgered(INIT_EPOCH, 1);
        assert!(!with_ledger.quoted(), "an allowance is not a measurement");
        assert!(with_ledger.limit_cu > 59_850);
        assert_eq!(with_ledger.measured_cu, Some(42_743), "the row is still named");
        assert!(ledgered(INIT_EPOCH, 2).limit_cu > with_ledger.limit_cu);
    }

    #[test]
    fn the_close_family_is_marked_unquoted() {
        for quote in [
            RELEASE_RESERVATION,
            CLOSE_RECEIPT,
            CLOSE_RESERVATION,
            CLOSE_PAGE,
            CLOSE_POT,
            CLOSE_CANDIDATE,
            CLOSE_CLEAR_WORK,
            CLOSE_EPOCH,
        ] {
            assert!(!quote.quoted(), "{} must not claim a row", quote.route);
        }
    }

    #[test]
    fn an_unmeasured_batch_width_is_priced_and_flagged() {
        assert!(fold_batch(12).quoted());
        let six = fold_batch(6);
        assert!(!six.quoted());
        assert_eq!(six.limit_cu, selected_limit(6 * 84_753).expect("admits"));
    }
}
