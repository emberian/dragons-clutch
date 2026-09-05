//! Derivation of the Rational execution descriptor from Structured terms.
//!
//! Decision 0011 §3c: under Option A, Structured authors no artifacts.  The
//! four Rational Hot bundles are already landed and are parameterized by a
//! [`RepresentationDescriptorV2`], so **the descriptor is the one genuinely new
//! host-side object Structured needs**, and this module is it.
//!
//! # Why this is a derivation and not a builder
//!
//! Every existing producer of a descriptor preimage in this tree is a test
//! module filling `exposure_id: id(11), exposure_digest: id(12), root_id:
//! id(13)` by hand -- five of them, in
//! `rational-lifecycle-hot-v3/src/{compact_operator_v4,compact_artifacts_v4,selected_bundle_v5,selected_bundle_v6}.rs`
//! and `representation-composition-v3-operator/tests/operator.rs`.  A
//! descriptor assembled that way asserts its own joins.  This function asserts
//! none of them: every field is either read out of the immutable Structured
//! terms or read out of an already-authenticated composition record, and the
//! two are joined before a byte is written.
//!
//! # The identity that is easy to get wrong
//!
//! `RepresentationDescriptorV2::graph_id()` is **not** the source graph.  The
//! Claims adapter hands it to `CompositionExposureBundleV3::decode` as
//! `RecordAdmissionV3::selected_id`
//! (`rational-representation-v2-operator/src/lib.rs:558-576`) and
//! `authenticate_exposure` requires it to equal `exposure.bundle_id()`
//! (`rational-representation-v2-kernel/src/lib.rs:902`).  The descriptor's own
//! encoder gets the name right -- [`RepresentationDescriptorInputV3::exposure_id`].
//! So it comes from [`StructuredTermsV2::shard_exposure`], and the SOURCE graph
//! identity ([`StructuredTermsV2::graph_id`]) reaches the descriptor only
//! transitively, through the exposure bundle that names it.  **That transitive
//! join is checked here**, because this is the only place in Structured's
//! lowering that reads the exposure record at all;
//! [`bind_structured_child_descriptor_v2`](crate::structured::bind_structured_child_descriptor_v2)
//! sees named coordinates and cannot check it.
//!
//! # `root_id` has no live consumer, and that is recorded rather than hidden
//!
//! The preimage requires a nonzero `root_id`, and its only reader is
//! `RepresentationDescriptorV2::authenticate_graph`, which joins the
//! **superseded** `RepresentationGraphV2` record (`DCRRGRP2`) and has zero
//! non-test callers in the tree -- `authenticate_exposure`'s own doc says the
//! exposure bundle "supersedes the legacy graph as the selected live record".
//! The same two descriptor fields are therefore double-booked: under the dead
//! path they mean the legacy graph record, under the live path they mean the
//! exposure bundle.
//!
//! This derivation supplies the canonical composition graph's root node
//! identity, which is what the only complete worked composition in the tree
//! does (`representation-composition-v3-operator/tests/operator.rs:200,239,411`
//! -- `root_id` is the graph's rank-N root node id).  It does not invent a
//! value and it does not leave the field to a caller.  **The double-booking is
//! a RECORDS-MIGRATE row, not something to resolve here**: collapsing it moves
//! every live `descriptor_id`, hence every shard Mint, custody account,
//! Position and replay record of every representation.

use dclutch_claims::rational_kernel::{
    RepresentationDescriptorV2,
    descriptor_v3::{
        RepresentationDescriptorInputV3, encode_representation_descriptor_v3_atomic,
        representation_descriptor_bytes_v3,
    },
};
use dclutch_claims::composition::{
    CompositionBundleV3, CompositionExposureBundleV3,
};
use dclutch_claims::structured_kernel::StructuredTermsV2;

use crate::structured::{Error, Result, child_request::STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2};

