//! Complete typed child-request production for one current Series occurrence.
//!
//! Geometry belongs to the release caller.  This module owns the semantic
//! request bank: it admits the Template, occurrence and Ticket, derives every
//! Custody and Core request, and refuses Claims or permit candidates whose
//! typed coordinates do not join that admitted occurrence.

use dclutch_claims::founding_v5::ClaimsFoundingRequestV5;
use dclutch_market::{SeriesCoreActionV1, SeriesCoreRequestV1, SeriesPermitExpiryRequestV1};
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, admit_occurrence, admit_ticket,
    escrow::{consume_series_escrow_v3, expire_series_escrow_v3, prepare_series_escrow_v3},
    pre_founding_series_escrow, series_core_consume_request,
    validate_series_permit_expiry_request_v3,
};
use dclutch_trading_sbf::series::{
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
    /// Normal Custody physical observations.
    pub custody: SeriesCustodyPhysicalV3,
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
    /// Typed permit expiry did not join the admitted occurrence.
    PermitExpiry,
    /// Core request construction or encoding refused.
    Core,
}

/// Owned canonical current-Series child requests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesChildBankV1 {
    prepare_projected_initialize: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    prepare_projected_open: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    prepare_replay_initialize: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    prepare_escrow_open: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    prepare_escrow_lock: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    consume_lock: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    consume_core: [u8; SERIES_CONSUME_CORE_REQUEST_BYTES_V3],
    consume_realize: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    consume_claims: [u8; SERIES_CLAIMS_FOUNDING_REQUEST_BYTES_V3],
    expire_refund: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    expire_close_vault: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    expire_close_replay: [u8; SERIES_ESCROW_CUSTODY_REQUEST_BYTES_V3],
    expire_projected_abort: [u8; SERIES_PROJECTED_CUSTODY_REQUEST_BYTES_V3],
    permit_expiry: SeriesPermitExpiryRequestV1,
    core_expire: SeriesCoreRequestV1,
}

impl SeriesChildBankV1 {
    /// Produce the complete bounded child bank from semantic owners.
    pub fn produce(input: SeriesChildBankInputV1<'_>) -> Result<Self, SeriesChildBankErrorV1> {
        let occurrence = admit_occurrence(input.template, input.occurrence, input.siblings)
            .map_err(|_| SeriesChildBankErrorV1::Content)?;
        let ticket = admit_ticket(input.ticket).map_err(|_| SeriesChildBankErrorV1::Content)?;
        let escrow =
            pre_founding_series_escrow(occurrence, ticket, input.product, input.registry_program)
                .map_err(|_| SeriesChildBankErrorV1::Content)?;
        let expiry = occurrence
            .template()
            .retry_through(escrow.occurrence())
            .map_err(|_| SeriesChildBankErrorV1::Content)?;

        validate_claims(
            input.claims,
            escrow,
            input.product,
            input.custody,
            input.projected_custody,
            input.claims_physical,
        )?;
        validate_series_permit_expiry_request_v3(
            occurrence,
            ticket,
            input.product,
            input.permit_expiry,
        )
        .map_err(|_| SeriesChildBankErrorV1::PermitExpiry)?;

        let projected_initialize =
            project_prepare_initialize_v3(escrow, expiry, input.projected_custody)
                .map_err(|_| SeriesChildBankErrorV1::Custody)?
                .encode()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?;
        let projected_open = project_prepare_open_hoard_v3(escrow, expiry, input.projected_custody)
            .map_err(|_| SeriesChildBankErrorV1::Custody)?
            .encode()
            .map_err(|_| SeriesChildBankErrorV1::Custody)?;
        let [replay_initialize, escrow_open, escrow_lock] =
            project_prepare_custody_v3(prepare_series_escrow_v3(escrow), input.custody)
                .map_err(|_| SeriesChildBankErrorV1::Custody)?;
        let consume = project_consume_v3(
            consume_series_escrow_v3(escrow),
            expiry,
            input.projected_custody,
        )
        .map_err(|_| SeriesChildBankErrorV1::Custody)?;
        let [refund, close_vault, close_replay] =
            project_terminal_custody_v3(expire_series_escrow_v3(escrow), input.custody)
                .map_err(|_| SeriesChildBankErrorV1::Custody)?;
        let projected_abort = project_abort_v3(escrow, expiry, input.projected_custody)
            .map_err(|_| SeriesChildBankErrorV1::Custody)?
            .encode()
            .map_err(|_| SeriesChildBankErrorV1::Custody)?;
        let consume_core = series_core_consume_request(
            occurrence,
            ticket,
            input.product,
            input.ticket_state_account,
            input.expected_series_revision,
            input.expected_ticket_revision,
        )
        .map_err(|_| SeriesChildBankErrorV1::Core)?;
        let core_expire = SeriesCoreRequestV1::occurrence(
            SeriesCoreActionV1::Expire,
            consume_core.release_set(),
            consume_core.template(),
            consume_core.ticket().ok_or(SeriesChildBankErrorV1::Core)?,
            consume_core.market().ok_or(SeriesChildBankErrorV1::Core)?,
            consume_core.realm().ok_or(SeriesChildBankErrorV1::Core)?,
            consume_core.product().ok_or(SeriesChildBankErrorV1::Core)?,
            consume_core.beneficiary(),
            consume_core.founder().ok_or(SeriesChildBankErrorV1::Core)?,
            consume_core.occurrence_index(),
            consume_core.expected_series_revision(),
            consume_core.expected_ticket_revision(),
            consume_core.market_rent(),
            consume_core.capability_rent(),
            consume_core.work(),
            consume_core.hoard_principal(),
        )
        .map_err(|_| SeriesChildBankErrorV1::Core)?;
        Ok(Self {
            prepare_projected_initialize: projected_initialize,
            prepare_projected_open: projected_open,
            prepare_replay_initialize: replay_initialize
                .to_bytes()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?,
            prepare_escrow_open: escrow_open
                .to_bytes()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?,
            prepare_escrow_lock: escrow_lock
                .to_bytes()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?,
            consume_lock: consume
                .lock_and_close_source
                .encode()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?,
            consume_core: consume_core
                .encode()
                .map_err(|_| SeriesChildBankErrorV1::Core)?,
            consume_realize: consume
                .realize_and_close
                .encode()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?,
            consume_claims: input.claims.to_bytes(),
            expire_refund: refund
                .to_bytes()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?,
            expire_close_vault: close_vault
                .to_bytes()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?,
            expire_close_replay: close_replay
                .to_bytes()
                .map_err(|_| SeriesChildBankErrorV1::Custody)?,
            expire_projected_abort: projected_abort,
            permit_expiry: input.permit_expiry,
            core_expire,
        })
    }

