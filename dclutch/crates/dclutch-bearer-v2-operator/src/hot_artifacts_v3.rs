//! Typed RequestProfile and TransitionVM artifacts for terminal Bearer redemption.

use dclutch_rational_representation_v2_contract::{
    CallerRoleV2, RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3, RATIONAL_TERMINAL_HOT_ACTOR_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_CLAIMS_CUSTODY_OWNER_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_COEFFICIENT_OFFSET_V3, RATIONAL_TERMINAL_HOT_ASSET_COUNT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_SHARD_MINT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_ASSET_STRUCTURED_CUSTODY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_COLLATERAL_RECIPIENT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3, RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3,
    RATIONAL_TERMINAL_HOT_DENOMINATOR_OFFSET_V3, RATIONAL_TERMINAL_HOT_DESCRIPTOR_ID_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_ACTOR_POSITION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_GENERATION_OFFSET_V3, RATIONAL_TERMINAL_HOT_GRAPH_ID_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3, RATIONAL_TERMINAL_HOT_MAGIC_V3,
    RATIONAL_TERMINAL_HOT_MARKET_OFFSET_V3, RATIONAL_TERMINAL_HOT_OUTCOME_COUNT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3, RATIONAL_TERMINAL_HOT_QUANTITY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_REALM_OFFSET_V3, RATIONAL_TERMINAL_HOT_RECEIPT_ACCOUNT_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_RECEIPT_MINT_OFFSET_V3, RATIONAL_TERMINAL_HOT_RELEASE_SET_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_REPRESENTATION_AUTHORITY_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3, RATIONAL_TERMINAL_HOT_RESERVED_HEADER_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_RESERVED_TAIL_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_SELECTED_OUTCOME_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_TOKEN_PROGRAM_OFFSET_V3, RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3,
    RATIONAL_TERMINAL_HOT_VERSION_V3, RATIONAL_TERMINAL_IDENTITY_ACTOR_SHARD_ACCOUNT_V3,
    RATIONAL_TERMINAL_IDENTITY_ACTOR_V3, RATIONAL_TERMINAL_IDENTITY_CLAIMS_CUSTODY_OWNER_V3,
    RATIONAL_TERMINAL_IDENTITY_COLLATERAL_RECIPIENT_V3, RATIONAL_TERMINAL_IDENTITY_DESCRIPTOR_V3,
    RATIONAL_TERMINAL_IDENTITY_GRAPH_V3, RATIONAL_TERMINAL_IDENTITY_MARKET_V3,
    RATIONAL_TERMINAL_IDENTITY_REALM_V3, RATIONAL_TERMINAL_IDENTITY_RECEIPT_MINT_V3,
    RATIONAL_TERMINAL_IDENTITY_RELEASE_SET_V3,
    RATIONAL_TERMINAL_IDENTITY_REPRESENTATION_AUTHORITY_V3,
    RATIONAL_TERMINAL_IDENTITY_SHARD_MINT_V3, RATIONAL_TERMINAL_IDENTITY_STRUCTURED_CUSTODY_V3,
    RATIONAL_TERMINAL_IDENTITY_TOKEN_PROGRAM_V3,
    RATIONAL_TERMINAL_SCALAR_ACTOR_POSITION_REVISION_V3, RATIONAL_TERMINAL_SCALAR_ACTOR_SHARDS_V3,
    RATIONAL_TERMINAL_SCALAR_ASSET_COUNT_V3, RATIONAL_TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3, RATIONAL_TERMINAL_SCALAR_CUSTODY_POSITION_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_CUSTODY_REPLAY_REVISION_V3, RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3,
    RATIONAL_TERMINAL_SCALAR_GENERATION_V3, RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3,
    RATIONAL_TERMINAL_SCALAR_QUANTITY_V3, RATIONAL_TERMINAL_SCALAR_RECEIPT_SUPPLY_V3,
    RATIONAL_TERMINAL_SCALAR_REPRESENTATION_REVISION_V3,
    RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3, RATIONAL_TERMINAL_SCALAR_SHARD_SUPPLY_V3,
    RATIONAL_TERMINAL_SCALAR_STRUCTURED_SHARDS_V3, RepresentationActionV2,
};
use dclutch_request_profile_contract::{
    HEADER_BYTES as REQUEST_PROFILE_HEADER_BYTES, OPERATION_BYTES as REQUEST_OPERATION_BYTES,
    encode::{
        IdentityRegisterV1, RequestCoordinateV1, RequestGeometryV1, RequestInstructionV1,
        ScalarRegisterV1, encode_request_profile_v1_atomic,
    },
};
use dclutch_transition_vm::v3::{
    HEADER_BYTES as TRANSITION_HEADER_BYTES, INSTRUCTION_BYTES as TRANSITION_INSTRUCTION_BYTES,
    InstructionV3, ProgramGeometryV3, ScalarRegisterV3, encode_program_atomic,
};

