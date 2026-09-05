//! Caller-side builder for the Claims-owned Fractional atomic child.
//!
//! The returned instruction is a Trading CPI instruction: its caller-authority
//! and Fractional-root signer bits are satisfied only by the activated Trading
//! program's two `invoke_signed` seed sets. It is not a directly submittable
//! wallet instruction.

use dclutch_claims::{
    frame_spec_v1::SignedDeltaFrameSpecV3,
    liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2,
    protocol_position_v2::ProtocolPositionSeedsV2,
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3, TERMINAL_SETTLEMENT_COLLATERAL_MINT_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_AUTHORITY_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_CALLER_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3, TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_EXPOSURE_STAGING_ACCOUNT_V3, TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_REALM_ACCOUNT_V3, TERMINAL_SETTLEMENT_REALM_STAGING_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3,
    },
};
use dclutch_claims::fractional::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ACTOR_V3,
    FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3, FRACTIONAL_ATOMIC_ROOT_V3, FRACTIONAL_ATOMIC_SHARD_MINT_V3,
    FRACTIONAL_ATOMIC_TERMS_RAW_V3, FRACTIONAL_ATOMIC_TERMS_STAGING_V3,
    FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3, FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3,
    FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3, FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3,
    FRACTIONAL_TERMINAL_ACTOR_V3, FRACTIONAL_TERMINAL_ROOT_V3, FRACTIONAL_TERMINAL_SHARD_MINT_V3,
    FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3, FRACTIONAL_TERMINAL_TERMS_RAW_V3,
    FRACTIONAL_TERMINAL_TERMS_STAGING_V3, FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
    FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3, FractionalCapabilityRootV4,
    FractionalExposureActionV2, FractionalExposureRequestV2,
};
use dclutch_claims::fractional_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FRACTIONAL_SELECTION_CONFIG_BYTES_V1,
    FractionalExposureTermsV2, encode_fractional_selection_config_v1,
    fractional_selection_config_from_terms_v1,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_claims::composition::{
    COMPOSITION_EXPOSURE_SCHEMA_ID_V3, CompositionExposureBundleV3,
    CompositionExposureExecutionExpectedV3, RecordAdmissionV3,
};
use dclutch_custody::token_svm::TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};

use crate::fractional::{Error, Result};

const MARKET: usize = 1;
const REGISTRY: usize = 13;
const TRADING_PROGRAM: usize = 14;
const CLAIMS_PROGRAM: usize = 16;
const POSITION_0: usize = 20;
const POSITION_1: usize = 21;