/// One Structured product's derived Rational execution descriptor.
///
/// This is the whole of Structured's on-chain physics under decision 0011 §3b:
/// [`descriptor_id`](Self::descriptor_id) keys the representation authority,
/// every shard Mint, every Structured custody account, every Claims custody
/// owner and the replay record.  `terms_id` keys nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructuredRepresentationDescriptorV2 {
    /// Exact finalized descriptor preimage, the bytes the Record account holds.
    pub preimage: Vec<u8>,
    /// SHA-256 of [`preimage`](Self::preimage): the descriptor's content identity.
    ///
    /// The physical adapter derives the representation authority from it as
    /// `find_program_address([RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2, this],
    /// claims_program)`.  This crate does not derive PDAs; see the crate root.
    pub descriptor_id: [u8; 32],
    /// Representation width `K`, equal to the coefficient count.
    pub outcome_count: u32,
    /// Shard atoms backing one whole native claim.
    pub denominator: u64,
}

/// Derive one Structured product's Rational execution descriptor.
///
/// `bundle` and `exposure` must already have been decoded through their own
/// admissions -- this function reads authenticated objects and never a caller's
/// assertion about them.  `bundle` supplies the canonical graph (and, through
/// `decode_composition_bundle_v3`, the graph/translation/descriptor cross-joins
/// for free); `exposure` supplies the record the live chain path actually
/// admits.
///
/// Every join it enforces, and where the chain enforces the same fact:
///
/// | joined here | re-checked on chain |
/// |---|---|
/// | `exposure.bundle_id() == terms.shard_exposure()` | `authenticate_exposure` vs `descriptor.graph_id()` |
/// | `exposure.graph_id() == terms.graph_id()` | nowhere on this route -- see the module doc |
/// | `graph.graph_id() == exposure.graph_id()` | `CompositionGraphV3::decode` vs its own descriptor |
/// | `exposure.market/result_domain/release_set == terms.*` | `LiabilityBasisMarketViewV2` join in `authenticate_common` |
/// | `exposure.representation_width() == terms.representation_width()` | `authenticate_exposure` vs `descriptor.outcome_count()` |
///
/// The width ceiling is applied here rather than at encode, so a Product too
/// wide to execute is refused before a descriptor exists to found -- see
/// [`STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2`].
pub fn derive_structured_representation_descriptor_v2(
    terms: StructuredTermsV2<'_>,
    bundle: CompositionBundleV3<'_>,
    exposure: CompositionExposureBundleV3<'_>,
) -> Result<StructuredRepresentationDescriptorV2> {
    let outcome_count = terms.representation_width();
    if outcome_count == 0 || outcome_count > STRUCTURED_CHILD_MAXIMUM_OUTCOMES_V2 {
        return Err(Error::ChildWidth);
    }

    // The record the LIVE chain path admits, and the one the descriptor names.
    if exposure.bundle_id() != terms.shard_exposure()
        || exposure.representation_width() != outcome_count
        || exposure.market() != terms.market()
        || exposure.result_domain() != terms.result_domain()
        || exposure.release_set() != terms.release_set()
    {
        return Err(Error::ChildIdentity);
    }
    // The SOURCE graph, which the descriptor never names directly. This is the
    // only place Structured's lowering can check it, so it checks it here.
    if exposure.graph_id() != terms.graph_id()
        || bundle.graph().graph_id() != terms.graph_id()
        || bundle.graph().outcome_count() != outcome_count
    {
        return Err(Error::ChildIdentity);
    }
    // Two different records with a shared name; the terms decoder already
    // proves they differ, so an equality here is a lowering mistake upstream.
    if terms.shard_exposure() == terms.graph_id() {
        return Err(Error::ChildIdentity);
    }

    let width = usize::try_from(outcome_count).map_err(|_| Error::ChildWidth)?;
    let mut coefficients = Vec::with_capacity(width);
    let mut coordinate = 0_u32;
    while coordinate < outcome_count {
        coefficients.push(terms.coefficient(coordinate).map_err(Error::Structured)?);
        coordinate = coordinate.checked_add(1).ok_or(Error::ChildWidth)?;
    }
    require_coefficients_are_the_composition_root(terms, bundle, &coefficients)?;

    let bytes =
        representation_descriptor_bytes_v3(width).map_err(Error::RationalRepresentationKernel)?;
    let mut scratch = vec![0_u8; bytes];
    let mut preimage = vec![0_u8; bytes];
    encode_representation_descriptor_v3_atomic(
        RepresentationDescriptorInputV3 {
            exposure_id: exposure.bundle_id(),
            exposure_digest: exposure.bundle_digest(),
            root_id: bundle.graph().root_id(),
            market: terms.market(),
            release_set: terms.release_set(),
            receipt_mint: terms.receipt_mint(),
            token_program: terms.token_program(),
            denominator: terms.denominator(),
            coefficients: &coefficients,
        },
        &mut scratch,
        &mut preimage,
    )
    .map_err(Error::RationalRepresentationKernel)?;

    let descriptor_id = dclutch_sha256_adapter::digest(&preimage);
    Ok(StructuredRepresentationDescriptorV2 {
        preimage,
        descriptor_id,
        outcome_count,
        denominator: terms.denominator(),
    })
}

