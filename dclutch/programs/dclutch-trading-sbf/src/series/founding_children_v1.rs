//! Typed derivation of the dynamic Series Consume children.
//!
//! The current release commits templates before a root and Core poststate exist.
//! This module is the shared adapter semantic owner that turns those authenticated
//! post-Prepare states into the actual projected receipt digest, permit and
//! Claims request used by one Consume execution.

#![allow(missing_docs)]

extern crate alloc;

use alloc::boxed::Box;

use crate::series::projected_custody_v3::{SeriesProjectedCustodyPhysicalV3, project_consume_v3};
use dclutch_claims::founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5};
use dclutch_custody::{CustodyReplayV1, ProjectedCustodyStateV2};
use dclutch_market::{
    CoreState, FoundingIntentV5, Identity, Phase, Readiness, SeriesFoundingPermitV1,
    SeriesPermitExpiryRequestV1,
};
use dclutch_sha256_adapter::digest;
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, admit_occurrence,
    escrow::consume_series_escrow_v3,
};

/// Projected-Custody physical observations before the realization receipt exists.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesProjectedCustodyPhysicalInputV1 {
    /// Physical projection with every static identity and rent observation.
    pub physical: SeriesProjectedCustodyPhysicalV3,
}

/// Claims accounts and prepaid rent observations that no Series record owns.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFoundingClaimsPhysicalV1 {
    /// Finalized linked-basis record digest.
    pub linked_basis_record_digest: [u8; 32],
    /// Finalized semantic LiabilityBasis identity.
    pub semantic_basis_id: [u8; 32],
    /// M0 Portfolio's exact runtime claim-vector width. The Claims V6
    /// failure-escrow PDA derives from this width even when the categorical
    /// basis leaves that escrow vacant.
    pub claim_count: u32,
    /// Canonical Claims aggregate, founder Position and admission accounts.
    pub aggregate: [u8; 32],
    pub position: [u8; 32],
    pub admission: [u8; 32],
    /// Selected Claims program and observed rent principals.
    pub claims_program: [u8; 32],
    pub aggregate_rent_principal: u64,
    pub position_rent_principal: u64,
    pub admission_rent_principal: u64,
    /// Exact observed lamports at the three vacant Claims accounts.
    pub observed_aggregate_lamports: u64,
    pub observed_position_lamports: u64,
    pub observed_admission_lamports: u64,
    /// Core-selected nonzero permit bump and the normal-replay poststate revision.
    pub permit_bump: u8,
    pub normal_replay_revision: u64,
}

/// Authenticated inputs required to derive one actual Series founding child set.
#[derive(Clone, Copy, Debug)]
pub struct SeriesFoundingChildrenInputV1<'a> {
    /// Immutable admitted Series evidence.
    pub template: &'a [u8],
    pub occurrence: &'a [u8],
    pub siblings: &'a [[u8; 32]],
    pub ticket: &'a [u8],
    pub product: AuthenticatedProductProjectionV2,
    pub registry_program: AccountKeyV3,
    /// Physical projection observations. Its receipt field is ignored and
    /// replaced from the typed Core ProjectFound receipt below.
    pub projected: SeriesProjectedCustodyPhysicalInputV1,
    /// Principal ceiling from the authenticated Source projection.
    pub principal_cap_sets: u64,
    /// Decoded projected state at HoardOpen and normal replay at revision three.
    pub projected_state: ProjectedCustodyStateV2,
    pub source_replay: CustodyReplayV1,
    /// Trading's SeriesEscrow source cursor that projected Lock reads and
    /// closes. It remains distinct from the projected State account Realize
    /// converts into the normal Hoard replay.
    pub source_replay_account: [u8; 32],
    /// Projected State's canonical PDA. Realize rewrites this address in place
    /// to the normal Trading Custody replay that Claims then advances.
    pub realized_hoard_replay_account: [u8; 32],
    /// Canonical Core state Core Found will persist, including its predicted PDA bumps.
    pub predicted_core_state: &'a CoreState,
    /// Current root, selected programs, and Claims physical observations.
    pub parent_root: [u8; 32],
    pub trading_program: [u8; 32],
    pub rent_program: [u8; 32],
    pub claims: SeriesFoundingClaimsPhysicalV1,
}

/// Stable refusal from dynamic Series founding-child derivation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SeriesFoundingChildrenErrorV1 {
    Content,
    Projection,
    Claims,
    Permit,
}

