//! Chain-derived Hot family and unsigned transaction construction for selected opens.

use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, OpenRepresentationHotRequestV3, REPRESENTATION_FRAME_SPEC_V2,
    RepresentationActionV2, RepresentationRequestV2,
};
use dclutch_rational_representation_v2_operator::{
    ConstructedInstructionV2, RationalObservationV2, SelectedActionInputV2,
};
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey};

use crate::hot_transaction_v3::build_profiled_hot_instruction_from_claims_child_v3;
use crate::open_capability_set_v3::require_open_program_selection_v3;
use crate::{
    Error, RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3, RationalOpenCapabilityProgramSetV3,
    RationalOpenSelectedHotBundleV3, RationalTerminalHotStateV3, Result,
    construct_chain_denominate, construct_chain_reconstitute,
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3,
};

/// One exact parent-free open family request and its canonical Claims child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructedHotOpenSelectedV3 {
    /// Exact wallet-facing variable-width open family request.
    pub family_request: Vec<u8>,
    /// SHA-256 of the complete family request.
    pub family_digest: [u8; 32],
    /// Exact 36-account selected Claims child under `family_digest`.
    pub claims_child: ConstructedInstructionV2,
}

/// Same-finalized Hot38 observation used by selected open actions.
pub type RationalOpenSelectedHotStateV3<'a> = RationalTerminalHotStateV3<'a>;

/// Complete unsigned Trading instruction and explicit wallet signer set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalOpenSelectedHotInstructionV3 {
    /// Exact user-triggered Trading instruction. Nothing here signs or submits it.
    pub instruction: Instruction,
    /// Wallet identities which must sign the eventual transaction.
    pub required_wallet_signers: Vec<Pubkey>,
    /// Exact family request digest bound into the Claims child.
    pub family_digest: [u8; 32],
    /// Digest of the checked multiprogram release manifest.
    pub checked_manifest_digest: [u8; 32],
    /// Finalized slot shared by every chain observation used to build it.
    pub finalized_slot: u64,
}

/// Construct a parent-free Denominate family request from authenticated chain state.
pub fn construct_chain_hot_denominate_v3(
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
) -> Result<ConstructedHotOpenSelectedV3> {
    construct_chain_hot_selected_v3(observation, input, RepresentationActionV2::Denominate)
}

/// Construct a parent-free Reconstitute family request from authenticated chain state.
pub fn construct_chain_hot_reconstitute_v3(
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
) -> Result<ConstructedHotOpenSelectedV3> {
    construct_chain_hot_selected_v3(observation, input, RepresentationActionV2::Reconstitute)
}

/// Build one complete unsigned Hot38 selected-open instruction.
///
/// The Trading caller PDA is a signer only in the downstream Claims CPI, so
/// its child signer flag is removed from the outer transaction. The actor's
/// existing wallet signature is retained.
pub fn build_rational_open_selected_hot_instruction_v3(
    state: &RationalOpenSelectedHotStateV3<'_>,
    selected: &ConstructedHotOpenSelectedV3,
    bundle: &RationalOpenSelectedHotBundleV3,
    capability_set: &RationalOpenCapabilityProgramSetV3,
    authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
) -> Result<RationalOpenSelectedHotInstructionV3> {
    let mut scratch = vec![0_u8; selected.family_request.len()];
    let family =
        OpenRepresentationHotRequestV3::decode_with_scratch(&selected.family_request, &mut scratch)
            .map_err(Error::HotContract)?;
    if !matches!(
        family.action().map_err(Error::HotContract)?,
        RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
    ) || family.asset_count().map_err(Error::HotContract)? != 1
    {
        return Err(Error::HotInstruction);
    }
    require_open_program_selection_v3(
        capability_set,
        authenticated_token_behavior,
        &selected.family_request,
        &bundle.descriptor,
    )?;
    let child = RepresentationRequestV2::decode(&selected.claims_child.instruction.data)
        .map_err(Error::HotContract)?;
    if child.header().descriptor_id != authenticated_token_behavior.descriptor_id()
        || child.header().release_set != authenticated_token_behavior.selection().release_set()
        || child.header().release_set != state.release_set
    {
        return Err(Error::HotInstruction);
    }
    validate_rational_open_selected_hot_bundle_for_authenticated_selection_v3(
        bundle,
        authenticated_token_behavior,
    )?;
    let built = build_profiled_hot_instruction_from_claims_child_v3(
        state,
        &selected.family_request,
        selected.family_digest,
        &selected.claims_child,
        usize::from(RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3),
        &bundle.account_profile,
        0,
    )?;
    Ok(RationalOpenSelectedHotInstructionV3 {
        instruction: built.instruction,
        required_wallet_signers: built.required_wallet_signers,
        family_digest: selected.family_digest,
        checked_manifest_digest: built.checked_manifest_digest,
        finalized_slot: state.finalized_slot,
    })
}

