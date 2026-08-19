//! Instruction routing.
//!
//! This module is a router and nothing else.  It decodes the reference request
//! envelope, matches the action tag, and hands the already-routed request to
//! exactly one instruction-family module.  It performs no account validation,
//! reads no account data, and writes nothing: every check lives either in
//! [`crate::accounts`] or in the family module that owns the instruction.
//!
//! ## Why the request is decoded first
//!
//! The bring-up program had one instruction, so it could authenticate its fixed
//! nine-account list before looking at the instruction data at all.  A program
//! with an instruction *set* cannot: how many accounts an instruction takes,
//! which are writable, and what each one must be is a function of which
//! instruction it is, and that fact lives in the data.  So the envelope is
//! decoded here, before any account is touched.
//!
//! The consequence is named rather than hidden.  For a request that decodes,
//! nothing changed: the family module runs the same checks in the same order
//! and produces the same refusal codes.  For a request that does *not* decode
//! and is also presented with bad accounts, the codec refusal now wins where
//! the account refusal used to.  Both are refusals, no state is read or written
//! in either case, and the SVM differential is unaffected — but it is an
//! ordering change, so it is written down here and in
//! `docs/implementation/SBF_BRINGUP.md` instead of being discovered later.
//!
//! ## Refusal discipline for families that are not written yet
//!
//! A family module refuses with [`ClutchError::NotYetImplemented`] unless the
//! offline reference adapter refuses the same action for a *stronger,
//! structural* reason, in which case this program mirrors that reason exactly.
//! Historically that was `CreateMarket` refusing
//! [`ClutchError::AuthorizationUnavailable`] before an authority model existed,
//! and `Resolve`/`RedeemInternal` refusing
//! [`ClutchError::ResolutionEvidenceUnavailable`] before the evidence plane
//! landed; today the one stub is `SettlePage`, refusing
//! [`ClutchError::NotYetImplemented`] for the measured frame reason recorded
//! in [`crate::instructions::orders_batch`].  Nothing anywhere returns success
//! it did not earn.

use crate::accounts::Outcome;
use crate::error::ClutchError;
use crate::instructions::{
    artifact, cash_exit, external_exit, genesis, market_init, merge_materialize, observe_resolve,
    orders_batch, split,
};
use clutch_solana_layout::Intent;
use clutch_solana_reference::{Action, Request};
use solana_account_info::AccountInfo;
use solana_pubkey::Pubkey;

/// Decode one request and route it to the instruction family that owns it.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> Outcome<()> {
    let request = Request::decode(instruction_data)?;
    match request.action {
        Action::Layout(Intent::Split {
            market,
            owner,
            quantity,
        }) => split::process(
            program_id,
            accounts,
            &split::SplitRequest {
                sequence: request.sequence,
                market,
                owner,
                quantity,
            },
        ),
        Action::Layout(Intent::Merge { .. })
        | Action::Layout(Intent::Materialize { .. })
        | Action::Layout(Intent::Dematerialize { .. }) => {
            merge_materialize::process(program_id, accounts, &request)
        }
        Action::Layout(Intent::CreateMarket { .. }) => {
            market_init::process(program_id, accounts, &request)
        }
        Action::Layout(Intent::FeedAdvance { .. })
        | Action::Resolve { .. }
        | Action::RedeemInternal { .. } => observe_resolve::process(program_id, accounts, &request),
        Action::Layout(Intent::RedeemExternal { .. }) => {
            external_exit::process(program_id, accounts, &request)
        }
        Action::Layout(Intent::WithdrawCash { .. }) => {
            cash_exit::process(program_id, accounts, &request)
        }
        Action::Layout(Intent::BeginArtifact { .. })
        | Action::Layout(Intent::WriteArtifact { .. })
        | Action::Layout(Intent::SealArtifact { .. })
        | Action::Layout(Intent::AbortArtifact { .. }) => {
            artifact::process(program_id, accounts, &request)
        }
        Action::Layout(Intent::PlaceOrder { .. })
        | Action::Layout(Intent::CancelOrder { .. })
        | Action::Layout(Intent::SettlePage { .. }) => {
            orders_batch::process(program_id, accounts, &request)
        }
        Action::Layout(Intent::InitRealm { .. })
        | Action::Layout(Intent::InitProfile { .. })
        | Action::Layout(Intent::InitPriceGrid { .. })
        | Action::Layout(Intent::InitTerms { .. })
        | Action::Layout(Intent::InitOrderPage { .. })
        | Action::Layout(Intent::Endow { .. }) => genesis::process(program_id, accounts, &request),
    }
}

/// The refusal a family module returns when its transition is not written yet.
///
/// Kept here so that every stub returns one identical, greppable thing and a
/// lane replacing a stub deletes a call to this function rather than editing a
/// bespoke error expression.
pub fn not_yet_implemented() -> Outcome<()> {
    Err(ClutchError::NotYetImplemented.into())
}