/// Require the coefficients to BE the composition root's payoff, in lowest terms.
///
/// This restores a join the live chain path lost, and it is the reason this
/// derivation takes the composition bundle at all.
///
/// `RepresentationDescriptorV2::authenticate_graph` used to hold it -- for every
/// outcome, `coefficient * graph.scale() == graph.root_exposure(outcome) *
/// denominator` -- but it reads the superseded `RepresentationGraphV2` record
/// and has zero non-test callers.  The live route runs `authenticate_exposure`
/// instead, which checks only the bundle's identity, digest and width.  So the
/// chain admits whatever coefficients a content-addressed descriptor happens to
/// carry.
///
/// That is tolerable ONLY because the coefficients are immutable and the
/// descriptor is content-addressed: a wrong recipe is a wrong *founding*, not a
/// forgeable request.  It does mean founding is the last moment the recipe can
/// be checked against the composition it claims to represent, and under
/// decision 0011 §3b the Structured terms themselves reach no on-chain reader
/// at all (`terms_id` names no account), so nothing downstream will ever look
/// again.  It is checked here.
///
/// The comparison is the cross-multiplication `c_i * root_denominator ==
/// numerator_i * denominator`, which is exact in `u128` and scale-invariant, so
/// the Structured terms need not be in the canonical lowest form the graph
/// encoder enforces.  Root terms are sparse and strictly ascending
/// (`graph.rs:457-461`), so an outcome the root omits must carry coefficient
/// zero -- an inert coordinate, which the wire still demands a row for.
fn require_coefficients_are_the_composition_root(
    terms: StructuredTermsV2<'_>,
    bundle: CompositionBundleV3<'_>,
    coefficients: &[u64],
) -> Result<()> {
    let graph = bundle.graph();
    let root_denominator = u128::from(
        graph
            .root_denominator()
            .map_err(Error::RepresentationComposition)?,
    );
    let denominator = u128::from(terms.denominator());
    let mut numerators = vec![0_u128; coefficients.len()];
    let term_count = graph
        .root_term_count()
        .map_err(Error::RepresentationComposition)?;
    let mut index = 0_u32;
    while index < term_count {
        let term = graph
            .root_term(index)
            .map_err(Error::RepresentationComposition)?;
        let slot = usize::try_from(term.outcome).map_err(|_| Error::ChildWidth)?;
        *numerators.get_mut(slot).ok_or(Error::ChildWidth)? = u128::from(term.numerator);
        index = index.checked_add(1).ok_or(Error::ChildWidth)?;
    }
    for (coefficient, numerator) in coefficients.iter().zip(numerators.iter()) {
        let left = u128::from(*coefficient)
            .checked_mul(root_denominator)
            .ok_or(Error::ChildWidth)?;
        let right = numerator
            .checked_mul(denominator)
            .ok_or(Error::ChildWidth)?;
        if left != right {
            return Err(Error::Terms);
        }
    }
    Ok(())
}

