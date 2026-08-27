//! Exact Structured V2 family request.
//!
//! The request carries one action, the identities that must join, one receipt
//! quantity, and the optimistic replay revision.  It deliberately carries no
//! coefficient, no payout, no supply, and no custody value: every one of those
//! already has a semantic owner that the executor reads independently.

use core::convert::TryInto;

use dclutch_structured_v2_kernel::{
    STRUCTURED_ACTION_ISSUE_V2, STRUCTURED_ACTION_TERMINAL_REDEEM_V2, STRUCTURED_ACTION_UNWRAP_V2,
    STRUCTURED_ACTION_ZERO_SUPPLY_RETIRE_V2, STRUCTURED_REQUEST_ACTION_OFFSET_V2,
    STRUCTURED_REQUEST_BYTES_V2, STRUCTURED_REQUEST_EXPECTED_REVISION_OFFSET_V2,
    STRUCTURED_REQUEST_MAGIC_OFFSET_V2, STRUCTURED_REQUEST_MAGIC_V2,
    STRUCTURED_REQUEST_MARKET_OFFSET_V2, STRUCTURED_REQUEST_OWNER_OFFSET_V2,
    STRUCTURED_REQUEST_PRODUCT_RECORD_OFFSET_V2, STRUCTURED_REQUEST_QUANTITY_OFFSET_V2,
    STRUCTURED_REQUEST_RECEIPT_DESTINATION_OFFSET_V2, STRUCTURED_REQUEST_RECEIPT_SOURCE_OFFSET_V2,
    STRUCTURED_REQUEST_RELEASE_SET_OFFSET_V2, STRUCTURED_REQUEST_RESERVED_HEADER_OFFSET_V2,
    STRUCTURED_REQUEST_RESERVED_TAIL_OFFSET_V2, STRUCTURED_REQUEST_RESULT_DOMAIN_OFFSET_V2,
    STRUCTURED_REQUEST_SHARD_EXPOSURE_OFFSET_V2, STRUCTURED_REQUEST_SHARD_TERMS_OFFSET_V2,
    STRUCTURED_REQUEST_TERMINAL_DIGEST_OFFSET_V2, STRUCTURED_REQUEST_TERMS_OFFSET_V2,
    STRUCTURED_REQUEST_TOKEN_BEHAVIOR_OFFSET_V2, STRUCTURED_REQUEST_VERSION_OFFSET_V2,
    STRUCTURED_SCHEMA_VERSION_V2, StructuredTermsV2,
};

const RESERVED_HEADER_BYTES: usize = 5;
const RESERVED_TAIL_BYTES: usize = 16;

/// Exactly the four Structured V2 actions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum StructuredActionV2 {
    /// Lock the exact shard basket and mint receipt atoms.
    Issue = 0,
    /// Burn receipt atoms and release the exact shard basket while open.
    Unwrap = 1,
    /// Burn receipt atoms after terminal resolution and settle exactly.
    TerminalRedeem = 2,
    /// Close a zero-supply, zero-custody node and recover rent.
    ZeroSupplyRetire = 3,
}

impl StructuredActionV2 {
    /// Stable wire discriminator.
    pub const fn byte(self) -> u8 {
        self as u8
    }

    /// Whether this action carries a positive receipt quantity.
    pub const fn carries_quantity(self) -> bool {
        matches!(self, Self::Issue | Self::Unwrap | Self::TerminalRedeem)
    }

    /// Whether this action requires authenticated terminal Product evidence.
    pub const fn requires_terminal(self) -> bool {
        matches!(self, Self::TerminalRedeem | Self::ZeroSupplyRetire)
    }

    /// Whether this action burns receipt atoms from a source Token account.
    pub const fn burns_receipts(self) -> bool {
        matches!(self, Self::Unwrap | Self::TerminalRedeem)
    }

