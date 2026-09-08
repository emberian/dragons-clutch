//! Seed immutable Series programs from authenticated occurrence-local facts.
//!
//! This adapter reads only the logical observations already admitted by the
//! selected AccountProfile, the authenticated Hot frame, and native owners.
//! No request buffer, receipt digest, or principal cap is accepted from a caller.

use super::{
    frame::{AuthenticatedChildProgramsV3, HotFrameV3},
    series_source::{
        authenticate_series_record_observations_v1, derive_series_prepare_principal_cap_v1,
    },
};
use crate::{
    TradingSbfError,
    series::{
        consume_artifacts_v4 as consume,
        custody_v3::SeriesCustodyPhysicalV3,
        derived_prepare_v1::{
            SeriesPrepareDerivedRequestInputV1, derive_prefounding_escrow,
            derive_series_prepare_requests_v1,
        },
        derived_terminal_v1::{
            SeriesConsumeDerivedRequestInputV1, SeriesExpireDerivedRequestInputV1,
            derive_series_consume_requests_v1, derive_series_expire_requests_v1,
        },
        expire_funding_artifacts_v5 as expire,
        founding_children_v1::{
            SeriesFoundingChildrenInputV1, SeriesFoundingClaimsPhysicalV1,
            SeriesProjectedCustodyPhysicalInputV1, project_found_core_state_v1,
        },
        operator::SeriesOccurrenceSnapshotV3,
        prepare_funding_artifacts_v5 as prepare,
        projected_custody_v3::SeriesProjectedCustodyPhysicalV3,
    },
};
extern crate alloc;

use alloc::{boxed::Box, vec::Vec};
use dclutch_custody::{
    CallerRoleV1, CompartmentV1, CustodyReplaySeedsV1, CustodyReplayV1, CustodyVaultSeedsV1,
    PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCustodyStateSeedsV2, ProjectedCustodyStateV2,
};
use dclutch_market::{
    CoreState, Identity, MarketCoreStateSeedsV2, ProductGraphBumpsV1, StateBumpsV1,
    capability_program::{CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1},
    realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1},
};
use dclutch_product::svm_reader::AuthenticatedProductRuntimeV3;
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, PrefoundingSeriesEscrowV3,
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_SUCCESSOR_KIND_PREIMAGE_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, TemplateV3, admit_occurrence, admit_ticket,
    replay::{SeriesStateV3, TicketStateSeedsV3, TicketStateV3},
    request::{SeriesActionRequestV3, SeriesActionV3},
};
use dclutch_vm::account_profile::AccountObservationV1;
use solana_program::{
    clock::Clock,
    hash::{hash, hashv},
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};
use solana_sdk_ids::system_program;

type Result<T> = core::result::Result<T, ProgramError>;

