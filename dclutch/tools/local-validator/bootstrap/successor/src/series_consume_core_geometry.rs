//! Core-owned routes in the canonical fixed Series Consume frame.
//!
//! This mapper has one job: derive Core Found and Core Open coordinates from
//! the exact child request owned by [`dclutch_operator::SeriesChildBankV1`].
//! It deliberately does not accept account addresses, record bodies, widths,
//! or aliases from its caller.  The Market M0 publication supplies immutable
//! frame material. Post-Prepare state is a finalized observation; accounts
//! created by Core Found or Claims remain named PDA predictions with the
//! layout owned by the program that will create them.

use dclutch_claims::founding_v5::ClaimsFoundingRequestV5;
use dclutch_core_contract::ContentId;
use dclutch_market::{MarketCoreStateSeedsV2, SeriesCoreActionV1, SeriesCoreRequestV1};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading::series::{replay::TicketStateSeedsV3, template_content_id, ticket_content_id};
use dclutch_trading_sbf::series::{
    account_profile_v4::{SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4, SERIES_CONSUME_ROUTE_ALIASES_V4},
    artifacts_v3::{
        SERIES_CONSUME_CORE_FOUND_ACCOUNT_BASE_V3, SERIES_CONSUME_CORE_OPEN_ACCOUNT_COUNT_V3,
    },
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
};
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
const _: () = assert!(CORE_OPEN_WIDTH == 39);

/// Populate Core's two immutable fixed route ranges.  Dynamic FundingState
/// coordinates are intentionally absent: the V4 profile alone owns their
/// ordered insertion between the Found prefix and suffix.
pub(crate) fn populate_series_consume_core_routes_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    children: SeriesConsumeChildRequestsV4<'_>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
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
    permit: Pubkey,
    founder: Pubkey,
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
        let template = template_content_id(input.records.template.body)
            .map_err(|_| Error::new("Series Consume Template identity was not canonical"))?;
        let ticket_context = ticket_content_id(input.records.ticket.body)
            .map_err(|_| Error::new("Series Consume Ticket identity was not canonical"))?;
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
        let physical = input.preprofile.physical();
        if claims.release_set() != request.release_set().to_bytes()
            || claims.market() != market.to_bytes()
            || claims.founder() != founder.to_bytes()
            || claims.aggregate() != physical.claims.aggregate.to_bytes()
            || claims.position() != physical.claims.position.to_bytes()
            || claims.admission() != physical.claims.admission.to_bytes()
            || claims.funding_source() != physical.projected.escrow_vault
            || claims.hoard() != physical.projected.hoard_vault
            || claims.custody_replay() != physical.realized_hoard_replay.to_bytes()
            || claims.rent_credit() != physical.projected.rent_credit
            || claims.rent_program() != input.rent_program.to_bytes()
            || claims.claims_program() != input.claims.to_bytes()
            || claims.trading_program() != input.trading.to_bytes()
        {
            return Err(Error::new(
                "Series Consume Claims request did not join Core future physical owners",
            ));
        }
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
            permit: physical.permit,
            founder,
        })
    }
}

fn populate_found_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    facts: &CoreConsumeFactsV1<'a>,
) -> Result<()> {
    for local in 0..37 {
        let source = match local {
            0 => vacancy_v1("Series Core caller", facts.caller, 0)?,
            1 => vacancy_v1("future M0 Core Market", facts.market, 0)?,
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
    // The fixed Core suffix reuses the four Lock accounts.  Consume has two
    // valid prestate phases: before Prepare those positions are canonical
    // vacancies, while the post-Prepare frame observes final accounts.  The
    // already-populated Lock representatives own that distinction, so copy
    // their complete source form instead of manufacturing a second one here.
    let projected_state = copied_lock_source_v1(roles, 6)?;
    let projected_hoard = copied_lock_source_v1(roles, 12)?;
    let escrow_vault = copied_lock_source_v1(roles, 13)?;
    let custody_replay = copied_lock_source_v1(roles, 17)?;
    let physical = input.preprofile.physical();
    let suffix = [
        vacancy_v1("Series founding permit", facts.permit, 0)?,
        projected_state,
        projected_hoard,
        escrow_vault,
        custody_replay,
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
        final_source_v1(
            "prepaid Claims aggregate",
            physical.claims.aggregate,
            system_program::ID,
            Some(&[]),
        ),
        final_source_v1(
            "prepaid Claims position",
            physical.claims.position,
            system_program::ID,
            Some(&[]),
        ),
        final_source_v1(
            "prepaid Claims admission",
            physical.claims.admission,
            system_program::ID,
            Some(&[]),
        ),
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

fn copied_lock_source_v1<'a>(
    roles: &[Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    coordinate: usize,
) -> Result<SeriesConsumeRoleSourceV1<'a>> {
    roles
        .get(coordinate)
        .and_then(Option::as_ref)
        .cloned()
        .ok_or_else(|| Error::new("Series Consume Core suffix lacked Lock representative"))
}

fn populate_open_v1<'a>(
    input: &'a SeriesConsumeGeometryInputV1<'a>,
    roles: &mut [Option<SeriesConsumeRoleSourceV1<'a>>; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4],
    facts: &CoreConsumeFactsV1<'a>,
) -> Result<()> {
    if CORE_OPEN_WIDTH != 39 {
        return Err(Error::new(
            "Series Consume Core Open ABI was not 39 accounts",
        ));
    }
    let physical = input.preprofile.physical();
    let values = [
        vacancy_v1("Series Core caller", facts.caller, 0)?,
        vacancy_v1("future M0 Core Market", facts.market, 0)?,
        vacancy_v1("Series founding permit", facts.permit, 0)?,
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
            physical.realized_hoard_replay,
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
            "prepaid Claims aggregate",
            physical.claims.aggregate,
            system_program::ID,
            Some(&[]),
        ),
        final_source_v1(
            "prepaid Claims position",
            physical.claims.position,
            system_program::ID,
            Some(&[]),
        ),
        final_source_v1(
            "prepaid Claims admission",
            physical.claims.admission,
            system_program::ID,
            Some(&[]),
        ),
        final_source_v1("Clock sysvar", sysvar::clock::ID, sysvar::ID, None),
        final_source_v1("Rent sysvar", sysvar::rent::ID, sysvar::ID, None),
        copied_lock_source_v1(roles, 123)?,
        copied_lock_source_v1(roles, 124)?,
    ];
    if values.len() != CORE_OPEN_WIDTH {
        return Err(Error::new("Series Consume Core Open source frame drifted"));
    }
    for (offset, source) in values.into_iter().enumerate() {
        let coordinate = SERIES_CONSUME_CORE_OPEN_START_V1 + offset;
        if SERIES_CONSUME_ROUTE_ALIASES_V4
            .iter()
            .any(|(alias, _)| *alias == coordinate)
        {
            // Generated aliases are filled from their native representative
            // after every route has named only canonical coordinates. This
            // prevents a second hand-derived address from diverging from the
            // representative's physical account.
            continue;
        }
        put_series_consume_role_v1(roles, coordinate, source)?;
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
