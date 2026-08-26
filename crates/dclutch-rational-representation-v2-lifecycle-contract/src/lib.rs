#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Exact resource activation and retirement semantics for Rational
//! Representation V2.
//!
//! This contract deliberately persists no lifecycle ledger. Token-2022 owns
//! Mint supply and token balances, Claims owns LBV2 Position quantities, and
//! RentCredit owns reclaimed native rent. The contract instead binds one
//! release-pinned request to immutable descriptor truth, exact chain
//! observations, and a state-last completion receipt.
//!
//! Activation is split into receipt-Mint and single-coordinate steps so
//! runtime width does not create one unbounded CPI frame. Coordinates are
//! exactly the descriptor's ordered nonzero coefficient support: zero
//! coefficients create no Mint, custody account, Position, admission record, or rent
//! obligation. Retirement closes coordinates independently once all physical
//! quantities are zero, then closes the receipt Mint only after an exact
//! ordered vacancy scan over the complete nonzero support.

use core::convert::TryInto;

use dclutch_rational_representation_v2_kernel::RepresentationDescriptorV2;
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

/// Exact fixed lifecycle request header width.
pub const LIFECYCLE_HEADER_BYTES_V2: usize = 400;
/// Exact width of one physical nonzero-support coordinate observation.
pub const LIFECYCLE_COORDINATE_BYTES_V2: usize = 272;
/// Exact completion receipt width.
pub const LIFECYCLE_RECEIPT_BYTES_V2: usize = 320;
/// Lifecycle request magic.
pub const LIFECYCLE_REQUEST_MAGIC_V2: [u8; 8] = *b"DCRRLC02";
/// Lifecycle receipt magic.
pub const LIFECYCLE_RECEIPT_MAGIC_V2: [u8; 8] = *b"DCRRLR02";
/// Implemented lifecycle wire version.
pub const LIFECYCLE_VERSION_V2: u16 = 2;
/// Absent coordinate sentinel used only by receipt-wide actions.
pub const ABSENT_OUTCOME_V2: u32 = u32::MAX;
/// Absent Position revision sentinel used only by proven-vacant rows.
pub const ABSENT_POSITION_REVISION_V2: u64 = u64::MAX;

const ACTION_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const GRAPH_ID_OFFSET: usize = 80;
const DESCRIPTOR_ID_OFFSET: usize = 112;
const PARENT_CONTEXT_OFFSET: usize = 144;
const REPRESENTATION_AUTHORITY_OFFSET: usize = 176;
const RECEIPT_MINT_OFFSET: usize = 208;
const TOKEN_PROGRAM_OFFSET: usize = 240;
const RENT_CREDIT_OFFSET: usize = 272;
const RENT_PROGRAM_OFFSET: usize = 304;
const GENERATION_OFFSET: usize = 336;
const EXPECTED_MARKET_REVISION_OFFSET: usize = 344;
const OBSERVED_RECEIPT_LAMPORTS_OFFSET: usize = 352;
const RECEIPT_RENT_PRINCIPAL_OFFSET: usize = 360;
const EXPECTED_RECEIPT_SUPPLY_OFFSET: usize = 368;
const OUTCOME_COUNT_OFFSET: usize = 376;
const COORDINATE_COUNT_OFFSET: usize = 380;
const RENT_CREDIT_BEFORE_OFFSET: usize = 384;
const RENT_CREDIT_AFTER_OFFSET: usize = 392;

const ROW_OUTCOME_OFFSET: usize = 0;
const ROW_RESERVED_HEAD_OFFSET: usize = 4;
const ROW_COEFFICIENT_OFFSET: usize = 8;
const ROW_SHARD_MINT_OFFSET: usize = 16;
const ROW_STRUCTURED_CUSTODY_OFFSET: usize = 48;
const ROW_CUSTODY_OWNER_OFFSET: usize = 80;
const ROW_CUSTODY_POSITION_OFFSET: usize = 112;
const ROW_POSITION_ADMISSION_OFFSET: usize = 144;
const ROW_SHARD_LAMPORTS_OFFSET: usize = 176;
const ROW_STRUCTURED_LAMPORTS_OFFSET: usize = 184;
const ROW_POSITION_LAMPORTS_OFFSET: usize = 192;
const ROW_ADMISSION_LAMPORTS_OFFSET: usize = 200;
const ROW_SHARD_RENT_OFFSET: usize = 208;
const ROW_STRUCTURED_RENT_OFFSET: usize = 216;
const ROW_POSITION_RENT_OFFSET: usize = 224;
const ROW_ADMISSION_RENT_OFFSET: usize = 232;
const ROW_SHARD_SUPPLY_OFFSET: usize = 240;
const ROW_STRUCTURED_AMOUNT_OFFSET: usize = 248;
const ROW_POSITION_REVISION_OFFSET: usize = 256;
const ROW_RESERVED_TAIL_OFFSET: usize = 264;

