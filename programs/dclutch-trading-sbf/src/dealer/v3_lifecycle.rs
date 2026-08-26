//! Canonical Dealer activation and quiescent retirement under Trading V3.
//!
//! The common state-lifecycle executor allocates/assigns the inert Trading
//! child root and obligation PDA. Claims remains the sole creator and closer
//! of the runtime-width Dealer inventory Position. This family adapter derives
//! both child requests, verifies their immediate receipts/postconditions, and
//! never introduces a Dealer registry, mint authority, or private initializer.

use dclutch_claims_svm::protocol_position_v2::{
    ProtocolPositionActionV2, ProtocolPositionAdmissionSeedsV2, ProtocolPositionAdmissionV2,
    ProtocolPositionCloseReceiptV2, ProtocolPositionOwnerKindV2, ProtocolPositionPresenceV2,
    ProtocolPositionRequestV2, ProtocolPositionSeedsV2,
};
use solana_program::{hash::hash, pubkey::Pubkey};

use super::v3_obligation::{
    DealerObligationProjectionV3, ObligationAccountObservationV3, ObligationClosePlanV3,
    ObligationExpectationV3, ObligationOpenInputV3, ObligationOpenPlanV3,
    prepare_obligation_close_v3, prepare_obligation_open_v3,
};

/// Stable refusal from Dealer activation/retirement composition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerLifecycleErrorV3 {
    /// A required immutable identity, program, or parent digest was zero.
    InvalidContext,
    /// Claims Position/admission PDAs or the Trading obligation PDA differed.
    AccountMismatch,
    /// Runtime width or rent funding was invalid.
    InvalidFunding,
    /// Canonical Claims request construction refused.
    Claims,
    /// Obligation initialization or quiescent close refused.
    Obligation,
    /// An immediate Claims receipt or write-last obligation poststate differed.
    Postcondition,
}

/// Immutable coordinates shared by activation and terminal retirement.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerLifecycleContextV3 {
    /// Current Registry-selected Trading program.
    pub trading_program: [u8; 32],
    /// Current Registry-selected Claims program.
    pub claims_program: [u8; 32],
    /// Current program owning the permanent RentCredit.
    pub rent_program: [u8; 32],
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Canonical Claims aggregate PDA selected by the Market.
    pub claims_aggregate: [u8; 32],
    /// Exact finalized Product Runtime graph digest.
    pub product_record_digest: [u8; 32],
    /// Product-owned semantic LiabilityBasis identity.
    pub semantic_basis_id: [u8; 32],
    /// Exact finalized linked-basis record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Already-assigned inert Trading child root and Position owner.
    pub child_root: [u8; 32],
    /// Digest of the exact parent family request.
    pub parent_request_digest: [u8; 32],
    /// Current Core Market generation.
    pub generation: u64,
    /// Current Claims aggregate revision, unchanged by Position lifecycle.
    pub claims_market_revision: u64,
    /// Runtime Product outcome width.
    pub outcome_count: u32,
}

/// Exact canonical Claims Position/admission account pair.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerClaimsPositionAccountsV3 {
    /// Canonical runtime-width LBV2 Position PDA.
    pub position: [u8; 32],
    /// Canonical Claims-owned admission PDA.
    pub admission: [u8; 32],
}

/// Exact prepaid Position lifecycle funding and permanent refund destination.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerClaimsPositionFundingV3 {
    /// Permanent RentCredit receiving both resources on close.
    pub rent_credit: [u8; 32],
    /// Current/prepaid Position lamports, including permitted dust.
    pub position_lamports: u64,
    /// Current/prepaid admission lamports, including permitted dust.
    pub admission_lamports: u64,
    /// Current Position rent minimum.
    pub position_rent_principal: u64,
    /// Current admission-record rent minimum.
    pub admission_rent_principal: u64,
}

