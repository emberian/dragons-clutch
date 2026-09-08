//! Complete typed child-request production for one current Series occurrence.
//!
//! Geometry belongs to the release caller.  This module owns the semantic
//! request bank: it admits the Template, occurrence and Ticket, derives every
//! Custody and Core request, and refuses Claims or permit candidates whose
//! typed coordinates do not join that admitted occurrence.

extern crate alloc;

use alloc::{boxed::Box, vec};

use crate::series::{
    artifacts_v3::{
        SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3, SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    consume_artifacts_v4::SeriesConsumeChildRequestsV4,
    custody_v3::{
        SeriesCustodyPhysicalV3, project_prepare_custody_v3, project_terminal_custody_v3,
    },
    expire_funding_artifacts_v5::SeriesExpireChildRequestsV5,
    occurrence_artifacts_v4::SeriesPrepareChildRequestsV4,
    projected_custody_v3::{
        SeriesProjectedCustodyPhysicalV3, project_abort_v3, project_consume_v3,
        project_prepare_initialize_v3, project_prepare_open_hoard_v3,
    },
};
use dclutch_claims::founding_v5::ClaimsFoundingRequestV5;
use dclutch_market::{SeriesCoreActionV1, SeriesCoreRequestV1, SeriesPermitExpiryRequestV1};
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, admit_occurrence, admit_ticket,
    escrow::{consume_series_escrow_v3, expire_series_escrow_v3, prepare_series_escrow_v3},
    series_core_consume_request, validate_series_permit_expiry_request_v3,
};

/// Typed physical Claims identities whose semantic meaning is outside Claims.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesClaimsPhysicalV1 {
    /// Selected Claims writer program.
    pub claims_program: [u8; 32],
    /// Projected-Custody replay account consumed by Claims founding.
    pub custody_replay: [u8; 32],
}

/// Admitted evidence and typed child inputs for one current Series occurrence.
#[derive(Clone, Copy, Debug)]
pub struct SeriesChildBankInputV1<'a> {
    /// Immutable Template bytes.
    pub template: &'a [u8],
    /// Candidate occurrence bytes and ordered Merkle siblings.
    pub occurrence: &'a [u8],
    /// Ordered Merkle siblings authenticating `occurrence`.
    pub siblings: &'a [[u8; 32]],
    /// Immutable Ticket bytes.
    pub ticket: &'a [u8],
    /// Product projection authenticated against the occurrence.
    pub product: AuthenticatedProductProjectionV2,
    /// Current Registry program identity.
    pub registry_program: AccountKeyV3,
    /// Normal Custody observations committed by the Prepare parent request.
    pub prepare_custody: SeriesCustodyPhysicalV3,
    /// Normal Custody observations committed by the distinct Expire parent request.
    pub expire_custody: SeriesCustodyPhysicalV3,
    /// Projected-Custody physical observations.
    pub projected_custody: SeriesProjectedCustodyPhysicalV3,
    /// Claims program identity selected by the current release.
    pub claims_physical: SeriesClaimsPhysicalV1,
    /// Typed Claims founding candidate.
    pub claims: ClaimsFoundingRequestV5,
    /// Typed permissionless permit-expiry candidate.
    pub permit_expiry: SeriesPermitExpiryRequestV1,
    /// Series Ticket replay account used by Core.
    pub ticket_state_account: AccountKeyV3,
    /// Current Series replay revision.
    pub expected_series_revision: u64,
    /// Current Ticket replay revision.
    pub expected_ticket_revision: u64,
}

/// Stable refusal from child-request production.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesChildBankErrorV1 {
    /// Template, occurrence, Ticket, Product, or Registry evidence refused.
    Content,
    /// A Custody projection refused its physical observations.
    Custody,
    /// Typed Claims coordinates did not join the admitted occurrence.
    Claims,
    /// Claims must advance the fresh realized Hoard replay, not Lock's closing source.
    ClaimsReplay,
    /// Typed permit expiry did not join the admitted occurrence.
    PermitExpiry,
    /// Core request construction or encoding refused.
    Core,
}

