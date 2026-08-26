//! Runtime-width exact liability-basis successor.
//!
//! This module removes V2's fixed knot and term arrays. A canonical borrowed
//! record defines either the categorical `Q = 1` basis or a runtime-width set
//! of nonnegative rational graded curves followed by one exact complement.
//! Every graded term is evaluated with the parent module's exact signed-rational
//! arithmetic and sole final-floor boundary. The conservative sum of all term
//! amplitudes may not exceed `Q`, so the final claim is always `Q - sum(primary)`.
//!
//! Product and result-domain links are authenticated by the full raw-record
//! digest. They are deliberately omitted from [`semantic_basis_preimage_v3`]
//! so a Product result-domain may commit the semantic basis identity without a
//! hash cycle. The raw digest, Product identity, result-domain identity, and
//! semantic identity therefore remain four distinct joins at admission.

use core::convert::{TryFrom, TryInto};

use super::{ShapeV2, interpolation_floor, rational_compare};

/// Canonical runtime basis magic.
pub const BASIS_MAGIC_V3: [u8; 8] = *b"DCLTPAY3";
/// Canonical runtime basis schema.
pub const BASIS_SCHEMA_V3: u16 = 3;
/// Fixed header before all runtime tails.
pub const BASIS_HEADER_BYTES_V3: usize = 256;
/// Width of one exact knot numerator.
pub const KNOT_BYTES_V3: usize = 16;
/// Width of one canonical graded term.
pub const TERM_BYTES_V3: usize = 32;
/// Categorical exact/no-rounding boundary tag.
pub const EXACT_CATEGORICAL_BOUNDARY_V3: u8 = 0;
/// Per-term final floor followed by exact-complement boundary tag.
pub const TERM_FLOOR_EXACT_COMPLEMENT_BOUNDARY_V3: u8 = 1;
/// Content-hash domain for semantic basis identity.
pub const SEMANTIC_BASIS_CONTENT_DOMAIN_V3: &[u8] = b"dclutch/product-basis/semantic/v3";
/// Content-hash domain for the full Product-linked raw record.
pub const LINKED_BASIS_CONTENT_DOMAIN_V3: &[u8] = b"dclutch/product-basis/linked/v3";

const HEADER_BYTES_OFFSET: usize = 10;
const RECORD_BYTES_OFFSET: usize = 12;
const KIND_OFFSET: usize = 16;
const ROUNDING_OFFSET: usize = 17;
const HEADER_RESERVED_OFFSET: usize = 18;
const BASIS_WIDTH_OFFSET: usize = 20;
const KNOT_COUNT_OFFSET: usize = 24;
const TERM_COUNT_OFFSET: usize = 28;
const PRODUCT_ID_OFFSET: usize = 32;
const RESULT_DOMAIN_ID_OFFSET: usize = 64;
const COORDINATE_DOMAIN_ID_OFFSET: usize = 96;
const RESULT_UNIT_ID_OFFSET: usize = 128;
const PAYOUT_SCALE_OFFSET: usize = 160;
const KNOT_DENOMINATOR_OFFSET: usize = 168;
const EVALUATOR_RELEASE_ID_OFFSET: usize = 176;
const HEADER_TAIL_RESERVED_OFFSET: usize = 208;
const PRODUCT_LINK_END: usize = RESULT_DOMAIN_ID_OFFSET + 32;

const CATEGORICAL_KIND: u8 = 1;
const GRADED_COMPLEMENT_KIND: u8 = 2;

/// Refusal from hostile decoding, construction, or exact evaluation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A record or caller buffer did not have its exact derived width.
    InvalidLength,
    /// Magic selected another record family.
    InvalidMagic,
    /// Schema or fixed-header width selected another layout.
    UnsupportedSchema,
    /// Kind or named rounding boundary was unsupported.
    UnsupportedKind,
    /// Reserved or kind-inactive fields were nonzero.
    NonCanonicalReserved,
    /// A persisted content identity was all zero.
    ZeroIdentifier,
    /// Runtime basis width, knot count, or term count was invalid.
    InvalidCount,
    /// The payout scale was zero.
    ZeroScale,
    /// An exact rational denominator was zero.
    ZeroDenominator,
    /// Active knots were not strictly increasing.
    UnorderedKnots,
    /// A term referenced another claim or malformed shape.
    InvalidTerm,
    /// Terms were not in unique canonical `(claim, shape)` order.
    NonCanonicalTermOrder,
    /// Payouts were not an exact nonnegative partition of `Q`.
    NonPartition,
    /// A caller-selected terminal coordinate did not match this basis kind.
    UnsupportedCoordinate,
    /// A categorical selector was outside runtime width.
    SelectorOutOfRange,
    /// Checked record sizing or exact arithmetic overflowed.
    ArithmeticOverflow,
}

/// Result alias for the runtime basis successor.
pub type Result<T> = core::result::Result<T, Error>;

/// Runtime basis evaluator family.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BasisKindV3 {
    /// Runtime-width one-hot basis with `Q = 1`.
    CategoricalQ1,
    /// Nonnegative rational graded curves plus one exact complement.
    GradedExactComplement,
}

impl BasisKindV3 {
    fn tag(self) -> u8 {
        match self {
            Self::CategoricalQ1 => CATEGORICAL_KIND,
            Self::GradedExactComplement => GRADED_COMPLEMENT_KIND,
        }
    }

    fn decode(tag: u8) -> Result<Self> {
        match tag {
            CATEGORICAL_KIND => Ok(Self::CategoricalQ1),
            GRADED_COMPLEMENT_KIND => Ok(Self::GradedExactComplement),
            _ => Err(Error::UnsupportedKind),
        }
    }
}

