//! Independent SVM authentication and decoding for Product Runtime graphs.
//!
//! The exact Product-record digest is the graph root. The Product body selects
//! the domain and portfolio digests; neither an admission receipt nor a caller
//! may select those children independently. A receipt can be rechecked after
//! authentication as a coordinate cache, but it is never authority. The V3
//! frame additionally authenticates one canonical ProductBasisV3 raw/staging
//! pair. Product owns only its semantic liability-basis identity: the reader
//! authenticates and returns the independently finalized raw coordinate rather
//! than adding a raw-record digest to ProductRecordV2.
//!
//! That convergence is done for every live Claims consumer. `affine_batch_v2`
//! now consumes [`authenticate_product_basis_v3`] through the single shared
//! `authenticate_runtime_product_basis_core_v3` boundary, which is the basis
//! authority for all four live routes — `founding_v5`, `affine_batch_v2`,
//! `signed_delta_v3` and `protocol_position_v2` — so Claims founding
//! authenticates exactly the Registry-owned record Core commits into a
//! founding permit. `rational_representation_v2` reaches the same authority
//! through `rational_product_v3`, and `sparse_native_transfer_v1` and
//! `terminal_settlement_v3` already called this reader directly. The
//! superseded Core-owned `LinkedBasisRecordV2` expectation was deleted from
//! each of those paths in the same cycle; no parallel decode fallback remains.
//!
//! There are no legacy consumers left. The one that was -- the `DCLLBX02`
//! route in `dclutch-claims-sbf::liability_basis_v2`, which still expected a
//! Core-owned `LinkedBasisRecordV2` -- was deleted rather than converged, and
//! its module is now just the shared LBV2 state vocabulary.
//! `LinkedBasisRecordV2` no longer exists in Rust at all: the last model of it
//! was `dclutch-liability-basis-v2-kernel::product_claims`, which was deleted
//! once it was the only thing keeping a retired record family alive. `DCLTLNK2`
//! now appears in this tree solely as prose.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

use crate::admission::{
    AdmissionReceiptV2, FinalizedRecordCoordinateV2, PORTFOLIO_SCHEMA_ID_V2,
    PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2, RESULT_DOMAIN_SCHEMA_ID_V2,
    admit_authenticated_views_v2,
};
use crate::payoff::price_gate_v1::verify_price_gate_v1;
use crate::payoff::registry_v3::PRICE_GATE_RECORD_SCHEMA_ID_V1;
pub use crate::payoff::runtime_v3::BASIS_WIDTH_OFFSET_V3;
use crate::payoff::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisKindV3, Error as BasisError, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
    },
};
use crate::{ContentId, PortfolioV2, ResultDomainV2};
use dclutch_market::capability_manifest::funding::funded_rent_persists_v1;
use dclutch_registry::record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    pubkey::Pubkey,
};
use solana_sdk_ids::system_program;

/// Product Runtime V2 SVM-reader refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Raw/staging accounts aliased or carried forbidden privileges.
    AccountFrame,
    /// Product record owner, PDA, digest, rent, or staging vacancy refused.
    ProductRecord,
    /// Result-domain record owner, PDA, digest, rent, or staging vacancy refused.
    ResultDomainRecord,
    /// Portfolio record owner, PDA, digest, rent, or staging vacancy refused.
    PortfolioRecord,
    /// ProductBasisV3 record owner, PDA, digest, rent, or staging vacancy refused.
    LinkedBasisRecord,
    /// ProductBasisV3 decoding, semantic identity, or Product/domain links refused.
    LinkedBasisComposition,
    /// Product/domain/portfolio decoding or exact identity composition refused.
    Composition,
    /// Rational representation descriptor record authentication refused.
    RepresentationDescriptorRecord,
    /// Rational representation graph record authentication refused.
    RepresentationGraphRecord,
    /// Rational representation descriptor, graph, or Product join refused.
    RepresentationComposition,
    /// Optional receipt coordinates differed from independently authenticated facts.
    ReceiptMismatch,
    /// Account data could not be borrowed.
    Borrow,
    /// A basis declaring degree >= 2 was founded with no price-gate
    /// certificate account offered.
    ///
    /// Degree <= 1 is exempt from the gate **by proof**; above it the simplex
    /// condition stops being the no-arbitrage condition, so founding without a
    /// certificate would admit an executable arbitrage.
    PriceGateRequired,
    /// The certificate account offered was not the one the authenticated basis
    /// record names.
    ///
    /// The digest is read off the basis, never off the caller, so this covers
    /// a wrong account, a byte-identical certificate at a non-canonical
    /// address, a Registry-unowned account, and a certificate below rent
    /// exemption for its exact width.
    PriceGateBasisMismatch,
    /// **The hull identity failed.** `price * mass != sum(weight * payout)` at
    /// some claim, with every payout recomputed through the production
    /// evaluator rather than read from the certificate.
    PriceGateHullRefused,
    /// The certificate carried no hull atoms, or more than the
    /// affine-Caratheodory capacity of ten permits.
    PriceGateCapacity,
    /// The certificate's **body** was non-canonical: padding past a declared
    /// width, coordinates not strictly increasing, a zero atom weight, a
    /// non-primitive weight scale, prices not partitioning the scale, or an
    /// unimplemented profile.
    ///
    /// Distinct from [`Error::PriceGateBasisMismatch`], which is about the
    /// certificate's *address and authenticity* -- wrong PDA, wrong owner,
    /// writable, or below rent exemption. One says the record is not the one
    /// the basis names; this one says the record is malformed.
    PriceGateNonCanonical,
}