    /// Whether this action mints receipt atoms into a destination Token account.
    pub const fn mints_receipts(self) -> bool {
        matches!(self, Self::Issue)
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            STRUCTURED_ACTION_ISSUE_V2 => Ok(Self::Issue),
            STRUCTURED_ACTION_UNWRAP_V2 => Ok(Self::Unwrap),
            STRUCTURED_ACTION_TERMINAL_REDEEM_V2 => Ok(Self::TerminalRedeem),
            STRUCTURED_ACTION_ZERO_SUPPLY_RETIRE_V2 => Ok(Self::ZeroSupplyRetire),
            _ => Err(StructuredRequestErrorV2::UnknownAction),
        }
    }
}

/// Checked fields for one Structured V2 request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRequestInputV2 {
    /// Immutable selected release set.
    pub release_set: [u8; 32],
    /// Logical Core Market.
    pub market: [u8; 32],
    /// Finalized Product root digest.
    pub product_record: [u8; 32],
    /// Product-owned result-domain identity and ordering.
    pub result_domain: [u8; 32],
    /// Finalized Structured V2 terms identity.
    pub terms: [u8; 32],
    /// Finalized receipt Token behavior selection identity.
    pub token_behavior: [u8; 32],
    /// Finalized exact claim-shard terms identity.
    pub shard_terms: [u8; 32],
    /// Finalized Product-N to Claims-K exposure identity.
    pub shard_exposure: [u8; 32],
    /// Wallet or protocol owner for actor-bound actions; zero otherwise.
    pub owner: [u8; 32],
    /// Exact receipt source Token account when active; zero otherwise.
    pub receipt_source: [u8; 32],
    /// Exact receipt destination Token account when active; zero otherwise.
    pub receipt_destination: [u8; 32],
    /// Finalized terminal-coordinate digest; zero for open actions.
    pub terminal_digest: [u8; 32],
    /// Optimistic Structured-root revision observed from chain.
    pub expected_revision: u64,
    /// Exact receipt atoms; zero exactly for retirement.
    pub quantity: u64,
}

/// Hostile-decoded Structured V2 request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StructuredRequestV2 {
    action: StructuredActionV2,
    input: StructuredRequestInputV2,
}

impl StructuredRequestV2 {
    /// Construct and validate one canonical request.
    pub fn new(action: StructuredActionV2, input: StructuredRequestInputV2) -> Result<Self> {
        validate_shape(action, input)?;
        Ok(Self { action, input })
    }

