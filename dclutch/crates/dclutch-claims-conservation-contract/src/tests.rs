// Fixed-width fixture buffers whose bounds are `const` in this file; the same
// allowance `crates/dclutch-operator/src/general_selected_release_v1/tests.rs`
// takes. Library code carries none of it.
#![allow(clippy::indexing_slicing)]

//! Adversarial tests for the conservative complete-set contract.
//!
//! Two groups. The first proves the contract's own refusals, each asserted on
//! the exact [`Error`] variant rather than on "some error". The second executes
//! the REAL legacy kernel and the REAL Custody validator to exhibit what a bare
//! complete-set action does and does not do today.

use super::*;

use dclutch_economic_slice_kernel as slice;

const CLAIM_COUNT: u32 = 3;
const BASIS_SCALE: u64 = 11;
const QUANTITY: u64 = 7;
const COLLATERAL: u64 = 77;

const MARKET_WIDTH: usize = slice::MARKET_HEADER_BYTES + 3 * (CLAIM_COUNT as usize) * 8;
const POSITION_WIDTH: usize = slice::POSITION_HEADER_BYTES + 2 * (CLAIM_COUNT as usize) * 8;

fn id(value: u8) -> [u8; 32] {
    [value; 32]
}

fn split() -> ClaimsConservationRequestV1 {
    ClaimsConservationRequestV1 {
        direction: ClaimsConservationDirectionV1::Split,
        realm: id(1),
        market: id(2),
        release_set: id(3),
        custody_context: id(4),
        aggregate: id(5),
        position: id(6),
        owner: id(7),
        external_collateral: id(8),
        hoard_vault: id(9),
        mint: id(10),
        token_program: id(11),
        claims_program: id(12),
        product_record_digest: id(13),
        linked_basis_record_digest: id(14),
        semantic_basis_id: id(15),
        generation: 21,
        quantity: QUANTITY,
        basis_scale: BASIS_SCALE,
        collateral_atoms: COLLATERAL,
        expected_market_revision: 4,
        expected_position_revision: 2,
        expected_custody_revision: 6,
        pre_external_amount: 1_000,
        post_external_amount: 1_000 - COLLATERAL,
        pre_hoard_amount: 500,
        post_hoard_amount: 500 + COLLATERAL,
        claim_count: CLAIM_COUNT,
    }
}

fn merge() -> ClaimsConservationRequestV1 {
    ClaimsConservationRequestV1 {
        direction: ClaimsConservationDirectionV1::Merge,
        pre_external_amount: 1_000,
        post_external_amount: 1_000 + COLLATERAL,
        pre_hoard_amount: 500,
        post_hoard_amount: 500 - COLLATERAL,
        ..split()
    }
}

// ------------------------------------------------------------- the arithmetic

/// The exact founding vector, restated here so a drift in either place is a
/// red test rather than a silent divergence of two "same" rules.
///
/// `crates/dclutch-claims-svm/src/founding_v5.rs:983-990` asserts
/// `(quantity, basis_scale, collateral_transferred) == (7, 11, 77)`.
#[test]
fn collateral_is_the_founding_products_exact_integer() {
    assert_eq!(collateral_atoms_v1(QUANTITY, BASIS_SCALE), Ok(COLLATERAL));
    assert_eq!(collateral_atoms_v1(1, 1), Ok(1));
    assert_eq!(collateral_atoms_v1(u64::MAX, 1), Ok(u64::MAX));
}

#[test]
fn collateral_refuses_a_zero_side_rather_than_returning_zero() {
    assert_eq!(
        collateral_atoms_v1(0, BASIS_SCALE),
        Err(Error::InvalidQuantity)
    );
    assert_eq!(
        collateral_atoms_v1(QUANTITY, 0),
        Err(Error::InvalidQuantity)
    );
    assert_eq!(collateral_atoms_v1(0, 0), Err(Error::InvalidQuantity));
}

