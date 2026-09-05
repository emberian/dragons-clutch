//! Build one conservative complete-set act.
//!
//! `dclutch-claims::conservation` owns the arithmetic and the shape of
//! a split or a merge, and its own module documentation records that nothing
//! reaches it: no dispatcher, no operator, no client. This is the operator half
//! of that gap. It does not make the act reachable — the Claims-owned outer
//! route that would dispatch `DCLCNS01` still does not exist — but it is the
//! first thing in the tree that CONSTRUCTS a conservation request, and building
//! one runs the contract's `validate` against derived coordinates rather than
//! against a hand-written fixture.
//!
//! # What the caller may state, and what it may not
//!
//! A caller states only the market facts it can authenticate and the PRESTATE
//! it observed. Every address is derived here — the Claims aggregate, the
//! actor's Position, the Market's HoardPrincipal vault and the Claims-role
//! Custody replay — and so is the poststate: `post_external` and `post_hoard`
//! are computed from the direction and `quantity * basis_scale`, never taken
//! from the caller.
//!
//! That asymmetry is deliberate. The contract's `ExternalBalanceMismatch` and
//! `HoardBalanceMismatch` conjuncts exist to refuse a request whose stated
//! poststate does not follow from its own arithmetic, and a caller that could
//! hand this builder a poststate would be handing it the answer. Because they
//! are derived, those two conjuncts cannot fail on anything this operator
//! builds; they remain live for the on-chain route, which receives a request it
//! did not construct. This module is therefore not a witness for them, and does
//! not pretend to be.

use dclutch_claims::conservation::{
    CLAIMS_CONSERVATION_REQUEST_BYTES_V1, ClaimsConservationDirectionV1,
    ClaimsConservationRequestV1, Error as ConservationError, collateral_atoms_v1,
};
use dclutch_claims::{ClaimsAggregateSeedsV1, protocol_position_v2::ProtocolPositionSeedsV2};
use dclutch_custody::{CompartmentV1, CustodyVaultSeedsV1};
use solana_program::pubkey::Pubkey;

/// Stable refusal from conservation planning.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimsConservationOperatorErrorV1 {
    /// A coordinate the caller supplied was zero or self-aliased.
    Identity,
    /// The conservation contract refused the assembled request.
    Contract(ConservationError),
    /// `dclutch_claims` refused; the cause is its own.
    Claims(dclutch_claims::Error),
    /// `dclutch_claims` refused; the cause is its own.
    ProtocolPosition(dclutch_claims::protocol_position_v2::ProtocolPositionErrorV2),
}

/// Everything an operator must authenticate before it may plan a split or merge.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsConservationInputV1 {
    /// Release-selected Claims program, which is Custody's caller here.
    pub claims_program: Pubkey,
    /// Release-selected Custody program.
    pub custody_program: Pubkey,
    /// Canonical Core Market.
    pub market: Pubkey,
    /// Immutable selected execution release set.
    pub release_set: [u8; 32],
    /// The Market's Custody namespace, as the Claims aggregate persists it.
    pub custody_context: [u8; 32],
    /// Immutable Realm content identity.
    pub realm: [u8; 32],
    /// Realm-selected collateral mint.
    pub mint: Pubkey,
    /// Realm-selected token program.
    pub token_program: Pubkey,
    /// Digest of the finalized Product record.
    pub product_record_digest: [u8; 32],
    /// Digest of the finalized linked basis record.
    pub linked_basis_record_digest: [u8; 32],
    /// Semantic basis identity.
    pub semantic_basis_id: [u8; 32],
    /// Market generation.
    pub generation: u64,
    /// Authenticated `ProductBasisV3::payout_scale`, atoms per complete set.
    pub basis_scale: u64,
    /// Runtime complete-set width.
    pub claim_count: u32,
    /// Split or merge.
    pub direction: ClaimsConservationDirectionV1,
    /// The Position owner, who signs.
    pub owner: Pubkey,
    /// The actor's own external collateral token account.
    pub external_collateral: Pubkey,
    /// Exact complete sets created or destroyed.
    pub quantity: u64,
    /// Observed Claims aggregate revision.
    pub expected_market_revision: u64,
    /// Observed Position revision.
    pub expected_position_revision: u64,
    /// Observed Claims-role Custody replay revision.
    pub expected_custody_revision: u64,
    /// Observed external token balance.
    pub pre_external_amount: u64,
    /// Observed Hoard vault token balance.
    pub pre_hoard_amount: u64,
}

/// One validated conservation act and every address it addresses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClaimsConservationPlanV1 {
    /// The validated request.
    pub request: ClaimsConservationRequestV1,
    /// Its exact canonical bytes.
    pub bytes: [u8; CLAIMS_CONSERVATION_REQUEST_BYTES_V1],
    /// Derived Claims aggregate.
    pub aggregate: Pubkey,
    /// Derived actor Position.
    pub position: Pubkey,
    /// Derived HoardPrincipal vault.
    pub hoard_vault: Pubkey,
    /// Derived Claims-role Custody replay.
    pub custody_replay: Pubkey,
    /// Exact collateral atoms the act moves.
    pub collateral_atoms: u64,
}

