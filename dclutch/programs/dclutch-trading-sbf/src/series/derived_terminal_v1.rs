//! Native per-occurrence Consume and Expire requests for immutable Series Effects.
//!
//! These banks admit the exact family request against current native replay and
//! immutable records, then invoke the existing child semantic owners. Physical
//! adapters must authenticate the supplied account and Source observations.

extern crate alloc;

use alloc::{boxed::Box, vec};

use super::{
    artifacts_v3::{
        SERIES_CONSUME_CORE_REQUEST_BYTES_V3, SERIES_CONSUME_IR_REQUEST_BYTES_V3,
        SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
    },
    custody_v3::{SeriesCustodyPhysicalV3, project_terminal_custody_v3},
    derived_prepare_v1::derive_series_project_found_receipt_v1,
    expire_funding_artifacts_v5::SERIES_EXPIRE_REQUEST_BANK_BYTES_V5,
    founding_children_v1::{SeriesFoundingChildrenInputV1, derive_series_founding_children_v1},
    operator::{SeriesOccurrenceSnapshotV3, build_consume_v3, build_expire_v3},
    projected_custody_v3::{SeriesProjectedCustodyPhysicalV3, project_abort_v3},
};
use dclutch_claims::series_founding_transport_v1::SeriesClaimsFoundingTransportV1;
use dclutch_market::SeriesUnallocatedPermitExpiryRequestV1;
use dclutch_sha256_adapter::digest;
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, admit_occurrence, admit_ticket,
    escrow::expire_series_escrow_v3, series_core_consume_request,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
/// Located refusal from native terminal request derivation.
pub enum SeriesTerminalDerivedRequestErrorV1 {
    /// Immutable evidence, native replay, or schedule refused.
    Content,
    /// The actual family bytes differ from the native snapshot request.
    FamilyRequest,
    /// Physical root or account projections disagree.
    Physical,
    /// The canonical expected Core projection could not be constructed.
    ProjectFound,
    /// The native founding child owner refused its input states.
    Founding,
    /// A native Custody request could not be projected or encoded.
    Custody,
    /// A native Core request could not be projected or encoded.
    Core,
    /// The native Claims receipt transport refused.
    Claims,
    /// The scalar destination differs from the exact codec-derived width.
    ScalarGeometry,
}
type Result<T> = core::result::Result<T, SeriesTerminalDerivedRequestErrorV1>;

#[derive(Clone, Debug, Eq, PartialEq)]
/// Private family-and-root-bound Consume request bank.
pub struct SeriesConsumeDerivedRequestsV1 {
    family_digest: [u8; 32],
    parent_root: [u8; 32],
    requests: Box<[u8]>,
}

#[derive(Clone, Copy)]
/// Same-snapshot semantic and physical inputs for one Consume.
pub struct SeriesConsumeDerivedRequestInputV1<'a> {
    /// Exact current family instruction bytes.
    pub family_request: &'a [u8],
    /// Admitted records, replay observations, and current Clock guard.
    pub snapshot: SeriesOccurrenceSnapshotV3<'a>,
    /// Typed observations consumed by the canonical founding child owner.
    pub founding: SeriesFoundingChildrenInputV1<'a>,
    /// Authenticated Ticket replay PDA passed to the Core child.
    pub ticket_state_account: AccountKeyV3,
    /// Authenticated Core permit PDA used by the Claims transport.
    pub permit_account: AccountKeyV3,
    /// Principal ceiling projected by the authenticated Source owner.
    pub principal_cap_sets: u64,
}

impl SeriesConsumeDerivedRequestsV1 {
    /// Copy exact words only when both the family and root match; refusal leaves output intact.
    pub fn write_scalar_words(
        &self,
        family_request: &[u8],
        parent_root: [u8; 32],
        output: &mut [u64],
    ) -> Result<()> {
        if parent_root != self.parent_root {
            return Err(SeriesTerminalDerivedRequestErrorV1::Physical);
        }
        write_words(self.family_digest, &self.requests, family_request, output)
    }
    /// Borrow the privately derived complete request bank in native route order.
    pub fn request_bytes(&self) -> &[u8] {
        &self.requests
    }
}

