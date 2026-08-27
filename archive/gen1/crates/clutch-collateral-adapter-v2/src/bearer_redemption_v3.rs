// SPDX-License-Identifier: AGPL-3.0-or-later

//! Positionless bearer-claim redemption against Resolution V5.
//!
//! This contract orders the two external effects: an exact Token-2022 bearer
//! burn must be accepted before the collateral payout request is exposed, and
//! canonical Hoard/ClaimLedger poststates are exposed only after the exact
//! zero or nonzero Realm-selected collateral postcondition is accepted.

use clutch_retirement::{PositionV3Sha256Backend, MAX_OUTCOMES};

use crate::{
    digest, AcceptedClaimRedemptionCollateralV2, AcceptedZeroClaimRedemptionCollateralV2,
    BoundClaimIssuanceV1, ClaimLedgerV3, ClaimRedemptionCollateralRequestV2, CollateralBackingV2,
    CustodyTransferKindV2, Error, FractionalClaimLedgerPlanV3, FractionalClaimSupplyMutationV3,
    HoardV2, Id, MarketLiabilityLifecycleV1, ResolutionStateV5, ResolutionV5, Result,
};

/// Canonical bearer redemption transition domain.
pub const BEARER_CLAIM_REDEMPTION_TRANSITION_DOMAIN_V3: &[u8] =
    b"dragons-clutch/bearer-claim/redemption-transition/v3\0";
/// Accepted exact bearer burn receipt domain.
pub const BEARER_CLAIM_BURN_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/bearer-claim/burn-receipt/v3\0";
/// Accepted Fractional-owned bearer burn receipt domain.
pub const FRACTIONAL_BEARER_CLAIM_BURN_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/bearer-claim/fractional-burn-receipt/v3\0";
/// Atomic bearer-burn/collateral-payout receipt domain.
pub const BEARER_CLAIM_REDEMPTION_RECEIPT_DOMAIN_V3: &[u8] =
    b"dragons-clutch/bearer-claim/redemption-receipt/v3\0";

/// Runtime-authenticated selected mint and bearer source observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdapterBearerClaimObservationV3 {
    /// Exact selected outcome mint.
    pub mint: Id,
    /// Program-owned authority retained in the selected mint bytes.
    pub mint_authority: Id,
    /// Exact bearer source token account.
    pub source_token_account: Id,
    /// Current source owner retained in hostile token-account bytes.
    pub source_owner: Id,
    /// Selected mint supply.
    pub mint_supply_atoms: u64,
    /// Source bearer balance.
    pub source_atoms: u64,
}

/// Sole Token-2022 burn authorized by a prepared bearer redemption.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BearerClaimBurnIntentV3 {
    /// Exact selected outcome mint.
    pub mint: Id,
    /// Exact bearer source token account.
    pub source_token_account: Id,
    /// Claimant signer that owns the source bytes.
    pub claimant: Id,
    /// Raw bearer claim atoms to burn.
    pub quantity: u64,
}

/// Prepared Token-2022 burn bound to one private canonical Fractional
/// ClaimLedger/0xa5 successor. This value exposes no collateral request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedFractionalBearerClaimBurnV3 {
    claim_binding_id: Id,
    fractional: FractionalClaimLedgerPlanV3,
    claimant: Id,
    outcome: u8,
    quantity: u64,
    observed_materialized_before: [u64; MAX_OUTCOMES],
    expected_materialized_after: [u64; MAX_OUTCOMES],
    token_before: AdapterBearerClaimObservationV3,
    burn_intent: BearerClaimBurnIntentV3,
}

impl PreparedFractionalBearerClaimBurnV3 {
    /// Sole independently released Token-2022 burn admitted by this plan.
    pub const fn burn_intent(self) -> BearerClaimBurnIntentV3 {
        self.burn_intent
    }
}

/// Exact accepted Token-2022 burn bound to one Fractional transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedFractionalBearerClaimBurnV3 {
    prepared: PreparedFractionalBearerClaimBurnV3,
    token_after: AdapterBearerClaimObservationV3,
    burn_receipt_id: Id,
}

impl AcceptedFractionalBearerClaimBurnV3 {
    /// Exact Token-2022 mint/source/authority/amount accepted after reload.
    pub const fn burn_intent(self) -> BearerClaimBurnIntentV3 {
        self.prepared.burn_intent
    }

