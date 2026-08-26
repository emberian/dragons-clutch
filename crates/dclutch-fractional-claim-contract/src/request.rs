//! Canonical family request with one explicit terminal coordinate.

use core::convert::TryInto;

/// Exact fixed family-request width.
pub const FRACTIONAL_FAMILY_REQUEST_BYTES_V1: usize = 384;
/// Family-request magic.
pub const FRACTIONAL_FAMILY_REQUEST_MAGIC_V1: [u8; 8] = *b"DCLTFC01";
/// Finalized request schema label.
pub const FRACTIONAL_FAMILY_REQUEST_SCHEMA_PREIMAGE_V1: &[u8] =
    b"dclutch/schema/fractional-family-request-v1";
/// SHA-256 identity of [`FRACTIONAL_FAMILY_REQUEST_SCHEMA_PREIMAGE_V1`].
pub const FRACTIONAL_FAMILY_REQUEST_SCHEMA_ID_V1: [u8; 32] = [
    0xbc, 0xcc, 0x63, 0x63, 0x34, 0xef, 0x8e, 0xdf, 0xc5, 0xf6, 0xe6, 0xc6, 0xb6, 0x0d, 0xa6, 0xb0,
    0xdf, 0x8b, 0x14, 0x7d, 0xb8, 0x09, 0xbf, 0x41, 0x81, 0x73, 0x5b, 0x68, 0xb4, 0x28, 0xdc, 0x6a,
];
/// Current request wire version.
pub const FRACTIONAL_FAMILY_REQUEST_VERSION_V1: u16 = 1;
/// Canonical absent terminal-outcome sentinel.
pub const NO_TERMINAL_OUTCOME_V1: u32 = u32::MAX;

const ACTION_OFFSET: usize = 10;
const HEADER_RESERVED_OFFSET: usize = 11;
const RELEASE_SET_OFFSET: usize = 16;
const MARKET_OFFSET: usize = 48;
const PRODUCT_RECORD_OFFSET: usize = 80;
const RESULT_DOMAIN_OFFSET: usize = 112;
const TERMS_OFFSET: usize = 144;
const TOKEN_BEHAVIOR_OFFSET: usize = 176;
const OWNER_OFFSET: usize = 208;
const SOURCE_TOKEN_OFFSET: usize = 240;
const DESTINATION_TOKEN_OFFSET: usize = 272;
const TERMINAL_DIGEST_OFFSET: usize = 304;
const EXPECTED_REVISION_OFFSET: usize = 336;
const QUANTITY_OFFSET: usize = 344;
const OUTCOME_OFFSET: usize = 352;
const TERMINAL_OUTCOME_OFFSET: usize = 356;
const TAIL_RESERVED_OFFSET: usize = 360;

/// Action discriminator byte offset in the exact family request.
pub const FRACTIONAL_REQUEST_ACTION_OFFSET_V1: usize = ACTION_OFFSET;
/// First reserved-header byte offset.
pub const FRACTIONAL_REQUEST_HEADER_RESERVED_OFFSET_V1: usize = HEADER_RESERVED_OFFSET;
/// Reserved-header byte width.
pub const FRACTIONAL_REQUEST_HEADER_RESERVED_BYTES_V1: usize = 5;
/// Release-set identity offset.
pub const FRACTIONAL_REQUEST_RELEASE_SET_OFFSET_V1: usize = RELEASE_SET_OFFSET;
/// Logical Market identity offset.
pub const FRACTIONAL_REQUEST_MARKET_OFFSET_V1: usize = MARKET_OFFSET;
/// Finalized Product-record digest offset.
pub const FRACTIONAL_REQUEST_PRODUCT_RECORD_OFFSET_V1: usize = PRODUCT_RECORD_OFFSET;
/// Product-owned ResultDomain digest offset.
pub const FRACTIONAL_REQUEST_RESULT_DOMAIN_OFFSET_V1: usize = RESULT_DOMAIN_OFFSET;
/// Fractional terms digest offset.
pub const FRACTIONAL_REQUEST_TERMS_OFFSET_V1: usize = TERMS_OFFSET;
/// Selected TokenBehaviorV2 digest offset.
pub const FRACTIONAL_REQUEST_TOKEN_BEHAVIOR_OFFSET_V1: usize = TOKEN_BEHAVIOR_OFFSET;
/// Actor owner identity offset.
pub const FRACTIONAL_REQUEST_OWNER_OFFSET_V1: usize = OWNER_OFFSET;
/// Source Token-account identity offset.
pub const FRACTIONAL_REQUEST_SOURCE_TOKEN_OFFSET_V1: usize = SOURCE_TOKEN_OFFSET;
/// Destination Token-account identity offset.
pub const FRACTIONAL_REQUEST_DESTINATION_TOKEN_OFFSET_V1: usize = DESTINATION_TOKEN_OFFSET;
/// Finalized terminal-coordinate digest offset.
pub const FRACTIONAL_REQUEST_TERMINAL_DIGEST_OFFSET_V1: usize = TERMINAL_DIGEST_OFFSET;
/// Expected replay revision offset.
pub const FRACTIONAL_REQUEST_EXPECTED_REVISION_OFFSET_V1: usize = EXPECTED_REVISION_OFFSET;
/// Exact action quantity offset.
pub const FRACTIONAL_REQUEST_QUANTITY_OFFSET_V1: usize = QUANTITY_OFFSET;
/// Selected Product outcome offset.
pub const FRACTIONAL_REQUEST_OUTCOME_OFFSET_V1: usize = OUTCOME_OFFSET;
/// Authenticated terminal outcome offset.
pub const FRACTIONAL_REQUEST_TERMINAL_OUTCOME_OFFSET_V1: usize = TERMINAL_OUTCOME_OFFSET;
/// First reserved-tail byte offset.
pub const FRACTIONAL_REQUEST_TAIL_RESERVED_OFFSET_V1: usize = TAIL_RESERVED_OFFSET;
/// Reserved-tail byte width.
pub const FRACTIONAL_REQUEST_TAIL_RESERVED_BYTES_V1: usize = 24;

