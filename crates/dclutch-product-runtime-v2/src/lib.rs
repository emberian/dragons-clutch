//! Runtime-width Product result domains and exact rational portfolios.
//!
//! Records use hostile-decodable runtime tails rather than const generics or a
//! universal maximum outcome count. The crate is safe `no_std`, `no_alloc`,
//! and performs no hashing, account access, or Solana SDK work. Content hashes
//! are authenticated by adapters and supplied here as [`ContentId`] values.

#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

use core::{cmp::Ordering, convert::TryFrom};

#[allow(missing_docs)]
mod generated;

pub use generated::*;

/// Content-hash namespace for a Product-owned runtime result domain.
pub const RESULT_DOMAIN_CONTENT_DOMAIN_V2: &[u8] = b"dclutch.product-result-domain.v2";
/// Content-hash namespace for a runtime-width rational portfolio.
pub const PORTFOLIO_CONTENT_DOMAIN_V2: &[u8] = b"dclutch.product-portfolio.v2";

const HEADER_BYTES_OFFSET: usize = 10;
const RECORD_BYTES_OFFSET: usize = 12;
const DOMAIN_FLAGS_OFFSET: usize = 24;
const DOMAIN_RESERVED_TAIL_OFFSET: usize = 232;
const PORTFOLIO_RESERVED_OFFSET: usize = 21;
const PORTFOLIO_RESERVED_TAIL_OFFSET: usize = 200;

/// Runtime Product decoding, compilation, or exact-arithmetic refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed header or runtime tail had the wrong exact length.
    InvalidLength,
    /// A record did not carry its canonical magic.
    InvalidMagic,
    /// The schema version or fixed header width was unsupported.
    UnsupportedSchema,
    /// Reserved or inactive bytes were nonzero.
    NonCanonicalReserved,
    /// A persisted content identity was all zero.
    ZeroContentId,
    /// A runtime count was zero or inconsistent with another derived count.
    InvalidCount,
    /// A denominator was zero.
    ZeroDenominator,
    /// Result cuts were not strictly increasing.
    UnorderedCuts,
    /// Portfolio coefficients were empty or all zero.
    EmptyPortfolio,
    /// A rational portfolio had a reducible common representation.
    NonCanonicalPortfolio,
    /// The selected rounding boundary was not the Product V2 final floor.
    UnsupportedRounding,
    /// An authenticated Product/domain/basis/representation identity differed.
    IdentityMismatch,
    /// A caller output buffer had the wrong exact runtime width.
    OutputLength,
    /// Checked byte sizing or exact arithmetic overflowed its physical integer.
    ArithmeticOverflow,
}

/// Product result for this physical refinement.
pub type Result<T> = core::result::Result<T, Error>;

/// Nonzero authenticated 32-byte content identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ContentId([u8; 32]);

impl ContentId {
    /// Validate one adapter-authenticated digest.
    pub fn new(bytes: [u8; 32]) -> Result<Self> {
        if bytes.iter().all(|byte| *byte == 0) {
            return Err(Error::ZeroContentId);
        }
        Ok(Self(bytes))
    }

    /// Return the exact digest bytes.
    pub const fn to_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Inputs for compiling one Product-owned runtime-width result domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultDomainInputV2<'a> {
    /// Stable Product semantic identity.
    pub product_id: ContentId,
    /// Exact coordinate/statistic domain identity.
    pub coordinate_domain_id: ContentId,
    /// Exact result-unit identity.
    pub result_unit_id: ContentId,
    /// Product-selected liability-basis identity.
    pub liability_basis_id: ContentId,
    /// Product-selected representation semantic release.
    pub representation_release_id: ContentId,
    /// Product-selected mapping semantic release.
    pub mapping_release_id: ContentId,
    /// Positive common denominator for every cut numerator.
    pub cut_denominator: u64,
    /// Strictly increasing cut numerators. Length is runtime-selected.
    pub cuts: &'a [i128],
}

