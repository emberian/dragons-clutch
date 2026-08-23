// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact resolved-claim payout at the Market Hoard boundary.
//!
//! Burning or otherwise retiring an outcome claim belongs to the separately
//! identified claim plane. This module only moves the already-computed,
//! authenticated collateral payout. It therefore cannot mint or burn an Egg,
//! select a payout vector, or reinterpret claim quantity as collateral atoms.

use crate::{
    accept_collateral_transfer_v2, admit_collateral_account_v2, prepare_collateral_transfer_v2,
    AcceptedCollateralTransferV2, BoundCollateralProfileV2, CheckedTransferCpiV2,
    CollateralBackingV2, CustodyTransferKindV2, Error, Id, PreparedCollateralTransferV2, Result,
    RuntimeAccountViewV2, TokenAccountRoleV2, TransferAuthorityV2, TransferEndpointV2,
    TransferRequestV2,
};

/// Content domain for one accepted claim-plane collateral payout.
pub const CLAIM_REDEMPTION_COLLATERAL_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/claim-redemption-collateral-receipt/v2\0";

/// Claim-plane authorization already authenticated by the consuming runtime.
///
/// The claim adapter remains the semantic owner of `claim_redemption_id` and
/// of the payout arithmetic. The collateral adapter consumes only the exact
/// payout in raw collateral atoms and the before/after locked-principal join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimRedemptionCollateralRequestV2 {
    /// Full-width semantic identity of the authenticated claim redemption.
    pub claim_redemption_id: Id,
    /// Exact receive-only collateral token account named by the claim action.
    pub destination_token_account: Id,
    /// Semantic owner of the claim action, normally its authenticated claimant.
    pub claim_semantic_owner: Id,
    /// Exact collateral payout, already computed by the claim plane.
    pub payout_atoms: u64,
    /// Market locked collateral before the claim is retired.
    pub backing_before: CollateralBackingV2,
}

/// Prepared nonzero Hoard-to-claimant collateral movement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedClaimRedemptionCollateralV2 {
    inner: PreparedCollateralTransferV2,
    bound: BoundCollateralProfileV2,
    request: ClaimRedemptionCollateralRequestV2,
    backing_after: CollateralBackingV2,
}

impl PreparedClaimRedemptionCollateralV2 {
    /// Sole release-selected collateral CPI authorized by this preparation.
    pub const fn cpi(self) -> CheckedTransferCpiV2 {
        self.inner.cpi()
    }

    /// Derived locked-principal state after the exact claim payout.
    pub const fn backing_after(self) -> CollateralBackingV2 {
        self.backing_after
    }
}

/// Accepted exact payout and the claim-plane receipt join safe to commit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedClaimRedemptionCollateralV2 {
    /// Complete admitted custody result.
    pub custody: AcceptedCollateralTransferV2,
    /// Derived locked-principal state after claim retirement.
    pub backing_after: CollateralBackingV2,
    /// Receipt binding claim identity, payout, exact deltas, and backing state.
    pub receipt_id: Id,
}

/// Prepare one nonzero claim payout without touching Token-2022 claim state.
///
/// Zero-payout claims deliberately have no collateral CPI and must be handled
/// as a claim-plane-only transition. For a nonzero payout, this requires the
/// Hoard to cover the complete prestate liability, derives the only permitted
/// backing decrease, and prepares a release-selected checked transfer to the
/// exact receive-only destination account.
#[allow(clippy::too_many_arguments)]
pub fn prepare_claim_redemption_collateral_v2(
    bound: BoundCollateralProfileV2,
    request: ClaimRedemptionCollateralRequestV2,
    authority: TransferAuthorityV2,
    mint: RuntimeAccountViewV2<'_>,
    hoard_source: RuntimeAccountViewV2<'_>,
    destination: RuntimeAccountViewV2<'_>,
) -> Result<PreparedClaimRedemptionCollateralV2> {
    request.claim_redemption_id.require_live()?;
    request.destination_token_account.require_live()?;
    request.claim_semantic_owner.require_live()?;
    if request.payout_atoms == 0 || destination.key != request.destination_token_account {
        return Err(Error::InvalidParameter);
    }
    let market = bound.market();
    if authority.address != market.hoard_authority || hoard_source.key != market.hoard_token_account
    {
        return Err(Error::MismatchedBinding);
    }
    let hoard_before = admit_collateral_account_v2(bound, hoard_source, TokenAccountRoleV2::Hoard)?;
    request
        .backing_before
        .validate(bound, hoard_before.amount_atoms)?;
    let backing_after = request.backing_before.after_unlock(
        bound,
        hoard_before.amount_atoms,
        request.payout_atoms,
    )?;
    let inner = prepare_collateral_transfer_v2(
        bound,
        TransferRequestV2 {
            kind: CustodyTransferKindV2::ClaimRedemption,
            source: TransferEndpointV2 {
                token_role: TokenAccountRoleV2::Hoard,
                semantic_owner: market.market,
                compartment: 1,
            },
            destination: TransferEndpointV2 {
                token_role: TokenAccountRoleV2::ReceiveOnly {
                    account: request.destination_token_account,
                },
                semantic_owner: request.claim_semantic_owner,
                compartment: 0,
            },
            authority,
            amount_atoms: request.payout_atoms,
            position_cash: None,
            locked_collateral_atoms: backing_after.locked_atoms,
        },
        mint,
        hoard_source,
        destination,
    )?;
    Ok(PreparedClaimRedemptionCollateralV2 {
        inner,
        bound,
        request,
        backing_after,
    })
}

/// Reparse exact post-CPI bytes and bind the custody result to claim retirement.
pub fn accept_claim_redemption_collateral_v2(
    prepared: PreparedClaimRedemptionCollateralV2,
    mint_after: RuntimeAccountViewV2<'_>,
    hoard_after: RuntimeAccountViewV2<'_>,
    destination_after: RuntimeAccountViewV2<'_>,
) -> Result<AcceptedClaimRedemptionCollateralV2> {
    let custody =
        accept_collateral_transfer_v2(prepared.inner, mint_after, hoard_after, destination_after)?;
    let visible_hoard_after = custody
        .hoard_atoms_after
        .ok_or(Error::PostAdmissionFailed)?;
    prepared
        .backing_after
        .validate(prepared.bound, visible_hoard_after)?;
    let request = prepared.request;
    let receipt_id = crate::digest(
        CLAIM_REDEMPTION_COLLATERAL_RECEIPT_DOMAIN_V2,
        &[
            &request.claim_redemption_id.bytes(),
            &prepared.bound.market().market.bytes(),
            &request.destination_token_account.bytes(),
            &request.claim_semantic_owner.bytes(),
            &request.payout_atoms.to_le_bytes(),
            &request.backing_before.locked_atoms.to_le_bytes(),
            &prepared.backing_after.locked_atoms.to_le_bytes(),
            &custody.source_atoms_after.to_le_bytes(),
            &custody.destination_atoms_after.to_le_bytes(),
            &custody.mint_supply_after.to_le_bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(AcceptedClaimRedemptionCollateralV2 {
        custody,
        backing_after: prepared.backing_after,
        receipt_id,
    })
}
