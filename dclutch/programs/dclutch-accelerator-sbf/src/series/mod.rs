//! The Series shadow: stateless Shadow-AOT evaluation of one exact
//! recurring-Series artifact bundle, selected at compile time.
//!
//! The comparison core comes first. A physical evaluation is possible only
//! through a generator-emitted bundle selected at build time
//! (`DCLUTCH_SERIES_SHADOW_GENERATED_INCLUDE`); there is deliberately no
//! instruction that accepts caller-supplied artifact bytes, and an ELF built
//! without a selected bundle refuses every Shadow request as
//! `NoSelectedRelease`.

/// Exact generic-interpreter and Series-semantic comparison boundary.
pub mod evaluator;
/// Compile-time selected, generator-produced release bundle boundary.
pub mod release;

use alloc::vec::Vec;

use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_market::execution_strategy::{
    shadow_digest_v3::ShadowRuntimeObservationV3,
    shadow_v3::{ShadowAckV3, ShadowRequestV3},
};
use dclutch_product::svm_reader::{
    FinalizedRecordFrameV2, ProductRuntimeFrameV3,
    authenticate_content_addressed_product_runtime_v3,
};
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_trading::series::{
    AuthenticatedProductProjectionV2,
    generated::{
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    },
    request::{SERIES_ACTION_HEADER_BYTES_V3, SeriesActionRequestV3, SeriesActionV3},
};
use dclutch_trading::shadow_accelerator_auth::{
    AuthenticatedShadowAcceleratorInvocationV4, authenticate_shadow_accelerator_invocation_v4,
};
use dclutch_vm::account_profile::{
    AccountObservationV1,
    v2::{AccountPrestateV2, AccountProfileV2},
};
use solana_program::{
    account_info::AccountInfo, clock::Clock, entrypoint::ProgramResult, hash::hash,
    program::set_return_data, pubkey::Pubkey, sysvar::SysvarSerialize,
};
use solana_sdk_ids::system_program;

use self::{
    evaluator::{
        SERIES_CLOCK_COORDINATE_V4, SERIES_LINKED_BASIS_STAGING_COORDINATE_V4,
        SERIES_OCCURRENCE_RAW_COORDINATE_V4, SERIES_OCCURRENCE_STAGING_COORDINATE_V4,
        SERIES_TEMPLATE_RAW_COORDINATE_V4, SERIES_TEMPLATE_STAGING_COORDINATE_V4,
        SERIES_TICKET_RAW_COORDINATE_V4, SERIES_TICKET_STAGING_COORDINATE_V4,
        SeriesShadowAuthenticatedFactsV4, SeriesShadowEvaluationV4, evaluate_series_shadow_aot_v4,
    },
    release::{SelectedSeriesShadowReleaseV1, selected_series_shadow_release_v1},
};

const CONFIG_COORDINATE: usize = 1;
const PRODUCT_RAW_COORDINATE: usize = 2;
const PORTFOLIO_RAW_COORDINATE: usize = 3;
const LINKED_BASIS_RAW_COORDINATE: usize = 4;
const PRODUCT_STAGING_COORDINATE: usize = 26;
const RESULT_DOMAIN_RAW_COORDINATE: usize = 27;
const RESULT_DOMAIN_STAGING_COORDINATE: usize = 28;
const PORTFOLIO_STAGING_COORDINATE: usize = 30;

/// Stable physical refusal from the Series Shadow SBF adapter.
#[repr(u32)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesShadowSbfErrorV4 {
    /// Common Trading could not authenticate the Shadow callback.
    InvalidInvocation = 0xC200,
    /// This ELF has no deliberately selected generated release.
    NoSelectedRelease = 0xC201,
    /// Profile13 geometry or normalized runtime observations differed.
    Runtime = 0xC202,
    /// A finalized Series or Product record did not authenticate.
    FinalizedRecord = 0xC203,
    /// The typed acknowledgement could not be encoded.
    InvalidAcknowledgement = 0xC204,
}

dclutch_refusal_registry::pin_refusal_band!(
    SeriesShadowSbfErrorV4,
    dclutch_refusal_registry::ACCELERATOR_REFUSAL_BASE + 0x200,
    [
        InvalidInvocation,
        NoSelectedRelease,
        Runtime,
        FinalizedRecord,
        InvalidAcknowledgement
    ]
);

