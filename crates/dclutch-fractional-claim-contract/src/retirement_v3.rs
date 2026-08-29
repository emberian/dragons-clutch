//! Ordered, constant-width retirement for runtime-width Fractional shard sets.
//!
//! The V2 all-at-once retirement route expands every shard Mint into one
//! transaction. This contract instead makes progress an authenticated,
//! strictly ordered fact: one transaction retires exactly the next
//! terms-owned coordinate, and a fixed-account finish is available only after
//! all `K` coordinates have advanced.

use core::convert::TryInto;

use dclutch_fractional_claim_kernel::FractionalExposureTermsV2;

/// Exact persisted cursor width.
pub const FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3: usize = 296;
/// Exact request width for begin, one-coordinate advance, and finish.
pub const FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3: usize = 288;
/// Cursor-state magic.
pub const FRACTIONAL_RETIREMENT_CURSOR_MAGIC_V3: [u8; 8] = *b"DCFRCR03";
/// Retirement-request magic.
pub const FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3: [u8; 8] = *b"DCFRRQ03";
/// Cursor schema preimage.
pub const FRACTIONAL_RETIREMENT_CURSOR_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/fractional-retirement-cursor-v3|bytes296|terms-owned-ordered-coordinate|constant-width|revision-bound";
/// SHA-256 identity of [`FRACTIONAL_RETIREMENT_CURSOR_SCHEMA_PREIMAGE_V3`].
pub const FRACTIONAL_RETIREMENT_CURSOR_SCHEMA_ID_V3: [u8; 32] = [
    0xff, 0x68, 0x94, 0x5c, 0xa5, 0x7f, 0x9e, 0xdf, 0xb8, 0x6e, 0xa5, 0x6a, 0xb5, 0xb8, 0x28, 0xd4,
    0x5d, 0xaa, 0xd8, 0x1f, 0x78, 0x89, 0x48, 0xae, 0x07, 0x2f, 0xd1, 0xf1, 0x67, 0xa3, 0x49, 0x94,
];
/// Request schema preimage.
pub const FRACTIONAL_RETIREMENT_REQUEST_SCHEMA_PREIMAGE_V3: &[u8] = b"dclutch/schema/fractional-retirement-request-v3|bytes288|begin-step-finish|terms-bound|no-K-account-tail";
/// SHA-256 identity of [`FRACTIONAL_RETIREMENT_REQUEST_SCHEMA_PREIMAGE_V3`].
pub const FRACTIONAL_RETIREMENT_REQUEST_SCHEMA_ID_V3: [u8; 32] = [
    0x2f, 0x06, 0xf1, 0x5f, 0xe6, 0xcc, 0x7d, 0x48, 0xc6, 0xfd, 0x99, 0xe9, 0xaf, 0x9d, 0x7e, 0x55,
    0xa2, 0x7e, 0x93, 0x3b, 0x4c, 0x4f, 0x3a, 0xd6, 0x8f, 0x56, 0xcd, 0xcc, 0x6c, 0x16, 0xa8, 0xf3,
];

const VERSION_V3: u16 = 3;
const ACTION_OFFSET: usize = 10;
const BUMP_OFFSET: usize = 10;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const TERMS_OFFSET: usize = 80;
const TOKEN_PROGRAM_OFFSET: usize = 112;
const TOKEN_BEHAVIOR_OFFSET: usize = 144;
const EXPOSURE_OFFSET: usize = 176;
const ROOT_OFFSET: usize = 208;
const RENT_CREDIT_OFFSET: usize = 240;
const REQUEST_REVISION_OFFSET: usize = 272;
const REQUEST_COORDINATE_OFFSET: usize = 280;
const CURSOR_NEXT_OFFSET: usize = 272;
const CURSOR_WIDTH_OFFSET: usize = 276;
const CURSOR_REVISION_OFFSET: usize = 280;
const CURSOR_RENT_PRINCIPAL_OFFSET: usize = 288;

/// Canonical absent coordinate on begin and finish.
pub const NO_RETIREMENT_COORDINATE_V3: u32 = u32::MAX;

/// Exact bounded retirement action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FractionalRetirementActionV3 {
    /// Create the cursor and move the root into ordered retirement.
    Begin = 0,
    /// Close exactly the next zero-supply Mint and empty Claims reserve.
    RetireCoordinate = 1,
    /// Close the completed cursor and fixed producer-root resources.
    Finish = 2,
}