/// Borrowed, validated runtime-width Product result domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResultDomainV2<'a> {
    bytes: &'a [u8],
    product_id: ContentId,
    coordinate_domain_id: ContentId,
    result_unit_id: ContentId,
    liability_basis_id: ContentId,
    representation_release_id: ContentId,
    mapping_release_id: ContentId,
    cut_denominator: u64,
    region_count: u32,
    cut_count: u32,
}

impl<'a> ResultDomainV2<'a> {
    /// Hostile-decode and fully validate one exact runtime-tail record.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < DOMAIN_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != DOMAIN_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_common_header(bytes, DOMAIN_HEADER_BYTES)?;
        require_zero(bytes, DOMAIN_FLAGS_OFFSET, 8)?;
        require_zero(bytes, DOMAIN_RESERVED_TAIL_OFFSET, 8)?;
        let region_count = read_u32(bytes, DOMAIN_REGION_COUNT_OFFSET)?;
        let cut_count = read_u32(bytes, DOMAIN_CUT_COUNT_OFFSET)?;
        if region_count == 0
            || region_count != cut_count.checked_add(1).ok_or(Error::ArithmeticOverflow)?
        {
            return Err(Error::InvalidCount);
        }
        let expected = record_len(DOMAIN_HEADER_BYTES, cut_count, DOMAIN_CUT_BYTES)?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let value = Self {
            bytes,
            product_id: read_id(bytes, DOMAIN_PRODUCT_ID_OFFSET)?,
            coordinate_domain_id: read_id(bytes, DOMAIN_COORDINATE_DOMAIN_ID_OFFSET)?,
            result_unit_id: read_id(bytes, DOMAIN_RESULT_UNIT_ID_OFFSET)?,
            liability_basis_id: read_id(bytes, DOMAIN_LIABILITY_BASIS_ID_OFFSET)?,
            representation_release_id: read_id(bytes, DOMAIN_REPRESENTATION_RELEASE_ID_OFFSET)?,
            mapping_release_id: read_id(bytes, DOMAIN_MAPPING_RELEASE_ID_OFFSET)?,
            cut_denominator: read_u64(bytes, DOMAIN_CUT_DENOMINATOR_OFFSET)?,
            region_count,
            cut_count,
        };
        if value.cut_denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        let mut previous = None;
        for cut in value.cuts() {
            if previous.is_some_and(|prior| cut <= prior) {
                return Err(Error::UnorderedCuts);
            }
            previous = Some(cut);
        }
        Ok(value)
    }

    /// Stable Product identity owning this domain relation.
    pub const fn product_id(self) -> ContentId {
        self.product_id
    }
    /// Exact coordinate-domain identity.
    pub const fn coordinate_domain_id(self) -> ContentId {
        self.coordinate_domain_id
    }
    /// Exact result-unit identity.
    pub const fn result_unit_id(self) -> ContentId {
        self.result_unit_id
    }
    /// Product-selected liability-basis identity.
    pub const fn liability_basis_id(self) -> ContentId {
        self.liability_basis_id
    }
    /// Product-selected representation semantic release.
    pub const fn representation_release_id(self) -> ContentId {
        self.representation_release_id
    }
    /// Product-selected coordinate-mapping semantic release.
    pub const fn mapping_release_id(self) -> ContentId {
        self.mapping_release_id
    }
    /// Positive common cut denominator.
    pub const fn cut_denominator(self) -> u64 {
        self.cut_denominator
    }
    /// Runtime ordinary-region count.
    pub const fn region_count(self) -> u32 {
        self.region_count
    }
    /// Runtime native outcome count: ordinary regions plus explicit failure.
    pub fn outcome_count(self) -> Result<u32> {
        self.region_count
            .checked_add(1)
            .ok_or(Error::ArithmeticOverflow)
    }
    /// Explicit failure selector, distinct from every ordinary selector.
    pub const fn failure_selector(self) -> u32 {
        self.region_count
    }
    /// Borrow all exact cut numerators without allocating.
    pub fn cuts(self) -> CutIter<'a> {
        CutIter {
            bytes: self.bytes.get(DOMAIN_HEADER_BYTES..).unwrap_or(&[]),
            next: 0,
            count: self.cut_count,
        }
    }

    /// Map an exact signed-rational coordinate to one ordinary selector.
    pub fn select_ordinary(self, numerator: i128, denominator: u64) -> Result<u32> {
        if denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        let mut selector = 0_u32;
        for cut in self.cuts() {
            if compare_signed_rational(numerator, denominator, cut, self.cut_denominator)
                == Ordering::Less
            {
                return Ok(selector);
            }
            selector = selector.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if selector >= self.region_count {
            return Err(Error::InvalidCount);
        }
        Ok(selector)
    }
}