/// Owned canonical current-Series child requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesChildBankV1 {
    requests: Box<[u8]>,
    permit_expiry: SeriesPermitExpiryRequestV1,
    core_expire: SeriesCoreRequestV1,
}

use crate::series::artifacts_v3::{
    SERIES_CONSUME_CLAIMS_OFFSET_V3, SERIES_CONSUME_CORE_FOUND_OFFSET_V3,
    SERIES_CONSUME_CORE_OPEN_OFFSET_V3, SERIES_CONSUME_IR_REQUEST_BYTES_V3,
    SERIES_CONSUME_LOCK_OFFSET_V3, SERIES_CONSUME_REALIZE_OFFSET_V3,
    SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3, SERIES_PREPARE_IR_REQUEST_BYTES_V3,
    SERIES_PREPARE_PROJECTED_INITIALIZE_OFFSET_V3, SERIES_PREPARE_PROJECTED_OPEN_OFFSET_V3,
    SERIES_PREPARE_REPLAY_INITIALIZE_OFFSET_V3,
};
const CONSUME_START: usize = SERIES_PREPARE_IR_REQUEST_BYTES_V3;
const EXPIRE_START: usize = CONSUME_START + SERIES_CONSUME_IR_REQUEST_BYTES_V3;
const BANK_BYTES: usize = EXPIRE_START
    + 3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3
    + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3;

