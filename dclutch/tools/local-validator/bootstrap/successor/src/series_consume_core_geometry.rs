//! Core-owned routes in the canonical fixed Series Consume frame.
//!
//! This mapper has one job: derive Core Found and Core Open coordinates from
//! the exact child request owned by [`dclutch_operator::SeriesChildBankV1`].
//! It deliberately does not accept account addresses, record bodies, widths,
//! or aliases from its caller.  The Market M0 publication supplies immutable
//! frame material. Post-Prepare state is a finalized observation; accounts
//! created by Core Found or Claims remain named PDA predictions with the
//! layout owned by the program that will create them.

use dclutch_claims::{
    founding_v5::ClaimsFoundingRequestV5,
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        liability_basis_vector_width_v2,
    },
    protocol_position_v2::PROTOCOL_POSITION_ADMISSION_BYTES_V2,
};
use dclutch_core_contract::ContentId;
use dclutch_market::{
    MarketCoreStateSeedsV2, SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES, SeriesCoreActionV1,
    SeriesCoreRequestV1,
};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::series::replay::TicketStateSeedsV3;
use dclutch_trading_sbf::series::{
    artifacts_v3::{
        SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3, SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3,
    },
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
};
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use crate::{
    Error, Result,
    series_consume_geometry::{
        SERIES_CONSUME_CORE_FOUND_START_V1, SERIES_CONSUME_CORE_OPEN_START_V1,
        SeriesConsumeGeometryInputV1, SeriesConsumeRoleSourceV1, final_source_v1, m0_source_v1,
        put_series_consume_role_v1, vacancy_v1,
    },
    series_found_prepare_driver::SeriesPrepareFinalizedRecordV1,
    series_found_prepare_input::SeriesParentRootFactV1,
};

const CORE_FOUND_WIDTH: usize = SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3 as usize;
const CORE_OPEN_WIDTH: usize = SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3 as usize;
const CORE_FOUND_PREFIX_WIDTH: usize = 48;
const CORE_FOUND_SUFFIX_WIDTH: usize = 13;

const _: () = assert!(CORE_FOUND_WIDTH == CORE_FOUND_PREFIX_WIDTH + CORE_FOUND_SUFFIX_WIDTH);
const _: () = assert!(CORE_OPEN_WIDTH == 37);

/// Populate Core's two immutable fixed route ranges.  Dynamic FundingState
/// coordinates are intentionally absent: the V4 profile alone owns their
/// ordered insertion between the Found prefix and suffix.
pub(crate) fn populate_series_consume_core_routes_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    children: SeriesConsumeChildRequestsV4<'_>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; 161],
) -> Result<()> {
    let request = SeriesCoreRequestV1::decode(children.core)
        .map_err(|_| Error::new("Series Consume Core request refused decode"))?;
    let claims = ClaimsFoundingRequestV5::decode(children.claims)
        .map_err(|_| Error::new("Series Consume Claims request refused decode"))?;
    let facts = CoreConsumeFactsV1::derive(input, request, claims, children.core)?;
    populate_found_v1(input, roles, &facts)?;
    populate_open_v1(input, roles, &facts)
}

#[derive(Clone)]
struct CoreConsumeFactsV1<'a> {
    caller: Pubkey,
    market: Pubkey,
    root: SeriesConsumeRoleSourceV1<'a>,
    ticket_state: Pubkey,
    activation_cache: Pubkey,
    projected_replay: Pubkey,
    permit: Pubkey,
    founder: Pubkey,
    aggregate_width: usize,
    position_width: usize,
}