const RECEIPT_ACTION_OFFSET: usize = 10;
const RECEIPT_STATUS_OFFSET: usize = 11;
const RECEIPT_RESERVED_HEAD_OFFSET: usize = 12;
const RECEIPT_REQUEST_DIGEST_OFFSET: usize = 16;
const RECEIPT_DESCRIPTOR_ID_OFFSET: usize = 48;
const RECEIPT_MARKET_OFFSET: usize = 80;
const RECEIPT_POST_RESOURCE_DIGEST_OFFSET: usize = 112;
const RECEIPT_POSITION_RECEIPT_DIGEST_OFFSET: usize = 144;
const RECEIPT_RENT_CREDIT_OFFSET: usize = 176;
const RECEIPT_RENT_PROGRAM_OFFSET: usize = 208;
const RECEIPT_GENERATION_OFFSET: usize = 240;
const RECEIPT_OUTCOME_OFFSET: usize = 248;
const RECEIPT_RENT_BEFORE_OFFSET: usize = 256;
const RECEIPT_RENT_AFTER_OFFSET: usize = 264;
const RECEIPT_CREDITED_OFFSET: usize = 272;
const RECEIPT_COORDINATE_COUNT_OFFSET: usize = 280;
const RECEIPT_RESERVED_TAIL_OFFSET: usize = 284;

/// Stable lifecycle refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// Bytes did not have the exact runtime-derived width.
    InvalidLength,
    /// Magic or version selected another protocol.
    InvalidHeader,
    /// Reserved bytes or an action-specific inactive field was noncanonical.
    NonCanonical,
    /// An action tag was unknown.
    UnknownAction,
    /// A required identity was zero or two resource identities aliased.
    InvalidIdentity,
    /// The selected token program cannot create closeable Mints.
    UnsupportedTokenProgram,
    /// Descriptor and request identities, width, or denominator differed.
    DescriptorMismatch,
    /// A coordinate was zero-weight, missing, extra, duplicated, or reordered.
    InvalidSupport,
    /// Pre-funded lamports or current rent principals differed.
    InvalidRent,
    /// Supply, custody, Position revision, or vacancy shape differed.
    InvalidPhysicalState,
    /// Checked native-rent accounting overflowed or did not balance.
    ArithmeticOverflow,
    /// Completion evidence did not join the prepared transition.
    InvalidCompletion,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, Error>;

/// Granular lifecycle action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LifecycleActionV2 {
    /// Initialize the descriptor's closeable Structured receipt Mint.
    ActivateReceipt = 0,
    /// Initialize one exact nonzero-support shard/custody/Position coordinate.
    ActivateCoordinate = 1,
    /// Close one exact zero-supply, empty-custody coordinate.
    RetireCoordinate = 2,
    /// Close the zero-supply receipt Mint after every support row is vacant.
    RetireReceipt = 3,
}

impl LifecycleActionV2 {
    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::ActivateReceipt),
            1 => Ok(Self::ActivateCoordinate),
            2 => Ok(Self::RetireCoordinate),
            3 => Ok(Self::RetireReceipt),
            _ => Err(Error::UnknownAction),
        }
    }

    const fn byte(self) -> u8 {
        match self {
            Self::ActivateReceipt => 0,
            Self::ActivateCoordinate => 1,
            Self::RetireCoordinate => 2,
            Self::RetireReceipt => 3,
        }
    }

    /// Whether this action creates a physical resource.
    pub const fn activates(self) -> bool {
        matches!(self, Self::ActivateReceipt | Self::ActivateCoordinate)
    }

    /// Whether this action closes physical resources into RentCredit.
    pub const fn retires(self) -> bool {
        matches!(self, Self::RetireCoordinate | Self::RetireReceipt)
    }
}

/// Fixed lifecycle request header.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleHeaderV2 {
    /// Granular resource transition.
    pub action: LifecycleActionV2,
    /// Immutable selected execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market identity.
    pub market: [u8; 32],
    /// Immutable representation graph identity.
    pub graph_id: [u8; 32],
    /// Finalized descriptor digest.
    pub descriptor_id: [u8; 32],
    /// Complete Trading parent request/replay context.
    pub parent_context: [u8; 32],
    /// Canonical Claims representation-authority PDA.
    pub representation_authority: [u8; 32],
    /// Canonical closeable Structured receipt Mint PDA.
    pub receipt_mint: [u8; 32],
    /// Exact Token-2022 program.
    pub token_program: [u8; 32],
    /// Permanent RentCredit receiving closed account lamports.
    pub rent_credit: [u8; 32],
    /// Executable program owning the RentCredit.
    pub rent_program: [u8; 32],
    /// Immutable Market generation.
    pub generation: u64,
    /// Chain-observed Claims Market revision.
    pub expected_claims_market_revision: u64,
    /// Complete observed/prepaid receipt Mint lamports.
    pub observed_receipt_lamports: u64,
    /// Current Rent minimum for the closeable receipt Mint width.
    pub receipt_rent_principal: u64,
    /// Chain-observed receipt supply; zero on every lifecycle transition.
    pub expected_receipt_supply: u64,
    /// Product-owned outcome width.
    pub outcome_count: u32,
    /// Number of encoded coordinate rows.
    pub coordinate_count: u32,
    /// Chain-observed RentCredit lamports before execution.
    pub rent_credit_before: u64,
    /// Exact required RentCredit lamports after execution.
    pub rent_credit_after: u64,
}

