//! Chain-derived Hot family and unsigned transaction construction for Structured actions.

use dclutch_rational_representation_v2_contract::{
    AuthenticatedTokenBehaviorV2, OpenRepresentationHotRequestV3, REPRESENTATION_FRAME_SPEC_V2,
    RepresentationActionV2, RepresentationRequestV2,
};
use dclutch_rational_representation_v2_operator::{
    ConstructedInstructionV2, RationalObservationV2, StructuredActionInputV2,
    construct_issue_structured, construct_unwrap_structured,
};
use solana_program::{hash::hash, instruction::Instruction, pubkey::Pubkey};

use crate::hot_transaction_v3::build_profiled_hot_instruction_from_claims_child_v3;
use crate::open_capability_set_v3::require_open_program_selection_v3;
use crate::{
    Error, RationalOpenCapabilityProgramSetV3, RationalOpenStructuredHotBundleV3,
    RationalTerminalHotStateV3, Result,
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3,
};

/// One exact parent-free Structured family request and canonical Claims child.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConstructedHotOpenStructuredV3 {
    /// Exact wallet-facing variable-width open family request.
    pub family_request: Vec<u8>,
    /// SHA-256 of the complete family request.
    pub family_digest: [u8; 32],
    /// Exact `32 + 4*N` account Claims child under `family_digest`.
    pub claims_child: ConstructedInstructionV2,
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
}

/// Same-finalized Hot38 observation used by Structured actions.
pub type RationalOpenStructuredHotStateV3<'a> = RationalTerminalHotStateV3<'a>;

/// Complete unsigned Trading instruction and explicit wallet signer set.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RationalOpenStructuredHotInstructionV3 {
    /// Exact user-triggered Trading instruction. Nothing here signs or submits it.
    pub instruction: Instruction,
    /// Wallet identities which must sign the eventual transaction.
    pub required_wallet_signers: Vec<Pubkey>,
    /// Exact family request digest bound into the Claims child.
    pub family_digest: [u8; 32],
    /// Product-authenticated runtime outcome count.
    pub outcome_count: u32,
    /// Digest of the checked multiprogram release manifest.
    pub checked_manifest_digest: [u8; 32],
    /// Finalized slot shared by every chain observation used to build it.
    pub finalized_slot: u64,
}

/// Construct a parent-free IssueStructured request from authenticated chain state.
pub fn construct_chain_hot_issue_structured_v3(
    observation: RationalObservationV2<'_>,
    input: StructuredActionInputV2,
) -> Result<ConstructedHotOpenStructuredV3> {
    construct_chain_hot_structured_v3(observation, input, RepresentationActionV2::IssueStructured)
}

/// Construct a parent-free UnwrapStructured request from authenticated chain state.
pub fn construct_chain_hot_unwrap_structured_v3(
    observation: RationalObservationV2<'_>,
    input: StructuredActionInputV2,
) -> Result<ConstructedHotOpenStructuredV3> {
    construct_chain_hot_structured_v3(observation, input, RepresentationActionV2::UnwrapStructured)
}

/// Build one complete unsigned Hot38 Structured instruction.
pub fn build_rational_open_structured_hot_instruction_v3(
    state: &RationalOpenStructuredHotStateV3<'_>,
    structured: &ConstructedHotOpenStructuredV3,
    bundle: &RationalOpenStructuredHotBundleV3,
    capability_set: &RationalOpenCapabilityProgramSetV3,
    authenticated_token_behavior: AuthenticatedTokenBehaviorV2,
) -> Result<RationalOpenStructuredHotInstructionV3> {
    let mut scratch = vec![0_u8; structured.family_request.len()];
    let family = OpenRepresentationHotRequestV3::decode_with_scratch(
        &structured.family_request,
        &mut scratch,
    )
    .map_err(Error::HotContract)?;
    if !family.is_structured().map_err(Error::HotContract)?
        || family.asset_count().map_err(Error::HotContract)? != structured.outcome_count
    {
        return Err(Error::HotInstruction);
    }
    let expected_child_accounts = REPRESENTATION_FRAME_SPEC_V2
        .fixed_accounts()
        .checked_add(
            usize::try_from(structured.outcome_count)
                .map_err(|_| Error::HotInstruction)?
                .checked_mul(REPRESENTATION_FRAME_SPEC_V2.asset_account_stride())
                .ok_or(Error::HotInstruction)?,
        )
        .ok_or(Error::HotInstruction)?;
    require_open_program_selection_v3(
        capability_set,
        authenticated_token_behavior,
        &structured.family_request,
        &bundle.descriptor,
    )?;
    let child = RepresentationRequestV2::decode(&structured.claims_child.instruction.data)
        .map_err(Error::HotContract)?;
    if child.header().descriptor_id != authenticated_token_behavior.descriptor_id()
        || child.header().release_set != authenticated_token_behavior.selection().release_set()
        || child.header().release_set != state.release_set
    {
        return Err(Error::HotInstruction);
    }
    validate_rational_open_structured_hot_bundle_for_authenticated_selection_v3(
        bundle,
        authenticated_token_behavior,
    )?;
    let built = build_profiled_hot_instruction_from_claims_child_v3(
        state,
        &structured.family_request,
        structured.family_digest,
        &structured.claims_child,
        expected_child_accounts,
        &bundle.account_profile,
        structured.outcome_count,
    )?;
    Ok(RationalOpenStructuredHotInstructionV3 {
        instruction: built.instruction,
        required_wallet_signers: built.required_wallet_signers,
        family_digest: structured.family_digest,
        outcome_count: structured.outcome_count,
        checked_manifest_digest: built.checked_manifest_digest,
        finalized_slot: state.finalized_slot,
    })
}

