//! Hostile decoding and phase-dependent per-outcome conservation.

use core::convert::TryInto;

/// Immutable terms magic.
pub const FRACTIONAL_TERMS_MAGIC_V1: [u8; 8] = *b"DCFRTRM1";
/// Runtime projection magic.
pub const FRACTIONAL_PROJECTION_MAGIC_V1: [u8; 8] = *b"DCFPROJ1";
/// Implemented schema version.
pub const SCHEMA_VERSION_V1: u16 = 1;
/// Fixed immutable-terms header width before one Mint identity per outcome.
pub const FRACTIONAL_TERMS_HEADER_BYTES_V1: usize = 192;
/// Width of one immutable shard Mint identity.
pub const FRACTIONAL_TERMS_MINT_BYTES_V1: usize = 32;
/// Fixed projection header width before reserve rows.
pub const FRACTIONAL_PROJECTION_HEADER_BYTES_V1: usize = 96;
/// Width of one `(locked native claims, shard supply)` reserve row.
pub const FRACTIONAL_PROJECTION_ROW_BYTES_V1: usize = 16;

const TERMS_VERSION_OFFSET: usize = 8;
const TERMS_RESERVED_A_OFFSET: usize = 10;
const TERMS_RESERVED_A_BYTES: usize = 6;
const TERMS_MARKET_OFFSET: usize = 16;
const TERMS_RESULT_DOMAIN_OFFSET: usize = 48;
const TERMS_RELEASE_SET_OFFSET: usize = 80;
const TERMS_TOKEN_PROGRAM_OFFSET: usize = 112;
const TERMS_TOKEN_BEHAVIOR_OFFSET: usize = 144;
const TERMS_OUTCOME_COUNT_OFFSET: usize = 176;
const TERMS_RESERVED_B_OFFSET: usize = 180;
const TERMS_RESERVED_B_BYTES: usize = 4;
const TERMS_DENOMINATOR_OFFSET: usize = 184;

const PROJECTION_VERSION_OFFSET: usize = 8;
const PROJECTION_PHASE_OFFSET: usize = 10;
const PROJECTION_RESERVED_OFFSET: usize = 11;
const PROJECTION_RESERVED_BYTES: usize = 5;
const PROJECTION_TERMS_ID_OFFSET: usize = 16;
const PROJECTION_MARKET_OFFSET: usize = 48;
const PROJECTION_OUTCOME_COUNT_OFFSET: usize = 80;
const PROJECTION_TERMINAL_OUTCOME_OFFSET: usize = 84;
const PROJECTION_REVISION_OFFSET: usize = 88;

const NO_TERMINAL_OUTCOME: u32 = u32::MAX;

/// Stable hostile-decode or exact-transition refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// A fixed or runtime-derived byte width differed.
    InvalidLength,
    /// Magic bytes selected another schema family.
    InvalidMagic,
    /// The schema version is unsupported.
    UnsupportedVersion,
    /// Reserved bytes, a phase tag, or a phase payload were noncanonical.
    NonCanonical,
    /// A required content, Market, program, behavior, Mint, or account identity was zero.
    ZeroIdentity,
    /// The Product-owned outcome width was zero or an outcome was outside it.
    InvalidOutcome,
    /// Fractional denomination requires a denominator greater than one.
    NonFractionalDenominator,
    /// Two Product outcomes aliased the same shard Mint.
    DuplicateShardMint,
    /// Selected, finalized, recomputed, or projected immutable identities differed.
    AdmissionMismatch,
    /// Finalized Record ownership, PDA, vacancy, digest, or rent was not authenticated.
    UnauthenticatedRecord,
    /// A checked byte width, sum, product, subtraction, or revision overflowed.
    ArithmeticOverflow,
    /// Shard supply violated the exact phase-dependent reserve invariant.
    ReserveMismatch,
    /// The requested action is not admitted in the observed lifecycle phase.
    InvalidPhase,
    /// An exact state-changing action carried zero quantity.
    ZeroQuantity,
    /// An observed holder balance exceeded Mint supply or could not fund the action.
    InsufficientBalance,
    /// A selected shard input contained no whole native claim.
    NoWholeClaim,
    /// A transfer source and destination were equal.
    AccountAlias,
    /// A shard Mint still had supply at retirement.
    OutstandingShardSupply,
    /// Winning native claims remained after winning shard supply reached zero.
    OutstandingWinningClaims,
}