/// Authenticate and evaluate one exact compile-time selected Shadow request.
///
/// Physical authentication failure returns a program error and no data. Once
/// the complete callback and runtime have authenticated, a semantic mismatch
/// returns the canonical typed refused acknowledgement. Trading accepts only
/// an exact accepted acknowledgement and remains the sole state/CPI authority.
pub fn process(
    program_id: &Pubkey,
    accounts: &[AccountInfo<'_>],
    instruction_data: &[u8],
) -> ProgramResult {
    let invocation =
        authenticate_shadow_accelerator_invocation_v4(program_id, accounts, instruction_data)
            .map_err(|_| SeriesShadowSbfErrorV4::InvalidInvocation)?;
    evaluate_selected_and_publish(&invocation, instruction_data)
}

#[inline(never)]
fn evaluate_selected_and_publish(
    invocation: &AuthenticatedShadowAcceleratorInvocationV4<'_, '_, '_>,
    instruction_data: &[u8],
) -> ProgramResult {
    let selected = selected_series_shadow_release_v1()
        .map_err(|_| SeriesShadowSbfErrorV4::NoSelectedRelease)?
        .ok_or(SeriesShadowSbfErrorV4::NoSelectedRelease)?;
    let request = invocation.request();
    let request_digest = invocation.request_digest();
    let acknowledgement =
        match evaluate_authenticated_invocation(invocation, instruction_data, selected) {
            Ok(accepted) => accepted,
            Err(SeriesShadowSbfErrorV4::Runtime | SeriesShadowSbfErrorV4::FinalizedRecord) => {
                ShadowAckV3::refused(request, request_digest)
            }
            Err(error) => return Err(error.into()),
        };
    publish_acknowledgement(acknowledgement)
}

#[inline(never)]
fn publish_acknowledgement(acknowledgement: ShadowAckV3) -> ProgramResult {
    let output = acknowledgement
        .to_bytes()
        .map_err(|_| SeriesShadowSbfErrorV4::InvalidAcknowledgement)?;
    set_return_data(&output);
    Ok(())
}

#[inline(never)]
fn evaluate_authenticated_invocation(
    invocation: &AuthenticatedShadowAcceleratorInvocationV4<'_, '_, '_>,
    instruction_data: &[u8],
    selected: SelectedSeriesShadowReleaseV1,
) -> Result<ShadowAckV3, SeriesShadowSbfErrorV4> {
    let request = invocation.request();
    let runtime = invocation.runtime_accounts();
    let funding_count = funding_count(request)?;
    let span = [u32::try_from(funding_count).map_err(|_| SeriesShadowSbfErrorV4::Runtime)?];
    let profile = AccountProfileV2::decode(selected.bundle.account_profile)
        .map_err(|_| SeriesShadowSbfErrorV4::Runtime)?;
    if profile
        .logical_account_count_with_dynamic_spans(request.shape.tail_count, &span)
        .map_err(|_| SeriesShadowSbfErrorV4::Runtime)?
        != runtime.len()
    {
        return Err(SeriesShadowSbfErrorV4::Runtime);
    }

    let series_request = SeriesActionRequestV3::decode(request.family_request)
        .map_err(|_| SeriesShadowSbfErrorV4::Runtime)?;
    if series_request.action() != SeriesActionV3::Consume
        || request.family_request.len() < SERIES_ACTION_HEADER_BYTES_V3
    {
        return Err(SeriesShadowSbfErrorV4::Runtime);
    }
    authenticate_series_records(runtime, invocation.registry().key, series_request)?;
    let product_runtime = authenticate_product(runtime, invocation.registry().key)?;
    let product = AuthenticatedProductProjectionV2::new(
        core_content_id(
            product_runtime
                .runtime
                .product_record
                .content_digest
                .to_bytes(),
        )?,
        core_content_id(product_runtime.runtime.product_id.to_bytes())?,
        core_content_id(
            product_runtime
                .runtime
                .result_domain_record
                .content_digest
                .to_bytes(),
        )?,
    );
    let now_slot = Clock::from_account_info(account(runtime, SERIES_CLOCK_COORDINATE_V4)?)
        .map_err(|_| SeriesShadowSbfErrorV4::Runtime)?
        .slot;

    let runtime_data = runtime
        .iter()
        .map(|current| {
            current
                .try_borrow_data()
                .map_err(|_| SeriesShadowSbfErrorV4::Runtime)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let config_is_variable = profile
        .rule(
            false,
            u16::try_from(CONFIG_COORDINATE).map_err(|_| SeriesShadowSbfErrorV4::Runtime)?,
        )
        .map_err(|_| SeriesShadowSbfErrorV4::Runtime)?
        .prestate()
        == AccountPrestateV2::AdapterAuthenticatedVariableData;
    let projections = LogicalProjectionKeysV4 {
        config: series_request.template().to_bytes(),
        product: product_runtime
            .runtime
            .product_record
            .content_digest
            .to_bytes(),
        portfolio: product_runtime
            .runtime
            .portfolio_record
            .content_digest
            .to_bytes(),
        linked_basis: product_runtime
            .linked_basis_record
            .content_digest
            .to_bytes(),
    };
    let mut profile_observations = Vec::with_capacity(runtime.len());
    let mut transcript_observations = Vec::with_capacity(runtime.len());
    for (coordinate, (current, data)) in runtime.iter().zip(&runtime_data).enumerate() {
        let physical = profile
            .physical_account_ordinal_with_dynamic_spans(
                request.shape.tail_count,
                &span,
                coordinate,
            )
            .map_err(|_| SeriesShadowSbfErrorV4::Runtime)?;
        let privileges = profile
            .physical_account_geometry_with_dynamic_spans(request.shape.tail_count, &span, physical)
            .map_err(|_| SeriesShadowSbfErrorV4::Runtime)?
            .privileges();
        if privileges.executable() != current.executable {
            return Err(SeriesShadowSbfErrorV4::Runtime);
        }
        let key = logical_projection_key(coordinate, current.key, &projections);
        let profile_observation = if coordinate == LINKED_BASIS_RAW_COORDINATE
            || (coordinate == CONFIG_COORDINATE && config_is_variable)
        {
            AccountObservationV1::new_adapter_authenticated_variable_data(
                key,
                current.owner.as_array(),
                current.lamports(),
                data.as_ref(),
                privileges.signer(),
                privileges.writable(),
                privileges.executable(),
            )
        } else {
            AccountObservationV1::new(
                key,
                current.owner.as_array(),
                current.lamports(),
                data.as_ref(),
                privileges.signer(),
                privileges.writable(),
                privileges.executable(),
            )
        };
        profile_observations.push(profile_observation);
        transcript_observations.push(ShadowRuntimeObservationV3 {
            key: *key,
            owner: current.owner.to_bytes(),
            lamports: current.lamports(),
            data: data.as_ref(),
            signer: false,
            writable: false,
            executable: current.executable,
        });
    }
    invocation
        .validate_runtime_transcript(&transcript_observations)
        .map_err(|_| SeriesShadowSbfErrorV4::Runtime)?;
    evaluate_series_shadow_aot_v4(SeriesShadowEvaluationV4 {
        shadow_request: instruction_data,
        bundle: selected.bundle,
        profile_observations: &profile_observations,
        transcript_observations: &transcript_observations,
        authenticated_facts: SeriesShadowAuthenticatedFactsV4 { product, now_slot },
    })
    .map_err(|_| SeriesShadowSbfErrorV4::Runtime)
}

#[derive(Clone, Copy)]
struct LogicalProjectionKeysV4 {
    config: [u8; 32],
    product: [u8; 32],
    portfolio: [u8; 32],
    linked_basis: [u8; 32],
}

/// Borrowed, not copied: the observation bank holds one entry per logical
/// coordinate and the SBF allocator never frees, so a by-value identity is
/// paid for twice in every alias.
const fn logical_projection_key<'a>(
    coordinate: usize,
    physical: &'a Pubkey,
    projections: &'a LogicalProjectionKeysV4,
) -> &'a [u8; 32] {
    match coordinate {
        CONFIG_COORDINATE => &projections.config,
        PRODUCT_RAW_COORDINATE => &projections.product,
        PORTFOLIO_RAW_COORDINATE => &projections.portfolio,
        LINKED_BASIS_RAW_COORDINATE => &projections.linked_basis,
        _ => physical.as_array(),
    }
}

fn funding_count(request: ShadowRequestV3<'_>) -> Result<usize, SeriesShadowSbfErrorV4> {
    evaluator::funding_count(request).map_err(|_| SeriesShadowSbfErrorV4::Runtime)
}

fn authenticate_product<'accounts, 'info>(
    runtime: &'accounts [AccountInfo<'info>],
    registry: &Pubkey,
) -> Result<
    dclutch_product::svm_reader::AuthenticatedProductRuntimeV3<'accounts, 'info>,
    SeriesShadowSbfErrorV4,