impl FractionalRetirementActionV3 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Begin),
            1 => Ok(Self::RetireCoordinate),
            2 => Ok(Self::Finish),
            _ => Err(FractionalRetirementErrorV3::UnknownAction),
        }
    }
}

/// Exact immutable request coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRetirementRequestInputV3 {
    /// Checked release-set identity.
    pub release_set: [u8; 32],
    /// Logical Market identity.
    pub market: [u8; 32],
    /// Finalized Fractional terms identity.
    pub terms: [u8; 32],
    /// Terms-selected Token-2022 program.
    pub token_program: [u8; 32],
    /// Terms-selected TokenBehavior record.
    pub token_behavior: [u8; 32],
    /// Terms-selected Product-to-Claims exposure.
    pub exposure: [u8; 32],
    /// Fractional producer root.
    pub root: [u8; 32],
    /// Root-bound lifecycle RentCredit.
    pub rent_credit: [u8; 32],
    /// Optimistic root/cursor revision.
    pub expected_revision: u64,
    /// Next coordinate for a step; absent on begin and finish.
    pub representation_coordinate: u32,
}

/// Hostile-decoded constant-width retirement request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRetirementRequestV3 {
    action: FractionalRetirementActionV3,
    input: FractionalRetirementRequestInputV3,
}

impl FractionalRetirementRequestV3 {
    /// Construct one canonical request.
    pub fn new(
        action: FractionalRetirementActionV3,
        input: FractionalRetirementRequestInputV3,
    ) -> Result<Self> {
        if [
            input.release_set,
            input.market,
            input.terms,
            input.token_program,
            input.token_behavior,
            input.exposure,
            input.root,
            input.rent_credit,
        ]
        .contains(&[0; 32])
            || input.root == input.rent_credit
            || (action == FractionalRetirementActionV3::RetireCoordinate)
                != (input.representation_coordinate != NO_RETIREMENT_COORDINATE_V3)
        {
            return Err(FractionalRetirementErrorV3::NonCanonical);
        }
        Ok(Self { action, input })
    }

    /// Hostile-decode exact request bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3
            || array::<8>(bytes, 0)? != FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3
            || read_u16(bytes, 8)? != VERSION_V3
            || bytes
                .get(11..16)
                .is_none_or(|tail| tail.iter().any(|byte| *byte != 0))
            || bytes
                .get(284..288)
                .is_none_or(|tail| tail.iter().any(|byte| *byte != 0))
        {
            return Err(FractionalRetirementErrorV3::InvalidEncoding);
        }
        Self::new(
            FractionalRetirementActionV3::decode(byte(bytes, ACTION_OFFSET)?)?,
            FractionalRetirementRequestInputV3 {
                release_set: array(bytes, RELEASE_SET_OFFSET)?,
                market: array(bytes, MARKET_OFFSET)?,
                terms: array(bytes, TERMS_OFFSET)?,
                token_program: array(bytes, TOKEN_PROGRAM_OFFSET)?,
                token_behavior: array(bytes, TOKEN_BEHAVIOR_OFFSET)?,
                exposure: array(bytes, EXPOSURE_OFFSET)?,
                root: array(bytes, ROOT_OFFSET)?,
                rent_credit: array(bytes, RENT_CREDIT_OFFSET)?,
                expected_revision: read_u64(bytes, REQUEST_REVISION_OFFSET)?,
                representation_coordinate: read_u32(bytes, REQUEST_COORDINATE_OFFSET)?,
            },
        )
    }

    /// Encode exact canonical request bytes.
    pub fn to_bytes(self) -> Result<[u8; FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3]> {
        let mut output = [0; FRACTIONAL_RETIREMENT_REQUEST_BYTES_V3];
        output[..8].copy_from_slice(&FRACTIONAL_RETIREMENT_REQUEST_MAGIC_V3);
        output[8..10].copy_from_slice(&VERSION_V3.to_le_bytes());
        output[ACTION_OFFSET] = self.action as u8;
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.input.release_set),
            (MARKET_OFFSET, self.input.market),
            (TERMS_OFFSET, self.input.terms),
            (TOKEN_PROGRAM_OFFSET, self.input.token_program),
            (TOKEN_BEHAVIOR_OFFSET, self.input.token_behavior),
            (EXPOSURE_OFFSET, self.input.exposure),
            (ROOT_OFFSET, self.input.root),
            (RENT_CREDIT_OFFSET, self.input.rent_credit),
        ] {
            put(&mut output, offset, &value)?;
        }
        output[REQUEST_REVISION_OFFSET..REQUEST_REVISION_OFFSET + 8]
            .copy_from_slice(&self.input.expected_revision.to_le_bytes());
        output[REQUEST_COORDINATE_OFFSET..REQUEST_COORDINATE_OFFSET + 4]
            .copy_from_slice(&self.input.representation_coordinate.to_le_bytes());
        Ok(output)
    }

    /// Selected action.
    pub const fn action(self) -> FractionalRetirementActionV3 {
        self.action
    }

    /// Exact request fields.
    pub const fn input(self) -> FractionalRetirementRequestInputV3 {
        self.input
    }

    /// Bind every duplicated request identity to the sole finalized terms.
    pub fn bind_terms(self, terms: FractionalExposureTermsV2<'_>) -> Result<Self> {
        if self.input.release_set != terms.release_set()
            || self.input.market != terms.market()
            || self.input.terms != terms.terms_id()
            || self.input.token_program != terms.token_program()
            || self.input.token_behavior != terms.token_behavior()
            || self.input.exposure != terms.exposure_id()
            || (self.action == FractionalRetirementActionV3::RetireCoordinate
                && self.input.representation_coordinate >= terms.representation_width())
        {
            return Err(FractionalRetirementErrorV3::IdentityMismatch);
        }
        Ok(self)
    }
}