/// Reader result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// One read-only finalized raw/staging account pair.
#[derive(Clone, Copy)]
pub struct FinalizedRecordFrameV2<'accounts, 'info> {
    /// Registry-owned exact raw body.
    pub raw: &'accounts AccountInfo<'info>,
    /// System-owned vacant staging cursor PDA.
    pub staging: &'accounts AccountInfo<'info>,
}

/// Exact Product/domain/portfolio read-only account frame.
#[derive(Clone, Copy)]
pub struct ProductRuntimeFrameV2<'accounts, 'info> {
    /// Product graph-root record.
    pub product: FinalizedRecordFrameV2<'accounts, 'info>,
    /// Product-selected result domain.
    pub result_domain: FinalizedRecordFrameV2<'accounts, 'info>,
    /// Product-selected exact rational portfolio.
    pub portfolio: FinalizedRecordFrameV2<'accounts, 'info>,
}

/// Exact Product/domain/portfolio/ProductBasisV3 read-only account frame.
///
/// The basis schema is fixed to
/// [`GRADED_BASIS_RECORD_SCHEMA_ID_V3`]; neither a caller nor family action
/// selects an alternate decoder. Both categorical-Q1 and graded exact-
/// complement kinds are admitted by the same canonical ProductBasisV3 decoder.
#[derive(Clone, Copy)]
pub struct ProductRuntimeFrameV3<'accounts, 'info> {
    /// Product graph-root record.
    pub product: FinalizedRecordFrameV2<'accounts, 'info>,
    /// Product-selected result domain.
    pub result_domain: FinalizedRecordFrameV2<'accounts, 'info>,
    /// Product-selected exact rational portfolio.
    pub portfolio: FinalizedRecordFrameV2<'accounts, 'info>,
    /// Independently finalized Product-linked ProductBasisV3 record.
    pub linked_basis: FinalizedRecordFrameV2<'accounts, 'info>,
}

/// One independently authenticated finalized coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedRecordV2 {
    /// Canonical record schema.
    pub schema_id: ContentId,
    /// SHA-256 digest of the complete raw body.
    pub content_digest: ContentId,
    /// Canonical Registry raw PDA.
    pub raw_account: Pubkey,
    /// Canonical vacant staging PDA.
    pub staging_account: Pubkey,
}

impl AuthenticatedRecordV2 {
    /// Project the exact finalized coordinate after authentication. This is a
    /// reference cache, not a substitute for repeating authentication.
    pub fn coordinate(self) -> Result<FinalizedRecordCoordinateV2> {
        Ok(FinalizedRecordCoordinateV2 {
            schema_id: self.schema_id,
            content_digest: self.content_digest,
            raw_account: content(self.raw_account.to_bytes())?,
            staging_account: content(self.staging_account.to_bytes())?,
        })
    }
}

/// Ephemeral, independently authenticated Product Runtime V2 projection.
///
/// These fixed-size fields are copied from decoded authenticated raw bodies,
/// never from receipt bytes. Persisted Product facts remain owned by those raw
/// records and every consumer repeats authentication and decoding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthenticatedProductRuntimeV2 {
    /// Authenticated Product graph-root record.
    pub product_record: AuthenticatedRecordV2,
    /// Authenticated Product-selected domain record.
    pub result_domain_record: AuthenticatedRecordV2,
    /// Authenticated Product-selected portfolio record.
    pub portfolio_record: AuthenticatedRecordV2,
    /// Stable semantic Product identity inside the graph-root record.
    pub product_id: ContentId,
    /// Coordinate/statistic domain selected by Product.
    pub coordinate_domain_id: ContentId,
    /// Exact result unit selected by Product.
    pub result_unit_id: ContentId,
    /// Native claim basis selected by the portfolio.
    pub claim_basis_id: ContentId,
    /// Liability basis shared by domain and portfolio.
    pub liability_basis_id: ContentId,
    /// Representation semantic release shared by domain and portfolio.
    pub representation_release_id: ContentId,
    /// Coordinate mapping semantic release selected by the domain.
    pub mapping_release_id: ContentId,
    /// Runtime native outcome count including explicit failure.
    pub outcome_count: u32,
    /// The six record PDA bumps THIS walk used, in the eight-slot bank's
    /// positions 0 through 5; the linked-basis pair is zero until
    /// [`authenticate_product_basis_v3`] fills it.
    ///
    /// A COORDINATE CACHE, exactly like [`AuthenticatedRecordV2::coordinate`]:
    /// a fact about addresses this walk already reproduced and compared, never
    /// a substitute for repeating the authentication. Core's founding persists
    /// it in the Market's `StateBumpsV1` so that every later reader of the same
    /// graph reproduces each address instead of searching for it.
    pub record_bumps: ProductRecordBumpsV3,
}