/// Exact Fractional physical action.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum FractionalActionV1 {
    /// Lock native categorical claims and mint exact denominator-scaled shards.
    Wrap = 0,
    /// Transfer raw same-Mint shard atoms without changing wrapper state.
    Transfer = 1,
    /// Burn a whole-denominator multiple and return native categorical claims.
    WholeUnwrap = 2,
    /// Burn winning shards and request exact terminal collateral payout.
    WinningRedeem = 3,
    /// Burn losing shards for an explicit zero-valued terminal payout.
    LosingZeroBurn = 4,
    /// Bind the wrapper to one authenticated terminal coordinate.
    Terminalize = 5,
    /// Close only after all Token shard supplies and native reserves are zero.
    ZeroSupplyRetire = 6,
}

impl FractionalActionV1 {
    /// Stable request/action discriminator.
    pub const fn byte(self) -> u8 {
        self as u8
    }

    /// Whether this action requires a positive raw quantity.
    pub const fn carries_quantity(self) -> bool {
        matches!(
            self,
            Self::Wrap
                | Self::Transfer
                | Self::WholeUnwrap
                | Self::WinningRedeem
                | Self::LosingZeroBurn
        )
    }

    /// Whether the action requires an authenticated terminal coordinate.
    pub const fn requires_terminal(self) -> bool {
        matches!(
            self,
            Self::WinningRedeem | Self::LosingZeroBurn | Self::Terminalize | Self::ZeroSupplyRetire
        )
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            0 => Ok(Self::Wrap),
            1 => Ok(Self::Transfer),
            2 => Ok(Self::WholeUnwrap),
            3 => Ok(Self::WinningRedeem),
            4 => Ok(Self::LosingZeroBurn),
            5 => Ok(Self::Terminalize),
            6 => Ok(Self::ZeroSupplyRetire),
            _ => Err(FractionalRequestErrorV1::UnknownAction),
        }
    }
}

/// Checked fields used to construct one canonical request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalFamilyRequestInputV1 {
    /// Selected execution release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized Product graph-root record digest.
    pub product_record: [u8; 32],
    /// Product-owned ResultDomain record digest and ordering.
    pub result_domain: [u8; 32],
    /// Exact finalized fractional terms digest.
    pub terms: [u8; 32],
    /// Selected finalized TokenBehaviorV2 digest.
    pub token_behavior: [u8; 32],
    /// Wallet or protocol owner for actor-bound actions; zero for permissionless actions.
    pub owner: [u8; 32],
    /// Exact source Token account when selected by the action.
    pub source_token_account: [u8; 32],
    /// Exact destination Token account when selected by the action.
    pub destination_token_account: [u8; 32],
    /// Finalized terminal-coordinate digest, or zero while open.
    pub terminal_digest: [u8; 32],
    /// Optimistic wrapper revision derived from chain state.
    pub expected_revision: u64,
    /// Native claims or raw shard atoms, according to the action.
    pub quantity: u64,
    /// Product-owned selected outcome, or [`NO_TERMINAL_OUTCOME_V1`] for retirement.
    pub outcome: u32,
    /// Authenticated terminal outcome, or [`NO_TERMINAL_OUTCOME_V1`] while open.
    pub terminal_outcome: u32,
}

/// Hostile-decoded canonical family request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FractionalFamilyRequestV1 {
    action: FractionalActionV1,
    input: FractionalFamilyRequestInputV1,
}

impl FractionalFamilyRequestV1 {
    /// Construct and validate one request before encoding it.
    pub fn new(action: FractionalActionV1, input: FractionalFamilyRequestInputV1) -> Result<Self> {
        validate(action, input)?;
        Ok(Self { action, input })
    }

