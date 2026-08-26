//! Exact Series V3 projection into canonical pre-founding Custody.
//!
//! The Series kernel owns the future-Market and SeriesEscrow semantics.  This
//! module adds only adapter-authenticated physical identities and Rent facts,
//! then emits the sole canonical [`ProjectedCustodyRequestV1`] wire.  It does
//! not choose an amount, Market, revision, refund owner, or replay context.

use dclutch_custody_contract::{
    CompartmentV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCallerRoleV1,
    ProjectedCustodyOperationV1, ProjectedCustodyRequestV1,
};
use dclutch_series_v3_kernel::escrow::ConsumeSeriesEscrowPlanV3;
use solana_program::hash::hashv;

/// Stable refusal from the Series projected-Custody boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesProjectedCustodyErrorV3 {
    /// An adapter-authenticated identity or digest was zero.
    MissingIdentity,
    /// A chain-derived Rent observation was zero.
    MissingRent,
    /// The canonical Projected Custody request refused the joined values.
    Request,
}

/// Result alias for projected-Custody request construction.
pub type Result<T> = core::result::Result<T, SeriesProjectedCustodyErrorV3>;

/// Physical observations that Series semantics deliberately do not own.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesProjectedCustodyPhysicalV3 {
    /// Current release-selected Trading program.
    pub caller_program: [u8; 32],
    /// Current release-selected Core program.
    pub core_program: [u8; 32],
    /// Infrastructure-selected Rent program.
    pub rent_program: [u8; 32],
    /// Trading capability/root account identity authenticated by the outer.
    pub parent_capability_root: [u8; 32],
    /// SHA-256 of the immediate Core ProjectFound receipt from Prepare.
    pub projection_receipt_digest: [u8; 32],
    /// Exact payer persisted across the projected lifecycle.
    pub payer: [u8; 32],
    /// Permanent RentCredit beneficiary for every close.
    pub rent_credit: [u8; 32],
    /// Canonical future-Market Hoard vault.
    pub hoard_vault: [u8; 32],
    /// Canonical normal-Custody SeriesEscrow source vault.
    pub escrow_vault: [u8; 32],
    /// Realm-selected collateral Mint.
    pub mint: [u8; 32],
    /// Realm-selected token program.
    pub token_program: [u8; 32],
    /// Realm-selected collateral-adapter release.
    pub collateral_release: [u8; 32],
    /// Exact projected replay rent.
    pub projected_state_rent_lamports: u64,
    /// Exact projected Hoard-vault rent.
    pub hoard_vault_rent_lamports: u64,
    /// Exact normal SeriesEscrow replay rent returned on Consume.
    pub escrow_replay_rent_lamports: u64,
    /// Exact normal SeriesEscrow Vault rent returned on Consume.
    pub escrow_vault_rent_lamports: u64,
}

/// Exact two projected requests executed during Prepare.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesProjectedPrepareV3 {
    /// Initialize projected replay from an immediate Core ProjectFound receipt.
    pub initialize: ProjectedCustodyRequestV1,
    /// Open the canonical empty future-Market Hoard.
    pub open_hoard: ProjectedCustodyRequestV1,
}

/// Exact projected requests surrounding Core Found during Consume.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesProjectedConsumeV3 {
    /// Atomically credit Hoard and close the normal SeriesEscrow source.
    pub lock_and_close_source: ProjectedCustodyRequestV1,
    /// After Core Found, realize Hoard and close projected replay.
    pub realize_and_close: ProjectedCustodyRequestV1,
}

/// Project Initialize and OpenHoard without granting either physical authority.
pub fn project_prepare_v3(
    escrow: dclutch_series_v3_kernel::PrefoundingSeriesEscrowV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
) -> Result<SeriesProjectedPrepareV3> {
    let base = base_request(escrow, expiry_slot, physical)?;
    Ok(SeriesProjectedPrepareV3 {
        initialize: with_transition(base, ProjectedCustodyOperationV1::Initialize, 0, 0)?,
        open_hoard: with_transition(base, ProjectedCustodyOperationV1::OpenHoard, 1, 0)?,
    })
}