/// Exact-size borrowed iterator over runtime cut numerators.
#[derive(Clone, Debug)]
pub struct CutIter<'a> {
    bytes: &'a [u8],
    next: u32,
    count: u32,
}

impl Iterator for CutIter<'_> {
    type Item = i128;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }
        let index = usize::try_from(self.next).ok()?;
        let offset = index.checked_mul(DOMAIN_CUT_BYTES)?;
        let value = i128::from_le_bytes(array::<16>(self.bytes, offset).ok()?);
        self.next = self.next.checked_add(1)?;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::try_from(self.count.saturating_sub(self.next)).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for CutIter<'_> {}

/// Inputs for compiling one runtime-width exact rational portfolio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioInputV2<'a> {
    /// Stable Product identity matching the Product-owned domain.
    pub product_id: ContentId,
    /// Authenticated content identity of the exact result-domain record.
    pub result_domain_id: ContentId,
    /// Exact native claim-basis identity.
    pub claim_basis_id: ContentId,
    /// Liability basis selected by Product.
    pub liability_basis_id: ContentId,
    /// Representation semantic release selected by Product.
    pub representation_release_id: ContentId,
    /// Positive common coefficient denominator.
    pub denominator: u64,
    /// Nonnegative coefficient numerators in native outcome order.
    pub coefficients: &'a [u64],
}

/// Borrowed, validated runtime-width rational portfolio.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PortfolioV2<'a> {
    bytes: &'a [u8],
    product_id: ContentId,
    result_domain_id: ContentId,
    claim_basis_id: ContentId,
    liability_basis_id: ContentId,
    representation_release_id: ContentId,
    denominator: u64,
    coefficient_count: u32,
}

impl<'a> PortfolioV2<'a> {
    /// Hostile-decode and validate one canonical rational portfolio.
    pub fn decode(bytes: &'a [u8]) -> Result<Self> {
        if bytes.len() < PORTFOLIO_HEADER_BYTES {
            return Err(Error::InvalidLength);
        }
        if array::<8>(bytes, 0)? != PORTFOLIO_MAGIC {
            return Err(Error::InvalidMagic);
        }
        validate_common_header(bytes, PORTFOLIO_HEADER_BYTES)?;
        let coefficient_count = read_u32(bytes, PORTFOLIO_COEFFICIENT_COUNT_OFFSET)?;
        if coefficient_count == 0 {
            return Err(Error::InvalidCount);
        }
        if byte(bytes, PORTFOLIO_ROUNDING_OFFSET)? != REPRESENTATION_FLOOR_TAG {
            return Err(Error::UnsupportedRounding);
        }
        require_zero(bytes, PORTFOLIO_RESERVED_OFFSET, 11)?;
        require_zero(bytes, PORTFOLIO_RESERVED_TAIL_OFFSET, 8)?;
        let expected = record_len(
            PORTFOLIO_HEADER_BYTES,
            coefficient_count,
            PORTFOLIO_COEFFICIENT_BYTES,
        )?;
        if bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let value = Self {
            bytes,
            product_id: read_id(bytes, PORTFOLIO_PRODUCT_ID_OFFSET)?,
            result_domain_id: read_id(bytes, PORTFOLIO_RESULT_DOMAIN_ID_OFFSET)?,
            claim_basis_id: read_id(bytes, PORTFOLIO_CLAIM_BASIS_ID_OFFSET)?,
            liability_basis_id: read_id(bytes, PORTFOLIO_LIABILITY_BASIS_ID_OFFSET)?,
            representation_release_id: read_id(bytes, PORTFOLIO_REPRESENTATION_RELEASE_ID_OFFSET)?,
            denominator: read_u64(bytes, PORTFOLIO_DENOMINATOR_OFFSET)?,
            coefficient_count,
        };
        if value.denominator == 0 {
            return Err(Error::ZeroDenominator);
        }
        let mut divisor = value.denominator;
        let mut nonzero = false;
        for coefficient in value.coefficients() {
            nonzero |= coefficient != 0;
            divisor = gcd(divisor, coefficient);
        }
        if !nonzero {
            return Err(Error::EmptyPortfolio);
        }
        if divisor != 1 {
            return Err(Error::NonCanonicalPortfolio);
        }
        Ok(value)
    }

