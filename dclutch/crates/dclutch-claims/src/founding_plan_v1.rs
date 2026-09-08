//! Shared native construction of a matched Claims founding request and Core permit.
//!
//! Core retains physical admission and actual permit writes. Trading predicts
//! these exact bytes from authenticated occurrence facts; neither the result
//! nor its hashes grant account authority. This module owns the single tuple
//! projection, validates exact complete-set capitalization, and allocates nothing.

use crate::founding_v5::{ClaimsFoundingRequestInputV5, ClaimsFoundingRequestV5};
use dclutch_market::{FoundingIntentV5, Identity, SeriesFoundingPermitV1};
use dclutch_sha256_adapter::digest;

/// Located refusal from the pure matched construction.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FoundingPlanErrorV1 {
    /// Founding cannot capitalize zero principal.
    ZeroPrincipal,
    /// Product's payout scale cannot be zero.
    ZeroScale,
    /// Principal cannot fund even one complete set.
    ZeroQuantity,
    /// Principal is not an exact multiple of Product's payout scale.
    NonIntegralPrincipal,
    /// The supplied quantity differs from the exact capitalized quotient.
    QuantityMismatch,
    /// A required identity is zero.
    Identity,
    /// The native Market intent codec refused.
    Intent,
    /// The native Claims request contract refused.
    Claims,
    /// The native Market permit codec refused.
    Permit,
}
/// Result of native founding construction.
pub type Result<T> = core::result::Result<T, FoundingPlanErrorV1>;

/// Derive positive complete sets with no rounding or discarded collateral.
pub fn founding_quantity_v1(principal: u64, basis_scale: u64) -> Result<u64> {
    if principal == 0 {
        return Err(FoundingPlanErrorV1::ZeroPrincipal);
    }
    let quantity = principal
        .checked_div(basis_scale)
        .ok_or(FoundingPlanErrorV1::ZeroScale)?;
    if quantity == 0 {
        return Err(FoundingPlanErrorV1::ZeroQuantity);
    }
    if principal % basis_scale != 0 {
        return Err(FoundingPlanErrorV1::NonIntegralPrincipal);
    }
    Ok(quantity)
}

/// Native authenticated facts shared by the actual writer and its projection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FoundingPlanInputV1 {
    /// Authenticated bump.
    pub bump: u8,
    /// Authenticated release set.
    pub release_set: [u8; 32],
    /// Authenticated market.
    pub market: [u8; 32],
    /// Authenticated product record.
    pub product_record: [u8; 32],
    /// Authenticated product id.
    pub product_id: [u8; 32],
    /// Authenticated linked basis record.
    pub linked_basis_record: [u8; 32],
    /// Authenticated semantic basis.
    pub semantic_basis: [u8; 32],
    /// Authenticated source.
    pub source: [u8; 32],
    /// Authenticated founder.
    pub founder: [u8; 32],
    /// Authenticated context.
    pub context: [u8; 32],
    /// Authenticated capability root.
    pub capability_root: [u8; 32],
    /// Authenticated projected replay.
    pub projected_replay: [u8; 32],
    /// Authenticated funding source.
    pub funding_source: [u8; 32],
    /// Authenticated hoard.
    pub hoard: [u8; 32],
    /// Authenticated projected request digest.
    pub projected_request_digest: [u8; 32],
    /// Authenticated projected receipt digest.
    pub projected_receipt_digest: [u8; 32],
    /// Authenticated custody lock request digest.
    pub custody_lock_request_digest: [u8; 32],
    /// Authenticated custody lock receipt digest.
    pub custody_lock_receipt_digest: [u8; 32],
    /// Authenticated trading program.
    pub trading_program: [u8; 32],
    /// Authenticated claims program.
    pub claims_program: [u8; 32],
    /// Authenticated rent credit.
    pub rent_credit: [u8; 32],
    /// Authenticated rent program.
    pub rent_program: [u8; 32],
    /// Authenticated aggregate.
    pub aggregate: [u8; 32],
    /// Authenticated position.
    pub position: [u8; 32],
    /// Authenticated admission.
    pub admission: [u8; 32],
    /// Authenticated generation.
    pub generation: u64,
    /// Authenticated claim count.
    pub claim_count: u32,
    /// Authenticated quantity.
    pub quantity: u64,
    /// Authenticated basis scale.
    pub basis_scale: u64,
    /// Authenticated expiry slot.
    pub expiry_slot: u64,
    /// Authenticated projected resulting revision.
    pub projected_resulting_revision: u64,
    /// Authenticated normal replay revision.
    pub normal_replay_revision: u64,
    /// Authenticated source amount.
    pub source_amount: u64,
    /// Authenticated hoard amount.
    pub hoard_amount: u64,
    /// Authenticated aggregate rent.
    pub aggregate_rent: u64,
    /// Authenticated position rent.
    pub position_rent: u64,
    /// Authenticated admission rent.
    pub admission_rent: u64,
    /// Authenticated aggregate lamports.
    pub aggregate_lamports: u64,
    /// Authenticated position lamports.
    pub position_lamports: u64,
    /// Authenticated admission lamports.
    pub admission_lamports: u64,
}