/// Project atomic source close before Found and realization after Found.
pub fn project_consume_v3(
    plan: ConsumeSeriesEscrowPlanV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
) -> Result<SeriesProjectedConsumeV3> {
    Ok(SeriesProjectedConsumeV3 {
        lock_and_close_source: project_consume_edge_v3(
            plan,
            expiry_slot,
            physical,
            ProjectedCustodyOperationV1::LockHoardAndCloseSource,
            2,
        )?,
        realize_and_close: project_consume_edge_v3(
            plan,
            expiry_slot,
            physical,
            ProjectedCustodyOperationV1::RealizeAndClose,
            3,
        )?,
    })
}

/// Project only the pre-Found Hoard credit and SeriesEscrow source closure.
///
/// This narrow entrypoint lets SBF callers validate one 768-byte child packet
/// at a time rather than retaining both Consume packets in one stack frame.
#[inline(never)]
pub fn project_lock_and_close_source_v3(
    plan: ConsumeSeriesEscrowPlanV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
) -> Result<ProjectedCustodyRequestV1> {
    project_consume_edge_v3(
        plan,
        expiry_slot,
        physical,
        ProjectedCustodyOperationV1::LockHoardAndCloseSource,
        2,
    )
}

/// Project only the post-Found projected-Hoard realization and replay close.
#[inline(never)]
pub fn project_realize_and_close_v3(
    plan: ConsumeSeriesEscrowPlanV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
) -> Result<ProjectedCustodyRequestV1> {
    project_consume_edge_v3(
        plan,
        expiry_slot,
        physical,
        ProjectedCustodyOperationV1::RealizeAndClose,
        3,
    )
}

/// Project expiry cleanup of the prepared but still-empty projected Hoard.
pub fn project_abort_v3(
    escrow: dclutch_series_v3_kernel::PrefoundingSeriesEscrowV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
) -> Result<ProjectedCustodyRequestV1> {
    with_transition(
        base_request(escrow, expiry_slot, physical)?,
        ProjectedCustodyOperationV1::AbortOpenAndClose,
        2,
        0,
    )
}

fn base_request(
    escrow: dclutch_series_v3_kernel::PrefoundingSeriesEscrowV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
) -> Result<ProjectedCustodyRequestV1> {
    for value in [
        physical.caller_program,
        physical.core_program,
        physical.rent_program,
        physical.parent_capability_root,
        physical.projection_receipt_digest,
        physical.payer,
        physical.rent_credit,
        physical.hoard_vault,
        physical.escrow_vault,
        physical.mint,
        physical.token_program,
        physical.collateral_release,
    ] {
        if value == [0; 32] {
            return Err(SeriesProjectedCustodyErrorV3::MissingIdentity);
        }
    }
    if physical.projected_state_rent_lamports == 0
        || physical.hoard_vault_rent_lamports == 0
        || physical.escrow_replay_rent_lamports == 0
        || physical.escrow_vault_rent_lamports == 0
    {
        return Err(SeriesProjectedCustodyErrorV3::MissingRent);
    }
    let identity = escrow.future_market().identity();
    let ticket = escrow.ticket_id().to_bytes();
    let context_digest = hashv(&[PROJECTED_HOARD_CONTEXT_DOMAIN_V1, &ticket]).to_bytes();
    let request = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::Initialize,
        caller_role: ProjectedCallerRoleV1::TradingCapability,
        market: escrow.market().to_bytes(),
        generation: escrow.generation(),
        realm: escrow.realm().to_bytes(),
        product_record: identity.product_record.to_bytes(),
        product: identity.product_id.to_bytes(),
        source: identity.resolution_policy.to_bytes(),
        release_set: escrow.release_set().to_bytes(),
        projection_receipt_digest: physical.projection_receipt_digest,
        parent_capability_root: physical.parent_capability_root,
        context_digest,
        caller_program: physical.caller_program,
        payer: physical.payer,
        core_program: physical.core_program,
        rent_program: physical.rent_program,
        refund_owner: escrow.refund_owner().to_bytes(),
        rent_credit: physical.rent_credit,
        hoard_vault: physical.hoard_vault,
        funding_source_vault: physical.escrow_vault,
        funding_source_context: ticket,
        funding_source_compartment: CompartmentV1::SeriesEscrow,
        mint: physical.mint,
        token_program: physical.token_program,
        collateral_release: physical.collateral_release,
        expiry_slot,
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        state_rent_lamports: physical.projected_state_rent_lamports,
        vault_rent_lamports: physical.hoard_vault_rent_lamports,
        funding_source_replay_revision: 3,
        funding_source_state_rent_lamports: physical.escrow_replay_rent_lamports,
        funding_source_vault_rent_lamports: physical.escrow_vault_rent_lamports,
    };
    request
        .validate()
        .map_err(|_| SeriesProjectedCustodyErrorV3::Request)?;
    Ok(request)
}