> {
    authenticate_content_addressed_product_runtime_v3(
        registry,
        ProductRuntimeFrameV3 {
            product: FinalizedRecordFrameV2 {
                raw: account(runtime, PRODUCT_RAW_COORDINATE)?,
                staging: account(runtime, PRODUCT_STAGING_COORDINATE)?,
            },
            result_domain: FinalizedRecordFrameV2 {
                raw: account(runtime, RESULT_DOMAIN_RAW_COORDINATE)?,
                staging: account(runtime, RESULT_DOMAIN_STAGING_COORDINATE)?,
            },
            portfolio: FinalizedRecordFrameV2 {
                raw: account(runtime, PORTFOLIO_RAW_COORDINATE)?,
                staging: account(runtime, PORTFOLIO_STAGING_COORDINATE)?,
            },
            linked_basis: FinalizedRecordFrameV2 {
                raw: account(runtime, LINKED_BASIS_RAW_COORDINATE)?,
                staging: account(runtime, SERIES_LINKED_BASIS_STAGING_COORDINATE_V4)?,
            },
        },
    )
    .map_err(|_| SeriesShadowSbfErrorV4::FinalizedRecord)
}

fn authenticate_series_records(
    runtime: &[AccountInfo<'_>],
    registry: &Pubkey,
    request: SeriesActionRequestV3<'_>,
) -> Result<(), SeriesShadowSbfErrorV4> {
    let occurrence = request
        .occurrence()
        .ok_or(SeriesShadowSbfErrorV4::FinalizedRecord)?;
    let ticket = request
        .ticket()
        .ok_or(SeriesShadowSbfErrorV4::FinalizedRecord)?;
    for (raw_coordinate, staging_coordinate, schema, digest) in [
        (
            SERIES_TEMPLATE_RAW_COORDINATE_V4,
            SERIES_TEMPLATE_STAGING_COORDINATE_V4,
            SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
            request.template().to_bytes(),
        ),
        (
            SERIES_OCCURRENCE_RAW_COORDINATE_V4,
            SERIES_OCCURRENCE_STAGING_COORDINATE_V4,
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            occurrence.to_bytes(),
        ),
        (
            SERIES_TICKET_RAW_COORDINATE_V4,
            SERIES_TICKET_STAGING_COORDINATE_V4,
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            ticket.to_bytes(),
        ),
    ] {
        authenticate_finalized_record(
            registry,
            account(runtime, raw_coordinate)?,
            account(runtime, staging_coordinate)?,
            schema,
            digest,
        )?;
    }
    Ok(())
}

fn authenticate_finalized_record(
    registry: &Pubkey,
    raw: &AccountInfo<'_>,
    staging: &AccountInfo<'_>,
    schema: [u8; 32],
    digest: [u8; 32],
) -> Result<(), SeriesShadowSbfErrorV4> {
    let expected_raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], registry).0;
    let expected_staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], registry).0;
    let data = raw
        .try_borrow_data()
        .map_err(|_| SeriesShadowSbfErrorV4::FinalizedRecord)?;
    if raw.key != &expected_raw
        || raw.owner != registry
        || raw.is_signer
        || raw.is_writable
        || raw.executable
        || hash(&data).to_bytes() != digest
        || !funded_rent_persists_v1(raw.lamports())
        || staging.key != &expected_staging
        || staging.owner != &system_program::ID
        || staging.is_signer
        || staging.is_writable
        || staging.executable
        || staging.data_len() != 0
    {
        return Err(SeriesShadowSbfErrorV4::FinalizedRecord);
    }
    Ok(())
}