/// Exact prepare→Claims CPI→write-last activation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerActivationPlanV3 {
    /// Canonical Claims admission request executed under common Trading authority.
    pub claims_request: ProtocolPositionRequestV2,
    /// Digest of the exact Claims request.
    pub claims_request_digest: [u8; 32],
    /// Canonical Claims accounts.
    pub claims_accounts: DealerClaimsPositionAccountsV3,
    /// Exact Trading obligation allocation/write candidate.
    pub obligation: ObligationOpenPlanV3,
}

/// Exact Claims close plus Trading obligation reclamation plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DealerRetirementPlanV3 {
    /// Canonical Claims terminal close request.
    pub claims_request: ProtocolPositionRequestV2,
    /// Digest of the exact Claims close request.
    pub claims_request_digest: [u8; 32],
    /// Canonical Claims accounts.
    pub claims_accounts: DealerClaimsPositionAccountsV3,
    /// Exact quiescent Trading obligation reclamation candidate.
    pub obligation: ObligationClosePlanV3,
}

/// Prepare first Dealer state after the common outer assigns the inert root.
#[allow(clippy::too_many_arguments)]
pub fn prepare_dealer_activation_v3(
    context: DealerLifecycleContextV3,
    claims_accounts: DealerClaimsPositionAccountsV3,
    funding: DealerClaimsPositionFundingV3,
    vacant_obligation: ObligationAccountObservationV3<'_>,
    obligation_output: &mut [u8],
) -> Result<DealerActivationPlanV3, DealerLifecycleErrorV3> {
    validate_context(context)?;
    validate_claims_accounts(context, claims_accounts)?;
    validate_funding(funding)?;
    let claims_request = position_request(
        context,
        funding,
        ProtocolPositionActionV2::Admit,
        ProtocolPositionPresenceV2::Vacant,
        0,
    )?;
    let request_bytes = claims_request
        .to_bytes()
        .map_err(|_| DealerLifecycleErrorV3::Claims)?;
    let obligation = prepare_obligation_open_v3(
        context.trading_program,
        vacant_obligation,
        ObligationOpenInputV3 {
            market: context.market,
            product: context.product_record_digest,
            liability_basis: context.semantic_basis_id,
            position_owner: context.child_root,
            child_root: context.child_root,
            width: context.outcome_count,
        },
        obligation_output,
    )
    .map_err(|_| DealerLifecycleErrorV3::Obligation)?;
    Ok(DealerActivationPlanV3 {
        claims_request,
        claims_request_digest: hash(&request_bytes).to_bytes(),
        claims_accounts,
        obligation,
    })
}

/// Verify the immediate Claims receipt and exact write-last obligation state.
pub fn verify_dealer_activation_v3(
    context: DealerLifecycleContextV3,
    plan: DealerActivationPlanV3,
    claims_receipt_bytes: &[u8],
    observed_obligation_bytes: &[u8],
) -> Result<(), DealerLifecycleErrorV3> {
    let receipt = ProtocolPositionAdmissionV2::decode_receipt(claims_receipt_bytes)
        .map_err(|_| DealerLifecycleErrorV3::Claims)?;
    receipt
        .validate_request(
            plan.claims_request,
            plan.claims_request_digest,
            context.claims_program,
            context.trading_program,
        )
        .map_err(|_| DealerLifecycleErrorV3::Postcondition)?;
    if receipt.owner_kind() != ProtocolPositionOwnerKindV2::TradingRecord
        || receipt.position_owner() != context.child_root
        || receipt.product_record_digest() != context.product_record_digest
        || receipt.semantic_basis_id() != context.semantic_basis_id
        || receipt.linked_basis_record_digest() != context.linked_basis_record_digest
        || receipt.outcome_count() != context.outcome_count
        || hash(observed_obligation_bytes).to_bytes() != plan.obligation.initial_digest
    {
        return Err(DealerLifecycleErrorV3::Postcondition);
    }
    let obligation = DealerObligationProjectionV3::decode(observed_obligation_bytes)
        .map_err(|_| DealerLifecycleErrorV3::Postcondition)?;
    if obligation.revision() != plan.obligation.initial_revision
        || obligation.width() != context.outcome_count
        || obligation.position_owner() != context.child_root
        || obligation.lp_principal() != 0
        || obligation.obligations().any(|value| value != 0)
    {
        return Err(DealerLifecycleErrorV3::Postcondition);
    }
    Ok(())
}