#[inline(never)]
fn project_consume_edge_v3(
    plan: ConsumeSeriesEscrowPlanV3,
    expiry_slot: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
    operation: ProjectedCustodyOperationV1,
    expected_revision: u64,
) -> Result<ProjectedCustodyRequestV1> {
    let base = base_request(plan.escrow(), expiry_slot, physical)?;
    if plan.source_replay_revision() != base.funding_source_replay_revision {
        return Err(SeriesProjectedCustodyErrorV3::Request);
    }
    with_transition(base, operation, expected_revision, plan.amount())
}

fn with_transition(
    mut request: ProjectedCustodyRequestV1,
    operation: ProjectedCustodyOperationV1,
    expected_revision: u64,
    amount: u64,
) -> Result<ProjectedCustodyRequestV1> {
    request.operation = operation;
    request.expected_revision = expected_revision;
    request.resulting_revision = expected_revision
        .checked_add(1)
        .ok_or(SeriesProjectedCustodyErrorV3::Request)?;
    request.amount = amount;
    request
        .encode()
        .map_err(|_| SeriesProjectedCustodyErrorV3::Request)?;
    Ok(request)
}

#[cfg(test)]
mod tests {
    use dclutch_core_contract::ContentId;
    use dclutch_series_v3_kernel::escrow::consume_series_escrow_v3;
    use dclutch_series_v3_kernel::{
        AccountKeyV3, AuthenticatedProductProjectionV2, SERIES_OCCURRENCE_BYTES_V3,
        SERIES_TEMPLATE_BYTES_V3, SERIES_TICKET_BYTES_V3, admit_occurrence, admit_ticket,
        generated, occurrence_content_id, pre_founding_series_escrow, template_content_id,
    };
    use solana_program::hash::hashv;

    use super::*;

    fn key(value: u8) -> AccountKeyV3 {
        AccountKeyV3::new([value; 32]).expect("key")
    }

    fn put<const N: usize>(bytes: &mut [u8], offset: usize, value: &[u8; N]) {
        bytes[offset..offset + N].copy_from_slice(value);
    }

    fn node(left: &[u8; 32], right: &[u8; 32]) -> [u8; 32] {
        hashv(&[
            &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
            &[0],
            left,
            right,
        ])
        .to_bytes()
    }