/// Ephemeral authenticated Product Runtime V3 projection.
///
/// `linked_basis_raw` and `linked_basis_staging` are read-only evidence, not
/// routable effect accounts. A hot executor may project the raw content digest
/// as an immutable synthetic coordinate and borrow the exact raw body, but may
/// not authorize writes from that observation.
#[derive(Clone, Copy)]
pub struct AuthenticatedProductRuntimeV3<'accounts, 'info> {
    /// Existing independently authenticated Product/domain/portfolio graph.
    pub runtime: AuthenticatedProductRuntimeV2,
    /// Authenticated ProductBasisV3 schema, raw digest, and canonical PDAs.
    pub linked_basis_record: AuthenticatedRecordV2,
    /// Registry-owned read-only exact ProductBasisV3 body.
    pub linked_basis_raw: &'accounts AccountInfo<'info>,
    /// System-owned read-only vacant finalization cursor.
    pub linked_basis_staging: &'accounts AccountInfo<'info>,
    /// Product-owned semantic liability-basis identity.
    pub semantic_basis_id: ContentId,
    /// Canonical V3 evaluator kind.
    pub basis_kind: BasisKindV3,
    /// Runtime number of native basis claims.
    pub basis_width: u32,
    /// Exact native payout scale.
    pub payout_scale: u64,
    /// Immutable evaluator semantic release.
    pub evaluator_release_id: ContentId,
    /// The eight record PDA bumps this walk used, mined or searched.
    ///
    /// A COORDINATE CACHE, exactly like [`AuthenticatedRecordV2::coordinate`]:
    /// it is a fact about addresses this walk already reproduced and compared,
    /// and it is never a substitute for repeating the authentication. Its one
    /// consumer relays it to a SECOND program that runs the same walk over the
    /// same four records in the same instruction, so that program reproduces
    /// each address instead of searching for it. See
    /// [`authenticate_record_hinted`].
    pub record_bumps: ProductRecordBumpsV3,
}

impl AuthenticatedProductRuntimeV2 {
    /// Recheck an optional admission receipt after all facts and coordinates
    /// have already been independently authenticated.
    pub fn recheck_reference_receipt(self, receipt_bytes: &[u8]) -> Result<()> {
        let receipt =
            AdmissionReceiptV2::decode(receipt_bytes).map_err(|_| Error::ReceiptMismatch)?;
        let expected = AdmissionReceiptV2 {
            product: self.product_record.coordinate()?,
            result_domain: self.result_domain_record.coordinate()?,
            portfolio: self.portfolio_record.coordinate()?,
        };
        if receipt != expected {
            return Err(Error::ReceiptMismatch);
        }
        Ok(())
    }
}

/// Derive the Product graph-root digest from the exact raw body, authenticate
/// the entire graph, and return its ephemeral projection. Core Found uses this
/// form because creation selects a new content-addressed Product graph.
pub fn authenticate_content_addressed_product_runtime_v2<'accounts, 'info>(
    registry_program: &Pubkey,
    frame: ProductRuntimeFrameV2<'accounts, 'info>,
) -> Result<AuthenticatedProductRuntimeV2> {
    let product_data = frame
        .product
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let digest = content(hash(&product_data).to_bytes())?;
    drop(product_data);
    authenticate_product_runtime_v2(registry_program, digest, frame)
}

/// Authenticate and decode the exact Product graph already selected by a
/// Market or linked Claims record. The expected Product digest is the only
/// external graph authority; child identities come exclusively from the
/// authenticated Product record.
pub fn authenticate_product_runtime_v2<'accounts, 'info>(
    registry_program: &Pubkey,
    expected_product_digest: ContentId,
    frame: ProductRuntimeFrameV2<'accounts, 'info>,
) -> Result<AuthenticatedProductRuntimeV2> {
    // A NAMED SINK, not `&mut ProductRecordBumpsV3::ABSENT`. Every use of a
    // `const` item materialises a fresh temporary, so the old spelling had the
    // callee writing into an anonymous copy that `const_item_mutation` warned
    // about on every SBF build -- and a reader could not tell whether that was
    // the intent or a lost write. It is the intent: this arm searches for every
    // bump instead of relaying one, so it has nothing to hand back to a caller.
    // The bank still reaches the RESULT, because the callee returns it inside
    // `AuthenticatedProductRuntimeV2`.
    let mut derived = ProductRecordBumpsV3::ABSENT;
    authenticate_product_runtime_v2_hinted(
        registry_program,
        expected_product_digest,
        frame,
        ProductRecordBumpsV3::ABSENT,
        &mut derived,
    )
}

