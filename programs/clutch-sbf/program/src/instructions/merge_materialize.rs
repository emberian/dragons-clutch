//! `Intent::Merge`, `Intent::Materialize`, `Intent::Dematerialize` — **stub**.
//!
//! Nothing here is implemented.  Every request routed to this module is
//! refused with [`crate::error::ClutchError::NotYetImplemented`]; no account is
//! read, no byte is written, and no success is reported.
//!
//! This is a *gap*, not a policy.  The offline reference adapter
//! (`clutch_solana_reference::apply`) implements all three of these transitions
//! today — `MarketState::merge`, `MarketState::materialize`, and
//! `MarketState::dematerialize` are in `clutch-kernel` and are exercised there
//! — so the missing part is exactly the account plane: which accounts each
//! instruction takes, which are writable, and what the external-shadow
//! destination or source account must be checked against.
//!
//! ## What the owning lane has to decide
//!
//! - Merge shares the nine-account list of [`super::split`] as written, since
//!   it moves the same seam in the opposite direction; whether it should also
//!   carry the supply ledger is open.
//! - Materialize and Dematerialize name a destination or source in the intent
//!   itself, and the offline adapter checks it against
//!   `metadata.external.key`.  On-chain that comparison has to become a
//!   derivation against [`crate::seeds::external_pda`], because a caller-named
//!   destination is exactly the trusted binding obligation 1 of
//!   `docs/implementation/SOLANA_REFERENCE_ADAPTER.md` forbids.
//! - All three change the two-term supply ledger in the offline adapter's
//!   post-state, and the on-chain account set has no ledger account yet; see
//!   deferred check 13 in `docs/implementation/SBF_BRINGUP.md`.

use crate::accounts::Outcome;
use crate::dispatch;
use clutch_solana_reference::Request;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Refuse: the seam transitions other than `Split` are not written yet.
///
/// The parameters are the shape the owning lane inherits; they are unused
/// because a stub that inspected accounts would imply that the account list it
/// inspected is the right one, and choosing that list is this lane's decision.
pub fn process(_program_id: &Pubkey, _accounts: &[AccountInfo], _request: &Request) -> Outcome<()> {
    dispatch::not_yet_implemented()
}