/// Derived dynamic children and the physical projection carrying their receipt digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SeriesFoundingChildrenV1 {
    /// Physical projection for the child bank, whose receipt digest is derived here.
    pub projected_physical: SeriesProjectedCustodyPhysicalV3,
    /// Exact lock and realization requests and their producer receipts.
    pub lock: dclutch_custody::ProjectedCustodyRequestV1,
    pub realize: dclutch_custody::ProjectedCustodyRequestV1,
    pub claims: ClaimsFoundingRequestV5,
    pub permit_expiry: SeriesPermitExpiryRequestV1,
}

/// Derive all dynamic Consume children from admitted content and typed post-Prepare state.
pub fn derive_series_founding_children_v1(
    input: SeriesFoundingChildrenInputV1<'_>,
) -> Result<Box<SeriesFoundingChildrenV1>, SeriesFoundingChildrenErrorV1> {
    let (escrow, expiry) = admitted_escrow(&input)?;
    let projected_input = authenticated_projection(&input, &escrow)?;
    let consume = projected_consume(&escrow, expiry, projected_input)?;
    let lock = &consume.lock_and_close_source;
    let realize = &consume.realize_and_close;
    let (locked, lock_digest, lock_receipt_digest) = lock_poststate(&input, &escrow, lock)?;
    let (realize_digest, receipt_digest, receipt_revision) =
        realization_poststate(&input, &escrow, &locked, realize)?;
    let intent = founding_intent(
        &input,
        &escrow,
        expiry,
        realize_digest,
        receipt_digest,
        receipt_revision,
    )?;
    let intent_digest = intent_digest(&intent)?;
    let claims = founding_claims(
        &input,
        &escrow,
        lock,
        lock_digest,
        lock_receipt_digest,
        intent_digest,
    )?;
    let permit = founding_permit(*intent, intent_digest, &claims)?;
    assemble_children(projected_input, lock, realize, &claims, permit)
}

#[inline(never)]
fn admitted_escrow(
    input: &SeriesFoundingChildrenInputV1<'_>,
) -> Result<
    (Box<dclutch_trading::series::PrefoundingSeriesEscrowV3>, u64),
    SeriesFoundingChildrenErrorV1,
> {
    let occurrence = admit_occurrence(input.template, input.occurrence, input.siblings)
        .map_err(|_| SeriesFoundingChildrenErrorV1::Content)?;
    let escrow = super::derived_prepare_v1::derive_prefounding_escrow(
        occurrence,
        input.ticket,
        input.product,
        input.registry_program,
    )
    .map_err(|_| SeriesFoundingChildrenErrorV1::Content)?;
    // Claims V6 always carries the two failure-escrow coordinates.  The
    // categorical record may leave them vacant, but its runtime width still
    // has to derive their canonical identity; the kernel owns that floor.
    dclutch_product::economic_slice::refunding_failure_index(input.claims.claim_count)
        .map_err(|_| SeriesFoundingChildrenErrorV1::Claims)?;
    let expiry = occurrence
        .template()
        .retry_through(escrow.occurrence())
        .map_err(|_| SeriesFoundingChildrenErrorV1::Content)?;
    Ok((escrow, expiry))
}

#[inline(never)]
fn authenticated_projection(
    input: &SeriesFoundingChildrenInputV1<'_>,
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
) -> Result<SeriesProjectedCustodyPhysicalV3, SeriesFoundingChildrenErrorV1> {
    let future = escrow.future_market().identity();
    let state = input.predicted_core_state;
    if state.phase != Phase::Founding
        || state.readiness != Readiness::Prepaid
        || state.terminal_winner != 0
        || state.outstanding_capabilities != 0
        || state.terminal_receipt.is_some()
        || state.identity.market_id.to_bytes() != escrow.market().to_bytes()
        || state.identity.realm_id != future.realm_id
        || state.identity.product_record != future.product_record
        || state.identity.product_id != future.product_id
        || state.identity.resolution_policy != future.resolution_policy
        || state.identity.capability_manifest != future.capability_manifest
        || state.identity.selected_release_set != future.selected_release_set
        || state.identity.registry_program != future.registry_program
        || state.identity.generation != escrow.generation()
        || state.encode().is_err()
    {
        return Err(SeriesFoundingChildrenErrorV1::Projection);
    }
    let expected_receipt = super::derived_prepare_v1::derive_series_project_found_receipt_v1(
        future,
        input.projected.physical,
        input.principal_cap_sets,
    )
    .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?;
    let project_found_digest: [u8; 32] = digest(
        &expected_receipt
            .encode()
            .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?,
    )
    .into();
    let mut projected_input = input.projected.physical;
    projected_input.projection_receipt_digest = project_found_digest;
    Ok(projected_input)
}

