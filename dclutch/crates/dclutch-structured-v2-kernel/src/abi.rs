//! Hostile decoding of Structured V2 terms and the adapter-owned projection.

use core::convert::TryInto;

use dclutch_fractional_claim_kernel::FractionalExposureTermsV2;

pub use crate::generated_abi::{
    STRUCTURED_CAPABILITY_KIND_ID_V2, STRUCTURED_CAPABILITY_KIND_PREIMAGE_V2,
    STRUCTURED_CAPACITY_PROFILE_ID_V2, STRUCTURED_CAPACITY_PROFILE_PREIMAGE_V2,
    STRUCTURED_MAX_COORDINATES_V2, STRUCTURED_MIN_COORDINATES_V2, STRUCTURED_MIN_DENOMINATOR_V2,
    STRUCTURED_NO_COORDINATE_V2, STRUCTURED_PHASE_OPEN_V2, STRUCTURED_PHASE_RETIRED_V2,
    STRUCTURED_PHASE_TERMINAL_V2, STRUCTURED_PROJECTION_HEADER_BYTES_V2,
    STRUCTURED_PROJECTION_MAGIC_V2, STRUCTURED_PROJECTION_ROW_BYTES_V2,
    STRUCTURED_RECEIPT_DECIMALS_V2, STRUCTURED_SCHEMA_VERSION_V2,
    STRUCTURED_TERMS_COEFFICIENT_BYTES_V2, STRUCTURED_TERMS_HEADER_BYTES_V2,
    STRUCTURED_TERMS_MAGIC_V2, STRUCTURED_TERMS_SCHEMA_ID_V2, STRUCTURED_TERMS_SCHEMA_PREIMAGE_V2,
};
use crate::generated_abi::{
    STRUCTURED_PROJECTION_DENOMINATOR_OFFSET_V2, STRUCTURED_PROJECTION_MAGIC_OFFSET_V2,
    STRUCTURED_PROJECTION_MARKET_OFFSET_V2, STRUCTURED_PROJECTION_PHASE_OFFSET_V2,
    STRUCTURED_PROJECTION_RECEIPT_SUPPLY_OFFSET_V2,
    STRUCTURED_PROJECTION_REPRESENTATION_WIDTH_OFFSET_V2,
    STRUCTURED_PROJECTION_RESERVED_HEADER_OFFSET_V2, STRUCTURED_PROJECTION_RESERVED_TAIL_OFFSET_V2,
    STRUCTURED_PROJECTION_RESERVED_WIDTH_OFFSET_V2, STRUCTURED_PROJECTION_REVISION_OFFSET_V2,
    STRUCTURED_PROJECTION_ROW_CUSTODY_OFFSET_V2, STRUCTURED_PROJECTION_ROW_PAYOUT_OFFSET_V2,
    STRUCTURED_PROJECTION_SHARD_TERMS_OFFSET_V2, STRUCTURED_PROJECTION_TERMS_OFFSET_V2,
    STRUCTURED_PROJECTION_VERSION_OFFSET_V2, STRUCTURED_TERMS_DENOMINATOR_OFFSET_V2,
    STRUCTURED_TERMS_GRAPH_ID_OFFSET_V2, STRUCTURED_TERMS_MAGIC_OFFSET_V2,
    STRUCTURED_TERMS_MARKET_OFFSET_V2, STRUCTURED_TERMS_PRODUCT_RECORD_OFFSET_V2,
    STRUCTURED_TERMS_RECEIPT_DECIMALS_OFFSET_V2, STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2,
    STRUCTURED_TERMS_RELEASE_SET_OFFSET_V2, STRUCTURED_TERMS_REPRESENTATION_WIDTH_OFFSET_V2,
    STRUCTURED_TERMS_RESERVED_HEADER_OFFSET_V2, STRUCTURED_TERMS_RESERVED_TAIL_OFFSET_V2,
    STRUCTURED_TERMS_RESERVED_WIDTH_OFFSET_V2, STRUCTURED_TERMS_RESULT_DOMAIN_OFFSET_V2,
    STRUCTURED_TERMS_SHARD_EXPOSURE_OFFSET_V2, STRUCTURED_TERMS_SHARD_TERMS_OFFSET_V2,
    STRUCTURED_TERMS_TOKEN_BEHAVIOR_OFFSET_V2, STRUCTURED_TERMS_TOKEN_PROGRAM_OFFSET_V2,
    STRUCTURED_TERMS_VERSION_OFFSET_V2,
};

