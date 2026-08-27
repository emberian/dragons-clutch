//! Composition-admitted compact Trading/Claims lifecycle construction.

use dclutch_rational_lifecycle_hot_v3::{
    RationalLifecycleHotInstructionV3, RationalLifecycleHotStateV3,
    RationalLifecycleSelectedSelectionV5, build_rational_lifecycle_selected_hot_instruction_v5,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleCoordinateV2, LifecycleHeaderV2, LifecycleRequestV2,
    hot_v3::RationalLifecycleHotRequestV3,
};
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey};

use crate::{
    ClaimsLifecyclePlanV3, CompositionAdmissionPlanV3, Error, FinalizedCoordinateV3, Result,
    build_claims_lifecycle_plan_v3,
};

/// Complete unsigned compact-Hot plan joined to finalized composition records.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompositionLifecycleHotPlanV3 {
    /// Exact semantic-owner Claims lifecycle request and child geometry.
    pub lifecycle: ClaimsLifecyclePlanV3,
    /// Packet-oriented common Trading Hot instruction.
    pub hot: RationalLifecycleHotInstructionV3,
    /// Finalized rational execution descriptor selected by Claims.
    pub execution_descriptor: FinalizedCoordinateV3,
    /// Finalized Product-to-representation exposure record.
    pub exposure: FinalizedCoordinateV3,
    /// Claims/native representation width `K`.
    pub representation_width: u32,
    /// Independently authenticated Product terminal width `N`.
    pub product_width: u32,
}

/// Build one unsigned selected compact-Hot lifecycle instruction.
///
/// The function accepts no caller-carried K, N, descriptor, exposure, or
/// Claims request DTO. Those facts come from [`CompositionAdmissionPlanV3`],
/// while the lifecycle contract and selected Hot bundle independently rerun
/// the exact descriptor/request/account joins. The returned instruction does
/// not sign or submit anything.
pub fn build_composition_lifecycle_hot_plan_v3(
    admission: CompositionAdmissionPlanV3<'_>,
    state: &RationalLifecycleHotStateV3<'_>,
    header: LifecycleHeaderV2,
    coordinates: &[LifecycleCoordinateV2],
    claims_program: Pubkey,
    claims_accounts: &[solana_program::instruction::AccountMeta],
    selection: RationalLifecycleSelectedSelectionV5<'_>,
) -> Result<CompositionLifecycleHotPlanV3> {
    let admitted = admission.admitted();
    let descriptor = admitted.execution_descriptor();
    if claims_program == Pubkey::default()
        || claims_program != admitted.claims_program()
        || admitted.observation().slot != state.finalized_slot
        || descriptor.market_id() != state.market.to_bytes()
        || descriptor.release_set_id() != state.release_set
        || descriptor.descriptor_id() != selection.bundle.representation_descriptor_id
        || descriptor.descriptor_id() != selection.authenticated_token_behavior.descriptor_id()
        || descriptor.token_program() != selection.bundle.token_program
        || descriptor.outcome_count() != admitted.representation_width()
        || admitted.product_width() == 0
        || header.parent_context != [0; 32]
    {
        return Err(Error::HotAdapter);
    }
    let lifecycle = derive_hot_parent_and_build_lifecycle(admitted, header, coordinates)?;
    if claims_accounts.len() != lifecycle.account_count {
        return Err(Error::HotAdapter);
    }
    let child = Instruction {
        program_id: claims_program,
        accounts: claims_accounts.to_vec(),
        data: lifecycle.request.clone(),
    };
    let hot = build_rational_lifecycle_selected_hot_instruction_v5(state, &child, selection)
        .map_err(|_| Error::HotAdapter)?;
    if !hot.required_wallet_signers.is_empty()
        || hot.finalized_slot != admitted.observation().slot
        || hot.instruction.program_id == Pubkey::default()
    {
        return Err(Error::HotAdapter);
    }
    Ok(CompositionLifecycleHotPlanV3 {
        lifecycle,
        hot,
        execution_descriptor: admitted.execution_descriptor_record(),
        exposure: admitted.exposure_record(),
        representation_width: admitted.representation_width(),
        product_width: admitted.product_width(),
    })
}

fn derive_hot_parent_and_build_lifecycle(
    admitted: crate::AdmittedCompositionV3<'_>,
    mut header: LifecycleHeaderV2,
    coordinates: &[LifecycleCoordinateV2],
) -> Result<ClaimsLifecyclePlanV3> {
    header.parent_context = [1; 32];
    let provisional = build_claims_lifecycle_plan_v3(admitted, header, coordinates)?;
    let child = LifecycleRequestV2::decode(&provisional.request).map_err(|_| Error::HotAdapter)?;
    let mut family_bytes = vec![0_u8; provisional.request.len()];
    let family = RationalLifecycleHotRequestV3::from_child_into(child, &mut family_bytes)
        .map_err(|_| Error::HotAdapter)?;
    header.parent_context = hash(family.as_bytes()).to_bytes();
    build_claims_lifecycle_plan_v3(admitted, header, coordinates)
}