/// Plan one conservative complete-set act.
///
/// # Errors
///
/// Refuses a zero or self-aliased coordinate, a seed helper's refusal, or any
/// refusal the conservation contract raises over the assembled request.
pub fn plan_claims_conservation_v1(
    input: ClaimsConservationInputV1,
) -> Result<ClaimsConservationPlanV1, ClaimsConservationOperatorErrorV1> {
    let atoms = collateral_atoms_v1(input.quantity, input.basis_scale)
        .map_err(ClaimsConservationOperatorErrorV1::Contract)?;

    let aggregate = Pubkey::find_program_address(
        &ClaimsAggregateSeedsV1::new(input.market.to_bytes())
            .map_err(ClaimsConservationOperatorErrorV1::Claims)?
            .as_slices(),
        &input.claims_program,
    )
    .0;
    let position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), input.owner.to_bytes())
            .map_err(ClaimsConservationOperatorErrorV1::ProtocolPosition)?
            .as_slices(),
        &input.claims_program,
    )
    .0;
    let hoard_vault = Pubkey::find_program_address(
        &CustodyVaultSeedsV1::new(
            input.market.to_bytes(),
            input.release_set,
            input.custody_context,
            CompartmentV1::HoardPrincipal,
        )
        .as_slices(),
        &input.custody_program,
    )
    .0;

    // The poststate is DERIVED. A split debits the actor and credits the Hoard;
    // a merge does the reverse. `checked_*` rather than saturating: an actor who
    // cannot cover the split, or a Hoard that cannot cover the merge, is a
    // refusal and never a clamped number.
    let split = matches!(input.direction, ClaimsConservationDirectionV1::Split);
    let (post_external, post_hoard) = if split {
        (
            input.pre_external_amount.checked_sub(atoms).ok_or(
                ClaimsConservationOperatorErrorV1::Contract(
                    ConservationError::ExternalBalanceMismatch,
                ),
            )?,
            input.pre_hoard_amount.checked_add(atoms).ok_or(
                ClaimsConservationOperatorErrorV1::Contract(
                    ConservationError::HoardBalanceMismatch,
                ),
            )?,
        )
    } else {
        (
            input.pre_external_amount.checked_add(atoms).ok_or(
                ClaimsConservationOperatorErrorV1::Contract(
                    ConservationError::ExternalBalanceMismatch,
                ),
            )?,
            input.pre_hoard_amount.checked_sub(atoms).ok_or(
                ClaimsConservationOperatorErrorV1::Contract(
                    ConservationError::HoardBalanceMismatch,
                ),
            )?,
        )
    };

    let request = ClaimsConservationRequestV1 {
        direction: input.direction,
        realm: input.realm,
        market: input.market.to_bytes(),
        release_set: input.release_set,
        custody_context: input.custody_context,
        aggregate: aggregate.to_bytes(),
        position: position.to_bytes(),
        owner: input.owner.to_bytes(),
        external_collateral: input.external_collateral.to_bytes(),
        hoard_vault: hoard_vault.to_bytes(),
        mint: input.mint.to_bytes(),
        token_program: input.token_program.to_bytes(),
        claims_program: input.claims_program.to_bytes(),
        product_record_digest: input.product_record_digest,
        linked_basis_record_digest: input.linked_basis_record_digest,
        semantic_basis_id: input.semantic_basis_id,
        generation: input.generation,
        quantity: input.quantity,
        basis_scale: input.basis_scale,
        collateral_atoms: atoms,
        expected_market_revision: input.expected_market_revision,
        expected_position_revision: input.expected_position_revision,
        expected_custody_revision: input.expected_custody_revision,
        pre_external_amount: input.pre_external_amount,
        post_external_amount: post_external,
        pre_hoard_amount: input.pre_hoard_amount,
        post_hoard_amount: post_hoard,
        claim_count: input.claim_count,
    };
    request
        .validate()
        .map_err(ClaimsConservationOperatorErrorV1::Contract)?;

    // The vault address is stored in a field AND recoverable from the request's
    // own coordinates. Deriving it a second time through the contract's own
    // seed helper catches this builder wiring the wrong market or context into
    // the field, which comparing the field against itself would not.
    let recovered = Pubkey::find_program_address(
        &request.hoard_vault_seeds().as_slices(),
        &input.custody_program,
    )
    .0;
    if recovered != hoard_vault {
        return Err(ClaimsConservationOperatorErrorV1::Identity);
    }
    let custody_replay = Pubkey::find_program_address(
        &request.custody_replay_seeds().as_slices(),
        &input.custody_program,
    )
    .0;

    let bytes = request
        .to_bytes()
        .map_err(ClaimsConservationOperatorErrorV1::Contract)?;
    Ok(ClaimsConservationPlanV1 {
        request,
        bytes,
        aggregate,
        position,
        hoard_vault,
        custody_replay,
        collateral_atoms: atoms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(value: u8) -> Pubkey {
        Pubkey::new_from_array([value; 32])
    }

    fn input() -> ClaimsConservationInputV1 {
        ClaimsConservationInputV1 {
            claims_program: key(0x11),
            custody_program: key(0x12),
            market: key(0x13),
            release_set: [0x14; 32],
            custody_context: [0x15; 32],
            realm: [0x16; 32],
            mint: key(0x17),
            token_program: key(0x18),
            product_record_digest: [0x19; 32],
            linked_basis_record_digest: [0x1a; 32],
            semantic_basis_id: [0x1b; 32],
            generation: 7,
            // Deliberately NOT 1. Every in-tree fixture uses a unit scale, which
            // is the exact condition under which the legacy route's set-versus-
            // atom confusion is invisible.
            basis_scale: 97,
            claim_count: 3,
            direction: ClaimsConservationDirectionV1::Split,
            owner: key(0x1c),
            external_collateral: key(0x1d),
            quantity: 5,
            expected_market_revision: 2,
            expected_position_revision: 3,
            expected_custody_revision: 4,
            pre_external_amount: 1_000,
            pre_hoard_amount: 400,
        }
    }

    #[test]
    fn a_split_moves_quantity_times_basis_scale_and_round_trips() {
        let plan = plan_claims_conservation_v1(input()).expect("split plan");
        assert_eq!(plan.collateral_atoms, 5 * 97);
        assert_eq!(plan.request.post_external_amount, 1_000 - 485);
        assert_eq!(plan.request.post_hoard_amount, 400 + 485);
        let decoded = ClaimsConservationRequestV1::decode(&plan.bytes).expect("decode");
        assert_eq!(decoded, plan.request);
        assert_eq!(decoded.validate(), Ok(()));
    }

    /// The direction is the whole difference: the same numbers, reversed.
    #[test]
    fn a_merge_returns_the_same_collateral_class_it_took() {
        let mut value = input();
        value.direction = ClaimsConservationDirectionV1::Merge;
        value.pre_hoard_amount = 1_000;
        let plan = plan_claims_conservation_v1(value).expect("merge plan");
        assert_eq!(plan.collateral_atoms, 485);
        assert_eq!(plan.request.post_external_amount, 1_000 + 485);
        assert_eq!(plan.request.post_hoard_amount, 1_000 - 485);
        assert_eq!(
            plan.request.direction.source_compartment(),
            CompartmentV1::HoardPrincipal
        );
        assert_eq!(
            plan.request.direction.destination_compartment(),
            CompartmentV1::External
        );
    }

    /// A split the actor cannot cover, and a merge the Hoard cannot cover, are
    /// refusals rather than clamped numbers.
    #[test]
    fn an_uncoverable_act_refuses_rather_than_saturating() {
        let mut value = input();
        value.pre_external_amount = 484;
        assert_eq!(
            plan_claims_conservation_v1(value),
            Err(ClaimsConservationOperatorErrorV1::Contract(
                ConservationError::ExternalBalanceMismatch
            ))
        );
        let mut value = input();
        value.direction = ClaimsConservationDirectionV1::Merge;
        value.pre_hoard_amount = 484;
        assert_eq!(
            plan_claims_conservation_v1(value),
            Err(ClaimsConservationOperatorErrorV1::Contract(
                ConservationError::HoardBalanceMismatch
            ))
        );
    }

    /// The unit-scale blindness, stated as a test rather than as a comment: at
    /// `basis_scale == 1` the collateral equals the set count, so a route that
    /// confused the two would be indistinguishable here and distinguishable at
    /// any other scale.
    #[test]
    fn a_unit_scale_hides_the_set_versus_atom_distinction_and_a_real_scale_does_not() {
        let mut unit = input();
        unit.basis_scale = 1;
        let unit = plan_claims_conservation_v1(unit).expect("unit-scale plan");
        assert_eq!(unit.collateral_atoms, unit.request.quantity);

        let scaled = plan_claims_conservation_v1(input()).expect("scaled plan");
        assert_ne!(scaled.collateral_atoms, scaled.request.quantity);
    }

    /// Zero quantity and zero scale are refused by the contract's own boundary.
    #[test]
    fn a_zero_quantity_or_scale_is_refused() {
        for (quantity, basis_scale) in [(0, 97), (5, 0)] {
            let mut value = input();
            value.quantity = quantity;
            value.basis_scale = basis_scale;
            assert_eq!(
                plan_claims_conservation_v1(value),
                Err(ClaimsConservationOperatorErrorV1::Contract(
                    ConservationError::InvalidQuantity
                ))
            );
        }
    }
}
