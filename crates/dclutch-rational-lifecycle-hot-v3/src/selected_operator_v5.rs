//! Unsigned selected Hot operator for fixed-cardinality lifecycle actions.

use dclutch_account_profile_contract::v2::{AccountProfileV2, DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE};
use dclutch_capability_program_contract::hot_v3::{
    HOT_CONFIG_RAW_ACCOUNT_V3, HOT_FAMILY_REQUEST_OFFSET_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
    HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
    HotExecutionEnvelopeV3,
};
use dclutch_rational_representation_v2_contract::AuthenticatedTokenBehaviorV2;
use dclutch_rational_representation_v2_lifecycle_contract::{
    LifecycleActionV2, LifecycleRequestV2, hot_v3::RationalLifecycleHotRequestV3,
};
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
};

use crate::{
    Error, RationalLifecycleHotInstructionV3, RationalLifecycleHotStateV3,
    RationalLifecycleSelectedBundleV5, Result, lifecycle_claims_account_count_v3,
    operator::{MAX_SOLANA_PACKET_BYTES, validate_child_frame, validate_fixed_frame},
    validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5,
};

/// Exact selected authority for one fixed-cardinality lifecycle instruction.
#[derive(Clone, Copy, Debug)]
pub struct RationalLifecycleSelectedSelectionV5<'a> {
    /// Exact CapabilityV4/LifecycleV5/Profile13 bundle.
    pub bundle: &'a RationalLifecycleSelectedBundleV5,
    /// Independently authenticated descriptor/Realm/release Token behavior.
    pub authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
}

/// Build one complete unsigned selected Hot lifecycle instruction.
pub fn build_rational_lifecycle_selected_hot_instruction_v5(
    state: &RationalLifecycleHotStateV3<'_>,
    claims_child: &Instruction,
    selection: RationalLifecycleSelectedSelectionV5<'_>,
) -> Result<RationalLifecycleHotInstructionV3> {
    let checked = state.hot_outer.ok_or(Error::Operator)?;
    validate_fixed_frame(state, checked)?;
    validate_rational_lifecycle_selected_bundle_for_authenticated_selection_v5(
        selection.bundle,
        selection.authenticated_token_behavior,
    )?;
    if state.finalized_slot == 0
        || state.release_set == [0; 32]
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
        || header.descriptor_id != selection.bundle.representation_descriptor_id
        || header.descriptor_id != selection.authenticated_token_behavior.descriptor_id()
        || header.token_program != selection.bundle.token_program
        || header.token_program
            != selection
                .authenticated_token_behavior
                .selection()
                .token_program()
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
    let family = RationalLifecycleHotRequestV3::from_child_into(child, &mut family_bytes)
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

fn compact_profile13_claims_accounts_v5(
    state: &RationalLifecycleHotStateV3<'_>,
    child: &[AccountMeta],
    profile: AccountProfileV2<'_>,
) -> Result<Vec<AccountMeta>> {
    const INJECTED: usize = 5;
    const TAIL_COUNT: u32 = 0;
    let expected_logical = INJECTED.checked_add(child.len()).ok_or(Error::Operator)?;
    if profile.artifact_profile() != DYNAMIC_FIXED_SPAN_ARTIFACT_PROFILE
        || profile.dynamic_fixed_span_count() != 0
        || profile
            .logical_account_count_with_dynamic_spans(TAIL_COUNT, &[])
            .map_err(Error::AccountProfile)?
            != expected_logical
    {
        return Err(Error::Operator);
    }
    let physical = profile
        .physical_account_count_with_dynamic_spans(TAIL_COUNT, &[])
        .map_err(Error::AccountProfile)?;
    if physical < INJECTED {
        return Err(Error::Operator);
    }
    for coordinate in 0..INJECTED {
        if profile
            .representative_with_dynamic_spans(TAIL_COUNT, &[], coordinate)
            .map_err(Error::AccountProfile)?
            != coordinate
        {
            return Err(Error::Operator);
        }
        let meta = injected_meta_v5(state, coordinate)?;
        let (signer, writable) = physical_privileges_v5(profile, coordinate)?;
        if meta.is_signer != signer || meta.is_writable != writable {
            return Err(Error::Operator);
        }
    }
    let mut output = Vec::with_capacity(physical - INJECTED);
    for (child_index, account) in child.iter().enumerate() {
        let logical = INJECTED.checked_add(child_index).ok_or(Error::Operator)?;
        let route = profile
            .route_privileges_with_dynamic_spans(TAIL_COUNT, &[], logical)
            .map_err(Error::AccountProfile)?;
        if account.is_writable != route.writable()
            || (child_index != 0 && account.is_signer != route.signer())
            || (child_index == 0 && !account.is_signer)
        {
            return Err(Error::Operator);
        }
        let representative = profile
            .representative_with_dynamic_spans(TAIL_COUNT, &[], logical)
            .map_err(Error::AccountProfile)?;
        let representative_meta = if representative < INJECTED {
            injected_meta_v5(state, representative)?
        } else {
            child
                .get(
                    representative
                        .checked_sub(INJECTED)
                        .ok_or(Error::Operator)?,
                )
                .ok_or(Error::Operator)?
        };
        if representative_meta.pubkey != account.pubkey {
            return Err(Error::Operator);
        }
        if representative == logical {
            let (signer, writable) = physical_privileges_v5(profile, representative)?;
            let mut outer = account.clone();
            outer.is_signer = signer;
            outer.is_writable = writable;
            output.push(outer);
        }
    }
    if output.len() != physical - INJECTED {
        return Err(Error::Operator);
    }
    Ok(output)
}

fn physical_privileges_v5(
    profile: AccountProfileV2<'_>,
    representative: usize,
) -> Result<(bool, bool)> {
    const TAIL_COUNT: u32 = 0;
    let logical = profile
        .logical_account_count_with_dynamic_spans(TAIL_COUNT, &[])
        .map_err(Error::AccountProfile)?;
    let mut signer = false;
    let mut writable = false;
    for coordinate in 0..logical {
        if profile
            .representative_with_dynamic_spans(TAIL_COUNT, &[], coordinate)
            .map_err(Error::AccountProfile)?
            == representative
        {
            let route = profile
                .route_privileges_with_dynamic_spans(TAIL_COUNT, &[], coordinate)
                .map_err(Error::AccountProfile)?;
            signer |= route.signer();
            writable |= route.writable();
        }
    }
    Ok((signer, writable))
}

fn injected_meta_v5<'a>(
    state: &'a RationalLifecycleHotStateV3<'_>,
    logical: usize,
) -> Result<&'a AccountMeta> {
    let physical = match logical {
        0 => HOT_ROOT_ACCOUNT_V3,
        1 => HOT_CONFIG_RAW_ACCOUNT_V3,
        2 => HOT_PRODUCT_RAW_ACCOUNT_V3,
        3 => HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        4 => HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        _ => return Err(Error::Operator),
    };
    state.fixed_accounts.get(physical).ok_or(Error::Operator)
}