/// Exact cursor creation input derived from authenticated accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRetirementCursorInputV3 {
    /// Canonical PDA bump.
    pub bump: u8,
    /// Root revision before begin.
    pub pre_revision: u64,
    /// Historical cursor rent principal; never fee revenue.
    pub historical_rent_principal: u64,
}

/// One-coordinate finalized observations authenticated by the future adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRetireCoordinateObservationV3 {
    /// Exact observed terms-owned Mint.
    pub shard_mint: [u8; 32],
    /// Token-2022 Mint supply, which must be zero.
    pub shard_supply: u64,
    /// Claims-native balance in the Fractional reserve Position.
    pub reserve_claims: u64,
    /// Token Mint owner/data/authority checks were completed from account bytes.
    pub mint_authenticated: bool,
    /// Claims Position owner/market/coordinate checks were completed from bytes.
    pub reserve_authenticated: bool,
}

/// Persisted ordered progress. It owns no supply, balance, or remainder fact.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRetirementCursorV3 {
    bump: u8,
    release_set: [u8; 32],
    market: [u8; 32],
    terms: [u8; 32],
    token_program: [u8; 32],
    token_behavior: [u8; 32],
    exposure: [u8; 32],
    root: [u8; 32],
    rent_credit: [u8; 32],
    next_coordinate: u32,
    representation_width: u32,
    revision: u64,
    historical_rent_principal: u64,
}

impl FractionalRetirementCursorV3 {
    /// Begin ordered retirement and consume exactly one root revision.
    pub fn begin(
        terms: FractionalExposureTermsV2<'_>,
        request: FractionalRetirementRequestV3,
        input: FractionalRetirementCursorInputV3,
    ) -> Result<Self> {
        let request = request.bind_terms(terms)?;
        if request.action != FractionalRetirementActionV3::Begin
            || request.input.expected_revision != input.pre_revision
            || input.historical_rent_principal == 0
        {
            return Err(FractionalRetirementErrorV3::InvalidTransition);
        }
        Ok(Self {
            bump: input.bump,
            release_set: terms.release_set(),
            market: terms.market(),
            terms: terms.terms_id(),
            token_program: terms.token_program(),
            token_behavior: terms.token_behavior(),
            exposure: terms.exposure_id(),
            root: request.input.root,
            rent_credit: request.input.rent_credit,
            next_coordinate: 0,
            representation_width: terms.representation_width(),
            revision: input
                .pre_revision
                .checked_add(1)
                .ok_or(FractionalRetirementErrorV3::Arithmetic)?,
            historical_rent_principal: input.historical_rent_principal,
        })
    }