impl LifecycleHeaderV2 {
    /// Decode one exact fixed header.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() < LIFECYCLE_HEADER_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        exact(input, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
        if read_u16(input, 8)? != LIFECYCLE_VERSION_V2 {
            return Err(Error::InvalidHeader);
        }
        require_zero(input, HEADER_RESERVED_OFFSET, 5)?;
        Ok(Self {
            action: LifecycleActionV2::decode(read_byte(input, ACTION_OFFSET)?)?,
            release_set: read_array(input, RELEASE_SET_OFFSET)?,
            market: read_array(input, MARKET_OFFSET)?,
            graph_id: read_array(input, GRAPH_ID_OFFSET)?,
            descriptor_id: read_array(input, DESCRIPTOR_ID_OFFSET)?,
            parent_context: read_array(input, PARENT_CONTEXT_OFFSET)?,
            representation_authority: read_array(input, REPRESENTATION_AUTHORITY_OFFSET)?,
            receipt_mint: read_array(input, RECEIPT_MINT_OFFSET)?,
            token_program: read_array(input, TOKEN_PROGRAM_OFFSET)?,
            rent_credit: read_array(input, RENT_CREDIT_OFFSET)?,
            rent_program: read_array(input, RENT_PROGRAM_OFFSET)?,
            generation: read_u64(input, GENERATION_OFFSET)?,
            expected_claims_market_revision: read_u64(input, EXPECTED_MARKET_REVISION_OFFSET)?,
            observed_receipt_lamports: read_u64(input, OBSERVED_RECEIPT_LAMPORTS_OFFSET)?,
            receipt_rent_principal: read_u64(input, RECEIPT_RENT_PRINCIPAL_OFFSET)?,
            expected_receipt_supply: read_u64(input, EXPECTED_RECEIPT_SUPPLY_OFFSET)?,
            outcome_count: read_u32(input, OUTCOME_COUNT_OFFSET)?,
            coordinate_count: read_u32(input, COORDINATE_COUNT_OFFSET)?,
            rent_credit_before: read_u64(input, RENT_CREDIT_BEFORE_OFFSET)?,
            rent_credit_after: read_u64(input, RENT_CREDIT_AFTER_OFFSET)?,
        })
    }

    fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() < LIFECYCLE_HEADER_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        output
            .get_mut(..LIFECYCLE_HEADER_BYTES_V2)
            .ok_or(Error::InvalidLength)?
            .fill(0);
        put(output, 0, &LIFECYCLE_REQUEST_MAGIC_V2)?;
        put(output, 8, &LIFECYCLE_VERSION_V2.to_le_bytes())?;
        put_byte(output, ACTION_OFFSET, self.action.byte())?;
        put(output, RELEASE_SET_OFFSET, &self.release_set)?;
        put(output, MARKET_OFFSET, &self.market)?;
        put(output, GRAPH_ID_OFFSET, &self.graph_id)?;
        put(output, DESCRIPTOR_ID_OFFSET, &self.descriptor_id)?;
        put(output, PARENT_CONTEXT_OFFSET, &self.parent_context)?;
        put(
            output,
            REPRESENTATION_AUTHORITY_OFFSET,
            &self.representation_authority,
        )?;
        put(output, RECEIPT_MINT_OFFSET, &self.receipt_mint)?;
        put(output, TOKEN_PROGRAM_OFFSET, &self.token_program)?;
        put(output, RENT_CREDIT_OFFSET, &self.rent_credit)?;
        put(output, RENT_PROGRAM_OFFSET, &self.rent_program)?;
        put(output, GENERATION_OFFSET, &self.generation.to_le_bytes())?;
        put(
            output,
            EXPECTED_MARKET_REVISION_OFFSET,
            &self.expected_claims_market_revision.to_le_bytes(),
        )?;
        put(
            output,
            OBSERVED_RECEIPT_LAMPORTS_OFFSET,
            &self.observed_receipt_lamports.to_le_bytes(),
        )?;
        put(
            output,
            RECEIPT_RENT_PRINCIPAL_OFFSET,
            &self.receipt_rent_principal.to_le_bytes(),
        )?;
        put(
            output,
            EXPECTED_RECEIPT_SUPPLY_OFFSET,
            &self.expected_receipt_supply.to_le_bytes(),
        )?;
        put(
            output,
            OUTCOME_COUNT_OFFSET,
            &self.outcome_count.to_le_bytes(),
        )?;
        put(
            output,
            COORDINATE_COUNT_OFFSET,
            &self.coordinate_count.to_le_bytes(),
        )?;
        put(
            output,
            RENT_CREDIT_BEFORE_OFFSET,
            &self.rent_credit_before.to_le_bytes(),
        )?;
        put(
            output,
            RENT_CREDIT_AFTER_OFFSET,
            &self.rent_credit_after.to_le_bytes(),
        )
    }
}

/// One exact nonzero-support coordinate observation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleCoordinateV2 {
    /// Semantic Product outcome.
    pub outcome: u32,
    /// Exact nonzero descriptor coefficient.
    pub coefficient: u64,
    /// Canonical closeable shard Mint PDA.
    pub shard_mint: [u8; 32],
    /// Canonical Claims-derived closeable Structured custody token account.
    pub structured_custody_account: [u8; 32],
    /// Canonical Claims custody-owner PDA.
    pub claims_custody_owner: [u8; 32],
    /// Canonical LBV2 Claims custody Position PDA.
    pub claims_custody_position: [u8; 32],
    /// Canonical protocol-Position admission PDA.
    pub position_admission: [u8; 32],
    /// Complete observed/prepaid shard Mint lamports.
    pub observed_shard_lamports: u64,
    /// Complete observed/prepaid Structured custody-account lamports.
    pub observed_structured_lamports: u64,
    /// Complete observed/prepaid custody Position lamports.
    pub observed_position_lamports: u64,
    /// Complete observed/prepaid Position admission lamports.
    pub observed_admission_lamports: u64,
    /// Current shard Mint Rent minimum.
    pub shard_rent_principal: u64,
    /// Current Structured custody-account Rent minimum.
    pub structured_rent_principal: u64,
    /// Current LBV2 Position Rent minimum.
    pub position_rent_principal: u64,
    /// Current Position admission Rent minimum.
    pub admission_rent_principal: u64,
    /// Chain-observed shard supply.
    pub expected_shard_supply: u64,
    /// Chain-observed Structured custody amount.
    pub expected_structured_amount: u64,
    /// Chain-observed LBV2 custody Position revision.
    pub expected_position_revision: u64,
}