/// Matched wire values; actual Core admission and persistence remain separate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FoundingPlanV1 {
    /// Claims request whose exact digest the permit authorizes.
    pub claims: ClaimsFoundingRequestV5,
    /// Exact Core-owned permit for that request and the shared native intent.
    pub permit: SeriesFoundingPermitV1,
}

/// Derive one matched request and permit through their native codecs.
#[inline(never)]
pub fn derive_founding_plan_v1(input: &FoundingPlanInputV1) -> Result<FoundingPlanV1> {
    let quantity = founding_quantity_v1(input.hoard_amount, input.basis_scale)?;
    if input.quantity != quantity {
        return Err(FoundingPlanErrorV1::QuantityMismatch);
    }
    let intent = build_founding_intent(input)?;
    build_matched_plan(input, &intent)
}

// Keep the independent fixed wire values in separate SBF frames. No caller
// buffer or allocation becomes an authority or an alternative construction.
#[inline(never)]
fn build_matched_plan(
    input: &FoundingPlanInputV1,
    intent: &FoundingIntentV5,
) -> Result<FoundingPlanV1> {
    let intent_digest = digest_intent(intent)?;
    let claims = build_claims_request(input, intent_digest)?;
    finish_matched_plan(intent, intent_digest, &claims)
}

#[inline(never)]
fn finish_matched_plan(
    intent: &FoundingIntentV5,
    intent_digest: [u8; 32],
    claims: &ClaimsFoundingRequestV5,
) -> Result<FoundingPlanV1> {
    let permit = build_permit(intent, intent_digest, claims)?;
    Ok(FoundingPlanV1 {
        claims: *claims,
        permit,
    })
}

#[inline(never)]
fn digest_intent(intent: &FoundingIntentV5) -> Result<[u8; 32]> {
    Ok(digest(
        &intent.encode().map_err(|_| FoundingPlanErrorV1::Intent)?,
    ))
}

#[inline(never)]
fn build_permit(
    intent: &FoundingIntentV5,
    intent_digest: [u8; 32],
    claims: &ClaimsFoundingRequestV5,
) -> Result<SeriesFoundingPermitV1> {
    SeriesFoundingPermitV1::new(
        *intent,
        identity(intent_digest)?,
        identity(digest(&claims.to_bytes()))?,
    )
    .map_err(|_| FoundingPlanErrorV1::Permit)
}

fn identity(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|_| FoundingPlanErrorV1::Identity)
}

#[inline(never)]
fn build_founding_intent(input: &FoundingPlanInputV1) -> Result<FoundingIntentV5> {
    FoundingIntentV5::new(
        input.bump,
        identity(input.release_set)?,
        identity(input.market)?,
        identity(input.product_record)?,
        identity(input.source)?,
        identity(input.founder)?,
        identity(input.context)?,
        identity(input.capability_root)?,
        identity(input.projected_replay)?,
        identity(input.funding_source)?,
        identity(input.hoard)?,
        identity(input.projected_request_digest)?,
        identity(input.projected_receipt_digest)?,
        identity(input.trading_program)?,
        identity(input.claims_program)?,
        identity(input.rent_credit)?,
        input.generation,
        input.quantity,
        input.basis_scale,
        input.expiry_slot,
        input.projected_resulting_revision,
        input.normal_replay_revision,
    )
    .map_err(|_| FoundingPlanErrorV1::Intent)
}