/// Prepare terminal Claims Position and obligation reclamation.
#[allow(clippy::too_many_arguments)]
pub fn prepare_dealer_retirement_v3(
    context: DealerLifecycleContextV3,
    claims_accounts: DealerClaimsPositionAccountsV3,
    funding: DealerClaimsPositionFundingV3,
    expected_position_revision: u64,
    obligation_observation: ObligationAccountObservationV3<'_>,
    obligation_revision: u64,
    obligation_digest: [u8; 32],
) -> Result<DealerRetirementPlanV3, DealerLifecycleErrorV3> {
    validate_context(context)?;
    validate_claims_accounts(context, claims_accounts)?;
    validate_funding(funding)?;
    if obligation_revision == 0 || obligation_digest == [0; 32] {
        return Err(DealerLifecycleErrorV3::InvalidContext);
    }
    let claims_request = position_request(
        context,
        funding,
        ProtocolPositionActionV2::Close,
        ProtocolPositionPresenceV2::Existing,
        expected_position_revision,
    )?;
    let request_bytes = claims_request
        .to_bytes()
        .map_err(|_| DealerLifecycleErrorV3::Claims)?;
    let obligation = prepare_obligation_close_v3(
        context.trading_program,
        obligation_observation,
        ObligationExpectationV3 {
            market: context.market,
            product: context.product_record_digest,
            liability_basis: context.semantic_basis_id,
            position_owner: context.child_root,
            child_root: context.child_root,
            revision: obligation_revision,
            width: context.outcome_count,
            state_digest: obligation_digest,
        },
    )
    .map_err(|_| DealerLifecycleErrorV3::Obligation)?;
    Ok(DealerRetirementPlanV3 {
        claims_request,
        claims_request_digest: hash(&request_bytes).to_bytes(),
        claims_accounts,
        obligation,
    })
}

/// Verify the immediate Claims close receipt before common resource closure.
pub fn verify_dealer_retirement_v3(
    context: DealerLifecycleContextV3,
    plan: DealerRetirementPlanV3,
    claims_receipt_bytes: &[u8],
) -> Result<(), DealerLifecycleErrorV3> {
    let receipt = ProtocolPositionCloseReceiptV2::decode(claims_receipt_bytes)
        .map_err(|_| DealerLifecycleErrorV3::Claims)?;
    receipt
        .validate_request(
            plan.claims_request,
            plan.claims_request_digest,
            context.claims_program,
        )
        .map_err(|_| DealerLifecycleErrorV3::Postcondition)
}

fn position_request(
    context: DealerLifecycleContextV3,
    funding: DealerClaimsPositionFundingV3,
    action: ProtocolPositionActionV2,
    presence: ProtocolPositionPresenceV2,
    expected_position_revision: u64,
) -> Result<ProtocolPositionRequestV2, DealerLifecycleErrorV3> {
    ProtocolPositionRequestV2 {
        action,
        owner_kind: ProtocolPositionOwnerKindV2::TradingRecord,
        presence,
        release_set: context.release_set,
        market: context.market,
        position_owner: context.child_root,
        parent_request_digest: context.parent_request_digest,
        rent_credit: funding.rent_credit,
        rent_program: context.rent_program,
        generation: context.generation,
        expected_market_revision: context.claims_market_revision,
        expected_position_revision,
        observed_position_lamports: funding.position_lamports,
        observed_admission_lamports: funding.admission_lamports,
        position_rent_principal: funding.position_rent_principal,
        admission_rent_principal: funding.admission_rent_principal,
        capability_descriptor: [0; 32],
        capability_outcome: 0,
    }
    .new()
    .map_err(|_| DealerLifecycleErrorV3::Claims)
}