    /// Hostile-decode exact cursor state.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3
            || array::<8>(bytes, 0)? != FRACTIONAL_RETIREMENT_CURSOR_MAGIC_V3
            || read_u16(bytes, 8)? != VERSION_V3
            || bytes
                .get(11..16)
                .is_none_or(|tail| tail.iter().any(|byte| *byte != 0))
        {
            return Err(FractionalRetirementErrorV3::InvalidEncoding);
        }
        let value = Self {
            bump: byte(bytes, BUMP_OFFSET)?,
            release_set: array(bytes, RELEASE_SET_OFFSET)?,
            market: array(bytes, MARKET_OFFSET)?,
            terms: array(bytes, TERMS_OFFSET)?,
            token_program: array(bytes, TOKEN_PROGRAM_OFFSET)?,
            token_behavior: array(bytes, TOKEN_BEHAVIOR_OFFSET)?,
            exposure: array(bytes, EXPOSURE_OFFSET)?,
            root: array(bytes, ROOT_OFFSET)?,
            rent_credit: array(bytes, RENT_CREDIT_OFFSET)?,
            next_coordinate: read_u32(bytes, CURSOR_NEXT_OFFSET)?,
            representation_width: read_u32(bytes, CURSOR_WIDTH_OFFSET)?,
            revision: read_u64(bytes, CURSOR_REVISION_OFFSET)?,
            historical_rent_principal: read_u64(bytes, CURSOR_RENT_PRINCIPAL_OFFSET)?,
        };
        if [
            value.release_set,
            value.market,
            value.terms,
            value.token_program,
            value.token_behavior,
            value.exposure,
            value.root,
            value.rent_credit,
        ]
        .contains(&[0; 32])
            || value.root == value.rent_credit
            || value.representation_width == 0
            || value.next_coordinate > value.representation_width
            || value.historical_rent_principal == 0
        {
            return Err(FractionalRetirementErrorV3::NonCanonical);
        }
        Ok(value)
    }

    /// Encode exact cursor bytes.
    pub fn to_bytes(self) -> Result<[u8; FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3]> {
        let mut output = [0; FRACTIONAL_RETIREMENT_CURSOR_BYTES_V3];
        output[..8].copy_from_slice(&FRACTIONAL_RETIREMENT_CURSOR_MAGIC_V3);
        output[8..10].copy_from_slice(&VERSION_V3.to_le_bytes());
        output[BUMP_OFFSET] = self.bump;
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.release_set),
            (MARKET_OFFSET, self.market),
            (TERMS_OFFSET, self.terms),
            (TOKEN_PROGRAM_OFFSET, self.token_program),
            (TOKEN_BEHAVIOR_OFFSET, self.token_behavior),
            (EXPOSURE_OFFSET, self.exposure),
            (ROOT_OFFSET, self.root),
            (RENT_CREDIT_OFFSET, self.rent_credit),
        ] {
            put(&mut output, offset, &value)?;
        }
        output[CURSOR_NEXT_OFFSET..CURSOR_NEXT_OFFSET + 4]
            .copy_from_slice(&self.next_coordinate.to_le_bytes());
        output[CURSOR_WIDTH_OFFSET..CURSOR_WIDTH_OFFSET + 4]
            .copy_from_slice(&self.representation_width.to_le_bytes());
        output[CURSOR_REVISION_OFFSET..CURSOR_REVISION_OFFSET + 8]
            .copy_from_slice(&self.revision.to_le_bytes());
        output[CURSOR_RENT_PRINCIPAL_OFFSET..CURSOR_RENT_PRINCIPAL_OFFSET + 8]
            .copy_from_slice(&self.historical_rent_principal.to_le_bytes());
        Ok(output)
    }

    /// Advance exactly the next terms-owned coordinate after zero-state checks.
    pub fn advance(
        self,
        terms: FractionalExposureTermsV2<'_>,
        request: FractionalRetirementRequestV3,
        observed: FractionalRetireCoordinateObservationV3,
    ) -> Result<Self> {
        self.bind_terms(terms)?;
        let request = request.bind_terms(terms)?;
        let coordinate = request.input.representation_coordinate;
        if request.action != FractionalRetirementActionV3::RetireCoordinate
            || request.input.root != self.root
            || request.input.rent_credit != self.rent_credit
            || request.input.expected_revision != self.revision
            || coordinate != self.next_coordinate
            || coordinate >= self.representation_width
            || !observed.mint_authenticated
            || !observed.reserve_authenticated
            || observed.shard_supply != 0
            || observed.reserve_claims != 0
            || observed.shard_mint
                != terms
                    .shard_mint(coordinate)
                    .map_err(|_| FractionalRetirementErrorV3::IdentityMismatch)?
        {
            return Err(FractionalRetirementErrorV3::InvalidTransition);
        }
        Ok(Self {
            next_coordinate: self
                .next_coordinate
                .checked_add(1)
                .ok_or(FractionalRetirementErrorV3::Arithmetic)?,
            revision: self
                .revision
                .checked_add(1)
                .ok_or(FractionalRetirementErrorV3::Arithmetic)?,
            ..self
        })
    }

    /// Finish only after all K steps and consume one final revision.
    pub fn finish(
        self,
        terms: FractionalExposureTermsV2<'_>,
        request: FractionalRetirementRequestV3,
    ) -> Result<FractionalRetirementFinishV3> {
        self.bind_terms(terms)?;
        let request = request.bind_terms(terms)?;
        if request.action != FractionalRetirementActionV3::Finish
            || request.input.root != self.root
            || request.input.rent_credit != self.rent_credit
            || request.input.expected_revision != self.revision
            || self.next_coordinate != self.representation_width
        {
            return Err(FractionalRetirementErrorV3::InvalidTransition);
        }
        Ok(FractionalRetirementFinishV3 {
            terms: self.terms,
            market: self.market,
            release_set: self.release_set,
            root: self.root,
            rent_credit: self.rent_credit,
            coordinate_count: self.representation_width,
            terminal_revision: self
                .revision
                .checked_add(1)
                .ok_or(FractionalRetirementErrorV3::Arithmetic)?,
            cursor_rent_principal: self.historical_rent_principal,
        })
    }

    /// Next coordinate that must retire.
    pub const fn next_coordinate(self) -> u32 {
        self.next_coordinate
    }

    /// Exact runtime representation width.
    pub const fn representation_width(self) -> u32 {
        self.representation_width
    }

    /// Current optimistic revision.
    pub const fn revision(self) -> u64 {
        self.revision
    }

    fn bind_terms(self, terms: FractionalExposureTermsV2<'_>) -> Result<()> {
        if self.release_set != terms.release_set()
            || self.market != terms.market()
            || self.terms != terms.terms_id()
            || self.token_program != terms.token_program()
            || self.token_behavior != terms.token_behavior()
            || self.exposure != terms.exposure_id()
            || self.representation_width != terms.representation_width()
        {
            return Err(FractionalRetirementErrorV3::IdentityMismatch);
        }
        Ok(())
    }
}