use crate::{Error, Result};

const REQUEST_INSTRUCTION_COUNT: usize = 39;
const TRANSITION_INSTRUCTION_COUNT: usize = 4;

/// Exact encoded RequestProfile V1 width for terminal Rational Hot V3.
pub const RATIONAL_TERMINAL_REQUEST_PROFILE_BYTES_V3: usize =
    REQUEST_PROFILE_HEADER_BYTES + REQUEST_INSTRUCTION_COUNT * REQUEST_OPERATION_BYTES;
/// Exact encoded TransitionVM V3 width for terminal Rational Hot V3.
pub const RATIONAL_TERMINAL_TRANSITION_BYTES_V3: usize =
    TRANSITION_HEADER_BYTES + TRANSITION_INSTRUCTION_COUNT * TRANSITION_INSTRUCTION_BYTES;

/// Encode the exact terminal family RequestProfile from Lean-emitted coordinates.
///
/// The parent digest and Product outcome count are deliberately not read from
/// family bytes. Hot seeds the former from SHA-256 of the complete family and
/// AccountProfile projects the latter from independently authenticated Product
/// state. This artifact projects every other transition coordinate and leaves
/// those two authority registers intact.
pub fn encode_rational_terminal_request_profile_v3()
-> Result<[u8; RATIONAL_TERMINAL_REQUEST_PROFILE_BYTES_V3]> {
    let fixed = [
        RequestInstructionV1::require_u64(
            request(RATIONAL_TERMINAL_HOT_MAGIC_OFFSET_V3)?,
            u64::from_le_bytes(RATIONAL_TERMINAL_HOT_MAGIC_V3),
        ),
        RequestInstructionV1::require_u16(
            request(RATIONAL_TERMINAL_HOT_VERSION_OFFSET_V3)?,
            RATIONAL_TERMINAL_HOT_VERSION_V3,
        ),
        RequestInstructionV1::require_u8(
            request(RATIONAL_TERMINAL_HOT_ACTION_OFFSET_V3)?,
            RepresentationActionV2::RedeemTerminal as u8,
        ),
        RequestInstructionV1::require_u8(
            request(RATIONAL_TERMINAL_HOT_CALLER_ROLE_OFFSET_V3)?,
            CallerRoleV2::Trading as u8,
        ),
        RequestInstructionV1::require_zero(
            request(RATIONAL_TERMINAL_HOT_RESERVED_HEADER_OFFSET_V3)?,
            4,
        ),
        RequestInstructionV1::require_zero(
            request(RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3)?,
            32,
        ),
        RequestInstructionV1::require_zero(
            request(RATIONAL_TERMINAL_HOT_RECEIPT_ACCOUNT_OFFSET_V3)?,
            32,
        ),
        RequestInstructionV1::require_u32(request(RATIONAL_TERMINAL_HOT_ASSET_COUNT_OFFSET_V3)?, 1),
        RequestInstructionV1::require_zero(
            request(RATIONAL_TERMINAL_HOT_RESERVED_TAIL_OFFSET_V3)?,
            4,
        ),
        project_identity(
            RATIONAL_TERMINAL_HOT_RELEASE_SET_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_RELEASE_SET_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_MARKET_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_MARKET_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_GRAPH_ID_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_GRAPH_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_DESCRIPTOR_ID_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_DESCRIPTOR_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_ACTOR_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_ACTOR_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_RECEIPT_MINT_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_RECEIPT_MINT_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_REPRESENTATION_AUTHORITY_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_REPRESENTATION_AUTHORITY_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_TOKEN_PROGRAM_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_TOKEN_PROGRAM_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_REALM_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_REALM_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_COLLATERAL_RECIPIENT_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_COLLATERAL_RECIPIENT_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_ASSET_SHARD_MINT_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_SHARD_MINT_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_ASSET_ACTOR_SHARD_ACCOUNT_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_ACTOR_SHARD_ACCOUNT_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_ASSET_STRUCTURED_CUSTODY_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_STRUCTURED_CUSTODY_V3,
        )?,
        project_identity(
            RATIONAL_TERMINAL_HOT_ASSET_CLAIMS_CUSTODY_OWNER_OFFSET_V3,
            RATIONAL_TERMINAL_IDENTITY_CLAIMS_CUSTODY_OWNER_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_EXPECTED_REPRESENTATION_REVISION_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_REPRESENTATION_REVISION_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_EXPECTED_CLAIMS_MARKET_REVISION_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_CLAIMS_MARKET_REVISION_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_EXPECTED_ACTOR_POSITION_REVISION_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_ACTOR_POSITION_REVISION_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_POSITION_REVISION_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_CUSTODY_POSITION_REVISION_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_EXPECTED_CUSTODY_REPLAY_REVISION_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_CUSTODY_REPLAY_REVISION_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_GENERATION_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_GENERATION_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_QUANTITY_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_QUANTITY_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_DENOMINATOR_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_EXPECTED_RECEIPT_SUPPLY_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_RECEIPT_SUPPLY_V3,
        )?,
        project_u32(
            RATIONAL_TERMINAL_HOT_OUTCOME_COUNT_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3,
        )?,
        project_u32(
            RATIONAL_TERMINAL_HOT_SELECTED_OUTCOME_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3,
        )?,
        project_u32(
            RATIONAL_TERMINAL_HOT_ASSET_COUNT_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_ASSET_COUNT_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_ASSET_COEFFICIENT_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_SHARD_SUPPLY_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_SHARD_SUPPLY_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_ACTOR_SHARDS_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_ACTOR_SHARDS_V3,
        )?,
        project_u64(
            RATIONAL_TERMINAL_HOT_ASSET_EXPECTED_STRUCTURED_SHARDS_OFFSET_V3,
            RATIONAL_TERMINAL_SCALAR_STRUCTURED_SHARDS_V3,
        )?,
    ];
    let geometry = RequestGeometryV1::new(
        u32::try_from(RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3).map_err(|_| {
            Error::RequestProfileArtifact(dclutch_request_profile_contract::Error::InvalidLength)
        })?,
        0,
        u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3).map_err(|_| {
            Error::RequestProfileArtifact(dclutch_request_profile_contract::Error::InvalidLength)
        })?,
        0,
        u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3).map_err(|_| {
            Error::RequestProfileArtifact(dclutch_request_profile_contract::Error::InvalidLength)
        })?,
        0,
    );
    let mut scratch = [0_u8; RATIONAL_TERMINAL_REQUEST_PROFILE_BYTES_V3];
    let mut output = [0_u8; RATIONAL_TERMINAL_REQUEST_PROFILE_BYTES_V3];
    encode_request_profile_v1_atomic(geometry, &fixed, &[], &mut scratch, &mut output)
        .map_err(Error::RequestProfileArtifact)?;
    Ok(output)
}