/// The same walk, reproducing each record PDA at a mined bump where `hints`
/// supplies one and reporting into `derived` every bump it ended up using.
///
/// See [`authenticate_record_hinted`] for what a bump is and is not, and for
/// the measurement that made this worth threading.
pub fn authenticate_product_runtime_v2_hinted<'accounts, 'info>(
    registry_program: &Pubkey,
    expected_product_digest: ContentId,
    frame: ProductRuntimeFrameV2<'accounts, 'info>,
    hints: ProductRecordBumpsV3,
    derived: &mut ProductRecordBumpsV3,
) -> Result<AuthenticatedProductRuntimeV2> {
    require_distinct(frame)?;
    let (product_record, product_bumps) = authenticate_record_hinted(
        registry_program,
        frame.product,
        PRODUCT_RECORD_SCHEMA_ID_V2,
        expected_product_digest,
        Error::ProductRecord,
        hints.at(ProductWalkRecordV3::Product),
    )?;
    derived.set(ProductWalkRecordV3::Product, product_bumps.raw, product_bumps.staging);
    let product_data = frame
        .product
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let product = ProductRecordV2::decode(&product_data).map_err(|_| Error::Composition)?;
    let (result_domain_record, domain_bumps) = authenticate_record_hinted(
        registry_program,
        frame.result_domain,
        RESULT_DOMAIN_SCHEMA_ID_V2,
        product.result_domain_digest(),
        Error::ResultDomainRecord,
        hints.at(ProductWalkRecordV3::ResultDomain),
    )?;
    derived.set(ProductWalkRecordV3::ResultDomain, domain_bumps.raw, domain_bumps.staging);
    let (portfolio_record, portfolio_bumps) = authenticate_record_hinted(
        registry_program,
        frame.portfolio,
        PORTFOLIO_SCHEMA_ID_V2,
        product.portfolio_digest(),
        Error::PortfolioRecord,
        hints.at(ProductWalkRecordV3::Portfolio),
    )?;
    derived.set(ProductWalkRecordV3::Portfolio, portfolio_bumps.raw, portfolio_bumps.staging);
    let domain_data = frame
        .result_domain
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let portfolio_data = frame
        .portfolio
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let receipt = AdmissionReceiptV2 {
        product: product_record.coordinate()?,
        result_domain: result_domain_record.coordinate()?,
        portfolio: portfolio_record.coordinate()?,
    };
    let domain = ResultDomainV2::decode(&domain_data).map_err(|_| Error::Composition)?;
    let portfolio = PortfolioV2::decode(&portfolio_data).map_err(|_| Error::Composition)?;
    let projection = admit_authenticated_views_v2(receipt, product, domain, portfolio)
        .map_err(|_| Error::Composition)?;
    if projection.product_record_digest != product_record.content_digest
        || projection.portfolio_record_digest != portfolio_record.content_digest
        || projection.join.product_id != product.product_id()
        || projection.join.result_domain_id != result_domain_record.content_digest
        || projection.join.representation_id != portfolio_record.content_digest
    {
        return Err(Error::Composition);
    }
    Ok(AuthenticatedProductRuntimeV2 {
        record_bumps: *derived,
        product_record,
        result_domain_record,
        portfolio_record,
        product_id: projection.join.product_id,
        coordinate_domain_id: domain.coordinate_domain_id(),
        result_unit_id: domain.result_unit_id(),
        claim_basis_id: projection.join.claim_basis_id,
        liability_basis_id: projection.join.liability_basis_id,
        representation_release_id: portfolio.representation_release_id(),
        mapping_release_id: domain.mapping_release_id(),
        outcome_count: projection.join.outcome_count,
    })
}

/// Derive the Product graph-root digest from its exact raw body, authenticate
/// the Product/domain/portfolio graph and one V3 linked basis, and return the
/// complete ephemeral projection.
pub fn authenticate_content_addressed_product_runtime_v3<'accounts, 'info>(
    registry_program: &Pubkey,
    frame: ProductRuntimeFrameV3<'accounts, 'info>,
) -> Result<AuthenticatedProductRuntimeV3<'accounts, 'info>> {
    let product_data = frame
        .product
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let digest = content(hash(&product_data).to_bytes())?;
    drop(product_data);
    authenticate_product_runtime_v3(registry_program, digest, frame)
}