/// Build one exact atomic Claims CPI after checking every caller-owned key and privilege.
pub fn build_fractional_atomic_claims_instruction_v3(
    request: FractionalExposureRequestV2,
    terms: FractionalExposureTermsV2<'_>,
    root: FractionalCapabilityRootV4,
    accounts: &[AccountMeta],
) -> Result<Instruction> {
    request
        .bind_terms(terms)
        .map_err(Error::FractionalExposureRequest)?;
    if !matches!(
        request.action(),
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::WholeUnwrap
    ) || accounts.len() != FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3
    {
        return Err(Error::Claims);
    }
    let claims_program = meta(accounts, CLAIMS_PROGRAM)?.pubkey;
    let trading_program = meta(accounts, TRADING_PROGRAM)?.pubkey;
    let registry = meta(accounts, REGISTRY)?.pubkey;
    let bytes = request
        .to_bytes()
        .map_err(Error::FractionalExposureRequest)?;
    let request_digest = hash(&bytes).to_bytes();
    let input = request.input();
    let caller = CallerAuthoritySeedsV1::from_bytes(
        input.release_set,
        input.market,
        ExecutionRoleV1::Trading,
        input.terms,
        request_digest,
    )
    .map_err(Error::ReleaseSet)?;
    let expected_authority = Pubkey::find_program_address(&caller.as_slices(), &trading_program).0;
    let expected_market = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, input.market.as_slice()],
        &claims_program,
    )
    .0;
    let header = root.header();
    let root_state = root.state();
    let selection_config = selection_config_id(terms)?;
    let (expected_root, expected_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &trading_program);
    let root_identity_matches = match root_state {
        dclutch_claims::fractional::FractionalRootStateV2::V1(root) => {
            root.input().terms == input.terms
        }
        dclutch_claims::fractional::FractionalRootStateV2::V2(root) => {
            root.input().selection_config == selection_config
        }
    };
    if !root_identity_matches
        || root_state.market() != input.market
        || root_state.revision() != input.expected_revision
        || root_state.bump() != expected_bump
        || header.release_set().to_bytes() != input.release_set
        || header.market() != input.market
        || header.selection().config().to_bytes() != selection_config
        || meta(accounts, 0)?.pubkey != expected_authority
        || meta(accounts, MARKET)?.pubkey != expected_market
        || meta(accounts, FRACTIONAL_ATOMIC_ROOT_V3)?.pubkey != expected_root
        || meta(accounts, FRACTIONAL_ATOMIC_ACTOR_V3)?
            .pubkey
            .to_bytes()
            != input.owner
        || meta(accounts, FRACTIONAL_ATOMIC_SHARD_MINT_V3)?
            .pubkey
            .to_bytes()
            != terms
                .shard_mint(input.representation_coordinate)
                .map_err(Error::FractionalClaim)?
        || meta(accounts, FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3)?
            .pubkey
            .to_bytes()
            != match request.action() {
                FractionalExposureActionV2::Wrap => input.destination_token_account,
                FractionalExposureActionV2::WholeUnwrap => input.source_token_account,
                _ => return Err(Error::Claims),
            }
        || meta(accounts, FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3)?
            .pubkey
            .to_bytes()
            != terms.token_program()
    {
        return Err(Error::Claims);
    }

    require_record_pair(
        accounts,
        registry,
        FRACTIONAL_ATOMIC_TERMS_RAW_V3,
        FRACTIONAL_ATOMIC_TERMS_STAGING_V3,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
        input.terms,
    )?;
    require_record_pair(
        accounts,
        registry,
        FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
        FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3,
        TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
        input.token_behavior,
    )?;

    let spec = SignedDeltaFrameSpecV3::new(2).map_err(Error::FrameSpec)?;
    for index in 0..spec.account_count().map_err(Error::FrameSpec)? {
        let expected = spec.account(index).map_err(Error::FrameSpec)?.privileges();
        let observed = meta(accounts, usize::from(index))?;
        if observed.is_signer != expected.signer() || observed.is_writable != expected.writable() {
            return Err(Error::Claims);
        }
    }
    let actor_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(expected_market.to_bytes(), input.owner)
            .map_err(Error::ProtocolPosition)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let reserve_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(expected_market.to_bytes(), expected_root.to_bytes())
            .map_err(Error::ProtocolPosition)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let expected_positions = if input.owner < expected_root.to_bytes() {
        [actor_position, reserve_position]
    } else {
        [reserve_position, actor_position]
    };
    if meta(accounts, POSITION_0)?.pubkey != expected_positions[0]
        || meta(accounts, POSITION_1)?.pubkey != expected_positions[1]
    {
        return Err(Error::Claims);
    }
    for (index, signer, writable) in [
        (FRACTIONAL_ATOMIC_TERMS_RAW_V3, false, false),
        (FRACTIONAL_ATOMIC_TERMS_STAGING_V3, false, false),
        (FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3, false, false),
        (FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3, false, false),
        (FRACTIONAL_ATOMIC_ROOT_V3, true, true),
        (FRACTIONAL_ATOMIC_ACTOR_V3, true, false),
        (FRACTIONAL_ATOMIC_SHARD_MINT_V3, false, true),
        (FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3, false, true),
        (FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3, false, false),
    ] {
        let observed = meta(accounts, index)?;
        if observed.is_signer != signer || observed.is_writable != writable {
            return Err(Error::Claims);
        }
    }
    Ok(Instruction {
        program_id: claims_program,
        accounts: accounts.to_vec(),
        data: bytes.to_vec(),
    })
}