/// One canonical runtime graded term assigned to a primary basis claim.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisTermV3 {
    /// Zero-based claim index. The final complement claim is not term-defined.
    pub claim_index: u32,
    /// Exact nonnegative term shape.
    pub shape: ShapeV2,
    /// Positive amplitude in payout-scale atoms.
    pub amplitude: u64,
}

/// Caller-owned construction inputs for one canonical runtime basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BasisInputV3<'a> {
    /// Evaluator family.
    pub kind: BasisKindV3,
    /// Stable semantic Product identity. Omitted from semantic-basis hashing.
    pub product_id: [u8; 32],
    /// Exact Product-owned result-domain record identity. Omitted from semantic hashing.
    pub result_domain_id: [u8; 32],
    /// Exact coordinate-domain identity.
    pub coordinate_domain_id: [u8; 32],
    /// Exact result-unit identity.
    pub result_unit_id: [u8; 32],
    /// Immutable evaluator semantic release.
    pub evaluator_release_id: [u8; 32],
    /// Runtime number of basis claims.
    pub basis_width: u32,
    /// Positive exact payout scale `Q`.
    pub payout_scale: u64,
    /// Positive common denominator for all knot numerators.
    pub knot_denominator: u64,
    /// Strictly increasing Product-owned knot numerators.
    pub knots: &'a [i128],
    /// Unique ordered terms for primary claims.
    pub terms: &'a [BasisTermV3],
    /// Exact failure payout vector. Empty for categorical; width-sized for graded.
    pub failure_payouts: &'a [u64],
}

/// Canonical semantic preimage fragments excluding only the two acyclic links.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SemanticBasisPreimageV3<'a> {
    prefix: &'a [u8],
    suffix: &'a [u8],
}

impl<'a> SemanticBasisPreimageV3<'a> {
    /// Bytes before the omitted Product and result-domain links.
    pub const fn prefix(self) -> &'a [u8] {
        self.prefix
    }

    /// Bytes after the omitted Product and result-domain links.
    pub const fn suffix(self) -> &'a [u8] {
        self.suffix
    }
}

/// Borrowed hostile-decoded runtime liability basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductBasisV3<'a> {
    bytes: &'a [u8],
    kind: BasisKindV3,
    product_id: [u8; 32],
    result_domain_id: [u8; 32],
    coordinate_domain_id: [u8; 32],
    result_unit_id: [u8; 32],
    evaluator_release_id: [u8; 32],
    basis_width: u32,
    payout_scale: u64,
    knot_denominator: u64,
    knot_count: u32,
    term_count: u32,
}

