//! Runtime-width Fractional terms over a K-dimensional Claims representation.
//!
//! Product terminal semantics have width `N`; shard Mints and Claims reserve
//! coordinates have width `K`. The finalized composition exposure is the sole
//! N→K translation. This module never accepts a caller-authored matrix or
//! payout and retains the V1 quotient/remainder rule exactly.

use core::convert::TryInto;

use dclutch_representation_composition_v3_kernel::{
    CompositionExposureBundleV3, CompositionExposureExpectedV3, Error as CompositionError,
};

use crate::{Error, Result};

/// Fixed header before the ordered `K` shard Mints.
pub const FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2: usize = 384;
/// Width of one ordered shard Mint.
pub const FRACTIONAL_EXPOSURE_TERMS_MINT_BYTES_V2: usize = 32;
/// Exact V2 terms magic.
pub const FRACTIONAL_EXPOSURE_TERMS_MAGIC_V2: [u8; 8] = *b"DCFREX02";
/// V2 terms schema preimage.
pub const FRACTIONAL_EXPOSURE_TERMS_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/fractional-exposure-terms-v2|header384|K-mints32|productN1..512|claimsK1..256|exact-denominator|exposure-bound";
/// SHA-256 of [`FRACTIONAL_EXPOSURE_TERMS_SCHEMA_PREIMAGE_V2`].
pub const FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2: [u8; 32] = [
    0x99, 0x44, 0xc3, 0x34, 0x05, 0x45, 0x17, 0x7f, 0x08, 0x71, 0xd8, 0xcb, 0x2f, 0x73, 0x70, 0x3c,
    0xd8, 0xf1, 0x30, 0x0b, 0x19, 0x5d, 0xc4, 0xdb, 0x9f, 0x92, 0x44, 0xd3, 0x9b, 0xe0, 0x2a, 0x61,
];

const VERSION_V2: u16 = 2;
const MARKET_OFFSET: usize = 16;
const PRODUCT_RECORD_OFFSET: usize = 48;
const RESULT_DOMAIN_OFFSET: usize = 80;
const RELEASE_SET_OFFSET: usize = 112;
const TOKEN_PROGRAM_OFFSET: usize = 144;
const TOKEN_BEHAVIOR_OFFSET: usize = 176;
const EXPOSURE_ID_OFFSET: usize = 208;
const PRODUCT_BASIS_OFFSET: usize = 240;
const REPRESENTATION_BASIS_OFFSET: usize = 272;
const GRAPH_ID_OFFSET: usize = 304;
const PRODUCT_WIDTH_OFFSET: usize = 336;
const REPRESENTATION_WIDTH_OFFSET: usize = 340;
const DENOMINATOR_OFFSET: usize = 344;
const RESERVED_TAIL_OFFSET: usize = 352;

/// Finalized-record admission for V2 terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureTermsAdmissionV2 {
    /// Descriptor-selected terms schema.
    pub selected_schema_id: [u8; 32],
    /// Finalized Record schema.
    pub finalized_schema_id: [u8; 32],
    /// Descriptor-selected content identity.
    pub selected_terms_id: [u8; 32],
    /// Finalized Record content identity.
    pub finalized_terms_id: [u8; 32],
    /// Digest recomputed over exact bytes by the adapter.
    pub recomputed_terms_digest: [u8; 32],
    /// Digest committed by the finalized Record.
    pub finalized_terms_digest: [u8; 32],
    /// Owner/PDA/staging/rent authentication completed.
    pub record_authenticated: bool,
}

/// Borrowed exact V2 terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureTermsV2<'a> {
    terms_id: [u8; 32],
    market: [u8; 32],
    product_record: [u8; 32],
    result_domain: [u8; 32],
    release_set: [u8; 32],
    token_program: [u8; 32],
    token_behavior: [u8; 32],
    exposure_id: [u8; 32],
    product_basis: [u8; 32],
    representation_basis: [u8; 32],
    graph_id: [u8; 32],
    product_width: u32,
    representation_width: u32,
    denominator: u64,
    shard_mints: &'a [u8],
}