#[test]
fn collateral_refuses_overflow_rather_than_wrapping() {
    assert_eq!(
        collateral_atoms_v1(u64::MAX, 2),
        Err(Error::CollateralOverflow)
    );
    assert_eq!(
        collateral_atoms_v1(u64::MAX, u64::MAX),
        Err(Error::CollateralOverflow)
    );
}

// -------------------------------------------------------------- the wire

#[test]
fn both_directions_roundtrip_byte_for_byte() {
    for request in [split(), merge()] {
        let bytes = request.to_bytes().expect("encode");
        assert_eq!(bytes.len(), CLAIMS_CONSERVATION_REQUEST_BYTES_V1);
        assert_eq!(ClaimsConservationRequestV1::decode(&bytes), Ok(request));
    }
}

#[test]
fn a_short_or_long_input_is_a_different_field_not_this_one() {
    let bytes = split().to_bytes().expect("encode");
    assert_eq!(
        ClaimsConservationRequestV1::decode(&bytes[..bytes.len() - 1]),
        Err(Error::InvalidLength)
    );
    let mut long = [0_u8; CLAIMS_CONSERVATION_REQUEST_BYTES_V1 + 1];
    long[..bytes.len()].copy_from_slice(&bytes);
    assert_eq!(
        ClaimsConservationRequestV1::decode(&long),
        Err(Error::InvalidLength)
    );
}

#[test]
fn hostile_header_bytes_each_refuse_for_their_own_reason() {
    let good = split().to_bytes().expect("encode");

    let mut wrong_magic = good;
    wrong_magic[0] = b'X';
    assert_eq!(
        ClaimsConservationRequestV1::decode(&wrong_magic),
        Err(Error::InvalidMagic)
    );

    let mut wrong_version = good;
    wrong_version[VERSION_OFFSET] = 2;
    assert_eq!(
        ClaimsConservationRequestV1::decode(&wrong_version),
        Err(Error::InvalidVersion)
    );

    let mut dirty_header = good;
    dirty_header[HEADER_RESERVED_OFFSET] = 1;
    assert_eq!(
        ClaimsConservationRequestV1::decode(&dirty_header),
        Err(Error::NonCanonical)
    );

    let mut dirty_tail = good;
    dirty_tail[TAIL_RESERVED_OFFSET] = 1;
    assert_eq!(
        ClaimsConservationRequestV1::decode(&dirty_tail),
        Err(Error::NonCanonical)
    );

    let mut unknown_direction = good;
    unknown_direction[DIRECTION_OFFSET] = 2;
    assert_eq!(
        ClaimsConservationRequestV1::decode(&unknown_direction),
        Err(Error::UnknownDirection)
    );
}

#[test]
fn every_identity_coordinate_must_be_present() {
    let mut zeroed = 0_usize;
    for offset in [
        REALM_OFFSET,
        MARKET_OFFSET,
        RELEASE_SET_OFFSET,
        CUSTODY_CONTEXT_OFFSET,
        AGGREGATE_OFFSET,
        POSITION_OFFSET,
        OWNER_OFFSET,
        EXTERNAL_COLLATERAL_OFFSET,
        HOARD_VAULT_OFFSET,
        MINT_OFFSET,
        TOKEN_PROGRAM_OFFSET,
        CLAIMS_PROGRAM_OFFSET,
        PRODUCT_RECORD_DIGEST_OFFSET,
        LINKED_BASIS_RECORD_DIGEST_OFFSET,
        SEMANTIC_BASIS_ID_OFFSET,
    ] {
        let mut hostile = split().to_bytes().expect("encode");
        hostile[offset..offset + 32].fill(0);
        assert_eq!(
            ClaimsConservationRequestV1::decode(&hostile),
            Err(Error::ZeroIdentity),
            "offset {offset} was allowed to be absent"
        );
        zeroed += 1;
    }
    assert_eq!(zeroed, 15, "the identity sweep must cover every coordinate");
}