    /// Stable Product identity referenced by this recipe.
    pub const fn product_id(self) -> ContentId {
        self.product_id
    }
    /// Authenticated exact result-domain content identity.
    pub const fn result_domain_id(self) -> ContentId {
        self.result_domain_id
    }
    /// Exact native claim-basis identity.
    pub const fn claim_basis_id(self) -> ContentId {
        self.claim_basis_id
    }
    /// Referenced Product-selected liability basis.
    pub const fn liability_basis_id(self) -> ContentId {
        self.liability_basis_id
    }
    /// Referenced Product-selected representation release.
    pub const fn representation_release_id(self) -> ContentId {
        self.representation_release_id
    }
    /// Canonical positive common denominator.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }
    /// Runtime coefficient/native-claim width.
    pub const fn coefficient_count(self) -> u32 {
        self.coefficient_count
    }
    /// Borrow every canonical coefficient without allocating.
    pub fn coefficients(self) -> CoefficientIter<'a> {
        CoefficientIter {
            bytes: self.bytes.get(PORTFOLIO_HEADER_BYTES..).unwrap_or(&[]),
            next: 0,
            count: self.coefficient_count,
        }
    }

    /// Apply the one named final floor to every exact coefficient.
    ///
    /// All arithmetic and output widths are preflighted before the caller
    /// buffer is changed. Therefore every refusal leaves `output` unchanged.
    pub fn materialize_floor(self, scale: u64, output: &mut [u64]) -> Result<()> {
        if output.len()
            != usize::try_from(self.coefficient_count).map_err(|_| Error::OutputLength)?
        {
            return Err(Error::OutputLength);
        }
        for coefficient in self.coefficients() {
            let numerator = u128::from(coefficient)
                .checked_mul(u128::from(scale))
                .ok_or(Error::ArithmeticOverflow)?;
            let quantity = numerator
                .checked_div(u128::from(self.denominator))
                .ok_or(Error::ZeroDenominator)?;
            u64::try_from(quantity).map_err(|_| Error::ArithmeticOverflow)?;
        }
        for (destination, coefficient) in output.iter_mut().zip(self.coefficients()) {
            let numerator = u128::from(coefficient) * u128::from(scale);
            let quantity = numerator / u128::from(self.denominator);
            *destination = u64::try_from(quantity).map_err(|_| Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    /// Recheck caller materialization against the one named floor.
    pub fn recheck_materialization(self, scale: u64, quantities: &[u64]) -> Result<()> {
        if quantities.len()
            != usize::try_from(self.coefficient_count).map_err(|_| Error::InvalidLength)?
        {
            return Err(Error::InvalidLength);
        }
        for (quantity, coefficient) in quantities.iter().copied().zip(self.coefficients()) {
            let numerator = u128::from(coefficient) * u128::from(scale);
            let expected = u64::try_from(numerator / u128::from(self.denominator))
                .map_err(|_| Error::ArithmeticOverflow)?;
            if quantity != expected {
                return Err(Error::IdentityMismatch);
            }
        }
        Ok(())
    }
}

/// Exact-size borrowed iterator over runtime portfolio coefficients.
#[derive(Clone, Debug)]
pub struct CoefficientIter<'a> {
    bytes: &'a [u8],
    next: u32,
    count: u32,
}

impl Iterator for CoefficientIter<'_> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.count {
            return None;
        }
        let index = usize::try_from(self.next).ok()?;
        let offset = index.checked_mul(PORTFOLIO_COEFFICIENT_BYTES)?;
        let value = u64::from_le_bytes(array::<8>(self.bytes, offset).ok()?);
        self.next = self.next.checked_add(1)?;
        Some(value)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = usize::try_from(self.count.saturating_sub(self.next)).unwrap_or(usize::MAX);
        (remaining, Some(remaining))
    }
}