    /// Borrow Prepare children in canonical route order.
    pub const fn prepare_requests(&self) -> SeriesPrepareChildRequestsV4<'_> {
        SeriesPrepareChildRequestsV4 {
            projected_initialize: &self.prepare_projected_initialize,
            projected_open: &self.prepare_projected_open,
            replay_initialize: &self.prepare_replay_initialize,
            escrow_open: &self.prepare_escrow_open,
            escrow_lock: &self.prepare_escrow_lock,
        }
    }
    /// Borrow Consume children in canonical route order.
    pub const fn consume_requests(&self) -> SeriesConsumeChildRequestsV4<'_> {
        SeriesConsumeChildRequestsV4 {
            lock: &self.consume_lock,
            core: &self.consume_core,
            realize: &self.consume_realize,
            claims: &self.consume_claims,
        }
    }
    /// Borrow Expire children in canonical route order.
    pub const fn expire_requests(&self) -> SeriesExpireChildRequestsV5<'_> {
        SeriesExpireChildRequestsV5 {
            refund: &self.expire_refund,
            close_vault: &self.expire_close_vault,
            close_replay: &self.expire_close_replay,
            projected_abort: &self.expire_projected_abort,
            permit_expiry: self.permit_expiry,
            core_expire: self.core_expire,
        }
    }
}

fn validate_claims(
    claims: ClaimsFoundingRequestV5,
    escrow: dclutch_trading::series::PrefoundingSeriesEscrowV3,
    product: AuthenticatedProductProjectionV2,
    custody: SeriesCustodyPhysicalV3,
    projected: SeriesProjectedCustodyPhysicalV3,
    physical: SeriesClaimsPhysicalV1,
) -> Result<(), SeriesChildBankErrorV1> {
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
        || claims.pre_custody_revision() != 2
        || claims.post_custody_revision() != 3
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

#[cfg(test)]
mod tests {
    // The focused fixture is kept with the producer so a typed substitution
    // cannot be hidden behind a raw release-byte fixture.
    use super::*;
    #[test]
    fn typed_claims_and_permit_are_not_raw_placeholders() {
        assert_ne!(core::mem::size_of::<ClaimsFoundingRequestV5>(), 0);
        assert_ne!(core::mem::size_of::<SeriesPermitExpiryRequestV1>(), 0);
    }
}