/// Authenticate a Product-selected graph and an independently finalized,
/// canonical ProductBasisV3 record.
///
/// ProductRecordV2 deliberately does not pin a linked-basis raw digest. The
/// supplied basis is content-addressed under the sole V3 Registry schema, then
/// its semantic preimage and embedded Product/domain/unit links are checked
/// against the authenticated Product graph. This admits either canonical V3
/// evaluator kind without admitting legacy LinkedBasisRecordV2.
#[inline(never)]
pub fn authenticate_product_runtime_v3<'accounts, 'info>(
    registry_program: &Pubkey,
    expected_product_digest: ContentId,
    frame: ProductRuntimeFrameV3<'accounts, 'info>,
) -> Result<AuthenticatedProductRuntimeV3<'accounts, 'info>> {
    authenticate_product_runtime_v3_hinted(
        registry_program,
        expected_product_digest,
        frame,
        ProductRecordBumpsV3::ABSENT,
    )
}

/// The same complete walk, at mined bumps where `hints` supplies them,
/// returning in the projection's `record_bumps` every bump it used so one
/// caller can relay them to another. It names no `derived` out-parameter --
/// its doc used to, describing an argument this signature has never had.
///
/// The Dealer family runs this walk TWICE in one instruction -- Trading's own
/// prelude and, independently, the accelerator's -- and the second one is the
/// caller of this form: `admitted_composition_v3` relays what the first
/// derived in the prelude witness, and the accelerator reproduces each address
/// instead of searching for it a second time. See
/// [`authenticate_record_hinted`].
pub fn authenticate_product_runtime_v3_hinted<'accounts, 'info>(
    registry_program: &Pubkey,
    expected_product_digest: ContentId,
    frame: ProductRuntimeFrameV3<'accounts, 'info>,
    hints: ProductRecordBumpsV3,
) -> Result<AuthenticatedProductRuntimeV3<'accounts, 'info>> {
    // Named for the same reason as in `authenticate_product_runtime_v2`, and
    // this one is load-bearing rather than a sink: the V2 walk fills routes
    // 0-2 of this bank and the basis walk below fills route 3, and it is
    // `*derived` that becomes the returned `record_bumps`. Under the old
    // `&mut` on a `const` item that worked only by the temporary's lifetime
    // being extended to this block -- correct, and one edit away from silently
    // not being.
    let mut bank = ProductRecordBumpsV3::ABSENT;
    let derived = &mut bank;
    let runtime = authenticate_product_runtime_v2_hinted(
        registry_program,
        expected_product_digest,
        ProductRuntimeFrameV2 {
            product: frame.product,
            result_domain: frame.result_domain,
            portfolio: frame.portfolio,
        },
        hints,
        derived,
    )?;
    // This is a continuation over a basis Core already admitted when it
    // committed the founding permit. Authentication remains per-use; the
    // founding-only no-arbitrage conjunct does not.
    authenticate_product_basis_v3_with_admission(
        registry_program,
        runtime,
        frame.linked_basis,
        PreviouslyAdmittedBasisV3,
        hints,
        derived,
    )
}

/// Authenticate only the finalized ProductBasisV3 selected by an already
/// authenticated Product Runtime V2 projection.
///
/// This is the canonical continuation after a caller has authenticated the
/// Product/domain/portfolio graph in the same instruction. It does not decode
/// or hash those runtime-width tails a second time. The fixed basis schema,
/// content-addressed raw/staging coordinate, and every semantic Product join
/// are still independently checked here.
#[inline(always)]
pub fn authenticate_product_basis_v3<'accounts, 'info>(
    registry_program: &Pubkey,
    runtime: AuthenticatedProductRuntimeV2,
    linked_basis: FinalizedRecordFrameV2<'accounts, 'info>,
) -> Result<AuthenticatedProductRuntimeV3<'accounts, 'info>> {
    // Seeded from the walk that already ran, so the returned bank is the
    // COMPLETE eight and not just the pair this call derives. A caller that
    // records these -- Core's founding does -- would otherwise persist six
    // zeros beside two bumps and never know it.
    let mut derived = runtime.record_bumps;
    authenticate_product_basis_v3_with_admission(
        registry_program,
        runtime,
        linked_basis,
        PreviouslyAdmittedBasisV3,
        ProductRecordBumpsV3::ABSENT,
        &mut derived,
    )
}

/// Authenticate and admit the ProductBasisV3 selected by a founding Product.
///
/// This is the one-time no-arbitrage boundary for callers that can commit a
/// founding permit. Unlike [`authenticate_product_basis_v3`], it runs the
/// basis-selection cascade and authenticates the price-gate certificate named
/// by a curved basis. Continuations must use the authentication-only function:
/// the immutable basis digest in the founded Market is the evidence that this
/// conjunct already ran.
#[inline(always)]
pub fn authenticate_founding_product_basis_v3<'accounts, 'info>(
    registry_program: &Pubkey,
    runtime: AuthenticatedProductRuntimeV2,
    linked_basis: FinalizedRecordFrameV2<'accounts, 'info>,
    price_gate: Option<FinalizedRecordFrameV2<'accounts, 'info>>,
) -> Result<AuthenticatedProductRuntimeV3<'accounts, 'info>> {
    // The same seeding as `authenticate_product_basis_v3`: this is the call
    // Core's founding makes, and the eight bumps it returns are what the
    // Market records so that every later reader of this graph stops searching.
    let mut derived = runtime.record_bumps;
    authenticate_product_basis_v3_with_admission(
        registry_program,
        runtime,
        linked_basis,
        FoundingBasisAdmissionV3 { price_gate },
        ProductRecordBumpsV3::ABSENT,
        &mut derived,
    )
}