#[test]
fn the_actor_may_not_name_the_hoard_as_its_own_account() {
    let mut hostile = split();
    hostile.external_collateral = hostile.hoard_vault;
    assert_eq!(hostile.validate(), Err(Error::AliasedAccounts));
}

#[test]
fn a_stated_collateral_that_is_not_the_product_refuses() {
    for delta in [1_u64, COLLATERAL] {
        let mut hostile = split();
        hostile.collateral_atoms = COLLATERAL + delta;
        assert_eq!(hostile.validate(), Err(Error::CollateralMismatch));
    }
    let mut understated = split();
    understated.collateral_atoms = COLLATERAL - 1;
    assert_eq!(understated.validate(), Err(Error::CollateralMismatch));
}

/// The under-collateralization a bare Mint would allow, refused at the wire.
#[test]
fn a_split_that_moves_less_collateral_than_it_mints_refuses() {
    let mut hostile = split();
    hostile.post_hoard_amount = hostile.pre_hoard_amount + COLLATERAL - 1;
    assert_eq!(hostile.validate(), Err(Error::HoardBalanceMismatch));

    let mut nothing_moved = split();
    nothing_moved.post_hoard_amount = nothing_moved.pre_hoard_amount;
    assert_eq!(nothing_moved.validate(), Err(Error::HoardBalanceMismatch));

    let mut free_claims = split();
    free_claims.post_external_amount = free_claims.pre_external_amount;
    assert_eq!(free_claims.validate(), Err(Error::ExternalBalanceMismatch));
}

/// The over-payment a merge that used the kernel's set-unit payout would make.
#[test]
fn a_merge_that_returns_the_wrong_collateral_refuses() {
    let mut set_units = merge();
    // `payout` from the legacy kernel is QUANTITY, not QUANTITY * BASIS_SCALE.
    set_units.post_external_amount = set_units.pre_external_amount + QUANTITY;
    assert_eq!(
        set_units.validate(),
        Err(Error::ExternalBalanceMismatch),
        "a merge paid in complete-set units must not validate"
    );

    let mut hoard_kept = merge();
    hoard_kept.post_hoard_amount = hoard_kept.pre_hoard_amount;
    assert_eq!(hoard_kept.validate(), Err(Error::HoardBalanceMismatch));
}

#[test]
fn a_direction_swap_alone_makes_the_poststate_impossible() {
    let mut swapped = split();
    swapped.direction = ClaimsConservationDirectionV1::Merge;
    assert_eq!(swapped.validate(), Err(Error::ExternalBalanceMismatch));
}

#[test]
fn a_terminal_custody_revision_cannot_advance() {
    let mut hostile = split();
    hostile.expected_custody_revision = u64::MAX;
    assert_eq!(hostile.validate(), Err(Error::RevisionOverflow));
}

#[test]
fn a_zero_width_complete_set_refuses() {
    let mut hostile = split();
    hostile.claim_count = 0;
    assert_eq!(hostile.validate(), Err(Error::InvalidQuantity));
}

// ------------------------------------------------------- the Custody coupling

#[test]
fn split_derives_an_external_to_hoard_transfer_custody_itself_accepts() {
    let request = split().custody_request(id(40)).expect("custody request");
    assert_eq!(request.operation, OperationV1::Transfer);
    assert_eq!(request.caller_role, CallerRoleV1::Claims);
    assert_eq!(request.source_compartment, CompartmentV1::External);
    assert_eq!(
        request.destination_compartment,
        CompartmentV1::HoardPrincipal
    );
    assert_eq!(request.source, id(8));
    assert_eq!(request.destination, id(9));
    assert_eq!(request.amount, COLLATERAL);
    assert_eq!(request.context, id(4));
    assert_eq!(request.semantic.source_owner, id(7));
    assert_eq!(request.semantic.destination_owner, [0; 32]);
    assert_eq!(request.source_vault_context, [0; 32]);
    assert_eq!(request.destination_vault_context, id(4));
    assert_eq!(request.semantic.parent_request_digest, id(40));
    assert_eq!(request.expected_revision, 6);
    assert_eq!(request.resulting_revision, 7);
    // Custody's own validator, not a restatement of it.
    assert_eq!(request.validate(), Ok(()));
}

