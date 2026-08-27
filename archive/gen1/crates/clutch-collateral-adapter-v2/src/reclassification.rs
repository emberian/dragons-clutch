// SPDX-License-Identifier: AGPL-3.0-or-later

//! Exact backing reclassification over unchanged Realm collateral custody.
//!
//! Split and Merge change the portion of pooled Hoard collateral locked behind
//! complete-set liabilities. They do not move collateral tokens: holder entry
//! and exit are separate Position/Hoard boundary operations. This module makes
//! that absence of CPI an authenticated contract rather than a special case in
//! an SBF handler.

use crate::{
    admit_collateral_account_v2, admit_collateral_mint_v2, BoundCollateralProfileV2, ClaimLedgerV3,
    CollateralBackingV2, CompleteSetReclassificationKindV3, CompleteSetReclassificationPlanV3,
    Error, HoardV2, Id, MintObservationV2, Result, RuntimeAccountViewV2, TokenAccountObservationV2,
    TokenAccountRoleV2,
};
use clutch_retirement::{
    GeneralPositionProjectionV3, PositionAccountV3, PositionLifecycleV3, PositionV3Fields,
    PositionV3Sha256Backend,
};

/// Content domain for one accepted backing-only reclassification.
pub const BACKING_RECLASSIFICATION_RECEIPT_DOMAIN_V2: &[u8] =
    b"dragons-clutch/collateral-adapter/backing-reclassification-receipt/v2\0";
/// Full-width Position/Hoard/ClaimLedger complete-set intent domain.
pub const COMPLETE_SET_POSITION_TRANSITION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/complete-set/position-transition/v3\0";
/// Accepted full-width complete-set transition domain.
pub const ACCEPTED_COMPLETE_SET_POSITION_TRANSITION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/complete-set/accepted-position-transition/v3\0";

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

/// Prepared full-width Position/Hoard/ClaimLedger transition plus exact token
/// nonmovement proof. Private fields prevent a runtime from publishing one
/// subset of the successor accounts independently.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedCompleteSetPositionTransitionV3 {
    backing: PreparedBackingReclassificationV2,
    liability: CompleteSetReclassificationPlanV3,
    position_account: Id,
    position_before_id: Id,
    position_after: PositionAccountV3,
    position_after_id: Id,
    transition_id: Id,
}

impl PreparedCompleteSetPositionTransitionV3 {
    /// Exact transition identity to bind into GEN1 Replay.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }

    /// Complete Position successor, publishable only after token acceptance.
    pub const fn position_after(self) -> PositionAccountV3 {
        self.position_after
    }
}

/// Accepted complete-set successor across all three semantic owners.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedCompleteSetPositionTransitionV3 {
    liability: CompleteSetReclassificationPlanV3,
    position_account: Id,
    position_before_id: Id,
    position_after: PositionAccountV3,
    position_after_id: Id,
    token_nonmovement_receipt_id: Id,
    transition_id: Id,
    receipt_id: Id,
}

impl AcceptedCompleteSetPositionTransitionV3 {
    /// Complete Hoard/ClaimLedger successor plan.
    pub const fn liability(self) -> CompleteSetReclassificationPlanV3 {
        self.liability
    }

    /// Exact canonical Position account.
    pub const fn position_account(self) -> Id {
        self.position_account
    }

    /// Position semantic ID before this transition.
    pub const fn position_before_id(self) -> Id {
        self.position_before_id
    }

    /// Complete permitted Position successor.
    pub const fn position_after(self) -> PositionAccountV3 {
        self.position_after
    }

    /// Position semantic ID after this transition.
    pub const fn position_after_id(self) -> Id {
        self.position_after_id
    }

    /// Exact zero-token-delta receipt.
    pub const fn token_nonmovement_receipt_id(self) -> Id {
        self.token_nonmovement_receipt_id
    }

    /// Exact transition identity prepared before token observation.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }

    /// Canonical accepted receipt for GEN1 evidence.
    pub const fn receipt_id(self) -> Id {
        self.receipt_id
    }
}