struct PreviouslyAdmittedBasisV3;

struct FoundingBasisAdmissionV3<'accounts, 'info> {
    price_gate: Option<FinalizedRecordFrameV2<'accounts, 'info>>,
}

trait BasisAdmissionBoundaryV3 {
    fn admit(self, registry_program: &Pubkey, basis: ProductBasisV3<'_>) -> Result<()>;
}

impl BasisAdmissionBoundaryV3 for PreviouslyAdmittedBasisV3 {
    #[inline(always)]
    fn admit(self, _registry_program: &Pubkey, _basis: ProductBasisV3<'_>) -> Result<()> {
        Ok(())
    }
}

impl BasisAdmissionBoundaryV3 for FoundingBasisAdmissionV3<'_, '_> {
    #[inline(always)]
    fn admit(self, registry_program: &Pubkey, basis: ProductBasisV3<'_>) -> Result<()> {
        basis
            .admit_selection_v3()
            .map_err(|_| Error::LinkedBasisComposition)?;
        let certificate_digest = basis.price_gate_certificate_digest_v3();
        if certificate_digest == [0_u8; 32] {
            return Ok(());
        }
        let frame = self.price_gate.ok_or(Error::PriceGateRequired)?;
        authenticate_record(
            registry_program,
            frame,
            PRICE_GATE_RECORD_SCHEMA_ID_V1,
            content(certificate_digest).map_err(|_| Error::PriceGateBasisMismatch)?,
            Error::PriceGateBasisMismatch,
        )?;
        let degree = match basis.kind() {
            BasisKindV3::SplineDegree2To3 { degree, .. } => degree,
            // Unreachable: `decode` requires the digest zero for every kind the
            // gate exempts, so a nonzero digest is a curved basis. Stated as a
            // refusal rather than assumed away.
            _ => return Err(Error::PriceGateBasisMismatch),
        };
        let certificate_data = frame.raw.try_borrow_data().map_err(|_| Error::Borrow)?;
        // Every atom is recomputed through the production evaluator here.
        // Nothing about a payout vector is taken from the certificate.
        verify_price_gate_v1(
            &basis,
            basis.knot_denominator(),
            basis.payout_scale(),
            degree,
            basis.basis_width(),
            &certificate_data,
        )
        .map_err(|error| match error {
            BasisError::PriceGateCapacity => Error::PriceGateCapacity,
            BasisError::PriceGateBasisMismatch => Error::PriceGateBasisMismatch,
            BasisError::PriceGateUnsupportedProfile
            | BasisError::NonCanonicalReserved
            | BasisError::PriceGateNonCanonicalPadding
            | BasisError::PriceGateNonCanonicalAtomOrder
            | BasisError::PriceGateZeroAtomWeight
            | BasisError::PriceGateZeroMass
            | BasisError::PriceGateWidthOutOfRange
            | BasisError::PriceGateNonPrimitiveWeightScale
            | BasisError::PriceGateWeightMassMismatch
            | BasisError::PriceGatePriceNotPartition => Error::PriceGateNonCanonical,
            _ => Error::PriceGateHullRefused,
        })?;
        Ok(())
    }
}

#[inline(never)]
#[allow(clippy::too_many_arguments)]
fn authenticate_product_basis_v3_with_admission<'accounts, 'info, Admission>(
    registry_program: &Pubkey,
    runtime: AuthenticatedProductRuntimeV2,
    linked_basis: FinalizedRecordFrameV2<'accounts, 'info>,
    admission: Admission,
    hints: ProductRecordBumpsV3,
    derived: &mut ProductRecordBumpsV3,
) -> Result<AuthenticatedProductRuntimeV3<'accounts, 'info>>
where
    Admission: BasisAdmissionBoundaryV3,
{
    require_basis_distinct(runtime, linked_basis)?;
    let basis_data = linked_basis
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let basis_digest = content(hash(&basis_data).to_bytes())?;
    drop(basis_data);
    let (linked_basis_record, basis_bumps) = authenticate_record_hinted(
        registry_program,
        linked_basis,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        basis_digest,
        Error::LinkedBasisRecord,
        hints.at(ProductWalkRecordV3::LinkedBasis),
    )?;
    derived.set(ProductWalkRecordV3::LinkedBasis, basis_bumps.raw, basis_bumps.staging);
    let basis_data = linked_basis
        .raw
        .try_borrow_data()
        .map_err(|_| Error::Borrow)?;
    let basis = ProductBasisV3::decode(&basis_data).map_err(|_| Error::LinkedBasisComposition)?;
    admission.admit(registry_program, basis)?;
    let semantic = basis
        .semantic_preimage_v3()
        .map_err(|_| Error::LinkedBasisComposition)?;
    let semantic_basis_id = content(
        hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes(),
    )?;
    let evaluator_release_id =
        content(basis.evaluator_release_id()).map_err(|_| Error::LinkedBasisComposition)?;
    if semantic_basis_id != runtime.liability_basis_id
        || basis.product_id() != runtime.product_id.to_bytes()
        || basis.result_domain_id() != runtime.result_domain_record.content_digest.to_bytes()
        || basis.coordinate_domain_id() != runtime.coordinate_domain_id.to_bytes()
        || basis.result_unit_id() != runtime.result_unit_id.to_bytes()
    {
        return Err(Error::LinkedBasisComposition);
    }
    Ok(AuthenticatedProductRuntimeV3 {
        runtime,
        linked_basis_record,
        linked_basis_raw: linked_basis.raw,
        linked_basis_staging: linked_basis.staging,
        semantic_basis_id,
        basis_kind: basis.kind(),
        basis_width: basis.basis_width(),
        payout_scale: basis.payout_scale(),
        evaluator_release_id,
        record_bumps: *derived,
    })
}

/// Authenticate one finalized record frame against the Registry program, its
/// expected schema and content digest, refusing with `refusal` on any mismatch.
#[inline(never)]
pub fn authenticate_record(
    registry_program: &Pubkey,
    frame: FinalizedRecordFrameV2<'_, '_>,
    schema: [u8; 32],
    expected_digest: ContentId,
    refusal: Error,
) -> Result<AuthenticatedRecordV2> {
    authenticate_record_hinted(
        registry_program,
        frame,
        schema,
        expected_digest,
        refusal,
        RecordBumpHintsV2::ABSENT,
    )
    .map(|(record, _)| record)
}

/// One record's raw and staging bumps: a search hint, never an authority.
///
/// Zero is absent and the reader searches exactly as it used to, which is what
/// every caller that has not mined them passes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RecordBumpHintsV2 {
    /// Bump of the Registry raw-record PDA.
    pub raw: u8,
    /// Bump of that record's Registry staging cursor PDA.
    pub staging: u8,
}