const RESERVED_HEADER_BYTES: usize = 5;
const RESERVED_WIDTH_BYTES: usize = 4;
const TERMS_RESERVED_TAIL_BYTES: usize = 32;
const PROJECTION_RESERVED_TAIL_BYTES: usize = 16;

/// Stable hostile-decode or exact-transition refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed or runtime-derived byte width differed.
    InvalidLength,
    /// Magic bytes selected another schema family.
    InvalidMagic,
    /// The schema version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes, a phase tag, decimals, or an inert field were noncanonical.
    NonCanonical,
    /// A required content, Market, program, Mint, or graph identity was zero.
    ZeroIdentity,
    /// Two immutable identities that must differ aliased each other.
    DuplicateIdentity,
    /// The representation width was zero, beyond the capacity profile, or out of range.
    InvalidCoordinate,
    /// Exact shard backing requires a denominator greater than one.
    NonFractionalDenominator,
    /// Selected, finalized, recomputed, or projected immutable identities differed.
    AdmissionMismatch,
    /// Finalized Record ownership, PDA, vacancy, digest, or rent was not authenticated.
    UnauthenticatedRecord,
    /// The observed shard layer is not the one this immutable basis selected.
    ShardLayerMismatch,
    /// Every coefficient was zero, so a receipt atom would denote no exposure.
    UnbackedBasis,
    /// A checked byte width, sum, product, subtraction, or revision overflowed.
    ArithmeticOverflow,
    /// Observed shard custody could not cover the exact required backing.
    BackingMismatch,
    /// The requested action is not admitted in the observed lifecycle phase.
    InvalidPhase,
    /// An exact state-changing action carried zero receipt atoms.
    ZeroQuantity,
    /// An observed holder balance exceeded supply or could not fund the action.
    InsufficientBalance,
    /// Receipt supply remained when the node was asked to retire.
    OutstandingReceiptSupply,
    /// Observed shard custody, including donated surplus, remained at retirement.
    OutstandingShardCustody,
}

/// Result alias for this total kernel.
pub type Result<T> = core::result::Result<T, Error>;

/// Finalized immutable-terms authentication supplied by the Record adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredTermsAdmissionV2 {
    /// Terms schema selected by the immutable capability descriptor.
    pub selected_schema_id: [u8; 32],
    /// Terms schema observed in finalized Record coordinates.
    pub finalized_schema_id: [u8; 32],
    /// Terms identity selected by the immutable capability descriptor.
    pub selected_terms_id: [u8; 32],
    /// Terms identity observed in the finalized Record coordinates.
    pub finalized_terms_id: [u8; 32],
    /// SHA-256 recomputed over the exact terms bytes by the outer adapter.
    pub recomputed_terms_digest: [u8; 32],
    /// Digest committed by the finalized Record identity.
    pub finalized_terms_digest: [u8; 32],
    /// Finalized owner/PDA, vacant staging PDA, digest, and rent were authenticated.
    pub record_authenticated: bool,
}

/// Hostile-decoded immutable Structured V2 terms.
///
/// The denominator and representation width are owned by the exact claim-shard
/// terms; they are restated here only so every join is self-authenticating, and
/// [`StructuredTermsV2::decode`] refuses when they disagree.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredTermsV2<'a> {
    terms_id: [u8; 32],
    market: [u8; 32],
    product_record: [u8; 32],
    result_domain: [u8; 32],
    release_set: [u8; 32],
    token_program: [u8; 32],
    token_behavior: [u8; 32],
    shard_terms: [u8; 32],
    shard_exposure: [u8; 32],
    receipt_mint: [u8; 32],
    graph_id: [u8; 32],
    representation_width: u32,
    denominator: u64,
    coefficients: &'a [u8],
}