impl<'a> ProductBasisV3<'a> {
    /// Hostile-decode and completely validate a canonical runtime basis.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < BASIS_HEADER_BYTES_V3 {
            return Err(Error::InvalidLength);
        }
        if read_array::<8>(bytes, 0)? != BASIS_MAGIC_V3 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(bytes, 8)? != BASIS_SCHEMA_V3
            || usize::from(read_u16(bytes, HEADER_BYTES_OFFSET)?) != BASIS_HEADER_BYTES_V3
        {
            return Err(Error::UnsupportedSchema);
        }
        let encoded_bytes = usize::try_from(read_u32(bytes, RECORD_BYTES_OFFSET)?)
            .map_err(|_| Error::InvalidLength)?;
        if encoded_bytes != bytes.len() {
            return Err(Error::InvalidLength);
        }
        require_zero(bytes, HEADER_RESERVED_OFFSET, 2)?;
        require_zero(bytes, HEADER_TAIL_RESERVED_OFFSET, 48)?;
        let value = Self {
            bytes,
            kind: BasisKindV3::decode(read_byte(bytes, KIND_OFFSET)?)?,
            product_id: read_nonzero_id(bytes, PRODUCT_ID_OFFSET)?,
            result_domain_id: read_nonzero_id(bytes, RESULT_DOMAIN_ID_OFFSET)?,
            coordinate_domain_id: read_nonzero_id(bytes, COORDINATE_DOMAIN_ID_OFFSET)?,
            result_unit_id: read_nonzero_id(bytes, RESULT_UNIT_ID_OFFSET)?,
            evaluator_release_id: read_nonzero_id(bytes, EVALUATOR_RELEASE_ID_OFFSET)?,
            basis_width: read_u32(bytes, BASIS_WIDTH_OFFSET)?,
            payout_scale: read_u64(bytes, PAYOUT_SCALE_OFFSET)?,
            knot_denominator: read_u64(bytes, KNOT_DENOMINATOR_OFFSET)?,
            knot_count: read_u32(bytes, KNOT_COUNT_OFFSET)?,
            term_count: read_u32(bytes, TERM_COUNT_OFFSET)?,
        };
        value.validate()
    }

    fn validate(self) -> Result<Self> {
        if self.basis_width == 0 {
            return Err(Error::InvalidCount);
        }
        if self.payout_scale == 0 {
            return Err(Error::ZeroScale);
        }
        let expected = basis_record_bytes_v3(
            self.kind,
            usize::try_from(self.basis_width).map_err(|_| Error::InvalidLength)?,
            usize::try_from(self.knot_count).map_err(|_| Error::InvalidLength)?,
            usize::try_from(self.term_count).map_err(|_| Error::InvalidLength)?,
        )?;
        if expected != self.bytes.len() {
            return Err(Error::InvalidLength);
        }
        match self.kind {
            BasisKindV3::CategoricalQ1 => {
                if read_byte(self.bytes, ROUNDING_OFFSET)? != EXACT_CATEGORICAL_BOUNDARY_V3 {
                    return Err(Error::UnsupportedKind);
                }
                if self.payout_scale != 1
                    || self.knot_denominator != 1
                    || self.knot_count != 0
                    || self.term_count != 0
                {
                    return Err(Error::NonCanonicalReserved);
                }
            }
            BasisKindV3::GradedExactComplement => {
                if read_byte(self.bytes, ROUNDING_OFFSET)?
                    != TERM_FLOOR_EXACT_COMPLEMENT_BOUNDARY_V3
                {
                    return Err(Error::UnsupportedKind);
                }
                if self.basis_width < 2 || self.knot_denominator == 0 || self.term_count == 0 {
                    return Err(Error::InvalidCount);
                }
                self.validate_knots()?;
                self.validate_terms()?;
                validate_partition(self.failure_payouts(), self.payout_scale)?;
            }
        }
        Ok(self)
    }

    fn validate_knots(self) -> Result<()> {
        let mut prior = None;
        for knot in self.knots() {
            if prior.is_some_and(|value| knot <= value) {
                return Err(Error::UnorderedKnots);
            }
            prior = Some(knot);
        }
        Ok(())
    }

    fn validate_terms(self) -> Result<()> {
        let primary_count = self.basis_width.checked_sub(1).ok_or(Error::InvalidCount)?;
        let mut prior = None;
        let mut seen_claim = 0_u32;
        let mut amplitude_sum = 0_u64;
        for term in self.terms() {
            if term.claim_index >= primary_count {
                return Err(Error::InvalidTerm);
            }
            validate_shape(term.shape, self.knot_count)?;
            if term.amplitude == 0 {
                return Err(Error::InvalidTerm);
            }
            let key = (term.claim_index, shape_key(term.shape));
            if prior.is_some_and(|value| key <= value) {
                return Err(Error::NonCanonicalTermOrder);
            }
            if term.claim_index > seen_claim {
                if term.claim_index != seen_claim.checked_add(1).ok_or(Error::InvalidCount)? {
                    return Err(Error::InvalidTerm);
                }
                seen_claim = term.claim_index;
            }
            amplitude_sum = amplitude_sum
                .checked_add(term.amplitude)
                .ok_or(Error::ArithmeticOverflow)?;
            prior = Some(key);
        }
        if seen_claim.checked_add(1).ok_or(Error::InvalidCount)? != primary_count {
            return Err(Error::InvalidTerm);
        }
        if amplitude_sum > self.payout_scale {
            return Err(Error::NonPartition);
        }
        Ok(())
    }

    /// Basis evaluator family.
    pub const fn kind(self) -> BasisKindV3 {
        self.kind
    }

    /// Stable Product identity from the full linked record.
    pub const fn product_id(self) -> [u8; 32] {
        self.product_id
    }

    /// Exact Product-owned result-domain identity from the full linked record.
    pub const fn result_domain_id(self) -> [u8; 32] {
        self.result_domain_id
    }

    /// Semantic coordinate-domain identity.
    pub const fn coordinate_domain_id(self) -> [u8; 32] {
        self.coordinate_domain_id
    }

    /// Exact result-unit identity.
    pub const fn result_unit_id(self) -> [u8; 32] {
        self.result_unit_id
    }

    /// Immutable evaluator semantic release.
    pub const fn evaluator_release_id(self) -> [u8; 32] {
        self.evaluator_release_id
    }

    /// Runtime basis width.
    pub const fn basis_width(self) -> u32 {
        self.basis_width
    }

    /// Exact positive payout scale `Q`.
    pub const fn payout_scale(self) -> u64 {
        self.payout_scale
    }

    /// Positive common knot denominator.
    pub const fn knot_denominator(self) -> u64 {
        self.knot_denominator
    }

    /// Runtime knot count.
    pub const fn knot_count(self) -> u32 {
        self.knot_count
    }

    /// Runtime term count.
    pub const fn term_count(self) -> u32 {
        self.term_count
    }

    /// Borrow all exact Product-owned knot numerators.
    pub fn knots(self) -> KnotIterV3<'a> {
        let start = self.knots_offset().unwrap_or(self.bytes.len());
        KnotIterV3 {
            bytes: self.bytes.get(start..).unwrap_or(&[]),
            next: 0,
            count: self.knot_count,
        }
    }

    /// Borrow all unique ordered primary-claim terms.
    pub fn terms(self) -> TermIterV3<'a> {
        let start = self.terms_offset().unwrap_or(self.bytes.len());
        TermIterV3 {
            bytes: self.bytes.get(start..).unwrap_or(&[]),
            next: 0,
            count: self.term_count,
        }
    }

    /// Borrow the exact failure payout vector for a graded basis.
    pub fn failure_payouts(self) -> PayoutIterV3<'a> {
        let count = if self.kind == BasisKindV3::GradedExactComplement {
            self.basis_width
        } else {
            0
        };
        PayoutIterV3 {
            bytes: self.bytes.get(BASIS_HEADER_BYTES_V3..).unwrap_or(&[]),
            next: 0,
            count,
        }
    }

    /// Evaluate an ordinary exact rational coordinate into an exact partition.
    ///
    /// Every possible refusal is preflighted before `output` changes.
    pub fn evaluate_rational(
        self,
        numerator: i128,
        denominator: u64,
        output: &mut [u64],
    ) -> Result<()> {
        if self.kind != BasisKindV3::GradedExactComplement {
            return Err(Error::UnsupportedCoordinate);
        }
        self.require_output(output)?;
        if denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        for term in self.terms() {
            evaluate_term(self, term, numerator, denominator)?;
        }
        output.fill(0);
        let mut total = 0_u64;
        for term in self.terms() {
            let payout = evaluate_term(self, term, numerator, denominator)?;
            let index = usize::try_from(term.claim_index).map_err(|_| Error::InvalidTerm)?;
            let claim = output.get_mut(index).ok_or(Error::InvalidTerm)?;
            *claim = claim
                .checked_add(payout)
                .ok_or(Error::ArithmeticOverflow)?;
            total = total
                .checked_add(payout)
                .ok_or(Error::ArithmeticOverflow)?;
        }
        let complement = self
            .payout_scale
            .checked_sub(total)
            .ok_or(Error::NonPartition)?;
        let last = output.last_mut().ok_or(Error::InvalidLength)?;
        *last = complement;
        Ok(())
    }

    /// Evaluate the Product's explicit resolution-failure terminal result.
    pub fn evaluate_failure(self, output: &mut [u64]) -> Result<()> {
        if self.kind != BasisKindV3::GradedExactComplement {
            return Err(Error::UnsupportedCoordinate);
        }
        self.require_output(output)?;
        for (destination, payout) in output.iter_mut().zip(self.failure_payouts()) {
            *destination = payout;
        }
        Ok(())
    }

    /// Evaluate a categorical selector. This is the exact `Q = 1` embedding.
    pub fn evaluate_categorical(self, selector: u32, output: &mut [u64]) -> Result<()> {
        if self.kind != BasisKindV3::CategoricalQ1 {
            return Err(Error::UnsupportedCoordinate);
        }
        self.require_output(output)?;
        if selector >= self.basis_width {
            return Err(Error::SelectorOutOfRange);
        }
        let index = usize::try_from(selector).map_err(|_| Error::SelectorOutOfRange)?;
        output.fill(0);
        *output.get_mut(index).ok_or(Error::SelectorOutOfRange)? = 1;
        Ok(())
    }

    fn require_output(self, output: &[u64]) -> Result<()> {
        if output.len()
            != usize::try_from(self.basis_width).map_err(|_| Error::InvalidLength)?
        {
            return Err(Error::InvalidLength);
        }
        Ok(())
    }

    fn knots_offset(self) -> Result<usize> {
        let failures = if self.kind == BasisKindV3::GradedExactComplement {
            usize::try_from(self.basis_width).map_err(|_| Error::InvalidLength)?
        } else {
            0
        };
        BASIS_HEADER_BYTES_V3
            .checked_add(failures.checked_mul(8).ok_or(Error::InvalidLength)?)
            .ok_or(Error::InvalidLength)
    }

    fn terms_offset(self) -> Result<usize> {
        self.knots_offset()?
            .checked_add(
                usize::try_from(self.knot_count)
                    .map_err(|_| Error::InvalidLength)?
                    .checked_mul(KNOT_BYTES_V3)
                    .ok_or(Error::InvalidLength)?,
            )
            .ok_or(Error::InvalidLength)
    }
}

