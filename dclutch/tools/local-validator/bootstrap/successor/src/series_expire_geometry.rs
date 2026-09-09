//! Canonical finality/prediction geometry for the Series Expire frame.
//!
//! The public input is the semantic child bank and finalized record closure.
//! It never accepts a caller-authored role or width array.

use std::collections::BTreeMap;

use dclutch_core_contract::ContentId;
use dclutch_custody::{
    CustodyFrameRoleV1, CustodyFrameSpecV1, ProjectedCustodyCallerSeedsV1,
    ProjectedCustodyStateSeedsV2,
};
use dclutch_registry::record::{ContentDigest, RecordKeyV1, SchemaReleaseId};
use dclutch_registry::release_set::{CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_trading_sbf::series::{
    expire_funding_artifacts_v5::{
        SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5, SERIES_EXPIRE_ROUTE_ALIASES_V5,
    },
    state::SERIES_TICKET_STATE_BYTES_V3,
};
use solana_program::hash::hash;
use solana_sdk::pubkey::Pubkey;
use solana_sdk_ids::{bpf_loader_upgradeable, native_loader, system_program, sysvar};

use crate::{
    Error, Result,
    rpc::{Rpc, RpcAccount},
    series_consume_geometry::SeriesConsumePrestateV1,
    series_found_prepare_campaign::SeriesFoundPreparePreprofileV1,
    series_found_prepare_driver::{
        SeriesPrepareFinalizedRecordV1, SeriesPrepareHydrationRecordsV1, SeriesPrepareM0FrameV1,
    },
    series_found_prepare_input::SeriesParentRootFactV1,
};

/// Authenticated program/root identities which do not belong to any child
/// request. Every address is validated against a named owner below.
pub(crate) struct SeriesExpireGeometryInputV1<'a> {
    pub(crate) preprofile: &'a SeriesFoundPreparePreprofileV1,
    pub(crate) m0: SeriesPrepareM0FrameV1<'a>,
    pub(crate) records: SeriesPrepareHydrationRecordsV1<'a>,
    pub(crate) parent_root: SeriesParentRootFactV1,
    pub(crate) registry: Pubkey,
    pub(crate) trading: Pubkey,
    pub(crate) custody: Pubkey,
    pub(crate) minimum_slot: u64,
    pub(crate) prestate: SeriesConsumePrestateV1,
}

#[derive(Clone, Debug)]
enum Source<'a> {
    Final {
        role: &'static str,
        address: Pubkey,
        owner: Pubkey,
        body: Option<&'a [u8]>,
    },
    Prepared {
        role: &'static str,
        address: Pubkey,
        owner: Pubkey,
        expected: PreparedState,
    },
    Vacancy {
        role: &'static str,
        address: Pubkey,
        width: u32,
    },
}

impl Source<'_> {
    fn address(&self) -> Pubkey {
        match self {
            Self::Final { address, .. }
            | Self::Prepared { address, .. }
            | Self::Vacancy { address, .. } => *address,
        }
    }
}

/// These expected values come from the native Prepare projection, never an
/// observed width or a permissive fallback from a missing account.
#[derive(Clone, Copy, Debug)]
enum PreparedState {
    Ticket(dclutch_trading::series::replay::TicketStateV3),
    Replay(dclutch_custody::CustodyReplayV1),
    Projected(dclutch_custody::ProjectedCustodyStateV2),
    Token(dclutch_custody::token_svm::TokenAccount),
}

impl PreparedState {
    fn width(self) -> usize {
        match self {
            Self::Ticket(_) => SERIES_TICKET_STATE_BYTES_V3,
            Self::Replay(_) => dclutch_custody::CUSTODY_REPLAY_BYTES_V1,
            Self::Projected(_) => dclutch_custody::PROJECTED_CUSTODY_STATE_BYTES_V2,
            Self::Token(_) => dclutch_custody::token_svm::ACCOUNT_BYTES,
        }
    }

    fn matches(self, bytes: &[u8]) -> bool {
        match self {
            Self::Ticket(expected) => dclutch_trading::series::replay::TicketStateV3::decode(bytes)
                .is_ok_and(|actual| actual == expected),
            Self::Replay(expected) => dclutch_custody::CustodyReplayV1::decode(bytes)
                .is_ok_and(|actual| actual == expected),
            Self::Projected(expected) => dclutch_custody::ProjectedCustodyStateV2::decode(bytes)
                .is_ok_and(|actual| actual == expected),
            Self::Token(expected) => dclutch_custody::token_svm::TokenAccount::parse(bytes)
                .is_ok_and(|actual| actual == expected),
        }
    }
}