/// Build one exact terminal Claims/Custody/Token CPI from authenticated exposure bytes.
///
/// This remains an inner Trading instruction: coordinate zero and the Fractional
/// root are program signers, while the shard holder is the sole wallet signer.
pub fn build_fractional_terminal_atomic_claims_instruction_v3(
    request: FractionalExposureRequestV2,
    terms: FractionalExposureTermsV2<'_>,
    root: FractionalCapabilityRootV4,
    composition_exposure_bytes: &[u8],
    accounts: &[AccountMeta],
) -> Result<Instruction> {
    request
        .bind_terms(terms)
        .map_err(Error::FractionalExposureRequest)?;
    if !matches!(
        request.action(),
        FractionalExposureActionV2::TerminalRedeem | FractionalExposureActionV2::TerminalZeroBurn
    ) || accounts.len() != FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3
    {
        return Err(Error::Claims);
    }
    let bytes = request
        .to_bytes()
        .map_err(Error::FractionalExposureRequest)?;
    let request_digest = hash(&bytes).to_bytes();
    let input = request.input();
    let exposure_digest = hash(composition_exposure_bytes).to_bytes();
    CompositionExposureBundleV3::decode(
        composition_exposure_bytes,
        RecordAdmissionV3 {
            selected_id: input.exposure,
            finalized_id: input.exposure,
            recomputed_digest: exposure_digest,
            finalized_digest: exposure_digest,
            record_authenticated: true,
        },
    )
    .and_then(|exposure| {
        exposure.verify_execution_for(CompositionExposureExecutionExpectedV3 {
            market: input.market,
            result_domain: input.result_domain,
            release_set: input.release_set,
            product_basis: terms.product_basis(),
            representation_basis: terms.representation_basis(),
            product_width: terms.product_width(),
            representation_width: terms.representation_width(),
        })
    })
    .map_err(Error::RepresentationComposition)?;

    let claims_program = meta(accounts, CLAIMS_PROGRAM)?.pubkey;
    let trading_program = meta(accounts, TRADING_PROGRAM)?.pubkey;
    let registry = meta(accounts, REGISTRY)?.pubkey;
    let caller = CallerAuthoritySeedsV1::from_bytes(
        input.release_set,
        input.market,
        ExecutionRoleV1::Trading,
        input.terms,
        request_digest,
    )
    .map_err(Error::ReleaseSet)?;
    let expected_authority = Pubkey::find_program_address(&caller.as_slices(), &trading_program).0;
    let expected_market = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, input.market.as_slice()],
        &claims_program,
    )
    .0;
    let header = root.header();
    let root_state = root.state();
    let selection_config = selection_config_id(terms)?;
    let (expected_root, expected_bump) =
        Pubkey::find_program_address(&header.seeds().as_slices(), &trading_program);
    let reserve_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(expected_market.to_bytes(), expected_root.to_bytes())
            .map_err(Error::ProtocolPosition)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let root_identity_matches = match root_state {
        dclutch_claims::fractional::FractionalRootStateV2::V1(root) => {
            root.input().terms == input.terms
        }
        dclutch_claims::fractional::FractionalRootStateV2::V2(root) => {
            root.input().selection_config == selection_config
        }
    };
    if !root_identity_matches
        || root_state.market() != input.market
        || root_state.revision() != input.expected_revision
        || root_state.bump() != expected_bump
        || header.release_set().to_bytes() != input.release_set
        || header.market() != input.market
        || header.selection().config().to_bytes() != selection_config
        || meta(accounts, 0)?.pubkey != expected_authority
        || meta(accounts, MARKET)?.pubkey != expected_market
        || meta(accounts, POSITION_0)?.pubkey != reserve_position
        || meta(accounts, FRACTIONAL_TERMINAL_ROOT_V3)?.pubkey != expected_root
        || meta(accounts, FRACTIONAL_TERMINAL_ACTOR_V3)?
            .pubkey
            .to_bytes()
            != input.owner
        || meta(accounts, FRACTIONAL_TERMINAL_SHARD_MINT_V3)?
            .pubkey
            .to_bytes()
            != terms
                .shard_mint(input.representation_coordinate)
                .map_err(Error::FractionalClaim)?
        || meta(accounts, FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3)?
            .pubkey
            .to_bytes()
            != input.source_token_account
        || meta(accounts, TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3)?
            .pubkey
            .to_bytes()
            != input.terminal_digest
        || meta(accounts, TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3)?
            .pubkey
            .to_bytes()
            != terms.token_program()
    {
        return Err(Error::Claims);
    }

    for (raw, staging, schema, digest) in [
        (
            FRACTIONAL_TERMINAL_TERMS_RAW_V3,
            FRACTIONAL_TERMINAL_TERMS_STAGING_V3,
            FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            input.terms,
        ),
        (
            FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
            FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3,
            TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
            input.token_behavior,
        ),
        (
            TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3,
            TERMINAL_SETTLEMENT_EXPOSURE_STAGING_ACCOUNT_V3,
            COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
            exposure_digest,
        ),
    ] {
        require_record_pair(accounts, registry, raw, staging, schema, digest)?;
    }

    let spec = SignedDeltaFrameSpecV3::new(1).map_err(Error::FrameSpec)?;
    for index in 0..spec.account_count().map_err(Error::FrameSpec)? {
        let expected = spec.account(index).map_err(Error::FrameSpec)?.privileges();
        require_privilege(
            accounts,
            usize::from(index),
            expected.signer(),
            expected.writable(),
        )?;
    }
    for (index, signer, writable) in [
        (TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3, false, false),
        (
            TERMINAL_SETTLEMENT_EXPOSURE_STAGING_ACCOUNT_V3,
            false,
            false,
        ),
        (TERMINAL_SETTLEMENT_CUSTODY_CALLER_ACCOUNT_V3, false, false),
        (TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3, false, false),
        (TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3, false, false),
        (
            TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
            false,
            false,
        ),
        (
            TERMINAL_SETTLEMENT_RESOLUTION_PROGRAMDATA_ACCOUNT_V3,
            false,
            false,
        ),
        (TERMINAL_SETTLEMENT_REALM_ACCOUNT_V3, false, false),
        (TERMINAL_SETTLEMENT_REALM_STAGING_ACCOUNT_V3, false, false),
        (TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3, false, true),
        (TERMINAL_SETTLEMENT_COLLATERAL_MINT_ACCOUNT_V3, false, false),
        (TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3, false, true),
        (TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3, false, true),
        (
            TERMINAL_SETTLEMENT_CUSTODY_AUTHORITY_ACCOUNT_V3,
            false,
            false,
        ),
        (TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3, false, false),
        (FRACTIONAL_TERMINAL_TERMS_RAW_V3, false, false),
        (FRACTIONAL_TERMINAL_TERMS_STAGING_V3, false, false),
        (FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3, false, false),
        (FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3, false, false),
        (FRACTIONAL_TERMINAL_ROOT_V3, true, true),
        (FRACTIONAL_TERMINAL_ACTOR_V3, true, false),
        (FRACTIONAL_TERMINAL_SHARD_MINT_V3, false, true),
        (FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3, false, true),
    ] {
        require_privilege(accounts, index, signer, writable)?;
    }
    Ok(Instruction {
        program_id: claims_program,
        accounts: accounts.to_vec(),
        data: bytes.to_vec(),
    })
}