fn construct_chain_hot_structured_v3(
    observation: RationalObservationV2<'_>,
    input: StructuredActionInputV2,
    action: RepresentationActionV2,
) -> Result<ConstructedHotOpenStructuredV3> {
    if observation.parent_context != [0; 32] {
        return Err(Error::NonCanonicalParent);
    }
    let template_observation = RationalObservationV2 {
        parent_context: [1; 32],
        ..observation
    };
    let template = construct_structured(template_observation, input, action)?;
    let template_request =
        RepresentationRequestV2::decode(&template.instruction.data).map_err(Error::HotContract)?;
    let outcome_count = validate_structured_child(template_request, &template, action)?;
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
    let claims_child = construct_structured(exact_observation, input, action)?;
    let exact_request = RepresentationRequestV2::decode(&claims_child.instruction.data)
        .map_err(Error::HotContract)?;
    if validate_structured_child(exact_request, &claims_child, action)? != outcome_count {
        return Err(Error::HotChildMismatch);
    }
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
    Ok(ConstructedHotOpenStructuredV3 {
        family_request,
        family_digest,
        claims_child,
        outcome_count,
    })
}

fn construct_structured(
    observation: RationalObservationV2<'_>,
    input: StructuredActionInputV2,
    action: RepresentationActionV2,
) -> Result<ConstructedInstructionV2> {
    match action {
        RepresentationActionV2::IssueStructured => {
            construct_issue_structured(observation, input).map_err(Error::ChainOperator)
        }
        RepresentationActionV2::UnwrapStructured => {
            construct_unwrap_structured(observation, input).map_err(Error::ChainOperator)
        }
        _ => Err(Error::HotInstruction),
    }
}