#[inline(never)]
fn projected_consume(
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
    expiry: u64,
    physical: SeriesProjectedCustodyPhysicalV3,
) -> Result<
    Box<crate::series::projected_custody_v3::SeriesProjectedConsumeV3>,
    SeriesFoundingChildrenErrorV1,
> {
    project_consume_v3(consume_series_escrow_v3(*escrow), expiry, physical)
        .map(Box::new)
        .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)
}

#[inline(never)]
fn lock_poststate(
    input: &SeriesFoundingChildrenInputV1<'_>,
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
    lock: &dclutch_custody::ProjectedCustodyRequestV1,
) -> Result<(Box<ProjectedCustodyStateV2>, [u8; 32], [u8; 32]), SeriesFoundingChildrenErrorV1> {
    let lock_digest: [u8; 32] = digest(
        &lock
            .encode()
            .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?,
    )
    .into();
    let (locked, lock_receipt) = input
        .projected_state
        .lock_hoard_and_close_source(
            *lock,
            lock_digest,
            input.source_replay_account,
            input.source_replay,
            escrow.hoard_principal(),
            0,
            0,
            escrow.hoard_principal(),
            input.projected.physical.escrow_vault_rent_lamports,
            input.projected.physical.escrow_replay_rent_lamports,
            input.projected.physical.rent_credit,
            true,
        )
        .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?;
    let lock_receipt_digest: [u8; 32] = digest(
        &lock_receipt
            .encode()
            .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?,
    )
    .into();
    Ok((Box::new(locked), lock_digest, lock_receipt_digest))
}

#[inline(never)]
fn realization_poststate(
    input: &SeriesFoundingChildrenInputV1<'_>,
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
    locked: &ProjectedCustodyStateV2,
    realize: &dclutch_custody::ProjectedCustodyRequestV1,
) -> Result<([u8; 32], [u8; 32], u64), SeriesFoundingChildrenErrorV1> {
    let realize_digest: [u8; 32] = digest(
        &realize
            .encode()
            .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?,
    )
    .into();
    let core_bytes = input
        .predicted_core_state
        .encode()
        .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?;
    let core_digest: [u8; 32] = digest(&core_bytes).into();
    let receipt = locked
        .realize_and_close_ref(
            &realize,
            realize_digest,
            input.predicted_core_state,
            core_digest,
            escrow.hoard_principal(),
            input.projected.physical.rent_credit,
        )
        .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?;
    let receipt_digest: [u8; 32] = digest(
        &receipt
            .encode()
            .map_err(|_| SeriesFoundingChildrenErrorV1::Projection)?,
    )
    .into();
    Ok((realize_digest, receipt_digest, receipt.resulting_revision))
}

#[inline(never)]
fn founding_intent(
    input: &SeriesFoundingChildrenInputV1<'_>,
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
    expiry: u64,
    realize_digest: [u8; 32],
    receipt_digest: [u8; 32],
    receipt_revision: u64,
) -> Result<Box<FoundingIntentV5>, SeriesFoundingChildrenErrorV1> {
    let intent = FoundingIntentV5::new(
        input.claims.permit_bump,
        identity(escrow.release_set().to_bytes())?,
        identity(escrow.market().to_bytes())?,
        identity(input.product.product_record().to_bytes())?,
        identity(
            escrow
                .future_market()
                .identity()
                .resolution_policy
                .to_bytes(),
        )?,
        identity(escrow.founder().to_bytes())?,
        identity(escrow.ticket_id().to_bytes())?,
        identity(input.parent_root)?,
        identity(input.source_replay_account)?,
        identity(input.projected.physical.escrow_vault)?,
        identity(input.projected.physical.hoard_vault)?,
        identity(realize_digest)?,
        identity(receipt_digest)?,
        identity(input.trading_program)?,
        identity(input.claims.claims_program)?,
        identity(input.projected.physical.rent_credit)?,
        escrow.generation(),
        1,
        escrow.hoard_principal(),
        expiry,
        receipt_revision,
        input.claims.normal_replay_revision,
    )
    .map_err(|_| SeriesFoundingChildrenErrorV1::Permit)?;
    Ok(Box::new(intent))
}