fn require_record_pair(
    accounts: &[AccountMeta],
    registry: Pubkey,
    raw_index: usize,
    staging_index: usize,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<()> {
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    if meta(accounts, raw_index)?.pubkey != raw || meta(accounts, staging_index)?.pubkey != staging
    {
        Err(Error::Claims)
    } else {
        Ok(())
    }
}

/// Recompute the sole market-free config a current capability header selects.
///
/// Terms remain the separate market-bound execution record. Comparing their
/// digest to `header.selection().config()` was the pre-split fixed point and
/// would accept a header that named execution terms instead of the manifest
/// config.
fn selection_config_id(terms: FractionalExposureTermsV2<'_>) -> Result<[u8; 32]> {
    let mut bytes = [0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(
        fractional_selection_config_from_terms_v1(terms),
        &mut bytes,
    )
    .map_err(Error::FractionalClaim)?;
    Ok(hash(&bytes).to_bytes())
}

fn meta(accounts: &[AccountMeta], index: usize) -> Result<&AccountMeta> {
    accounts.get(index).ok_or(Error::Claims)
}

fn require_privilege(
    accounts: &[AccountMeta],
    index: usize,
    signer: bool,
    writable: bool,
) -> Result<()> {
    let observed = meta(accounts, index)?;
    if observed.is_signer != signer || observed.is_writable != writable {
        Err(Error::Claims)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_market::capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
    use dclutch_core_contract::ContentId;
    use dclutch_claims::fractional::{
        FRACTIONAL_CAPABILITY_ROOT_BYTES_V4, FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4,
        FractionalExposureRequestInputV2, FractionalRootInputV1, FractionalRootV1,
        decode_fractional_capability_root_v4,
    };
    use dclutch_claims::fractional_kernel::{
        FractionalExposureTermsAdmissionV2, FractionalExposureTermsInputV2,
        encode_fractional_exposure_terms_v2, fractional_exposure_terms_bytes_v2,
    };
    use dclutch_registry::release_set::CapabilityExecutionSelectionV1;
    use dclutch_claims::composition::{
        CompositionExposureInputV3, CompositionExposureRowInputV3, CompositionExposureTermV3,
        composition_exposure_bytes_v3, encode_composition_exposure_v3_atomic,
    };
    use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;

    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }

    struct Fixture {
        request: FractionalExposureRequestV2,
        terms_bytes: Vec<u8>,
        terms_id: [u8; 32],
        root: FractionalCapabilityRootV4,
        accounts: Vec<AccountMeta>,
    }

    struct TerminalFixture {
        base: Fixture,
        exposure_bytes: Vec<u8>,
    }

    impl Fixture {
        fn terms(&self) -> FractionalExposureTermsV2<'_> {
            FractionalExposureTermsV2::decode(
                &self.terms_bytes,
                FractionalExposureTermsAdmissionV2 {
                    selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                    finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                    selected_terms_id: self.terms_id,
                    finalized_terms_id: self.terms_id,
                    recomputed_terms_digest: self.terms_id,
                    finalized_terms_digest: self.terms_id,
                    record_authenticated: true,
                },
            )
            .expect("terms")
        }
    }

    fn fixture() -> Fixture {
        let market = id(2);
        let release = id(3);
        let token_behavior = id(4);
        let actor = id(5);
        let destination = id(6);
        let mints = [id(7), id(8)];
        let width = fractional_exposure_terms_bytes_v2(mints.len()).expect("width");
        let mut scratch = vec![0; width];
        let mut terms_bytes = vec![0; width];
        encode_fractional_exposure_terms_v2(
            FractionalExposureTermsInputV2 {
                market,
                product_record: id(9),
                result_domain: id(10),
                release_set: release,
                token_program: TOKEN_2022_PROGRAM_ID,
                token_behavior,
                exposure_id: id(11),
                product_basis: id(12),
                representation_basis: id(13),
                graph_id: id(14),
                product_width: 3,
                denominator: 100,
                shard_mints: &mints,
            },
            &mut scratch,
            &mut terms_bytes,
        )
        .expect("encode terms");
        let terms_id = hash(&terms_bytes).to_bytes();
        let selection_config = selection_config_id(
            FractionalExposureTermsV2::decode(
                &terms_bytes,
                FractionalExposureTermsAdmissionV2 {
                    selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                    finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                    selected_terms_id: terms_id,
                    finalized_terms_id: terms_id,
                    recomputed_terms_digest: terms_id,
                    finalized_terms_digest: terms_id,
                    record_authenticated: true,
                },
            )
            .expect("terms"),
        )
        .expect("selection config");
        let request = FractionalExposureRequestV2::new(
            FractionalExposureActionV2::Wrap,
            FractionalExposureRequestInputV2 {
                release_set: release,
                market,
                product_record: id(9),
                result_domain: id(10),
                terms: terms_id,
                token_behavior,
                exposure: id(11),
                owner: actor,
                source_token_account: [0; 32],
                destination_token_account: destination,
                terminal_digest: [0; 32],
                expected_revision: 4,
                quantity: 2,
                representation_coordinate: 1,
            },
        )
        .expect("request");
        let claims = Pubkey::new_from_array(id(20));
        let trading = Pubkey::new_from_array(id(21));
        let registry = Pubkey::new_from_array(id(22));
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            ContentId::new(id(24)).expect("manifest"),
            ContentId::new(id(25)).expect("kind"),
            ContentId::new(id(26)).expect("release"),
            ContentId::new(selection_config).expect("selection config"),
        )
        .expect("selection");
        let header = CapabilityRootHeaderV1::new(
            ContentId::new(release).expect("release set"),
            market,
            1,
            selection,
            SelectedRecordBumpsV1::default(),
        )
        .expect("header");
        let (root_key, bump) = Pubkey::find_program_address(&header.seeds().as_slices(), &trading);
        let root_state = FractionalRootV1::new(FractionalRootInputV1 {
            bump,
            terms: terms_id,
            market,
            rent_beneficiary: id(23),
            revision: 4,
            historical_rent_principal: 1,
        })
        .expect("root");
        let mut root_bytes = [0_u8; FRACTIONAL_CAPABILITY_ROOT_BYTES_V4];
        root_bytes[..FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4]
            .copy_from_slice(&header.to_bytes());
        root_bytes[FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4..]
            .copy_from_slice(&root_state.to_bytes());
        assert!(decode_fractional_capability_root_v4(&root_state.to_bytes()).is_none());
        let mut substituted_header = root_bytes;
        substituted_header[0] ^= 1;
        assert!(decode_fractional_capability_root_v4(&substituted_header).is_none());
        let mut substituted_state = root_bytes;
        substituted_state[FRACTIONAL_CAPABILITY_ROOT_STATE_OFFSET_V4 + 11] = 1;
        assert!(decode_fractional_capability_root_v4(&substituted_state).is_none());
        let root = decode_fractional_capability_root_v4(&root_bytes).expect("composite root");
        let mut accounts = Vec::new();
        let spec = SignedDeltaFrameSpecV3::new(2).expect("spec");
        for index in 0..spec.account_count().expect("count") {
            let privileges = spec.account(index).expect("account").privileges();
            accounts.push(if privileges.writable() {
                AccountMeta::new(
                    Pubkey::new_from_array(id(u8::try_from(index + 40).expect("id"))),
                    privileges.signer(),
                )
            } else {
                AccountMeta::new_readonly(
                    Pubkey::new_from_array(id(u8::try_from(index + 40).expect("id"))),
                    privileges.signer(),
                )
            });
        }
        accounts.push(AccountMeta::new_readonly(Pubkey::default(), false));
        accounts.push(AccountMeta::new_readonly(Pubkey::default(), false));
        accounts.push(AccountMeta::new_readonly(Pubkey::default(), false));
        accounts.push(AccountMeta::new_readonly(Pubkey::default(), false));
        accounts.push(AccountMeta::new_readonly(root_key, true));
        accounts.push(AccountMeta::new_readonly(
            Pubkey::new_from_array(actor),
            true,
        ));
        accounts.push(AccountMeta::new(Pubkey::new_from_array(mints[1]), false));
        accounts.push(AccountMeta::new(Pubkey::new_from_array(destination), false));
        accounts.push(AccountMeta::new_readonly(
            Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID),
            false,
        ));
        accounts[FRACTIONAL_ATOMIC_ROOT_V3].is_writable = true;
        accounts[REGISTRY].pubkey = registry;
        accounts[TRADING_PROGRAM].pubkey = trading;
        accounts[CLAIMS_PROGRAM].pubkey = claims;
        let request_digest = hash(&request.to_bytes().expect("bytes")).to_bytes();
        let caller = CallerAuthoritySeedsV1::from_bytes(
            release,
            market,
            ExecutionRoleV1::Trading,
            terms_id,
            request_digest,
        )
        .expect("caller");
        accounts[0].pubkey = Pubkey::find_program_address(&caller.as_slices(), &trading).0;
        let claims_market =
            Pubkey::find_program_address(&[LIABILITY_BASIS_MARKET_SEED_V2, &market], &claims).0;
        accounts[MARKET].pubkey = claims_market;
        let actor_position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(claims_market.to_bytes(), actor)
                .expect("actor seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let reserve_position = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(claims_market.to_bytes(), root_key.to_bytes())
                .expect("reserve seeds")
                .as_slices(),
            &claims,
        )
        .0;
        let ordered = if actor < root_key.to_bytes() {
            [actor_position, reserve_position]
        } else {
            [reserve_position, actor_position]
        };
        accounts[POSITION_0].pubkey = ordered[0];
        accounts[POSITION_1].pubkey = ordered[1];
        for (raw_index, staging_index, schema, digest) in [
            (
                FRACTIONAL_ATOMIC_TERMS_RAW_V3,
                FRACTIONAL_ATOMIC_TERMS_STAGING_V3,
                FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                terms_id,
            ),
            (
                FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_RAW_V3,
                FRACTIONAL_ATOMIC_TOKEN_BEHAVIOR_STAGING_V3,
                TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                token_behavior,
            ),
        ] {
            accounts[raw_index].pubkey = Pubkey::find_program_address(
                &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0;
            accounts[staging_index].pubkey = Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0;
        }
        Fixture {
            request,
            terms_bytes,
            terms_id,
            root,
            accounts,
        }
    }

    fn terminal_fixture() -> TerminalFixture {
        let mut base = fixture();
        let first_terms = [CompositionExposureTermV3 {
            product_coordinate: 0,
            numerator: 1,
        }];
        let second_terms = [CompositionExposureTermV3 {
            product_coordinate: 1,
            numerator: 1,
        }];
        let rows = [
            CompositionExposureRowInputV3 {
                node_id: id(60),
                denominator: 1,
                terms: &first_terms,
            },
            CompositionExposureRowInputV3 {
                node_id: id(61),
                denominator: 1,
                terms: &second_terms,
            },
        ];
        let width = composition_exposure_bytes_v3(2, 2).expect("exposure width");
        let mut scratch = vec![0; width];
        let mut exposure_bytes = vec![0; width];
        encode_composition_exposure_v3_atomic(
            CompositionExposureInputV3 {
                market: id(2),
                result_domain: id(10),
                release_set: id(3),
                product_basis: id(12),
                representation_basis: id(13),
                graph_id: id(14),
                product_width: 3,
                rows: &rows,
            },
            &mut scratch,
            &mut exposure_bytes,
        )
        .expect("exposure");
        let input = base.request.input();
        base.request = FractionalExposureRequestV2::new(
            FractionalExposureActionV2::TerminalRedeem,
            FractionalExposureRequestInputV2 {
                source_token_account: id(6),
                destination_token_account: [0; 32],
                terminal_digest: id(90),
                ..input
            },
        )
        .expect("terminal request");

        let claims = base.accounts[CLAIMS_PROGRAM].pubkey;
        let trading = base.accounts[TRADING_PROGRAM].pubkey;
        let registry = base.accounts[REGISTRY].pubkey;
        let root_key = base.accounts[FRACTIONAL_ATOMIC_ROOT_V3].pubkey;
        let mut accounts = Vec::new();
        let spec = SignedDeltaFrameSpecV3::new(1).expect("spec");
        for index in 0..spec.account_count().expect("count") {
            let privileges = spec.account(index).expect("account").privileges();
            let key = Pubkey::new_from_array(id(u8::try_from(index + 100).expect("id")));
            accounts.push(if privileges.writable() {
                AccountMeta::new(key, privileges.signer())
            } else {
                AccountMeta::new_readonly(key, privileges.signer())
            });
        }
        for (signer, writable) in [
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, true),
            (false, false),
            (false, true),
            (false, true),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (false, false),
            (true, false),
            (true, false),
            (false, true),
            (false, true),
        ] {
            let key = Pubkey::new_unique();
            accounts.push(if writable {
                AccountMeta::new(key, signer)
            } else {
                AccountMeta::new_readonly(key, signer)
            });
        }
        assert_eq!(accounts.len(), FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3);
        accounts[REGISTRY].pubkey = registry;
        accounts[TRADING_PROGRAM].pubkey = trading;
        accounts[CLAIMS_PROGRAM].pubkey = claims;
        let terminal_input = base.request.input();
        let request_digest = hash(&base.request.to_bytes().expect("bytes")).to_bytes();
        let caller = CallerAuthoritySeedsV1::from_bytes(
            terminal_input.release_set,
            terminal_input.market,
            ExecutionRoleV1::Trading,
            terminal_input.terms,
            request_digest,
        )
        .expect("caller");
        accounts[0].pubkey = Pubkey::find_program_address(&caller.as_slices(), &trading).0;
        let claims_market = Pubkey::find_program_address(
            &[LIABILITY_BASIS_MARKET_SEED_V2, &terminal_input.market],
            &claims,
        )
        .0;
        accounts[MARKET].pubkey = claims_market;
        accounts[POSITION_0].pubkey = Pubkey::find_program_address(
            &ProtocolPositionSeedsV2::new(claims_market.to_bytes(), root_key.to_bytes())
                .expect("reserve seeds")
                .as_slices(),
            &claims,
        )
        .0;
        accounts[TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3].pubkey =
            Pubkey::new_from_array(terminal_input.terminal_digest);
        accounts[TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3].pubkey =
            Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        accounts[FRACTIONAL_TERMINAL_ROOT_V3].pubkey = root_key;
        accounts[FRACTIONAL_TERMINAL_ROOT_V3].is_writable = true;
        accounts[FRACTIONAL_TERMINAL_ACTOR_V3].pubkey =
            Pubkey::new_from_array(terminal_input.owner);
        accounts[FRACTIONAL_TERMINAL_SHARD_MINT_V3].pubkey = Pubkey::new_from_array(id(8));
        accounts[FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3].pubkey =
            Pubkey::new_from_array(terminal_input.source_token_account);
        for (raw, staging, schema, digest) in [
            (
                FRACTIONAL_TERMINAL_TERMS_RAW_V3,
                FRACTIONAL_TERMINAL_TERMS_STAGING_V3,
                FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
                terminal_input.terms,
            ),
            (
                FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
                FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3,
                TOKEN_BEHAVIOR_SELECTION_SCHEMA_ID_V2,
                terminal_input.token_behavior,
            ),
            (
                TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3,
                TERMINAL_SETTLEMENT_EXPOSURE_STAGING_ACCOUNT_V3,
                COMPOSITION_EXPOSURE_SCHEMA_ID_V3,
                hash(&exposure_bytes).to_bytes(),
            ),
        ] {
            accounts[raw].pubkey = Pubkey::find_program_address(
                &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0;
            accounts[staging].pubkey = Pubkey::find_program_address(
                &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
                &registry,
            )
            .0;
        }
        base.accounts = accounts;
        TerminalFixture {
            base,
            exposure_bytes,
        }
    }

    #[test]
    fn exact_inner_caller_builds_and_binds_both_program_signers() {
        let fixture = fixture();
        let instruction = build_fractional_atomic_claims_instruction_v3(
            fixture.request,
            fixture.terms(),
            fixture.root,
            &fixture.accounts,
        )
        .expect("instruction");
        assert_eq!(instruction.accounts.len(), 31);
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[FRACTIONAL_ATOMIC_ROOT_V3].is_signer);
        assert!(instruction.accounts[FRACTIONAL_ATOMIC_ROOT_V3].is_writable);
        assert!(instruction.accounts[FRACTIONAL_ATOMIC_ACTOR_V3].is_signer);
        assert_eq!(instruction.data.len(), 416);
    }

    #[test]
    fn root_actor_record_and_position_substitution_refuse() {
        for index in [
            0,
            POSITION_0,
            FRACTIONAL_ATOMIC_TERMS_RAW_V3,
            FRACTIONAL_ATOMIC_ROOT_V3,
            FRACTIONAL_ATOMIC_ACTOR_V3,
            FRACTIONAL_ATOMIC_SHARD_MINT_V3,
        ] {
            let mut fixture = fixture();
            fixture.accounts[index].pubkey = Pubkey::new_unique();
            assert!(
                build_fractional_atomic_claims_instruction_v3(
                    fixture.request,
                    fixture.terms(),
                    fixture.root,
                    &fixture.accounts,
                )
                .is_err(),
                "index {index}"
            );
        }
        let mut fixture = fixture();
        fixture.accounts[FRACTIONAL_ATOMIC_ROOT_V3].is_signer = false;
        assert!(
            build_fractional_atomic_claims_instruction_v3(
                fixture.request,
                fixture.terms(),
                fixture.root,
                &fixture.accounts,
            )
            .is_err()
        );
    }

    #[test]
    fn terminal_builder_binds_chain_derived_exposure_and_distinct_44_frame() {
        let fixture = terminal_fixture();
        let instruction = build_fractional_terminal_atomic_claims_instruction_v3(
            fixture.base.request,
            fixture.base.terms(),
            fixture.base.root,
            &fixture.exposure_bytes,
            &fixture.base.accounts,
        )
        .expect("terminal instruction");
        assert_eq!(instruction.accounts.len(), 44);
        assert!(instruction.accounts[0].is_signer);
        assert!(instruction.accounts[FRACTIONAL_TERMINAL_ROOT_V3].is_signer);
        assert!(instruction.accounts[FRACTIONAL_TERMINAL_ROOT_V3].is_writable);
        assert!(instruction.accounts[FRACTIONAL_TERMINAL_ACTOR_V3].is_signer);
        assert_eq!(instruction.data.len(), 416);
    }

    #[test]
    fn terminal_exposure_root_token_and_privilege_substitution_refuse() {
        for index in [
            0,
            POSITION_0,
            TERMINAL_SETTLEMENT_EXPOSURE_RAW_ACCOUNT_V3,
            TERMINAL_SETTLEMENT_CERTIFICATE_ACCOUNT_V3,
            FRACTIONAL_TERMINAL_ROOT_V3,
            FRACTIONAL_TERMINAL_ACTOR_V3,
            FRACTIONAL_TERMINAL_SHARD_MINT_V3,
            FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3,
        ] {
            let mut fixture = terminal_fixture();
            fixture.base.accounts[index].pubkey = Pubkey::new_unique();
            assert!(
                build_fractional_terminal_atomic_claims_instruction_v3(
                    fixture.base.request,
                    fixture.base.terms(),
                    fixture.base.root,
                    &fixture.exposure_bytes,
                    &fixture.base.accounts,
                )
                .is_err(),
                "index {index}"
            );
        }
        let mut fixture = terminal_fixture();
        fixture.exposure_bytes[16] ^= 1;
        assert!(
            build_fractional_terminal_atomic_claims_instruction_v3(
                fixture.base.request,
                fixture.base.terms(),
                fixture.base.root,
                &fixture.exposure_bytes,
                &fixture.base.accounts,
            )
            .is_err()
        );
        let mut fixture = terminal_fixture();
        fixture.base.accounts[TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3].is_writable = false;
        assert!(
            build_fractional_terminal_atomic_claims_instruction_v3(
                fixture.base.request,
                fixture.base.terms(),
                fixture.base.root,
                &fixture.exposure_bytes,
                &fixture.base.accounts,
            )
            .is_err()
        );
    }
}