/// Encode the terminal transition which checks representation-local width.
///
/// Claims remains the sole economic authority. This small transition checks
/// only family-local invariants needed before dispatch: the selected claim is
/// bounded by representation width `K`, quantity and denominator are positive,
/// and the Bearer basis-vector coefficient equals the denominator. Product
/// result width `N` is independent authenticated terminal evidence.
pub fn encode_rational_terminal_transition_v3()
-> Result<[u8; RATIONAL_TERMINAL_TRANSITION_BYTES_V3]> {
    let s = |index: usize| {
        u16::try_from(index)
            .map(ScalarRegisterV3::common)
            .map_err(|_| {
                Error::TransitionArtifact(dclutch_transition_vm::v3::Error::InvalidRegister)
            })
    };
    let prelude = [
        InstructionV3::scalar_lt(
            s(RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3)?,
            s(RATIONAL_TERMINAL_SCALAR_OUTCOME_COUNT_V3)?,
        ),
        InstructionV3::nonzero(s(RATIONAL_TERMINAL_SCALAR_QUANTITY_V3)?),
        InstructionV3::nonzero(s(RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3)?),
        InstructionV3::scalar_eq(
            s(RATIONAL_TERMINAL_SCALAR_COEFFICIENT_V3)?,
            s(RATIONAL_TERMINAL_SCALAR_DENOMINATOR_V3)?,
        ),
    ];
    let geometry = ProgramGeometryV3 {
        common_scalars: u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3).map_err(|_| {
            Error::TransitionArtifact(dclutch_transition_vm::v3::Error::InvalidRegister)
        })?,
        item_scalar_stride: 0,
        common_identities: u16::try_from(RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3).map_err(
            |_| Error::TransitionArtifact(dclutch_transition_vm::v3::Error::InvalidRegister),
        )?,
        item_identity_stride: 0,
    };
    let mut scratch = [0_u8; RATIONAL_TERMINAL_TRANSITION_BYTES_V3];
    let mut output = [0_u8; RATIONAL_TERMINAL_TRANSITION_BYTES_V3];
    encode_program_atomic(geometry, &prelude, &[], &[], &mut scratch, &mut output)
        .map_err(Error::TransitionArtifact)?;
    Ok(output)
}

