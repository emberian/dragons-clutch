//! Market-neutral V6 composition-admitted Trading/Claims construction.

use dclutch_rational_lifecycle_hot_v3::{
    RationalLifecycleHotStateV3, RationalLifecycleSelectedSelectionV6,
    build_rational_lifecycle_selected_hot_instruction_v6,
};
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleCoordinateV2, LifecycleHeaderV2, LifecycleRequestV2,
    hot_v6::RationalLifecycleHotRequestV6,
};
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey};

use crate::{
    ClaimsLifecyclePlanV3, CompositionAdmissionPlanV3, Error, Result,
    build_claims_lifecycle_plan_v3, hot_v3::CompositionLifecycleHotPlanV3,
};

/// Build one unsigned composition-admitted V6 Hot lifecycle instruction.
///
/// The selected capability is Market-neutral. The exact finalized per-Market
/// representation descriptor comes only from `admission` and must be the same
/// descriptor passed to the selected V6 runtime join. The Claims request is
/// derived after that join; callers cannot supply an alternate descriptor ID.
pub fn build_composition_lifecycle_hot_plan_v6(
    admission: CompositionAdmissionPlanV3<'_>,
    state: &RationalLifecycleHotStateV3<'_>,
    header: LifecycleHeaderV2,
    coordinates: &[LifecycleCoordinateV2],
    claims_program: Pubkey,
    claims_accounts: &[solana_program::instruction::AccountMeta],
    selection: RationalLifecycleSelectedSelectionV6<'_>,
) -> Result<CompositionLifecycleHotPlanV3> {
    let admitted = admission.admitted();
    let descriptor = admitted.execution_descriptor();
    let selected_descriptor = selection.representation_descriptor;
    if claims_program == Pubkey::default()
        || claims_program != admitted.claims_program()
        || admitted.observation().slot != state.finalized_slot
        || descriptor.market_id() != state.market.to_bytes()
        || descriptor.release_set_id() != state.release_set
        || descriptor.descriptor_id() != selected_descriptor.descriptor_id()
        || descriptor.market_id() != selected_descriptor.market_id()
        || descriptor.graph_id() != selected_descriptor.graph_id()
        || descriptor.graph_digest() != selected_descriptor.graph_digest()
        || descriptor.release_set_id() != selected_descriptor.release_set_id()
        || descriptor.receipt_mint() != selected_descriptor.receipt_mint()
        || descriptor.token_program() != selected_descriptor.token_program()
        || descriptor.representation_authority() != selected_descriptor.representation_authority()
        || descriptor.outcome_count() != selected_descriptor.outcome_count()
        || descriptor.denominator() != selected_descriptor.denominator()
        || descriptor.descriptor_id() != selection.authenticated_token_behavior.descriptor_id()
        || descriptor.release_set_id() != selection.bundle.release_set
        || descriptor.token_program() != selection.bundle.token_program
        || descriptor.outcome_count() != admitted.representation_width()
        || admitted.product_width() == 0
        || header.parent_context != [0; 32]
    {
        return Err(Error::HotAdapter);
    }
    let lifecycle = derive_v6_parent_and_build_lifecycle(admitted, header, coordinates)?;
    if claims_accounts.len() != lifecycle.account_count {
        return Err(Error::HotAdapter);
    }
    let child = Instruction {
        program_id: claims_program,
        accounts: claims_accounts.to_vec(),
        data: lifecycle.request.clone(),
    };
    let hot = build_rational_lifecycle_selected_hot_instruction_v6(state, &child, selection)
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

fn derive_v6_parent_and_build_lifecycle(
    admitted: crate::AdmittedCompositionV3<'_>,
    mut header: LifecycleHeaderV2,
    coordinates: &[LifecycleCoordinateV2],
) -> Result<ClaimsLifecyclePlanV3> {
    header.parent_context = [1; 32];
    let provisional = build_claims_lifecycle_plan_v3(admitted, header, coordinates)?;
    let child = LifecycleRequestV2::decode(&provisional.request).map_err(|_| Error::HotAdapter)?;
    let mut family_bytes = vec![0_u8; provisional.request.len()];
    let family = RationalLifecycleHotRequestV6::from_child_into(child, &mut family_bytes)
        .map_err(|_| Error::HotAdapter)?;
    header.parent_context = hash(family.as_bytes()).to_bytes();
    build_claims_lifecycle_plan_v3(admitted, header, coordinates)
}