impl<'a> FractionalExposureTermsV2<'a> {
    /// Hostile-decode exact V2 terms after finalized Record admission.
    pub fn decode(input: &'a [u8], admission: FractionalExposureTermsAdmissionV2) -> Result<Self> {
        if input.len() < FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if array::<8>(input, 0)? != FRACTIONAL_EXPOSURE_TERMS_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if u16_at(input, 8)? != VERSION_V2 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, 10, 6)?;
        require_zero(input, RESERVED_TAIL_OFFSET, 32)?;
        if !admission.record_authenticated {
            return Err(Error::UnauthenticatedRecord);
        }
        if admission.selected_schema_id != FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2
            || admission.finalized_schema_id != FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2
            || admission.selected_terms_id == [0; 32]
            || admission.selected_terms_id != admission.finalized_terms_id
            || admission.selected_terms_id != admission.recomputed_terms_digest
            || admission.selected_terms_id != admission.finalized_terms_digest
        {
            return Err(Error::AdmissionMismatch);
        }
        let product_width = u32_at(input, PRODUCT_WIDTH_OFFSET)?;
        let representation_width = u32_at(input, REPRESENTATION_WIDTH_OFFSET)?;
        if product_width == 0
            || product_width > 512
            || representation_width == 0
            || representation_width > 256
        {
            return Err(Error::InvalidOutcome);
        }
        let denominator = u64_at(input, DENOMINATOR_OFFSET)?;
        if denominator <= 1 {
            return Err(Error::NonFractionalDenominator);
        }
        let mint_bytes = usize::try_from(representation_width)
            .map_err(|_| Error::InvalidLength)?
            .checked_mul(FRACTIONAL_EXPOSURE_TERMS_MINT_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        let exact = FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2
            .checked_add(mint_bytes)
            .ok_or(Error::InvalidLength)?;
        if input.len() != exact {
            return Err(Error::InvalidLength);
        }
        let shard_mints = input
            .get(FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2..)
            .ok_or(Error::InvalidLength)?;
        validate_mints(shard_mints, representation_width)?;
        let value = Self {
            terms_id: admission.selected_terms_id,
            market: nonzero(input, MARKET_OFFSET)?,
            product_record: nonzero(input, PRODUCT_RECORD_OFFSET)?,
            result_domain: nonzero(input, RESULT_DOMAIN_OFFSET)?,
            release_set: nonzero(input, RELEASE_SET_OFFSET)?,
            token_program: nonzero(input, TOKEN_PROGRAM_OFFSET)?,
            token_behavior: nonzero(input, TOKEN_BEHAVIOR_OFFSET)?,
            exposure_id: nonzero(input, EXPOSURE_ID_OFFSET)?,
            product_basis: nonzero(input, PRODUCT_BASIS_OFFSET)?,
            representation_basis: nonzero(input, REPRESENTATION_BASIS_OFFSET)?,
            graph_id: nonzero(input, GRAPH_ID_OFFSET)?,
            product_width,
            representation_width,
            denominator,
            shard_mints,
        };
        Ok(value)
    }

    /// Finalized terms identity.
    pub const fn terms_id(self) -> [u8; 32] {
        self.terms_id
    }
    /// Logical Market.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Finalized Product root digest.
    pub const fn product_record(self) -> [u8; 32] {
        self.product_record
    }
    /// Product-owned result domain.
    pub const fn result_domain(self) -> [u8; 32] {
        self.result_domain
    }
    /// Immutable release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set
    }
    /// Selected Token program.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }
    /// Finalized TokenBehaviorV2 identity.
    pub const fn token_behavior(self) -> [u8; 32] {
        self.token_behavior
    }
    /// Finalized N→K exposure identity.
    pub const fn exposure_id(self) -> [u8; 32] {
        self.exposure_id
    }
    /// Product terminal-result basis.
    pub const fn product_basis(self) -> [u8; 32] {
        self.product_basis
    }
    /// Claims representation basis.
    pub const fn representation_basis(self) -> [u8; 32] {
        self.representation_basis
    }
    /// Stable source graph identity.
    pub const fn graph_id(self) -> [u8; 32] {
        self.graph_id
    }
    /// Product terminal-result width `N`.
    pub const fn product_width(self) -> u32 {
        self.product_width
    }
    /// Claims/shard representation width `K`.
    pub const fn representation_width(self) -> u32 {
        self.representation_width
    }
    /// Exact shard atoms per whole Claims coordinate.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Terms-owned Mint for one Claims representation coordinate.
    pub fn shard_mint(self, coordinate: u32) -> Result<[u8; 32]> {
        if coordinate >= self.representation_width {
            return Err(Error::InvalidOutcome);
        }
        let offset = usize::try_from(coordinate)
            .map_err(|_| Error::InvalidOutcome)?
            .checked_mul(FRACTIONAL_EXPOSURE_TERMS_MINT_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        array(self.shard_mints, offset)
    }
}