/// Result alias for this total kernel.
pub type Result<T> = core::result::Result<T, Error>;

/// Finalized immutable-terms authentication supplied by the Record adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTermsAdmissionV1 {
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

/// Hostile-decoded immutable exact claim-shard terms.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalTermsV1<'a> {
    terms_id: [u8; 32],
    market_id: [u8; 32],
    result_domain_id: [u8; 32],
    release_set_id: [u8; 32],
    token_program: [u8; 32],
    token_behavior_selection_id: [u8; 32],
    outcome_count: u32,
    denominator: u64,
    shard_mints: &'a [u8],
}

impl<'a> FractionalTermsV1<'a> {
    /// Decode exact immutable bytes after joining all finalized-record identities.
    pub fn decode(input: &'a [u8], admission: FractionalTermsAdmissionV1) -> Result<Self> {
        if input.len() < FRACTIONAL_TERMS_HEADER_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes::<8>(input, 0)? != FRACTIONAL_TERMS_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, TERMS_VERSION_OFFSET)? != SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, TERMS_RESERVED_A_OFFSET, TERMS_RESERVED_A_BYTES)?;
        require_zero(input, TERMS_RESERVED_B_OFFSET, TERMS_RESERVED_B_BYTES)?;
        if !admission.record_authenticated {
            return Err(Error::UnauthenticatedRecord);
        }
        if is_zero(&admission.selected_terms_id)
            || admission.selected_terms_id != admission.finalized_terms_id
            || admission.selected_terms_id != admission.recomputed_terms_digest
            || admission.selected_terms_id != admission.finalized_terms_digest
        {
            return Err(Error::AdmissionMismatch);
        }
        let market_id = bytes::<32>(input, TERMS_MARKET_OFFSET)?;
        let result_domain_id = bytes::<32>(input, TERMS_RESULT_DOMAIN_OFFSET)?;
        let release_set_id = bytes::<32>(input, TERMS_RELEASE_SET_OFFSET)?;
        let token_program = bytes::<32>(input, TERMS_TOKEN_PROGRAM_OFFSET)?;
        let token_behavior_selection_id = bytes::<32>(input, TERMS_TOKEN_BEHAVIOR_OFFSET)?;
        if [
            market_id,
            result_domain_id,
            release_set_id,
            token_program,
            token_behavior_selection_id,
        ]
        .iter()
        .any(is_zero)
        {
            return Err(Error::ZeroIdentity);
        }
        let outcome_count = read_u32(input, TERMS_OUTCOME_COUNT_OFFSET)?;
        if outcome_count == 0 {
            return Err(Error::InvalidOutcome);
        }
        let denominator = read_u64(input, TERMS_DENOMINATOR_OFFSET)?;
        if denominator <= 1 {
            return Err(Error::NonFractionalDenominator);
        }
        let mint_bytes = usize::try_from(outcome_count)
            .map_err(|_| Error::InvalidOutcome)?
            .checked_mul(FRACTIONAL_TERMS_MINT_BYTES_V1)
            .ok_or(Error::InvalidLength)?;
        let exact = FRACTIONAL_TERMS_HEADER_BYTES_V1
            .checked_add(mint_bytes)
            .ok_or(Error::InvalidLength)?;
        if input.len() != exact {
            return Err(Error::InvalidLength);
        }
        let shard_mints = slice(input, FRACTIONAL_TERMS_HEADER_BYTES_V1, mint_bytes)?;
        validate_mints(shard_mints, outcome_count)?;
        Ok(Self {
            terms_id: admission.selected_terms_id,
            market_id,
            result_domain_id,
            release_set_id,
            token_program,
            token_behavior_selection_id,
            outcome_count,
            denominator,
            shard_mints,
        })
    }

    /// Finalized content identity of the exact terms bytes.
    pub const fn terms_id(self) -> [u8; 32] {
        self.terms_id
    }

    /// Immutable Market identity.
    pub const fn market_id(self) -> [u8; 32] {
        self.market_id
    }

    /// Product-owned result-domain identity and outcome ordering.
    pub const fn result_domain_id(self) -> [u8; 32] {
        self.result_domain_id
    }

    /// Immutable release-set identity.
    pub const fn release_set_id(self) -> [u8; 32] {
        self.release_set_id
    }

    /// Release-selected Token program identity.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }

    /// Finalized Token behavior selection identity.
    pub const fn token_behavior_selection_id(self) -> [u8; 32] {
        self.token_behavior_selection_id
    }

    /// Product-owned runtime outcome width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Exact number of shard atoms per native categorical claim.
    pub const fn denominator(self) -> u64 {
        self.denominator
    }

    /// Immutable shard Mint identity for one Product outcome.
    pub fn shard_mint(self, outcome: u32) -> Result<[u8; 32]> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidOutcome);
        }
        let offset = usize::try_from(outcome)
            .map_err(|_| Error::InvalidOutcome)?
            .checked_mul(FRACTIONAL_TERMS_MINT_BYTES_V1)
            .ok_or(Error::InvalidLength)?;
        bytes::<32>(self.shard_mints, offset)
    }
}