/// Derive Consume from actual prepared replay and native child semantic owners.
pub fn derive_series_consume_requests_v1(
    input: SeriesConsumeDerivedRequestInputV1<'_>,
) -> Result<SeriesConsumeDerivedRequestsV1> {
    let expected = build_consume_v3(input.snapshot)
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Content)?;
    if expected.as_bytes() != input.family_request {
        return Err(SeriesTerminalDerivedRequestErrorV1::FamilyRequest);
    }
    let founding = input.founding;
    if founding.template != input.snapshot.template_bytes
        || founding.occurrence != input.snapshot.occurrence_bytes
        || founding.ticket != input.snapshot.ticket_bytes
        || founding.siblings != input.snapshot.siblings
        || founding.parent_root == [0; 32]
        || founding.parent_root != founding.projected.physical.parent_capability_root
        || founding.trading_program != founding.projected.physical.caller_program
        || founding.rent_program != founding.projected.physical.rent_program
    {
        return Err(SeriesTerminalDerivedRequestErrorV1::Physical);
    }
    if founding.principal_cap_sets != input.principal_cap_sets {
        return Err(SeriesTerminalDerivedRequestErrorV1::Physical);
    }
    let children = derive_series_founding_children_v1(founding)
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Founding)?;
    let mut requests = vec![0_u8; SERIES_CONSUME_IR_REQUEST_BYTES_V3].into_boxed_slice();
    write_projected_request(&children.lock, 0, &mut requests)?;
    write_projected_request(
        &children.realize,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3 + SERIES_CONSUME_CORE_REQUEST_BYTES_V3,
        &mut requests,
    )?;
    write_consume_core(&input, &mut requests)?;
    write_consume_claims(&children.claims, input.permit_account, &mut requests)?;
    Ok(SeriesConsumeDerivedRequestsV1 {
        family_digest: digest(input.family_request),
        parent_root: founding.parent_root,
        requests,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
/// Private family-and-root-bound Expire request bank.
pub struct SeriesExpireDerivedRequestsV1 {
    family_digest: [u8; 32],
    parent_root: [u8; 32],
    requests: Box<[u8]>,
}

#[derive(Clone, Copy)]
/// Same-snapshot semantic and physical inputs for one Expire.
pub struct SeriesExpireDerivedRequestInputV1<'a> {
    /// Exact current family instruction bytes.
    pub family_request: &'a [u8],
    /// Admitted records, replay observations, and current Clock guard.
    pub snapshot: SeriesOccurrenceSnapshotV3<'a>,
    /// Authenticated Product graph projection.
    pub product: AuthenticatedProductProjectionV2,
    /// Selected Registry program identity.
    pub registry_program: AccountKeyV3,
    /// Authenticated current capability-root account identity.
    pub parent_root: [u8; 32],
    /// Observed normal Custody frame; its parent digest is derived here.
    pub custody: SeriesCustodyPhysicalV3,
    /// Observed projected Custody frame; its receipt digest is derived here.
    pub projected: SeriesProjectedCustodyPhysicalV3,
    /// Principal ceiling projected by the authenticated Source owner.
    pub principal_cap_sets: u64,
}

impl SeriesExpireDerivedRequestsV1 {
    /// Copy exact words only when both the family and root match; refusal leaves output intact.
    pub fn write_scalar_words(
        &self,
        family_request: &[u8],
        parent_root: [u8; 32],
        output: &mut [u64],
    ) -> Result<()> {
        if parent_root != self.parent_root {
            return Err(SeriesTerminalDerivedRequestErrorV1::Physical);
        }
        write_words(self.family_digest, &self.requests, family_request, output)
    }
    /// Borrow the privately derived complete request bank in native route order.
    pub fn request_bytes(&self) -> &[u8] {
        &self.requests
    }
}

/// Derive Expire from actual prepared replay after the committed retry deadline.
pub fn derive_series_expire_requests_v1(
    input: SeriesExpireDerivedRequestInputV1<'_>,
) -> Result<SeriesExpireDerivedRequestsV1> {
    derive_expire(&input)
}

#[inline(never)]
fn derive_expire(
    input: &SeriesExpireDerivedRequestInputV1<'_>,
) -> Result<SeriesExpireDerivedRequestsV1> {
    require_expire_family(input)?;
    if input.parent_root == [0; 32]
        || input.projected.parent_capability_root != input.parent_root
        || input.custody.caller_program != input.projected.caller_program
        || input.custody.mint != input.projected.mint
        || input.custody.token_program != input.projected.token_program
        || input.custody.escrow_vault != input.projected.escrow_vault
        || input.custody.hoard_vault != input.projected.hoard_vault
        || input.custody.rent_credit != input.projected.rent_credit
        || input.custody.replay_rent_lamports != input.projected.escrow_replay_rent_lamports
        || input.custody.vault_rent_lamports != input.projected.escrow_vault_rent_lamports
    {
        return Err(SeriesTerminalDerivedRequestErrorV1::Physical);
    }
    let (escrow, expiry) = expiry_context(input)?;
    let mut projected = input.projected;
    projected.projection_receipt_digest = expected_projection_digest(
        escrow.future_market().identity(),
        projected,
        input.principal_cap_sets,
    )?;
    let mut custody = input.custody;
    custody.parent_request_digest = digest(input.family_request);
    let mut requests = vec![0_u8; SERIES_EXPIRE_REQUEST_BANK_BYTES_V5].into_boxed_slice();
    write_expire_normal(expire_series_escrow_v3(*escrow), custody, &mut requests)?;
    write_expire_abort(*escrow, expiry, projected, &mut requests)?;
    let ticket_state = input
        .snapshot
        .ticket_state
        .ok_or(SeriesTerminalDerivedRequestErrorV1::Content)?;
    let permit = SeriesUnallocatedPermitExpiryRequestV1::new(
        input.snapshot.series.revision(),
        ticket_state.revision(),
    )
    .encode();
    copy_request(
        &mut requests,
        3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3 + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        &permit,
    )?;
    Ok(SeriesExpireDerivedRequestsV1 {
        family_digest: digest(input.family_request),
        parent_root: input.parent_root,
        requests,
    })
}

#[inline(never)]
fn require_expire_family(input: &SeriesExpireDerivedRequestInputV1<'_>) -> Result<()> {
    let expected = build_expire_v3(input.snapshot)
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Content)?;
    if expected.as_bytes() != input.family_request {
        return Err(SeriesTerminalDerivedRequestErrorV1::FamilyRequest);
    }
    Ok(())
}