impl SeriesChildBankV1 {
    /// Produce the complete bounded child bank from semantic owners.
    pub fn produce(input: SeriesChildBankInputV1<'_>) -> Result<Self, SeriesChildBankErrorV1> {
        produce_bank(&input)
    }
    /// Borrow Prepare children in canonical route order.
    pub fn prepare_requests(&self) -> SeriesPrepareChildRequestsV4<'_> {
        let take = |offset, len| &self.requests[offset..offset + len];
        SeriesPrepareChildRequestsV4 {
            projected_initialize: take(
                SERIES_PREPARE_PROJECTED_INITIALIZE_OFFSET_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            projected_open: take(
                SERIES_PREPARE_PROJECTED_OPEN_OFFSET_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            replay_initialize: take(
                SERIES_PREPARE_REPLAY_INITIALIZE_OFFSET_V3,
                SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            escrow_open: take(
                SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3,
                SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            escrow_lock: take(
                SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3 + SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
        }
    }
    /// Borrow Consume children in canonical route order.
    pub fn consume_requests(&self) -> SeriesConsumeChildRequestsV4<'_> {
        let take =
            |offset, len| &self.requests[CONSUME_START + offset..CONSUME_START + offset + len];
        SeriesConsumeChildRequestsV4 {
            lock: take(
                SERIES_CONSUME_LOCK_OFFSET_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            core: take(
                SERIES_CONSUME_CORE_FOUND_OFFSET_V3,
                SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            realize: take(
                SERIES_CONSUME_REALIZE_OFFSET_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            claims: take(
                SERIES_CONSUME_CLAIMS_OFFSET_V3,
                SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
        }
    }
    /// Borrow Expire children in canonical route order.
    pub fn expire_requests(&self) -> SeriesExpireChildRequestsV5<'_> {
        let take = |offset, len| &self.requests[EXPIRE_START + offset..EXPIRE_START + offset + len];
        SeriesExpireChildRequestsV5 {
            refund: take(0, SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3)
                .try_into()
                .expect("private canonical request region"),
            close_vault: take(
                SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            close_replay: take(
                2 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            projected_abort: take(
                3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
                SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
            )
            .try_into()
            .expect("private canonical request region"),
            permit_expiry: self.permit_expiry,
            core_expire: self.core_expire,
        }
    }
}

#[inline(never)]
fn produce_bank(
    input: &SeriesChildBankInputV1<'_>,
) -> Result<SeriesChildBankV1, SeriesChildBankErrorV1> {
    let (escrow, expiry) = admitted_context(input)?;
    let mut requests = vec![0_u8; BANK_BYTES].into_boxed_slice();
    write_prepare_projected(&escrow, expiry, input.projected_custody, &mut requests)?;
    write_prepare_normal(
        prepare_series_escrow_v3(*escrow),
        input.prepare_custody,
        &mut requests,
    )?;
    write_consume_projected(&escrow, expiry, input.projected_custody, &mut requests)?;
    write_terminal_normal(
        expire_series_escrow_v3(*escrow),
        input.expire_custody,
        &mut requests,
    )?;
    write_abort(&escrow, expiry, input.projected_custody, &mut requests)?;
    write_claims(&input.claims, &mut requests);
    let core_expire = write_core(input, &mut requests)?;
    Ok(SeriesChildBankV1 {
        requests,
        permit_expiry: input.permit_expiry,
        core_expire,
    })
}

#[inline(never)]
fn admitted_context(
    input: &SeriesChildBankInputV1<'_>,
) -> Result<(Box<dclutch_trading::series::PrefoundingSeriesEscrowV3>, u64), SeriesChildBankErrorV1>
{
    let occurrence = admit_occurrence(input.template, input.occurrence, input.siblings)
        .map_err(|_| SeriesChildBankErrorV1::Content)?;
    let escrow = super::derived_prepare_v1::derive_prefounding_escrow(
        occurrence,
        input.ticket,
        input.product,
        input.registry_program,
    )
    .map_err(|_| SeriesChildBankErrorV1::Content)?;
    let expiry = occurrence
        .template()
        .retry_through(escrow.occurrence())
        .map_err(|_| SeriesChildBankErrorV1::Content)?;
    validate_physical(
        input.prepare_custody,
        input.expire_custody,
        input.projected_custody,
    )?;
    validate_claims(
        input.claims,
        *escrow,
        input.product,
        input.prepare_custody,
        input.projected_custody,
        input.claims_physical,
    )?;
    let ticket = admit_ticket(input.ticket).map_err(|_| SeriesChildBankErrorV1::Content)?;
    validate_series_permit_expiry_request_v3(
        occurrence,
        ticket,
        input.product,
        input.permit_expiry,
    )
    .map_err(|_| SeriesChildBankErrorV1::PermitExpiry)?;
    Ok((escrow, expiry))
}

#[inline(never)]
fn write_prepare_projected(
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
    expiry: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<(), SeriesChildBankErrorV1> {
    let init = project_prepare_initialize_v3(*escrow, expiry, physical)
        .map_err(|_| SeriesChildBankErrorV1::Custody)?;
    write_projected(&init, SERIES_PREPARE_PROJECTED_INITIALIZE_OFFSET_V3, output)?;
    let open = project_prepare_open_hoard_v3(*escrow, expiry, physical)
        .map_err(|_| SeriesChildBankErrorV1::Custody)?;
    write_projected(&open, SERIES_PREPARE_PROJECTED_OPEN_OFFSET_V3, output)
}

#[inline(never)]
fn write_prepare_normal(
    plan: dclutch_trading::series::escrow::PrepareSeriesEscrowPlanV3,
    physical: SeriesCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<(), SeriesChildBankErrorV1> {
    let [init, open, lock] =
        project_prepare_custody_v3(plan, physical).map_err(|_| SeriesChildBankErrorV1::Custody)?;
    write_normal(init, SERIES_PREPARE_REPLAY_INITIALIZE_OFFSET_V3, output)?;
    write_normal(open, SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3, output)?;
    write_normal(
        lock,
        SERIES_PREPARE_ESCROW_OPEN_OFFSET_V3 + SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
        output,
    )
}

#[inline(never)]
fn write_consume_projected(
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
    expiry: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<(), SeriesChildBankErrorV1> {
    let consume = project_consume_v3(consume_series_escrow_v3(*escrow), expiry, physical)
        .map_err(|_| SeriesChildBankErrorV1::Custody)?;
    write_projected(
        &consume.lock_and_close_source,
        CONSUME_START + SERIES_CONSUME_LOCK_OFFSET_V3,
        output,
    )?;
    write_projected(
        &consume.realize_and_close,
        CONSUME_START + SERIES_CONSUME_REALIZE_OFFSET_V3,
        output,
    )
}

#[inline(never)]
fn write_terminal_normal(
    plan: dclutch_trading::series::escrow::TerminalSeriesEscrowPlanV3,
    physical: SeriesCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<(), SeriesChildBankErrorV1> {
    let [refund, vault, replay] =
        project_terminal_custody_v3(plan, physical).map_err(|_| SeriesChildBankErrorV1::Custody)?;
    write_normal(refund, EXPIRE_START, output)?;
    write_normal(
        vault,
        EXPIRE_START + SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
        output,
    )?;
    write_normal(
        replay,
        EXPIRE_START + 2 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
        output,
    )
}

#[inline(never)]
fn write_abort(
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
    expiry: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<(), SeriesChildBankErrorV1> {
    let request =
        project_abort_v3(*escrow, expiry, physical).map_err(|_| SeriesChildBankErrorV1::Custody)?;
    write_projected(
        &request,
        EXPIRE_START + 3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3,
        output,
    )
}

#[inline(never)]
fn write_projected(
    request: &dclutch_custody::ProjectedCustodyRequestV1,
    offset: usize,
    output: &mut [u8],
) -> Result<(), SeriesChildBankErrorV1> {
    let bytes = request
        .encode()
        .map_err(|_| SeriesChildBankErrorV1::Custody)?;
    output[offset..offset + bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

#[inline(never)]
fn write_normal(
    request: dclutch_custody::CustodyRequestV1,
    offset: usize,
    output: &mut [u8],
) -> Result<(), SeriesChildBankErrorV1> {
    let bytes = request
        .to_bytes()
        .map_err(|_| SeriesChildBankErrorV1::Custody)?;
    output[offset..offset + bytes.len()].copy_from_slice(&bytes);
    Ok(())
}

#[inline(never)]
fn write_claims(request: &ClaimsFoundingRequestV5, output: &mut [u8]) {
    let bytes = request.to_bytes();
    let offset = CONSUME_START + SERIES_CONSUME_CLAIMS_OFFSET_V3;
    output[offset..offset + bytes.len()].copy_from_slice(&bytes);
}

#[inline(never)]
fn write_core(
    input: &SeriesChildBankInputV1<'_>,
    output: &mut [u8],
) -> Result<SeriesCoreRequestV1, SeriesChildBankErrorV1> {
    let occurrence = admit_occurrence(input.template, input.occurrence, input.siblings)
        .map_err(|_| SeriesChildBankErrorV1::Content)?;
    let ticket = admit_ticket(input.ticket).map_err(|_| SeriesChildBankErrorV1::Content)?;
    let core = series_core_consume_request(
        occurrence,
        ticket,
        input.product,
        input.ticket_state_account,
        input.expected_series_revision,
        input.expected_ticket_revision,
    )
    .map_err(|_| SeriesChildBankErrorV1::Core)?;
    write_core_request(core, output)?;
    SeriesCoreRequestV1::occurrence(
        SeriesCoreActionV1::Expire,
        core.release_set(),
        core.template(),
        core.ticket().ok_or(SeriesChildBankErrorV1::Core)?,
        core.market().ok_or(SeriesChildBankErrorV1::Core)?,
        core.realm().ok_or(SeriesChildBankErrorV1::Core)?,
        core.product().ok_or(SeriesChildBankErrorV1::Core)?,
        core.beneficiary(),
        core.founder().ok_or(SeriesChildBankErrorV1::Core)?,
        core.occurrence_index(),
        core.expected_series_revision(),
        core.expected_ticket_revision(),
        core.market_rent(),
        core.capability_rent(),
        core.work(),
        core.hoard_principal(),
    )
    .map_err(|_| SeriesChildBankErrorV1::Core)
}

#[inline(never)]
fn write_core_request(
    request: SeriesCoreRequestV1,
    output: &mut [u8],
) -> Result<(), SeriesChildBankErrorV1> {
    let bytes = request.encode().map_err(|_| SeriesChildBankErrorV1::Core)?;
    for offset in [
        SERIES_CONSUME_CORE_FOUND_OFFSET_V3,
        SERIES_CONSUME_CORE_OPEN_OFFSET_V3,
    ] {
        output[CONSUME_START + offset..CONSUME_START + offset + bytes.len()]
            .copy_from_slice(&bytes);
    }
    Ok(())
}

fn validate_claims(
    claims: ClaimsFoundingRequestV5,
    escrow: dclutch_trading::series::PrefoundingSeriesEscrowV3,
    product: AuthenticatedProductProjectionV2,
    custody: SeriesCustodyPhysicalV3,
    projected: SeriesProjectedCustodyPhysicalV3,
    physical: SeriesClaimsPhysicalV1,
) -> Result<(), SeriesChildBankErrorV1> {
    if claims.pre_custody_revision() != 0
        || claims.post_custody_revision() != dclutch_custody::SOURCE_COMPARTMENT_REPLAY_REVISION_V1
    {
        return Err(SeriesChildBankErrorV1::ClaimsReplay);
    }
    let identity = escrow.future_market().identity();
    if claims.release_set() != escrow.release_set().to_bytes()
        || claims.market() != escrow.market().to_bytes()
        || claims.product_record_digest() != product.product_record().to_bytes()
        || claims.product_instance_id() != product.stable_product_id().to_bytes()
        || claims.founder() != escrow.founder().to_bytes()
        || claims.generation() != escrow.generation()
        || claims.collateral_transferred() != escrow.hoard_principal()
        || claims.pre_source_amount() != escrow.hoard_principal()
        || claims.post_source_amount() != 0
        || claims.pre_hoard_amount() != 0
        || claims.post_hoard_amount() != escrow.hoard_principal()
        || claims.funding_source() != custody.escrow_vault
        || claims.hoard() != projected.hoard_vault
        || claims.custody_replay() != physical.custody_replay
        || claims.rent_credit() != projected.rent_credit
        || claims.rent_program() != projected.rent_program
        || claims.claims_program() != physical.claims_program
        || claims.trading_program() != projected.caller_program
        || identity.product_record.to_bytes() != product.product_record().to_bytes()
    {
        return Err(SeriesChildBankErrorV1::Claims);
    }
    Ok(())
}

fn validate_physical(
    prepare: SeriesCustodyPhysicalV3,
    expire: SeriesCustodyPhysicalV3,
    projected: SeriesProjectedCustodyPhysicalV3,
) -> Result<(), SeriesChildBankErrorV1> {
    if prepare.parent_request_digest == expire.parent_request_digest
        || prepare.caller_program != expire.caller_program
        || prepare.payer != expire.payer
        || prepare.mint != expire.mint
        || prepare.token_program != expire.token_program
        || prepare.founder_source != expire.founder_source
        || prepare.escrow_vault != expire.escrow_vault
        || prepare.hoard_vault != expire.hoard_vault
        || prepare.refund_destination != expire.refund_destination
        || prepare.rent_credit != expire.rent_credit
        || prepare.replay_rent_lamports != expire.replay_rent_lamports
        || prepare.vault_rent_lamports != expire.vault_rent_lamports
        || prepare.caller_program != projected.caller_program
        || prepare.escrow_vault != projected.escrow_vault
        || prepare.hoard_vault != projected.hoard_vault
        || prepare.rent_credit != projected.rent_credit
        || prepare.mint != projected.mint
        || prepare.token_program != projected.token_program
        || prepare.replay_rent_lamports != projected.escrow_replay_rent_lamports
        || prepare.vault_rent_lamports != projected.escrow_vault_rent_lamports
    {
        return Err(SeriesChildBankErrorV1::Custody);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::series::{
        account_profile_v4::SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4,
        expire_funding_artifacts_v5::{
            SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5, SeriesExpireAccountProfileInputV5,
        },
        prepare_funding_artifacts_v5::{
            SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5, SeriesPrepareAccountProfileInputV5,
        },
        release_v5::{
            SeriesCurrentReleaseInputV5, compile_series_release_v5,
            emit_current_series_release_source_v5,
        },
    };
    use dclutch_claims::founding_v5::ClaimsFoundingRequestInputV5;
    use dclutch_core_contract::ContentId;
    use dclutch_custody::{CustodyRequestV1, OperationV1, ProjectedCustodyRequestV1};
    use dclutch_market::{FoundingIntentV5, Identity, SeriesFoundingPermitV1};
    use dclutch_trading::series::{
        FoundingFundsV3, admit_occurrence,
        encode::{
            OccurrenceRecordInputV3, TemplateRecordInputV3, encode_occurrence_v3,
            encode_template_v3, encode_ticket_v3,
        },
        occurrence_content_id, template_content_id,
    };

    fn id(value: u8) -> ContentId {
        ContentId::new([value; 32]).expect("identity")
    }
    fn key(value: u8) -> AccountKeyV3 {
        AccountKeyV3::new([value; 32]).expect("key")
    }
    fn mid(bytes: [u8; 32]) -> Identity {
        Identity::new(bytes).expect("core identity")
    }

    struct Fixture {
        template: [u8; dclutch_trading::series::SERIES_TEMPLATE_BYTES_V3],
        occurrence: [u8; dclutch_trading::series::SERIES_OCCURRENCE_BYTES_V3],
        ticket: [u8; dclutch_trading::series::SERIES_TICKET_BYTES_V3],
        product: AuthenticatedProductProjectionV2,
        prepare: SeriesCustodyPhysicalV3,
        expire: SeriesCustodyPhysicalV3,
        projected: SeriesProjectedCustodyPhysicalV3,
        claims: ClaimsFoundingRequestV5,
        bad_claims: ClaimsFoundingRequestV5,
        permit: SeriesPermitExpiryRequestV1,
        bad_permit: SeriesPermitExpiryRequestV1,
    }
    impl Fixture {
        fn input(&self) -> SeriesChildBankInputV1<'_> {
            SeriesChildBankInputV1 {
                template: &self.template,
                occurrence: &self.occurrence,
                siblings: &[],
                ticket: &self.ticket,
                product: self.product,
                registry_program: key(90),
                prepare_custody: self.prepare,
                expire_custody: self.expire,
                projected_custody: self.projected,
                claims_physical: SeriesClaimsPhysicalV1 {
                    claims_program: [45; 32],
                    custody_replay: [44; 32],
                },
                claims: self.claims,
                permit_expiry: self.permit,
                ticket_state_account: key(91),
                expected_series_revision: 7,
                expected_ticket_revision: 3,
            }
        }
    }
    fn fixture() -> Fixture {
        let occurrence = encode_occurrence_v3(OccurrenceRecordInputV3 {
            occurrence: 0,
            scheduled_slot: 100,
            product_record: id(4),
            resolution_policy: id(5),
            liability_basis: id(6),
            rational_representation: id(7),
            capability_manifest: id(8),
            funding_list: id(9),
            market: key(10),
            funds: FoundingFundsV3::new(9, 22, 23, 24).expect("funds"),
        })
        .expect("occurrence");
        let root = occurrence_content_id(&occurrence).expect("occurrence id");
        let template = encode_template_v3(TemplateRecordInputV3 {
            realm: id(1),
            release_set: id(2),
            product_generator: id(11),
            occurrence_generator: id(12),
            capability_template: id(13),
            product_derivation: id(14),
            occurrence_derivation: id(15),
            capability_derivation: id(16),
            funding_derivation: id(17),
            projection_root: root,
            refund_owner: key(19),
            occurrence_count: 1,
            first_slot: 100,
            period_slots: 10,
            retry_window: 5,
            close_rent: 20,
        })
        .expect("template");
        let admitted = admit_occurrence(&template, &occurrence, &[]).expect("admitted occurrence");
        let ticket = encode_ticket_v3(admitted, key(18)).expect("ticket");
        let product = AuthenticatedProductProjectionV2::new(id(4), id(20), id(21));
        let prepare = SeriesCustodyPhysicalV3 {
            caller_program: [30; 32],
            parent_request_digest: [31; 32],
            payer: [32; 32],
            mint: [33; 32],
            token_program: [34; 32],
            founder_source: [35; 32],
            escrow_vault: [36; 32],
            hoard_vault: [37; 32],
            refund_destination: [38; 32],
            rent_credit: [39; 32],
            replay_rent_lamports: 40,
            vault_rent_lamports: 41,
        };
        let expire = SeriesCustodyPhysicalV3 {
            parent_request_digest: [42; 32],
            ..prepare
        };
        let projected = SeriesProjectedCustodyPhysicalV3 {
            caller_program: [30; 32],
            core_program: [43; 32],
            rent_program: [46; 32],
            parent_capability_root: [47; 32],
            projection_receipt_digest: [48; 32],
            payer: [32; 32],
            rent_credit: [39; 32],
            hoard_vault: [37; 32],
            escrow_vault: [36; 32],
            mint: [33; 32],
            token_program: [34; 32],
            collateral_release: [49; 32],
            projected_state_rent_lamports: 50,
            hoard_vault_rent_lamports: 51,
            escrow_replay_rent_lamports: 40,
            escrow_vault_rent_lamports: 41,
        };
        let template_id = template_content_id(&template).expect("template id");
        let ticket_id = dclutch_trading::series::admit_ticket(&ticket)
            .expect("admitted ticket")
            .content_id();
        let expiry = 105;
        let intent = |market| {
            FoundingIntentV5::new(
                1,
                mid(id(2).to_bytes()),
                market,
                mid(id(4).to_bytes()),
                mid(id(5).to_bytes()),
                mid([18; 32]),
                mid(ticket_id.to_bytes()),
                mid([47; 32]),
                mid([44; 32]),
                mid([36; 32]),
                mid([37; 32]),
                mid([52; 32]),
                mid([53; 32]),
                mid([30; 32]),
                mid([45; 32]),
                mid([39; 32]),
                1,
                3,
                3,
                expiry,
                4,
                1,
            )
            .expect("intent")
        };
        let permit = SeriesPermitExpiryRequestV1::new(
            SeriesFoundingPermitV1::new(intent(mid([10; 32])), mid([54; 32]), mid([55; 32]))
                .expect("permit"),
        );
        let bad_permit = SeriesPermitExpiryRequestV1::new(
            SeriesFoundingPermitV1::new(intent(mid([56; 32])), mid([54; 32]), mid([55; 32]))
                .expect("bad permit"),
        );
        let claims_input = ClaimsFoundingRequestInputV5 {
            release_set: id(2).to_bytes(),
            market: [10; 32],
            product_record_digest: id(4).to_bytes(),
            product_instance_id: id(20).to_bytes(),
            linked_basis_record_digest: [57; 32],
            semantic_basis_id: [58; 32],
            founder: [18; 32],
            founding_intent_digest: [59; 32],
            aggregate: [60; 32],
            position: [61; 32],
            admission: [62; 32],
            funding_source: [36; 32],
            hoard: [37; 32],
            custody_replay: [44; 32],
            rent_credit: [39; 32],
            rent_program: [46; 32],
            claims_program: [45; 32],
            trading_program: [30; 32],
            custody_request_digest: [63; 32],
            custody_receipt_digest: [64; 32],
            generation: 1,
            claim_count: 3,
            quantity: 3,
            basis_scale: 3,
            pre_source_amount: 9,
            post_source_amount: 0,
            pre_hoard_amount: 0,
            post_hoard_amount: 9,
            pre_custody_revision: 0,
            post_custody_revision: dclutch_custody::SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
            aggregate_rent_principal: 65,
            position_rent_principal: 66,
            admission_rent_principal: 67,
            observed_aggregate_lamports: 68,
            observed_position_lamports: 69,
            observed_admission_lamports: 70,
            pre_aggregate_revision: 0,
            post_aggregate_revision: 1,
            pre_position_revision: 0,
            post_position_revision: 1,
        };
        let claims = ClaimsFoundingRequestV5::new(claims_input).expect("claims");
        let bad_claims = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            founder: [71; 32],
            ..claims_input
        })
        .expect("bad claims");
        let _ = template_id;
        Fixture {
            template,
            occurrence,
            ticket,
            product,
            prepare,
            expire,
            projected,
            claims,
            bad_claims,
            permit,
            bad_permit,
        }
    }

    #[test]
    fn admitted_bank_produces_and_compiles_current_release() {
        let fixture = fixture();
        let bank = SeriesChildBankV1::produce(fixture.input()).expect("accepted bank");
        let prepare = bank.prepare_requests();
        let replay = CustodyRequestV1::decode(prepare.replay_initialize).expect("prepare replay");
        assert_eq!(
            (
                replay.operation,
                replay.expected_revision,
                replay.resulting_revision,
                replay.amount
            ),
            (OperationV1::InitializeReplay, 0, 1, 0)
        );
        let lock =
            ProjectedCustodyRequestV1::decode(bank.consume_requests().lock).expect("consume lock");
        assert_eq!(
            (lock.expected_revision, lock.resulting_revision, lock.amount),
            (2, 3, 9)
        );
        let refund =
            CustodyRequestV1::decode(bank.expire_requests().refund).expect("expire refund");
        assert_eq!(
            (
                refund.operation,
                refund.expected_revision,
                refund.resulting_revision,
                refund.amount
            ),
            (OperationV1::Transfer, 3, 4, 9)
        );
        assert_eq!(
            bank.expire_requests().core_expire.action(),
            SeriesCoreActionV1::Expire
        );
        let prepare_lengths = [0; SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize];
        let consume_lengths = [0; SERIES_CONSUME_FIXED_ACCOUNT_COUNT_V4];
        let mut expire_lengths = [0; SERIES_EXPIRE_FIXED_ACCOUNT_COUNT_V5 as usize];
        expire_lengths[73] = dclutch_trading::series::SERIES_OCCURRENCE_BYTES_V3 as u32;
        expire_lengths[75] = dclutch_trading::series::SERIES_TICKET_BYTES_V3 as u32;
        let source = emit_current_series_release_source_v5(SeriesCurrentReleaseInputV5 {
            template: template_content_id(&fixture.template).expect("template id"),
            template_occurrence_count: 1,
            consume_shadow_certificate_program: id(80),
            prepare_profile: SeriesPrepareAccountProfileInputV5 {
                fixed_data_lengths: &prepare_lengths,
            },
            prepare_requests: bank.prepare_requests(),
            prepare_ticket_rent_lamports: 81,
            consume_observed_data_lengths: &consume_lengths,
            consume_requests: bank.consume_requests(),
            consume_funding_count: 3,
            expire_profile: SeriesExpireAccountProfileInputV5 {
                fixed_data_lengths: &expire_lengths,
            },
            expire_requests: bank.expire_requests(),
        })
        .expect("current source");
        compile_series_release_v5(source.as_source()).expect("compiled release");
    }
    #[test]
    fn closing_source_revision_cannot_masquerade_as_realized_hoard_replay() {
        let fixture = fixture();
        let mut input = fixture.input();
        input.claims = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
            pre_custody_revision: 2,
            post_custody_revision: 3,
            ..input.claims.input()
        })
        .expect("well-formed different replay transition");
        assert_eq!(
            SeriesChildBankV1::produce(input),
            Err(SeriesChildBankErrorV1::ClaimsReplay)
        );
    }

    #[test]
    fn typed_and_physical_substitutions_refuse_exactly() {
        let fixture = fixture();
        let mut claims = fixture.input();
        claims.claims = fixture.bad_claims;
        assert_eq!(
            SeriesChildBankV1::produce(claims),
            Err(SeriesChildBankErrorV1::Claims)
        );
        let mut permit = fixture.input();
        permit.permit_expiry = fixture.bad_permit;
        assert_eq!(
            SeriesChildBankV1::produce(permit),
            Err(SeriesChildBankErrorV1::PermitExpiry)
        );
        let mut physical = fixture.input();
        physical.expire_custody.escrow_vault = [99; 32];
        assert_eq!(
            SeriesChildBankV1::produce(physical),
            Err(SeriesChildBankErrorV1::Custody)
        );
    }
}