    /// Hostile-decode exact request bytes.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != STRUCTURED_REQUEST_BYTES_V2 {
            return Err(StructuredRequestErrorV2::InvalidLength);
        }
        if array::<8>(bytes, STRUCTURED_REQUEST_MAGIC_OFFSET_V2)? != STRUCTURED_REQUEST_MAGIC_V2
            || u16_at(bytes, STRUCTURED_REQUEST_VERSION_OFFSET_V2)? != STRUCTURED_SCHEMA_VERSION_V2
        {
            return Err(StructuredRequestErrorV2::InvalidHeader);
        }
        require_zero(
            bytes,
            STRUCTURED_REQUEST_RESERVED_HEADER_OFFSET_V2,
            RESERVED_HEADER_BYTES,
        )?;
        require_zero(
            bytes,
            STRUCTURED_REQUEST_RESERVED_TAIL_OFFSET_V2,
            RESERVED_TAIL_BYTES,
        )?;
        Self::new(
            StructuredActionV2::decode(byte(bytes, STRUCTURED_REQUEST_ACTION_OFFSET_V2)?)?,
            StructuredRequestInputV2 {
                release_set: array(bytes, STRUCTURED_REQUEST_RELEASE_SET_OFFSET_V2)?,
                market: array(bytes, STRUCTURED_REQUEST_MARKET_OFFSET_V2)?,
                product_record: array(bytes, STRUCTURED_REQUEST_PRODUCT_RECORD_OFFSET_V2)?,
                result_domain: array(bytes, STRUCTURED_REQUEST_RESULT_DOMAIN_OFFSET_V2)?,
                terms: array(bytes, STRUCTURED_REQUEST_TERMS_OFFSET_V2)?,
                token_behavior: array(bytes, STRUCTURED_REQUEST_TOKEN_BEHAVIOR_OFFSET_V2)?,
                shard_terms: array(bytes, STRUCTURED_REQUEST_SHARD_TERMS_OFFSET_V2)?,
                shard_exposure: array(bytes, STRUCTURED_REQUEST_SHARD_EXPOSURE_OFFSET_V2)?,
                owner: array(bytes, STRUCTURED_REQUEST_OWNER_OFFSET_V2)?,
                receipt_source: array(bytes, STRUCTURED_REQUEST_RECEIPT_SOURCE_OFFSET_V2)?,
                receipt_destination: array(
                    bytes,
                    STRUCTURED_REQUEST_RECEIPT_DESTINATION_OFFSET_V2,
                )?,
                terminal_digest: array(bytes, STRUCTURED_REQUEST_TERMINAL_DIGEST_OFFSET_V2)?,
                expected_revision: u64_at(bytes, STRUCTURED_REQUEST_EXPECTED_REVISION_OFFSET_V2)?,
                quantity: u64_at(bytes, STRUCTURED_REQUEST_QUANTITY_OFFSET_V2)?,
            },
        )
    }

    /// Encode exact canonical bytes.
    pub fn to_bytes(self) -> Result<[u8; STRUCTURED_REQUEST_BYTES_V2]> {
        let mut output = [0_u8; STRUCTURED_REQUEST_BYTES_V2];
        put(
            &mut output,
            STRUCTURED_REQUEST_MAGIC_OFFSET_V2,
            &STRUCTURED_REQUEST_MAGIC_V2,
        )?;
        put(
            &mut output,
            STRUCTURED_REQUEST_VERSION_OFFSET_V2,
            &STRUCTURED_SCHEMA_VERSION_V2.to_le_bytes(),
        )?;
        put(
            &mut output,
            STRUCTURED_REQUEST_ACTION_OFFSET_V2,
            &[self.action.byte()],
        )?;
        for (offset, value) in [
            (
                STRUCTURED_REQUEST_RELEASE_SET_OFFSET_V2,
                self.input.release_set,
            ),
            (STRUCTURED_REQUEST_MARKET_OFFSET_V2, self.input.market),
            (
                STRUCTURED_REQUEST_PRODUCT_RECORD_OFFSET_V2,
                self.input.product_record,
            ),
            (
                STRUCTURED_REQUEST_RESULT_DOMAIN_OFFSET_V2,
                self.input.result_domain,
            ),
            (STRUCTURED_REQUEST_TERMS_OFFSET_V2, self.input.terms),
            (
                STRUCTURED_REQUEST_TOKEN_BEHAVIOR_OFFSET_V2,
                self.input.token_behavior,
            ),
            (
                STRUCTURED_REQUEST_SHARD_TERMS_OFFSET_V2,
                self.input.shard_terms,
            ),
            (
                STRUCTURED_REQUEST_SHARD_EXPOSURE_OFFSET_V2,
                self.input.shard_exposure,
            ),
            (STRUCTURED_REQUEST_OWNER_OFFSET_V2, self.input.owner),
            (
                STRUCTURED_REQUEST_RECEIPT_SOURCE_OFFSET_V2,
                self.input.receipt_source,
            ),
            (
                STRUCTURED_REQUEST_RECEIPT_DESTINATION_OFFSET_V2,
                self.input.receipt_destination,
            ),
            (
                STRUCTURED_REQUEST_TERMINAL_DIGEST_OFFSET_V2,
                self.input.terminal_digest,
            ),
        ] {
            put(&mut output, offset, &value)?;
        }
        put(
            &mut output,
            STRUCTURED_REQUEST_EXPECTED_REVISION_OFFSET_V2,
            &self.input.expected_revision.to_le_bytes(),
        )?;
        put(
            &mut output,
            STRUCTURED_REQUEST_QUANTITY_OFFSET_V2,
            &self.input.quantity.to_le_bytes(),
        )?;
        Ok(output)
    }

    /// Selected action.
    pub const fn action(self) -> StructuredActionV2 {
        self.action
    }

    /// Exact checked fields.
    pub const fn input(self) -> StructuredRequestInputV2 {
        self.input
    }

    /// Bind this request to authenticated immutable Structured V2 terms.
    pub fn bind_terms(self, terms: StructuredTermsV2<'_>) -> Result<Self> {
        let input = self.input;
        if input.release_set != terms.release_set()
            || input.market != terms.market()
            || input.product_record != terms.product_record()
            || input.result_domain != terms.result_domain()
            || input.terms != terms.terms_id()
            || input.token_behavior != terms.token_behavior()
            || input.shard_terms != terms.shard_terms()
            || input.shard_exposure != terms.shard_exposure()
        {
            return Err(StructuredRequestErrorV2::TermsMismatch);
        }
        Ok(self)
    }
}

