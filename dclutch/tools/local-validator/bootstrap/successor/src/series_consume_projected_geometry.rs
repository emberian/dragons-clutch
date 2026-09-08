//! Canonical projected-Custody branches of the Series Consume frame.
//!
//! This leaf does not retain a Consume request copy or accept caller-built
//! account vectors. It decodes the two Custody-owned child packets, derives
//! every Custody PDA from their typed seed owners, and leaves the common mapper
//! to observe the resulting finalized facts or canonical vacancies.

use dclutch_custody::{
    CUSTODY_REPLAY_BYTES_V1, CompartmentV1, CustodyAuthoritySeedsV1, CustodyVaultSeedsV1,
    PROJECTED_CUSTODY_STATE_BYTES_V2, ProjectedCallerRoleV1, ProjectedCustodyCallerSeedsV1,
    ProjectedCustodyOperationV1, ProjectedCustodyRequestV1, ProjectedCustodySourceReplaySeedsV1,
    ProjectedCustodyStateSeedsV2,
};
use dclutch_trading_sbf::series::{
    account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
    artifacts_v3::{SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3, SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3},
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
};
use solana_program::hash::hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::bpf_loader_upgradeable;

use crate::{
    Error, Result,
    series_consume_geometry::{
        SERIES_CONSUME_REALIZE_START_V1, SeriesConsumeGeometryInputV1, SeriesConsumePrestateV1,
        SeriesConsumeRoleSourceV1, final_source_v1, m0_nonrecord_v1, m0_source_v1,
        prepared_prediction_v1, put_series_consume_role_v1, vacancy_v1,
    },
    series_found_prepare_input::SeriesParentRootFactV1,
};

const LOCK_START: usize = 5;
const _: () = assert!(LOCK_START == 5);
const _: () = assert!(SERIES_CONSUME_REALIZE_START_V1 == 80);

/// Add the exact projected-Custody Lock and Realize branches to the global
/// fixed-coordinate Consume role map.
///
/// The two child packets originate solely in `SeriesChildBankV1`. This mapper
/// only decodes them and projects physical addresses from their Custody-owned
/// seed types; neither a ProgramData account nor a token account is assigned a
/// caller-selected width or owner.
pub(crate) fn populate_series_consume_projected_routes_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    children: SeriesConsumeChildRequestsV4<'a>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    let lock = ProjectedCustodyRequestV1::decode(children.lock)
        .map_err(|_| Error::new("Series Consume projected Lock child refused decode"))?;
    let realize = ProjectedCustodyRequestV1::decode(children.realize)
        .map_err(|_| Error::new("Series Consume projected Realize child refused decode"))?;
    require_projected_pair_v1(input, lock, realize)?;
    populate_lock_v1(input, lock, children.lock, roles)?;
    populate_realize_v1(input, realize, children.realize, roles)
}

