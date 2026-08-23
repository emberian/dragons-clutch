use clutch_source_plane_v3::{
    FixedCodec, OpenRawPageV3, RawPageV3, SourceHeadV3, StatisticResultV3, WindowSealV3,
    WindowWorkV3,
};
use clutch_source_plane_v3_adapter::AccountFamilyV3;

use crate::auth::RuntimeKey;
use crate::{Error, Result};

const RUNTIME_ACCOUNT_MAGIC: [u8; 8] = *b"DCSRTA01";

/// Exact SourcePlane runtime account-envelope version.
pub const RUNTIME_ACCOUNT_LAYOUT_VERSION: u16 = 1;
/// Exact bytes before the semantic core body.
pub const RUNTIME_ACCOUNT_HEADER_BYTES: usize = 72;

/// Runtime account header with prefund-safe optional principal ownership.
///
/// `principal_recipient` is zero exactly when `payer_principal_lamports` is
/// zero. This is the intentional difference from the older proposed terminal
/// header: a fully prefunded PDA is creatable but does not invent refund
/// authority for the transaction submitter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAccountHeaderV1 {
    /// Exact semantic account family.
    pub family: AccountFamilyV3,
    /// Stored canonical PDA bump.
    pub bump: u8,
    /// Optional exact rent-principal recipient.
    pub principal_recipient: RuntimeKey,
    /// Exact payer-funded rent shortfall.
    pub payer_principal_lamports: u64,
    /// Monotone lower bound of sink-owned unsolicited lamports.
    pub donation_floor_lamports: u64,
    /// Durable close/reopen generation.
    pub generation: u64,
}

impl RuntimeAccountHeaderV1 {
    /// Validate optional principal and frozen neutral-sink separation.
    pub fn validate(&self, neutral_sink: RuntimeKey) -> Result<()> {
        neutral_sink.validate()?;
        if self.generation == 0
            || (self.payer_principal_lamports == 0) != self.principal_recipient.is_zero()
            || (!self.principal_recipient.is_zero() && self.principal_recipient == neutral_sink)
        {
            return Err(Error::FundingMismatch);
        }
        self.payer_principal_lamports
            .checked_add(self.donation_floor_lamports)
            .ok_or(Error::ArithmeticOverflow)?;
        Ok(())
    }
}

/// Associate one exact core codec with its SourcePlane runtime family.
pub trait RuntimeAccountBodyV1: FixedCodec {
    /// Sole family permitted to wrap this semantic body.
    const FAMILY: AccountFamilyV3;
}

macro_rules! runtime_body {
    ($type:ty, $family:ident) => {
        impl RuntimeAccountBodyV1 for $type {
            const FAMILY: AccountFamilyV3 = AccountFamilyV3::$family;
        }
    };
}

runtime_body!(SourceHeadV3, SourceHead);
runtime_body!(OpenRawPageV3, OpenRawPage);
runtime_body!(RawPageV3, RawPage);
runtime_body!(WindowWorkV3, WindowWork);
runtime_body!(WindowSealV3, WindowSeal);
runtime_body!(StatisticResultV3, StatisticResult);

/// Encode one exact runtime header and its sole semantic body.
pub fn encode_runtime_account<T: RuntimeAccountBodyV1>(
    header: RuntimeAccountHeaderV1,
    body: &T,
    neutral_sink: RuntimeKey,
    output: &mut [u8],
) -> Result<()> {
    header.validate(neutral_sink)?;
    if header.family != T::FAMILY || header.family.body_len() != T::ENCODED_LEN {
        return Err(Error::InvalidCodec);
    }
    let expected = RUNTIME_ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Error::ArithmeticOverflow)?;
    if output.len() != expected {
        return Err(Error::InvalidCodec);
    }
    output.fill(0);
    output[..8].copy_from_slice(&RUNTIME_ACCOUNT_MAGIC);
    output[8..10].copy_from_slice(&RUNTIME_ACCOUNT_LAYOUT_VERSION.to_le_bytes());
    output[10..12].copy_from_slice(&(header.family as u16).to_le_bytes());
    output[12] = header.bump;
    output[16..48].copy_from_slice(&header.principal_recipient.bytes());
    output[48..56].copy_from_slice(&header.payer_principal_lamports.to_le_bytes());
    output[56..64].copy_from_slice(&header.donation_floor_lamports.to_le_bytes());
    output[64..72].copy_from_slice(&header.generation.to_le_bytes());
    body.encode_into(&mut output[RUNTIME_ACCOUNT_HEADER_BYTES..])?;
    Ok(())
}

/// Hostile-decode one exact runtime account envelope and semantic body.
pub fn decode_runtime_account<T: RuntimeAccountBodyV1>(
    input: &[u8],
    neutral_sink: RuntimeKey,
) -> Result<(RuntimeAccountHeaderV1, T)> {
    let expected = RUNTIME_ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Error::ArithmeticOverflow)?;
    if input.len() != expected
        || input[..8] != RUNTIME_ACCOUNT_MAGIC
        || le_u16(&input[8..10]) != RUNTIME_ACCOUNT_LAYOUT_VERSION
        || input[13..16].iter().any(|byte| *byte != 0)
    {
        return Err(Error::InvalidCodec);
    }
    let family = AccountFamilyV3::decode(le_u16(&input[10..12]))?;
    if family != T::FAMILY || family.body_len() != T::ENCODED_LEN {
        return Err(Error::InvalidCodec);
    }
    let header = RuntimeAccountHeaderV1 {
        family,
        bump: input[12],
        principal_recipient: key_at(input, 16),
        payer_principal_lamports: le_u64(&input[48..56]),
        donation_floor_lamports: le_u64(&input[56..64]),
        generation: le_u64(&input[64..72]),
    };
    header.validate(neutral_sink)?;
    let body = T::decode(&input[RUNTIME_ACCOUNT_HEADER_BYTES..])?;
    Ok((header, body))
}

/// Observe one exact post-transition balance and monotonically update donations.
pub fn observe_runtime_account_header(
    header: RuntimeAccountHeaderV1,
    neutral_sink: RuntimeKey,
    actual_balance_lamports: u64,
    accounted_balance_lamports: u64,
) -> Result<RuntimeAccountHeaderV1> {
    header.validate(neutral_sink)?;
    if accounted_balance_lamports < header.payer_principal_lamports
        || actual_balance_lamports < accounted_balance_lamports
    {
        return Err(Error::FundingMismatch);
    }
    let observed_donation = actual_balance_lamports
        .checked_sub(accounted_balance_lamports)
        .ok_or(Error::ArithmeticOverflow)?;
    let next = RuntimeAccountHeaderV1 {
        donation_floor_lamports: header.donation_floor_lamports.max(observed_donation),
        ..header
    };
    next.validate(neutral_sink)?;
    Ok(next)
}

fn key_at(input: &[u8], at: usize) -> RuntimeKey {
    let mut bytes = [0; 32];
    bytes.copy_from_slice(&input[at..at + 32]);
    RuntimeKey::from_bytes(bytes)
}

fn le_u16(input: &[u8]) -> u16 {
    let mut bytes = [0; 2];
    bytes.copy_from_slice(input);
    u16::from_le_bytes(bytes)
}

fn le_u64(input: &[u8]) -> u64 {
    let mut bytes = [0; 8];
    bytes.copy_from_slice(input);
    u64::from_le_bytes(bytes)
}