impl LifecycleCoordinateV2 {
    /// Decode one exact coordinate.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != LIFECYCLE_COORDINATE_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        require_zero(input, ROW_RESERVED_HEAD_OFFSET, 4)?;
        require_zero(input, ROW_RESERVED_TAIL_OFFSET, 8)?;
        Ok(Self {
            outcome: read_u32(input, ROW_OUTCOME_OFFSET)?,
            coefficient: read_u64(input, ROW_COEFFICIENT_OFFSET)?,
            shard_mint: read_array(input, ROW_SHARD_MINT_OFFSET)?,
            structured_custody_account: read_array(input, ROW_STRUCTURED_CUSTODY_OFFSET)?,
            claims_custody_owner: read_array(input, ROW_CUSTODY_OWNER_OFFSET)?,
            claims_custody_position: read_array(input, ROW_CUSTODY_POSITION_OFFSET)?,
            position_admission: read_array(input, ROW_POSITION_ADMISSION_OFFSET)?,
            observed_shard_lamports: read_u64(input, ROW_SHARD_LAMPORTS_OFFSET)?,
            observed_structured_lamports: read_u64(input, ROW_STRUCTURED_LAMPORTS_OFFSET)?,
            observed_position_lamports: read_u64(input, ROW_POSITION_LAMPORTS_OFFSET)?,
            observed_admission_lamports: read_u64(input, ROW_ADMISSION_LAMPORTS_OFFSET)?,
            shard_rent_principal: read_u64(input, ROW_SHARD_RENT_OFFSET)?,
            structured_rent_principal: read_u64(input, ROW_STRUCTURED_RENT_OFFSET)?,
            position_rent_principal: read_u64(input, ROW_POSITION_RENT_OFFSET)?,
            admission_rent_principal: read_u64(input, ROW_ADMISSION_RENT_OFFSET)?,
            expected_shard_supply: read_u64(input, ROW_SHARD_SUPPLY_OFFSET)?,
            expected_structured_amount: read_u64(input, ROW_STRUCTURED_AMOUNT_OFFSET)?,
            expected_position_revision: read_u64(input, ROW_POSITION_REVISION_OFFSET)?,
        })
    }

    /// Encode one exact coordinate.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        if output.len() != LIFECYCLE_COORDINATE_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        output.fill(0);
        put(output, ROW_OUTCOME_OFFSET, &self.outcome.to_le_bytes())?;
        put(
            output,
            ROW_COEFFICIENT_OFFSET,
            &self.coefficient.to_le_bytes(),
        )?;
        put(output, ROW_SHARD_MINT_OFFSET, &self.shard_mint)?;
        put(
            output,
            ROW_STRUCTURED_CUSTODY_OFFSET,
            &self.structured_custody_account,
        )?;
        put(output, ROW_CUSTODY_OWNER_OFFSET, &self.claims_custody_owner)?;
        put(
            output,
            ROW_CUSTODY_POSITION_OFFSET,
            &self.claims_custody_position,
        )?;
        put(
            output,
            ROW_POSITION_ADMISSION_OFFSET,
            &self.position_admission,
        )?;
        for (offset, value) in [
            (ROW_SHARD_LAMPORTS_OFFSET, self.observed_shard_lamports),
            (
                ROW_STRUCTURED_LAMPORTS_OFFSET,
                self.observed_structured_lamports,
            ),
            (
                ROW_POSITION_LAMPORTS_OFFSET,
                self.observed_position_lamports,
            ),
            (
                ROW_ADMISSION_LAMPORTS_OFFSET,
                self.observed_admission_lamports,
            ),
            (ROW_SHARD_RENT_OFFSET, self.shard_rent_principal),
            (ROW_STRUCTURED_RENT_OFFSET, self.structured_rent_principal),
            (ROW_POSITION_RENT_OFFSET, self.position_rent_principal),
            (ROW_ADMISSION_RENT_OFFSET, self.admission_rent_principal),
            (ROW_SHARD_SUPPLY_OFFSET, self.expected_shard_supply),
            (
                ROW_STRUCTURED_AMOUNT_OFFSET,
                self.expected_structured_amount,
            ),
            (
                ROW_POSITION_REVISION_OFFSET,
                self.expected_position_revision,
            ),
        ] {
            put(output, offset, &value.to_le_bytes())?;
        }
        Ok(())
    }

    fn observed_lamports(self) -> Result<u64> {
        self.observed_shard_lamports
            .checked_add(self.observed_structured_lamports)
            .and_then(|value| value.checked_add(self.observed_position_lamports))
            .and_then(|value| value.checked_add(self.observed_admission_lamports))
            .ok_or(Error::ArithmeticOverflow)
    }
}

/// Borrowed exact lifecycle request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleRequestV2<'a> {
    header: LifecycleHeaderV2,
    coordinate_bytes: &'a [u8],
}