fn require_projected_pair_v1(
    input: &SeriesConsumeGeometryInputV1<'_>,
    lock: ProjectedCustodyRequestV1,
    realize: ProjectedCustodyRequestV1,
) -> Result<()> {
    let physical = input.preprofile.projected_custody();
    let market = input.m0.project_found[1].to_bytes();
    let parent_root = match input.parent_root {
        SeriesParentRootFactV1::Predicted(prediction) => prediction.root,
        SeriesParentRootFactV1::Finalized { root, .. } => root,
    };
    if lock.operation != ProjectedCustodyOperationV1::LockHoardAndCloseSource
        || realize.operation != ProjectedCustodyOperationV1::RealizeAndClose
        || lock.caller_role != ProjectedCallerRoleV1::TradingCapability
        || realize.caller_role != ProjectedCallerRoleV1::TradingCapability
        || lock.market != market
        || realize.market != market
        || lock.parent_capability_root != physical.parent_capability_root
        || realize.parent_capability_root != physical.parent_capability_root
        || lock.parent_capability_root != parent_root.to_bytes()
        || lock.caller_program != input.trading.to_bytes()
        || realize.caller_program != input.trading.to_bytes()
        || lock.core_program != input.core.to_bytes()
        || realize.core_program != input.core.to_bytes()
        || lock.rent_program != physical.rent_program
        || realize.rent_program != physical.rent_program
        || lock.hoard_vault != physical.hoard_vault
        || realize.hoard_vault != physical.hoard_vault
        || lock.funding_source_vault != physical.escrow_vault
        || realize.funding_source_vault != physical.escrow_vault
        || lock.funding_source_compartment != CompartmentV1::SeriesEscrow
        || realize.funding_source_compartment != CompartmentV1::SeriesEscrow
        || lock.mint != physical.mint
        || realize.mint != physical.mint
        || lock.token_program != physical.token_program
        || realize.token_program != physical.token_program
    {
        return Err(Error::new(
            "Series Consume projected children did not join canonical Series physical facts",
        ));
    }
    let mut expected_realize = lock;
    expected_realize.operation = ProjectedCustodyOperationV1::RealizeAndClose;
    expected_realize.expected_revision = lock.resulting_revision;
    expected_realize.resulting_revision = lock
        .resulting_revision
        .checked_add(1)
        .ok_or_else(|| Error::new("Series Consume projected revision overflow"))?;
    if expected_realize != realize {
        return Err(Error::new(
            "Series Consume projected Realize was not Lock's canonical successor",
        ));
    }
    Ok(())
}

fn projected_common_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    request: ProjectedCustodyRequestV1,
    bytes: &[u8],
) -> Result<[SeriesConsumeRoleSourceV1<'a>; 7]> {
    let caller = Pubkey::find_program_address(
        &ProjectedCustodyCallerSeedsV1::new(request, hash(bytes).to_bytes()).as_slices(),
        &input.trading,
    )
    .0;
    let state = Pubkey::find_program_address(
        &ProjectedCustodyStateSeedsV2::from_request(request).as_slices(),
        &input.custody,
    )
    .0;
    let cache = Pubkey::find_program_address(
        &[
            dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
            &request.release_set,
        ],
        &input.registry,
    )
    .0;
    Ok([
        vacancy_v1("projected Custody caller", caller, 0)?,
        projected_state_source_v1(input, state)?,
        final_source_v1("activation cache", cache, input.registry, None),
        final_source_v1(
            "Registry program",
            input.registry,
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source_v1(
            "Trading program",
            input.trading,
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source_v1(
            "Trading ProgramData",
            crate::upgrade::target_programdata(input.trading),
            bpf_loader_upgradeable::ID,
            None,
        ),
        m0_nonrecord_v1(input, Pubkey::new_from_array(request.rent_credit))?,
    ])
}

fn populate_lock_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    request: ProjectedCustodyRequestV1,
    bytes: &[u8],
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    let mut values = Vec::from(projected_common_v1(input, request, bytes)?);
    let hoard = canonical_vault_v1(
        input,
        request,
        request.hoard_vault,
        request.context_digest,
        CompartmentV1::HoardPrincipal,
        "prepared projected Hoard vault",
    )?;
    let source = canonical_vault_v1(
        input,
        request,
        request.funding_source_vault,
        request.funding_source_context,
        request.funding_source_compartment,
        "prepared SeriesEscrow vault",
    )?;
    let authority = custody_authority_v1(input, request)?;
    let source_replay = Pubkey::find_program_address(
        &ProjectedCustodySourceReplaySeedsV1::from_request(request).as_slices(),
        &input.custody,
    )
    .0;
    if source_replay != input.preprofile.physical().normal_replay {
        return Err(Error::new(
            "Series Consume projected Lock source replay differed from canonical SeriesEscrow replay",
        ));
    }
    values.extend([
        hoard,
        source,
        authority,
        m0_nonrecord_v1(input, Pubkey::new_from_array(request.mint))?,
        m0_nonrecord_v1(input, Pubkey::new_from_array(request.token_program))?,
        source_replay_source_v1(input, source_replay)?,
        m0_source_v1(input, Pubkey::new_from_array(request.market))?,
    ]);
    put_route_v1(roles, LOCK_START, values, "Lock")
}

fn populate_realize_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    request: ProjectedCustodyRequestV1,
    bytes: &[u8],
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
) -> Result<()> {
    let mut values = Vec::from(projected_common_v1(input, request, bytes)?);
    values.extend([
        canonical_vault_v1(
            input,
            request,
            request.hoard_vault,
            request.context_digest,
            CompartmentV1::HoardPrincipal,
            "prepared projected Hoard vault",
        )?,
        m0_source_v1(input, Pubkey::new_from_array(request.market))?,
        custody_authority_v1(input, request)?,
        m0_nonrecord_v1(input, Pubkey::new_from_array(request.mint))?,
        m0_nonrecord_v1(input, Pubkey::new_from_array(request.token_program))?,
    ]);
    put_route_v1(roles, SERIES_CONSUME_REALIZE_START_V1, values, "Realize")
}