/// Atomic encoder input for V2 terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureTermsInputV2<'a> {
    /// Logical Market.
    pub market: [u8; 32],
    /// Finalized Product root digest.
    pub product_record: [u8; 32],
    /// Product-owned result domain.
    pub result_domain: [u8; 32],
    /// Immutable release set.
    pub release_set: [u8; 32],
    /// Selected Token program.
    pub token_program: [u8; 32],
    /// Finalized TokenBehaviorV2 identity.
    pub token_behavior: [u8; 32],
    /// Finalized N→K exposure identity.
    pub exposure_id: [u8; 32],
    /// Product terminal-result basis.
    pub product_basis: [u8; 32],
    /// Claims representation basis.
    pub representation_basis: [u8; 32],
    /// Stable source graph identity.
    pub graph_id: [u8; 32],
    /// Product terminal-result width `N`.
    pub product_width: u32,
    /// Exact shard atoms per whole Claims coordinate.
    pub denominator: u64,
    /// Exactly `K` ordered, unique shard Mints.
    pub shard_mints: &'a [[u8; 32]],
}

/// Return the exact encoded terms width for `K`.
pub fn fractional_exposure_terms_bytes_v2(representation_width: usize) -> Result<usize> {
    if representation_width == 0 || representation_width > 256 {
        return Err(Error::InvalidOutcome);
    }
    FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2
        .checked_add(
            representation_width
                .checked_mul(FRACTIONAL_EXPOSURE_TERMS_MINT_BYTES_V2)
                .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)
}

/// Encode V2 terms atomically into equal-width caller buffers.
pub fn encode_fractional_exposure_terms_v2(
    input: FractionalExposureTermsInputV2<'_>,
    scratch: &mut [u8],
    output: &mut [u8],
) -> Result<()> {
    let width = fractional_exposure_terms_bytes_v2(input.shard_mints.len())?;
    if scratch.len() != width
        || output.len() != width
        || input.product_width == 0
        || input.product_width > 512
        || input.denominator <= 1
    {
        return Err(Error::InvalidLength);
    }
    for value in [
        input.market,
        input.product_record,
        input.result_domain,
        input.release_set,
        input.token_program,
        input.token_behavior,
        input.exposure_id,
        input.product_basis,
        input.representation_basis,
        input.graph_id,
    ] {
        if value == [0; 32] {
            return Err(Error::ZeroIdentity);
        }
    }
    for (index, mint) in input.shard_mints.iter().enumerate() {
        if *mint == [0; 32]
            || input
                .shard_mints
                .get(..index)
                .is_some_and(|prior| prior.contains(mint))
        {
            return Err(if *mint == [0; 32] {
                Error::ZeroIdentity
            } else {
                Error::DuplicateShardMint
            });
        }
    }
    scratch.fill(0);
    put(scratch, 0, &FRACTIONAL_EXPOSURE_TERMS_MAGIC_V2)?;
    put(scratch, 8, &VERSION_V2.to_le_bytes())?;
    for (offset, value) in [
        (MARKET_OFFSET, input.market),
        (PRODUCT_RECORD_OFFSET, input.product_record),
        (RESULT_DOMAIN_OFFSET, input.result_domain),
        (RELEASE_SET_OFFSET, input.release_set),
        (TOKEN_PROGRAM_OFFSET, input.token_program),
        (TOKEN_BEHAVIOR_OFFSET, input.token_behavior),
        (EXPOSURE_ID_OFFSET, input.exposure_id),
        (PRODUCT_BASIS_OFFSET, input.product_basis),
        (REPRESENTATION_BASIS_OFFSET, input.representation_basis),
        (GRAPH_ID_OFFSET, input.graph_id),
    ] {
        put(scratch, offset, &value)?;
    }
    put(
        scratch,
        PRODUCT_WIDTH_OFFSET,
        &input.product_width.to_le_bytes(),
    )?;
    put(
        scratch,
        REPRESENTATION_WIDTH_OFFSET,
        &u32::try_from(input.shard_mints.len())
            .map_err(|_| Error::InvalidOutcome)?
            .to_le_bytes(),
    )?;
    put(
        scratch,
        DENOMINATOR_OFFSET,
        &input.denominator.to_le_bytes(),
    )?;
    for (index, mint) in input.shard_mints.iter().enumerate() {
        let offset = FRACTIONAL_EXPOSURE_TERMS_HEADER_BYTES_V2
            .checked_add(index.checked_mul(32).ok_or(Error::InvalidLength)?)
            .ok_or(Error::InvalidLength)?;
        put(scratch, offset, mint)?;
    }
    output.copy_from_slice(scratch);
    Ok(())
}

