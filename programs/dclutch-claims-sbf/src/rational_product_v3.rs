//! ProductRuntimeV3 authentication for Rational representation execution.
//!
//! This is the sole Rational reader for Product, ProductBasis, descriptor, and
//! representation graph authority. It projects only ephemeral checked facts;
//! the Registry records and canonical LBV2/Core state remain semantic owners.

extern crate alloc;

use alloc::boxed::Box;

use dclutch_claims_svm::liability_basis_state_v2::{
    LiabilityBasisMarketViewV2 as MarketViewV2, LiabilityBasisPositionViewV2 as PositionViewV2,
};
use dclutch_market_core_codec::{
    CoreState, MarketCoreStateSeedsV2, Phase as CorePhase, STATE_BYTES,
};
use dclutch_product_payoff_v2_codec::runtime_v3::BasisKindV3;
use dclutch_product_runtime_v2::ContentId;
use dclutch_product_runtime_v2_svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV3,
    representation_v3::{
        RepresentationRuntimeContextV3, RepresentationRuntimeFrameV3,
        authenticate_product_representation_v3,
    },
};
use dclutch_rational_representation_v2_contract::{
    RepresentationActionV2, RepresentationRequestV2,
};
use dclutch_rational_representation_v2_kernel::product_v3::{
    RepresentationAdmissionV3, TerminalScenarioV3,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{
    account_info::AccountInfo, hash::hash, program_error::ProgramError, pubkey::Pubkey, rent::Rent,
    sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;

use super::{ClaimsSbfError, liability_basis_v2::LIABILITY_BASIS_MARKET_SEED_V2};

#[derive(Clone, Copy)]
pub(crate) struct RationalProductFrameV3<'accounts, 'info> {
    pub(crate) aggregate: &'accounts AccountInfo<'info>,
    pub(crate) actor_position: &'accounts AccountInfo<'info>,
    pub(crate) linked_basis_record: &'accounts AccountInfo<'info>,
    pub(crate) linked_basis_staging: &'accounts AccountInfo<'info>,
    pub(crate) product_record: &'accounts AccountInfo<'info>,
    pub(crate) product_staging: &'accounts AccountInfo<'info>,
    pub(crate) result_domain_record: &'accounts AccountInfo<'info>,
    pub(crate) result_domain_staging: &'accounts AccountInfo<'info>,
    pub(crate) portfolio_record: &'accounts AccountInfo<'info>,
    pub(crate) portfolio_staging: &'accounts AccountInfo<'info>,
    pub(crate) descriptor_record: &'accounts AccountInfo<'info>,
    pub(crate) descriptor_staging: &'accounts AccountInfo<'info>,
    pub(crate) graph_record: &'accounts AccountInfo<'info>,
    pub(crate) graph_staging: &'accounts AccountInfo<'info>,
    pub(crate) receipt_mint: &'accounts AccountInfo<'info>,
    pub(crate) token_program: &'accounts AccountInfo<'info>,
    pub(crate) rent: &'accounts AccountInfo<'info>,
    pub(crate) registry: &'accounts AccountInfo<'info>,
    pub(crate) core_market: &'accounts AccountInfo<'info>,
    pub(crate) core_program: &'accounts AccountInfo<'info>,
    pub(crate) claims_program: &'accounts AccountInfo<'info>,
}

#[derive(Clone, Copy)]
pub(crate) struct AuthenticatedRationalProductV3 {
    pub(crate) market: MarketViewV2,
    pub(crate) core: CoreState,
    pub(crate) product_record_digest: [u8; 32],
    pub(crate) result_outcome_count: u32,
    pub(crate) admission: RepresentationAdmissionV3,
}

#[inline(never)]
pub(crate) fn authenticate_rational_product_v3(
    program_id: &Pubkey,
    frame: RationalProductFrameV3<'_, '_>,
    request: RepresentationRequestV2<'_>,
) -> Result<Box<AuthenticatedRationalProductV3>, ProgramError> {
    let header = request.header();
    let aggregate = frame
        .aggregate
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let market = MarketViewV2::decode(&aggregate).map_err(|_| ClaimsSbfError::Economic)?;
    let expected_market = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, header.market.as_slice()],
        program_id,
    )
    .0;
    if frame.aggregate.owner != program_id
        || frame.aggregate.key != &expected_market
        || market.logical_market != header.market
        || market.release_set != header.release_set
        || market.registry_program != frame.registry.key.to_bytes()
        || market.claim_count != header.outcome_count
        || market.generation != header.generation
        || (header.action.uses_claims()
            && market.revision != header.expected_claims_market_revision)
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    drop(aggregate);
    if matches!(
        header.action,
        RepresentationActionV2::Denominate | RepresentationActionV2::Reconstitute
    ) {
        let actor = frame
            .actor_position
            .try_borrow_data()
            .map_err(|_| ClaimsSbfError::Accounts)?;
        let position = PositionViewV2::decode(&actor).map_err(|_| ClaimsSbfError::Economic)?;
        if position.market_account != frame.aggregate.key.to_bytes()
            || position.owner != header.actor
            || position.basis_id != market.basis_id
            || position.claim_count != market.claim_count
            || position.revision != header.expected_actor_position_revision
        {
            return Err(ClaimsSbfError::Identity.into());
        }
    }
    let core = authenticate_core(frame, market, header.action)?;
    let rent = Rent::from_account_info(frame.rent).map_err(|_| ClaimsSbfError::Accounts)?;
    let authenticated = authenticate_product_representation_v3(
        frame.registry.key,
        &rent,
        ContentId::new(core.identity.product_record.to_bytes())
            .map_err(|_| ClaimsSbfError::Identity)?,
        ContentId::new(header.descriptor_id).map_err(|_| ClaimsSbfError::Identity)?,
        RepresentationRuntimeContextV3 {
            claims_program: *frame.claims_program.key,
            market: Pubkey::new_from_array(header.market),
            release_set: Pubkey::new_from_array(header.release_set),
            claims_basis_id: ContentId::new(market.basis_id)
                .map_err(|_| ClaimsSbfError::Identity)?,
            claims_width: market.claim_count,
            receipt_mint: *frame.receipt_mint.key,
            token_program: *frame.token_program.key,
        },
        RepresentationRuntimeFrameV3 {
            product: ProductRuntimeFrameV3 {
                product: record(frame.product_record, frame.product_staging),
                result_domain: record(frame.result_domain_record, frame.result_domain_staging),
                portfolio: record(frame.portfolio_record, frame.portfolio_staging),
                linked_basis: record(frame.linked_basis_record, frame.linked_basis_staging),
            },
            descriptor: record(frame.descriptor_record, frame.descriptor_staging),
            graph: record(frame.graph_record, frame.graph_staging),
        },
    )
    .map_err(|_| ClaimsSbfError::Identity)?;
    let admission = authenticated.admission;
    if admission.descriptor_id() != header.descriptor_id
        || admission.graph_id() != header.graph_id
        || admission.market_id() != header.market
        || admission.release_set_id() != header.release_set
        || admission.semantic_basis_id() != market.basis_id
        || admission.product_id() != market.product_instance_id
        || admission.basis_width() != header.outcome_count
        || admission.receipt_mint() != header.receipt_mint
        || admission.token_program() != header.token_program
        || admission.representation_authority() != header.representation_authority
        || authenticated.product_record_digest.to_bytes() != core.identity.product_record.to_bytes()
        || (header.action == RepresentationActionV2::RedeemTerminal
            && core.terminal_winner >= authenticated.result_outcome_count)
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(Box::new(AuthenticatedRationalProductV3 {
        market,
        core,
        product_record_digest: authenticated.product_record_digest.to_bytes(),
        result_outcome_count: authenticated.result_outcome_count,
        admission,
    }))
}