#[inline(never)]
fn intent_digest(intent: &FoundingIntentV5) -> Result<[u8; 32], SeriesFoundingChildrenErrorV1> {
    let intent_digest: [u8; 32] = digest(
        &intent
            .encode()
            .map_err(|_| SeriesFoundingChildrenErrorV1::Permit)?,
    )
    .into();
    Ok(intent_digest)
}

#[inline(never)]
fn founding_claims(
    input: &SeriesFoundingChildrenInputV1<'_>,
    escrow: &dclutch_trading::series::PrefoundingSeriesEscrowV3,
    lock: &dclutch_custody::ProjectedCustodyRequestV1,
    lock_digest: [u8; 32],
    lock_receipt_digest: [u8; 32],
    intent_digest: [u8; 32],
) -> Result<Box<ClaimsFoundingRequestV5>, SeriesFoundingChildrenErrorV1> {
    let claims_input = ClaimsFoundingRequestInputV5 {
        release_set: escrow.release_set().to_bytes(),
        market: escrow.market().to_bytes(),
        product_record_digest: input.product.product_record().to_bytes(),
        product_instance_id: input.product.stable_product_id().to_bytes(),
        linked_basis_record_digest: input.claims.linked_basis_record_digest,
        semantic_basis_id: input.claims.semantic_basis_id,
        founder: escrow.founder().to_bytes(),
        founding_intent_digest: intent_digest,
        aggregate: input.claims.aggregate,
        position: input.claims.position,
        admission: input.claims.admission,
        funding_source: input.projected.physical.escrow_vault,
        hoard: input.projected.physical.hoard_vault,
        custody_replay: input.realized_hoard_replay_account,
        rent_credit: input.projected.physical.rent_credit,
        rent_program: input.rent_program,
        claims_program: input.claims.claims_program,
        trading_program: input.trading_program,
        custody_request_digest: lock_digest,
        custody_receipt_digest: lock_receipt_digest,
        generation: escrow.generation(),
        claim_count: input.claims.claim_count,
        quantity: 1,
        basis_scale: escrow.hoard_principal(),
        pre_source_amount: escrow.hoard_principal(),
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: escrow.hoard_principal(),
        pre_custody_revision: lock.expected_revision,
        post_custody_revision: lock.resulting_revision,
        aggregate_rent_principal: input.claims.aggregate_rent_principal,
        position_rent_principal: input.claims.position_rent_principal,
        admission_rent_principal: input.claims.admission_rent_principal,
        observed_aggregate_lamports: input.claims.observed_aggregate_lamports,
        observed_position_lamports: input.claims.observed_position_lamports,
        observed_admission_lamports: input.claims.observed_admission_lamports,
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    };
    let claims = ClaimsFoundingRequestV5::new(claims_input)
        .map_err(|_| SeriesFoundingChildrenErrorV1::Claims)?;
    Ok(Box::new(claims))
}

#[inline(never)]
fn founding_permit(
    intent: FoundingIntentV5,
    intent_digest: [u8; 32],
    claims: &ClaimsFoundingRequestV5,
) -> Result<SeriesPermitExpiryRequestV1, SeriesFoundingChildrenErrorV1> {
    let claims_digest: [u8; 32] = digest(&claims.to_bytes()).into();
    let permit =
        SeriesFoundingPermitV1::new(intent, identity(intent_digest)?, identity(claims_digest)?)
            .map_err(|_| SeriesFoundingChildrenErrorV1::Permit)?;
    Ok(SeriesPermitExpiryRequestV1::new(permit))
}

#[inline(never)]
fn assemble_children(
    physical: SeriesProjectedCustodyPhysicalV3,
    lock: &dclutch_custody::ProjectedCustodyRequestV1,
    realize: &dclutch_custody::ProjectedCustodyRequestV1,
    claims: &ClaimsFoundingRequestV5,
    permit_expiry: SeriesPermitExpiryRequestV1,
) -> Result<Box<SeriesFoundingChildrenV1>, SeriesFoundingChildrenErrorV1> {
    Ok(Box::new(SeriesFoundingChildrenV1 {
        projected_physical: physical,
        lock: *lock,
        realize: *realize,
        claims: *claims,
        permit_expiry,
    }))
}