/// Bind exact V2 terms to the sole finalized N→K exposure.
pub fn check_fractional_exposure_bundle_v2<'a>(
    terms: FractionalExposureTermsV2<'_>,
    bundle: CompositionExposureBundleV3<'a>,
) -> Result<CompositionExposureBundleV3<'a>> {
    if bundle.bundle_id() != terms.exposure_id() {
        return Err(Error::AdmissionMismatch);
    }
    bundle
        .verify_for(CompositionExposureExpectedV3 {
            market: terms.market(),
            result_domain: terms.result_domain(),
            release_set: terms.release_set(),
            product_basis: terms.product_basis(),
            representation_basis: terms.representation_basis(),
            graph_id: terms.graph_id(),
            product_width: terms.product_width(),
            representation_width: terms.representation_width(),
        })
        .map_err(|_| Error::AdmissionMismatch)
}

/// Require the explicit categorical embedding: `K=N` and every row is one-hot.
pub fn require_categorical_embedding_v2(
    terms: FractionalExposureTermsV2<'_>,
    bundle: CompositionExposureBundleV3<'_>,
) -> Result<()> {
    let bundle = check_fractional_exposure_bundle_v2(terms, bundle)?;
    if terms.product_width() != terms.representation_width() {
        return Err(Error::AdmissionMismatch);
    }
    let mut coordinate = 0_u32;
    while coordinate < terms.representation_width() {
        let row = bundle
            .row(coordinate)
            .map_err(|_| Error::AdmissionMismatch)?;
        let term = bundle
            .row_term(row, 0)
            .map_err(|_| Error::AdmissionMismatch)?;
        if row.representation_coordinate() != coordinate
            || row.denominator() != 1
            || row.term_count() != 1
            || term.product_coordinate != coordinate
            || term.numerator != 1
        {
            return Err(Error::AdmissionMismatch);
        }
        coordinate = coordinate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

/// Exact Token-owned shard instrument for one Claims coordinate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExposureShardInstrumentV2 {
    /// Finalized V2 terms identity.
    pub terms_id: [u8; 32],
    /// Claims representation coordinate in `[0,K)`.
    pub representation_coordinate: u32,
    /// Terms-owned same-Mint identity.
    pub shard_mint: [u8; 32],
    /// Exact raw Token base units.
    pub shard_atoms: u64,
}

/// Sole exact quotient/remainder boundary for one K-coordinate shard input.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExposureShardDivisionV2 {
    /// Full same-Mint input.
    pub input: ExposureShardInstrumentV2,
    /// Whole Claims-coordinate quantity.
    pub whole_claims: u64,
    /// Exact denominator multiple burned.
    pub consumed: ExposureShardInstrumentV2,
    /// Explicit same-Mint Token-owned remainder.
    pub change: ExposureShardInstrumentV2,
}

/// Divide exact shard atoms without rounding or change minting.
pub fn divide_exposure_shards_v2(
    terms: FractionalExposureTermsV2<'_>,
    coordinate: u32,
    shard_atoms: u64,
) -> Result<ExposureShardDivisionV2> {
    if shard_atoms == 0 {
        return Err(Error::ZeroQuantity);
    }
    let denominator = terms.denominator();
    let whole_claims = shard_atoms / denominator;
    if whole_claims == 0 {
        return Err(Error::NoWholeClaim);
    }
    let consumed_atoms = whole_claims
        .checked_mul(denominator)
        .ok_or(Error::ArithmeticOverflow)?;
    let change_atoms = shard_atoms
        .checked_sub(consumed_atoms)
        .ok_or(Error::ArithmeticOverflow)?;
    let mint = terms.shard_mint(coordinate)?;
    let instrument = |atoms| ExposureShardInstrumentV2 {
        terms_id: terms.terms_id(),
        representation_coordinate: coordinate,
        shard_mint: mint,
        shard_atoms: atoms,
    };
    Ok(ExposureShardDivisionV2 {
        input: instrument(shard_atoms),
        whole_claims,
        consumed: instrument(consumed_atoms),
        change: instrument(change_atoms),
    })
}