impl RecordBumpHintsV2 {
    /// Nothing mined: both derivations search.
    pub const ABSENT: Self = Self { raw: 0, staging: 0 };
}

/// The eight bumps one Product graph walk derives, in canonical record order.
///
/// Product raw, Product staging, ResultDomain raw, ResultDomain staging,
/// Portfolio raw, Portfolio staging, linked basis raw, linked basis staging --
/// the order the walk visits them and the order
/// `AdmittedPreludeWitnessV1::record_bumps` carries them.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProductRecordBumpsV3(pub [u8; 8]);

/// The four records the Product graph walk visits, in the order it visits them
/// and the order [`ProductRecordBumpsV3`] carries their pairs.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProductWalkRecordV3 {
    /// The Product record itself.
    Product,
    /// The ResultDomain the Product names.
    ResultDomain,
    /// The Portfolio the Product names.
    Portfolio,
    /// The basis record the Product links.
    LinkedBasis,
}

impl ProductRecordBumpsV3 {
    /// Nothing mined: every derivation on the walk searches.
    pub const ABSENT: Self = Self([0; 8]);

    /// One record's pair.
    #[must_use]
    pub const fn at(self, record: ProductWalkRecordV3) -> RecordBumpHintsV2 {
        let [p0, p1, d0, d1, f0, f1, b0, b1] = self.0;
        let (raw, staging) = match record {
            ProductWalkRecordV3::Product => (p0, p1),
            ProductWalkRecordV3::ResultDomain => (d0, d1),
            ProductWalkRecordV3::Portfolio => (f0, f1),
            ProductWalkRecordV3::LinkedBasis => (b0, b1),
        };
        RecordBumpHintsV2 { raw, staging }
    }

    fn set(&mut self, record: ProductWalkRecordV3, raw: u8, staging: u8) {
        let [p0, p1, d0, d1, f0, f1, b0, b1] = &mut self.0;
        let (slot_raw, slot_staging) = match record {
            ProductWalkRecordV3::Product => (p0, p1),
            ProductWalkRecordV3::ResultDomain => (d0, d1),
            ProductWalkRecordV3::Portfolio => (f0, f1),
            ProductWalkRecordV3::LinkedBasis => (b0, b1),
        };
        *slot_raw = raw;
        *slot_staging = staging;
    }
}