fn identity(value: [u8; 32]) -> Result<Identity, SeriesFoundingChildrenErrorV1> {
    Identity::new(value).map_err(|_| SeriesFoundingChildrenErrorV1::Content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::series::projected_custody_v3::{
        project_prepare_initialize_v3, project_prepare_open_hoard_v3,
    };
    use dclutch_core_contract::ContentId;
    use dclutch_custody::ProjectedCustodyPhaseV1;
    use dclutch_market::{MarketIdentity, StateBumpsV1};
    use dclutch_trading::series::{
        FoundingFundsV3, admit_occurrence,
        encode::{
            OccurrenceRecordInputV3, TemplateRecordInputV3, encode_occurrence_v3,
            encode_template_v3, encode_ticket_v3,
        },
        occurrence_content_id,
    };
    use dclutch_trading::series::{admit_ticket, pre_founding_series_escrow};

    fn cid(n: u8) -> ContentId {
        ContentId::new([n; 32]).expect("id")
    }
    fn key(n: u8) -> AccountKeyV3 {
        AccountKeyV3::new([n; 32]).expect("key")
    }
    fn ident(n: u8) -> Identity {
        Identity::new([n; 32]).expect("identity")
    }
    fn physical() -> SeriesProjectedCustodyPhysicalV3 {
        SeriesProjectedCustodyPhysicalV3 {
            caller_program: [30; 32],
            core_program: [31; 32],
            rent_program: [32; 32],
            parent_capability_root: [33; 32],
            projection_receipt_digest: [0; 32],
            payer: [34; 32],
            rent_credit: [35; 32],
            hoard_vault: [36; 32],
            escrow_vault: [37; 32],
            mint: [38; 32],
            token_program: [39; 32],
            collateral_release: [40; 32],
            projected_state_rent_lamports: 41,
            hoard_vault_rent_lamports: 42,
            escrow_replay_rent_lamports: 43,
            escrow_vault_rent_lamports: 44,
        }
    }

    #[test]
    fn derives_actual_children_from_hoard_open() {
        let occurrence = encode_occurrence_v3(OccurrenceRecordInputV3 {
            occurrence: 0,
            scheduled_slot: 100,
            product_record: cid(4),
            resolution_policy: cid(5),
            liability_basis: cid(6),
            rational_representation: cid(7),
            capability_manifest: cid(8),
            funding_list: cid(9),
            market: key(10),
            funds: FoundingFundsV3::new(9, 22, 23, 24).expect("funds"),
        })
        .expect("occurrence");
        let template = encode_template_v3(TemplateRecordInputV3 {
            realm: cid(1),
            release_set: cid(2),
            product_generator: cid(11),
            occurrence_generator: cid(12),
            capability_template: cid(13),
            product_derivation: cid(14),
            occurrence_derivation: cid(15),
            capability_derivation: cid(16),
            funding_derivation: cid(17),
            projection_root: occurrence_content_id(&occurrence).expect("root"),
            refund_owner: key(19),
            occurrence_count: 1,
            first_slot: 100,
            period_slots: 10,
            retry_window: 5,
            close_rent: 20,
        })
        .expect("template");
        let admitted = admit_occurrence(&template, &occurrence, &[]).expect("admit");
        let ticket = encode_ticket_v3(admitted, key(18)).expect("ticket");
        let product = AuthenticatedProductProjectionV2::new(cid(4), cid(20), cid(21));
        let mut p = physical();
        let escrow = pre_founding_series_escrow(
            admitted,
            admit_ticket(&ticket).expect("ticket admit"),
            product,
            key(90),
        )
        .expect("escrow");
        let receipt = super::super::derived_prepare_v1::derive_series_project_found_receipt_v1(
            escrow.future_market().identity(),
            p,
            1,
        )
        .expect("canonical expected Core receipt");
        let receipt_digest = digest(&receipt.encode().expect("receipt bytes"));
        p.projection_receipt_digest = receipt_digest;
        let expiry = 105;
        let initialize = project_prepare_initialize_v3(escrow, expiry, p).expect("initialize");
        let init_digest: [u8; 32] = digest(&initialize.encode().expect("init bytes")).into();
        let state = ProjectedCustodyStateV2::initialize(
            initialize,
            receipt,
            [31; 32],
            receipt_digest,
            init_digest,
            100,
            true,
            1,
        )
        .expect("state");
        let open = project_prepare_open_hoard_v3(escrow, expiry, p).expect("open");
        let open_digest: [u8; 32] = digest(&open.encode().expect("open bytes")).into();
        let state = state
            .open_hoard(open, open_digest, 0, true)
            .expect("hoard open");
        assert_eq!(state.phase, ProjectedCustodyPhaseV1::HoardOpen);
        let replay = CustodyReplayV1 {
            caller_role: dclutch_custody::CallerRoleV1::Trading,
            release_set: [2; 32],
            market: [10; 32],
            realm: [1; 32],
            context: dclutch_trading::series::admit_ticket(&ticket)
                .expect("ticket")
                .content_id()
                .to_bytes(),
            caller_program: [30; 32],
            rent_refund: [35; 32],
            open_vault_count: 1,
            next_revision: 3,
            generation: 1,
            last_request_digest: [42; 32],
            last_poststate_commitment: [43; 32],
        };
        let core = CoreState {
            phase: Phase::Founding,
            readiness: Readiness::Prepaid,
            terminal_winner: 0,
            identity: MarketIdentity {
                market_id: ident(10),
                realm_id: ident(1),
                product_record: ident(4),
                product_id: ident(20),
                resolution_policy: ident(5),
                capability_manifest: ident(8),
                selected_release_set: ident(2),
                registry_program: ident(90),
                generation: 1,
            },
            outstanding_capabilities: 0,
            principal_cap_sets: 1,
            rent_beneficiary: ident(35),
            terminal_receipt: None,
            bumps: StateBumpsV1::UNRECORDED,
        };
        let claims = SeriesFoundingClaimsPhysicalV1 {
            linked_basis_record_digest: [50; 32],
            semantic_basis_id: [51; 32],
            claim_count: 3,
            aggregate: [52; 32],
            position: [53; 32],
            admission: [54; 32],
            claims_program: [55; 32],
            aggregate_rent_principal: 56,
            position_rent_principal: 57,
            admission_rent_principal: 58,
            observed_aggregate_lamports: 56,
            observed_position_lamports: 57,
            observed_admission_lamports: 58,
            permit_bump: 1,
            normal_replay_revision: 1,
        };
        let input = SeriesFoundingChildrenInputV1 {
            template: &template,
            occurrence: &occurrence,
            siblings: &[],
            ticket: &ticket,
            product,
            registry_program: key(90),
            projected: SeriesProjectedCustodyPhysicalInputV1 {
                physical: physical(),
            },
            principal_cap_sets: receipt.principal_cap_sets,
            projected_state: state,
            source_replay: replay,
            source_replay_account: [60; 32],
            realized_hoard_replay_account: [61; 32],
            predicted_core_state: &core,
            parent_root: [33; 32],
            trading_program: [30; 32],
            rent_program: [32; 32],
            claims,
        };
        let output = derive_series_founding_children_v1(input).expect("derived children");
        assert_eq!(
            output.projected_physical.projection_receipt_digest,
            receipt_digest
        );
        assert_eq!(output.lock.expected_revision, 2);
        assert_eq!(output.realize.resulting_revision, 4);
        assert_eq!(
            output.claims.custody_replay(),
            [61; 32],
            "Claims advances the projected State address Realize rewrites, not Lock's source cursor",
        );
        assert_eq!(output.claims.collateral_transferred(), 9);
        output
            .permit_expiry
            .permit()
            .join_for_intent_and_request(
                output.permit_expiry.permit().intent(),
                identity(
                    digest(
                        &output
                            .permit_expiry
                            .permit()
                            .intent()
                            .encode()
                            .expect("intent"),
                    )
                    .into(),
                )
                .expect("intent id"),
                identity(digest(&output.claims.to_bytes()).into()).expect("claims id"),
            )
            .expect("permit joins");
        let mut bad = core;
        bad.identity.market_id = ident(61);
        let bad_input = SeriesFoundingChildrenInputV1 {
            predicted_core_state: &bad,
            ..input
        };
        assert_eq!(
            derive_series_founding_children_v1(bad_input),
            Err(SeriesFoundingChildrenErrorV1::Projection)
        );
        let mut narrow_claims = claims;
        narrow_claims.claim_count = 1;
        let narrow_input = SeriesFoundingChildrenInputV1 {
            claims: narrow_claims,
            ..input
        };
        assert_eq!(
            derive_series_founding_children_v1(narrow_input),
            Err(SeriesFoundingChildrenErrorV1::Claims)
        );
    }
}