fn validate_context(context: DealerLifecycleContextV3) -> Result<(), DealerLifecycleErrorV3> {
    for identity in [
        context.trading_program,
        context.claims_program,
        context.rent_program,
        context.release_set,
        context.market,
        context.claims_aggregate,
        context.product_record_digest,
        context.semantic_basis_id,
        context.linked_basis_record_digest,
        context.child_root,
        context.parent_request_digest,
    ] {
        if identity == [0; 32] {
            return Err(DealerLifecycleErrorV3::InvalidContext);
        }
    }
    if context.generation == 0
        || context.outcome_count == 0
        || context.child_root == context.market
        || context.child_root == context.claims_aggregate
    {
        return Err(DealerLifecycleErrorV3::InvalidContext);
    }
    Ok(())
}

fn validate_claims_accounts(
    context: DealerLifecycleContextV3,
    accounts: DealerClaimsPositionAccountsV3,
) -> Result<(), DealerLifecycleErrorV3> {
    let claims = Pubkey::new_from_array(context.claims_program);
    let position = ProtocolPositionSeedsV2::new(context.claims_aggregate, context.child_root)
        .map_err(|_| DealerLifecycleErrorV3::Claims)?;
    let admission =
        ProtocolPositionAdmissionSeedsV2::new(context.claims_aggregate, context.child_root)
            .map_err(|_| DealerLifecycleErrorV3::Claims)?;
    let expected_position = Pubkey::find_program_address(&position.as_slices(), &claims)
        .0
        .to_bytes();
    let expected_admission = Pubkey::find_program_address(&admission.as_slices(), &claims)
        .0
        .to_bytes();
    if accounts.position != expected_position
        || accounts.admission != expected_admission
        || accounts.position == accounts.admission
    {
        return Err(DealerLifecycleErrorV3::AccountMismatch);
    }
    Ok(())
}