/// Exact record width for caller-owned allocation.
pub fn basis_record_bytes_v3(
    kind: BasisKindV3,
    basis_width: usize,
    knot_count: usize,
    term_count: usize,
) -> Result<usize> {
    let failure_count = match kind {
        BasisKindV3::CategoricalQ1 => 0,
        BasisKindV3::GradedExactComplement => basis_width,
    };
    BASIS_HEADER_BYTES_V3
        .checked_add(failure_count.checked_mul(8).ok_or(Error::InvalidLength)?)
        .and_then(|value| value.checked_add(knot_count.checked_mul(KNOT_BYTES_V3)?))
        .and_then(|value| value.checked_add(term_count.checked_mul(TERM_BYTES_V3)?))
        .ok_or(Error::InvalidLength)
}

/// Compile one canonical runtime basis into an exact caller-owned buffer.
///
/// All input validation completes before the first output mutation.
pub fn compile_basis_v3(input: BasisInputV3<'_>, output: &mut [u8]) -> Result<()> {
    validate_input(input)?;
    let expected = basis_record_bytes_v3(
        input.kind,
        usize::try_from(input.basis_width).map_err(|_| Error::InvalidLength)?,
        input.knots.len(),
        input.terms.len(),
    )?;
    if output.len() != expected {
        return Err(Error::InvalidLength);
    }
    let record_bytes = u32::try_from(expected).map_err(|_| Error::InvalidLength)?;
    let knot_count = u32::try_from(input.knots.len()).map_err(|_| Error::InvalidCount)?;
    let term_count = u32::try_from(input.terms.len()).map_err(|_| Error::InvalidCount)?;
    output.fill(0);
    put(output, 0, &BASIS_MAGIC_V3)?;
    put(output, 8, &BASIS_SCHEMA_V3.to_le_bytes())?;
    put(
        output,
        HEADER_BYTES_OFFSET,
        &u16::try_from(BASIS_HEADER_BYTES_V3)
            .map_err(|_| Error::InvalidLength)?
            .to_le_bytes(),
    )?;
    put(output, RECORD_BYTES_OFFSET, &record_bytes.to_le_bytes())?;
    put(output, KIND_OFFSET, &[input.kind.tag()])?;
    let rounding = match input.kind {
        BasisKindV3::CategoricalQ1 => EXACT_CATEGORICAL_BOUNDARY_V3,
        BasisKindV3::GradedExactComplement => TERM_FLOOR_EXACT_COMPLEMENT_BOUNDARY_V3,
    };
    put(output, ROUNDING_OFFSET, &[rounding])?;
    put(output, BASIS_WIDTH_OFFSET, &input.basis_width.to_le_bytes())?;
    put(output, KNOT_COUNT_OFFSET, &knot_count.to_le_bytes())?;
    put(output, TERM_COUNT_OFFSET, &term_count.to_le_bytes())?;
    put(output, PRODUCT_ID_OFFSET, &input.product_id)?;
    put(output, RESULT_DOMAIN_ID_OFFSET, &input.result_domain_id)?;
    put(output, COORDINATE_DOMAIN_ID_OFFSET, &input.coordinate_domain_id)?;
    put(output, RESULT_UNIT_ID_OFFSET, &input.result_unit_id)?;
    put(output, PAYOUT_SCALE_OFFSET, &input.payout_scale.to_le_bytes())?;
    put(
        output,
        KNOT_DENOMINATOR_OFFSET,
        &input.knot_denominator.to_le_bytes(),
    )?;
    put(
        output,
        EVALUATOR_RELEASE_ID_OFFSET,
        &input.evaluator_release_id,
    )?;
    let mut offset = BASIS_HEADER_BYTES_V3;
    for payout in input.failure_payouts {
        put(output, offset, &payout.to_le_bytes())?;
        offset = offset.checked_add(8).ok_or(Error::InvalidLength)?;
    }
    for knot in input.knots {
        put(output, offset, &knot.to_le_bytes())?;
        offset = offset
            .checked_add(KNOT_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
    }
    for term in input.terms {
        encode_term(output, offset, *term)?;
        offset = offset
            .checked_add(TERM_BYTES_V3)
            .ok_or(Error::InvalidLength)?;
    }
    let _ = ProductBasisV3::decode(output)?;
    Ok(())
}

/// Validate one record and expose its acyclic semantic-identity fragments.
pub fn semantic_basis_preimage_v3(bytes: &[u8]) -> Result<SemanticBasisPreimageV3<'_>> {
    let _ = ProductBasisV3::decode(bytes)?;
    Ok(SemanticBasisPreimageV3 {
        prefix: bytes.get(..PRODUCT_ID_OFFSET).ok_or(Error::InvalidLength)?,
        suffix: bytes.get(PRODUCT_LINK_END..).ok_or(Error::InvalidLength)?,
    })
}