impl<'a> CoreConsumeFactsV1<'a> {
    fn derive(
        input: &'a SeriesConsumeGeometryInputV1<'a>,
        request: SeriesCoreRequestV1,
        claims: ClaimsFoundingRequestV5,
        request_bytes: &[u8],
    ) -> Result<Self> {
        if request.action() != SeriesCoreActionV1::Consume {
            return Err(Error::new(
                "Series Consume Core child named a non-Consume action",
            ));
        }
        let market = Pubkey::new_from_array(
            request
                .market()
                .ok_or_else(|| Error::new("Series Consume Core request omitted Market"))?
                .to_bytes(),
        );
        let founder = Pubkey::new_from_array(
            request
                .founder()
                .ok_or_else(|| Error::new("Series Consume Core request omitted founder"))?
                .to_bytes(),
        );
        let template = ContentId::new(Sha256::digest(input.records.template.body).into())
            .map_err(|_| Error::new("Series Consume Template identity was zero"))?;
        let ticket_context = ContentId::new(Sha256::digest(input.records.ticket.body).into())
            .map_err(|_| Error::new("Series Consume Ticket identity was zero"))?;
        if request.template().to_bytes() != template.to_bytes()
            || market != input.m0.project_found[1]
            || request.release_set().to_bytes()
                != input
                    .preprofile
                    .predicted_core
                    .identity
                    .selected_release_set
                    .to_bytes()
        {
            return Err(Error::new(
                "Series Consume Core request did not join the canonical M0 market",
            ));
        }
        let expected_market = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(input.preprofile.predicted_core.identity).as_slices(),
            &input.core,
        )
        .0;
        if market != expected_market {
            return Err(Error::new(
                "Series Consume Core Market PDA differed from its immutable M0 identity",
            ));
        }
        let root = parent_root_source_v1(input)?;
        let ticket_state = Pubkey::find_program_address(
            &TicketStateSeedsV3::new(root.address().to_bytes(), ticket_context).as_slices(),
            &input.trading,
        )
        .0;
        if request.ticket().map(|ticket| ticket.to_bytes()) != Some(ticket_state.to_bytes()) {
            return Err(Error::new(
                "Series Consume Core request ticket was not the canonical M1 replay PDA",
            ));
        }
        let caller = Pubkey::find_program_address(
            &CallerAuthoritySeedsV1::from_bytes(
                request.release_set().to_bytes(),
                market.to_bytes(),
                ExecutionRoleV1::Trading,
                ticket_context.to_bytes(),
                solana_program::hash::hash(request_bytes).to_bytes(),
            )
            .map_err(|_| Error::new("Series Consume Core caller seeds refused"))?
            .as_slices(),
            &input.trading,
        )
        .0;
        let lock = dclutch_custody::ProjectedCustodyRequestV1::decode(
            input.preprofile.prepare_children.consume_requests().lock,
        )
        .map_err(|_| Error::new("Series Consume projected Lock request refused decode"))?;
        if lock.market != market.to_bytes()
            || lock.caller_program != input.trading.to_bytes()
            || lock.core_program != input.core.to_bytes()
        {
            return Err(Error::new(
                "Series Consume projected Lock request did not join Core and Trading",
            ));
        }
        let projected_replay = Pubkey::find_program_address(
            &dclutch_custody::ProjectedCustodyStateSeedsV2::from_request(lock).as_slices(),
            &input.custody,
        )
        .0;
        let physical = input.preprofile.physical();
        if claims.release_set() != request.release_set().to_bytes()
            || claims.market() != market.to_bytes()
            || claims.founder() != founder.to_bytes()
            || claims.aggregate() != physical.claims.aggregate.to_bytes()
            || claims.position() != physical.claims.position.to_bytes()
            || claims.admission() != physical.claims.admission.to_bytes()
            || claims.funding_source() != physical.projected.escrow_vault
            || claims.hoard() != physical.projected.hoard_vault
            || claims.custody_replay() != physical.normal_replay.to_bytes()
            || claims.rent_credit() != physical.projected.rent_credit
            || claims.rent_program() != input.rent_program.to_bytes()
            || claims.claims_program() != input.claims.to_bytes()
            || claims.trading_program() != input.trading.to_bytes()
        {
            return Err(Error::new(
                "Series Consume Claims request did not join Core future physical owners",
            ));
        }
        let aggregate_width = liability_basis_vector_width_v2(
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2,
            claims.claim_count(),
        )
        .map_err(|_| Error::new("Series Consume Claims aggregate width refused"))?;
        let position_width = liability_basis_vector_width_v2(
            LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            claims.claim_count(),
        )
        .map_err(|_| Error::new("Series Consume Claims position width refused"))?;
        let activation_cache = Pubkey::find_program_address(
            &[
                dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
                &request.release_set().to_bytes(),
            ],
            &input.registry,
        )
        .0;
        Ok(Self {
            caller,
            market,
            root,
            ticket_state,
            activation_cache,
            projected_replay,
            permit: physical.permit,
            founder,
            aggregate_width,
            position_width,
        })
    }
}