fn construct_chain_hot_selected_v3(
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
    action: RepresentationActionV2,
) -> Result<ConstructedHotOpenSelectedV3> {
    if observation.parent_context != [0; 32] {
        return Err(Error::NonCanonicalParent);
    }
    let template_observation = RationalObservationV2 {
        parent_context: [1; 32],
        ..observation
    };
    let template = construct_selected(template_observation, input, action)?;
    let template_request =
        RepresentationRequestV2::decode(&template.instruction.data).map_err(Error::HotContract)?;
    validate_selected_child(template_request, &template, action)?;

    let request_len = template.instruction.data.len();
    let mut family_request = vec![0_u8; request_len];
    let family =
        OpenRepresentationHotRequestV3::from_child_into(template_request, &mut family_request)
            .map_err(Error::HotContract)?;
    let family_digest = hash(family.as_bytes()).to_bytes();

    let exact_observation = RationalObservationV2 {
        parent_context: family_digest,
        ..observation
    };
    let claims_child = construct_selected(exact_observation, input, action)?;
    let exact_request = RepresentationRequestV2::decode(&claims_child.instruction.data)
        .map_err(Error::HotContract)?;
    validate_selected_child(exact_request, &claims_child, action)?;
    let mut specialized = vec![0_u8; request_len];
    let specialized_parent = family
        .specialize_child_into(family_digest, &mut specialized)
        .map_err(Error::HotContract)?
        .header()
        .parent_context;
    if claims_child.instruction.data != specialized
        || claims_child.request_digest != hash(&specialized).to_bytes()
        || specialized_parent != family_digest
    {
        return Err(Error::HotChildMismatch);
    }
    Ok(ConstructedHotOpenSelectedV3 {
        family_request,
        family_digest,
        claims_child,
    })
}

fn construct_selected(
    observation: RationalObservationV2<'_>,
    input: SelectedActionInputV2,
    action: RepresentationActionV2,
) -> Result<ConstructedInstructionV2> {
    match action {
        RepresentationActionV2::Denominate => construct_chain_denominate(observation, input),
        RepresentationActionV2::Reconstitute => construct_chain_reconstitute(observation, input),
        _ => Err(Error::HotInstruction),
    }
}