    /// Exact Fractional ClaimLedger/0xa5 transition authenticated by the burn.
    pub const fn fractional(self) -> FractionalClaimLedgerPlanV3 {
        self.prepared.fractional
    }

    /// Exact accepted claim-issuance binding.
    pub const fn claim_binding_id(self) -> Id {
        self.prepared.claim_binding_id
    }

    /// Exact claimant whose bearer balance authorized the burn.
    pub const fn claimant(self) -> Id {
        self.prepared.claimant
    }

    /// Exact accepted burn receipt.
    pub const fn burn_receipt_id(self) -> Id {
        self.burn_receipt_id
    }

    /// Exact selected outcome.
    pub const fn outcome(self) -> u8 {
        self.prepared.outcome
    }

    /// Exact bearer quantity destroyed.
    pub const fn quantity(self) -> u64 {
        self.prepared.quantity
    }

    /// Exact selected mint/source observation accepted after the burn.
    pub const fn token_after(self) -> AdapterBearerClaimObservationV3 {
        self.token_after
    }
}

/// Prepare only the bearer burn owned by the independent claim release.
///
/// The private Fractional plan proves which materialized ClaimLedger successor
/// the burn must realize. This adapter never computes payout arithmetic and
/// never becomes an alternative owner of the ClaimLedger successor.
#[allow(clippy::too_many_arguments)]
pub fn prepare_fractional_bearer_claim_burn_v3(
    claim: BoundClaimIssuanceV1,
    expected_mint_authority: Id,
    claimant: Id,
    outcome: u8,
    quantity: u64,
    observed_materialized_before: [u64; MAX_OUTCOMES],
    token_before: AdapterBearerClaimObservationV3,
    fractional: FractionalClaimLedgerPlanV3,
) -> Result<PreparedFractionalBearerClaimBurnV3> {
    claim.binding().validate()?;
    expected_mint_authority.require_live()?;
    claimant.require_live()?;
    let FractionalClaimSupplyMutationV3::BurnMaterialized {
        outcome: planned_outcome,
        amount: planned_amount,
        observed_before: planned_observed,
    } = fractional.supply_mutation()
    else {
        return Err(Error::MismatchedBinding);
    };
    if outcome != planned_outcome
        || quantity != planned_amount
        || observed_materialized_before != planned_observed
        || outcome >= fractional.claim_ledger_after().outcome_count
        || quantity == 0
        || token_before.mint.is_zero()
        || token_before.source_token_account.is_zero()
        || token_before.mint == token_before.source_token_account
        || token_before.mint_authority != expected_mint_authority
        || token_before.source_owner != claimant
        || token_before.source_atoms < quantity
        || token_before.mint_supply_atoms != observed_materialized_before[usize::from(outcome)]
    {
        return Err(Error::MismatchedBinding);
    }
    let mut expected_materialized_after = observed_materialized_before;
    expected_materialized_after[usize::from(outcome)] = expected_materialized_after
        [usize::from(outcome)]
    .checked_sub(quantity)
    .ok_or(Error::AggregateLiabilityInsufficient)?;
    if fractional
        .claim_ledger_after()
        .aggregate_materialized_supply
        != expected_materialized_after
    {
        return Err(Error::MismatchedBinding);
    }
    Ok(PreparedFractionalBearerClaimBurnV3 {
        claim_binding_id: claim.binding_id(),
        fractional,
        claimant,
        outcome,
        quantity,
        observed_materialized_before,
        expected_materialized_after,
        token_before,
        burn_intent: BearerClaimBurnIntentV3 {
            mint: token_before.mint,
            source_token_account: token_before.source_token_account,
            claimant,
            quantity,
        },
    })
}