fn prepared_source(
    prestate: SeriesConsumePrestateV1,
    role: &'static str,
    address: Pubkey,
    owner: Pubkey,
    expected: PreparedState,
) -> Result<Source<'static>> {
    // The native projected-state encoder persists immutable request facts;
    // decoding reconstructs phase/revision fields rather than retaining the
    // transient Initialize request held by the compiler's projection value.
    let expected = match expected {
        PreparedState::Projected(state) => PreparedState::Projected(
            dclutch_custody::ProjectedCustodyStateV2::decode(&state.encode().map_err(|error| {
                Error::new(format!(
                    "Series Expire projected poststate encoding: {error:?}"
                ))
            })?)
            .map_err(|error| {
                Error::new(format!(
                    "Series Expire projected poststate decoding: {error:?}"
                ))
            })?,
        ),
        other => other,
    };
    match prestate {
        SeriesConsumePrestateV1::PreparedPrediction => vacancy(role, address, expected.width()),
        SeriesConsumePrestateV1::ObservedPrepared => Ok(Source::Prepared {
            role,
            address,
            owner,
            expected,
        }),
    }
}

fn prepared_token(mint: [u8; 32], authority: Pubkey, amount: u64) -> Result<PreparedState> {
    let bytes = dclutch_custody::token_svm::TokenAccount::initialized_base_bytes(
        mint,
        authority.to_bytes(),
    )
    .map_err(|error| {
        Error::new(format!(
            "Series Expire native token initialization: {error:?}"
        ))
    })?;
    let mut expected = dclutch_custody::token_svm::TokenAccount::parse(&bytes)
        .map_err(|error| Error::new(format!("Series Expire native token decode: {error:?}")))?;
    expected.amount = amount;
    Ok(PreparedState::Token(expected))
}

fn final_source<'a>(
    role: &'static str,
    address: Pubkey,
    owner: Pubkey,
    body: Option<&'a [u8]>,
) -> Source<'a> {
    Source::Final {
        role,
        address,
        owner,
        body,
    }
}
fn vacancy(role: &'static str, address: Pubkey, width: usize) -> Result<Source<'static>> {
    Ok(Source::Vacancy {
        role,
        address,
        width: u32::try_from(width)
            .map_err(|_| Error::new("Series Expire fixed width escaped u32"))?,
    })
}

/// Derive, observe, and validate all 82 Expire widths at one finalized slot.
/// Prepare-created state is either a canonical vacancy prediction or a
/// finalized account decoded against its exact native projected poststate.
pub(crate) fn derive_series_expire_fixed_data_lengths_v1(
    rpc: &mut Rpc,
    input: SeriesExpireGeometryInputV1<'_>,
) -> Result<[u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize]> {
    if input.minimum_slot == 0 {
        return Err(Error::new(
            "Series Expire geometry requires a finalized slot",
        ));
    }
    let roles = derive_roles_v1(&input)?;
    observe_roles_v1(rpc, &roles, input.minimum_slot)
}

fn put<'a>(roles: &mut [Option<Source<'a>>], coordinate: usize, source: Source<'a>) -> Result<()> {
    let slot = roles
        .get_mut(coordinate)
        .ok_or_else(|| Error::new("Series Expire coordinate escaped frame"))?;
    if slot.replace(source).is_some() {
        return Err(Error::new(
            "Series Expire semantic mapper assigned a coordinate twice",
        ));
    }
    Ok(())
}