fn request(offset: usize) -> Result<RequestCoordinateV1> {
    u32::try_from(offset)
        .map(RequestCoordinateV1::fixed)
        .map_err(|_| {
            Error::RequestProfileArtifact(dclutch_request_profile_contract::Error::InvalidLength)
        })
}

fn identity(index: usize) -> Result<IdentityRegisterV1> {
    u16::try_from(index)
        .map(IdentityRegisterV1::common)
        .map_err(|_| {
            Error::RequestProfileArtifact(dclutch_request_profile_contract::Error::InvalidLength)
        })
}

fn scalar(index: usize) -> Result<ScalarRegisterV1> {
    u16::try_from(index)
        .map(ScalarRegisterV1::common)
        .map_err(|_| {
            Error::RequestProfileArtifact(dclutch_request_profile_contract::Error::InvalidLength)
        })
}

fn project_identity(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_identity(
        request(offset)?,
        identity(register)?,
    ))
}

fn project_u64(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_u64(
        request(offset)?,
        scalar(register)?,
    ))
}

fn project_u32(offset: usize, register: usize) -> Result<RequestInstructionV1> {
    Ok(RequestInstructionV1::project_u32(
        request(offset)?,
        scalar(register)?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_rational_representation_v2_contract::{
        ABSENT_REVISION, ASSET_BYTES_V2, AssetV2, RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3,
        RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3, RationalTerminalHotRequestV3,
        RepresentationRequestHeaderV2, RepresentationRequestV2,
    };
    use dclutch_request_profile_contract::{
        ProjectionRegistersV1, RequestProfileV1, project_atomic,
    };
    use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;
    use dclutch_transition_vm::v3::{
        ProgramV3, RegisterInput, RegisterOutput, execute_fold_atomic,
    };

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    fn family_bytes() -> [u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3] {
        let mut asset_bytes = [0_u8; ASSET_BYTES_V2];
        AssetV2 {
            shard_mint: id(20),
            actor_shard_account: id(21),
            structured_custody_account: id(22),
            claims_custody_owner: id(23),
            coefficient: 10,
            expected_shard_supply: 100,
            expected_actor_shards: 30,
            expected_structured_shards: 0,
        }
        .encode_into(&mut asset_bytes)
        .expect("asset");
        let child = RepresentationRequestV2::new(
            RepresentationRequestHeaderV2 {
                action: RepresentationActionV2::RedeemTerminal,
                caller_role: CallerRoleV2::Trading,
                release_set: id(1),
                market: id(2),
                graph_id: id(3),
                descriptor_id: id(4),
                parent_context: id(5),
                actor: id(6),
                receipt_mint: id(7),
                receipt_account: [0; 32],
                representation_authority: id(8),
                token_program: TOKEN_2022_PROGRAM_ID,
                realm: id(9),
                collateral_recipient: id(10),
                expected_representation_revision: 4,
                expected_claims_market_revision: 11,
                expected_actor_position_revision: ABSENT_REVISION,
                expected_custody_position_revision: 12,
                expected_custody_replay_revision: 13,
                generation: 14,
                quantity: 2,
                denominator: 10,
                expected_receipt_supply: 0,
                outcome_count: 3,
                selected_outcome: 2,
                asset_count: 1,
            },
            &asset_bytes,
        )
        .expect("child");
        let mut family = [0_u8; RATIONAL_TERMINAL_HOT_REQUEST_BYTES_V3];
        RationalTerminalHotRequestV3::from_child_into(child, &mut family).expect("family");
        family
    }

    #[test]
    fn typed_artifacts_keep_representation_k_independent_from_product_n() {
        let profile_bytes = encode_rational_terminal_request_profile_v3().expect("profile bytes");
        let profile = RequestProfileV1::decode(&profile_bytes).expect("profile");
        let family_bytes = family_bytes();
        let family = RationalTerminalHotRequestV3::decode(&family_bytes).expect("family");
        let family_digest = id(91);
        let expected = family
            .project_registers(family_digest, 258)
            .expect("semantic registers");

        let mut input_scalars = [0_u64; RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3];
        input_scalars[RATIONAL_TERMINAL_SCALAR_PRODUCT_OUTCOME_COUNT_V3] = 258;
        let mut input_identities = [[0_u8; 32]; RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3];
        input_identities[0] = family_digest;
        let mut scratch_scalars = input_scalars;
        let mut scratch_identities = input_identities;
        let mut output_scalars = input_scalars;
        let mut output_identities = input_identities;
        project_atomic(
            profile,
            258,
            &family_bytes,
            ProjectionRegistersV1 {
                input_scalars: &input_scalars,
                input_identities: &input_identities,
                scratch_scalars: &mut scratch_scalars,
                scratch_identities: &mut scratch_identities,
                output_scalars: &mut output_scalars,
                output_identities: &mut output_identities,
            },
        )
        .expect("project");
        assert_eq!(&output_scalars, expected.scalars());
        assert_eq!(&output_identities, expected.identities());

        let transition_bytes = encode_rational_terminal_transition_v3().expect("transition bytes");
        let transition = ProgramV3::decode(&transition_bytes).expect("transition");
        let mut transition_scratch_scalars = output_scalars;
        let mut transition_scratch_identities = output_identities;
        let mut transition_output_scalars = output_scalars;
        let mut transition_output_identities = output_identities;
        execute_fold_atomic(
            transition,
            258,
            RegisterInput {
                scalars: &output_scalars,
                identities: &output_identities,
            },
            RegisterOutput {
                scalars: &mut transition_scratch_scalars,
                identities: &mut transition_scratch_identities,
            },
            RegisterOutput {
                scalars: &mut transition_output_scalars,
                identities: &mut transition_output_identities,
            },
        )
        .expect("transition accepts");

        output_scalars[RATIONAL_TERMINAL_SCALAR_SELECTED_OUTCOME_V3] = 3;
        assert!(
            execute_fold_atomic(
                transition,
                258,
                RegisterInput {
                    scalars: &output_scalars,
                    identities: &output_identities,
                },
                RegisterOutput {
                    scalars: &mut transition_scratch_scalars,
                    identities: &mut transition_scratch_identities,
                },
                RegisterOutput {
                    scalars: &mut transition_output_scalars,
                    identities: &mut transition_output_identities,
                },
            )
            .is_err()
        );
    }

    #[test]
    fn request_profile_refuses_parent_and_shape_substitution() {
        let profile_bytes = encode_rational_terminal_request_profile_v3().expect("profile bytes");
        let profile = RequestProfileV1::decode(&profile_bytes).expect("profile");
        let mut family = family_bytes();
        *family
            .get_mut(RATIONAL_TERMINAL_HOT_PARENT_CONTEXT_OFFSET_V3)
            .expect("parent") = 1;
        let input_scalars = [0_u64; RATIONAL_TERMINAL_HOT_COMMON_SCALARS_V3];
        let input_identities = [[0_u8; 32]; RATIONAL_TERMINAL_HOT_COMMON_IDENTITIES_V3];
        let mut scratch_scalars = input_scalars;
        let mut scratch_identities = input_identities;
        let mut output_scalars = input_scalars;
        let mut output_identities = input_identities;
        assert!(
            project_atomic(
                profile,
                258,
                &family,
                ProjectionRegistersV1 {
                    input_scalars: &input_scalars,
                    input_identities: &input_identities,
                    scratch_scalars: &mut scratch_scalars,
                    scratch_identities: &mut scratch_identities,
                    output_scalars: &mut output_scalars,
                    output_identities: &mut output_identities,
                },
            )
            .is_err()
        );
    }
}