#[test]
fn merge_derives_a_hoard_to_external_transfer_custody_itself_accepts() {
    let request = merge().custody_request(id(41)).expect("custody request");
    assert_eq!(request.source_compartment, CompartmentV1::HoardPrincipal);
    assert_eq!(request.destination_compartment, CompartmentV1::External);
    assert_eq!(request.source, id(9));
    assert_eq!(request.destination, id(8));
    assert_eq!(request.source_vault_context, id(4));
    assert_eq!(request.destination_vault_context, [0; 32]);
    assert_eq!(request.semantic.source_owner, [0; 32]);
    assert_eq!(request.semantic.destination_owner, id(7));
    assert_eq!(request.amount, COLLATERAL);
    assert_eq!(request.validate(), Ok(()));
}

/// The exact predicate that makes a split inadmissible on Custody's V1 wire.
///
/// `programs/dclutch-custody-sbf/src/lib.rs:1389` is
/// `if request.source_compartment == CompartmentV1::External { return
/// Err(CustodySbfError::Instruction.into()); }`, and this asserts the derived
/// split satisfies it while the derived merge does not. That is a statement
/// about the shape this crate produces; it is not a claim that the SBF program
/// was executed here.
#[test]
fn only_merge_is_admissible_on_the_v1_transfer_wire() {
    let split_request = split().custody_request(id(40)).expect("split");
    let merge_request = merge().custody_request(id(41)).expect("merge");
    assert_eq!(split_request.source_compartment, CompartmentV1::External);
    assert_ne!(merge_request.source_compartment, CompartmentV1::External);
}

#[test]
fn a_split_is_one_atomic_total_debit_with_the_delegation_fully_revoked() {
    let authority = id(50);
    let request = split()
        .delegated_custody_request(id(40), authority)
        .expect("delegated request");
    assert_eq!(request.total_debit, COLLATERAL);
    assert_eq!(request.allowance_before, COLLATERAL);
    assert_eq!(request.allowance_after, 0);
    assert!(request.starts_atomic_debit);
    assert!(request.terminal);
    assert_eq!(request.delegate_before, authority);
    assert_eq!(
        request.delegate_after, [0; 32],
        "a residual delegate is exactly the hidden authority the V2 wire forbids"
    );
    assert_eq!(request.custody.amount, COLLATERAL);
    // Custody's own successor validator.
    assert_eq!(request.validate(), Ok(()));
}

#[test]
fn a_merge_may_not_be_built_on_the_delegated_wire() {
    assert_eq!(
        merge().delegated_custody_request(id(40), id(50)),
        Err(Error::WrongDirectionWire)
    );
}

/// The three Custody shape rules this contract's construction is threading.
///
/// `custody_request` runs Custody's own `validate` on its output, and that call
/// is currently unreachable-failing: nothing this crate can build violates it.
/// So the guard is pinned from the other side instead — each rule is broken by
/// hand on a request we DID produce, and Custody's exact discriminant asserted.
/// If Custody's operation-shape table moves, this goes red.
#[test]
fn custody_itself_refuses_the_shapes_this_contract_is_careful_not_to_build() {
    use dclutch_custody_contract::Error as CustodyError;

    let good = split().custody_request(id(40)).expect("split");
    assert_eq!(good.validate(), Ok(()));

    let mut vault_context_on_external = good;
    vault_context_on_external.source_vault_context = id(4);
    assert_eq!(
        vault_context_on_external.validate(),
        Err(CustodyError::InvalidOperationShape),
        "an External source is not Custody-owned and may not name a vault context"
    );

    let mut anonymous_external = good;
    anonymous_external.semantic.source_owner = [0; 32];
    assert_eq!(
        anonymous_external.validate(),
        Err(CustodyError::ExternalOwnerMismatch),
        "an External source must name the actor whose tokens are being debited"
    );

    let mut aliased = good;
    aliased.destination = aliased.source;
    assert_eq!(
        aliased.validate(),
        Err(CustodyError::AliasedTransferAccounts)
    );
}