fn validate_input(input: BasisInputV3<'_>) -> Result<()> {
    for id in [
        input.product_id,
        input.result_domain_id,
        input.coordinate_domain_id,
        input.result_unit_id,
        input.evaluator_release_id,
    ] {
        if id.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroIdentifier);
        }
    }
    if input.basis_width == 0 {
        return Err(Error::InvalidCount);
    }
    if input.payout_scale == 0 {
        return Err(Error::ZeroScale);
    }
    let _ = u32::try_from(input.knots.len()).map_err(|_| Error::InvalidCount)?;
    let _ = u32::try_from(input.terms.len()).map_err(|_| Error::InvalidCount)?;
    match input.kind {
        BasisKindV3::CategoricalQ1 => {
            if input.payout_scale != 1
                || input.knot_denominator != 1
                || !input.knots.is_empty()
                || !input.terms.is_empty()
                || !input.failure_payouts.is_empty()
            {
                return Err(Error::NonCanonicalReserved);
            }
        }
        BasisKindV3::GradedExactComplement => {
            if input.basis_width < 2 || input.knot_denominator == 0 || input.terms.is_empty() {
                return Err(Error::InvalidCount);
            }
            if input.failure_payouts.len()
                != usize::try_from(input.basis_width).map_err(|_| Error::InvalidLength)?
            {
                return Err(Error::InvalidLength);
            }
            let mut prior_knot = None;
            for knot in input.knots {
                if prior_knot.is_some_and(|value| *knot <= value) {
                    return Err(Error::UnorderedKnots);
                }
                prior_knot = Some(*knot);
            }
            let primary_count = input.basis_width.checked_sub(1).ok_or(Error::InvalidCount)?;
            let knot_count = u32::try_from(input.knots.len()).map_err(|_| Error::InvalidCount)?;
            let mut prior_term = None;
            let mut seen_claim = 0_u32;
            let mut amplitude_sum = 0_u64;
            for term in input.terms {
                if term.claim_index >= primary_count || term.amplitude == 0 {
                    return Err(Error::InvalidTerm);
                }
                validate_shape(term.shape, knot_count)?;
                let key = (term.claim_index, shape_key(term.shape));
                if prior_term.is_some_and(|prior| key <= prior) {
                    return Err(Error::NonCanonicalTermOrder);
                }
                if term.claim_index > seen_claim {
                    if term.claim_index != seen_claim.checked_add(1).ok_or(Error::InvalidCount)? {
                        return Err(Error::InvalidTerm);
                    }
                    seen_claim = term.claim_index;
                }
                amplitude_sum = amplitude_sum
                    .checked_add(term.amplitude)
                    .ok_or(Error::ArithmeticOverflow)?;
                prior_term = Some(key);
            }
            if seen_claim.checked_add(1).ok_or(Error::InvalidCount)? != primary_count {
                return Err(Error::InvalidTerm);
            }
            if amplitude_sum > input.payout_scale {
                return Err(Error::NonPartition);
            }
            validate_partition(input.failure_payouts.iter().copied(), input.payout_scale)?;
        }
    }
    Ok(())
}

fn validate_partition(payouts: impl Iterator<Item = u64>, scale: u64) -> Result<()> {
    let mut count = 0_u32;
    let mut total = 0_u64;
    for payout in payouts {
        count = count.checked_add(1).ok_or(Error::InvalidCount)?;
        if payout > scale {
            return Err(Error::NonPartition);
        }
        total = total
            .checked_add(payout)
            .ok_or(Error::ArithmeticOverflow)?;
    }
    if count == 0 || total != scale {
        return Err(Error::NonPartition);
    }
    Ok(())
}