fn derive_roles_v1<'a>(
    input: &'a SeriesExpireGeometryInputV1<'a>,
) -> Result<[Source<'a>; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize]> {
    let mut roles: Vec<Option<Source<'a>>> = (0..SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize)
        .map(|_| None)
        .collect();
    let physical = input.preprofile.physical();
    let children = input.preprofile.prepare_children.expire_requests();
    let refund = dclutch_custody::CustodyRequestV1::decode(children.refund)
        .map_err(|_| Error::new("Series Expire refund child refused decode"))?;
    let close_vault = dclutch_custody::CustodyRequestV1::decode(children.close_vault)
        .map_err(|_| Error::new("Series Expire close-vault child refused decode"))?;
    let close_replay = dclutch_custody::CustodyRequestV1::decode(children.close_replay)
        .map_err(|_| Error::new("Series Expire close-replay child refused decode"))?;
    let abort = dclutch_custody::ProjectedCustodyRequestV1::decode(children.projected_abort)
        .map_err(|_| Error::new("Series Expire projected-abort child refused decode"))?;

    let parent_root = parent_root_source_v1(input)?;
    let parent_root_key = parent_root.address();
    put(&mut roles, 0, parent_root.clone())?;
    put(
        &mut roles,
        1,
        record_source("Template", input.registry, input.records.template)?,
    )?;
    put(&mut roles, 2, m0_source(input, input.m0.project_found[6])?)?;
    put(
        &mut roles,
        3,
        record_source("Portfolio", input.registry, input.records.portfolio)?,
    )?;
    put(&mut roles, 4, m0_source(input, input.m0.project_found[12])?)?;
    let ticket = dclutch_trading::series::admit_ticket(input.records.ticket.body)
        .map_err(|_| Error::new("Series Expire Ticket record refused"))?
        .content_id();
    let ticket_state = Pubkey::find_program_address(
        &[
            dclutch_trading::series::replay::SERIES_TICKET_STATE_PDA_DOMAIN_V3,
            parent_root_key.as_ref(),
            ticket.as_bytes(),
        ],
        &input.trading,
    )
    .0;
    put(
        &mut roles,
        5,
        prepared_source(
            input.prestate,
            "prepared Ticket state",
            ticket_state,
            input.trading,
            PreparedState::Ticket(dclutch_trading::series::replay::TicketStateV3::prepared(
                ticket,
            )),
        )?,
    )?;

    custody_route(
        input,
        &mut roles,
        6,
        refund,
        children.refund,
        physical.normal_replay,
        physical.expire.escrow_vault,
    )?;
    custody_route(
        input,
        &mut roles,
        20,
        close_vault,
        children.close_vault,
        physical.normal_replay,
        physical.expire.escrow_vault,
    )?;
    custody_route(
        input,
        &mut roles,
        34,
        close_replay,
        children.close_replay,
        physical.normal_replay,
        physical.expire.escrow_vault,
    )?;
    projected_abort_route(input, &mut roles, abort, children.projected_abort)?;

    // Core's generated 26-coordinate permit-expiry precommit frame.
    put(
        &mut roles,
        55,
        vacancy(
            "Series founding permit",
            physical.permit,
            dclutch_market::SERIES_FOUNDING_PERMIT_BYTES_V1,
        )?,
    )?;
    put(
        &mut roles,
        56,
        m0_source(input, Pubkey::new_from_array(physical.expire.rent_credit))?,
    )?;
    put(&mut roles, 57, m0_source(input, input.m0.project_found[3])?)?;
    put(
        &mut roles,
        58,
        m0_source(input, input.m0.project_found[29])?,
    )?;
    put(
        &mut roles,
        59,
        m0_source(input, input.m0.project_found[30])?,
    )?;
    put(
        &mut roles,
        60,
        m0_source(input, input.m0.project_found[31])?,
    )?;
    put(
        &mut roles,
        61,
        m0_source(input, input.m0.project_found[27])?,
    )?;
    put(
        &mut roles,
        62,
        m0_source(input, input.m0.project_found[32])?,
    )?;
    put(
        &mut roles,
        63,
        m0_source(input, input.m0.project_found[33])?,
    )?;
    put(
        &mut roles,
        64,
        m0_source(input, input.m0.project_found[34])?,
    )?;
    put(
        &mut roles,
        65,
        m0_source(input, input.m0.project_found[35])?,
    )?;
    put(
        &mut roles,
        66,
        m0_source(input, input.m0.project_found[24])?,
    )?;
    put(
        &mut roles,
        67,
        final_source(
            "Trading program",
            input.trading,
            bpf_loader_upgradeable::ID,
            None,
        ),
    )?;
    put(
        &mut roles,
        68,
        final_source(
            "Trading ProgramData",
            crate::upgrade::target_programdata(input.trading),
            bpf_loader_upgradeable::ID,
            None,
        ),
    )?;
    put(&mut roles, 69, parent_root)?;
    put(
        &mut roles,
        70,
        prepared_source(
            input.prestate,
            "prepared Ticket state",
            ticket_state,
            input.trading,
            PreparedState::Ticket(dclutch_trading::series::replay::TicketStateV3::prepared(
                ticket,
            )),
        )?,
    )?;
    put(
        &mut roles,
        71,
        record_source("Template", input.registry, input.records.template)?,
    )?;
    put(
        &mut roles,
        72,
        vacancy("Template staging", input.records.template.staging, 0)?,
    )?;
    put(
        &mut roles,
        73,
        record_source("occurrence", input.registry, input.records.occurrence)?,
    )?;
    put(
        &mut roles,
        74,
        vacancy("occurrence staging", input.records.occurrence.staging, 0)?,
    )?;
    put(
        &mut roles,
        75,
        record_source("Ticket", input.registry, input.records.ticket)?,
    )?;
    put(
        &mut roles,
        76,
        vacancy("Ticket staging", input.records.ticket.staging, 0)?,
    )?;
    put(
        &mut roles,
        77,
        final_source("Clock sysvar", sysvar::clock::ID, sysvar::ID, None),
    )?;
    put(
        &mut roles,
        78,
        final_source("Rent sysvar", sysvar::rent::ID, sysvar::ID, None),
    )?;
    put(
        &mut roles,
        79,
        final_source(
            "System program",
            system_program::ID,
            native_loader::ID,
            None,
        ),
    )?;
    let permit_bytes = children
        .permit_expiry
        .encode()
        .map_err(|_| Error::new("Series permit-expiry request refused encode"))?;
    let precommit = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(refund.release_set)
                .map_err(|_| Error::new("Series release was zero"))?,
            refund.market,
            ExecutionRoleV1::Trading,
            ticket.to_bytes(),
            hash(&permit_bytes).to_bytes(),
        )
        .map_err(|_| Error::new("Series precommit caller seeds refused"))?
        .as_slices(),
        &input.trading,
    )
    .0;
    put(
        &mut roles,
        80,
        vacancy("Core precommit caller", precommit, 0)?,
    )?;
    put(
        &mut roles,
        81,
        final_source(
            "Custody program",
            input.custody,
            bpf_loader_upgradeable::ID,
            None,
        ),
    )?;

    for &(alias, representative) in SERIES_EXPIRE_ROUTE_ALIASES_V5.iter() {
        let source = roles
            .get(usize::from(representative))
            .and_then(Clone::clone)
            .ok_or_else(|| Error::new("Series Expire generated alias lacked representative"))?;
        let alias_slot = roles
            .get_mut(usize::from(alias))
            .ok_or_else(|| Error::new("Series Expire alias escaped frame"))?;
        if let Some(existing) = alias_slot {
            if existing.address() != source.address() {
                return Err(Error::new(
                    "Series Expire generated alias named a different physical account",
                ));
            }
            continue;
        }
        *alias_slot = Some(source);
    }
    roles
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| Error::new("Series Expire semantic mapper left a coordinate unowned"))?
        .try_into()
        .map_err(|_| Error::new("Series Expire generated frame cardinality drifted"))
}

