//! The founding arm of a child market: prove the parent reference against the
//! parents themselves, and the child's own records against the reference.
//!
//! A child market (`docs/design/MECHANISM_CONDITIONAL_MARKETS_2026_09_04.md`;
//! `DClutchSemantics.ConditionalMarketV1`) names two parent Markets at
//! founding and observes nothing itself. What its founding has to establish,
//! once, is that the reference it will settle against describes real parents
//! in a state a child may bind, and that the child's own cells are the joint
//! index the reference implies. Everything else -- which certificate a parent
//! wrote, what the child's selector is -- is the derived provider's, at
//! settlement, and reads the reference this arm proved.
//!
//! Every conjunct refuses by its own name (`CoreSbfError::Parent*`); the order
//! is the Lean's (`ProductShape.found?` / `ConditionalShape.found?`) after the
//! physical checks that precede it.

use dclutch_market::{CoreState, MarketCoreStateSeedsV2, Phase};
use dclutch_product::ResultDomainV2;
use dclutch_product::admission::{
    PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_product::payoff::runtime_v3::{BasisKindV3, ProductBasisV3};
use dclutch_product::svm_reader::AuthenticatedProductRuntimeV2;
use dclutch_source::parent_reference_v1::{
    ChildShapeV1, PARENT_REFERENCE_BYTES_V1, PARENT_REFERENCE_SCHEMA_ID_V1, ParentRefV1,
    ParentReferenceV1, ParentRefusalV1,
};
use dclutch_source::{SourceAccessProfile, SourceSpecV1};
use solana_program::pubkey::Pubkey;

use crate::{
    CoreSbfError,
    frame::{FoundAccounts, ParentFoundSlotsV3, ParentFoundTailV3},
    records::authenticate_content_addressed_record,
};

/// Map the Lean's founding refusals onto Core's band. The six settlement
/// refusals cannot arise here and fold to `ParentReference`, which is stated
/// rather than assumed: a founding that somehow produced one has a reference
/// it cannot describe.
const fn founding_refusal(refusal: ParentRefusalV1) -> CoreSbfError {
    match refusal {
        ParentRefusalV1::SameParent => CoreSbfError::ParentSame,
        ParentRefusalV1::EmptyParent => CoreSbfError::ParentEmpty,
        ParentRefusalV1::ConditionOnFailure => CoreSbfError::ParentConditionOnFailure,
        ParentRefusalV1::ConditionOutOfRange => CoreSbfError::ParentConditionOutOfRange,
        ParentRefusalV1::WidthOverflow => CoreSbfError::ParentWidthOverflow,
        ParentRefusalV1::ParentNotTerminal
        | ParentRefusalV1::WrongParent
        | ParentRefusalV1::ParentGenerationMismatch
        | ParentRefusalV1::ParentRecordMismatch
        | ParentRefusalV1::ParentWidthMismatch
        | ParentRefusalV1::SelectorOutOfRange => CoreSbfError::ParentReference,
    }
}

/// The child's founding arm, or the proof that this founding is not a child.
///
/// `source_spec` is the child's authenticated `SourceSpecV1`; its access
/// profile decides which of the two this is, and the frame must agree:
///
/// * profile `DerivedFromParents` with no tail, or a tail with another
///   profile, refuses `ParentReference` -- a child without parents and parents
///   without a child are the same defect seen from two sides;
/// * neither, and this returns `Ok(None)`: an ordinary founding.
#[inline(never)]
pub(crate) fn authenticate_parent_reference_v1(
    frame: &FoundAccounts<'_, '_>,
    registry: &Pubkey,
    source_spec: SourceSpecV1,
    runtime: &AuthenticatedProductRuntimeV2,
) -> Result<Option<ChildShapeV1>, CoreSbfError> {
    let derived = source_spec.access_profile() == SourceAccessProfile::DerivedFromParents;
    let tail = match (derived, frame.parents) {
        (false, None) => return Ok(None),
        (true, Some(tail)) => tail,
        _ => return Err(CoreSbfError::ParentReference),
    };
    let reference = authenticate_reference_record(registry, tail, source_spec)?;
    let core_program = frame.core_program.key;
    if reference.parent_a.market == frame.market.key.to_bytes()
        || reference.parent_b.market == frame.market.key.to_bytes()
    {
        return Err(CoreSbfError::ParentSame);
    }
    authenticate_parent(core_program, tail.a, reference.parent_a)?;
    authenticate_parent(core_program, tail.b, reference.parent_b)?;
    let shape = reference.admit_founding().map_err(founding_refusal)?;
    authenticate_child_records(frame, runtime, shape)?;
    Ok(Some(shape))
}

/// The reference record: Registry-finalized under its schema, and the exact
/// record the child's own Source spec names as its adapter configuration.
#[inline(never)]
fn authenticate_reference_record(
    registry: &Pubkey,
    tail: ParentFoundTailV3<'_, '_>,
    source_spec: SourceSpecV1,
) -> Result<ParentReferenceV1, CoreSbfError> {
    let data = tail
        .reference_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    if data.len() != PARENT_REFERENCE_BYTES_V1 {
        return Err(CoreSbfError::ParentReference);
    }
    let (digest, bytes, _) = authenticate_content_addressed_record(
        registry,
        tail.reference_raw,
        tail.reference_staging,
        PARENT_REFERENCE_SCHEMA_ID_V1,
        &data,
    )?;
    if digest != source_spec.adapter_config_id().to_bytes() {
        return Err(CoreSbfError::ParentReference);
    }
    ParentReferenceV1::decode(bytes).map_err(|_| CoreSbfError::ParentReference)
}

/// One parent: a Core Market of this program at its own derived address, Open
/// or Terminal, at the referenced generation and Product record; and its
/// `ResultDomainV2` -- reached through its Product record, both finalized under
/// the parent's own Registry -- with exactly the referenced ordinary count.
#[inline(never)]
fn authenticate_parent(
    core_program: &Pubkey,
    slots: ParentFoundSlotsV3<'_, '_>,
    reference: ParentRefV1,
) -> Result<(), CoreSbfError> {
    if slots.market.key.to_bytes() != reference.market || slots.market.owner != core_program {
        return Err(CoreSbfError::ParentBinding);
    }
    let market_data = slots
        .market
        .try_borrow_data()
        .map_err(|_| CoreSbfError::ParentBinding)?;
    let state = CoreState::decode(&market_data).map_err(|_| CoreSbfError::ParentBinding)?;
    if Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(state.identity).as_slices(),
        core_program,
    )
    .0 != *slots.market.key
    {
        return Err(CoreSbfError::ParentBinding);
    }
    // Open or Terminal, never Founding (its Product record is not yet
    // authenticated by a completed founding) and never Retiring/Retired (its
    // certificate account may be reclaimed before the child reads it; the
    // parent-side reference count that would block retirement was refused as
    // a foreign write into the parent's lifecycle, design §4.4).
    if !matches!(state.phase, Phase::Open | Phase::Terminal) {
        return Err(CoreSbfError::ParentPhase);
    }
    if state.identity.generation != reference.generation
        || state.identity.product_record.to_bytes() != reference.product_record_digest
    {
        return Err(CoreSbfError::ParentBinding);
    }
    let parent_registry = Pubkey::new_from_array(state.identity.registry_program.to_bytes());
    drop(market_data);

    let product_data = slots
        .product_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let (product_digest, product_bytes, _) = authenticate_content_addressed_record(
        &parent_registry,
        slots.product_raw,
        slots.product_staging,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        &product_data,
    )?;
    if product_digest != reference.product_record_digest {
        return Err(CoreSbfError::ParentBinding);
    }
    let product =
        ProductRecordV2::decode(product_bytes).map_err(|_| CoreSbfError::ParentBinding)?;
    let domain_digest = product.result_domain_digest();
    drop(product_data);

    let domain_data = slots
        .result_domain_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let (digest, domain_bytes, _) = authenticate_content_addressed_record(
        &parent_registry,
        slots.result_domain_raw,
        slots.result_domain_staging,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        &domain_data,
    )?;
    if digest != domain_digest.to_bytes() {
        return Err(CoreSbfError::ParentBinding);
    }
    let domain = ResultDomainV2::decode(domain_bytes).map_err(|_| CoreSbfError::ParentBinding)?;
    // The one number the reference carries that the parent's Market does not:
    // proved here, so the child's cells are the parent's regions and not the
    // founder's opinion of them.
    if domain.region_count() != reference.ordinary_count {
        return Err(CoreSbfError::ParentBinding);
    }
    Ok(())
}