#[inline(never)]
fn write_projected_request(
    request: &dclutch_custody::ProjectedCustodyRequestV1,
    offset: usize,
    output: &mut [u8],
) -> Result<()> {
    let bytes = request
        .encode()
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Custody)?;
    copy_request(output, offset, &bytes)
}

#[inline(never)]
fn write_consume_claims(
    request: &dclutch_claims::founding_v5::ClaimsFoundingRequestV5,
    permit: AccountKeyV3,
    output: &mut [u8],
) -> Result<()> {
    let bytes = SeriesClaimsFoundingTransportV1::from_canonical_v5(permit.to_bytes(), *request)
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Claims)?
        .to_bytes();
    copy_request(
        output,
        SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3
            + SERIES_CONSUME_CORE_REQUEST_BYTES_V3
            + SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3,
        &bytes,
    )
}

#[inline(never)]
fn write_consume_core(
    input: &SeriesConsumeDerivedRequestInputV1<'_>,
    output: &mut [u8],
) -> Result<()> {
    let occurrence = admit_occurrence(
        input.snapshot.template_bytes,
        input.snapshot.occurrence_bytes,
        input.snapshot.siblings,
    )
    .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Content)?;
    let ticket = admit_ticket(input.snapshot.ticket_bytes)
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Content)?;
    let ticket_state = input
        .snapshot
        .ticket_state
        .ok_or(SeriesTerminalDerivedRequestErrorV1::Content)?;
    let core = series_core_consume_request(
        occurrence,
        ticket,
        input.founding.product,
        input.ticket_state_account,
        input.snapshot.series.revision(),
        ticket_state.revision(),
    )
    .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Core)?
    .encode()
    .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Core)?;
    copy_request(output, SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3, &core)?;
    copy_request(
        output,
        SERIES_CONSUME_IR_REQUEST_BYTES_V3 - core.len(),
        &core,
    )
}