impl<'a> LifecycleRequestV2<'a> {
    /// Construct a request from a header and exact encoded coordinate rows.
    pub fn new(header: LifecycleHeaderV2, coordinate_bytes: &'a [u8]) -> Result<Self> {
        let expected = usize::try_from(header.coordinate_count)
            .map_err(|_| Error::InvalidLength)?
            .checked_mul(LIFECYCLE_COORDINATE_BYTES_V2)
            .ok_or(Error::InvalidLength)?;
        if coordinate_bytes.len() != expected {
            return Err(Error::InvalidLength);
        }
        let request = Self {
            header,
            coordinate_bytes,
        };
        request.validate_header()?;
        for coordinate in request.coordinates() {
            coordinate?;
        }
        Ok(request)
    }

    /// Hostile-decode one exact runtime-width request.
    pub fn decode(input: &'a [u8]) -> Result<Self> {
        let header = LifecycleHeaderV2::decode(input)?;
        let coordinate_bytes = input
            .get(LIFECYCLE_HEADER_BYTES_V2..)
            .ok_or(Error::InvalidLength)?;
        Self::new(header, coordinate_bytes)
    }

    /// Return the fixed request header.
    pub const fn header(self) -> LifecycleHeaderV2 {
        self.header
    }

    /// Iterate exact coordinate rows.
    pub const fn coordinates(self) -> LifecycleCoordinateIterV2<'a> {
        LifecycleCoordinateIterV2 {
            remaining: self.coordinate_bytes,
        }
    }

    /// Encode the request into an exact caller-owned output buffer.
    pub fn encode_into(self, output: &mut [u8]) -> Result<()> {
        let expected = LIFECYCLE_HEADER_BYTES_V2
            .checked_add(self.coordinate_bytes.len())
            .ok_or(Error::InvalidLength)?;
        if output.len() != expected {
            return Err(Error::InvalidLength);
        }
        self.header.encode_into(output)?;
        output
            .get_mut(LIFECYCLE_HEADER_BYTES_V2..)
            .ok_or(Error::InvalidLength)?
            .copy_from_slice(self.coordinate_bytes);
        Ok(())
    }

    fn validate_header(self) -> Result<()> {
        let header = self.header;
        for identity in [
            header.release_set,
            header.market,
            header.graph_id,
            header.descriptor_id,
            header.parent_context,
            header.representation_authority,
            header.receipt_mint,
            header.token_program,
            header.rent_credit,
            header.rent_program,
        ] {
            require_nonzero(identity)?;
        }
        if header.token_program != TOKEN_2022_PROGRAM_ID {
            return Err(Error::UnsupportedTokenProgram);
        }
        if header.outcome_count == 0
            || header.receipt_rent_principal == 0
            || header.expected_receipt_supply != 0
            || header.observed_receipt_lamports < header.receipt_rent_principal
        {
            return Err(Error::InvalidPhysicalState);
        }
        require_distinct(&[
            header.market,
            header.descriptor_id,
            header.representation_authority,
            header.receipt_mint,
            header.token_program,
            header.rent_credit,
            header.rent_program,
        ])?;
        match header.action {
            LifecycleActionV2::ActivateReceipt => {
                if header.coordinate_count != 0
                    || header.rent_credit_after != header.rent_credit_before
                {
                    return Err(Error::NonCanonical);
                }
            }
            LifecycleActionV2::ActivateCoordinate => {
                if header.coordinate_count != 1
                    || header.rent_credit_after != header.rent_credit_before
                {
                    return Err(Error::NonCanonical);
                }
            }
            LifecycleActionV2::RetireCoordinate => {
                if header.coordinate_count != 1 {
                    return Err(Error::NonCanonical);
                }
            }
            LifecycleActionV2::RetireReceipt => {}
        }
        Ok(())
    }
}

/// Iterator over borrowed coordinate rows.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleCoordinateIterV2<'a> {
    remaining: &'a [u8],
}

impl Iterator for LifecycleCoordinateIterV2<'_> {
    type Item = Result<LifecycleCoordinateV2>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.remaining.is_empty() {
            return None;
        }
        let (current, rest) = match self
            .remaining
            .split_at_checked(LIFECYCLE_COORDINATE_BYTES_V2)
        {
            Some(value) => value,
            None => {
                self.remaining = &[];
                return Some(Err(Error::InvalidLength));
            }
        };
        self.remaining = rest;
        Some(LifecycleCoordinateV2::decode(current))
    }
}

/// Prepared lifecycle transition after exact descriptor/support/rent checks.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedLifecycleV2 {
    header: LifecycleHeaderV2,
    selected_outcome: u32,
    expected_credit: u64,
}

impl PreparedLifecycleV2 {
    /// Return the granular action.
    pub const fn action(self) -> LifecycleActionV2 {
        self.header.action
    }

    /// Return the descriptor identity.
    pub const fn descriptor_id(self) -> [u8; 32] {
        self.header.descriptor_id
    }

    /// Return the exact selected outcome or [`ABSENT_OUTCOME_V2`].
    pub const fn selected_outcome(self) -> u32 {
        self.selected_outcome
    }

    /// Return the exact rent credited by this transition.
    pub const fn expected_credit(self) -> u64 {
        self.expected_credit
    }

    /// Return the exact coordinate row count.
    pub const fn coordinate_count(self) -> u32 {
        self.header.coordinate_count
    }
}