impl ExactSizeIterator for CoefficientIter<'_> {}

/// Exact authenticated Product→domain→basis→representation join.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductJoinV2 {
    /// Stable Product identity.
    pub product_id: ContentId,
    /// Authenticated result-domain content identity.
    pub result_domain_id: ContentId,
    /// Product-selected liability basis.
    pub liability_basis_id: ContentId,
    /// Authenticated portfolio/representation content identity.
    pub representation_id: ContentId,
    /// Native claim-basis identity referenced by the representation.
    pub claim_basis_id: ContentId,
    /// Runtime native outcome width.
    pub outcome_count: u32,
}

/// Join exact authenticated content identities without inventing a parallel
/// DTO authority.
pub fn join_product_v2(
    result_domain_id: ContentId,
    representation_id: ContentId,
    domain: ResultDomainV2<'_>,
    portfolio: PortfolioV2<'_>,
) -> Result<ProductJoinV2> {
    let outcome_count = domain.outcome_count()?;
    if portfolio.product_id != domain.product_id
        || portfolio.result_domain_id != result_domain_id
        || portfolio.liability_basis_id != domain.liability_basis_id
        || portfolio.representation_release_id != domain.representation_release_id
        || portfolio.coefficient_count != outcome_count
    {
        return Err(Error::IdentityMismatch);
    }
    Ok(ProductJoinV2 {
        product_id: domain.product_id,
        result_domain_id,
        liability_basis_id: domain.liability_basis_id,
        representation_id,
        claim_basis_id: portfolio.claim_basis_id,
        outcome_count,
    })
}

/// Exact encoded byte length for a runtime cut count.
pub fn result_domain_record_bytes(cut_count: usize) -> Result<usize> {
    let count = u32::try_from(cut_count).map_err(|_| Error::ArithmeticOverflow)?;
    record_len(DOMAIN_HEADER_BYTES, count, DOMAIN_CUT_BYTES)
}

/// Exact encoded byte length for a runtime portfolio width.
pub fn portfolio_record_bytes(coefficient_count: usize) -> Result<usize> {
    let count = u32::try_from(coefficient_count).map_err(|_| Error::ArithmeticOverflow)?;
    record_len(PORTFOLIO_HEADER_BYTES, count, PORTFOLIO_COEFFICIENT_BYTES)
}

