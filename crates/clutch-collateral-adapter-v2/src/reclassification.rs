// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact backing reclassification over unchanged Realm collateral custody.
//!
//! Split and Merge change the portion of pooled Hoard collateral locked behind
//! complete-set liabilities. They do not move collateral tokens: holder entry
//! and exit are separate Position/Hoard boundary operations. This module makes
//! that absence of CPI an authenticated contract rather than a special case in
//! an SBF handler.

use crate::{
    admit_collateral_account_v2, admit_collateral_mint_v2, BoundCollateralProfileV2,
    CollateralBackingV2, Error, Id, MintObservationV2, Result, RuntimeAccountViewV2,
    TokenAccountObservationV2, TokenAccountRoleV2,
};

/// Content domain for one accepted backing-only reclassification.
pub const BACKING_RECLASSIFICATION_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/collateral-adapter/backing-reclassification-receipt/v2\0";

/// Direction in which complete-set accounting changes locked backing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum BackingReclassificationKindV2 {
    /// A Split creates complete-set liabilities and locks backing.
    Lock = 1,
    /// A Merge destroys complete-set liabilities and unlocks backing.
    Unlock = 2,
}

/// Exact semantic transition joined to an unchanged mint and Hoard.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BackingReclassificationRequestV2 {
    /// Content identity of the kernel/replay transition authorizing the change.
    pub transition_id: Id,
    /// Exact canonical Position account whose internal liabilities change.
    pub position_account: Id,
    /// Whether backing is locked or unlocked.
    pub kind: BackingReclassificationKindV2,
    /// Exact raw collateral atoms reclassified; decimals never rescale it.
    pub amount_atoms: u64,
    /// Locked-principal state before the kernel transition.
    pub backing_before: CollateralBackingV2,
}

/// Prepared proof that authorizes no external invocation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedBackingReclassificationV2 {
    bound: BoundCollateralProfileV2,
    request: BackingReclassificationRequestV2,
    backing_after: CollateralBackingV2,
    mint_before: MintObservationV2,
    hoard_before: TokenAccountObservationV2,
}

impl PreparedBackingReclassificationV2 {
    /// Exact locked-principal state the semantic transition must commit.
    pub const fn backing_after(self) -> CollateralBackingV2 {
        self.backing_after
    }
}

/// Accepted nonmovement proof safe to join to the semantic transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedBackingReclassificationV2 {
    /// Kernel/replay transition identity supplied by the semantic owner.
    pub transition_id: Id,
    /// Exact canonical Position account whose liabilities changed.
    pub position_account: Id,
    /// Applied backing direction.
    pub kind: BackingReclassificationKindV2,
    /// Exact raw collateral atoms reclassified.
    pub amount_atoms: u64,
    /// Locked-principal state before the transition.
    pub backing_before: CollateralBackingV2,
    /// Locked-principal state after the transition.
    pub backing_after: CollateralBackingV2,
    /// Exact visible Hoard atoms, unchanged across the transition.
    pub visible_hoard_atoms: u64,
    /// Claim-free receipt binding the transition to exact unchanged custody.
    pub receipt_id: Id,
}

/// Admit a selected collateral mint and Hoard before a backing-only change.
///
/// Both external token accounts are read-only because this contract emits no
/// CPI. The separately authenticated semantic owner must derive
/// `transition_id`; this receipt alone never authorizes a kernel or Position
/// write.
pub fn prepare_backing_reclassification_v2(
    bound: BoundCollateralProfileV2,
    request: BackingReclassificationRequestV2,
    mint: RuntimeAccountViewV2<'_>,
    hoard: RuntimeAccountViewV2<'_>,
) -> Result<PreparedBackingReclassificationV2> {
    request.transition_id.require_live()?;
    request.position_account.require_live()?;
    if request.amount_atoms == 0
        || mint.is_writable
        || hoard.is_writable
        || mint.key == hoard.key
        || hoard.key != bound.market().hoard_token_account
    {
        return Err(Error::WrongAccountRole);
    }
    let mint_before = admit_collateral_mint_v2(bound, mint)?;
    let hoard_before = admit_collateral_account_v2(bound, hoard, TokenAccountRoleV2::Hoard)?;
    request
        .backing_before
        .validate(bound, hoard_before.amount_atoms)?;
    let backing_after = match request.kind {
        BackingReclassificationKindV2::Lock => request.backing_before.after_lock(
            bound,
            hoard_before.amount_atoms,
            request.amount_atoms,
        )?,
        BackingReclassificationKindV2::Unlock => request.backing_before.after_unlock(
            bound,
            hoard_before.amount_atoms,
            request.amount_atoms,
        )?,
    };
    Ok(PreparedBackingReclassificationV2 {
        bound,
        request,
        backing_after,
        mint_before,
        hoard_before,
    })
}

/// Reparse exact post-transition bytes and require complete token nonmutation.
pub fn accept_backing_reclassification_v2(
    prepared: PreparedBackingReclassificationV2,
    mint_after: RuntimeAccountViewV2<'_>,
    hoard_after: RuntimeAccountViewV2<'_>,
) -> Result<AcceptedBackingReclassificationV2> {
    if mint_after.is_writable || hoard_after.is_writable {
        return Err(Error::PostAdmissionFailed);
    }
    let mint = admit_collateral_mint_v2(prepared.bound, mint_after)
        .map_err(|_| Error::PostAdmissionFailed)?;
    let hoard = admit_collateral_account_v2(prepared.bound, hoard_after, TokenAccountRoleV2::Hoard)
        .map_err(|_| Error::PostAdmissionFailed)?;
    if mint != prepared.mint_before || hoard != prepared.hoard_before {
        return Err(Error::TransferDeltaMismatch);
    }
    prepared
        .backing_after
        .validate(prepared.bound, hoard.amount_atoms)?;
    let request = prepared.request;
    let kind = match request.kind {
        BackingReclassificationKindV2::Lock => [1],
        BackingReclassificationKindV2::Unlock => [2],
    };
    let realm = prepared.bound.realm_bound().realm();
    let release_id = prepared.bound.release().id()?;
    let receipt_id = crate::digest(
        BACKING_RECLASSIFICATION_RECEIPT_DOMAIN_V2,
        &[
            &request.transition_id.bytes(),
            &request.position_account.bytes(),
            &prepared.bound.market().market.bytes(),
            &realm.realm.bytes(),
            &realm.profile.bytes(),
            &prepared.bound.policy_id().bytes(),
            &release_id.bytes(),
            &mint.address.bytes(),
            &hoard.address.bytes(),
            &kind,
            &request.amount_atoms.to_le_bytes(),
            &request.backing_before.locked_atoms.to_le_bytes(),
            &request.backing_before.cap_atoms.to_le_bytes(),
            &prepared.backing_after.locked_atoms.to_le_bytes(),
            &prepared.backing_after.cap_atoms.to_le_bytes(),
            &hoard.amount_atoms.to_le_bytes(),
            &mint.supply_atoms.to_le_bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(AcceptedBackingReclassificationV2 {
        transition_id: request.transition_id,
        position_account: request.position_account,
        kind: request.kind,
        amount_atoms: request.amount_atoms,
        backing_before: request.backing_before,
        backing_after: prepared.backing_after,
        visible_hoard_atoms: hoard.amount_atoms,
        receipt_id,
    })
}