fn evaluate_term(
    basis: ProductBasisV3<'_>,
    term: BasisTermV3,
    numerator: i128,
    denominator: u64,
) -> Result<u64> {
    let knot = |index: u8| -> Result<i128> {
        basis
            .knots()
            .nth(usize::from(index))
            .ok_or(Error::InvalidTerm)
    };
    match term.shape {
        ShapeV2::Constant => Ok(term.amplitude),
        ShapeV2::RampUp { left, right } => ramp(
            term.amplitude,
            knot(left)?,
            knot(right)?,
            basis.knot_denominator,
            numerator,
            denominator,
            true,
        ),
        ShapeV2::RampDown { left, right } => ramp(
            term.amplitude,
            knot(left)?,
            knot(right)?,
            basis.knot_denominator,
            numerator,
            denominator,
            false,
        ),
        ShapeV2::Tent { left, peak, right } => {
            let rising = ramp(
                term.amplitude,
                knot(left)?,
                knot(peak)?,
                basis.knot_denominator,
                numerator,
                denominator,
                true,
            )?;
            let falling = ramp(
                term.amplitude,
                knot(peak)?,
                knot(right)?,
                basis.knot_denominator,
                numerator,
                denominator,
                false,
            )?;
            Ok(rising.min(falling))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn ramp(
    amplitude: u64,
    left: i128,
    right: i128,
    knot_denominator: u64,
    numerator: i128,
    denominator: u64,
    rising: bool,
) -> Result<u64> {
    let left_cmp = rational_compare(numerator, denominator, left, knot_denominator)
        .map_err(|_| Error::ArithmeticOverflow)?;
    let right_cmp = rational_compare(numerator, denominator, right, knot_denominator)
        .map_err(|_| Error::ArithmeticOverflow)?;
    if rising {
        if left_cmp != core::cmp::Ordering::Greater {
            Ok(0)
        } else if right_cmp != core::cmp::Ordering::Less {
            Ok(amplitude)
        } else {
            interpolation_floor(
                amplitude,
                numerator,
                denominator,
                left,
                right,
                knot_denominator,
                true,
            )
            .map_err(|_| Error::ArithmeticOverflow)
        }
    } else if left_cmp != core::cmp::Ordering::Greater {
        Ok(amplitude)
    } else if right_cmp != core::cmp::Ordering::Less {
        Ok(0)
    } else {
        interpolation_floor(
            amplitude,
            numerator,
            denominator,
            left,
            right,
            knot_denominator,
            false,
        )
        .map_err(|_| Error::ArithmeticOverflow)
    }
}

fn validate_shape(shape: ShapeV2, knot_count: u32) -> Result<()> {
    match shape {
        ShapeV2::Constant => Ok(()),
        ShapeV2::RampUp { left, right } | ShapeV2::RampDown { left, right }
            if left < right && u32::from(right) < knot_count =>
        {
            Ok(())
        }
        ShapeV2::Tent { left, peak, right }
            if left < peak && peak < right && u32::from(right) < knot_count =>
        {
            Ok(())
        }
        _ => Err(Error::InvalidTerm),
    }
}

fn shape_key(shape: ShapeV2) -> u64 {
    match shape {
        ShapeV2::Constant => 0,
        ShapeV2::RampUp { left, right } => 1_u64 << 56 | u64::from(left) << 8 | u64::from(right),
        ShapeV2::RampDown { left, right } => 2_u64 << 56 | u64::from(left) << 8 | u64::from(right),
        ShapeV2::Tent { left, peak, right } => {
            3_u64 << 56 | u64::from(left) << 16 | u64::from(peak) << 8 | u64::from(right)
        }
    }
}

fn encode_term(output: &mut [u8], offset: usize, term: BasisTermV3) -> Result<()> {
    put(output, offset, &term.claim_index.to_le_bytes())?;
    let (tag, left, peak, right) = match term.shape {
        ShapeV2::Constant => (0, 0, 0, 0),
        ShapeV2::RampUp { left, right } => (1, left, 0, right),
        ShapeV2::RampDown { left, right } => (2, left, 0, right),
        ShapeV2::Tent { left, peak, right } => (3, left, peak, right),
    };
    put(output, offset.checked_add(4).ok_or(Error::InvalidLength)?, &[tag])?;
    put(output, offset.checked_add(5).ok_or(Error::InvalidLength)?, &[left])?;
    put(output, offset.checked_add(6).ok_or(Error::InvalidLength)?, &[peak])?;
    put(output, offset.checked_add(7).ok_or(Error::InvalidLength)?, &[right])?;
    put(
        output,
        offset.checked_add(24).ok_or(Error::InvalidLength)?,
        &term.amplitude.to_le_bytes(),
    )
}

fn decode_term(input: &[u8], offset: usize) -> Result<BasisTermV3> {
    require_zero(input, offset.checked_add(8).ok_or(Error::InvalidLength)?, 16)?;
    let claim_index = read_u32(input, offset)?;
    let tag = read_byte(input, offset.checked_add(4).ok_or(Error::InvalidLength)?)?;
    let left = read_byte(input, offset.checked_add(5).ok_or(Error::InvalidLength)?)?;
    let peak = read_byte(input, offset.checked_add(6).ok_or(Error::InvalidLength)?)?;
    let right = read_byte(input, offset.checked_add(7).ok_or(Error::InvalidLength)?)?;
    let shape = match tag {
        0 if left == 0 && peak == 0 && right == 0 => ShapeV2::Constant,
        1 if peak == 0 => ShapeV2::RampUp { left, right },
        2 if peak == 0 => ShapeV2::RampDown { left, right },
        3 => ShapeV2::Tent { left, peak, right },
        _ => return Err(Error::InvalidTerm),
    };
    Ok(BasisTermV3 {
        claim_index,
        shape,
        amplitude: read_u64(input, offset.checked_add(24).ok_or(Error::InvalidLength)?)?,
    })
}

/// Exact-size borrowed knot iterator.
#[derive(Clone, Debug)]
pub struct KnotIterV3<'a> {
    bytes: &'a [u8],
    next: u32,
    count: u32,
}

impl Iterator for KnotIterV3<'_> {
    type Item = i128;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }
        let index = usize::try_from(self.next).ok()?;
        let offset = index.checked_mul(KNOT_BYTES_V3)?;
        let value = i128::from_le_bytes(read_array::<16>(self.bytes, offset).ok()?);
        self.next = self.next.checked_add(1)?;
        Some(value)
    }
}