fn canonical_vault_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    request: ProjectedCustodyRequestV1,
    supplied: [u8; 32],
    context: [u8; 32],
    compartment: CompartmentV1,
    role: &'static str,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    let address = Pubkey::new_from_array(supplied);
    let expected = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(request.market, request.release_set, context, compartment)
            .as_slices(),
        &input.custody,
    )
    .0;
    if address != expected {
        return Err(Error::new(
            "Series Consume projected Custody vault differed from canonical seeds",
        ));
    }
    match input.prestate {
        SeriesConsumePrestateV1::PreparedPrediction => {
            prepared_prediction_v1(role, address, dclutch_custody::token_svm::ACCOUNT_BYTES)
        }
        SeriesConsumePrestateV1::ObservedPrepared => Ok(final_source_v1(
            role,
            address,
            Pubkey::new_from_array(request.token_program),
            None,
        )),
    }
}

fn projected_state_source_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    state: Pubkey,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    match input.prestate {
        SeriesConsumePrestateV1::PreparedPrediction => prepared_prediction_v1(
            "prepared projected Custody state",
            state,
            PROJECTED_CUSTODY_STATE_BYTES_V2,
        ),
        SeriesConsumePrestateV1::ObservedPrepared => Ok(final_source_v1(
            "prepared projected Custody state",
            state,
            input.custody,
            None,
        )),
    }
}

fn source_replay_source_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    replay: Pubkey,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    match input.prestate {
        SeriesConsumePrestateV1::PreparedPrediction => prepared_prediction_v1(
            "prepared SeriesEscrow replay",
            replay,
            CUSTODY_REPLAY_BYTES_V1,
        ),
        SeriesConsumePrestateV1::ObservedPrepared => Ok(final_source_v1(
            "prepared SeriesEscrow replay",
            replay,
            input.custody,
            None,
        )),
    }
}

fn custody_authority_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    request: ProjectedCustodyRequestV1,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(request.market, request.release_set).as_slices(),
        &input.custody,
    )
    .0;
    if authority != input.preprofile.physical().custody_authority {
        return Err(Error::new(
            "Series Consume projected Custody authority differed from canonical physical fact",
        ));
    }
    vacancy_v1("Custody authority", authority, 0)
}

fn put_route_v1<'a>(
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    start: usize,
    values: Vec<SeriesConsumeRoleSourceV1<'a>>,
    route: &str,
) -> Result<()> {
    let expected = match start {
        LOCK_START => SERIES_CONSUME_LOCK_ACCOUNT_COUNT_V3 as usize,
        SERIES_CONSUME_REALIZE_START_V1 => SERIES_CONSUME_REALIZE_ACCOUNT_COUNT_V3 as usize,
        _ => {
            return Err(Error::new(
                "Series Consume projected route start was unknown",
            ));
        }
    };
    if values.len() != expected {
        return Err(Error::new(format!(
            "Series Consume projected {route} route cardinality drifted"
        )));
    }
    for (local, source) in values.into_iter().enumerate() {
        put_series_consume_role_v1(roles, start + local, source)?;
    }
    Ok(())
}