#[test]
fn a_custody_request_needs_a_real_parent_digest_and_a_real_delegate() {
    assert_eq!(split().custody_request([0; 32]), Err(Error::ZeroIdentity));
    assert_eq!(
        split().delegated_custody_request(id(40), [0; 32]),
        Err(Error::ZeroIdentity)
    );
}

// ------------------------------------------------------------- the replay

/// The replay decision, pinned so that changing it is a red test.
#[test]
fn the_replay_is_the_claims_role_cursor_over_the_markets_custody_namespace() {
    let request = split();
    let seeds = request.custody_replay_seeds();
    assert_eq!(
        seeds,
        CustodyReplaySeedsV1::new(id(2), id(3), CallerRoleV1::Claims, id(4))
    );
    // The same coordinate the derived Custody request projects: one cursor,
    // not two.
    assert_eq!(
        seeds,
        CustodyReplaySeedsV1::from_request(request.custody_request(id(40)).expect("custody"))
    );
    let slices = seeds.as_slices();
    assert_eq!(slices.len(), 5);
    assert_eq!(slices[0], b"dclutch:custody-replay:v1");
    assert_eq!(slices[1], id(2));
    assert_eq!(slices[2], id(3));
    assert_eq!(slices[3], &[CallerRoleV1::Claims as u8]);
    assert_eq!(
        slices[4],
        id(4),
        "the namespace is the aggregate's custody_context, never the Market address"
    );
    assert_ne!(slices[4], slices[1]);
}

/// A merge and a split in one Market share the cursor, and so does the terminal
/// payout path: they are one role's ordered effects on one pool.
#[test]
fn split_and_merge_do_not_fork_the_cursor() {
    assert_eq!(
        split().custody_replay_seeds(),
        merge().custody_replay_seeds()
    );
}

#[test]
fn the_hoard_is_the_hoard_principal_compartment_of_the_same_namespace() {
    let seeds = split().hoard_vault_seeds();
    let slices = seeds.as_slices();
    assert_eq!(slices.len(), 5);
    assert_eq!(slices[0], b"dclutch:custody-vault:v1");
    assert_eq!(slices[1], id(2));
    assert_eq!(slices[2], id(3));
    assert_eq!(slices[3], id(4));
    assert_eq!(
        slices[4],
        &[CompartmentV1::HoardPrincipal.tag()],
        "Hoard principal is never a fee, liveness, reserve or trading compartment"
    );
}

// ---------------------------------------------------------- uniform deltas

#[test]
fn the_complete_set_moves_uniformly_or_not_at_all() {
    let request = split();
    let mut vector = [0_u8; (CLAIM_COUNT as usize) * 8];
    request
        .write_uniform_quantities(&mut vector)
        .expect("uniform vector");
    for index in 0..CLAIM_COUNT as usize {
        let mut scalar = [0_u8; 8];
        scalar.copy_from_slice(&vector[index * 8..index * 8 + 8]);
        assert_eq!(u64::from_le_bytes(scalar), QUANTITY);
    }
    assert!(request.direction.claim_delta_is_credit());
    assert!(!merge().direction.claim_delta_is_credit());
}

#[test]
fn a_quantity_vector_of_the_wrong_width_refuses() {
    let request = split();
    let mut short = [0_u8; (CLAIM_COUNT as usize) * 8 - 1];
    assert_eq!(
        request.write_uniform_quantities(&mut short),
        Err(Error::WidthMismatch)
    );
    let mut long = [0_u8; (CLAIM_COUNT as usize) * 8 + 8];
    assert_eq!(
        request.write_uniform_quantities(&mut long),
        Err(Error::WidthMismatch)
    );
}

// ------------------------------------------------------------- capacity