fn authenticate_core(
    frame: RationalProductFrameV3<'_, '_>,
    market: MarketViewV2,
    action: RepresentationActionV2,
) -> Result<CoreState, ProgramError> {
    if frame.core_market.owner != frame.core_program.key
        || frame.core_market.key.to_bytes() != market.logical_market
        || frame.core_market.data_len() != STATE_BYTES
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    let bytes = frame
        .core_market
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    let core = CoreState::decode(&bytes).map_err(|_| ClaimsSbfError::Identity)?;
    let expected = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(core.identity).as_slices(),
        frame.core_program.key,
    )
    .0;
    let expected_phase = if action == RepresentationActionV2::RedeemTerminal {
        CorePhase::Terminal
    } else {
        CorePhase::Open
    };
    if expected != *frame.core_market.key
        || core.phase != expected_phase
        || core.identity.market_id.to_bytes() != market.logical_market
        || core.identity.product_id.to_bytes() != market.product_instance_id
        || core.identity.selected_release_set.to_bytes() != market.release_set
        || core.identity.registry_program.to_bytes() != frame.registry.key.to_bytes()
        || core.identity.generation != market.generation
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(core)
}

const fn record<'accounts, 'info>(
    raw: &'accounts AccountInfo<'info>,
    staging: &'accounts AccountInfo<'info>,
) -> FinalizedRecordFrameV2<'accounts, 'info> {
    FinalizedRecordFrameV2 { raw, staging }
}