    fn escrow() -> dclutch_series_v3_kernel::PrefoundingSeriesEscrowV3 {
        let mut template: [u8; SERIES_TEMPLATE_BYTES_V3] = generated::SERIES_EXAMPLE_TEMPLATE_V3;
        let occurrence: [u8; SERIES_OCCURRENCE_BYTES_V3] = generated::SERIES_EXAMPLE_OCCURRENCE_V3;
        let mut ticket: [u8; SERIES_TICKET_BYTES_V3] = generated::SERIES_EXAMPLE_TICKET_V3;
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence");
        let siblings = [[90; 32], [91; 32]];
        let root = node(&node(&siblings[0], &occurrence_id.to_bytes()), &siblings[1]);
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &root,
        );
        let template_id = template_content_id(&template).expect("template");
        put(
            &mut ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        let admitted = admit_occurrence(&template, &occurrence, &siblings).expect("occurrence");
        let product = AuthenticatedProductProjectionV2::new(
            admitted.occurrence().product_record(),
            ContentId::new([61; 32]).expect("product"),
            ContentId::new([62; 32]).expect("domain"),
        );
        pre_founding_series_escrow(
            admitted,
            admit_ticket(&ticket).expect("ticket"),
            product,
            key(59),
        )
        .expect("escrow")
    }

    const fn physical() -> SeriesProjectedCustodyPhysicalV3 {
        SeriesProjectedCustodyPhysicalV3 {
            caller_program: [1; 32],
            core_program: [2; 32],
            rent_program: [3; 32],
            parent_capability_root: [4; 32],
            projection_receipt_digest: [5; 32],
            payer: [6; 32],
            rent_credit: [7; 32],
            hoard_vault: [8; 32],
            escrow_vault: [9; 32],
            mint: [10; 32],
            token_program: [11; 32],
            collateral_release: [12; 32],
            projected_state_rent_lamports: 13,
            hoard_vault_rent_lamports: 14,
            escrow_replay_rent_lamports: 15,
            escrow_vault_rent_lamports: 16,
        }
    }

    #[test]
    fn one_series_projection_owns_prepare_consume_and_expiry_coordinates() {
        let escrow = escrow();
        let prepare = project_prepare_v3(escrow, 100, physical()).expect("prepare");
        assert_eq!(prepare.initialize.expected_revision, 0);
        assert_eq!(prepare.open_hoard.expected_revision, 1);
        let consume =
            project_consume_v3(consume_series_escrow_v3(escrow), 100, physical()).expect("consume");
        assert_eq!(
            consume.lock_and_close_source.operation,
            ProjectedCustodyOperationV1::LockHoardAndCloseSource
        );
        assert_eq!(consume.lock_and_close_source.expected_revision, 2);
        assert_eq!(
            consume.lock_and_close_source.amount,
            escrow.hoard_principal()
        );
        assert_eq!(
            consume.realize_and_close.operation,
            ProjectedCustodyOperationV1::RealizeAndClose
        );
        assert_eq!(consume.realize_and_close.expected_revision, 3);
        let abort = project_abort_v3(escrow, 100, physical()).expect("abort");
        assert_eq!(
            abort.operation,
            ProjectedCustodyOperationV1::AbortOpenAndClose
        );
        assert_eq!(abort.expected_revision, 2);
        assert_eq!(abort.amount, 0);
    }

    #[test]
    fn ticket_context_product_and_physical_substitution_refuse() {
        let escrow = escrow();
        let request = project_consume_v3(consume_series_escrow_v3(escrow), 100, physical())
            .expect("consume")
            .lock_and_close_source;
        assert_eq!(
            request.funding_source_context,
            escrow.ticket_id().to_bytes()
        );
        assert_eq!(
            request.context_digest,
            hashv(&[
                PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
                &escrow.ticket_id().to_bytes(),
            ])
            .to_bytes()
        );
        assert_eq!(
            request.product_record,
            escrow.future_market().identity().product_record.to_bytes()
        );
        let mut missing = physical();
        missing.rent_credit = [0; 32];
        assert_eq!(
            project_consume_v3(consume_series_escrow_v3(escrow), 100, missing),
            Err(SeriesProjectedCustodyErrorV3::MissingIdentity)
        );
        let mut no_rent = physical();
        no_rent.escrow_vault_rent_lamports = 0;
        assert_eq!(
            project_prepare_v3(escrow, 100, no_rent),
            Err(SeriesProjectedCustodyErrorV3::MissingRent)
        );
    }
}