/// Accept the exact selected mint/source delta after the Fractional bearer burn.
pub fn accept_fractional_bearer_claim_burn_v3(
    prepared: PreparedFractionalBearerClaimBurnV3,
    observed_materialized_after: [u64; MAX_OUTCOMES],
    token_after: AdapterBearerClaimObservationV3,
) -> Result<AcceptedFractionalBearerClaimBurnV3> {
    if observed_materialized_after != prepared.expected_materialized_after
        || token_after.mint != prepared.token_before.mint
        || token_after.mint_authority != prepared.token_before.mint_authority
        || token_after.source_token_account != prepared.token_before.source_token_account
        || token_after.source_owner != prepared.token_before.source_owner
        || token_after.mint_supply_atoms
            != observed_materialized_after[usize::from(prepared.outcome)]
        || token_after.source_atoms
            != prepared
                .token_before
                .source_atoms
                .checked_sub(prepared.quantity)
                .ok_or(Error::PostAdmissionFailed)?
    {
        return Err(Error::PostAdmissionFailed);
    }
    let observed_before = encode_amounts(prepared.observed_materialized_before);
    let observed_after = encode_amounts(observed_materialized_after);
    let burn_receipt_id = digest(
        FRACTIONAL_BEARER_CLAIM_BURN_RECEIPT_DOMAIN_V3,
        &[
            &prepared.fractional.transition_id().bytes(),
            &prepared.claim_binding_id.bytes(),
            &prepared.token_before.mint.bytes(),
            &prepared.token_before.source_token_account.bytes(),
            &prepared.claimant.bytes(),
            &observed_before,
            &observed_after,
            &[prepared.outcome],
            &prepared.quantity.to_le_bytes(),
            &token_after.source_atoms.to_le_bytes(),
        ],
    );
    burn_receipt_id.require_live()?;
    Ok(AcceptedFractionalBearerClaimBurnV3 {
        prepared,
        token_after,
        burn_receipt_id,
    })
}

/// Prepared semantic successors whose collateral request remains private until
/// the exact bearer burn is accepted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedBearerClaimRedemptionV3 {
    claim_binding_id: Id,
    resolution_account: Id,
    resolution_semantic_id: Id,
    resolution_data_id: Id,
    claimant: Id,
    destination_token_account: Id,
    outcome: u8,
    quantity: u64,
    payout_atoms: u64,
    observed_materialized_before: [u64; MAX_OUTCOMES],
    expected_materialized_after: [u64; MAX_OUTCOMES],
    token_before: AdapterBearerClaimObservationV3,
    burn_intent: BearerClaimBurnIntentV3,
    claim_ledger_before_id: Id,
    claim_ledger_after: ClaimLedgerV3,
    claim_ledger_after_id: Id,
    hoard_before_id: Id,
    hoard_after: HoardV2,
    hoard_after_id: Id,
    collateral_request: ClaimRedemptionCollateralRequestV2,
    transition_id: Id,
}

impl PreparedBearerClaimRedemptionV3 {
    /// Sole Token-2022 burn this semantic transition permits.
    pub const fn burn_intent(self) -> BearerClaimBurnIntentV3 {
        self.burn_intent
    }

    /// Canonical transition identity that becomes the collateral request ID.
    pub const fn transition_id(self) -> Id {
        self.transition_id
    }
}

/// Accepted exact Token-2022 burn. It exposes the collateral request but not
/// publishable Hoard or ClaimLedger state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedBearerClaimBurnV3 {
    prepared: PreparedBearerClaimRedemptionV3,
    token_after: AdapterBearerClaimObservationV3,
    burn_receipt_id: Id,
}

impl AcceptedBearerClaimBurnV3 {
    /// Exact request that may now prepare the zero/nonzero collateral branch.
    pub const fn collateral_request(self) -> ClaimRedemptionCollateralRequestV2 {
        self.prepared.collateral_request
    }

    /// Exact accepted bearer-burn receipt.
    pub const fn burn_receipt_id(self) -> Id {
        self.burn_receipt_id
    }
}

/// Accepted Realm-selected collateral result after an accepted bearer burn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptedBearerRedemptionCollateralV3 {
    /// Exact nonmutation proof for an exact zero payout.
    Zero(AcceptedZeroClaimRedemptionCollateralV2),
    /// Exact Hoard debit and destination credit for a nonzero payout.
    Nonzero(AcceptedClaimRedemptionCollateralV2),
}

/// Fully accepted burn, collateral disposition, and canonical ledger states.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceptedBearerClaimRedemptionV3 {
    burn: AcceptedBearerClaimBurnV3,
    collateral_receipt_id: Id,
    receipt_id: Id,
}

impl AcceptedBearerClaimRedemptionV3 {
    /// ClaimLedger semantic prestate before direct-burn synchronization.
    pub const fn claim_ledger_before_id(self) -> Id {
        self.burn.prepared.claim_ledger_before_id
    }

    /// Complete canonical ClaimLedger successor.
    pub const fn claim_ledger_after(self) -> ClaimLedgerV3 {
        self.burn.prepared.claim_ledger_after
    }