/// The child's own domain and basis are the shape the reference implies: cuts
/// `[1, …, n − 1]` over denominator one (`ParentReferenceV1Abi.childCuts`), a
/// categorical basis of width `n + 1` refunding at scale `n`
/// (`categoricalRefundsOnFailure`), and the runtime's outcome count is `n + 1`.
#[inline(never)]
fn authenticate_child_records(
    frame: &FoundAccounts<'_, '_>,
    runtime: &AuthenticatedProductRuntimeV2,
    shape: ChildShapeV1,
) -> Result<(), CoreSbfError> {
    let ordinary = shape
        .ordinary_count()
        .map_err(|_| CoreSbfError::ParentWidthOverflow)?;
    let width = shape
        .width()
        .map_err(|_| CoreSbfError::ParentWidthOverflow)?;
    if runtime.outcome_count != width {
        return Err(CoreSbfError::ParentReference);
    }
    let domain_data = frame
        .result_domain_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let domain = ResultDomainV2::decode(&domain_data).map_err(|_| CoreSbfError::Reference)?;
    if domain.region_count() != ordinary || domain.cut_denominator() != 1 {
        return Err(CoreSbfError::ParentReference);
    }
    let mut expected: i128 = 1;
    for cut in domain.cuts() {
        if cut != expected {
            return Err(CoreSbfError::ParentReference);
        }
        expected = expected.checked_add(1).ok_or(CoreSbfError::Arithmetic)?;
    }
    drop(domain_data);

    let basis_data = frame
        .linked_basis_raw
        .try_borrow_data()
        .map_err(|_| CoreSbfError::FinalizedRecord)?;
    let basis = ProductBasisV3::decode(&basis_data).map_err(|_| CoreSbfError::Reference)?;
    let scale = shape
        .payout_scale()
        .map_err(|_| CoreSbfError::ParentWidthOverflow)?;
    if basis.kind() != BasisKindV3::CategoricalQ1
        || basis.basis_width() != width
        || basis.payout_scale() != scale
        || !basis.refunds_on_failure()
    {
        // A child that does not refund on its failure coordinate would pay
        // its founder on a parent's outage -- decision 0025 one level up.
        return Err(CoreSbfError::ParentReference);
    }
    Ok(())
}