    /// Hostile-decode one exact request.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != FRACTIONAL_FAMILY_REQUEST_BYTES_V1 {
            return Err(FractionalRequestErrorV1::InvalidLength);
        }
        if array::<8>(bytes, 0)? != FRACTIONAL_FAMILY_REQUEST_MAGIC_V1
            || read_u16(bytes, 8)? != FRACTIONAL_FAMILY_REQUEST_VERSION_V1
        {
            return Err(FractionalRequestErrorV1::InvalidHeader);
        }
        require_zero(bytes, HEADER_RESERVED_OFFSET, 5)?;
        require_zero(bytes, TAIL_RESERVED_OFFSET, 24)?;
        Self::new(
            FractionalActionV1::decode(byte(bytes, ACTION_OFFSET)?)?,
            FractionalFamilyRequestInputV1 {
                release_set: array(bytes, RELEASE_SET_OFFSET)?,
                market: array(bytes, MARKET_OFFSET)?,
                product_record: array(bytes, PRODUCT_RECORD_OFFSET)?,
                result_domain: array(bytes, RESULT_DOMAIN_OFFSET)?,
                terms: array(bytes, TERMS_OFFSET)?,
                token_behavior: array(bytes, TOKEN_BEHAVIOR_OFFSET)?,
                owner: array(bytes, OWNER_OFFSET)?,
                source_token_account: array(bytes, SOURCE_TOKEN_OFFSET)?,
                destination_token_account: array(bytes, DESTINATION_TOKEN_OFFSET)?,
                terminal_digest: array(bytes, TERMINAL_DIGEST_OFFSET)?,
                expected_revision: read_u64(bytes, EXPECTED_REVISION_OFFSET)?,
                quantity: read_u64(bytes, QUANTITY_OFFSET)?,
                outcome: read_u32(bytes, OUTCOME_OFFSET)?,
                terminal_outcome: read_u32(bytes, TERMINAL_OUTCOME_OFFSET)?,
            },
        )
    }

    /// Encode the canonical request.
    pub fn to_bytes(self) -> [u8; FRACTIONAL_FAMILY_REQUEST_BYTES_V1] {
        let mut output = [0; FRACTIONAL_FAMILY_REQUEST_BYTES_V1];
        put(&mut output, 0, &FRACTIONAL_FAMILY_REQUEST_MAGIC_V1);
        put(
            &mut output,
            8,
            &FRACTIONAL_FAMILY_REQUEST_VERSION_V1.to_le_bytes(),
        );
        output[ACTION_OFFSET] = self.action.byte();
        for (offset, value) in [
            (RELEASE_SET_OFFSET, self.input.release_set),
            (MARKET_OFFSET, self.input.market),
            (PRODUCT_RECORD_OFFSET, self.input.product_record),
            (RESULT_DOMAIN_OFFSET, self.input.result_domain),
            (TERMS_OFFSET, self.input.terms),
            (TOKEN_BEHAVIOR_OFFSET, self.input.token_behavior),
            (OWNER_OFFSET, self.input.owner),
            (SOURCE_TOKEN_OFFSET, self.input.source_token_account),
            (
                DESTINATION_TOKEN_OFFSET,
                self.input.destination_token_account,
            ),
            (TERMINAL_DIGEST_OFFSET, self.input.terminal_digest),
        ] {
            put(&mut output, offset, &value);
        }
        put(
            &mut output,
            EXPECTED_REVISION_OFFSET,
            &self.input.expected_revision.to_le_bytes(),
        );
        put(
            &mut output,
            QUANTITY_OFFSET,
            &self.input.quantity.to_le_bytes(),
        );
        put(
            &mut output,
            OUTCOME_OFFSET,
            &self.input.outcome.to_le_bytes(),
        );
        put(
            &mut output,
            TERMINAL_OUTCOME_OFFSET,
            &self.input.terminal_outcome.to_le_bytes(),
        );
        output
    }

    /// Selected action.
    pub const fn action(self) -> FractionalActionV1 {
        self.action
    }

    /// Exact checked fields.
    pub const fn input(self) -> FractionalFamilyRequestInputV1 {
        self.input
    }
}

/// Stable request refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FractionalRequestErrorV1 {
    /// Request bytes had another exact width.
    InvalidLength,
    /// Magic or version selected another schema.
    InvalidHeader,
    /// Reserved or inactive fields were noncanonical.
    NonCanonical,
    /// Action tag was unknown.
    UnknownAction,
    /// A required content, program, owner, or Token-account identity was zero or aliased.
    InvalidIdentity,
    /// A state-changing quantity was zero or a permissionless action carried quantity.
    InvalidQuantity,
    /// Open and terminal coordinates did not match the selected action.
    InvalidTerminal,
}