/// Fixed-width terminal evidence produced before closing the cursor/root.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalRetirementFinishV3 {
    /// Finalized terms identity.
    pub terms: [u8; 32],
    /// Logical Market.
    pub market: [u8; 32],
    /// Checked release set.
    pub release_set: [u8; 32],
    /// Producer root being closed.
    pub root: [u8; 32],
    /// RentCredit receiving historical rent principal.
    pub rent_credit: [u8; 32],
    /// Exact number of ordered completed coordinates.
    pub coordinate_count: u32,
    /// Final consumed revision.
    pub terminal_revision: u64,
    /// Historical cursor rent principal; never a fee.
    pub cursor_rent_principal: u64,
}

/// Stable retirement refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalRetirementErrorV3 {
    /// Byte width, magic, version, or reserved bytes differed.
    InvalidEncoding,
    /// Action tag was unknown.
    UnknownAction,
    /// Required identities or inactive coordinates were not canonical.
    NonCanonical,
    /// Request or cursor identities differed from authenticated terms.
    IdentityMismatch,
    /// Revision, order, supply, reserve, or authentication checks refused.
    InvalidTransition,
    /// Revision/cursor arithmetic overflowed.
    Arithmetic,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, FractionalRetirementErrorV3>;

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(FractionalRetirementErrorV3::InvalidEncoding)?,
        )
        .ok_or(FractionalRetirementErrorV3::InvalidEncoding)?
        .try_into()
        .map_err(|_| FractionalRetirementErrorV3::InvalidEncoding)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(FractionalRetirementErrorV3::InvalidEncoding)
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

fn put(output: &mut [u8], offset: usize, value: &[u8]) -> Result<()> {
    output
        .get_mut(
            offset
                ..offset
                    .checked_add(value.len())
                    .ok_or(FractionalRetirementErrorV3::InvalidEncoding)?,
        )
        .ok_or(FractionalRetirementErrorV3::InvalidEncoding)?
        .copy_from_slice(value);
    Ok(())
}