/// Authenticate one finalized record, reproducing both PDAs at a mined bump
/// where the caller supplied one and searching where it did not.
///
/// # The derivation IS the check
///
/// A hint is fed to `create_program_address` over seeds this function derives
/// for itself -- the record PDA domain, the canonical schema id, and the
/// content digest its caller established -- and the result is compared against
/// the account the frame supplied by the same equality that was always here. A
/// wrong bump derives a different address, or none, and refuses. Canonicality
/// is enforced where the account is MADE: the Registry writes finalized records
/// only at the canonical bump, so a non-canonical hint names an address at
/// which no Registry-owned record exists.
///
/// Measured 2026-09-03 on the Dealer partial equity Remove: the two searches
/// per record, over the four records of a Product graph walk, are **30,172 CU**
/// of the 39,217-CU `acc-product-runtime` span, and the span is identical to
/// the digit across every invocation and every ELF because these seeds are a
/// schema and a content digest -- fixture data, not release-set data. It is
/// draw-free AND it is a search; those are not the same property.
#[allow(clippy::too_many_arguments)]
fn authenticate_record_hinted(
    registry_program: &Pubkey,
    frame: FinalizedRecordFrameV2<'_, '_>,
    schema: [u8; 32],
    expected_digest: ContentId,
    refusal: Error,
    hints: RecordBumpHintsV2,
) -> Result<(AuthenticatedRecordV2, RecordBumpHintsV2)> {
    let digest = expected_digest.to_bytes();
    let (expected_raw, raw_bump) = record_address(
        RAW_RECORD_PDA_SEED_V1,
        schema,
        digest,
        registry_program,
        hints.raw,
        refusal,
    )?;
    let (expected_staging, staging_bump) = record_address(
        STAGING_CURSOR_PDA_SEED_V1,
        schema,
        digest,
        registry_program,
        hints.staging,
        refusal,
    )?;
    let raw_data = frame.raw.try_borrow_data().map_err(|_| Error::Borrow)?;
    if frame.raw.key != &expected_raw
        || frame.raw.owner != registry_program
        || frame.raw.is_signer
        || frame.raw.is_writable
        || frame.raw.executable
        || hash(&raw_data).to_bytes() != digest
        || !funded_rent_persists_v1(frame.raw.lamports())
        || frame.staging.key != &expected_staging
        || frame.staging.owner != &system_program::ID
        || frame.staging.is_signer
        || frame.staging.is_writable
        || frame.staging.executable
        || frame.staging.data_len() != 0
    {
        return Err(refusal);
    }
    Ok((
        AuthenticatedRecordV2 {
            schema_id: content(schema)?,
            content_digest: expected_digest,
            raw_account: *frame.raw.key,
            staging_account: *frame.staging.key,
        },
        RecordBumpHintsV2 {
            raw: raw_bump,
            staging: staging_bump,
        },
    ))
}

/// Reproduce one record PDA at a mined bump, or search for it and report the
/// bump the search found so a caller can relay it.
fn record_address(
    domain: &[u8],
    schema: [u8; 32],
    digest: [u8; 32],
    registry_program: &Pubkey,
    hint: u8,
    refusal: Error,
) -> Result<(Pubkey, u8)> {
    let base: [&[u8]; 3] = [domain, schema.as_slice(), digest.as_slice()];
    if hint == 0 {
        return Ok(Pubkey::find_program_address(&base, registry_program));
    }
    let bump_seed = [hint];
    Pubkey::create_program_address(&[base[0], base[1], base[2], &bump_seed], registry_program)
        .map(|address| (address, hint))
        .map_err(|_| refusal)
}

/// A nonzero content identity, or the composition refusal for the zero digest.
pub fn content(bytes: [u8; 32]) -> Result<ContentId> {
    ContentId::new(bytes).map_err(|_| Error::Composition)
}

fn require_distinct(frame: ProductRuntimeFrameV2<'_, '_>) -> Result<()> {
    let accounts = [
        frame.product.raw,
        frame.product.staging,
        frame.result_domain.raw,
        frame.result_domain.staging,
        frame.portfolio.raw,
        frame.portfolio.staging,
    ];
    for (left_index, left) in accounts.iter().enumerate() {
        if accounts
            .iter()
            .skip(left_index.saturating_add(1))
            .any(|right| left.key == right.key)
        {
            return Err(Error::AccountFrame);
        }
    }
    Ok(())
}

fn require_basis_distinct(
    runtime: AuthenticatedProductRuntimeV2,
    linked_basis: FinalizedRecordFrameV2<'_, '_>,
) -> Result<()> {
    let existing = [
        runtime.product_record.raw_account,
        runtime.product_record.staging_account,
        runtime.result_domain_record.raw_account,
        runtime.result_domain_record.staging_account,
        runtime.portfolio_record.raw_account,
        runtime.portfolio_record.staging_account,
    ];
    if linked_basis.raw.key == linked_basis.staging.key
        || existing
            .iter()
            .any(|key| linked_basis.raw.key == key || linked_basis.staging.key == key)
    {
        return Err(Error::AccountFrame);
    }
    Ok(())
}