#[test]
fn a_split_is_admitted_at_the_cap_and_refused_past_it() {
    let request = split();
    assert_eq!(request.admit_capacity(3, 10), Ok(()));
    assert_eq!(
        request.admit_capacity(4, 10),
        Err(Error::PrincipalCapacity),
        "7 new sets on top of 4 outstanding exceeds a 10-set cap"
    );
    assert_eq!(
        request.admit_capacity(u64::MAX, u64::MAX),
        Err(Error::PrincipalCapacity),
        "no cap admits a set count the u64 wire cannot carry"
    );
    assert_eq!(
        request.admit_capacity(0, 0),
        Err(Error::PrincipalCapacity),
        "an unstated cap is a refusal, not an unbounded one"
    );
}

#[test]
fn a_merge_never_needs_growth_capacity() {
    assert_eq!(merge().admit_capacity(0, 0), Ok(()));
}

#[test]
fn the_conserved_hoard_scalar_tracks_the_direction() {
    assert_eq!(split().conserved_hoard_sets_after(3), Ok(10));
    assert_eq!(merge().conserved_hoard_sets_after(10), Ok(3));
    assert_eq!(
        merge().conserved_hoard_sets_after(6),
        Err(Error::CollateralOverflow),
        "a merge cannot destroy more sets than are outstanding"
    );
}

// ============================================================================
// The legacy kernel, executed. These are not restatements: they run
// `dclutch_economic_slice_kernel` and report what it actually does.
// ============================================================================

fn open_market() -> [u8; MARKET_WIDTH] {
    let mut bytes = [0_u8; MARKET_WIDTH];
    slice::initialize_market(
        &mut bytes,
        id(2),
        id(3),
        id(30),
        CLAIM_COUNT,
        slice::Phase::Open,
        0,
    )
    .expect("market");
    bytes
}

fn empty_position() -> [u8; POSITION_WIDTH] {
    let mut bytes = [0_u8; POSITION_WIDTH];
    slice::initialize_position(&mut bytes, id(2), id(7), CLAIM_COUNT).expect("position");
    bytes
}

fn uniform(quantity: u64) -> [u8; (CLAIM_COUNT as usize) * 8] {
    let mut vector = [0_u8; (CLAIM_COUNT as usize) * 8];
    let encoded = quantity.to_le_bytes();
    for index in 0..CLAIM_COUNT as usize {
        vector[index * 8..index * 8 + 8].copy_from_slice(&encoded);
    }
    vector
}

/// **The hole, executed.**
///
/// A bare `MintCompleteSet` credits the aggregate's Hoard scalar by the full
/// complete-set count and returns `Payout { amount: 0 }`. There is no collateral
/// coordinate anywhere in the call, so an adapter driving this action has
/// nothing to transfer and nothing to refuse: claims exist against a Hoard that
/// received no atoms. `dclutch-claims-sbf` declares a refusal for exactly this
/// -- `ClaimsSbfError::CustodyRequired = 0x5006`, "This action requires the
/// canonical Custody child composition" -- and raises it nowhere.
#[test]
fn mint_complete_set_grows_the_hoard_count_and_moves_no_collateral() {
    let mut market = open_market();
    let mut position = empty_position();
    let quantities = uniform(QUANTITY);

    assert_eq!(slice::market_hoard(&market), Ok(0));
    let payout = slice::execute_basket(
        &mut market,
        None,
        Some(&mut position),
        slice::BasketFrame {
            expected_market_revision: 0,
            expected_source_revision: None,
            expected_destination_revision: Some(0),
            action: slice::BasketAction::MintCompleteSet,
            quantities: &quantities,
            quantity_multiplier: 1,
        },
    )
    .expect("mint");

    assert_eq!(
        slice::market_hoard(&market),
        Ok(QUANTITY),
        "the Hoard scalar grew by the full complete-set count"
    );
    assert_eq!(
        payout,
        slice::Payout {
            recipient: [0; 32],
            amount: 0
        },
        "and the kernel demands not one collateral atom for it"
    );

    // What the conservation contract requires instead, for the same act.
    let request = ClaimsConservationRequestV1 {
        pre_hoard_amount: 0,
        post_hoard_amount: COLLATERAL,
        ..split()
    };
    assert_eq!(
        request.conserved_hoard_sets_after(0),
        Ok(QUANTITY),
        "the same aggregate poststate"
    );
    assert_eq!(
        request.custody_request(id(40)).expect("custody").amount,
        COLLATERAL,
        "with 77 atoms actually moved into the Hoard, not zero"
    );
}