impl ExactSizeIterator for KnotIterV3<'_> {
    fn len(&self) -> usize {
        usize::try_from(self.count.saturating_sub(self.next)).unwrap_or(usize::MAX)
    }
}

/// Exact-size borrowed graded-term iterator.
#[derive(Clone, Debug)]
pub struct TermIterV3<'a> {
    bytes: &'a [u8],
    next: u32,
    count: u32,
}

impl Iterator for TermIterV3<'_> {
    type Item = BasisTermV3;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }
        let index = usize::try_from(self.next).ok()?;
        let offset = index.checked_mul(TERM_BYTES_V3)?;
        let value = decode_term(self.bytes, offset).ok()?;
        self.next = self.next.checked_add(1)?;
        Some(value)
    }
}

impl ExactSizeIterator for TermIterV3<'_> {
    fn len(&self) -> usize {
        usize::try_from(self.count.saturating_sub(self.next)).unwrap_or(usize::MAX)
    }
}

/// Exact-size borrowed payout iterator.
#[derive(Clone, Debug)]
pub struct PayoutIterV3<'a> {
    bytes: &'a [u8],
    next: u32,
    count: u32,
}

impl Iterator for PayoutIterV3<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }
        let index = usize::try_from(self.next).ok()?;
        let offset = index.checked_mul(8)?;
        let value = u64::from_le_bytes(read_array::<8>(self.bytes, offset).ok()?);
        self.next = self.next.checked_add(1)?;
        Some(value)
    }
}

impl ExactSizeIterator for PayoutIterV3<'_> {
    fn len(&self) -> usize {
        usize::try_from(self.count.saturating_sub(self.next)).unwrap_or(usize::MAX)
    }
}

fn read_nonzero_id(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = read_array(input, offset)?;
    if value.iter().all(|byte| *byte == 0) {
        return Err(Error::ZeroIdentifier);
    }
    Ok(value)
}

fn read_byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(read_array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(read_array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(read_array(input, offset)?))
}