impl<'a> StructuredTermsV2<'a> {
    /// Decode exact immutable bytes after joining the finalized Record identities
    /// and the independently authenticated exact claim-shard terms.
    pub fn decode(
        input: &'a [u8],
        admission: StructuredTermsAdmissionV2,
        shard_terms: FractionalExposureTermsV2<'_>,
    ) -> Result<Self> {
        if input.len() < STRUCTURED_TERMS_HEADER_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if bytes::<8>(input, STRUCTURED_TERMS_MAGIC_OFFSET_V2)? != STRUCTURED_TERMS_MAGIC_V2 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, STRUCTURED_TERMS_VERSION_OFFSET_V2)? != STRUCTURED_SCHEMA_VERSION_V2 {
            return Err(Error::UnsupportedVersion);
        }
        if byte(input, STRUCTURED_TERMS_RECEIPT_DECIMALS_OFFSET_V2)?
            != STRUCTURED_RECEIPT_DECIMALS_V2
        {
            return Err(Error::NonCanonical);
        }
        require_zero(
            input,
            STRUCTURED_TERMS_RESERVED_HEADER_OFFSET_V2,
            RESERVED_HEADER_BYTES,
        )?;
        require_zero(
            input,
            STRUCTURED_TERMS_RESERVED_WIDTH_OFFSET_V2,
            RESERVED_WIDTH_BYTES,
        )?;
        require_zero(
            input,
            STRUCTURED_TERMS_RESERVED_TAIL_OFFSET_V2,
            TERMS_RESERVED_TAIL_BYTES,
        )?;
        if !admission.record_authenticated {
            return Err(Error::UnauthenticatedRecord);
        }
        if admission.selected_schema_id != STRUCTURED_TERMS_SCHEMA_ID_V2
            || admission.finalized_schema_id != STRUCTURED_TERMS_SCHEMA_ID_V2
            || is_zero(&admission.selected_terms_id)
            || admission.selected_terms_id != admission.finalized_terms_id
            || admission.selected_terms_id != admission.recomputed_terms_digest
            || admission.selected_terms_id != admission.finalized_terms_digest
        {
            return Err(Error::AdmissionMismatch);
        }
        let market = bytes::<32>(input, STRUCTURED_TERMS_MARKET_OFFSET_V2)?;
        let product_record = bytes::<32>(input, STRUCTURED_TERMS_PRODUCT_RECORD_OFFSET_V2)?;
        let result_domain = bytes::<32>(input, STRUCTURED_TERMS_RESULT_DOMAIN_OFFSET_V2)?;
        let release_set = bytes::<32>(input, STRUCTURED_TERMS_RELEASE_SET_OFFSET_V2)?;
        let token_program = bytes::<32>(input, STRUCTURED_TERMS_TOKEN_PROGRAM_OFFSET_V2)?;
        let token_behavior = bytes::<32>(input, STRUCTURED_TERMS_TOKEN_BEHAVIOR_OFFSET_V2)?;
        let shard_terms_id = bytes::<32>(input, STRUCTURED_TERMS_SHARD_TERMS_OFFSET_V2)?;
        let shard_exposure = bytes::<32>(input, STRUCTURED_TERMS_SHARD_EXPOSURE_OFFSET_V2)?;
        let receipt_mint = bytes::<32>(input, STRUCTURED_TERMS_RECEIPT_MINT_OFFSET_V2)?;
        let graph_id = bytes::<32>(input, STRUCTURED_TERMS_GRAPH_ID_OFFSET_V2)?;
        let identities = [
            market,
            product_record,
            result_domain,
            release_set,
            token_program,
            token_behavior,
            shard_terms_id,
            shard_exposure,
            receipt_mint,
            graph_id,
        ];
        if identities.iter().any(is_zero) {
            return Err(Error::ZeroIdentity);
        }
        let representation_width =
            read_u32(input, STRUCTURED_TERMS_REPRESENTATION_WIDTH_OFFSET_V2)?;
        if !(STRUCTURED_MIN_COORDINATES_V2..=STRUCTURED_MAX_COORDINATES_V2)
            .contains(&representation_width)
        {
            return Err(Error::InvalidCoordinate);
        }
        let denominator = read_u64(input, STRUCTURED_TERMS_DENOMINATOR_OFFSET_V2)?;
        if denominator < STRUCTURED_MIN_DENOMINATOR_V2 {
            return Err(Error::NonFractionalDenominator);
        }
        let coefficient_bytes = usize::try_from(representation_width)
            .map_err(|_| Error::InvalidCoordinate)?
            .checked_mul(STRUCTURED_TERMS_COEFFICIENT_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        let exact = STRUCTURED_TERMS_HEADER_BYTES_V2
            .checked_add(coefficient_bytes)
            .ok_or(Error::InvalidLength)?;
        if input.len() != exact {
            return Err(Error::InvalidLength);
        }
        let coefficients = slice(input, STRUCTURED_TERMS_HEADER_BYTES_V2, coefficient_bytes)?;
        let terms = Self {
            terms_id: admission.selected_terms_id,
            market,
            product_record,
            result_domain,
            release_set,
            token_program,
            token_behavior,
            shard_terms: shard_terms_id,
            shard_exposure,
            receipt_mint,
            graph_id,
            representation_width,
            denominator,
            coefficients,
        };
        terms.require_backing_exposure()?;
        terms.require_distinct_identities()?;
        terms.bind_shard_terms(shard_terms)?;
        Ok(terms)
    }