fn account<'accounts, 'info>(
    accounts: &'accounts [AccountInfo<'info>],
    coordinate: usize,
) -> Result<&'accounts AccountInfo<'info>, SeriesShadowSbfErrorV4> {
    accounts
        .get(coordinate)
        .ok_or(SeriesShadowSbfErrorV4::Runtime)
}

fn core_content_id(
    bytes: [u8; 32],
) -> Result<dclutch_core_contract::ContentId, SeriesShadowSbfErrorV4> {
    dclutch_core_contract::ContentId::new(bytes)
        .map_err(|_| SeriesShadowSbfErrorV4::FinalizedRecord)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_keys_replace_only_the_authenticated_logical_prefix() {
        let projections = LogicalProjectionKeysV4 {
            config: [2; 32],
            product: [3; 32],
            portfolio: [4; 32],
            linked_basis: [5; 32],
        };
        let physical = Pubkey::new_from_array([1; 32]);
        for (coordinate, expected) in [
            (0_usize, [1_u8; 32]),
            (CONFIG_COORDINATE, [2; 32]),
            (PRODUCT_RAW_COORDINATE, [3; 32]),
            (PORTFOLIO_RAW_COORDINATE, [4; 32]),
            (LINKED_BASIS_RAW_COORDINATE, [5; 32]),
            (5, [1; 32]),
        ] {
            assert_eq!(
                *logical_projection_key(coordinate, &physical, &projections),
                expected
            );
        }
    }

    #[test]
    fn funding_span_is_derived_only_from_exact_logical_width()
    -> Result<(), dclutch_core_contract::Error> {
        let family = [0_u8; SERIES_ACTION_HEADER_BYTES_V3];
        let request = ShadowRequestV3 {
            release_set: content(1)?,
            market: content(2)?,
            root: content(3)?,
            registry_program: content(4)?,
            trading_program: content(5)?,
            accelerator_program: content(6)?,
            artifacts: dclutch_market::execution_strategy::shadow_v3::ShadowArtifactTupleV3 {
                capability_program: content(7)?,
                account_profile: content(8)?,
                request_profile: content(9)?,
                transition: content(10)?,
                effect: content(11)?,
                strategy: content(12)?,
                certificate: content(13)?,
            },
            invocation_context: content(14)?,
            digests: dclutch_market::execution_strategy::shadow_v3::ShadowExecutionDigestsV3 {
                interpreted_candidate: content(15)?,
                interpreted_effect: content(16)?,
                runtime_observations: content(17)?,
                family_request: content(18)?,
            },
            shape: dclutch_market::execution_strategy::shadow_v3::ShadowRuntimeShapeV3 {
                tail_count: 0,
                account_count: 162,
                scalar_count: 5,
                identity_count: 1,
            },
            family_request: &family,
        };
        assert_eq!(funding_count(request), Ok(1));
        let underflow = ShadowRequestV3 {
            shape: dclutch_market::execution_strategy::shadow_v3::ShadowRuntimeShapeV3 {
                account_count: 160,
                ..request.shape
            },
            ..request
        };
        assert_eq!(
            funding_count(underflow),
            Err(SeriesShadowSbfErrorV4::Runtime)
        );
        let zero_funding = ShadowRequestV3 {
            shape: dclutch_market::execution_strategy::shadow_v3::ShadowRuntimeShapeV3 {
                account_count: 161,
                ..request.shape
            },
            ..request
        };
        assert_eq!(
            funding_count(zero_funding),
            Err(SeriesShadowSbfErrorV4::Runtime)
        );
        let too_large = ShadowRequestV3 {
            shape: dclutch_market::execution_strategy::shadow_v3::ShadowRuntimeShapeV3 {
                account_count: 178,
                ..request.shape
            },
            ..request
        };
        assert_eq!(
            funding_count(too_large),
            Err(SeriesShadowSbfErrorV4::Runtime)
        );
        Ok(())
    }

    fn content(byte: u8) -> Result<dclutch_core_contract::ContentId, dclutch_core_contract::Error> {
        dclutch_core_contract::ContentId::new([byte; 32])
    }
}