/// Authenticate one request against the finalized descriptor and exact
/// ordered nonzero coefficient support.
pub fn prepare(
    request: LifecycleRequestV2<'_>,
    descriptor: RepresentationDescriptorV2<'_>,
) -> Result<PreparedLifecycleV2> {
    let header = request.header;
    if descriptor.descriptor_id() != header.descriptor_id
        || descriptor.graph_id() != header.graph_id
        || descriptor.market_id() != header.market
        || descriptor.release_set_id() != header.release_set
        || descriptor.representation_authority() != header.representation_authority
        || descriptor.receipt_mint() != header.receipt_mint
        || descriptor.token_program() != header.token_program
        || descriptor.outcome_count() != header.outcome_count
    {
        return Err(Error::DescriptorMismatch);
    }
    let support_count = nonzero_support_count(descriptor)?;
    let selected_outcome = match header.action {
        LifecycleActionV2::ActivateReceipt => ABSENT_OUTCOME_V2,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate => {
            let coordinate = request
                .coordinates()
                .next()
                .ok_or(Error::InvalidSupport)??;
            authenticate_coordinate(descriptor, header, coordinate, false)?;
            coordinate.outcome
        }
        LifecycleActionV2::RetireReceipt => {
            if header.coordinate_count != support_count {
                return Err(Error::InvalidSupport);
            }
            authenticate_complete_support(request, descriptor)?;
            ABSENT_OUTCOME_V2
        }
    };
    let expected_credit = match header.action {
        LifecycleActionV2::ActivateReceipt | LifecycleActionV2::ActivateCoordinate => 0,
        LifecycleActionV2::RetireCoordinate => request
            .coordinates()
            .next()
            .ok_or(Error::InvalidSupport)??
            .observed_lamports()?,
        LifecycleActionV2::RetireReceipt => header.observed_receipt_lamports,
    };
    let expected_after = header
        .rent_credit_before
        .checked_add(expected_credit)
        .ok_or(Error::ArithmeticOverflow)?;
    if header.rent_credit_after != expected_after {
        return Err(Error::InvalidRent);
    }
    Ok(PreparedLifecycleV2 {
        header,
        selected_outcome,
        expected_credit,
    })
}

fn nonzero_support_count(descriptor: RepresentationDescriptorV2<'_>) -> Result<u32> {
    let mut count = 0_u32;
    for outcome in 0..descriptor.outcome_count() {
        if descriptor
            .coefficient(outcome)
            .map_err(|_| Error::DescriptorMismatch)?
            != 0
        {
            count = count.checked_add(1).ok_or(Error::ArithmeticOverflow)?;
        }
    }
    Ok(count)
}

fn authenticate_complete_support(
    request: LifecycleRequestV2<'_>,
    descriptor: RepresentationDescriptorV2<'_>,
) -> Result<()> {
    let mut rows = request.coordinates();
    let mut prior_outcome = None;
    for outcome in 0..descriptor.outcome_count() {
        let coefficient = descriptor
            .coefficient(outcome)
            .map_err(|_| Error::DescriptorMismatch)?;
        if coefficient == 0 {
            continue;
        }
        let row = rows.next().ok_or(Error::InvalidSupport)??;
        if row.outcome != outcome || prior_outcome.is_some_and(|prior| prior >= row.outcome) {
            return Err(Error::InvalidSupport);
        }
        authenticate_coordinate(descriptor, request.header, row, true)?;
        prior_outcome = Some(row.outcome);
    }
    if rows.next().is_some() {
        return Err(Error::InvalidSupport);
    }
    authenticate_global_aliases(request)
}

fn authenticate_coordinate(
    descriptor: RepresentationDescriptorV2<'_>,
    header: LifecycleHeaderV2,
    row: LifecycleCoordinateV2,
    vacant: bool,
) -> Result<()> {
    if row.outcome >= descriptor.outcome_count()
        || row.coefficient == 0
        || descriptor
            .coefficient(row.outcome)
            .map_err(|_| Error::DescriptorMismatch)?
            != row.coefficient
    {
        return Err(Error::InvalidSupport);
    }
    for identity in [
        row.shard_mint,
        row.structured_custody_account,
        row.claims_custody_owner,
        row.claims_custody_position,
        row.position_admission,
    ] {
        require_nonzero(identity)?;
    }
    require_distinct(&[
        header.receipt_mint,
        header.representation_authority,
        header.rent_credit,
        row.shard_mint,
        row.structured_custody_account,
        row.claims_custody_owner,
        row.claims_custody_position,
        row.position_admission,
    ])?;
    if vacant {
        if row.observed_shard_lamports != 0
            || row.observed_structured_lamports != 0
            || row.observed_position_lamports != 0
            || row.observed_admission_lamports != 0
            || row.shard_rent_principal != 0
            || row.structured_rent_principal != 0
            || row.position_rent_principal != 0
            || row.admission_rent_principal != 0
            || row.expected_shard_supply != 0
            || row.expected_structured_amount != 0
            || row.expected_position_revision != ABSENT_POSITION_REVISION_V2
        {
            return Err(Error::InvalidPhysicalState);
        }
        return Ok(());
    }
    if row.shard_rent_principal == 0
        || row.structured_rent_principal == 0
        || row.position_rent_principal == 0
        || row.admission_rent_principal == 0
        || row.observed_shard_lamports < row.shard_rent_principal
        || row.observed_structured_lamports < row.structured_rent_principal
        || row.observed_position_lamports < row.position_rent_principal
        || row.observed_admission_lamports < row.admission_rent_principal
        || row.expected_shard_supply != 0
        || row.expected_structured_amount != 0
    {
        return Err(Error::InvalidPhysicalState);
    }
    if header.action == LifecycleActionV2::ActivateCoordinate && row.expected_position_revision != 0
    {
        return Err(Error::InvalidPhysicalState);
    }
    if header.action == LifecycleActionV2::RetireCoordinate
        && row.expected_position_revision == ABSENT_POSITION_REVISION_V2
    {
        return Err(Error::InvalidPhysicalState);
    }
    Ok(())
}