#[inline(never)]
pub(crate) fn authenticate_terminal_scenario_v3(
    authenticated: &AuthenticatedRationalProductV3,
    core_program: &AccountInfo<'_>,
    rent_account: &AccountInfo<'_>,
    coordinate: &AccountInfo<'_>,
    coordinate_staging: &AccountInfo<'_>,
) -> Result<TerminalScenarioV3, ProgramError> {
    match authenticated.admission.basis_kind() {
        BasisKindV3::CategoricalQ1 => {
            require_coordinate_placeholders(core_program, coordinate, coordinate_staging)?;
            Ok(TerminalScenarioV3::Categorical(
                authenticated.core.terminal_winner,
            ))
        }
        BasisKindV3::GradedExactComplement => {
            let failure = authenticated
                .result_outcome_count
                .checked_sub(1)
                .ok_or(ClaimsSbfError::Identity)?;
            if authenticated.core.terminal_winner == failure {
                require_coordinate_placeholders(core_program, coordinate, coordinate_staging)?;
                return Ok(TerminalScenarioV3::Failure);
            }
            let digest = authenticated
                .core
                .terminal_receipt
                .ok_or(ClaimsSbfError::Identity)?
                .to_bytes();
            authenticate_terminal_coordinate(
                core_program,
                rent_account,
                coordinate,
                coordinate_staging,
                digest,
            )?;
            let bytes = coordinate
                .try_borrow_data()
                .map_err(|_| ClaimsSbfError::Accounts)?;
            if bytes.len() != super::liability_basis_v2::TERMINAL_COORDINATE_BYTES_V2
                || array::<8>(&bytes, 0)? != super::liability_basis_v2::TERMINAL_COORDINATE_MAGIC_V2
                || u16_at(&bytes, 8)? != 2
                || bytes
                    .get(10..16)
                    .is_none_or(|slice| slice.iter().any(|byte| *byte != 0))
                || bytes
                    .get(28..32)
                    .is_none_or(|slice| slice.iter().any(|byte| *byte != 0))
            {
                return Err(ClaimsSbfError::Identity.into());
            }
            let numerator = i64::from_le_bytes(array(&bytes, 16)?);
            let denominator = u32::from_le_bytes(array(&bytes, 24)?);
            if denominator == 0 {
                return Err(ClaimsSbfError::Identity.into());
            }
            Ok(TerminalScenarioV3::Rational {
                numerator: i128::from(numerator),
                denominator: u64::from(denominator),
            })
        }
    }
}

fn require_coordinate_placeholders(
    core_program: &AccountInfo<'_>,
    coordinate: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
) -> Result<(), ProgramError> {
    if coordinate.key != core_program.key || staging.key != core_program.key {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

fn authenticate_terminal_coordinate(
    core_program: &AccountInfo<'_>,
    rent_account: &AccountInfo<'_>,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    digest: [u8; 32],
) -> Result<(), ProgramError> {
    let schema = super::liability_basis_v2::TERMINAL_COORDINATE_SCHEMA_RELEASE_ID_V2;
    let expected_raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
        core_program.key,
    )
    .0;
    let expected_staging = Pubkey::find_program_address(
        &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
        core_program.key,
    )
    .0;
    let rent = Rent::from_account_info(rent_account).map_err(|_| ClaimsSbfError::Accounts)?;
    let bytes = raw
        .try_borrow_data()
        .map_err(|_| ClaimsSbfError::Accounts)?;
    if raw.key != &expected_raw
        || raw.owner != core_program.key
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || hash(&bytes).to_bytes() != digest
        || !rent.is_exempt(raw.lamports(), bytes.len())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.data_len() != 0
        || staging.is_signer
        || staging.is_writable
        || staging.executable
    {
        return Err(ClaimsSbfError::Identity.into());
    }
    Ok(())
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N], ProgramError> {
    bytes
        .get(offset..offset.checked_add(N).ok_or(ClaimsSbfError::Identity)?)
        .and_then(|slice| slice.try_into().ok())
        .ok_or_else(|| ClaimsSbfError::Identity.into())
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, ProgramError> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}