    /// Exact ClaimLedger successor identity.
    pub const fn claim_ledger_after_id(self) -> Id {
        self.burn.prepared.claim_ledger_after_id
    }

    /// Hoard semantic prestate before releasing locked principal.
    pub const fn hoard_before_id(self) -> Id {
        self.burn.prepared.hoard_before_id
    }

    /// Complete canonical Hoard successor.
    pub const fn hoard_after(self) -> HoardV2 {
        self.burn.prepared.hoard_after
    }

    /// Exact Hoard successor identity.
    pub const fn hoard_after_id(self) -> Id {
        self.burn.prepared.hoard_after_id
    }

    /// Exact collateral atoms released from locked principal.
    pub const fn payout_atoms(self) -> u64 {
        self.burn.prepared.payout_atoms
    }

    /// Canonical semantic transition identity.
    pub const fn transition_id(self) -> Id {
        self.burn.prepared.transition_id
    }

    /// Exact accepted Token-2022 burn receipt.
    pub const fn burn_receipt_id(self) -> Id {
        self.burn.burn_receipt_id
    }

    /// Exact zero/nonzero collateral receipt.
    pub const fn collateral_receipt_id(self) -> Id {
        self.collateral_receipt_id
    }

    /// Atomic bearer redemption receipt.
    pub const fn receipt_id(self) -> Id {
        self.receipt_id
    }
}