/// Authenticated Market lifecycle relevant to claim-shard behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalPhaseV1 {
    /// Market is unresolved; native claims may wrap and unwrap.
    Open,
    /// Market has one authenticated categorical winning outcome.
    Terminal {
        /// Product-owned winning outcome index.
        winning_outcome: u32,
    },
    /// All shard supplies and Claims-native custody are zero.
    Retired,
}

/// One exact authenticated reserve observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OutcomeReserveV1 {
    /// Native categorical claims in canonical shard custody.
    pub locked_native_claims: u64,
    /// Exact Token-owned shard Mint supply.
    pub shard_supply: u64,
}

/// Borrowed runtime-width projection of authenticated Claims and Token facts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalProjectionV1<'a> {
    phase: FractionalPhaseV1,
    terms_id: [u8; 32],
    market_id: [u8; 32],
    outcome_count: u32,
    revision: u64,
    rows: &'a [u8],
}

impl<'a> FractionalProjectionV1<'a> {
    /// Hostile-decode and validate every phase-dependent reserve row.
    pub fn decode(input: &'a [u8], terms: FractionalTermsV1<'_>) -> Result<Self> {
        if input.len() < FRACTIONAL_PROJECTION_HEADER_BYTES_V1 {
            return Err(Error::InvalidLength);
        }
        if bytes::<8>(input, 0)? != FRACTIONAL_PROJECTION_MAGIC_V1 {
            return Err(Error::InvalidMagic);
        }
        if read_u16(input, PROJECTION_VERSION_OFFSET)? != SCHEMA_VERSION_V1 {
            return Err(Error::UnsupportedVersion);
        }
        require_zero(input, PROJECTION_RESERVED_OFFSET, PROJECTION_RESERVED_BYTES)?;
        let terms_id = bytes::<32>(input, PROJECTION_TERMS_ID_OFFSET)?;
        let market_id = bytes::<32>(input, PROJECTION_MARKET_OFFSET)?;
        let outcome_count = read_u32(input, PROJECTION_OUTCOME_COUNT_OFFSET)?;
        if terms_id != terms.terms_id()
            || market_id != terms.market_id()
            || outcome_count != terms.outcome_count()
        {
            return Err(Error::AdmissionMismatch);
        }
        let terminal_outcome = read_u32(input, PROJECTION_TERMINAL_OUTCOME_OFFSET)?;
        let phase = match byte(input, PROJECTION_PHASE_OFFSET)? {
            0 if terminal_outcome == NO_TERMINAL_OUTCOME => FractionalPhaseV1::Open,
            1 if terminal_outcome < outcome_count => FractionalPhaseV1::Terminal {
                winning_outcome: terminal_outcome,
            },
            2 if terminal_outcome == NO_TERMINAL_OUTCOME => FractionalPhaseV1::Retired,
            _ => return Err(Error::NonCanonical),
        };
        let row_bytes = usize::try_from(outcome_count)
            .map_err(|_| Error::InvalidOutcome)?
            .checked_mul(FRACTIONAL_PROJECTION_ROW_BYTES_V1)
            .ok_or(Error::InvalidLength)?;
        let exact = FRACTIONAL_PROJECTION_HEADER_BYTES_V1
            .checked_add(row_bytes)
            .ok_or(Error::InvalidLength)?;
        if input.len() != exact {
            return Err(Error::InvalidLength);
        }
        let rows = slice(input, FRACTIONAL_PROJECTION_HEADER_BYTES_V1, row_bytes)?;
        let projection = Self {
            phase,
            terms_id,
            market_id,
            outcome_count,
            revision: read_u64(input, PROJECTION_REVISION_OFFSET)?,
            rows,
        };
        projection.validate_reserves(terms)?;
        Ok(projection)
    }