/// Compile a canonical runtime-tail result domain into a caller buffer.
///
/// Validation is complete before the first output write, so refusal preserves
/// the caller buffer byte-for-byte.
pub fn compile_result_domain_v2(input: ResultDomainInputV2<'_>, output: &mut [u8]) -> Result<()> {
    if input.cut_denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    let cut_count = u32::try_from(input.cuts.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let region_count = cut_count.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    let expected = result_domain_record_bytes(input.cuts.len())?;
    if output.len() != expected {
        return Err(Error::OutputLength);
    }
    let record_bytes = u32::try_from(expected).map_err(|_| Error::ArithmeticOverflow)?;
    let mut previous = None;
    for cut in input.cuts.iter().copied() {
        if previous.is_some_and(|prior| cut <= prior) {
            return Err(Error::UnorderedCuts);
        }
        previous = Some(cut);
    }
    output.fill(0);
    put(output, 0, &DOMAIN_MAGIC)?;
    put(output, 8, &PRODUCT_RUNTIME_V2_SCHEMA_VERSION.to_le_bytes())?;
    put(
        output,
        HEADER_BYTES_OFFSET,
        &u16::try_from(DOMAIN_HEADER_BYTES)
            .map_err(|_| Error::ArithmeticOverflow)?
            .to_le_bytes(),
    )?;
    put(output, RECORD_BYTES_OFFSET, &record_bytes.to_le_bytes())?;
    put(
        output,
        DOMAIN_REGION_COUNT_OFFSET,
        &region_count.to_le_bytes(),
    )?;
    put(output, DOMAIN_CUT_COUNT_OFFSET, &cut_count.to_le_bytes())?;
    put(
        output,
        DOMAIN_PRODUCT_ID_OFFSET,
        &input.product_id.to_bytes(),
    )?;
    put(
        output,
        DOMAIN_COORDINATE_DOMAIN_ID_OFFSET,
        &input.coordinate_domain_id.to_bytes(),
    )?;
    put(
        output,
        DOMAIN_RESULT_UNIT_ID_OFFSET,
        &input.result_unit_id.to_bytes(),
    )?;
    put(
        output,
        DOMAIN_LIABILITY_BASIS_ID_OFFSET,
        &input.liability_basis_id.to_bytes(),
    )?;
    put(
        output,
        DOMAIN_REPRESENTATION_RELEASE_ID_OFFSET,
        &input.representation_release_id.to_bytes(),
    )?;
    put(
        output,
        DOMAIN_MAPPING_RELEASE_ID_OFFSET,
        &input.mapping_release_id.to_bytes(),
    )?;
    put(
        output,
        DOMAIN_CUT_DENOMINATOR_OFFSET,
        &input.cut_denominator.to_le_bytes(),
    )?;
    for (index, cut) in input.cuts.iter().copied().enumerate() {
        let offset = DOMAIN_HEADER_BYTES
            .checked_add(
                index
                    .checked_mul(DOMAIN_CUT_BYTES)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        put(output, offset, &cut.to_le_bytes())?;
    }
    ResultDomainV2::decode(output)?;
    Ok(())
}

/// Compile and gcd-normalize a runtime-width rational portfolio.
///
/// Validation and normalization are complete before output mutation. The
/// exact final-floor tag is persisted in the record.
pub fn compile_portfolio_v2(input: PortfolioInputV2<'_>, output: &mut [u8]) -> Result<()> {
    if input.denominator == 0 {
        return Err(Error::ZeroDenominator);
    }
    if input.coefficients.is_empty()
        || input
            .coefficients
            .iter()
            .all(|coefficient| *coefficient == 0)
    {
        return Err(Error::EmptyPortfolio);
    }
    let coefficient_count =
        u32::try_from(input.coefficients.len()).map_err(|_| Error::ArithmeticOverflow)?;
    let expected = portfolio_record_bytes(input.coefficients.len())?;
    if output.len() != expected {
        return Err(Error::OutputLength);
    }
    let record_bytes = u32::try_from(expected).map_err(|_| Error::ArithmeticOverflow)?;
    let mut divisor = input.denominator;
    for coefficient in input.coefficients {
        divisor = gcd(divisor, *coefficient);
    }
    let denominator = input
        .denominator
        .checked_div(divisor)
        .ok_or(Error::ArithmeticOverflow)?;
    output.fill(0);
    put(output, 0, &PORTFOLIO_MAGIC)?;
    put(output, 8, &PRODUCT_RUNTIME_V2_SCHEMA_VERSION.to_le_bytes())?;
    put(
        output,
        HEADER_BYTES_OFFSET,
        &u16::try_from(PORTFOLIO_HEADER_BYTES)
            .map_err(|_| Error::ArithmeticOverflow)?
            .to_le_bytes(),
    )?;
    put(output, RECORD_BYTES_OFFSET, &record_bytes.to_le_bytes())?;
    put(
        output,
        PORTFOLIO_COEFFICIENT_COUNT_OFFSET,
        &coefficient_count.to_le_bytes(),
    )?;
    put(
        output,
        PORTFOLIO_ROUNDING_OFFSET,
        &[REPRESENTATION_FLOOR_TAG],
    )?;
    put(
        output,
        PORTFOLIO_PRODUCT_ID_OFFSET,
        &input.product_id.to_bytes(),
    )?;
    put(
        output,
        PORTFOLIO_RESULT_DOMAIN_ID_OFFSET,
        &input.result_domain_id.to_bytes(),
    )?;
    put(
        output,
        PORTFOLIO_CLAIM_BASIS_ID_OFFSET,
        &input.claim_basis_id.to_bytes(),
    )?;
    put(
        output,
        PORTFOLIO_LIABILITY_BASIS_ID_OFFSET,
        &input.liability_basis_id.to_bytes(),
    )?;
    put(
        output,
        PORTFOLIO_REPRESENTATION_RELEASE_ID_OFFSET,
        &input.representation_release_id.to_bytes(),
    )?;
    put(
        output,
        PORTFOLIO_DENOMINATOR_OFFSET,
        &denominator.to_le_bytes(),
    )?;
    for (index, coefficient) in input.coefficients.iter().copied().enumerate() {
        let normalized = coefficient
            .checked_div(divisor)
            .ok_or(Error::ArithmeticOverflow)?;
        let offset = PORTFOLIO_HEADER_BYTES
            .checked_add(
                index
                    .checked_mul(PORTFOLIO_COEFFICIENT_BYTES)
                    .ok_or(Error::ArithmeticOverflow)?,
            )
            .ok_or(Error::ArithmeticOverflow)?;
        put(output, offset, &normalized.to_le_bytes())?;
    }
    PortfolioV2::decode(output)?;
    Ok(())
}

fn compare_signed_rational(
    left: i128,
    left_denominator: u64,
    right: i128,
    right_denominator: u64,
) -> Ordering {
    match (left.is_negative(), right.is_negative()) {
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => compare_unsigned_rational(
            left.unsigned_abs(),
            left_denominator,
            right.unsigned_abs(),
            right_denominator,
        ),
        (true, true) => compare_unsigned_rational(
            left.unsigned_abs(),
            left_denominator,
            right.unsigned_abs(),
            right_denominator,
        )
        .reverse(),
    }
}

fn compare_unsigned_rational(
    left: u128,
    left_denominator: u64,
    right: u128,
    right_denominator: u64,
) -> Ordering {
    let left_denominator = u128::from(left_denominator);
    let right_denominator = u128::from(right_denominator);
    let left_quotient = left / left_denominator;
    let right_quotient = right / right_denominator;
    match left_quotient.cmp(&right_quotient) {
        Ordering::Equal => {
            let left_remainder = left % left_denominator;
            let right_remainder = right % right_denominator;
            (left_remainder * right_denominator).cmp(&(right_remainder * left_denominator))
        }
        other => other,
    }
}

fn record_len(header: usize, count: u32, item_bytes: usize) -> Result<usize> {
    usize::try_from(count)
        .ok()
        .and_then(|width| width.checked_mul(item_bytes))
        .and_then(|tail| header.checked_add(tail))
        .ok_or(Error::ArithmeticOverflow)
}

fn validate_common_header(bytes: &[u8], expected_header: usize) -> Result<()> {
    if read_u16(bytes, 8)? != PRODUCT_RUNTIME_V2_SCHEMA_VERSION
        || usize::from(read_u16(bytes, HEADER_BYTES_OFFSET)?) != expected_header
        || usize::try_from(read_u32(bytes, RECORD_BYTES_OFFSET)?)
            .map_err(|_| Error::InvalidLength)?
            != bytes.len()
    {
        return Err(Error::UnsupportedSchema);
    }
    Ok(())
}

fn read_id(bytes: &[u8], offset: usize) -> Result<ContentId> {
    ContentId::new(array(bytes, offset)?)
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes.get(offset).copied().ok_or(Error::InvalidLength)
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    let end = offset.checked_add(N).ok_or(Error::ArithmeticOverflow)?;
    bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::ArithmeticOverflow)?;
    let destination = output.get_mut(offset..end).ok_or(Error::OutputLength)?;
    destination.copy_from_slice(value);
    Ok(())
}

fn require_zero(bytes: &[u8], offset: usize, length: usize) -> Result<()> {
    let end = offset
        .checked_add(length)
        .ok_or(Error::ArithmeticOverflow)?;
    if bytes
        .get(offset..end)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonicalReserved);
    }
    Ok(())
}

fn gcd(mut left: u64, mut right: u64) -> u64 {
    while right != 0 {
        let remainder = left % right;
        left = right;
        right = remainder;
    }
    left
}
