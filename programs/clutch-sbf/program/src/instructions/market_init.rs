//! `Intent::CreateMarket` — **stub, and fail-closed for a second reason**.
//!
//! Nothing here is implemented.  A `CreateMarket` request is refused with
//! [`crate::error::ClutchError::AuthorizationUnavailable`], not with
//! [`crate::error::ClutchError::NotYetImplemented`], and the difference is the
//! point: even with the code written, this program would still have no answer
//! to *who is allowed to bring a market into existence*.  The offline reference
//! adapter refuses the same action with the same meaning
//! (`Error::AuthorizationUnavailable`), and mirroring it keeps the two adapters
//! saying the same thing about the same request.
//!
//! No account is read, no byte is written, and no success is reported.
//!
//! ## What the owning lane has to decide
//!
//! - **The authority model, first.** Writing the initializer before deciding
//!   who may call it would replace an honest refusal with an unauthenticated
//!   mint, which is strictly worse than not having the instruction.
//! - Account *creation*: this program has never created an account. Every
//!   account in the bring-up fixture is preloaded at genesis, so the
//!   `system_instruction::create_account` CPI, the rent-exemption computation,
//!   and the `invoke_signed` seed plumbing are all unwritten and untested. That
//!   is deferred checks 2, 4, and 6 of `docs/implementation/SBF_BRINGUP.md`.
//! - `clutch_solana_reference::validate_market_init` is the offline template
//!   for the byte and identity coherence an initializer must establish. It is
//!   also one of the two functions the SBF backend reports as overflowing the
//!   4 KiB frame, so it cannot be called from a program as written — the checks
//!   have to be re-composed here through `#[inline(never)]` readers, the same
//!   way [`super::split`] handles its large decoded values.
//! - `require_frozen_collateral_policy` in the reference adapter refuses a
//!   Realm whose Profile has not frozen its collateral policy. That refusal has
//!   no on-chain counterpart yet and belongs to this instruction.

use crate::accounts::Outcome;
use crate::error::ClutchError;
use clutch_solana_reference::Request;
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Refuse: no authority policy exists for market creation, so it fails closed.
///
/// The parameters are the shape the owning lane inherits; see the module docs
/// for why a stub deliberately inspects none of them.
pub fn process(_program_id: &Pubkey, _accounts: &[AccountInfo], _request: &Request) -> Outcome<()> {
    Err(ClutchError::AuthorizationUnavailable.into())
}