/// Admission a physical adapter observed for a derived descriptor.
///
/// The adapter owns `find_program_address`; this crate refuses to pretend it
/// does.  So the authority arrives as an observation and is checked against
/// what the descriptor commits, exactly as
/// [`StructuredChildDescriptorV2`](crate::structured::StructuredChildDescriptorV2)'s
/// coordinates are.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredDescriptorAuthorityV2 {
    /// `find_program_address([RATIONAL_REPRESENTATION_AUTHORITY_SEED_V2,
    /// descriptor_id], claims_program).0`, observed by the adapter.
    ///
    /// It is the receipt Mint authority for `MintReceipt` AND the
    /// permissioned-burn authority for `BurnReceipt`.  Founding must configure
    /// **both** roles or `BurnReceipt` fails inside the Token program with the
    /// descriptor already committed (decision 0011 §3b).
    pub representation_authority: [u8; 32],
}

/// Decode a derived descriptor back through the kernel's hostile decoder.
///
/// This is not a round-trip convenience: it is how the derivation earns the
/// right to be called authenticated.  `RepresentationDescriptorV2::decode`
/// re-runs every reserved-byte, width, denominator and empty-recipe rule the
/// chain runs, under a `DescriptorAdmissionV2` whose four identities are the
/// recomputed digest -- which is what the Claims adapter itself constructs at
/// `rational-representation-v2-operator/src/lib.rs:533-558`.
pub fn decode_derived_structured_descriptor_v2<'a>(
    derived: &'a StructuredRepresentationDescriptorV2,
    authority: StructuredDescriptorAuthorityV2,
) -> Result<RepresentationDescriptorV2<'a>> {
    if authority.representation_authority == [0; 32]
        || authority.representation_authority == derived.descriptor_id
    {
        return Err(Error::ChildIdentity);
    }
    RepresentationDescriptorV2::decode(
        &derived.preimage,
        dclutch_claims::rational_kernel::DescriptorAdmissionV2 {
            selected_descriptor_id: derived.descriptor_id,
            finalized_descriptor_id: derived.descriptor_id,
            recomputed_descriptor_digest: derived.descriptor_id,
            finalized_descriptor_digest: derived.descriptor_id,
            record_authenticated: true,
            derived_representation_authority: authority.representation_authority,
            authority_derivation_authenticated: true,
        },
    )
    .map_err(Error::RationalRepresentationKernel)
}

/// Build the child-wire descriptor coordinates from a derived descriptor.
///
/// Nothing here is chosen: every coordinate is read out of the derivation or
/// the terms it was derived from, so a caller cannot hand-fill the field that
/// [`bind_structured_child_descriptor_v2`](crate::structured::bind_structured_child_descriptor_v2)
/// exists to check.  The bind is still run, because this constructor is a
/// convenience and the join is the authority.
pub fn structured_child_descriptor_from_derivation_v2(
    terms: StructuredTermsV2<'_>,
    derived: &StructuredRepresentationDescriptorV2,
    authority: StructuredDescriptorAuthorityV2,
) -> Result<crate::structured::StructuredChildDescriptorV2> {
    let descriptor = crate::structured::StructuredChildDescriptorV2 {
        descriptor_id: derived.descriptor_id,
        exposure_id: terms.shard_exposure(),
        representation_authority: authority.representation_authority,
        receipt_mint: terms.receipt_mint(),
        market: terms.market(),
        release_set: terms.release_set(),
        token_program: terms.token_program(),
        outcome_count: derived.outcome_count,
        denominator: derived.denominator,
    };
    crate::structured::bind_structured_child_descriptor_v2(terms, descriptor)?;
    Ok(descriptor)
}