fn validate_selected_child(
    request: RepresentationRequestV2<'_>,
    child: &ConstructedInstructionV2,
    action: RepresentationActionV2,
) -> Result<()> {
    if request.header().action != action
        || request.header().asset_count != 1
        || REPRESENTATION_FRAME_SPEC_V2
            .account_count(request)
            .map_err(Error::HotContract)?
            != usize::from(RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3)
        || child.instruction.accounts.len() != usize::from(RATIONAL_OPEN_SELECTED_CHILD_ACCOUNTS_V3)
    {
        return Err(Error::HotInstruction);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_capability_program_contract::hot_v3::{
        HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3,
        HOT_PRODUCT_RAW_ACCOUNT_V3, HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3,
        HOT_TRADING_PROGRAM_ACCOUNT_V3, HotExecutionEnvelopeV3,
    };
    use dclutch_rational_representation_v2_contract::{
        ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, CallerRoleV2, REQUEST_HEADER_BYTES_V2,
        RepresentationRequestHeaderV2,
    };
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;
    use solana_program::instruction::AccountMeta;
    use solana_sdk_ids::sysvar;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn selected() -> ConstructedHotOpenSelectedV3 {
        let mut row = [0_u8; ASSET_BYTES_V2];
        AssetV2 {
            shard_mint: key(20).to_bytes(),
            actor_shard_account: key(21).to_bytes(),
            structured_custody_account: key(22).to_bytes(),
            claims_custody_owner: key(23).to_bytes(),
            coefficient: 10,
            expected_shard_supply: 100,
            expected_actor_shards: 30,
            expected_structured_shards: 0,
        }
        .encode_into(&mut row)
        .expect("asset");
        let template = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::Denominate,
                caller_role: CallerRoleV2::Trading,
                release_set: key(1).to_bytes(),
                market: key(2).to_bytes(),
                graph_id: key(3).to_bytes(),
                descriptor_id: key(4).to_bytes(),
                parent_context: key(5).to_bytes(),
                actor: key(6).to_bytes(),
                receipt_mint: key(7).to_bytes(),
                receipt_account: [0; 32],
                representation_authority: key(8).to_bytes(),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: [0; 32],
                collateral_recipient: [0; 32],
                expected_representation_revision: 4,
                expected_claims_market_revision: 11,
                expected_actor_position_revision: 12,
                expected_custody_position_revision: 13,
                expected_custody_replay_revision: ABSENT_REVISION,
                generation: 14,
                quantity: 2,
                denominator: 10,
                expected_receipt_supply: 0,
                outcome_count: 258,
                selected_outcome: 257,
                asset_count: 1,
            },
            &row,
        )
        .expect("template");
        let mut template_bytes = vec![0_u8; REQUEST_HEADER_BYTES_V2 + row.len()];
        template.encode_into(&mut template_bytes).expect("encode");
        let request_len = template_bytes.len();
        let mut family_request = vec![0_u8; request_len];
        let family = OpenRepresentationHotRequestV3::from_child_into(template, &mut family_request)
            .expect("family");
        let family_digest = hash(family.as_bytes()).to_bytes();
        let mut exact = vec![0_u8; request_len];
        let exact_parent = family
            .specialize_child_into(family_digest, &mut exact)
            .expect("child")
            .header()
            .parent_context;
        assert_eq!(exact_parent, family_digest);
        let claims_program = key(70);
        let mut accounts = (0_u8..36)
            .map(|index| AccountMeta::new_readonly(key(100_u8.wrapping_add(index)), false))
            .collect::<Vec<_>>();
        accounts.get_mut(0).expect("caller").is_signer = true;
        *accounts.get_mut(3).expect("actor") = AccountMeta::new_readonly(key(6), true);
        *accounts.get_mut(14).expect("Claims") = AccountMeta::new_readonly(claims_program, false);
        for index in [11_usize, 12, 23, 32, 33, 34] {
            accounts.get_mut(index).expect("writable child").is_writable = true;
        }
        ConstructedHotOpenSelectedV3 {
            family_request,
            family_digest,
            claims_child: ConstructedInstructionV2 {
                instruction: Instruction {
                    program_id: claims_program,
                    accounts,
                    data: exact.clone(),
                },
                request_digest: hash(&exact).to_bytes(),
                representation_authority: key(8),
                representation_replay: key(80),
                claims_aggregate: key(81),
                assets: Vec::new(),
                terminal: None,
            },
        }
    }

    fn fixed() -> Vec<AccountMeta> {
        let mut output = (0_u8..u8::try_from(HOT_FIXED_ACCOUNT_COUNT_V3).expect("Hot width"))
            .map(|index| AccountMeta::new_readonly(key(150_u8.wrapping_add(index)), false))
            .collect::<Vec<_>>();
        *output.get_mut(HOT_MARKET_ACCOUNT_V3).expect("Market") =
            AccountMeta::new_readonly(key(2), false);
        output
            .get_mut(HOT_ROOT_ACCOUNT_V3)
            .expect("root")
            .is_writable = true;
        *output
            .get_mut(HOT_TRADING_PROGRAM_ACCOUNT_V3)
            .expect("Trading") = AccountMeta::new_readonly(key(60), false);
        *output.get_mut(HOT_RENT_SYSVAR_ACCOUNT_V3).expect("Rent") =
            AccountMeta::new_readonly(sysvar::rent::ID, false);
        *output
            .get_mut(HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3)
            .expect("Instructions") = AccountMeta::new_readonly(sysvar::instructions::ID, false);
        output
    }

    fn bind_profile13_aliases(selected: &mut ConstructedHotOpenSelectedV3, fixed: &[AccountMeta]) {
        let accounts = &mut selected.claims_child.instruction.accounts;
        let claims = accounts.get(14).expect("Claims").clone();
        *accounts.get_mut(21).expect("Claims alias") = claims;
        *accounts.get_mut(24).expect("basis alias") = fixed
            .get(HOT_LINKED_BASIS_RAW_ACCOUNT_V3)
            .expect("basis")
            .clone();
        *accounts.get_mut(26).expect("Product alias") = fixed
            .get(HOT_PRODUCT_RAW_ACCOUNT_V3)
            .expect("Product")
            .clone();
        *accounts.get_mut(30).expect("portfolio alias") = fixed
            .get(HOT_PORTFOLIO_RAW_ACCOUNT_V3)
            .expect("portfolio")
            .clone();
    }

    #[test]
    fn selected_open_builds_hot38_and_binds_complete_family_digest() {
        let fixed = fixed();
        let mut selected = selected();
        bind_profile13_aliases(&mut selected, &fixed);
        let artifacts = crate::test_open_fixture_v3::open_artifact_fixture_v3(
            key(9).to_bytes(),
            key(1).to_bytes(),
            258,
        );
        let state = RationalOpenSelectedHotStateV3 {
            fixed_accounts: &fixed,
            strategy_accounts: &[],
            root_data: &[7; 64],
            release_set: key(1).to_bytes(),
            market: key(2),
            generation: 14,
            finalized_slot: 99,
            hot_outer: Some(crate::CheckedRationalHotOuterReleaseV3 {
                trading_program: key(60),
                artifact_release: key(61).to_bytes(),
                checked_manifest_digest: key(62).to_bytes(),
            }),
        };
        let built = build_rational_open_selected_hot_instruction_v3(
            &state,
            &selected,
            &artifacts.denominate,
            &artifacts.set,
            artifacts.token_behavior,
        )
        .expect("selected Hot");
        assert_eq!(
            built.instruction.accounts.len(),
            HOT_FIXED_ACCOUNT_COUNT_V3 + 32
        );
        assert!(
            !built
                .instruction
                .accounts
                .get(38)
                .expect("caller")
                .is_signer
        );
        assert!(built.instruction.accounts.get(41).expect("actor").is_signer);
        assert_eq!(built.required_wallet_signers, vec![key(6)]);
        let (envelope, family) =
            HotExecutionEnvelopeV3::split_instruction(&built.instruction.data).expect("envelope");
        assert_eq!(envelope.market(), key(2).to_bytes());
        assert_eq!(hash(family).to_bytes(), built.family_digest);
    }

    #[test]
    fn family_or_child_substitution_refuses_before_instruction_output() {
        let fixed = fixed();
        let artifacts = crate::test_open_fixture_v3::open_artifact_fixture_v3(
            key(9).to_bytes(),
            key(1).to_bytes(),
            258,
        );
        let state = RationalOpenSelectedHotStateV3 {
            fixed_accounts: &fixed,
            strategy_accounts: &[],
            root_data: &[7; 64],
            release_set: key(1).to_bytes(),
            market: key(2),
            generation: 14,
            finalized_slot: 99,
            hot_outer: Some(crate::CheckedRationalHotOuterReleaseV3 {
                trading_program: key(60),
                artifact_release: key(61).to_bytes(),
                checked_manifest_digest: key(62).to_bytes(),
            }),
        };
        let mut hostile = selected();
        bind_profile13_aliases(&mut hostile, &fixed);
        *hostile.family_request.last_mut().expect("family byte") ^= 1;
        assert_eq!(
            build_rational_open_selected_hot_instruction_v3(
                &state,
                &hostile,
                &artifacts.denominate,
                &artifacts.set,
                artifacts.token_behavior,
            ),
            Err(Error::HotInstruction)
        );

        let mut hostile = selected();
        bind_profile13_aliases(&mut hostile, &fixed);
        hostile
            .claims_child
            .instruction
            .accounts
            .get_mut(3)
            .expect("actor")
            .is_signer = false;
        assert_eq!(
            build_rational_open_selected_hot_instruction_v3(
                &state,
                &hostile,
                &artifacts.denominate,
                &artifacts.set,
                artifacts.token_behavior,
            ),
            Err(Error::HotInstruction)
        );

        let mut hostile = selected();
        bind_profile13_aliases(&mut hostile, &fixed);
        assert_eq!(
            build_rational_open_selected_hot_instruction_v3(
                &state,
                &hostile,
                &artifacts.denominate,
                &artifacts.set,
                crate::test_open_fixture_v3::authenticated_token_behavior_v3(
                    key(5).to_bytes(),
                    key(9).to_bytes(),
                    key(1).to_bytes(),
                    258,
                ),
            ),
            Err(Error::HotInstruction)
        );
    }
}
