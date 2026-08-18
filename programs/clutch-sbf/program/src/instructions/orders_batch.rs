//! `Intent::PlaceOrder`, `Intent::CancelOrder`, `Intent::SettlePage` — **stub**.
//!
//! Nothing here is implemented.  Every request routed to this module is refused
//! with [`crate::error::ClutchError::NotYetImplemented`]; no account is read,
//! no byte is written, and no success is reported.
//!
//! This family is the batch-auction plane, and it is the one the offline
//! reference adapter does *not* implement either: `apply` refuses all three
//! with `Error::UnsupportedIntent`.  The byte layouts exist and are
//! well-tested in [`clutch_solana_layout`] — dense ordered pages, the
//! cross-page set commitment `verify_page_set`, the frozen tick grid, candidate
//! records, the final pot, and settlement receipts — but no adapter, offline or
//! on-chain, joins them to a transition.
//!
//! ## What the owning lane has to decide
//!
//! - Where the relation runs.  A candidate is *verified*, not computed, and the
//!   verification is a batch relation over the whole frozen book; whether that
//!   fits an SBF compute budget at all is an open obligation-10 question and
//!   has to be measured, not assumed.
//! - **An order page cannot be decoded on-chain at all today.** This is the
//!   hard blocker for this family, and it is measured rather than predicted:
//!   `clutch_solana_layout::OrderPageAccount::decode` is reported by the SBF
//!   backend at an estimated 8640-byte frame against a 4096-byte maximum, and
//!   `decode_on_grid` at 8320. [`crate::accounts::read_order_page`] exists but
//!   is compiled off-chain only for exactly that reason, so reaching for it
//!   here is a compile error and not a frame overflow. The fix is a streaming
//!   header-and-commitment decoder in `clutch-solana-layout`, and it has to
//!   land before any of this family can be written.
//! - Page-set closure then spans up to `MAX_ORDER_PAGES` accounts at once, and
//!   `clutch_solana_layout::verify_page_set` takes the pages by value as a
//!   slice, which no frame can hold either; the on-chain check has to be
//!   streamed page by page over the commitment fields alone.
//! - Freeze discipline: an epoch commits to a page set only in
//!   `EPOCH_PHASE_FROZEN`, and every account in this family carries a phase or
//!   status enum that has to be checked against the epoch's, not trusted.
//! - Seeds for all of it exist and are unused:
//!   [`crate::seeds::epoch_pda`], [`crate::seeds::page_pda`],
//!   [`crate::seeds::grid_pda`], [`crate::seeds::candidate_pda`],
//!   [`crate::seeds::pot_pda`], and [`crate::seeds::receipt_pda`].

use crate::accounts::Outcome;
use crate::dispatch;
use clutch_solana_reference::Request;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Refuse: the batch-auction plane is not written yet.
///
/// The parameters are the shape the owning lane inherits; see the module docs
/// for why a stub inspects none of them.
pub fn process(_program_id: &Pubkey, _accounts: &[AccountInfo], _request: &Request) -> Outcome<()> {
    dispatch::not_yet_implemented()
}