    /// Authenticated lifecycle phase.
    pub const fn phase(self) -> FractionalPhaseV1 {
        self.phase
    }

    /// Finalized terms identity.
    pub const fn terms_id(self) -> [u8; 32] {
        self.terms_id
    }

    /// Immutable Market identity.
    pub const fn market_id(self) -> [u8; 32] {
        self.market_id
    }

    /// Product-owned runtime outcome width.
    pub const fn outcome_count(self) -> u32 {
        self.outcome_count
    }

    /// Adapter replay revision observed from its sole persisted owner.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    /// Read one exact reserve row.
    pub fn reserve(self, outcome: u32) -> Result<OutcomeReserveV1> {
        if outcome >= self.outcome_count {
            return Err(Error::InvalidOutcome);
        }
        let offset = usize::try_from(outcome)
            .map_err(|_| Error::InvalidOutcome)?
            .checked_mul(FRACTIONAL_PROJECTION_ROW_BYTES_V1)
            .ok_or(Error::InvalidLength)?;
        Ok(OutcomeReserveV1 {
            locked_native_claims: read_u64(self.rows, offset)?,
            shard_supply: read_u64(self.rows, offset + 8)?,
        })
    }

    fn validate_reserves(self, terms: FractionalTermsV1<'_>) -> Result<()> {
        let mut outcome = 0_u32;
        while outcome < self.outcome_count {
            let reserve = self.reserve(outcome)?;
            let capacity = exact_shard_capacity(terms.denominator(), reserve.locked_native_claims)?;
            match self.phase {
                FractionalPhaseV1::Open => {
                    if reserve.shard_supply != capacity {
                        return Err(Error::ReserveMismatch);
                    }
                }
                FractionalPhaseV1::Terminal { winning_outcome } if outcome == winning_outcome => {
                    if reserve.shard_supply != capacity {
                        return Err(Error::ReserveMismatch);
                    }
                }
                FractionalPhaseV1::Terminal { .. } => {
                    if reserve.shard_supply > capacity {
                        return Err(Error::ReserveMismatch);
                    }
                }
                FractionalPhaseV1::Retired => {
                    if reserve.locked_native_claims != 0 || reserve.shard_supply != 0 {
                        return Err(Error::ReserveMismatch);
                    }
                }
            }
            outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        Ok(())
    }
}

pub(crate) fn exact_shard_capacity(denominator: u64, native_claims: u64) -> Result<u64> {
    denominator
        .checked_mul(native_claims)
        .ok_or(Error::ArithmeticOverflow)
}

fn validate_mints(mints: &[u8], outcome_count: u32) -> Result<()> {
    let mut outcome = 0_u32;
    while outcome < outcome_count {
        let offset = usize::try_from(outcome)
            .map_err(|_| Error::InvalidOutcome)?
            .checked_mul(FRACTIONAL_TERMS_MINT_BYTES_V1)
            .ok_or(Error::InvalidLength)?;
        let mint = bytes::<32>(mints, offset)?;
        if is_zero(&mint) {
            return Err(Error::ZeroIdentity);
        }
        let mut prior = 0_u32;
        while prior < outcome {
            let prior_offset = usize::try_from(prior)
                .map_err(|_| Error::InvalidOutcome)?
                .checked_mul(FRACTIONAL_TERMS_MINT_BYTES_V1)
                .ok_or(Error::InvalidLength)?;
            if bytes::<32>(mints, prior_offset)? == mint {
                return Err(Error::DuplicateShardMint);
            }
            prior = prior.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
        outcome = outcome.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
    }
    Ok(())
}

fn byte(input: &[u8], offset: usize) -> Result<u8> {
    input.get(offset).copied().ok_or(Error::InvalidLength)
}

fn bytes<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    slice(input, offset, N)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(bytes(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(bytes(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(bytes(input, offset)?))
}

fn slice(input: &[u8], offset: usize, len: usize) -> Result<&[u8]> {
    let end = offset.checked_add(len).ok_or(Error::InvalidLength)?;
    input.get(offset..end).ok_or(Error::InvalidLength)
}

fn require_zero(input: &[u8], offset: usize, len: usize) -> Result<()> {
    if slice(input, offset, len)?.iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonical);
    }
    Ok(())
}

fn is_zero(identity: &[u8; 32]) -> bool {
    identity.iter().all(|byte| *byte == 0)
}