#[inline(never)]
fn expiry_context(
    input: &SeriesExpireDerivedRequestInputV1<'_>,
) -> Result<(Box<dclutch_trading::series::PrefoundingSeriesEscrowV3>, u64)> {
    let occurrence = admit_occurrence(
        input.snapshot.template_bytes,
        input.snapshot.occurrence_bytes,
        input.snapshot.siblings,
    )
    .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Content)?;
    let escrow = super::derived_prepare_v1::derive_prefounding_escrow(
        occurrence,
        input.snapshot.ticket_bytes,
        input.product,
        input.registry_program,
    )
    .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Content)?;
    let expiry = occurrence
        .template()
        .retry_through(escrow.occurrence())
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Content)?;
    Ok((escrow, expiry))
}

#[inline(never)]
fn expected_projection_digest(
    future: dclutch_market::MarketIdentity,
    physical: SeriesProjectedCustodyPhysicalV3,
    cap: u64,
) -> Result<[u8; 32]> {
    let receipt = derive_series_project_found_receipt_v1(future, physical, cap)
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::ProjectFound)?;
    Ok(digest(&receipt.encode().map_err(|_| {
        SeriesTerminalDerivedRequestErrorV1::ProjectFound
    })?))
}

#[inline(never)]
fn write_expire_normal(
    plan: dclutch_trading::series::escrow::TerminalSeriesEscrowPlanV3,
    physical: SeriesCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<()> {
    let [refund, vault, replay] = project_terminal_custody_v3(plan, physical)
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Custody)?;
    write_normal_request(refund, 0, output)?;
    write_normal_request(vault, SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, output)?;
    write_normal_request(replay, 2 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, output)
}

#[inline(never)]
fn write_normal_request(
    request: dclutch_custody::CustodyRequestV1,
    offset: usize,
    output: &mut [u8],
) -> Result<()> {
    let bytes = request
        .to_bytes()
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Custody)?;
    copy_request(output, offset, &bytes)
}

#[inline(never)]
fn write_expire_abort(
    escrow: dclutch_trading::series::PrefoundingSeriesEscrowV3,
    expiry: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    output: &mut [u8],
) -> Result<()> {
    let request = project_abort_v3(escrow, expiry, physical)
        .map_err(|_| SeriesTerminalDerivedRequestErrorV1::Custody)?;
    write_projected_request(&request, 3 * SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3, output)
}

fn copy_request(output: &mut [u8], offset: usize, request: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(request.len())
        .ok_or(SeriesTerminalDerivedRequestErrorV1::ScalarGeometry)?;
    output
        .get_mut(offset..end)
        .ok_or(SeriesTerminalDerivedRequestErrorV1::ScalarGeometry)?
        .copy_from_slice(request);
    Ok(())
}

fn write_words(
    expected_digest: [u8; 32],
    requests: &[u8],
    family: &[u8],
    output: &mut [u64],
) -> Result<()> {
    if digest(family) != expected_digest {
        return Err(SeriesTerminalDerivedRequestErrorV1::FamilyRequest);
    }
    if requests.len() % 8 != 0 || output.len() != requests.len() / 8 {
        return Err(SeriesTerminalDerivedRequestErrorV1::ScalarGeometry);
    }
    // Exact codec-sized chunks and destination geometry make all writes total
    // after the two checks; no second full request-size stack scratch is needed.
    for (word, chunk) in output.iter_mut().zip(requests.chunks_exact(8)) {
        let mut bytes = [0_u8; 8];
        bytes.copy_from_slice(chunk);
        *word = u64::from_le_bytes(bytes);
    }
    Ok(())
}