fn validate_structured_child(
    request: RepresentationRequestV2<'_>,
    child: &ConstructedInstructionV2,
    action: RepresentationActionV2,
) -> Result<u32> {
    let header = request.header();
    if header.action != action
        || header.asset_count == 0
        || header.asset_count != header.outcome_count
        || !matches!(
            action,
            RepresentationActionV2::IssueStructured | RepresentationActionV2::UnwrapStructured
        )
        || REPRESENTATION_FRAME_SPEC_V2
            .account_count(request)
            .map_err(Error::HotContract)?
            != child.instruction.accounts.len()
    {
        return Err(Error::HotInstruction);
    }
    Ok(header.outcome_count)
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

    const OUTCOMES: u32 = 2;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn structured() -> ConstructedHotOpenStructuredV3 {
        let mut rows = vec![0_u8; ASSET_BYTES_V2 * usize::try_from(OUTCOMES).expect("width")];
        for index in 0..OUTCOMES {
            let index_u8 = u8::try_from(index).expect("index");
            let start = usize::try_from(index).expect("index") * ASSET_BYTES_V2;
            AssetV2 {
                shard_mint: key(20 + index_u8).to_bytes(),
                actor_shard_account: key(30 + index_u8).to_bytes(),
                structured_custody_account: key(40 + index_u8).to_bytes(),
                claims_custody_owner: key(50 + index_u8).to_bytes(),
                coefficient: 10,
                expected_shard_supply: 100,
                expected_actor_shards: 30,
                expected_structured_shards: 20,
            }
            .encode_into(
                rows.get_mut(start..start + ASSET_BYTES_V2)
                    .expect("asset row"),
            )
            .expect("asset");
        }
        let template = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::IssueStructured,
                caller_role: CallerRoleV2::Trading,
                release_set: key(1).to_bytes(),
                market: key(2).to_bytes(),
                graph_id: key(3).to_bytes(),
                descriptor_id: key(4).to_bytes(),
                parent_context: key(5).to_bytes(),
                actor: key(6).to_bytes(),
                receipt_mint: key(7).to_bytes(),
                receipt_account: key(9).to_bytes(),
                representation_authority: key(8).to_bytes(),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: [0; 32],
                collateral_recipient: [0; 32],
                expected_representation_revision: 4,
                expected_claims_market_revision: ABSENT_REVISION,
                expected_actor_position_revision: ABSENT_REVISION,
                expected_custody_position_revision: ABSENT_REVISION,
                expected_custody_replay_revision: ABSENT_REVISION,
                generation: 14,
                quantity: 2,
                denominator: 10,
                expected_receipt_supply: 5,
                outcome_count: OUTCOMES,
                selected_outcome: u32::MAX,
                asset_count: OUTCOMES,
            },
            &rows,
        )
        .expect("template");
        let request_len = REQUEST_HEADER_BYTES_V2 + rows.len();
        let mut family_request = vec![0_u8; request_len];
        let family = OpenRepresentationHotRequestV3::from_child_into(template, &mut family_request)
            .expect("family");
        let family_digest = hash(family.as_bytes()).to_bytes();
        let mut exact = vec![0_u8; request_len];
        family
            .specialize_child_into(family_digest, &mut exact)
            .expect("child");
        let claims_program = key(70);
        let child_accounts = 32 + 4 * usize::try_from(OUTCOMES).expect("width");
        let mut accounts = (0..child_accounts)
            .map(|index| {
                AccountMeta::new_readonly(
                    key(100_u8.wrapping_add(u8::try_from(index).expect("account index"))),
                    false,
                )
            })
            .collect::<Vec<_>>();
        accounts.get_mut(0).expect("caller").is_signer = true;
        *accounts.get_mut(3).expect("actor") = AccountMeta::new_readonly(key(6), true);
        *accounts.get_mut(14).expect("Claims") = AccountMeta::new_readonly(claims_program, false);
        for index in [11_usize, 20, 21, 34, 35, 38, 39] {
            accounts.get_mut(index).expect("writable child").is_writable = true;
        }
        ConstructedHotOpenStructuredV3 {
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
            outcome_count: OUTCOMES,
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

    fn state<'a>(fixed: &'a [AccountMeta]) -> RationalOpenStructuredHotStateV3<'a> {
        RationalOpenStructuredHotStateV3 {
            fixed_accounts: fixed,
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
        }
    }

    fn bind_profile11_aliases(
        structured: &mut ConstructedHotOpenStructuredV3,
        fixed: &[AccountMeta],
    ) {
        let accounts = &mut structured.claims_child.instruction.accounts;
        let claims = accounts.get(14).expect("Claims").clone();
        *accounts.get_mut(23).expect("Claims alias") = claims;
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
    fn structured_builds_hot38_plus_exact_affine_child() {
        let fixed = fixed();
        let mut structured = structured();
        bind_profile11_aliases(&mut structured, &fixed);
        let artifacts = crate::test_open_fixture_v3::open_artifact_fixture_v3(
            key(9).to_bytes(),
            key(1).to_bytes(),
            OUTCOMES,
        );
        let built = build_rational_open_structured_hot_instruction_v3(
            &state(&fixed),
            &structured,
            &artifacts.issue,
            &artifacts.set,
            artifacts.token_behavior,
        )
        .expect("structured Hot");
        assert_eq!(built.outcome_count, OUTCOMES);
        assert_eq!(
            built.instruction.accounts.len(),
            HOT_FIXED_ACCOUNT_COUNT_V3 + 36
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
        let (envelope, family) =
            HotExecutionEnvelopeV3::split_instruction(&built.instruction.data).expect("envelope");
        assert_eq!(envelope.market(), key(2).to_bytes());
        assert_eq!(hash(family).to_bytes(), built.family_digest);
    }

    #[test]
    fn outcome_or_family_substitution_refuses() {
        let fixed = fixed();
        let artifacts = crate::test_open_fixture_v3::open_artifact_fixture_v3(
            key(9).to_bytes(),
            key(1).to_bytes(),
            OUTCOMES,
        );
        let mut hostile = structured();
        bind_profile11_aliases(&mut hostile, &fixed);
        hostile.outcome_count = 3;
        assert_eq!(
            build_rational_open_structured_hot_instruction_v3(
                &state(&fixed),
                &hostile,
                &artifacts.issue,
                &artifacts.set,
                artifacts.token_behavior,
            ),
            Err(Error::HotInstruction)
        );
        let mut hostile = structured();
        bind_profile11_aliases(&mut hostile, &fixed);
        *hostile.family_request.last_mut().expect("family byte") ^= 1;
        assert!(
            build_rational_open_structured_hot_instruction_v3(
                &state(&fixed),
                &hostile,
                &artifacts.issue,
                &artifacts.set,
                artifacts.token_behavior,
            )
            .is_err()
        );
    }
}