fn authenticate_global_aliases(request: LifecycleRequestV2<'_>) -> Result<()> {
    for (index, left) in request.coordinates().enumerate() {
        let left = left?;
        for right in request
            .coordinates()
            .skip(index.checked_add(1).ok_or(Error::ArithmeticOverflow)?)
        {
            let right = right?;
            for left_key in [
                left.shard_mint,
                left.structured_custody_account,
                left.claims_custody_owner,
                left.claims_custody_position,
                left.position_admission,
            ] {
                if [
                    right.shard_mint,
                    right.structured_custody_account,
                    right.claims_custody_owner,
                    right.claims_custody_position,
                    right.position_admission,
                ]
                .contains(&left_key)
                {
                    return Err(Error::InvalidIdentity);
                }
            }
        }
    }
    Ok(())
}

/// Adapter completion evidence supplied only after all child transitions and
/// exact postconditions succeed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleCompletionEvidenceV2 {
    /// SHA-256 of the complete lifecycle request.
    pub request_digest: [u8; 32],
    /// Recomputed finalized descriptor digest.
    pub descriptor_digest: [u8; 32],
    /// Digest of all final physical resource observations.
    pub post_resource_digest: [u8; 32],
    /// Direct protocol-Position Admit/Close receipt digest for coordinate
    /// actions; zero for receipt-wide actions.
    pub position_lifecycle_receipt_digest: [u8; 32],
    /// Observed RentCredit balance before execution.
    pub rent_credit_before: u64,
    /// Observed RentCredit balance after execution.
    pub rent_credit_after: u64,
    /// Current Trading caller/release binding was reauthenticated.
    pub caller_authenticated: bool,
    /// Finalized descriptor record and derived resource PDAs were authenticated.
    pub descriptor_and_resources_authenticated: bool,
    /// Token-2022 and protocol-Position effects reached exact postconditions.
    pub physical_effects_committed: bool,
}

/// Immediate state-last lifecycle receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LifecycleReceiptV2 {
    action: LifecycleActionV2,
    request_digest: [u8; 32],
    descriptor_id: [u8; 32],
    market: [u8; 32],
    post_resource_digest: [u8; 32],
    position_lifecycle_receipt_digest: [u8; 32],
    rent_credit: [u8; 32],
    rent_program: [u8; 32],
    generation: u64,
    outcome: u32,
    rent_credit_before: u64,
    rent_credit_after: u64,
    credited_lamports: u64,
    coordinate_count: u32,
}

