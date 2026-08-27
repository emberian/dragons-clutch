//! Exposure-bound Fractional request with distinct Product and Claims coordinates.

use core::convert::TryInto;

use dclutch_fractional_claim_kernel::FractionalExposureTermsV2;

/// Exact fixed V2 family-request width.
pub const FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2: usize = 416;
/// Exact V2 family-request magic.
pub const FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2: [u8; 8] = *b"DCFREQ02";
/// V2 request schema preimage.
pub const FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_PREIMAGE_V2: &[u8] = b"dclutch/schema/fractional-exposure-request-v2|bytes416|terms-select-exposure|representation-coordinate|terminal-record-digest|no-terminal-value-projection|no-payout-input";
/// SHA-256 of [`FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_PREIMAGE_V2`].
pub const FRACTIONAL_EXPOSURE_REQUEST_SCHEMA_ID_V2: [u8; 32] = [
    0x35, 0x7c, 0xcb, 0x60, 0x5a, 0x4c, 0x37, 0xea, 0x2f, 0x5c, 0x63, 0x36, 0x1e, 0x4d, 0x85, 0x99,
    0x95, 0x0d, 0xc8, 0x35, 0x6a, 0x5b, 0xb9, 0xc1, 0xc6, 0x08, 0xa6, 0xfa, 0x29, 0x0e, 0x9b, 0x59,
];
/// Canonical absent Product or Claims coordinate.
pub const NO_EXPOSURE_COORDINATE_V2: u32 = u32::MAX;

const VERSION_V2: u16 = 2;
const ACTION_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const PRODUCT_RECORD_OFFSET: usize = 80;
const RESULT_DOMAIN_OFFSET: usize = 112;
const TERMS_OFFSET: usize = 144;
const TOKEN_BEHAVIOR_OFFSET: usize = 176;
const EXPOSURE_OFFSET: usize = 208;
const OWNER_OFFSET: usize = 240;
const SOURCE_TOKEN_OFFSET: usize = 272;
const DESTINATION_TOKEN_OFFSET: usize = 304;
const TERMINAL_DIGEST_OFFSET: usize = 336;
const EXPECTED_REVISION_OFFSET: usize = 368;
const QUANTITY_OFFSET: usize = 376;
const REPRESENTATION_COORDINATE_OFFSET: usize = 384;
const TAIL_RESERVED_OFFSET: usize = 388;

/// Exact V2 Fractional action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FractionalExposureActionV2 {
    /// Lock one Claims coordinate and mint its exact denominator multiple.
    Wrap = 0,
    /// Transfer raw atoms of one terms-selected shard Mint.
    Transfer = 1,
    /// Burn only a whole-denominator multiple and unlock one Claims coordinate.
    WholeUnwrap = 2,
    /// Burn whole shards after the authenticated exposure evaluates positive payout.
    TerminalRedeem = 3,
    /// Burn whole shards after the authenticated exposure evaluates zero payout.
    TerminalZeroBurn = 4,
    /// Bind the Fractional root to one authenticated Product terminal coordinate.
    Terminalize = 5,
    /// Retire after all K shard supplies and Claims reserves are zero.
    ZeroSupplyRetire = 6,
}

impl FractionalExposureActionV2 {
    /// Stable action discriminator.
    pub const fn byte(self) -> u8 {
        self as u8
    }

    /// Whether this action carries a positive amount.
    pub const fn carries_quantity(self) -> bool {
        matches!(
            self,
            Self::Wrap
                | Self::Transfer
                | Self::WholeUnwrap
                | Self::TerminalRedeem
                | Self::TerminalZeroBurn
        )
    }

    /// Whether this action selects one Claims representation coordinate.
    pub const fn selects_representation_coordinate(self) -> bool {
        self.carries_quantity()
    }

    /// Whether this action requires authenticated terminal Product evidence.
    pub const fn requires_terminal(self) -> bool {
        matches!(
            self,
            Self::TerminalRedeem
                | Self::TerminalZeroBurn
                | Self::Terminalize
                | Self::ZeroSupplyRetire
        )
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Wrap),
            1 => Ok(Self::Transfer),
            2 => Ok(Self::WholeUnwrap),
            3 => Ok(Self::TerminalRedeem),
            4 => Ok(Self::TerminalZeroBurn),
            5 => Ok(Self::Terminalize),
            6 => Ok(Self::ZeroSupplyRetire),
            _ => Err(FractionalExposureRequestErrorV2::UnknownAction),
        }
    }
}