fn read_array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::InvalidLength)?;
    input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    let end = offset.checked_add(width).ok_or(Error::InvalidLength)?;
    if input
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReserved);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    fn id(fill: u8) -> [u8; 32] {
        [fill; 32]
    }

    fn graded_input<'a>(
        knots: &'a [i128],
        terms: &'a [BasisTermV3],
        failure: &'a [u64],
    ) -> BasisInputV3<'a> {
        BasisInputV3 {
            kind: BasisKindV3::GradedExactComplement,
            product_id: id(1),
            result_domain_id: id(2),
            coordinate_domain_id: id(3),
            result_unit_id: id(4),
            evaluator_release_id: id(5),
            basis_width: 3,
            payout_scale: 100,
            knot_denominator: 2,
            knots,
            terms,
            failure_payouts: failure,
        }
    }

    fn compile(input: BasisInputV3<'_>) -> Vec<u8> {
        let width = basis_record_bytes_v3(
            input.kind,
            usize::try_from(input.basis_width).expect("width"),
            input.knots.len(),
            input.terms.len(),
        )
        .expect("record width");
        let mut output = vec![0; width];
        compile_basis_v3(input, &mut output).expect("compile");
        output
    }

    #[test]
    fn categorical_q1_embeds_at_runtime_width_258() {
        let input = BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id: id(1),
            result_domain_id: id(2),
            coordinate_domain_id: id(3),
            result_unit_id: id(4),
            evaluator_release_id: id(5),
            basis_width: 258,
            payout_scale: 1,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
        };
        let bytes = compile(input);
        assert_eq!(bytes.len(), BASIS_HEADER_BYTES_V3);
        let basis = ProductBasisV3::decode(&bytes).expect("basis");
        let mut output = vec![9; 258];
        basis
            .evaluate_categorical(257, &mut output)
            .expect("one hot");
        assert_eq!(output.iter().sum::<u64>(), 1);
        assert_eq!(output.get(257), Some(&1));
    }

    #[test]
    fn graded_curves_and_exact_complement_partition_every_terminal() {
        let knots = [-20, 0, 20, 40];
        let terms = [
            BasisTermV3 {
                claim_index: 0,
                shape: ShapeV2::RampUp { left: 0, right: 2 },
                amplitude: 40,
            },
            BasisTermV3 {
                claim_index: 1,
                shape: ShapeV2::Tent {
                    left: 1,
                    peak: 2,
                    right: 3,
                },
                amplitude: 30,
            },
        ];
        let bytes = compile(graded_input(&knots, &terms, &[7, 11, 82]));
        let basis = ProductBasisV3::decode(&bytes).expect("basis");
        for (numerator, denominator) in [
            (i128::MIN, u64::MAX),
            (-1, 1),
            (7, 3),
            (i128::MAX, u64::MAX),
        ] {
            let mut payout = [99; 3];
            basis
                .evaluate_rational(numerator, denominator, &mut payout)
                .expect("total exact evaluation");
            assert_eq!(payout.iter().sum::<u64>(), 100);
        }
        let mut failure = [0; 3];
        basis.evaluate_failure(&mut failure).expect("failure");
        assert_eq!(failure, [7, 11, 82]);
    }

    #[test]
    fn runtime_tails_lift_sixteen_item_prototype_caps() {
        let knots: Vec<i128> = (-20..=20).map(i128::from).collect();
        let terms: Vec<BasisTermV3> = (0_u32..32)
            .map(|claim_index| BasisTermV3 {
                claim_index,
                shape: ShapeV2::RampUp {
                    left: u8::try_from(claim_index).expect("left"),
                    right: u8::try_from(claim_index + 1).expect("right"),
                },
                amplitude: 1,
            })
            .collect();
        let mut failure = vec![1; 33];
        *failure.last_mut().expect("last") = 68;
        let input = BasisInputV3 {
            basis_width: 33,
            payout_scale: 100,
            knot_denominator: 1,
            knots: &knots,
            terms: &terms,
            failure_payouts: &failure,
            ..graded_input(&[], &[], &[])
        };
        let bytes = compile(input);
        let basis = ProductBasisV3::decode(&bytes).expect("basis");
        assert_eq!(basis.knot_count(), 41);
        assert_eq!(basis.term_count(), 32);
        let mut output = vec![0; 33];
        basis
            .evaluate_rational(0, 1, &mut output)
            .expect("runtime width");
        assert_eq!(output.iter().sum::<u64>(), 100);
    }

    #[test]
    fn semantic_identity_omits_links_but_not_payoff_changes() {
        let knots = [0, 10];
        let terms = [BasisTermV3 {
            claim_index: 0,
            shape: ShapeV2::RampUp { left: 0, right: 1 },
            amplitude: 100,
        }];
        let input = BasisInputV3 {
            basis_width: 2,
            ..graded_input(&knots, &terms, &[0, 100])
        };
        let original = compile(input);
        let relinked = compile(BasisInputV3 {
            product_id: id(91),
            result_domain_id: id(92),
            ..input
        });
        let first = semantic_basis_preimage_v3(&original).expect("semantic");
        let second = semantic_basis_preimage_v3(&relinked).expect("semantic");
        assert_eq!(first.prefix(), second.prefix());
        assert_eq!(first.suffix(), second.suffix());
        assert_ne!(original, relinked);

        let changed_terms = [BasisTermV3 {
            amplitude: 99,
            ..terms[0]
        }];
        let changed = compile(BasisInputV3 {
            terms: &changed_terms,
            ..input
        });
        assert_ne!(
            first.suffix(),
            semantic_basis_preimage_v3(&changed)
                .expect("changed semantic")
                .suffix()
        );
    }

    #[test]
    fn hostile_refusals_preserve_outputs() {
        let knots = [0, 10];
        let terms = [BasisTermV3 {
            claim_index: 0,
            shape: ShapeV2::RampUp { left: 0, right: 1 },
            amplitude: 60,
        }];
        let input = BasisInputV3 {
            basis_width: 2,
            ..graded_input(&knots, &terms, &[1, 99])
        };
        let bytes = compile(input);
        for width in 0..bytes.len() {
            assert_eq!(
                ProductBasisV3::decode(bytes.get(..width).expect("prefix")),
                Err(Error::InvalidLength)
            );
        }
        let mut reserved = bytes.clone();
        *reserved.get_mut(208).expect("reserved") = 1;
        assert_eq!(
            ProductBasisV3::decode(&reserved),
            Err(Error::NonCanonicalReserved)
        );
        let basis = ProductBasisV3::decode(&bytes).expect("basis");
        let mut output = [7, 8];
        assert_eq!(
            basis.evaluate_rational(1, 0, &mut output),
            Err(Error::ZeroDenominator)
        );
        assert_eq!(output, [7, 8]);
        assert_eq!(
            basis.evaluate_rational(1, 1, &mut [0]),
            Err(Error::InvalidLength)
        );
    }

    #[test]
    fn excess_amplitudes_and_nonpartitioned_failure_refuse_before_write() {
        let knots = [0, 10];
        let excessive = [BasisTermV3 {
            claim_index: 0,
            shape: ShapeV2::RampUp { left: 0, right: 1 },
            amplitude: 101,
        }];
        let input = BasisInputV3 {
            basis_width: 2,
            ..graded_input(&knots, &excessive, &[0, 100])
        };
        let width = basis_record_bytes_v3(input.kind, 2, 2, 1).expect("width");
        let mut output = vec![0xa5; width];
        assert_eq!(compile_basis_v3(input, &mut output), Err(Error::NonPartition));
        assert!(output.iter().all(|byte| *byte == 0xa5));

        let valid_term = [BasisTermV3 {
            amplitude: 100,
            ..excessive[0]
        }];
        let invalid_failure = BasisInputV3 {
            terms: &valid_term,
            failure_payouts: &[0, 99],
            ..input
        };
        assert_eq!(
            compile_basis_v3(invalid_failure, &mut output),
            Err(Error::NonPartition)
        );
        assert!(output.iter().all(|byte| *byte == 0xa5));
    }
}