/// Exact terminal payout for one whole K-coordinate quantity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExposureTerminalPlanV2 {
    /// Sole quotient/remainder result.
    pub division: ExposureShardDivisionV2,
    /// Exact collateral atoms per whole Claims coordinate.
    pub collateral_atoms_per_claim: u64,
    /// Exact total payout; zero is an explicit valid result.
    pub collateral_atoms: u64,
}

/// Caller-owned runtime-width workspace with a commit-last translation output.
pub struct ExposureTranslationBuffersV2<'a> {
    /// Scratch mutated while checking every exposure row.
    pub scratch: &'a mut [u64],
    /// Complete candidate mutated only after all rows translate exactly.
    pub candidate: &'a mut [u64],
    /// Public output copied only after translation and payout multiplication succeed.
    pub output: &'a mut [u64],
}

/// Translate authenticated Product payouts N→K, then evaluate one coordinate.
///
/// `product_payouts` must be produced by the authenticated Product terminal
/// evaluator. The exposure kernel atomically refuses nonintegral translation
/// and never rounds.
pub fn evaluate_exposure_terminal_v2(
    terms: FractionalExposureTermsV2<'_>,
    bundle: CompositionExposureBundleV3<'_>,
    product_payouts: &[u64],
    representation_coordinate: u32,
    shard_atoms: u64,
    buffers: ExposureTranslationBuffersV2<'_>,
) -> Result<ExposureTerminalPlanV2> {
    let bundle = check_fractional_exposure_bundle_v2(terms, bundle)?;
    let division = divide_exposure_shards_v2(terms, representation_coordinate, shard_atoms)?;
    bundle
        .translate_product_payouts(product_payouts, buffers.scratch, buffers.candidate)
        .map_err(map_composition_error)?;
    if buffers.candidate.len() != buffers.output.len() {
        return Err(Error::InvalidLength);
    }
    let per_claim = *buffers
        .candidate
        .get(usize::try_from(representation_coordinate).map_err(|_| Error::InvalidOutcome)?)
        .ok_or(Error::InvalidOutcome)?;
    let collateral_atoms = division
        .whole_claims
        .checked_mul(per_claim)
        .ok_or(Error::ArithmeticOverflow)?;
    buffers.output.copy_from_slice(buffers.candidate);
    Ok(ExposureTerminalPlanV2 {
        division,
        collateral_atoms_per_claim: per_claim,
        collateral_atoms,
    })
}

const fn map_composition_error(error: CompositionError) -> Error {
    match error {
        CompositionError::InvalidLength => Error::InvalidLength,
        CompositionError::InvalidOutcome => Error::InvalidOutcome,
        CompositionError::ArithmeticOverflow => Error::ArithmeticOverflow,
        CompositionError::NonIntegralTranslation => Error::NonIntegralTranslation,
        _ => Error::AdmissionMismatch,
    }
}

fn validate_mints(bytes: &[u8], count: u32) -> Result<()> {
    let mut index = 0_u32;
    while index < count {
        let current = usize::try_from(index)
            .map_err(|_| Error::InvalidLength)?
            .checked_mul(32)
            .ok_or(Error::InvalidLength)?;
        let mint = array::<32>(bytes, current)?;
        if mint == [0; 32] {
            return Err(Error::ZeroIdentity);
        }
        let mut prior = 0_u32;
        while prior < index {
            let offset = usize::try_from(prior)
                .map_err(|_| Error::InvalidLength)?
                .checked_mul(32)
                .ok_or(Error::InvalidLength)?;
            if array::<32>(bytes, offset)? == mint {
                return Err(Error::DuplicateShardMint);
            }
            prior = prior.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        index = index.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn nonzero(input: &[u8], offset: usize) -> Result<[u8; 32]> {
    let value = array(input, offset)?;
    if value == [0; 32] {
        Err(Error::ZeroIdentity)
    } else {
        Ok(value)
    }
}
fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}
fn u16_at(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(input, offset)?))
}
fn u32_at(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(input, offset)?))
}
fn u64_at(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(input, offset)?))
}
fn require_zero(input: &[u8], offset: usize, len: usize) -> Result<()> {
    if input
        .get(offset..offset.checked_add(len).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(Error::NonCanonical)
    } else {
        Ok(())
    }
}
fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}