/// Stable Structured V2 request refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructuredRequestErrorV2 {
    /// Bytes had another exact width.
    InvalidLength,
    /// Magic or version selected another schema.
    InvalidHeader,
    /// Reserved or inactive fields were noncanonical.
    NonCanonical,
    /// Action tag was unknown.
    UnknownAction,
    /// A required identity was zero or two Token accounts aliased.
    InvalidIdentity,
    /// Quantity presence differed from the selected action.
    InvalidQuantity,
    /// Open and terminal fields differed from the selected action.
    InvalidTerminal,
    /// Request identities differed from authenticated finalized terms.
    TermsMismatch,
}

/// Result alias for hostile decoding and admission.
pub type Result<T> = core::result::Result<T, StructuredRequestErrorV2>;

fn validate_shape(action: StructuredActionV2, input: StructuredRequestInputV2) -> Result<()> {
    if [
        input.release_set,
        input.market,
        input.product_record,
        input.result_domain,
        input.terms,
        input.token_behavior,
        input.shard_terms,
        input.shard_exposure,
    ]
    .iter()
    .any(is_zero)
    {
        return Err(StructuredRequestErrorV2::InvalidIdentity);
    }
    if action.carries_quantity() != (input.quantity != 0) {
        return Err(StructuredRequestErrorV2::InvalidQuantity);
    }
    if action.requires_terminal() != !is_zero(&input.terminal_digest) {
        return Err(StructuredRequestErrorV2::InvalidTerminal);
    }
    if action.carries_quantity() == is_zero(&input.owner) {
        return Err(StructuredRequestErrorV2::InvalidIdentity);
    }
    if action.burns_receipts() == is_zero(&input.receipt_source)
        || action.mints_receipts() == is_zero(&input.receipt_destination)
    {
        return Err(StructuredRequestErrorV2::InvalidIdentity);
    }
    if action.burns_receipts()
        && action.mints_receipts()
        && input.receipt_source == input.receipt_destination
    {
        return Err(StructuredRequestErrorV2::InvalidIdentity);
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
                    .ok_or(StructuredRequestErrorV2::InvalidLength)?,
        )
        .ok_or(StructuredRequestErrorV2::InvalidLength)?
        .try_into()
        .map_err(|_| StructuredRequestErrorV2::InvalidLength)
}

fn byte(bytes: &[u8], offset: usize) -> Result<u8> {
    bytes
        .get(offset)
        .copied()
        .ok_or(StructuredRequestErrorV2::InvalidLength)
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_le_bytes(array(bytes, offset)?))
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
                    .ok_or(StructuredRequestErrorV2::InvalidLength)?,
        )
        .ok_or(StructuredRequestErrorV2::InvalidLength)?
        .iter()
        .any(|value| *value != 0)
    {
        Err(StructuredRequestErrorV2::NonCanonical)
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
                    .ok_or(StructuredRequestErrorV2::InvalidLength)?,
        )
        .ok_or(StructuredRequestErrorV2::InvalidLength)?
        .copy_from_slice(value);
    Ok(())
}