/// Checked fields for one V2 request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureRequestInputV2 {
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized Product root digest.
    pub product_record: [u8; 32],
    /// Product-owned result-domain identity and ordering.
    pub result_domain: [u8; 32],
    /// Finalized Fractional V2 terms identity.
    pub terms: [u8; 32],
    /// Finalized selected TokenBehaviorV2 identity.
    pub token_behavior: [u8; 32],
    /// Finalized Product-N to Claims-K exposure identity.
    pub exposure: [u8; 32],
    /// Wallet or protocol owner for actor-bound actions; zero otherwise.
    pub owner: [u8; 32],
    /// Exact source Token account when active; zero otherwise.
    pub source_token_account: [u8; 32],
    /// Exact destination Token account when active; zero otherwise.
    pub destination_token_account: [u8; 32],
    /// Finalized terminal-coordinate digest; zero for open actions.
    pub terminal_digest: [u8; 32],
    /// Optimistic Fractional-root revision observed from chain.
    pub expected_revision: u64,
    /// Native Claims units or raw shard atoms according to the action.
    pub quantity: u64,
    /// Selected Claims representation coordinate in `[0,K)`.
    pub representation_coordinate: u32,
}

/// Hostile-decoded exposure-bound request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalExposureRequestV2 {
    action: FractionalExposureActionV2,
    input: FractionalExposureRequestInputV2,
}

impl FractionalExposureRequestV2 {
    /// Construct and validate one canonical V2 request.
    pub fn new(
        action: FractionalExposureActionV2,
        input: FractionalExposureRequestInputV2,
    ) -> Result<Self> {
        validate_shape(action, input)?;
        Ok(Self { action, input })
    }

    /// Hostile-decode exact V2 bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2 {
            return Err(FractionalExposureRequestErrorV2::InvalidLength);
        }
        if array::<8>(bytes, 0)? != FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2
            || u16_at(bytes, 8)? != VERSION_V2
        {
            return Err(FractionalExposureRequestErrorV2::InvalidHeader);
        }
        require_zero(bytes, HEADER_RESERVED_OFFSET, 5)?;
        require_zero(bytes, TAIL_RESERVED_OFFSET, 28)?;
        Self::new(
            FractionalExposureActionV2::decode(byte(bytes, ACTION_OFFSET)?)?,
            FractionalExposureRequestInputV2 {
                release_set: array(bytes, RELEASE_SET_OFFSET)?,
                market: array(bytes, MARKET_OFFSET)?,
                product_record: array(bytes, PRODUCT_RECORD_OFFSET)?,
                result_domain: array(bytes, RESULT_DOMAIN_OFFSET)?,
                terms: array(bytes, TERMS_OFFSET)?,
                token_behavior: array(bytes, TOKEN_BEHAVIOR_OFFSET)?,
                exposure: array(bytes, EXPOSURE_OFFSET)?,
                owner: array(bytes, OWNER_OFFSET)?,
                source_token_account: array(bytes, SOURCE_TOKEN_OFFSET)?,
                destination_token_account: array(bytes, DESTINATION_TOKEN_OFFSET)?,
                terminal_digest: array(bytes, TERMINAL_DIGEST_OFFSET)?,
                expected_revision: u64_at(bytes, EXPECTED_REVISION_OFFSET)?,
                quantity: u64_at(bytes, QUANTITY_OFFSET)?,
                representation_coordinate: u32_at(bytes, REPRESENTATION_COORDINATE_OFFSET)?,
            },
        )
    }

    /// Encode exact canonical bytes.
    pub fn to_bytes(self) -> Result<[u8; FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2]> {
        let mut output = [0_u8; FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2];
        put(&mut output, 0, &FRACTIONAL_EXPOSURE_REQUEST_MAGIC_V2)?;
        put(&mut output, 8, &VERSION_V2.to_le_bytes())?;
        *output
            .get_mut(ACTION_OFFSET)
            .ok_or(FractionalExposureRequestErrorV2::InvalidLength)? = self.action.byte();
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.input.release_set),
            (MARKET_OFFSET, self.input.market),
            (PRODUCT_RECORD_OFFSET, self.input.product_record),
            (RESULT_DOMAIN_OFFSET, self.input.result_domain),
            (TERMS_OFFSET, self.input.terms),
            (TOKEN_BEHAVIOR_OFFSET, self.input.token_behavior),
            (EXPOSURE_OFFSET, self.input.exposure),
            (OWNER_OFFSET, self.input.owner),
            (SOURCE_TOKEN_OFFSET, self.input.source_token_account),
            (
                DESTINATION_TOKEN_OFFSET,
                self.input.destination_token_account,
            ),
            (TERMINAL_DIGEST_OFFSET, self.input.terminal_digest),
        ] {
            put(&mut output, offset, &value)?;
        }
        put(
            &mut output,
            EXPECTED_REVISION_OFFSET,
            &self.input.expected_revision.to_le_bytes(),
        )?;
        put(
            &mut output,
            QUANTITY_OFFSET,
            &self.input.quantity.to_le_bytes(),
        )?;
        put(
            &mut output,
            REPRESENTATION_COORDINATE_OFFSET,
            &self.input.representation_coordinate.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Selected action.
    pub const fn action(self) -> FractionalExposureActionV2 {
        self.action
    }

    /// Exact checked fields.
    pub const fn input(self) -> FractionalExposureRequestInputV2 {
        self.input
    }

    /// Bind this request to authenticated V2 terms and their independent widths.
    pub fn bind_terms(self, terms: FractionalExposureTermsV2<'_>) -> Result<Self> {
        let input = self.input;
        if input.release_set != terms.release_set()
            || input.market != terms.market()
            || input.product_record != terms.product_record()
            || input.result_domain != terms.result_domain()
            || input.terms != terms.terms_id()
            || input.token_behavior != terms.token_behavior()
            || input.exposure != terms.exposure_id()
        {
            return Err(FractionalExposureRequestErrorV2::TermsMismatch);
        }
        if self.action.selects_representation_coordinate()
            && input.representation_coordinate >= terms.representation_width()
        {
            return Err(FractionalExposureRequestErrorV2::InvalidCoordinate);
        }
        Ok(self)
    }
}