/// **The second defect, executed.**
///
/// `MergeCompleteSet` DOES return a payout -- but its magnitude is
/// `complete_quantity`, i.e. complete-set units, while a Custody transfer moves
/// collateral atoms. An adapter that paid `Payout::amount` would return 7 atoms
/// where 77 are owed. The unit error is invisible whenever `basis_scale == 1`,
/// which is what every in-tree fixture uses.
#[test]
fn merge_complete_set_reports_a_payout_in_set_units_not_collateral_atoms() {
    let mut market = open_market();
    let mut position = empty_position();
    let quantities = uniform(QUANTITY);

    slice::execute_basket(
        &mut market,
        None,
        Some(&mut position),
        slice::BasketFrame {
            expected_market_revision: 0,
            expected_source_revision: None,
            expected_destination_revision: Some(0),
            action: slice::BasketAction::MintCompleteSet,
            quantities: &quantities,
            quantity_multiplier: 1,
        },
    )
    .expect("mint");

    let payout = slice::execute_basket(
        &mut market,
        Some(&mut position),
        None,
        slice::BasketFrame {
            expected_market_revision: 1,
            expected_source_revision: Some(1),
            expected_destination_revision: None,
            action: slice::BasketAction::MergeCompleteSet,
            quantities: &quantities,
            quantity_multiplier: 1,
        },
    )
    .expect("merge");

    assert_eq!(slice::market_hoard(&market), Ok(0));
    assert_eq!(
        payout.amount, QUANTITY,
        "the kernel's payout is the SET count"
    );
    assert_ne!(
        payout.amount, COLLATERAL,
        "which is not the collateral the merger is owed"
    );
    assert_eq!(
        merge().custody_request(id(41)).expect("custody").amount,
        COLLATERAL,
        "the conservation contract moves atoms, not sets"
    );
}

/// The complete-set equality this contract writes is the one the kernel itself
/// enforces, and a non-uniform vector is refused by the real kernel.
#[test]
fn the_kernel_accepts_this_contracts_uniform_vector_and_refuses_a_skewed_one() {
    let mut market = open_market();
    let mut position = empty_position();
    let mut quantities = [0_u8; (CLAIM_COUNT as usize) * 8];
    split()
        .write_uniform_quantities(&mut quantities)
        .expect("uniform");

    assert!(
        slice::execute_basket(
            &mut market,
            None,
            Some(&mut position),
            slice::BasketFrame {
                expected_market_revision: 0,
                expected_source_revision: None,
                expected_destination_revision: Some(0),
                action: slice::BasketAction::MintCompleteSet,
                quantities: &quantities,
                quantity_multiplier: 1,
            },
        )
        .is_ok()
    );

    let mut skewed = quantities;
    skewed[8..16].copy_from_slice(&(QUANTITY + 1).to_le_bytes());
    let mut fresh_market = open_market();
    let mut fresh_position = empty_position();
    assert_eq!(
        slice::execute_basket(
            &mut fresh_market,
            None,
            Some(&mut fresh_position),
            slice::BasketFrame {
                expected_market_revision: 0,
                expected_source_revision: None,
                expected_destination_revision: Some(0),
                action: slice::BasketAction::MintCompleteSet,
                quantities: &skewed,
                quantity_multiplier: 1,
            },
        ),
        Err(slice::Error::CandidateInvariantFailure)
    );
}