/// Result alias.
pub type Result<T> = core::result::Result<T, FractionalRequestErrorV1>;

fn validate(action: FractionalActionV1, input: FractionalFamilyRequestInputV1) -> Result<()> {
    if [
        input.release_set,
        input.market,
        input.product_record,
        input.result_domain,
        input.terms,
        input.token_behavior,
    ]
    .iter()
    .any(is_zero)
    {
        return Err(FractionalRequestErrorV1::InvalidIdentity);
    }
    if action.carries_quantity() != (input.quantity != 0) {
        return Err(FractionalRequestErrorV1::InvalidQuantity);
    }
    let terminal_present = !is_zero(&input.terminal_digest);
    let terminal_outcome_present = input.terminal_outcome != NO_TERMINAL_OUTCOME_V1;
    if terminal_present != terminal_outcome_present
        || (action.requires_terminal() && !terminal_present)
        || matches!(
            action,
            FractionalActionV1::Wrap | FractionalActionV1::WholeUnwrap
        ) && terminal_present
    {
        return Err(FractionalRequestErrorV1::InvalidTerminal);
    }
    match action {
        FractionalActionV1::Wrap => {
            require_owner(input.owner)?;
            require_zero_identity(input.source_token_account)?;
            require_identity(input.destination_token_account)?;
        }
        FractionalActionV1::Transfer => {
            require_owner(input.owner)?;
            require_identity(input.source_token_account)?;
            require_identity(input.destination_token_account)?;
            if input.source_token_account == input.destination_token_account {
                return Err(FractionalRequestErrorV1::InvalidIdentity);
            }
        }
        FractionalActionV1::WholeUnwrap
        | FractionalActionV1::WinningRedeem
        | FractionalActionV1::LosingZeroBurn => {
            require_owner(input.owner)?;
            require_identity(input.source_token_account)?;
            // Exact quotient/remainder change stays in this same Token account.
            require_zero_identity(input.destination_token_account)?;
        }
        FractionalActionV1::Terminalize => {
            require_zero_identity(input.owner)?;
            require_zero_identity(input.source_token_account)?;
            require_zero_identity(input.destination_token_account)?;
            if input.outcome != input.terminal_outcome {
                return Err(FractionalRequestErrorV1::InvalidTerminal);
            }
        }
        FractionalActionV1::ZeroSupplyRetire => {
            require_zero_identity(input.owner)?;
            require_zero_identity(input.source_token_account)?;
            require_zero_identity(input.destination_token_account)?;
            if input.outcome != NO_TERMINAL_OUTCOME_V1 {
                return Err(FractionalRequestErrorV1::NonCanonical);
            }
        }
    }
    if action == FractionalActionV1::WinningRedeem && input.outcome != input.terminal_outcome {
        return Err(FractionalRequestErrorV1::InvalidTerminal);
    }
    if action == FractionalActionV1::LosingZeroBurn && input.outcome == input.terminal_outcome {
        return Err(FractionalRequestErrorV1::InvalidTerminal);
    }
    Ok(())
}

fn require_owner(value: [u8; 32]) -> Result<()> {
    require_identity(value)
}

fn require_identity(value: [u8; 32]) -> Result<()> {
    if is_zero(&value) {
        Err(FractionalRequestErrorV1::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn require_zero_identity(value: [u8; 32]) -> Result<()> {
    if is_zero(&value) {
        Ok(())
    } else {
        Err(FractionalRequestErrorV1::NonCanonical)
    }
}

fn is_zero(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn byte(input: &[u8], offset: usize) -> Result<u8> {
    input
        .get(offset)
        .copied()
        .ok_or(FractionalRequestErrorV1::InvalidLength)
}

fn array<const N: usize>(input: &[u8], offset: usize) -> Result<[u8; N]> {
    input
        .get(offset..offset + N)
        .ok_or(FractionalRequestErrorV1::InvalidLength)?
        .try_into()
        .map_err(|_| FractionalRequestErrorV1::InvalidLength)
}

fn read_u16(input: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(input, offset)?))
}

fn read_u32(input: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_le_bytes(array(input, offset)?))
}

fn read_u64(input: &[u8], offset: usize) -> Result<u64> {
    Ok(u64::from_le_bytes(array(input, offset)?))
}

fn require_zero(input: &[u8], offset: usize, len: usize) -> Result<()> {
    if input
        .get(offset..offset + len)
        .ok_or(FractionalRequestErrorV1::InvalidLength)?
        .iter()
        .any(|byte| *byte != 0)
    {
        return Err(FractionalRequestErrorV1::NonCanonical);
    }
    Ok(())
}

fn put(output: &mut [u8], offset: usize, value: &[u8]) {
    if let Some(target) = output.get_mut(offset..offset + value.len()) {
        target.copy_from_slice(value);
    }
}