fn parent_root_source_v1(input: &SeriesExpireGeometryInputV1<'_>) -> Result<Source<'static>> {
    let expected =
        dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5;
    match input.parent_root {
        SeriesParentRootFactV1::Predicted(prediction) => {
            if prediction.data_len != expected {
                return Err(Error::new(
                    "Series Expire predicted root width differed from canonical layout",
                ));
            }
            vacancy(
                "predicted active Series root",
                prediction.root,
                prediction.data_len,
            )
        }
        SeriesParentRootFactV1::Finalized {
            root,
            observed_data_len,
            ..
        } => {
            if observed_data_len != expected {
                return Err(Error::new(
                    "Series Expire finalized root width differed from canonical layout",
                ));
            }
            Ok(final_source(
                "active Series root",
                root,
                input.trading,
                None,
            ))
        }
    }
}

fn custody_route<'a>(
    input: &'a SeriesExpireGeometryInputV1<'a>,
    roles: &mut [Option<Source<'a>>],
    start: usize,
    request: dclutch_custody::CustodyRequestV1,
    bytes: &[u8],
    replay: Pubkey,
    escrow_vault: [u8; 32],
) -> Result<()> {
    let frame = CustodyFrameSpecV1::new(request.operation);
    if Pubkey::new_from_array(request.caller_program) != input.trading {
        return Err(Error::new(
            "Series Expire Custody child selected another Trading program",
        ));
    }
    let caller = Pubkey::find_program_address(
        &CallerAuthoritySeedsV1::new(
            ContentId::new(request.release_set)
                .map_err(|_| Error::new("Series Expire release was zero"))?,
            request.market,
            ExecutionRoleV1::Trading,
            request.context,
            hash(bytes).to_bytes(),
        )
        .map_err(|_| Error::new("Series Expire caller seeds refused"))?
        .as_slices(),
        &input.trading,
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
    for local in 0..usize::from(frame.account_count()) {
        let role = frame
            .account(
                u16::try_from(local)
                    .map_err(|_| Error::new("Series Expire frame index escaped u16"))?,
            )
            .map_err(|_| Error::new("Series Expire Custody frame refused coordinate"))?
            .role();
        let source = match role {
            CustodyFrameRoleV1::CallerAuthority => vacancy("Custody caller", caller, 0)?,
            CustodyFrameRoleV1::CoreMarket => {
                m0_source(input, Pubkey::new_from_array(request.market))?
            }
            CustodyFrameRoleV1::ActivationCache => {
                final_source("activation cache", cache, input.registry, None)
            }
            CustodyFrameRoleV1::RegistryProgram => final_source(
                "Registry program",
                input.registry,
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::CallerProgram => final_source(
                "Trading program",
                input.trading,
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::CallerProgramData => final_source(
                "Trading ProgramData",
                crate::upgrade::target_programdata(input.trading),
                bpf_loader_upgradeable::ID,
                None,
            ),
            CustodyFrameRoleV1::RealmRecord => m0_source(input, input.m0.project_found[4])?,
            CustodyFrameRoleV1::RealmStaging => m0_source(input, input.m0.project_found[5])?,
            CustodyFrameRoleV1::Replay => prepared_source(
                input.prestate,
                "normal Custody replay",
                replay,
                input.custody,
                PreparedState::Replay(input.preprofile.prepared_source_replay),
            )?,
            CustodyFrameRoleV1::Mint => m0_nonrecord(input, Pubkey::new_from_array(request.mint))?,
            CustodyFrameRoleV1::Vault | CustodyFrameRoleV1::TransferSource => prepared_source(
                input.prestate,
                "SeriesEscrow vault",
                Pubkey::new_from_array(escrow_vault),
                Pubkey::new_from_array(request.token_program),
                prepared_token(
                    request.mint,
                    input.preprofile.physical().custody_authority,
                    dclutch_custody::CustodyRequestV1::decode(
                        input.preprofile.prepare_children.expire_requests().refund,
                    )
                    .map_err(|error| {
                        Error::new(format!("Series Expire native refund decode: {error:?}"))
                    })?
                    .amount,
                )?,
            )?,
            CustodyFrameRoleV1::TransferDestination => {
                m0_nonrecord(input, Pubkey::new_from_array(request.destination))?
            }
            CustodyFrameRoleV1::CustodyAuthority => vacancy(
                "Custody authority",
                input.preprofile.physical().custody_authority,
                0,
            )?,
            CustodyFrameRoleV1::TokenProgram => {
                m0_nonrecord(input, Pubkey::new_from_array(request.token_program))?
            }
            CustodyFrameRoleV1::RentRefund => {
                m0_nonrecord(input, Pubkey::new_from_array(request.rent_refund))?
            }
            CustodyFrameRoleV1::Payer
            | CustodyFrameRoleV1::SystemProgram
            | CustodyFrameRoleV1::RentSysvar => {
                return Err(Error::new(
                    "Series Expire Custody route contained a non-Expire role",
                ));
            }
        };
        put(roles, start + local, source)?;
    }
    Ok(())
}

fn projected_abort_route<'a>(
    input: &'a SeriesExpireGeometryInputV1<'a>,
    roles: &mut [Option<Source<'a>>],
    request: dclutch_custody::ProjectedCustodyRequestV1,
    bytes: &[u8],
) -> Result<()> {
    use dclutch_custody::{ProjectedCustodyAbortFrameV1 as Frame, ProjectedCustodyOperationV1};
    if request.operation != ProjectedCustodyOperationV1::AbortOpenAndClose {
        return Err(Error::new(
            "Series Expire projected child was not AbortOpenAndClose",
        ));
    }
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
    let values = [
        vacancy("projected caller", caller, 0)?,
        prepared_source(
            input.prestate,
            "projected state",
            state,
            input.custody,
            PreparedState::Projected(input.preprofile.prepared_projected_state),
        )?,
        final_source("activation cache", cache, input.registry, None),
        final_source(
            "Registry program",
            input.registry,
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source(
            "Trading program",
            input.trading,
            bpf_loader_upgradeable::ID,
            None,
        ),
        final_source(
            "Trading ProgramData",
            crate::upgrade::target_programdata(input.trading),
            bpf_loader_upgradeable::ID,
            None,
        ),
        m0_nonrecord(input, Pubkey::new_from_array(request.rent_credit))?,
        prepared_source(
            input.prestate,
            "projected Hoard vault",
            Pubkey::new_from_array(request.hoard_vault),
            Pubkey::new_from_array(request.token_program),
            prepared_token(
                request.mint,
                input.preprofile.physical().custody_authority,
                0,
            )?,
        )?,
        vacancy(
            "Custody authority",
            input.preprofile.physical().custody_authority,
            0,
        )?,
        m0_nonrecord(input, Pubkey::new_from_array(request.token_program))?,
        m0_source(input, Pubkey::new_from_array(request.market))?,
    ];
    if values.len() != Frame::ACCOUNT_COUNT {
        return Err(Error::new("Series Expire projected Abort frame drifted"));
    }
    for (local, source) in values.into_iter().enumerate() {
        put(roles, 44 + local, source)?;
    }
    Ok(())
}

fn record_source<'a>(
    role: &'static str,
    registry: Pubkey,
    record: SeriesPrepareFinalizedRecordV1<'a>,
) -> Result<Source<'a>> {
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(record.schema)
            .map_err(|_| Error::new("Series Expire record schema was zero"))?,
        ContentDigest::new(hash(record.body).to_bytes())
            .map_err(|_| Error::new("Series Expire record digest was zero"))?,
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
            "Series Expire record raw address was noncanonical",
        ));
    }
    Ok(final_source(role, record.raw, registry, Some(record.body)))
}

