//! Unsigned market-neutral V6 Hot operator.

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_capability_program_contract::hot_v3::{
    HOT_FAMILY_REQUEST_OFFSET_V3, HotExecutionEnvelopeV3,
};
use dclutch_rational_representation_v2_contract::AuthenticatedTokenBehaviorV2;
use dclutch_rational_representation_v2_kernel::RepresentationDescriptorV2;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2, LifecycleRequestV2, hot_v6::RationalLifecycleHotRequestV6,
};
use solana_program::{hash::hash, instruction::Instruction};

use crate::{
    Error, RationalLifecycleHotInstructionV3, RationalLifecycleHotStateV3,
    RationalLifecycleSelectedBundleV6, Result, lifecycle_claims_account_count_v3,
    operator::{MAX_SOLANA_PACKET_BYTES, validate_child_frame, validate_fixed_frame},
    selected_operator_v5::compact_profile13_claims_accounts_v5,
    validate_rational_lifecycle_selected_bundle_v6,
};

/// Exact runtime descriptor authority for one V6 bundle.
#[derive(Clone, Copy, Debug)]
pub struct RationalLifecycleSelectedSelectionV6<'a> {
    /// Pre-founding-safe V6 artifacts.
    pub bundle: &'a RationalLifecycleSelectedBundleV6,
    /// Independently authenticated Realm/release Token behavior.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
    /// Finalized per-Market descriptor authenticated by the composition join.
    pub representation_descriptor: RepresentationDescriptorV2<'a>,
}

/// Build one complete unsigned selected V6 Hot instruction.
pub fn build_rational_lifecycle_selected_hot_instruction_v6(
    state: &RationalLifecycleHotStateV3<'_>,
    claims_child: &Instruction,
    selection: RationalLifecycleSelectedSelectionV6<'_>,
) -> Result<RationalLifecycleHotInstructionV3> {
    let checked = state.hot_outer.ok_or(Error::Operator)?;
    validate_fixed_frame(state, checked)?;
    validate_rational_lifecycle_selected_bundle_v6(selection.bundle)?;
    let descriptor = selection.representation_descriptor;
    let behavior = selection.authenticated_token_behavior;
    if behavior.descriptor_id() != descriptor.descriptor_id()
        || behavior.selection().release_set() != descriptor.release_set_id()
        || behavior.selection().token_program() != descriptor.token_program()
        || selection.bundle.release_set != descriptor.release_set_id()
        || selection.bundle.token_program != descriptor.token_program()
        || hash(&selection.bundle.token_behavior_selection).to_bytes() != behavior.content_digest()
        || state.finalized_slot == 0
        || checked.artifact_release == [0; 32]
        || checked.checked_manifest_digest == [0; 32]
        || claims_child.program_id == solana_program::pubkey::Pubkey::default()
    {
        return Err(Error::Operator);
    }
    let child = LifecycleRequestV2::decode(&claims_child.data).map_err(|_| Error::Operator)?;
    let header = child.header();
    let coordinate_count = match header.action {
        LifecycleActionV2::ActivateReceipt => 0,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => 1,
        LifecycleActionV2::RetireReceipt => return Err(Error::ActionGeometry),
    };
    if header.action != selection.bundle.action
        || header.release_set != state.release_set
        || header.market != state.market.to_bytes()
        || header.generation != state.generation
        || header.coordinate_count != coordinate_count
        || header.descriptor_id != descriptor.descriptor_id()
        || header.graph_id != descriptor.graph_id()
        || header.representation_authority != descriptor.representation_authority()
        || header.receipt_mint != descriptor.receipt_mint()
        || header.token_program != descriptor.token_program()
        || header.outcome_count != descriptor.outcome_count()
        || claims_child.accounts.len()
            != usize::from(lifecycle_claims_account_count_v3(
                header.action,
                coordinate_count,
            )?)
    {
        return Err(Error::Operator);
    }
    validate_child_frame(claims_child, header.action)?;

    let mut family_bytes = vec![0_u8; claims_child.data.len()];
    let family = RationalLifecycleHotRequestV6::from_child_into(child, &mut family_bytes)
        .map_err(Error::Lifecycle)?;
    let family_digest = hash(family.as_bytes()).to_bytes();
    let mut exact_child = vec![0_u8; claims_child.data.len()];
    family
        .specialize_child_into(family_digest, &mut exact_child)
        .map_err(Error::Lifecycle)?;
    if exact_child != claims_child.data {
        return Err(Error::Operator);
    }
    let envelope = HotExecutionEnvelopeV3::new(
        u32::try_from(family_bytes.len()).map_err(|_| Error::Operator)?,
        state.release_set,
        state.market.to_bytes(),
        state.generation,
        hash(state.root_data).to_bytes(),
    )
    .map_err(|_| Error::Operator)?;
    let mut data = Vec::with_capacity(
        HOT_FAMILY_REQUEST_OFFSET_V3
            .checked_add(family_bytes.len())
            .ok_or(Error::Operator)?,
    );
    data.extend_from_slice(&envelope.to_bytes());
    data.extend_from_slice(&family_bytes);
    if data.len() > MAX_SOLANA_PACKET_BYTES {
        return Err(Error::Operator);
    }

    let profile = AccountProfileV2::decode(&selection.bundle.account_profile)
        .map_err(Error::AccountProfile)?;
    let physical_child =
        compact_profile13_claims_accounts_v5(state, &claims_child.accounts, profile)?;
    let mut accounts = Vec::with_capacity(
        state
            .fixed_accounts
            .len()
            .checked_add(state.strategy_accounts.len())
            .and_then(|count| count.checked_add(physical_child.len()))
            .ok_or(Error::Operator)?,
    );
    accounts.extend_from_slice(state.fixed_accounts);
    accounts.extend_from_slice(state.strategy_accounts);
    accounts.extend(physical_child);
    Ok(RationalLifecycleHotInstructionV3 {
        instruction: Instruction {
            program_id: checked.trading_program,
            accounts,
            data,
        },
        required_wallet_signers: Vec::new(),
        family_digest,
        checked_manifest_digest: checked.checked_manifest_digest,
        finalized_slot: state.finalized_slot,
        requires_v0_address_lookup: true,
    })
}