/// Prepare exact bearer and liability postimages after every active outcome
/// mint has been authenticated and observed.
#[allow(clippy::too_many_arguments)]
pub fn prepare_bearer_claim_redemption_v3<B: PositionV3Sha256Backend>(
    claim: BoundClaimIssuanceV1,
    resolution_account: Id,
    resolution: ResolutionV5,
    expected_mint_authority: Id,
    hoard: HoardV2,
    claim_ledger: ClaimLedgerV3,
    claimant: Id,
    destination_token_account: Id,
    outcome: u8,
    quantity: u64,
    observed_materialized_supply: [u64; MAX_OUTCOMES],
    token_before: AdapterBearerClaimObservationV3,
    backend: &B,
) -> Result<PreparedBearerClaimRedemptionV3> {
    claim.binding().validate()?;
    resolution_account.require_live()?;
    expected_mint_authority.require_live()?;
    claimant.require_live()?;
    destination_token_account.require_live()?;
    resolution.validate()?;
    hoard.validate()?;
    claim_ledger.validate()?;
    if resolution.state != ResolutionStateV5::Finalized
        || hoard.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.lifecycle != MarketLiabilityLifecycleV1::Resolved
        || claim_ledger.resolution_account != resolution_account
        || resolution.facts.market_instance_id != hoard.market_instance_id
        || resolution.facts.market_instance_id != claim_ledger.market_instance_id
        || resolution.facts.native_claim_basis_id != claim_ledger.native_claim_basis_id
        || resolution.facts.outcome_count != hoard.outcome_count
        || resolution.facts.outcome_count != claim_ledger.outcome_count
        || hoard.realm_id != claim_ledger.realm_id
        || outcome >= resolution.facts.outcome_count
        || quantity == 0
        || token_before.mint.is_zero()
        || token_before.source_token_account.is_zero()
        || token_before.mint == token_before.source_token_account
        || token_before.mint_authority != expected_mint_authority
        || token_before.source_owner != claimant
        || token_before.source_atoms < quantity
        || token_before.mint_supply_atoms != observed_materialized_supply[usize::from(outcome)]
    {
        return Err(Error::MismatchedBinding);
    }

    let claim_ledger_before_id = claim_ledger.semantic_id(backend)?;
    let hoard_before_id = hoard.semantic_id(backend)?;
    let resolution_semantic_id = resolution.semantic_id(backend)?;
    let resolution_data_id = resolution.data_id(resolution_account)?;
    let mut synchronized_materialized = observed_materialized_supply;
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        if index < usize::from(claim_ledger.outcome_count) {
            if synchronized_materialized[index] > claim_ledger.aggregate_materialized_supply[index]
            {
                return Err(Error::AggregateLiabilityInsufficient);
            }
        } else if synchronized_materialized[index] != 0 {
            return Err(Error::NonCanonicalPadding);
        }
        index += 1;
    }
    let selected = usize::from(outcome);
    synchronized_materialized[selected] = synchronized_materialized[selected]
        .checked_sub(quantity)
        .ok_or(Error::AggregateLiabilityInsufficient)?;
    let claim_ledger_after = ClaimLedgerV3 {
        aggregate_materialized_supply: synchronized_materialized,
        ..claim_ledger
    };
    claim_ledger_after.validate()?;
    let claim_ledger_after_id = claim_ledger_after.semantic_id(backend)?;

    let payout_atoms = resolution.payout_atoms(outcome, quantity)?;
    let locked_claim_principal_atoms = hoard
        .locked_claim_principal_atoms
        .checked_sub(payout_atoms)
        .ok_or(Error::AggregateLiabilityInsufficient)?;
    let hoard_after = HoardV2 {
        locked_claim_principal_atoms,
        ..hoard
    };
    hoard_after.validate()?;
    let hoard_after_id = hoard_after.semantic_id(backend)?;
    let observed_before_bytes = encode_amounts(observed_materialized_supply);
    let transition_id = digest(
        BEARER_CLAIM_REDEMPTION_TRANSITION_DOMAIN_V3,
        &[
            &claim.binding_id().bytes(),
            &resolution_account.bytes(),
            &resolution_semantic_id.bytes(),
            &resolution_data_id.bytes(),
            &resolution.facts.market_instance_id.bytes(),
            &resolution.facts.native_claim_basis_id.bytes(),
            &claim_ledger_before_id.bytes(),
            &claim_ledger_after_id.bytes(),
            &hoard_before_id.bytes(),
            &hoard_after_id.bytes(),
            &claimant.bytes(),
            &token_before.source_token_account.bytes(),
            &destination_token_account.bytes(),
            &token_before.mint.bytes(),
            &observed_before_bytes,
            &[outcome],
            &quantity.to_le_bytes(),
            &payout_atoms.to_le_bytes(),
            &[resolution.facts.payout_unit_boundary as u8],
        ],
    );
    transition_id.require_live()?;
    let collateral_request = ClaimRedemptionCollateralRequestV2 {
        claim_redemption_id: transition_id,
        destination_token_account,
        claim_semantic_owner: claimant,
        payout_atoms,
        backing_before: CollateralBackingV2 {
            locked_atoms: hoard.locked_claim_principal_atoms,
            cap_atoms: hoard.collateral_cap_atoms,
        },
    };
    Ok(PreparedBearerClaimRedemptionV3 {
        claim_binding_id: claim.binding_id(),
        resolution_account,
        resolution_semantic_id,
        resolution_data_id,
        claimant,
        destination_token_account,
        outcome,
        quantity,
        payout_atoms,
        observed_materialized_before: observed_materialized_supply,
        expected_materialized_after: synchronized_materialized,
        token_before,
        burn_intent: BearerClaimBurnIntentV3 {
            mint: token_before.mint,
            source_token_account: token_before.source_token_account,
            claimant,
            quantity,
        },
        claim_ledger_before_id,
        claim_ledger_after,
        claim_ledger_after_id,
        hoard_before_id,
        hoard_after,
        hoard_after_id,
        collateral_request,
        transition_id,
    })
}

/// Accept the exact Token-2022 source and mint deltas before releasing the
/// collateral request.
pub fn accept_bearer_claim_burn_v3(
    prepared: PreparedBearerClaimRedemptionV3,
    observed_materialized_after: [u64; MAX_OUTCOMES],
    token_after: AdapterBearerClaimObservationV3,
) -> Result<AcceptedBearerClaimBurnV3> {
    if observed_materialized_after != prepared.expected_materialized_after
        || token_after.mint != prepared.token_before.mint
        || token_after.mint_authority != prepared.token_before.mint_authority
        || token_after.source_token_account != prepared.token_before.source_token_account
        || token_after.source_owner != prepared.token_before.source_owner
        || token_after.mint_supply_atoms
            != observed_materialized_after[usize::from(prepared.outcome)]
        || token_after.source_atoms
            != prepared
                .token_before
                .source_atoms
                .checked_sub(prepared.quantity)
                .ok_or(Error::PostAdmissionFailed)?
    {
        return Err(Error::PostAdmissionFailed);
    }
    let observed_after_bytes = encode_amounts(observed_materialized_after);
    let burn_receipt_id = digest(
        BEARER_CLAIM_BURN_RECEIPT_DOMAIN_V3,
        &[
            &prepared.transition_id.bytes(),
            &prepared.claim_binding_id.bytes(),
            &prepared.resolution_data_id.bytes(),
            &prepared.token_before.mint.bytes(),
            &prepared.token_before.source_token_account.bytes(),
            &prepared.claimant.bytes(),
            &prepared.observed_materialized_before[usize::from(prepared.outcome)].to_le_bytes(),
            &observed_after_bytes,
            &prepared.token_before.source_atoms.to_le_bytes(),
            &token_after.source_atoms.to_le_bytes(),
        ],
    );
    burn_receipt_id.require_live()?;
    Ok(AcceptedBearerClaimBurnV3 {
        prepared,
        token_after,
        burn_receipt_id,
    })
}