#[inline(never)]
fn build_claims_request(
    input: &FoundingPlanInputV1,
    intent_digest: [u8; 32],
) -> Result<ClaimsFoundingRequestV5> {
    let claims = ClaimsFoundingRequestV5::new(ClaimsFoundingRequestInputV5 {
        release_set: input.release_set,
        market: input.market,
        product_record_digest: input.product_record,
        product_instance_id: input.product_id,
        linked_basis_record_digest: input.linked_basis_record,
        semantic_basis_id: input.semantic_basis,
        founder: input.founder,
        founding_intent_digest: intent_digest,
        aggregate: input.aggregate,
        position: input.position,
        admission: input.admission,
        hoard: input.hoard,
        rent_credit: input.rent_credit,
        rent_program: input.rent_program,
        claims_program: input.claims_program,
        trading_program: input.trading_program,
        funding_source: input.funding_source,
        custody_replay: input.projected_replay,
        custody_request_digest: input.custody_lock_request_digest,
        custody_receipt_digest: input.custody_lock_receipt_digest,
        generation: input.generation,
        claim_count: input.claim_count,
        quantity: input.quantity,
        basis_scale: input.basis_scale,
        pre_source_amount: input.source_amount,
        post_source_amount: 0,
        pre_hoard_amount: 0,
        post_hoard_amount: input.hoard_amount,
        pre_custody_revision: 0,
        post_custody_revision: input.normal_replay_revision,
        aggregate_rent_principal: input.aggregate_rent,
        position_rent_principal: input.position_rent,
        admission_rent_principal: input.admission_rent,
        observed_aggregate_lamports: input.aggregate_lamports,
        observed_position_lamports: input.position_lamports,
        observed_admission_lamports: input.admission_lamports,
        pre_aggregate_revision: 0,
        post_aggregate_revision: 1,
        pre_position_revision: 0,
        post_position_revision: 1,
    })
    .map_err(|_| FoundingPlanErrorV1::Claims)?;
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn id(value: u8) -> [u8; 32] {
        [value; 32]
    }
    fn input() -> FoundingPlanInputV1 {
        FoundingPlanInputV1 {
            bump: 7,
            release_set: id(1),
            market: id(2),
            product_record: id(3),
            product_id: id(4),
            linked_basis_record: id(5),
            semantic_basis: id(6),
            source: id(7),
            founder: id(8),
            context: id(9),
            capability_root: id(10),
            projected_replay: id(11),
            funding_source: id(12),
            hoard: id(13),
            projected_request_digest: id(14),
            projected_receipt_digest: id(15),
            custody_lock_request_digest: id(16),
            custody_lock_receipt_digest: id(17),
            trading_program: id(18),
            claims_program: id(19),
            rent_credit: id(20),
            rent_program: id(21),
            aggregate: id(22),
            position: id(23),
            admission: id(24),
            generation: 1,
            claim_count: 3,
            quantity: 2,
            basis_scale: 5,
            expiry_slot: 100,
            projected_resulting_revision: 3,
            normal_replay_revision: 1,
            source_amount: 10,
            hoard_amount: 10,
            aggregate_rent: 100,
            position_rent: 100,
            admission_rent: 100,
            aggregate_lamports: 100,
            position_lamports: 100,
            admission_lamports: 100,
        }
    }

    #[test]
    fn native_plan_preserves_product_scale_and_rejects_quantity_mirrors() {
        let mut input = input();
        input.quantity = 3;
        input.basis_scale = 3;
        input.source_amount = 9;
        input.hoard_amount = 9;
        let plan = derive_founding_plan_v1(&input).expect("three actual complete sets");
        assert_eq!(plan.claims.quantity(), 3);
        assert_eq!(plan.claims.basis_scale(), 3);
        assert_eq!(plan.permit.intent().quantity(), 3);
        assert_eq!(plan.permit.intent().basis_scale(), 3);
        assert_eq!(plan.claims.custody_replay(), input.projected_replay);
        assert_eq!(plan.claims.pre_custody_revision(), 0);
        assert_eq!(
            plan.claims.post_custody_revision(),
            input.normal_replay_revision
        );
        input.quantity = 1;
        assert_eq!(
            derive_founding_plan_v1(&input),
            Err(FoundingPlanErrorV1::QuantityMismatch)
        );
        input.hoard_amount = 10;
        assert_eq!(
            derive_founding_plan_v1(&input),
            Err(FoundingPlanErrorV1::NonIntegralPrincipal)
        );
    }
    #[test]
    fn complete_set_quotient_is_exact_at_u64_boundaries() {
        assert_eq!(founding_quantity_v1(u64::MAX, u64::MAX), Ok(1));
        assert_eq!(founding_quantity_v1(u64::MAX, 1), Ok(u64::MAX));
        assert_eq!(
            founding_quantity_v1(0, 1),
            Err(FoundingPlanErrorV1::ZeroPrincipal)
        );
        assert_eq!(
            founding_quantity_v1(1, 0),
            Err(FoundingPlanErrorV1::ZeroScale)
        );
        assert_eq!(
            founding_quantity_v1(1, 2),
            Err(FoundingPlanErrorV1::ZeroQuantity)
        );
        assert_eq!(
            founding_quantity_v1(10, 3),
            Err(FoundingPlanErrorV1::NonIntegralPrincipal)
        );
    }
}