    /// Finalized content identity of the exact terms bytes.
    pub const fn terms_id(self) -> [u8; 32] {
        self.terms_id
    }
    /// Logical Core Market.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Finalized Product root digest.
    pub const fn product_record(self) -> [u8; 32] {
        self.product_record
    }
    /// Product-owned result-domain identity and ordering.
    pub const fn result_domain(self) -> [u8; 32] {
        self.result_domain
    }
    /// Immutable release set.
    pub const fn release_set(self) -> [u8; 32] {
        self.release_set
    }
    /// Release-selected Token program.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }
    /// Finalized Token behavior selection identity.
    pub const fn token_behavior(self) -> [u8; 32] {
        self.token_behavior
    }
    /// Finalized exact claim-shard terms owning the `K` shard Mints.
    pub const fn shard_terms(self) -> [u8; 32] {
        self.shard_terms
    }
    /// Finalized Product-N to Claims-K exposure identity.
    pub const fn shard_exposure(self) -> [u8; 32] {
        self.shard_exposure
    }
    /// Token-owned Structured receipt Mint.
    pub const fn receipt_mint(self) -> [u8; 32] {
        self.receipt_mint
    }
    /// Stable representation-composition graph identity.
    pub const fn graph_id(self) -> [u8; 32] {
        self.graph_id
    }
    /// Claims/shard representation width `K`.
    pub const fn representation_width(self) -> u32 {
        self.representation_width
    }
    /// Exact shard atoms per whole native claim, owned by the shard terms.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Exact shard atoms `c_i` backing one receipt atom at one coordinate.
    pub fn coefficient(self, representation_coordinate: u32) -> Result<u64> {
        if representation_coordinate >= self.representation_width {
            return Err(Error::InvalidCoordinate);
        }
        let offset = usize::try_from(representation_coordinate)
            .map_err(|_| Error::InvalidCoordinate)?
            .checked_mul(STRUCTURED_TERMS_COEFFICIENT_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        read_u64(self.coefficients, offset)
    }

    /// Exact shard atoms required to back `receipt_supply` receipt atoms at one
    /// coordinate.  **This is the exact backing invariant `K_i = S * c_i`.**
    pub fn required_shard_custody(
        self,
        representation_coordinate: u32,
        receipt_supply: u64,
    ) -> Result<u64> {
        receipt_supply
            .checked_mul(self.coefficient(representation_coordinate)?)
            .ok_or(Error::ArithmeticOverflow)
    }

    /// Bind these terms to the independently authenticated exact shard layer.
    pub fn bind_shard_terms(self, shard_terms: FractionalExposureTermsV2<'_>) -> Result<()> {
        if shard_terms.terms_id() != self.shard_terms {
            return Err(Error::ShardLayerMismatch);
        }
        if shard_terms.market() != self.market
            || shard_terms.product_record() != self.product_record
            || shard_terms.result_domain() != self.result_domain
            || shard_terms.release_set() != self.release_set
            || shard_terms.token_program() != self.token_program
            || shard_terms.exposure_id() != self.shard_exposure
            || shard_terms.graph_id() != self.graph_id
        {
            return Err(Error::AdmissionMismatch);
        }
        if shard_terms.representation_width() != self.representation_width {
            return Err(Error::InvalidCoordinate);
        }
        if shard_terms.denominator() != self.denominator {
            return Err(Error::NonFractionalDenominator);
        }
        // Physical form of the rank rule: the receipt Mint may never alias a
        // shard Mint, so a receipt can never be backed by itself.
        let mut coordinate = 0_u32;
        while coordinate < self.representation_width {
            let mint = shard_terms
                .shard_mint(coordinate)
                .map_err(|_| Error::ShardLayerMismatch)?;
            if mint == self.receipt_mint {
                return Err(Error::DuplicateIdentity);
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }

    fn require_backing_exposure(self) -> Result<()> {
        let mut coordinate = 0_u32;
        while coordinate < self.representation_width {
            if self.coefficient(coordinate)? != 0 {
                return Ok(());
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Err(Error::UnbackedBasis)
    }

    fn require_distinct_identities(self) -> Result<()> {
        let identities = [
            self.market,
            self.product_record,
            self.result_domain,
            self.terms_id,
            self.shard_terms,
            self.shard_exposure,
            self.receipt_mint,
            self.graph_id,
        ];
        for (index, left) in identities.iter().enumerate() {
            for right in identities.iter().skip(index + 1) {
                if left == right {
                    return Err(Error::DuplicateIdentity);
                }
            }
        }
        Ok(())
    }
}

/// Authenticated Market lifecycle projected onto the Structured basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredPhaseV2 {
    /// Market unresolved; receipts may be issued and unwrapped.
    Open,
    /// Market terminal; the projection carries the authenticated payout vector.
    Terminal,
    /// Zero supply and zero observed custody; the Structured node is closed.
    Retired,
}

/// One exact per-coordinate observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredCoordinateObservationV2 {
    /// Exact Token-observed Structured shard custody balance.
    pub observed_shard_custody: u64,
    /// Authenticated collateral atoms per whole native claim; zero before terminal.
    pub payout_per_claim: u64,
}

/// Borrowed runtime-width projection of authenticated Token and Market facts.
///
/// A Structured shard custody account is an ordinary Token account, so anyone
/// may donate into it.  The decoder therefore requires only solvency
/// (`observed >= required`) and exposes the difference as an explicitly named
/// [`StructuredProjectionV2::surplus_shard_custody`].  No plan reads it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredProjectionV2<'a> {
    phase: StructuredPhaseV2,
    terms_id: [u8; 32],
    market: [u8; 32],
    shard_terms: [u8; 32],
    denominator: u64,
    representation_width: u32,
    receipt_supply: u64,
    revision: u64,
    rows: &'a [u8],
}

impl<'a> StructuredProjectionV2<'a> {
    /// Hostile-decode and validate every phase-dependent backing row.
    pub fn decode(input: &'a [u8], terms: StructuredTermsV2<'_>) -> Result<Self> {
        if input.len() < STRUCTURED_PROJECTION_HEADER_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        if bytes::<8>(input, STRUCTURED_PROJECTION_MAGIC_OFFSET_V2)?
            != STRUCTURED_PROJECTION_MAGIC_V2
        {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, STRUCTURED_PROJECTION_VERSION_OFFSET_V2)? != STRUCTURED_SCHEMA_VERSION_V2
        {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(
            input,
            STRUCTURED_PROJECTION_RESERVED_HEADER_OFFSET_V2,
            RESERVED_HEADER_BYTES,
        )?;
        require_zero(
            input,
            STRUCTURED_PROJECTION_RESERVED_WIDTH_OFFSET_V2,
            RESERVED_WIDTH_BYTES,
        )?;
        require_zero(
            input,
            STRUCTURED_PROJECTION_RESERVED_TAIL_OFFSET_V2,
            PROJECTION_RESERVED_TAIL_BYTES,
        )?;
        let phase = match byte(input, STRUCTURED_PROJECTION_PHASE_OFFSET_V2)? {
            STRUCTURED_PHASE_OPEN_V2 => StructuredPhaseV2::Open,
            STRUCTURED_PHASE_TERMINAL_V2 => StructuredPhaseV2::Terminal,
            STRUCTURED_PHASE_RETIRED_V2 => StructuredPhaseV2::Retired,
            _ => return Err(Error::NonCanonical),
        };
        let terms_id = bytes::<32>(input, STRUCTURED_PROJECTION_TERMS_OFFSET_V2)?;
        let market = bytes::<32>(input, STRUCTURED_PROJECTION_MARKET_OFFSET_V2)?;
        let shard_terms = bytes::<32>(input, STRUCTURED_PROJECTION_SHARD_TERMS_OFFSET_V2)?;
        let representation_width =
            read_u32(input, STRUCTURED_PROJECTION_REPRESENTATION_WIDTH_OFFSET_V2)?;
        let denominator = read_u64(input, STRUCTURED_PROJECTION_DENOMINATOR_OFFSET_V2)?;
        if terms_id != terms.terms_id()
            || market != terms.market()
            || representation_width != terms.representation_width()
        {
            return Err(Error::AdmissionMismatch);
        }
        if shard_terms != terms.shard_terms() {
            return Err(Error::ShardLayerMismatch);
        }
        if denominator != terms.denominator() {
            return Err(Error::NonFractionalDenominator);
        }
        let row_bytes = usize::try_from(representation_width)
            .map_err(|_| Error::InvalidCoordinate)?
            .checked_mul(STRUCTURED_PROJECTION_ROW_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        let exact = STRUCTURED_PROJECTION_HEADER_BYTES_V2
            .checked_add(row_bytes)
            .ok_or(Error::InvalidLength)?;
        if input.len() != exact {
            return Err(Error::InvalidLength);
        }
        let projection = Self {
            phase,
            terms_id,
            market,
            shard_terms,
            denominator,
            representation_width,
            receipt_supply: read_u64(input, STRUCTURED_PROJECTION_RECEIPT_SUPPLY_OFFSET_V2)?,
            revision: read_u64(input, STRUCTURED_PROJECTION_REVISION_OFFSET_V2)?,
            rows: slice(input, STRUCTURED_PROJECTION_HEADER_BYTES_V2, row_bytes)?,
        };
        projection.validate_rows(terms)?;
        Ok(projection)
    }

    /// Authenticated lifecycle phase.
    pub const fn phase(self) -> StructuredPhaseV2 {
        self.phase
    }
    /// Finalized Structured terms identity.
    pub const fn terms_id(self) -> [u8; 32] {
        self.terms_id
    }
    /// Logical Core Market.
    pub const fn market(self) -> [u8; 32] {
        self.market
    }
    /// Finalized exact claim-shard terms identity observed on the shard layer.
    pub const fn shard_terms(self) -> [u8; 32] {
        self.shard_terms
    }
    /// Exact shard atoms per whole native claim.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }
    /// Observed representation width.
    pub const fn representation_width(self) -> u32 {
        self.representation_width
    }
    /// Token-owned Structured receipt Mint supply `S`.
    pub const fn receipt_supply(self) -> u64 {
        self.receipt_supply
    }
    /// Root replay revision observed from its sole persisted owner.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Read one exact observation row.
    pub fn observation(
        self,
        representation_coordinate: u32,
    ) -> Result<StructuredCoordinateObservationV2> {
        if representation_coordinate >= self.representation_width {
            return Err(Error::InvalidCoordinate);
        }
        let offset = usize::try_from(representation_coordinate)
            .map_err(|_| Error::InvalidCoordinate)?
            .checked_mul(STRUCTURED_PROJECTION_ROW_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        Ok(StructuredCoordinateObservationV2 {
            observed_shard_custody: read_u64(
                self.rows,
                offset
                    .checked_add(STRUCTURED_PROJECTION_ROW_CUSTODY_OFFSET_V2)
                    .ok_or(Error::InvalidLength)?,
            )?,
            payout_per_claim: read_u64(
                self.rows,
                offset
                    .checked_add(STRUCTURED_PROJECTION_ROW_PAYOUT_OFFSET_V2)
                    .ok_or(Error::InvalidLength)?,
            )?,
        })
    }

    /// Donated or otherwise unowned shard atoms above the exact backing.
    ///
    /// This quantity is never backing, never redeemable, and never distributed;
    /// it is exposed so retirement can refuse while it is nonzero.
    pub fn surplus_shard_custody(
        self,
        terms: StructuredTermsV2<'_>,
        representation_coordinate: u32,
    ) -> Result<u64> {
        let required =
            terms.required_shard_custody(representation_coordinate, self.receipt_supply)?;
        self.observation(representation_coordinate)?
            .observed_shard_custody
            .checked_sub(required)
            .ok_or(Error::BackingMismatch)
    }

    fn validate_rows(self, terms: StructuredTermsV2<'_>) -> Result<()> {
        let mut coordinate = 0_u32;
        while coordinate < self.representation_width {
            let observation = self.observation(coordinate)?;
            let required = terms.required_shard_custody(coordinate, self.receipt_supply)?;
            match self.phase {
                StructuredPhaseV2::Open => {
                    if observation.observed_shard_custody < required {
                        return Err(Error::BackingMismatch);
                    }
                    if observation.payout_per_claim != 0 {
                        return Err(Error::NonCanonical);
                    }
                }
                StructuredPhaseV2::Terminal => {
                    if observation.observed_shard_custody < required {
                        return Err(Error::BackingMismatch);
                    }
                }
                StructuredPhaseV2::Retired => {
                    if observation.observed_shard_custody != 0 || observation.payout_per_claim != 0
                    {
                        return Err(Error::NonCanonical);
                    }
                }
            }
            coordinate = coordinate.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        if self.phase == StructuredPhaseV2::Retired && self.receipt_supply != 0 {
            return Err(Error::NonCanonical);
        }
        Ok(())
    }
}

pub(crate) fn byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

pub(crate) fn bytes<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

pub(crate) fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(bytes(input, offset)?))
}

pub(crate) fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(input, offset)?))
}

pub(crate) fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(bytes(input, offset)?))
}

pub(crate) fn slice(input: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    input.get(offset..end).ok_or(Error::InvalidLength)
}

pub(crate) fn require_zero(input: &[u8], offset: usize, len: usize) -> Result<()> {
    if slice(input, offset, len)?.iter().any(|value| *value != 0) {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

pub(crate) fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    let end = offset
        .checked_add(value.len())
        .ok_or(Error::InvalidLength)?;
    output
        .get_mut(offset..end)
        .ok_or(Error::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}

pub(crate) fn is_zero(identity: &[u8; 32]) -> bool {
    identity.iter().all(|value| *value == 0)
}