impl LifecycleReceiptV2 {
    /// Decode one exact receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        if input.len() != LIFECYCLE_RECEIPT_BYTES_V2 {
            return Err(Error::InvalidLength);
        }
        exact(input, 0, &LIFECYCLE_RECEIPT_MAGIC_V2)?;
        if read_u16(input, 8)? != LIFECYCLE_VERSION_V2
            || read_byte(input, RECEIPT_STATUS_OFFSET)? != 1
        {
            return Err(Error::InvalidHeader);
        }
        require_zero(input, RECEIPT_RESERVED_HEAD_OFFSET, 4)?;
        require_zero(input, RECEIPT_RESERVED_TAIL_OFFSET, 36)?;
        Ok(Self {
            action: LifecycleActionV2::decode(read_byte(input, RECEIPT_ACTION_OFFSET)?)?,
            request_digest: read_array(input, RECEIPT_REQUEST_DIGEST_OFFSET)?,
            descriptor_id: read_array(input, RECEIPT_DESCRIPTOR_ID_OFFSET)?,
            market: read_array(input, RECEIPT_MARKET_OFFSET)?,
            post_resource_digest: read_array(input, RECEIPT_POST_RESOURCE_DIGEST_OFFSET)?,
            position_lifecycle_receipt_digest: read_array(
                input,
                RECEIPT_POSITION_RECEIPT_DIGEST_OFFSET,
            )?,
            rent_credit: read_array(input, RECEIPT_RENT_CREDIT_OFFSET)?,
            rent_program: read_array(input, RECEIPT_RENT_PROGRAM_OFFSET)?,
            generation: read_u64(input, RECEIPT_GENERATION_OFFSET)?,
            outcome: read_u32(input, RECEIPT_OUTCOME_OFFSET)?,
            rent_credit_before: read_u64(input, RECEIPT_RENT_BEFORE_OFFSET)?,
            rent_credit_after: read_u64(input, RECEIPT_RENT_AFTER_OFFSET)?,
            credited_lamports: read_u64(input, RECEIPT_CREDITED_OFFSET)?,
            coordinate_count: read_u32(input, RECEIPT_COORDINATE_COUNT_OFFSET)?,
        })
    }

    /// Encode one exact receipt.
    pub fn to_bytes(self) -> Result<[u8; LIFECYCLE_RECEIPT_BYTES_V2]> {
        let mut output = [0; LIFECYCLE_RECEIPT_BYTES_V2];
        put(&mut output, 0, &LIFECYCLE_RECEIPT_MAGIC_V2)?;
        put(&mut output, 8, &LIFECYCLE_VERSION_V2.to_le_bytes())?;
        put_byte(&mut output, RECEIPT_ACTION_OFFSET, self.action.byte())?;
        put_byte(&mut output, RECEIPT_STATUS_OFFSET, 1)?;
        put(
            &mut output,
            RECEIPT_REQUEST_DIGEST_OFFSET,
            &self.request_digest,
        )?;
        put(
            &mut output,
            RECEIPT_DESCRIPTOR_ID_OFFSET,
            &self.descriptor_id,
        )?;
        put(&mut output, RECEIPT_MARKET_OFFSET, &self.market)?;
        put(
            &mut output,
            RECEIPT_POST_RESOURCE_DIGEST_OFFSET,
            &self.post_resource_digest,
        )?;
        put(
            &mut output,
            RECEIPT_POSITION_RECEIPT_DIGEST_OFFSET,
            &self.position_lifecycle_receipt_digest,
        )?;
        put(&mut output, RECEIPT_RENT_CREDIT_OFFSET, &self.rent_credit)?;
        put(&mut output, RECEIPT_RENT_PROGRAM_OFFSET, &self.rent_program)?;
        put(
            &mut output,
            RECEIPT_GENERATION_OFFSET,
            &self.generation.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_OUTCOME_OFFSET,
            &self.outcome.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_RENT_BEFORE_OFFSET,
            &self.rent_credit_before.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_RENT_AFTER_OFFSET,
            &self.rent_credit_after.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_CREDITED_OFFSET,
            &self.credited_lamports.to_le_bytes(),
        )?;
        put(
            &mut output,
            RECEIPT_COORDINATE_COUNT_OFFSET,
            &self.coordinate_count.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Return the action.
    pub const fn action(self) -> LifecycleActionV2 {
        self.action
    }

    /// Return the complete request digest.
    pub const fn request_digest(self) -> [u8; 32] {
        self.request_digest
    }

    /// Return the selected outcome or absent sentinel.
    pub const fn outcome(self) -> u32 {
        self.outcome
    }

    /// Return exact reclaimed native rent.
    pub const fn credited_lamports(self) -> u64 {
        self.credited_lamports
    }
}

/// Finalize one prepared transition only after exact physical postconditions.
pub fn finalize(
    prepared: PreparedLifecycleV2,
    evidence: LifecycleCompletionEvidenceV2,
) -> Result<LifecycleReceiptV2> {
    let header = prepared.header;
    let coordinate_action = matches!(
        header.action,
        LifecycleActionV2::ActivateCoordinate | LifecycleActionV2::RetireCoordinate
    );
    if is_zero(&evidence.request_digest)
        || evidence.descriptor_digest != header.descriptor_id
        || is_zero(&evidence.post_resource_digest)
        || evidence.rent_credit_before != header.rent_credit_before
        || evidence.rent_credit_after != header.rent_credit_after
        || !evidence.caller_authenticated
        || !evidence.descriptor_and_resources_authenticated
        || !evidence.physical_effects_committed
        || coordinate_action == is_zero(&evidence.position_lifecycle_receipt_digest)
    {
        return Err(Error::InvalidCompletion);
    }
    Ok(LifecycleReceiptV2 {
        action: header.action,
        request_digest: evidence.request_digest,
        descriptor_id: header.descriptor_id,
        market: header.market,
        post_resource_digest: evidence.post_resource_digest,
        position_lifecycle_receipt_digest: evidence.position_lifecycle_receipt_digest,
        rent_credit: header.rent_credit,
        rent_program: header.rent_program,
        generation: header.generation,
        outcome: prepared.selected_outcome,
        rent_credit_before: header.rent_credit_before,
        rent_credit_after: header.rent_credit_after,
        credited_lamports: prepared.expected_credit,
        coordinate_count: header.coordinate_count,
    })
}

fn require_nonzero(value: [u8; 32]) -> Result<()> {
    if is_zero(&value) {
        Err(Error::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn require_distinct(values: &[[u8; 32]]) -> Result<()> {
    for (index, left) in values.iter().enumerate() {
        if values
            .iter()
            .skip(index.checked_add(1).ok_or(Error::ArithmeticOverflow)?)
            .any(|right| right == left)
        {
            return Err(Error::InvalidIdentity);
        }
    }
    Ok(())
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
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
    input
        .get(offset..offset.checked_add(N).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .try_into()
        .map_err(|_| Error::InvalidLength)
}

fn exact(input: &[u8], offset: usize, expected: &[u8]) -> Result<()> {
    if input
        .get(
            offset
                ..offset
                    .checked_add(expected.len())
                    .ok_or(Error::InvalidLength)?,
        )
        .ok_or(Error::InvalidLength)?
        != expected
    {
        return Err(Error::InvalidHeader);
    }
    Ok(())
}

fn require_zero(input: &[u8], offset: usize, width: usize) -> Result<()> {
    if input
        .get(offset..offset.checked_add(width).ok_or(Error::InvalidLength)?)
        .ok_or(Error::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(Error::NonCanonical);
    }
    Ok(())
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

fn put_byte(output: &mut [u8], offset: usize, value: u8) -> Result<()> {
    *output.get_mut(offset).ok_or(Error::InvalidLength)? = value;
    Ok(())
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests;
