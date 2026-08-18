//! `Intent::FeedAdvance`, `Action::Resolve`, `Action::RedeemInternal` — **stub**.
//!
//! Nothing here is implemented, and the two refusals this module returns are
//! deliberately different from each other.
//!
//! `Resolve` and `RedeemInternal` refuse with
//! [`crate::error::ClutchError::ResolutionEvidenceUnavailable`], mirroring the
//! offline reference adapter's `Error::ResolutionEvidenceUnavailable`.  That is
//! a structural statement, not a to-do: the adapter's resolution gate is driven
//! by a typed [`clutch_solana_reference::ResolutionEvidence`] plane —
//! observation records folded through the accumulator's
//! `Open -> Mature -> Sealed` state machine — and this program has no account,
//! no instruction data path, and no compute budget for one.  Absent evidence
//! must stay a *missing code path* rather than a flag somebody can set.
//!
//! `FeedAdvance` refuses with [`crate::error::ClutchError::NotYetImplemented`]:
//! the feed head is an ordinary account with an ordinary monotone cursor, and
//! nothing structural is missing, only the code.
//!
//! No account is read, no byte is written, and no success is reported.
//!
//! ## What the owning lane has to decide
//!
//! - Whether the window-evidence blob rides in instruction data (it is up to
//!   [`clutch_solana_reference::MAX_WINDOW_EVIDENCE_LEN`] bytes, against a
//!   1232-byte transaction) or in a preloaded account written by a separate
//!   fold instruction.  This is the resource question of obligation 10.
//! - The account list for the evidence plane: [`crate::accounts::read_terms`],
//!   [`crate::accounts::read_resolution`], and [`crate::accounts::read_feed`]
//!   exist and are unused, with seeds at [`crate::seeds::terms_pda`],
//!   [`crate::seeds::resolution_pda`], and [`crate::seeds::feed_pda`].
//! - `clutch_solana_reference::apply_with_evidence` composes the whole gate
//!   offline and is the semantic template; it is also the function the SBF
//!   backend reports as overflowing the 4 KiB frame, so the composition has to
//!   be rebuilt out of small frames rather than called.
//! - Redemption additionally moves collateral and position cash, so it shares
//!   the seam accounts of [`super::split`] as well as the evidence plane.

use crate::accounts::Outcome;
use crate::dispatch;
use crate::error::ClutchError;
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Refuse: the observation and resolution plane is not written yet.
///
/// The parameters other than the request are the shape the owning lane
/// inherits; see the module docs for why a stub inspects none of them.
pub fn process(_program_id: &Pubkey, _accounts: &[AccountInfo], request: &Request) -> Outcome<()> {
    match request.action {
        /* Not "unwritten": there is no evidence plane on-chain at all, and the
         * fail-closed default has to stay the absence of a code path. */
        Action::Resolve { .. } | Action::RedeemInternal { .. } => {
            Err(ClutchError::ResolutionEvidenceUnavailable.into())
        }
        Action::Layout(_) => dispatch::not_yet_implemented(),
    }
}