fn m0_source<'a>(
    input: &'a SeriesExpireGeometryInputV1<'a>,
    address: Pubkey,
) -> Result<Source<'a>> {
    if address == input.m0.project_found[1] {
        return vacancy("vacant future M0 Market", address, 0);
    }
    if input.m0.vacancies.contains(&address) {
        return vacancy("vacant M0 record", address, 0);
    }
    for record in input.m0.records {
        if address == record.raw {
            return record_source("M0 record", input.registry, *record);
        }
        if address == record.staging {
            record_source("M0 record", input.registry, *record)?;
            return vacancy("M0 record staging", address, 0);
        }
    }
    m0_nonrecord(input, address)
}
fn m0_nonrecord<'a>(
    input: &'a SeriesExpireGeometryInputV1<'a>,
    address: Pubkey,
) -> Result<Source<'a>> {
    let fact = input
        .m0
        .finalized_accounts
        .iter()
        .find(|fact| fact.address == address)
        .ok_or_else(|| {
            Error::new(format!(
                "Series Expire M0 omitted canonical finalized account {address}"
            ))
        })?;
    Ok(final_source(
        "finalized M0 account",
        address,
        fact.expected_owner,
        None,
    ))
}

fn observe_roles_v1(
    rpc: &mut Rpc,
    roles: &[Source<'_>; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
    minimum_slot: u64,
) -> Result<[u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize]> {
    require_alias_addresses_v1(roles)?;
    let mut keys = Vec::new();
    for role in roles {
        let expected_owner = match role {
            Source::Final { owner, .. } | Source::Prepared { owner, .. } => Some(*owner),
            Source::Vacancy { .. } => None,
        };
        crate::series_consume_geometry::require_series_geometry_address_v1(
            role.address(),
            expected_owner,
        )?;
        if !keys.contains(&role.address()) {
            keys.push(role.address());
        }
    }
    let (_, accounts) = rpc.finalized_accounts(&keys, minimum_slot)?;
    let observed = keys.into_iter().zip(accounts).collect::<BTreeMap<_, _>>();
    let mut widths = [0_u32; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize];
    for (coordinate, role) in roles.iter().enumerate() {
        let account = observed
            .get(&role.address())
            .ok_or_else(|| Error::new("Series Expire RPC snapshot omitted role"))?;
        widths[coordinate] = observe_source_width_v1(role, account.as_ref())?;
    }

    for &(alias, representative) in SERIES_EXPIRE_ROUTE_ALIASES_V5.iter() {
        if widths[usize::from(alias)] != widths[usize::from(representative)] {
            return Err(Error::new(
                "Series Expire alias width differs from representative",
            ));
        }
    }
    Ok(widths)
}

fn observe_source_width_v1(source: &Source<'_>, account: Option<&RpcAccount>) -> Result<u32> {
    match source {
        Source::Final {
            role, owner, body, ..
        } => {
            let account = account
                .ok_or_else(|| Error::new(format!("Series Expire finalized {role} was absent")))?;
            if account.owner != *owner || body.is_some_and(|body| account.data != body) {
                return Err(Error::new(format!(
                    "Series Expire finalized {role} owner or bytes differed"
                )));
            }
            u32::try_from(account.data.len())
                .map_err(|_| Error::new("Series Expire account width overflow"))
        }
        Source::Prepared {
            role,
            owner,
            expected,
            ..
        } => {
            let account = account
                .ok_or_else(|| Error::new(format!("Series Expire prepared {role} was absent")))?;
            if account.owner != *owner || account.executable {
                return Err(Error::new(format!(
                    "Series Expire prepared {role} owner or executable bit differed"
                )));
            }
            if !expected.matches(&account.data) {
                return Err(Error::new(format!(
                    "Series Expire prepared {role} native poststate differed"
                )));
            }
            u32::try_from(account.data.len())
                .map_err(|_| Error::new("Series Expire account width overflow"))
        }
        Source::Vacancy { role, width, .. } => {
            if account.is_some() {
                return Err(Error::new(format!(
                    "Series Expire predicted {role} already exists"
                )));
            }
            Ok(*width)
        }
    }
}

fn require_alias_addresses_v1(
    roles: &[Source<'_>; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize],
) -> Result<()> {
    for &(alias, representative) in SERIES_EXPIRE_ROUTE_ALIASES_V5.iter() {
        if roles[usize::from(alias)].address() != roles[usize::from(representative)].address() {
            return Err(Error::new(
                "Series Expire alias named a different physical account",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::series_found_prepare_campaign::derive_series_found_prepare_preprofile_v1;
    use crate::series_found_prepare_campaign::tests::{compiler_input, prepared_founder};
    use crate::series_found_prepare_driver::{
        SeriesPrepareFinalizedAccountV1, SeriesPrepareFinalizedRecordV1,
        series_prepare_m0_frame_from_publication_v1, series_prepare_records_from_m0_publication_v1,
    };
    use crate::series_found_prepare_input::{SeriesParentRootFactV1, SeriesPredictedParentRootV1};
    use dclutch_trading::series::{
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    };
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk_ids::bpf_loader_upgradeable;

    fn finalized(address: Pubkey) -> SeriesPrepareFinalizedAccountV1 {
        SeriesPrepareFinalizedAccountV1 {
            address,
            expected_owner: bpf_loader_upgradeable::ID,
        }
    }

    /// Build the same M0 record closure shape that the publisher hands to
    /// Prepare.  Bodies are the canonical Market compiler output; only the
    /// non-record ProjectFound accounts use deterministic test identities.

    #[test]
    fn derive_roles_accepts_canonical_m0_and_refuses_root_basis_and_alias_substitutions() {
        let prepared = prepared_founder();
        let parent_root = Pubkey::new_from_array([0x55; 32]);
        let mut selection = compiler_input(&prepared, parent_root, &prepared.admitted.tickets()[0]);
        selection.geometry = None;
        let preprofile = derive_series_found_prepare_preprofile_v1(&mut selection)
            .expect("canonical founder child bank");
        let registry = Pubkey::new_from_array(selection.registry_program.to_bytes());
        let (_, market_input, _, _, _) =
            crate::market::tests::selected_family_compiler_fixture_v1();
        let source_spec_body = crate::runtime::decode_hex(&market_input.source_spec_hex)
            .expect("canonical source spec body");
        let source_capacity_body =
            crate::runtime::decode_hex(&market_input.source_capacity_profile_hex)
                .expect("canonical source capacity body");
        let publication = crate::series_found_prepare_campaign::tests::canonical_m0_publication(
            &prepared,
            registry,
            selection.material.market,
            selection.material.payer,
            selection.material.rent_credit,
            selection.material.rent_program,
            selection.material.core,
            Pubkey::find_program_address(
                &[
                    dclutch_registry::ACTIVATION_PDA_DOMAIN_V1,
                    &preprofile
                        .predicted_core
                        .identity
                        .selected_release_set
                        .to_bytes(),
                ],
                &registry,
            )
            .0,
            &source_spec_body,
            &source_capacity_body,
        );
        let m0_records = series_prepare_records_from_m0_publication_v1(&publication);
        let mut finalized_accounts = publication
            .project_found
            .iter()
            .copied()
            .filter(|address| {
                !m0_records
                    .iter()
                    .any(|record| record.raw == *address || record.staging == *address)
                    && !publication.series_prepare_vacancies.contains(address)
                    && *address != publication.project_found[1]
            })
            .map(finalized)
            .collect::<Vec<_>>();
        let physical = preprofile.physical();
        finalized_accounts.extend(
            [
                selection.material.mint,
                selection.material.founder_source,
                selection.material.token_program,
                selection.material.refund_owner,
                selection.material.rent_credit,
                Pubkey::new_from_array(physical.expire.escrow_vault),
                Pubkey::new_from_array(physical.expire.hoard_vault),
                physical.custody_authority,
                selection.material.trading,
                selection.material.custody,
                selection.material.core,
                selection.material.rent_program,
                selection.material.claims,
            ]
            .into_iter()
            .map(finalized),
        );
        let m0 = series_prepare_m0_frame_from_publication_v1(
            &publication,
            &m0_records,
            &finalized_accounts,
        );
        let founder_record = |schema, body| {
            let published = crate::series_found_prepare_campaign::tests::published_record(
                registry, schema, body,
            );
            SeriesPrepareFinalizedRecordV1 {
                schema,
                body,
                raw: published.raw,
                staging: published.staging,
            }
        };
        let portfolio = m0_records
            .iter()
            .copied()
            .find(|record| record.raw == publication.portfolio.raw)
            .expect("M0 portfolio record");
        let records = SeriesPrepareHydrationRecordsV1 {
            template: founder_record(
                SERIES_TEMPLATE_SCHEMA_RELEASE_ID_V3,
                prepared.admitted.template(),
            ),
            occurrence: founder_record(
                SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
                &prepared.admitted.occurrences()[0],
            ),
            ticket: founder_record(
                SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
                &prepared.admitted.tickets()[0],
            ),
            portfolio,
        };
        let root_fact = SeriesParentRootFactV1::Predicted(SeriesPredictedParentRootV1 {
            root: parent_root,
            data_len: dclutch_trading_sbf::series::lifecycle_policy_v5::SERIES_CONSUME_ROOT_ACCOUNT_BYTES_V5,
        });
        // Drive the canonical Prepare role constructor too: native child
        // frames omit their own callee, so outer execution needs one carrier.
        let prepare_input = crate::series_found_prepare_driver::SeriesPrepareHydratorInputV1 {
            registry, core: selection.material.core, trading: selection.material.trading,
            custody: selection.material.custody, rent_program: selection.material.rent_program,
            parent_root,
            parent_root_state: crate::series_found_prepare_driver::SeriesPrepareParentRootStateV1::PreActivationPredicted,
            m0, template: records.template, occurrence: records.occurrence,
            ticket: records.ticket, portfolio: records.portfolio,
            children: &preprofile.prepare_children,
        };
        let prepare_layout =
            crate::series_found_prepare_driver::hydrate_series_prepare_role_layout_v1(
                &prepare_input,
            )
            .expect("canonical Prepare role constructor");
        let prepare_sources = crate::series_geometry::prepare_sources_v1(&prepare_layout);
        let custody_carriers: Vec<_> = prepare_sources
            .iter()
            .filter(|source| match source {
                crate::series_geometry::SeriesPrepareRoleSourceV1::Finalized {
                    address, ..
                } => *address == selection.material.custody,
                _ => false,
            })
            .collect();
        assert_eq!(
            custody_carriers.len(),
            1,
            "Prepare must carry the native Custody executable"
        );

        let input = SeriesExpireGeometryInputV1 {
            preprofile: &preprofile,
            m0,
            records,
            parent_root: root_fact,
            registry,
            trading: selection.material.trading,
            custody: selection.material.custody,
            minimum_slot: 1,
            prestate: SeriesConsumePrestateV1::PreparedPrediction,
        };
        let roles = derive_roles_v1(&input).expect("canonical M0 Expire role constructor");
        assert_eq!(roles.len(), SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize);
        require_alias_addresses_v1(&roles).expect("canonical alias representatives");

        let observed_input = SeriesExpireGeometryInputV1 {
            prestate: SeriesConsumePrestateV1::ObservedPrepared,
            ..input
        };
        let observed_roles = derive_roles_v1(&observed_input).expect("post-Prepare native roles");
        require_alias_addresses_v1(&observed_roles).expect("observed aliases");
        let mut prepared_keys = std::collections::BTreeSet::new();
        for (predicted, observed) in roles.iter().zip(&observed_roles) {
            assert_eq!(predicted.address(), observed.address());
            let Source::Prepared {
                role,
                owner,
                expected,
                ..
            } = observed
            else {
                continue;
            };
            prepared_keys.insert(observed.address());
            let data = match expected {
                PreparedState::Ticket(state) => state.encode().to_vec(),
                PreparedState::Replay(state) => state.to_bytes().expect("native replay").to_vec(),
                PreparedState::Projected(state) => {
                    state.encode().expect("native projected state").to_vec()
                }
                PreparedState::Token(state) => {
                    let initial = dclutch_custody::token_svm::TokenAccount::initialized_base_bytes(
                        state.mint,
                        state.owner,
                    )
                    .expect("native token init");
                    dclutch_custody::token_svm::TokenAccount::project_amount_poststate(
                        &initial,
                        state.amount,
                    )
                    .expect("native token amount")
                    .to_vec()
                }
            };
            let account = RpcAccount {
                owner: *owner,
                executable: false,
                lamports: 1,
                rent_epoch: 0,
                data,
            };
            let predicted_width =
                observe_source_width_v1(predicted, None).expect("vacant prediction");
            assert_eq!(
                observe_source_width_v1(observed, Some(&account)).expect("native prepared account"),
                predicted_width
            );
            assert_eq!(
                observe_source_width_v1(predicted, Some(&account))
                    .expect_err("prediction cannot silently observe")
                    .to_string(),
                format!("Series Expire predicted {role} already exists")
            );
            assert_eq!(
                observe_source_width_v1(observed, None)
                    .expect_err("observation cannot predict absence")
                    .to_string(),
                format!("Series Expire prepared {role} was absent")
            );
            let mut wrong_owner = account.clone();
            wrong_owner.owner = Pubkey::new_unique();
            assert_eq!(
                observe_source_width_v1(observed, Some(&wrong_owner))
                    .expect_err("wrong owner")
                    .to_string(),
                format!("Series Expire prepared {role} owner or executable bit differed")
            );
            let mut wrong_state = account.clone();
            wrong_state.data[0] ^= 1;
            assert_eq!(
                observe_source_width_v1(observed, Some(&wrong_state))
                    .expect_err("wrong native state")
                    .to_string(),
                format!("Series Expire prepared {role} native poststate differed")
            );
            if let PreparedState::Token(state) = expected {
                let mut wrong_amount = account.clone();
                wrong_amount.data =
                    dclutch_custody::token_svm::TokenAccount::project_amount_poststate(
                        &account.data,
                        state.amount.checked_add(1).expect("test amount"),
                    )
                    .expect("different canonical token balance")
                    .to_vec();
                assert_eq!(
                    observe_source_width_v1(observed, Some(&wrong_amount))
                        .expect_err("wrong escrow/hoard amount")
                        .to_string(),
                    format!("Series Expire prepared {role} native poststate differed")
                );
            }
        }
        assert_eq!(
            prepared_keys.len(),
            5,
            "Ticket, replay, escrow, projected state and Hoard"
        );

        let bad_root = SeriesExpireGeometryInputV1 {
            preprofile: input.preprofile,
            m0: input.m0,
            records: input.records,
            parent_root: SeriesParentRootFactV1::Predicted(SeriesPredictedParentRootV1 {
                root: parent_root,
                data_len: 0,
            }),
            registry: input.registry,
            trading: input.trading,
            custody: input.custody,
            minimum_slot: input.minimum_slot,
            prestate: input.prestate,
        };
        assert!(
            derive_roles_v1(&bad_root)
                .expect_err("wrong root width must refuse")
                .to_string()
                .contains("root width")
        );

        let mut bad_m0 = input.m0;
        bad_m0.project_found[12] = Pubkey::new_unique();
        let bad_basis = SeriesExpireGeometryInputV1 {
            preprofile: input.preprofile,
            m0: bad_m0,
            records: input.records,
            parent_root: input.parent_root,
            registry: input.registry,
            trading: input.trading,
            custody: input.custody,
            minimum_slot: input.minimum_slot,
            prestate: input.prestate,
        };
        assert!(
            derive_roles_v1(&bad_basis)
                .expect_err("substituted M0 basis must refuse")
                .to_string()
                .contains("M0")
        );

        let mut bad_aliases = roles;
        let (alias, _) = SERIES_EXPIRE_ROUTE_ALIASES_V5[0];
        bad_aliases[usize::from(alias)] = Source::Vacancy {
            role: "substituted alias",
            address: Pubkey::new_unique(),
            width: 0,
        };
        assert!(
            require_alias_addresses_v1(&bad_aliases)
                .expect_err("substituted alias must refuse")
                .to_string()
                .contains("alias")
        );
    }

    #[test]
    fn generated_aliases_accept_one_canonical_physical_role() {
        let address = Pubkey::new_unique();
        let roles = std::array::from_fn(|_| Source::Vacancy {
            role: "canonical future account",
            address,
            width: 9,
        });
        require_alias_addresses_v1(&roles).expect("canonical representative addresses");
    }

    #[test]
    fn generated_aliases_refuse_substituted_physical_role() {
        let address = Pubkey::new_unique();
        let mut roles = std::array::from_fn(|_| Source::Vacancy {
            role: "canonical future account",
            address,
            width: 9,
        });
        let (alias, _) = SERIES_EXPIRE_ROUTE_ALIASES_V5[0];
        roles[usize::from(alias)] = Source::Vacancy {
            role: "substituted alias",
            address: Pubkey::new_unique(),
            width: 9,
        };
        let error = require_alias_addresses_v1(&roles).expect_err("alias substitution");
        assert!(error.to_string().contains("alias"), "{error}");
    }
}