fn populate_found_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; 161],
    facts: &CoreConsumeFactsV1<'a>,
) -> Result<()> {
    for local in 0..37 {
        let source = match local {
            0 => vacancy_v1("Series Core caller", facts.caller, 0)?,
            1 => vacancy_v1("future M0 Core Market", facts.market, STATE_BYTES)?,
            28 => final_source_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None),
            _ => {
                let m0_coordinate = if local < 28 { local } else { local - 1 };
                m0_source_v1(input, input.m0.project_found[m0_coordinate])?
            }
        };
        put_series_consume_role_v1(roles, SERIES_CONSUME_CORE_FOUND_START_V1 + local, source)?;
    }
    let prefix = [
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
        facts.root.clone(),
        final_source_v1(
            "prepared Ticket state",
            facts.ticket_state,
            input.trading,
            None,
        ),
        record_source_v1("Series Template", input.registry, input.records.template)?,
        vacancy_v1("Series Template staging", input.records.template.staging, 0)?,
        record_source_v1(
            "Series occurrence",
            input.registry,
            input.records.occurrence,
        )?,
        vacancy_v1(
            "Series occurrence staging",
            input.records.occurrence.staging,
            0,
        )?,
        record_source_v1("Series Ticket", input.registry, input.records.ticket)?,
        vacancy_v1("Series Ticket staging", input.records.ticket.staging, 0)?,
        final_source_v1("Clock sysvar", sysvar::clock::ID, sysvar::ID, None),
    ];
    for (offset, source) in prefix.into_iter().enumerate() {
        put_series_consume_role_v1(
            roles,
            SERIES_CONSUME_CORE_FOUND_START_V1 + 37 + offset,
            source,
        )?;
    }
    let physical = input.preprofile.physical();
    let suffix = [
        vacancy_v1(
            "Series founding permit",
            facts.permit,
            SERIES_FOUNDING_PERMIT_BYTES_V1,
        )?,
        final_source_v1(
            "projected Custody state",
            facts.projected_replay,
            input.custody,
            None,
        ),
        final_source_v1(
            "projected Hoard vault",
            Pubkey::new_from_array(physical.projected.hoard_vault),
            Pubkey::new_from_array(physical.projected.token_program),
            None,
        ),
        final_source_v1(
            "SeriesEscrow funding vault",
            Pubkey::new_from_array(physical.projected.escrow_vault),
            Pubkey::new_from_array(physical.projected.token_program),
            None,
        ),
        final_source_v1(
            "normal Custody replay",
            physical.normal_replay,
            input.custody,
            None,
        ),
        final_source_v1(
            "Claims program",
            input.claims,
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source_v1(
            "Claims ProgramData",
            crate::upgrade::target_programdata(input.claims),
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source_v1(
            "Custody program",
            input.custody,
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source_v1(
            "Custody ProgramData",
            crate::upgrade::target_programdata(input.custody),
            bpf_loader_upgradeable::ID,
            None,
        ),
        vacancy_v1(
            "Claims aggregate",
            physical.claims.aggregate,
            facts.aggregate_width,
        )?,
        vacancy_v1(
            "Claims position",
            physical.claims.position,
            facts.position_width,
        )?,
        vacancy_v1(
            "Claims admission",
            physical.claims.admission,
            PROTOCOL_POSITION_ADMISSION_BYTES_V2,
        )?,
        final_source_v1("Series founder", facts.founder, system_program::ID, None),
    ];
    for (offset, source) in suffix.into_iter().enumerate() {
        put_series_consume_role_v1(
            roles,
            SERIES_CONSUME_CORE_FOUND_START_V1 + CORE_FOUND_PREFIX_WIDTH + offset,
            source,
        )?;
    }
    Ok(())
}

fn populate_open_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; 161],
    facts: &CoreConsumeFactsV1<'a>,
) -> Result<()> {
    // The source program currently contains a newer 39-account implementation,
    // but this selected V4 artifact pins the 37-account route.  Do not silently
    // widen the source-owned ABI from the host-side mapper.
    if CORE_OPEN_WIDTH != 37 {
        return Err(Error::new(
            "Series Consume selected Core Open ABI was not 37 accounts",
        ));
    }
    let physical = input.preprofile.physical();
    let values = [
        vacancy_v1("Series Core caller", facts.caller, 0)?,
        vacancy_v1("future M0 Core Market", facts.market, STATE_BYTES)?,
        vacancy_v1(
            "Series founding permit",
            facts.permit,
            SERIES_FOUNDING_PERMIT_BYTES_V1,
        )?,
        m0_source_v1(
            input,
            Pubkey::new_from_array(physical.projected.rent_credit),
        )?,
        m0_source_v1(input, input.m0.project_found[3])?,
        final_source_v1(
            "activation cache",
            facts.activation_cache,
            input.registry,
            None,
        ),
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
        final_source_v1(
            "Claims program",
            input.claims,
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source_v1(
            "Claims ProgramData",
            crate::upgrade::target_programdata(input.claims),
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source_v1(
            "Custody program",
            input.custody,
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source_v1(
            "Custody ProgramData",
            crate::upgrade::target_programdata(input.custody),
            bpf_loader_upgradeable::ID,
            None,
        ),
        m0_source_v1(input, input.m0.project_found[25])?,
        m0_source_v1(input, input.m0.project_found[26])?,
        facts.root.clone(),
        final_source_v1(
            "prepared Ticket state",
            facts.ticket_state,
            input.trading,
            None,
        ),
        record_source_v1("Series Template", input.registry, input.records.template)?,
        vacancy_v1("Series Template staging", input.records.template.staging, 0)?,
        record_source_v1(
            "Series occurrence",
            input.registry,
            input.records.occurrence,
        )?,
        vacancy_v1(
            "Series occurrence staging",
            input.records.occurrence.staging,
            0,
        )?,
        record_source_v1("Series Ticket", input.registry, input.records.ticket)?,
        vacancy_v1("Series Ticket staging", input.records.ticket.staging, 0)?,
        m0_source_v1(input, input.m0.project_found[6])?,
        m0_source_v1(input, input.m0.project_found[7])?,
        m0_source_v1(input, input.m0.project_found[8])?,
        m0_source_v1(input, input.m0.project_found[9])?,
        m0_source_v1(input, input.m0.project_found[10])?,
        m0_source_v1(input, input.m0.project_found[11])?,
        final_source_v1(
            "normal Custody replay",
            physical.normal_replay,
            input.custody,
            None,
        ),
        final_source_v1(
            "projected Hoard vault",
            Pubkey::new_from_array(physical.projected.hoard_vault),
            Pubkey::new_from_array(physical.projected.token_program),
            None,
        ),
        final_source_v1(
            "SeriesEscrow funding vault",
            Pubkey::new_from_array(physical.projected.escrow_vault),
            Pubkey::new_from_array(physical.projected.token_program),
            None,
        ),
        vacancy_v1(
            "Claims aggregate",
            physical.claims.aggregate,
            facts.aggregate_width,
        )?,
        vacancy_v1(
            "Claims position",
            physical.claims.position,
            facts.position_width,
        )?,
        vacancy_v1(
            "Claims admission",
            physical.claims.admission,
            PROTOCOL_POSITION_ADMISSION_BYTES_V2,
        )?,
        final_source_v1("Clock sysvar", sysvar::clock::ID, sysvar::ID, None),
        final_source_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None),
    ];
    if values.len() != CORE_OPEN_WIDTH {
        return Err(Error::new("Series Consume Core Open source frame drifted"));
    }
    for (offset, source) in values.into_iter().enumerate() {
        put_series_consume_role_v1(roles, SERIES_CONSUME_CORE_OPEN_START_V1 + offset, source)?;
    }
    Ok(())
}

fn parent_root_source_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    let expected =
        dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5;
    match input.parent_root {
        SeriesParentRootFactV1::Predicted(prediction) if prediction.data_len == expected => {
            vacancy_v1(
                "predicted active Series root",
                prediction.root,
                prediction.data_len,
            )
        }
        SeriesParentRootFactV1::Finalized {
            root,
            observed_data_len,
            ..
        } if observed_data_len == expected => Ok(final_source_v1(
            "active Series root",
            root,
            input.trading,
            None,
        )),
        _ => Err(Error::new(
            "Series Consume parent root width differed from its release-owned layout",
        )),
    }
}

fn record_source_v1<'a>(
    role: &'static str,
    registry: Pubkey,
    record: SeriesPrepareFinalizedRecordV1<'a>,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    use dclutch_registry::record::{ContentDigest, RecordKeyV1, SchemaReleaseId};

    let key = RecordKeyV1::new(
        SchemaReleaseId::new(record.schema)
            .map_err(|_| Error::new("Series Consume record schema was zero"))?,
        ContentDigest::new(solana_program::hash::hash(record.body).to_bytes())
            .map_err(|_| Error::new("Series Consume record digest was zero"))?,
    );
    let seeds = key.raw_record_pda_seeds();
    if Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        &registry,
    )
    .0 != record.raw
    {
        return Err(Error::new(
            "Series Consume record raw address was noncanonical",
        ));
    }
    Ok(final_source_v1(
        role,
        record.raw,
        registry,
        Some(record.body),
    ))
}