/// Stable V2 request refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalExposureRequestErrorV2 {
    /// Bytes had another exact width.
    InvalidLength,
    /// Magic or version selected another schema.
    InvalidHeader,
    /// Reserved or inactive fields were noncanonical.
    NonCanonical,
    /// Action tag was unknown.
    UnknownAction,
    /// A required identity was zero or Token accounts aliased.
    InvalidIdentity,
    /// Quantity presence differed from the selected action.
    InvalidQuantity,
    /// Open and terminal fields differed from the selected action.
    InvalidTerminal,
    /// A Product or Claims coordinate was absent or outside authenticated width.
    InvalidCoordinate,
    /// Request identities differed from authenticated finalized V2 terms.
    TermsMismatch,
}

/// Result alias for V2 hostile decoding and admission.
pub type Result<T> = core::result::Result<T, FractionalExposureRequestErrorV2>;

fn validate_shape(
    action: FractionalExposureActionV2,
    input: FractionalExposureRequestInputV2,
) -> Result<()> {
    if [
        input.release_set,
        input.market,
        input.product_record,
        input.result_domain,
        input.terms,
        input.token_behavior,
        input.exposure,
    ]
    .iter()
    .any(is_zero)
    {
        return Err(FractionalExposureRequestErrorV2::InvalidIdentity);
    }
    if action.carries_quantity() != (input.quantity != 0) {
        return Err(FractionalExposureRequestErrorV2::InvalidQuantity);
    }
    if action.selects_representation_coordinate()
        != (input.representation_coordinate != NO_EXPOSURE_COORDINATE_V2)
    {
        return Err(FractionalExposureRequestErrorV2::InvalidCoordinate);
    }
    let terminal_present = !is_zero(&input.terminal_digest);
    if action.requires_terminal() != terminal_present
        || (!action.requires_terminal() && !is_zero(&input.terminal_digest))
    {
        return Err(FractionalExposureRequestErrorV2::InvalidTerminal);
    }
    let owner_active = action.carries_quantity();
    if owner_active == is_zero(&input.owner) {
        return Err(FractionalExposureRequestErrorV2::InvalidIdentity);
    }
    let source_active = matches!(
        action,
        FractionalExposureActionV2::Transfer
            | FractionalExposureActionV2::WholeUnwrap
            | FractionalExposureActionV2::TerminalRedeem
            | FractionalExposureActionV2::TerminalZeroBurn
    );
    let destination_active = matches!(
        action,
        FractionalExposureActionV2::Wrap | FractionalExposureActionV2::Transfer
    );
    if source_active == is_zero(&input.source_token_account)
        || destination_active == is_zero(&input.destination_token_account)
        || (source_active
            && destination_active
            && input.source_token_account == input.destination_token_account)
    {
        return Err(FractionalExposureRequestErrorV2::InvalidIdentity);
    }
    Ok(())
}

fn is_zero(value: &[u8; 32]) -> bool {
    *value == [0; 32]
}

fn array<const N: usize>(bytes: &[u8], offset: usize) -> Result<[u8; N]> {
    bytes
        .get(
            offset
                ..offset
                    .checked_add(N)
                    .ok_or(FractionalExposureRequestErrorV2::InvalidLength)?,
        )
        .ok_or(FractionalExposureRequestErrorV2::InvalidLength)?
        .try_into()
        .map_err(|_| FractionalExposureRequestErrorV2::InvalidLength)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(FractionalExposureRequestErrorV2::InvalidLength)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(bytes, offset)?))
}

fn u64_at(bytes: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(bytes, offset)?))
}

fn require_zero(bytes: &[u8], offset: usize, len: usize) -> Result<()> {
    if bytes
        .get(
            offset
                ..offset
                    .checked_add(len)
                    .ok_or(FractionalExposureRequestErrorV2::InvalidLength)?,
        )
        .ok_or(FractionalExposureRequestErrorV2::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        Err(FractionalExposureRequestErrorV2::NonCanonical)
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
                    .ok_or(FractionalExposureRequestErrorV2::InvalidLength)?,
        )
        .ok_or(FractionalExposureRequestErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}