/// Authenticate records and native replay before writing any private bank words.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
pub(super) fn seed_authenticated_series_derived_scalars_v1(
    selected_kind: [u8; 32],
    action: u32,
    family_request: &[u8],
    program_id: &Pubkey,
    frame: &HotFrameV3<'_, '_>,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    rent: &Rent,
    child_programs: Option<AuthenticatedChildProgramsV3>,
    observations: &[AccountObservationV1<'_>],
    scalars: &mut [u64],
) -> Result<bool> {
    let now_slot = if selected_kind == hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes()
        && action <= SeriesActionV3::Expire as u32
    {
        Clock::get()?.slot
    } else {
        0
    };
    seed_authenticated_series_derived_scalars_at_slot_v1(
        selected_kind,
        action,
        family_request,
        program_id,
        frame,
        product,
        rent,
        child_programs,
        observations,
        scalars,
        now_slot,
    )
}

// The runtime wrapper supplies Clock; child tests exercise this same body at a
// deterministic slot without exposing a caller-controlled runtime clock input.
#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn seed_authenticated_series_derived_scalars_at_slot_v1(
    selected_kind: [u8; 32],
    action: u32,
    family_request: &[u8],
    program_id: &Pubkey,
    frame: &HotFrameV3<'_, '_>,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    rent: &Rent,
    child_programs: Option<AuthenticatedChildProgramsV3>,
    observations: &[AccountObservationV1<'_>],
    scalars: &mut [u64],
    now_slot: u64,
) -> Result<bool> {
    if selected_kind != hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes() {
        return Ok(false);
    }
    let request = SeriesActionRequestV3::decode(family_request).map_err(content)?;
    require(u32::from(request.action() as u8) == action)?;
    if !request.action().occurrence_bound() {
        return Ok(false);
    }
    let programs = child_programs.ok_or(TradingSbfError::Content)?;
    let (occurrence_coordinate, ticket_coordinate, replay_coordinate) = match request.action() {
        SeriesActionV3::Prepare => (
            prepare::SERIES_PREPARE_OCCURRENCE_RAW_COORDINATE_V5 as usize,
            prepare::SERIES_PREPARE_TICKET_RAW_COORDINATE_V5 as usize,
            5,
        ),
        SeriesActionV3::Consume => (62, 64, 59),
        SeriesActionV3::Expire => (
            dclutch_trading::series::generated_expire_frame_v5::SERIES_EXPIRE_OCCURRENCE_RAW_COORDINATE_V5 as usize,
            dclutch_trading::series::generated_expire_frame_v5::SERIES_EXPIRE_TICKET_RAW_COORDINATE_V5 as usize,
            5,
        ),
        _ => return Ok(false),
    };
    let (_, occurrence_bytes) = record(
        frame.registry.key,
        observations,
        occurrence_coordinate,
        SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
    )?;
    let (_, ticket_bytes) = record(
        frame.registry.key,
        observations,
        ticket_coordinate,
        SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
    )?;
    let template_bytes = observed(observations, 1)?.data();
    let template = TemplateV3::decode(template_bytes).map_err(content)?;
    let root = observed(observations, 0)?;
    require(root.key() == frame.root.key.to_bytes() && root.owner() == program_id.to_bytes())?;
    let header = CapabilityRootHeaderV1::decode(
        root.data()
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or(TradingSbfError::Content)?,
    )
    .map_err(content)?;
    require(
        header.selection().kind().to_bytes() == selected_kind
            && header.selection().config().to_bytes() == hash(template_bytes).to_bytes()
            && header.release_set().to_bytes() == template.release_set().to_bytes(),
    )?;
    // The Hot config has a synthetic content key; authenticate the actual
    // Registry account through Hot, then join its exact body to this projection.
    require(
        frame
            .config_raw
            .try_borrow_data()
            .map_err(content)?
            .as_ref()
            == template_bytes,
    )?;
    let series = SeriesStateV3::decode(
        root.data()
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or(TradingSbfError::Content)?,
        template.occurrence_count(),
    )
    .map_err(content)?;
    let ticket = admit_ticket(ticket_bytes).map_err(content)?;
    let replay = observed(observations, replay_coordinate)?;
    let expected_ticket = Pubkey::find_program_address(
        &TicketStateSeedsV3::new(root.key(), ticket.content_id()).as_slices(),
        program_id,
    )
    .0;
    require(replay.key() == expected_ticket.to_bytes())?;
    let ticket_state = if request.action() == SeriesActionV3::Prepare {
        require(replay.owner() == system_program::ID.to_bytes() && replay.data().is_empty())?;
        None
    } else {
        require(replay.owner() == program_id.to_bytes())?;
        Some(TicketStateV3::decode(replay.data()).map_err(content)?)
    };
    let siblings: Vec<[u8; 32]> = request
        .proof_bytes()
        .chunks_exact(32)
        .map(|bytes| bytes.try_into().map_err(content))
        .collect::<Result<_>>()?;
    let snapshot = SeriesOccurrenceSnapshotV3 {
        template_bytes,
        occurrence_bytes,
        ticket_bytes,
        siblings: &siblings,
        series,
        ticket_state,
        now_slot,
    };
    let projection = AuthenticatedProductProjectionV2::new(
        dclutch_core_contract::ContentId::new(
            product.runtime.product_record.content_digest.to_bytes(),
        )
        .map_err(content)?,
        dclutch_core_contract::ContentId::new(product.runtime.product_id.to_bytes())
            .map_err(content)?,
        dclutch_core_contract::ContentId::new(
            product
                .runtime
                .result_domain_record
                .content_digest
                .to_bytes(),
        )
        .map_err(content)?,
    );
    let occurrence =
        admit_occurrence(template_bytes, occurrence_bytes, &siblings).map_err(content)?;
    let escrow = derive_prefounding_escrow(
        occurrence,
        ticket_bytes,
        projection,
        account_key(frame.registry.key.to_bytes())?,
    )
    .map_err(content)?;
    let market = Pubkey::find_program_address(
        &escrow.future_market().seeds().as_slices(),
        frame.core_program.key,
    )
    .0;
    escrow
        .future_market()
        .require_address(account_key(market.to_bytes())?)
        .map_err(content)?;
    match request.action() {
        SeriesActionV3::Prepare => seed_prepare(
            family_request,
            snapshot,
            &escrow,
            projection,
            program_id,
            frame,
            product,
            rent,
            programs,
            observations,
            scalars,
        )?,
        SeriesActionV3::Consume | SeriesActionV3::Expire => seed_terminal(
            request.action(),
            family_request,
            snapshot,
            &escrow,
            projection,
            program_id,
            frame,
            product,
            rent,
            programs,
            observations,
            scalars,
        )?,
        _ => return Ok(false),
    }
    Ok(true)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn seed_prepare(
    family: &[u8],
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
    escrow: &PrefoundingSeriesEscrowV3,
    projection: AuthenticatedProductProjectionV2,
    program_id: &Pubkey,
    frame: &HotFrameV3<'_, '_>,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    rent: &Rent,
    programs: AuthenticatedChildProgramsV3,
    observations: &[AccountObservationV1<'_>],
    scalars: &mut [u64],
) -> Result<()> {
    require(scalars.len() == prepare::SERIES_PREPARE_COMMON_SCALAR_COUNT_V5 as usize)?;
    require(
        observed(
            observations,
            prepare::SERIES_PREPARE_CUSTODY_PROGRAM_COORDINATE_V5 as usize,
        )?
        .key()
            == programs.custody,
    )?;
    let (realm_digest, realm_bytes) = record(
        frame.registry.key,
        observations,
        21,
        REALM_SCHEMA_RELEASE_ID_V1,
    )?;
    require(realm_digest == escrow.realm().to_bytes())?;
    let realm = RealmV1::decode(realm_bytes).map_err(content)?;
    let cap = derive_series_prepare_principal_cap_v1(
        frame.registry.key,
        escrow
            .future_market()
            .identity()
            .resolution_policy
            .to_bytes(),
        projection.product_record().to_bytes(),
        *realm.collateral_mint(),
        product.payout_scale,
        observations,
    )?;
    let custody = SeriesCustodyPhysicalV3 {
        caller_program: program_id.to_bytes(),
        parent_request_digest: [0; 32],
        payer: observed(observations, 14)?.key(),
        mint: *realm.collateral_mint(),
        token_program: *realm.token_program(),
        founder_source: observed(observations, 107)?.key(),
        escrow_vault: observed(observations, 91)?.key(),
        hoard_vault: observed(observations, 60)?.key(),
        refund_destination: [0; 32],
        rent_credit: observed(observations, 12)?.key(),
        replay_rent_lamports: rent.minimum_balance(dclutch_custody::CUSTODY_REPLAY_BYTES_V1),
        vault_rent_lamports: rent.minimum_balance(dclutch_custody::token_svm::ACCOUNT_BYTES),
    };
    let projected = SeriesProjectedCustodyPhysicalV3 {
        caller_program: custody.caller_program,
        core_program: frame.core_program.key.to_bytes(),
        rent_program: observed(observations, 20)?.key(),
        parent_capability_root: frame.root.key.to_bytes(),
        projection_receipt_digest: [0; 32],
        payer: custody.payer,
        rent_credit: custody.rent_credit,
        hoard_vault: custody.hoard_vault,
        escrow_vault: custody.escrow_vault,
        mint: custody.mint,
        token_program: custody.token_program,
        collateral_release: *realm.collateral_adapter_release_id(),
        projected_state_rent_lamports: rent
            .minimum_balance(dclutch_custody::PROJECTED_CUSTODY_STATE_BYTES_V2),
        hoard_vault_rent_lamports: custody.vault_rent_lamports,
        escrow_replay_rent_lamports: custody.replay_rent_lamports,
        escrow_vault_rent_lamports: custody.vault_rent_lamports,
    };
    for coordinate in prepare::SERIES_PREPARE_CHILD_CREATED_COORDINATES_V5 {
        let vacancy = observed(observations, coordinate as usize)?;
        require(vacancy.owner() == system_program::ID.to_bytes() && vacancy.data().is_empty())?;
    }
    require(
        observed(observations, 106)?.key() == custody.mint
            && observed(observations, 110)?.key() == custody.token_program,
    )?;
    authenticate_custody_addresses(
        escrow,
        programs.custody,
        projected,
        observed(observations, 7)?.key(),
        observed(observations, 76)?.key(),
    )?;
    authenticate_credit(escrow, observed(observations, 12)?, projected.rent_program)?;
    let bank = derive_series_prepare_requests_v1(SeriesPrepareDerivedRequestInputV1 {
        family_request: family,
        snapshot,
        product: projection,
        registry_program: account_key(frame.registry.key.to_bytes())?,
        parent_root: frame.root.key.to_bytes(),
        custody,
        projected_custody: projected,
        principal_cap_sets: cap,
    })
    .map_err(content)?;
    bank.write_scalar_words(
        family,
        frame.root.key.to_bytes(),
        &mut scalars[prepare::SERIES_PREPARE_DERIVED_REQUEST_SCALAR_START_V1 as usize
            ..prepare::SERIES_PREPARE_DERIVED_REQUEST_SCALAR_END_V1 as usize],
    )
    .map_err(content)?;
    bank.write_replay_scalars(
        family,
        frame.root.key.to_bytes(),
        &mut scalars[prepare::SERIES_PREPARE_RESULT_PREPARED_SCALAR_V1 as usize..],
    )
    .map_err(content)
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn seed_terminal(
    action: SeriesActionV3,
    family: &[u8],
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
    escrow: &PrefoundingSeriesEscrowV3,
    projection: AuthenticatedProductProjectionV2,
    program_id: &Pubkey,
    frame: &HotFrameV3<'_, '_>,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    rent: &Rent,
    programs: AuthenticatedChildProgramsV3,
    observations: &[AccountObservationV1<'_>],
    scalars: &mut [u64],
) -> Result<()> {
    let is_consume = action == SeriesActionV3::Consume;
    let (state_coordinate, source_coordinate, credit_coordinate) = if is_consume {
        (6, 17, 11)
    } else {
        (45, 14, 50)
    };
    let state =
        authenticate_projected_state(observed(observations, state_coordinate)?, programs.custody)?;
    let projected = projected_physical(&state);
    require(
        state.phase == dclutch_custody::ProjectedCustodyPhaseV1::HoardOpen
            && state.next_revision == 2
            && state.locked_amount == 0,
    )?;
    let expected_receipt =
        crate::series::derived_prepare_v1::derive_series_project_found_receipt_v1(
            escrow.future_market().identity(),
            projected,
            state.principal_cap_sets,
        )
        .map_err(content)?;
    require(
        hash(&expected_receipt.encode().map_err(content)?).to_bytes()
            == state.request.projection_receipt_digest,
    )?;
    require(
        projected.caller_program == program_id.to_bytes()
            && projected.core_program == frame.core_program.key.to_bytes()
            && projected.parent_capability_root == frame.root.key.to_bytes(),
    )?;
    authenticate_custody_addresses(
        escrow,
        programs.custody,
        projected,
        observed(observations, state_coordinate)?.key(),
        observed(observations, source_coordinate)?.key(),
    )?;
    require(observed(observations, credit_coordinate)?.key() == projected.rent_credit)?;
    authenticate_credit(
        escrow,
        observed(observations, credit_coordinate)?,
        projected.rent_program,
    )?;
    let source = observed(observations, source_coordinate)?;
    require(source.owner() == programs.custody)?;
    let replay = CustodyReplayV1::decode(source.data()).map_err(content)?;
    require(
        replay.caller_program == program_id.to_bytes()
            && replay.market == escrow.market().to_bytes()
            && replay.release_set == escrow.release_set().to_bytes()
            && replay.context == escrow.ticket_id().to_bytes()
            && replay.rent_refund == projected.rent_credit
            && replay.caller_role == CallerRoleV1::Trading
            && replay.realm == escrow.realm().to_bytes()
            && replay.generation == escrow.generation()
            && replay.next_revision == 3
            && replay.open_vault_count == 1,
    )?;
    if is_consume {
        seed_consume(
            family,
            snapshot,
            escrow,
            projection,
            frame,
            product,
            rent,
            programs,
            observations,
            &state,
            replay,
            projected,
            scalars,
        )
    } else {
        require(scalars.len() == expire::SERIES_EXPIRE_COMMON_SCALAR_COUNT_V5 as usize)?;
        let custody = SeriesCustodyPhysicalV3 {
            caller_program: program_id.to_bytes(),
            parent_request_digest: [0; 32],
            payer: projected.payer,
            mint: projected.mint,
            token_program: projected.token_program,
            founder_source: [0; 32],
            escrow_vault: projected.escrow_vault,
            hoard_vault: projected.hoard_vault,
            refund_destination: observed(observations, 17)?.key(),
            rent_credit: projected.rent_credit,
            replay_rent_lamports: projected.escrow_replay_rent_lamports,
            vault_rent_lamports: projected.escrow_vault_rent_lamports,
        };
        let bank = derive_series_expire_requests_v1(SeriesExpireDerivedRequestInputV1 {
            family_request: family,
            snapshot,
            product: projection,
            registry_program: account_key(frame.registry.key.to_bytes())?,
            parent_root: frame.root.key.to_bytes(),
            custody,
            projected,
            principal_cap_sets: state.principal_cap_sets,
        })
        .map_err(content)?;
        bank.write_scalar_words(
            family,
            frame.root.key.to_bytes(),
            &mut scalars[expire::SERIES_EXPIRE_DERIVED_REQUEST_SCALAR_START_V1 as usize..],
        )
        .map_err(content)
    }
}

#[allow(clippy::too_many_arguments)]
#[inline(never)]
fn seed_consume(
    family: &[u8],
    snapshot: SeriesOccurrenceSnapshotV3<'_>,
    escrow: &PrefoundingSeriesEscrowV3,
    projection: AuthenticatedProductProjectionV2,
    frame: &HotFrameV3<'_, '_>,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    rent: &Rent,
    programs: AuthenticatedChildProgramsV3,
    observations: &[AccountObservationV1<'_>],
    state: &ProjectedCustodyStateV2,
    replay: CustodyReplayV1,
    projected: SeriesProjectedCustodyPhysicalV3,
    scalars: &mut [u64],
) -> Result<()> {
    require(scalars.len() == consume::SERIES_CONSUME_COMMON_SCALAR_COUNT_V4 as usize)?;
    let predicted = project_core(
        escrow,
        frame,
        product,
        state.principal_cap_sets,
        projected.rent_credit,
    )?;
    let claims = claims_physical(
        escrow,
        product,
        rent,
        programs,
        frame.core_program.key,
        observations,
    )?;
    let bank = derive_series_consume_requests_v1(SeriesConsumeDerivedRequestInputV1 {
        family_request: family,
        snapshot,
        founding: SeriesFoundingChildrenInputV1 {
            template: snapshot.template_bytes,
            occurrence: snapshot.occurrence_bytes,
            siblings: snapshot.siblings,
            ticket: snapshot.ticket_bytes,
            product: projection,
            registry_program: account_key(frame.registry.key.to_bytes())?,
            projected: SeriesProjectedCustodyPhysicalInputV1 {
                physical: projected,
            },
            principal_cap_sets: state.principal_cap_sets,
            projected_state: *state,
            source_replay: replay,
            source_replay_account: observed(observations, 17)?.key(),
            realized_hoard_replay_account: observed(observations, 6)?.key(),
            predicted_core_state: &predicted,
            parent_root: frame.root.key.to_bytes(),
            trading_program: frame.trading_program.key.to_bytes(),
            rent_program: projected.rent_program,
            claims,
        },
        ticket_state_account: account_key(observed(observations, 59)?.key())?,
        permit_account: account_key(observed(observations, 67)?.key())?,
        principal_cap_sets: state.principal_cap_sets,
    })
    .map_err(content)?;
    bank.write_scalar_words(
        family,
        frame.root.key.to_bytes(),
        &mut scalars[consume::SERIES_CONSUME_DERIVED_REQUEST_SCALAR_START_V1 as usize..],
    )
    .map_err(content)
}

#[inline(never)]
fn authenticate_projected_state(
    observation: AccountObservationV1<'_>,
    custody: [u8; 32],
) -> Result<Box<ProjectedCustodyStateV2>> {
    require(observation.owner() == custody)?;
    let state = ProjectedCustodyStateV2::decode(observation.data()).map_err(content)?;
    let seeds = ProjectedCustodyStateSeedsV2::from_request(state.request);
    let expected =
        Pubkey::find_program_address(&seeds.as_slices(), &Pubkey::new_from_array(custody));
    require(observation.key() == expected.0.to_bytes() && state.bump == expected.1)?;
    Ok(Box::new(state))
}

fn projected_physical(state: &ProjectedCustodyStateV2) -> SeriesProjectedCustodyPhysicalV3 {
    let request = &state.request;
    SeriesProjectedCustodyPhysicalV3 {
        caller_program: request.caller_program,
        core_program: request.core_program,
        rent_program: request.rent_program,
        parent_capability_root: request.parent_capability_root,
        projection_receipt_digest: request.projection_receipt_digest,
        payer: request.payer,
        rent_credit: request.rent_credit,
        hoard_vault: request.hoard_vault,
        escrow_vault: request.funding_source_vault,
        mint: request.mint,
        token_program: request.token_program,
        collateral_release: request.collateral_release,
        projected_state_rent_lamports: request.state_rent_lamports,
        hoard_vault_rent_lamports: request.vault_rent_lamports,
        escrow_replay_rent_lamports: request.funding_source_state_rent_lamports,
        escrow_vault_rent_lamports: request.funding_source_vault_rent_lamports,
    }
}

fn authenticate_custody_addresses(
    escrow: &PrefoundingSeriesEscrowV3,
    custody: [u8; 32],
    physical: SeriesProjectedCustodyPhysicalV3,
    state: [u8; 32],
    source_replay: [u8; 32],
) -> Result<()> {
    let program = Pubkey::new_from_array(custody);
    let market = escrow.market().to_bytes();
    let release = escrow.release_set().to_bytes();
    let ticket = escrow.ticket_id().to_bytes();
    let context = hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, &ticket]).to_bytes();
    require(
        state
            == Pubkey::find_program_address(
                &ProjectedCustodyStateSeedsV2::new(market, release, context).as_slices(),
                &program,
            )
            .0
            .to_bytes(),
    )?;
    require(
        source_replay
            == Pubkey::find_program_address(
                &CustodyReplaySeedsV1::new(market, release, CallerRoleV1::Trading, ticket)
                    .as_slices(),
                &program,
            )
            .0
            .to_bytes(),
    )?;
    for (address, context, compartment) in [
        (physical.hoard_vault, context, CompartmentV1::HoardPrincipal),
        (physical.escrow_vault, ticket, CompartmentV1::SeriesEscrow),
    ] {
        require(
            address
                == Pubkey::find_program_address(
                    &CustodyVaultSeedsV1::new(market, release, context, compartment).as_slices(),
                    &program,
                )
                .0
                .to_bytes(),
        )?;
    }
    Ok(())
}

fn authenticate_credit(
    escrow: &PrefoundingSeriesEscrowV3,
    observed: AccountObservationV1<'_>,
    rent_program: [u8; 32],
) -> Result<()> {
    use super::LifecycleRentCreditV2;
    use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
    let credit = LifecycleRentCreditV2::decode(observed.data()).map_err(content)?;
    require(
        credit.refund_wallet().to_bytes() == escrow.refund_owner().to_bytes()
            && credit.market().to_bytes() == escrow.market().to_bytes()
            && credit.release_set().to_bytes() == escrow.release_set().to_bytes()
            && credit.generation() == escrow.generation()
            && observed.owner() == rent_program
            && funded_rent_persists_v1(observed.lamports()),
    )?;
    let seeds = credit.pda_seeds();
    let bump = [seeds.bump()];
    let market = seeds.market().to_bytes();
    let generation = seeds.generation();
    let address = Pubkey::create_program_address(
        &[seeds.domain(), &market, &generation, &bump],
        &Pubkey::new_from_array(rent_program),
    )
    .map_err(content)?;
    require(observed.key() == address.to_bytes())
}

#[inline(never)]
fn project_core(
    escrow: &PrefoundingSeriesEscrowV3,
    frame: &HotFrameV3<'_, '_>,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    cap: u64,
    rent_credit: [u8; 32],
) -> Result<CoreState> {
    use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    let identity = escrow.future_market().identity();
    let realm = identity.realm_id.to_bytes();
    let raw = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, &REALM_SCHEMA_RELEASE_ID_V1, &realm],
        frame.registry.key,
    )
    .1;
    let staging = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &REALM_SCHEMA_RELEASE_ID_V1,
            &realm,
        ],
        frame.registry.key,
    )
    .1;
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(identity).as_slices(),
        frame.core_program.key,
    )
    .1;
    let mut graph = product.record_bumps.0;
    // Core's projected Found walks exactly Product, Domain, and Portfolio.
    graph[6..8].fill(0);
    project_found_core_state_v1(
        identity,
        cap,
        Identity::new(rent_credit).map_err(content)?,
        StateBumpsV1 {
            market: StateBumpsV1::record(market),
            realm_raw_record: StateBumpsV1::record(raw),
            realm_staging_record: StateBumpsV1::record(staging),
            product_graph: ProductGraphBumpsV1::record(graph),
        },
    )
    .map_err(content)
}