/// Accept the exact zero/nonzero collateral branch and finally release both
/// canonical liability successors.
pub fn accept_bearer_claim_redemption_v3(
    burn: AcceptedBearerClaimBurnV3,
    collateral: AcceptedBearerRedemptionCollateralV3,
) -> Result<AcceptedBearerClaimRedemptionV3> {
    let expected_request = burn.prepared.collateral_request;
    let required_custody = burn.prepared.hoard_after.required_custody_atoms()?;
    let collateral_receipt_id = match collateral {
        AcceptedBearerRedemptionCollateralV3::Zero(accepted) => {
            if burn.prepared.payout_atoms != 0
                || accepted.request() != expected_request
                || accepted.backing_after()
                    != (CollateralBackingV2 {
                        locked_atoms: burn.prepared.hoard_after.locked_claim_principal_atoms,
                        cap_atoms: burn.prepared.hoard_after.collateral_cap_atoms,
                    })
                || accepted.visible_hoard_atoms_after() < required_custody
            {
                return Err(Error::PostAdmissionFailed);
            }
            accepted.receipt_id()
        }
        AcceptedBearerRedemptionCollateralV3::Nonzero(accepted) => {
            let custody = accepted.custody();
            if burn.prepared.payout_atoms == 0
                || accepted.request() != expected_request
                || accepted.backing_after()
                    != (CollateralBackingV2 {
                        locked_atoms: burn.prepared.hoard_after.locked_claim_principal_atoms,
                        cap_atoms: burn.prepared.hoard_after.collateral_cap_atoms,
                    })
                || custody.kind != CustodyTransferKindV2::ClaimRedemption
                || custody.source_semantic_owner != burn.prepared.hoard_after.market_instance_id
                || custody.destination_semantic_owner != burn.prepared.claimant
                || custody.amount_atoms != burn.prepared.payout_atoms
                || custody
                    .hoard_atoms_after
                    .ok_or(Error::PostAdmissionFailed)?
                    < required_custody
            {
                return Err(Error::PostAdmissionFailed);
            }
            accepted.receipt_id()
        }
    };
    let receipt_id = digest(
        BEARER_CLAIM_REDEMPTION_RECEIPT_DOMAIN_V3,
        &[
            &burn.prepared.transition_id.bytes(),
            &burn.burn_receipt_id.bytes(),
            &collateral_receipt_id.bytes(),
            &burn.prepared.resolution_account.bytes(),
            &burn.prepared.resolution_semantic_id.bytes(),
            &burn.prepared.resolution_data_id.bytes(),
            &burn.prepared.claim_ledger_before_id.bytes(),
            &burn.prepared.claim_ledger_after_id.bytes(),
            &burn.prepared.hoard_before_id.bytes(),
            &burn.prepared.hoard_after_id.bytes(),
            &burn.prepared.destination_token_account.bytes(),
            &burn.prepared.payout_atoms.to_le_bytes(),
            &burn.token_after.mint_supply_atoms.to_le_bytes(),
        ],
    );
    receipt_id.require_live()?;
    Ok(AcceptedBearerClaimRedemptionV3 {
        burn,
        collateral_receipt_id,
        receipt_id,
    })
}

fn encode_amounts(values: [u64; MAX_OUTCOMES]) -> [u8; MAX_OUTCOMES * 8] {
    let mut bytes = [0; MAX_OUTCOMES * 8];
    let mut index = 0usize;
    while index < MAX_OUTCOMES {
        let at = index * 8;
        bytes[at..at + 8].copy_from_slice(&values[index].to_le_bytes());
        index += 1;
    }
    bytes
}