/// Prepare a Split/Merge across canonical Position V3, Hoard V2, and
/// ClaimLedger V3 while authorizing no token CPI.
#[allow(clippy::too_many_arguments)]
pub fn prepare_complete_set_position_transition_v3<B: PositionV3Sha256Backend>(
    bound: BoundCollateralProfileV2,
    position_account: Id,
    position: GeneralPositionProjectionV3,
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
    kind: CompleteSetReclassificationKindV3,
    quantity: u64,
    mint: RuntimeAccountViewV2<'_>,
    hoard_token: RuntimeAccountViewV2<'_>,
    backend: &B,
) -> Result<PreparedCompleteSetPositionTransitionV3> {
    position_account.require_live()?;
    let position_before = position.position();
    position_before
        .validate()
        .map_err(|_| Error::MismatchedBinding)?;
    let fields = position_before.fields();
    if position_before.lifecycle() != PositionLifecycleV3::Open
        || Id::from_bytes(fields.market_instance_id.bytes()) != hoard.market_instance_id
        || Id::from_bytes(fields.realm_id.bytes()) != hoard.realm_id
        || Id::from_bytes(fields.collateral_policy_id.bytes()) != hoard.collateral_policy_id
        || Id::from_bytes(fields.collateral_release_id.bytes()) != hoard.collateral_release_id
        || bound.market().market != hoard.market_instance_id
        || bound.market().realm != hoard.realm_id
        || bound.market().profile != hoard.profile_id
        || bound.policy_id() != hoard.collateral_policy_id
        || bound.release().id()? != hoard.collateral_release_id
        || quantity == 0
    {
        return Err(Error::MismatchedBinding);
    }
    let liability = crate::prepare_complete_set_reclassification_v3(
        hoard,
        claim_ledger,
        kind,
        quantity,
        backend,
    )?;
    let mut next_fields: PositionV3Fields = fields;
    match kind {
        CompleteSetReclassificationKindV3::Split => {
            let free = fields
                .cash_atoms
                .checked_sub(fields.reserved_cash_atoms)
                .ok_or(Error::AggregateLiabilityInsufficient)?;
            if quantity > free {
                return Err(Error::InsufficientUnreservedCash);
            }
            next_fields.cash_atoms = fields
                .cash_atoms
                .checked_sub(quantity)
                .ok_or(Error::AggregateLiabilityInsufficient)?;
        }
        CompleteSetReclassificationKindV3::Merge => {
            next_fields.cash_atoms = fields
                .cash_atoms
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?;
        }
    }
    let mut index = 0usize;
    while index < usize::from(fields.outcome_count) {
        next_fields.native_eggs[index] = match kind {
            CompleteSetReclassificationKindV3::Split => fields.native_eggs[index]
                .checked_add(quantity)
                .ok_or(Error::Arithmetic)?,
            CompleteSetReclassificationKindV3::Merge => fields.native_eggs[index]
                .checked_sub(quantity)
                .ok_or(Error::AggregateLiabilityInsufficient)?,
        };
        index += 1;
    }
    let position_after =
        PositionAccountV3::new(next_fields).map_err(|_| Error::MismatchedBinding)?;
    let position_before_id = Id::from_bytes(
        position_before
            .semantic_id(backend)
            .map_err(|_| Error::MismatchedBinding)?
            .bytes(),
    );
    let position_after_id = Id::from_bytes(
        position_after
            .semantic_id(backend)
            .map_err(|_| Error::MismatchedBinding)?
            .bytes(),
    );
    let kind_byte = [match kind {
        CompleteSetReclassificationKindV3::Split => 1,
        CompleteSetReclassificationKindV3::Merge => 2,
    }];
    let transition_id = crate::digest(
        COMPLETE_SET_POSITION_TRANSITION_DOMAIN_V3,
        &[
            &kind_byte,
            &position_account.bytes(),
            &position_before_id.bytes(),
            &position_after_id.bytes(),
            &liability.hoard_before_id.bytes(),
            &liability.hoard_after_id.bytes(),
            &liability.claim_ledger_before_id.bytes(),
            &liability.claim_ledger_after_id.bytes(),
            &quantity.to_le_bytes(),
        ],
    );
    transition_id.require_live()?;
    let backing_kind = match kind {
        CompleteSetReclassificationKindV3::Split => BackingReclassificationKindV2::Lock,
        CompleteSetReclassificationKindV3::Merge => BackingReclassificationKindV2::Unlock,
    };
    let backing = prepare_backing_reclassification_v2(
        bound,
        BackingReclassificationRequestV2 {
            transition_id,
            position_account,
            kind: backing_kind,
            amount_atoms: quantity,
            backing_before: CollateralBackingV2 {
                locked_atoms: hoard.locked_claim_principal_atoms,
                cap_atoms: hoard.collateral_cap_atoms,
            },
        },
        mint,
        hoard_token,
    )?;
    if backing.backing_after.locked_atoms != liability.hoard_after.locked_claim_principal_atoms
        || backing.backing_after.cap_atoms != liability.hoard_after.collateral_cap_atoms
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(PreparedCompleteSetPositionTransitionV3 {
        backing,
        liability,
        position_account,
        position_before_id,
        position_after,
        position_after_id,
        transition_id,
    })
}

/// Accept exact mint/Hoard token nonmutation and release the complete semantic
/// poststates as one capability.
pub fn accept_complete_set_position_transition_v3(
    prepared: PreparedCompleteSetPositionTransitionV3,
    mint_after: RuntimeAccountViewV2<'_>,
    hoard_token_after: RuntimeAccountViewV2<'_>,
) -> Result<AcceptedCompleteSetPositionTransitionV3> {
    let backing =
        accept_backing_reclassification_v2(prepared.backing, mint_after, hoard_token_after)?;
    if backing.transition_id != prepared.transition_id
        || backing.position_account != prepared.position_account
        || backing.backing_after.locked_atoms
            != prepared.liability.hoard_after.locked_claim_principal_atoms
        || backing.visible_hoard_atoms < prepared.liability.hoard_after.required_custody_atoms()?
    {
        return Err(Error::PostAdmissionFailed);
    }
    let receipt_id = crate::digest(
        ACCEPTED_COMPLETE_SET_POSITION_TRANSITION_DOMAIN_V3,
        &[
            &prepared.transition_id.bytes(),
            &backing.receipt_id.bytes(),
            &prepared.position_before_id.bytes(),
            &prepared.position_after_id.bytes(),
            &prepared.liability.receipt_id.bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(AcceptedCompleteSetPositionTransitionV3 {
        liability: prepared.liability,
        position_account: prepared.position_account,
        position_before_id: prepared.position_before_id,
        position_after: prepared.position_after,
        position_after_id: prepared.position_after_id,
        token_nonmovement_receipt_id: backing.receipt_id,
        transition_id: prepared.transition_id,
        receipt_id,
    })
}