#[inline(never)]
fn claims_physical(
    escrow: &PrefoundingSeriesEscrowV3,
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
    rent: &Rent,
    programs: AuthenticatedChildProgramsV3,
    core_program: &Pubkey,
    observations: &[AccountObservationV1<'_>],
) -> Result<SeriesFoundingClaimsPhysicalV1> {
    use dclutch_claims::{
        founding_v5::ClaimsFoundingAggregateSeedsV5,
        liability_basis_state_v2::{
            LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
            liability_basis_vector_width_v2,
        },
        protocol_position_v2::{
            PROTOCOL_POSITION_ADMISSION_BYTES_V2, ProtocolPositionAdmissionSeedsV2,
            ProtocolPositionSeedsV2,
        },
    };
    let claims_program = Pubkey::new_from_array(programs.claims);
    let aggregate = Pubkey::find_program_address(
        &ClaimsFoundingAggregateSeedsV5::new(escrow.market().to_bytes())
            .map_err(content)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let founder = escrow.founder().to_bytes();
    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), founder)
            .map_err(content)?
            .as_slices(),
        &claims_program,
    )
    .0;
    let admission = Pubkey::find_program_address(
        &ProtocolPositionAdmissionSeedsV2::new(aggregate.to_bytes(), founder)
            .map_err(content)?
            .as_slices(),
        &claims_program,
    )
    .0;
    for (coordinate, address) in [(94, aggregate), (95, position), (96, admission)] {
        let account = observed(observations, coordinate)?;
        require(
            account.key() == address.to_bytes()
                && account.owner() == system_program::ID.to_bytes()
                && account.data().is_empty(),
        )?;
    }
    let permit = Pubkey::find_program_address(
        &dclutch_market::SeriesFoundingPermitSeedsV1::new(
            Identity::new(escrow.release_set().to_bytes()).map_err(content)?,
            Identity::new(escrow.market().to_bytes()).map_err(content)?,
            Identity::new(escrow.ticket_id().to_bytes()).map_err(content)?,
        )
        .as_slices(),
        core_program,
    );
    require(observed(observations, 67)?.key() == permit.0.to_bytes())?;
    let count = product.runtime.outcome_count;
    Ok(SeriesFoundingClaimsPhysicalV1 {
        linked_basis_record_digest: product.linked_basis_record.content_digest.to_bytes(),
        semantic_basis_id: product.semantic_basis_id.to_bytes(),
        claim_count: count,
        aggregate: aggregate.to_bytes(),
        position: position.to_bytes(),
        admission: admission.to_bytes(),
        claims_program: programs.claims,
        aggregate_rent_principal: rent.minimum_balance(
            liability_basis_vector_width_v2(LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, count)
                .map_err(content)?,
        ),
        position_rent_principal: rent.minimum_balance(
            liability_basis_vector_width_v2(LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, count)
                .map_err(content)?,
        ),
        admission_rent_principal: rent.minimum_balance(PROTOCOL_POSITION_ADMISSION_BYTES_V2),
        observed_aggregate_lamports: observed(observations, 94)?.lamports(),
        observed_position_lamports: observed(observations, 95)?.lamports(),
        observed_admission_lamports: observed(observations, 96)?.lamports(),
        permit_bump: permit.1,
        normal_replay_revision: dclutch_custody::SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
    })
}

fn record<'a>(
    registry: &Pubkey,
    observations: &[AccountObservationV1<'a>],
    coordinate: usize,
    schema: [u8; 32],
) -> Result<([u8; 32], &'a [u8])> {
    authenticate_series_record_observations_v1(
        registry,
        observed(observations, coordinate)?,
        observed(observations, coordinate + 1)?,
        schema,
    )
}
fn observed<'a>(
    observations: &[AccountObservationV1<'a>],
    coordinate: usize,
) -> Result<AccountObservationV1<'a>> {
    observations
        .get(coordinate)
        .copied()
        .ok_or_else(|| TradingSbfError::Content.into())
}
fn account_key(bytes: [u8; 32]) -> Result<AccountKeyV3> {
    AccountKeyV3::new(bytes).map_err(content)
}
fn require(condition: bool) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(TradingSbfError::Content.into())
    }
}
fn content<T: core::fmt::Debug>(cause: T) -> ProgramError {
    solana_program::msg!("Series derived facts: {:?}", cause);
    TradingSbfError::Content.into()
}