fn validate_funding(funding: DealerClaimsPositionFundingV3) -> Result<(), DealerLifecycleErrorV3> {
    if funding.rent_credit == [0; 32]
        || funding.position_rent_principal == 0
        || funding.admission_rent_principal == 0
        || funding.position_lamports < funding.position_rent_principal
        || funding.admission_lamports < funding.admission_rent_principal
    {
        return Err(DealerLifecycleErrorV3::InvalidFunding);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dealer::v3_obligation::{
        DEALER_OBLIGATION_PDA_DOMAIN_V3, obligation_account_bytes_v3,
    };
    use dclutch_claims_svm::protocol_position_v2::{
        ProtocolPositionAdmissionEvidenceV2, ProtocolPositionCloseEvidenceV2,
    };

    fn context() -> DealerLifecycleContextV3 {
        DealerLifecycleContextV3 {
            trading_program: [1; 32],
            claims_program: [2; 32],
            rent_program: [3; 32],
            release_set: [4; 32],
            market: [5; 32],
            claims_aggregate: [6; 32],
            product_record_digest: [7; 32],
            semantic_basis_id: [8; 32],
            linked_basis_record_digest: [9; 32],
            child_root: [10; 32],
            parent_request_digest: [11; 32],
            generation: 4,
            claims_market_revision: 0,
            outcome_count: 3,
        }
    }

    fn accounts(context: DealerLifecycleContextV3) -> DealerClaimsPositionAccountsV3 {
        let claims = Pubkey::new_from_array(context.claims_program);
        let position = ProtocolPositionSeedsV2::new(context.claims_aggregate, context.child_root)
            .expect("position seeds");
        let admission =
            ProtocolPositionAdmissionSeedsV2::new(context.claims_aggregate, context.child_root)
                .expect("admission seeds");
        DealerClaimsPositionAccountsV3 {
            position: Pubkey::find_program_address(&position.as_slices(), &claims)
                .0
                .to_bytes(),
            admission: Pubkey::find_program_address(&admission.as_slices(), &claims)
                .0
                .to_bytes(),
        }
    }

    fn funding() -> DealerClaimsPositionFundingV3 {
        DealerClaimsPositionFundingV3 {
            rent_credit: [12; 32],
            position_lamports: 100,
            admission_lamports: 200,
            position_rent_principal: 80,
            admission_rent_principal: 150,
        }
    }

    #[test]
    fn activation_and_retirement_join_claims_and_obligation() {
        let context = context();
        let accounts = accounts(context);
        let obligation_address = Pubkey::find_program_address(
            &[DEALER_OBLIGATION_PDA_DOMAIN_V3, &context.child_root],
            &Pubkey::new_from_array(context.trading_program),
        )
        .0
        .to_bytes();
        let mut obligation = std::vec![
            0;
            obligation_account_bytes_v3(context.outcome_count).expect("width")
        ];
        let activation = prepare_dealer_activation_v3(
            context,
            accounts,
            funding(),
            ObligationAccountObservationV3 {
                address: obligation_address,
                owner: solana_system_interface::program::ID.to_bytes(),
                data: &[],
            },
            &mut obligation,
        )
        .expect("activation");
        assert_eq!(activation.obligation.obligation, obligation_address);
        assert_eq!(activation.claims_request.position_owner, context.child_root);
        let admission = ProtocolPositionAdmissionV2::new(
            activation.claims_request,
            ProtocolPositionAdmissionEvidenceV2 {
                product_record_digest: context.product_record_digest,
                semantic_basis_id: context.semantic_basis_id,
                linked_basis_record_digest: context.linked_basis_record_digest,
                request_digest: activation.claims_request_digest,
                claims_program: context.claims_program,
                trading_program: context.trading_program,
                capability_descriptor: [0; 32],
                capability_outcome: 0,
                outcome_count: context.outcome_count,
            },
        )
        .expect("admission");
        verify_dealer_activation_v3(
            context,
            activation,
            &admission.to_receipt_bytes().expect("receipt"),
            &obligation,
        )
        .expect("postconditions");

        let retirement = prepare_dealer_retirement_v3(
            context,
            accounts,
            funding(),
            0,
            ObligationAccountObservationV3 {
                address: obligation_address,
                owner: context.trading_program,
                data: &obligation,
            },
            1,
            hash(&obligation).to_bytes(),
        )
        .expect("quiescent retirement");
        let close = ProtocolPositionCloseReceiptV2::new(
            retirement.claims_request,
            ProtocolPositionCloseEvidenceV2 {
                request_digest: retirement.claims_request_digest,
                admission_digest: hash(&admission.to_state_bytes().expect("state")).to_bytes(),
                claims_program: context.claims_program,
                post_resource_digest: [14; 32],
                rent_credit_before: 50,
                rent_credit_after: 350,
            },
        )
        .expect("close receipt");
        verify_dealer_retirement_v3(context, retirement, &close.to_bytes().expect("close bytes"))
            .expect("retirement receipt");
    }

    #[test]
    fn substituted_position_refuses_before_obligation_output() {
        let context = context();
        let mut bad_accounts = accounts(context);
        bad_accounts.position[0] ^= 1;
        let mut output = std::vec![
            0xa5;
            obligation_account_bytes_v3(context.outcome_count).expect("width")
        ];
        assert_eq!(
            prepare_dealer_activation_v3(
                context,
                bad_accounts,
                funding(),
                ObligationAccountObservationV3 {
                    address: [1; 32],
                    owner: solana_system_interface::program::ID.to_bytes(),
                    data: &[],
                },
                &mut output,
            ),
            Err(DealerLifecycleErrorV3::AccountMismatch)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
}
