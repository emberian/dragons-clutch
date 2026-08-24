use clutch_source_plane_v3::{
    ContentId, DrawdownSummaryV3, FixedCodec, InstanceDescriptorV3, LiquidityEnvelopeV3,
    OpenRawPageV3, PayoutTableV3, ProductTemplateV3, RawPageV3, SeriesFundingV3, SeriesPlanV3,
    SourceHeadV3, SourcePlaneProgramV3, StatisticKeyV3, StatisticResultV3, SummaryProgramV3,
    WindowClosureReceiptV3, WindowSealV3, WindowSpecV3, WindowWorkV3, WorkEnvelopeV3,
    DRAWDOWN_SUMMARY_BYTES, INSTANCE_DESCRIPTOR_BYTES, LIQUIDITY_ENVELOPE_BYTES,
    OPEN_RAW_PAGE_BYTES, PAYOUT_TABLE_BYTES, PRODUCT_TEMPLATE_BYTES, RAW_PAGE_BYTES,
    SERIES_FUNDING_BYTES, SERIES_PLAN_BYTES, SOURCE_HEAD_BYTES, SOURCE_PLANE_PROGRAM_BYTES,
    STATISTIC_KEY_BYTES, STATISTIC_RESULT_BYTES, SUMMARY_PROGRAM_BYTES,
    WINDOW_CLOSURE_RECEIPT_BYTES, WINDOW_SEAL_BYTES, WINDOW_SPEC_BYTES, WINDOW_WORK_BYTES,
    WORK_ENVELOPE_BYTES,
};
use clutch_terminal_identity_v1::{Id, TerminalIdentityV1, HEADER_BYTES as TERMINAL_HEADER_BYTES};
use sha2::{Digest, Sha256};

use crate::{Error, Result};

const ACCOUNT_MAGIC: [u8; 8] = *b"DCSP3ACT";
const ACCOUNT_DIGEST_DOMAIN: &[u8] = b"dragons-clutch/source-plane-v3/account-state/v1";

/// Exact proposed account-envelope version. Unknown later versions refuse.
pub const ACCOUNT_LAYOUT_VERSION: u16 = 1;
/// Fixed bytes before the sole semantic core body.
pub const ACCOUNT_HEADER_BYTES: usize = 72;

const _: () = assert!(ACCOUNT_HEADER_BYTES == 16 + TERMINAL_HEADER_BYTES);

/// Proposed disjoint account-family registry local to the V3 adapter.
///
/// These values are not live SBF account tags. Promotion must assign global
/// tags explicitly rather than treating this proposal as dispatcher authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum AccountFamilyV3 {
    /// Reviewed SourcePlane release descriptor.
    SourcePlaneProgram = 1,
    /// Mutable source-only head.
    SourceHead = 2,
    /// Mutable one-page ingestion work.
    OpenRawPage = 3,
    /// Immutable reusable raw page.
    RawPage = 4,
    /// Immutable window semantics.
    WindowSpec = 5,
    /// Mutable page-fold cursor.
    WindowWork = 6,
    /// Immutable maturity proof.
    WindowClosureReceipt = 7,
    /// Immutable final window evidence.
    WindowSeal = 8,
    /// Reviewed statistic evaluator release.
    SummaryProgram = 9,
    /// Predictable statistic request.
    StatisticKey = 10,
    /// Immutable result at a predictable request address.
    StatisticResult = 11,
    /// Reusable product Template.
    ProductTemplate = 12,
    /// Exact payout table bound by a Template.
    PayoutTable = 13,
    /// Per-Instance prepaid work quote.
    WorkEnvelope = 14,
    /// Per-Instance funded-liquidity quote.
    LiquidityEnvelope = 15,
    /// Finite immutable recurrence schedule.
    SeriesPlan = 16,
    /// Mutable segregated Series compartments and ordinal cursor.
    SeriesFunding = 17,
    /// Canonical window-bound Instance descriptor.
    InstanceDescriptor = 18,
    /// Resumable exact drawdown fold.
    DrawdownSummary = 19,
}

impl AccountFamilyV3 {
    /// Exact frozen wire discriminant for this account family.
    pub const fn word(self) -> u16 {
        match self {
            Self::SourcePlaneProgram => 1,
            Self::SourceHead => 2,
            Self::OpenRawPage => 3,
            Self::RawPage => 4,
            Self::WindowSpec => 5,
            Self::WindowWork => 6,
            Self::WindowClosureReceipt => 7,
            Self::WindowSeal => 8,
            Self::SummaryProgram => 9,
            Self::StatisticKey => 10,
            Self::StatisticResult => 11,
            Self::ProductTemplate => 12,
            Self::PayoutTable => 13,
            Self::WorkEnvelope => 14,
            Self::LiquidityEnvelope => 15,
            Self::SeriesPlan => 16,
            Self::SeriesFunding => 17,
            Self::InstanceDescriptor => 18,
            Self::DrawdownSummary => 19,
        }
    }

    /// Decode only exact proposal values.
    pub fn decode(value: u16) -> Result<Self> {
        match value {
            1 => Ok(Self::SourcePlaneProgram),
            2 => Ok(Self::SourceHead),
            3 => Ok(Self::OpenRawPage),
            4 => Ok(Self::RawPage),
            5 => Ok(Self::WindowSpec),
            6 => Ok(Self::WindowWork),
            7 => Ok(Self::WindowClosureReceipt),
            8 => Ok(Self::WindowSeal),
            9 => Ok(Self::SummaryProgram),
            10 => Ok(Self::StatisticKey),
            11 => Ok(Self::StatisticResult),
            12 => Ok(Self::ProductTemplate),
            13 => Ok(Self::PayoutTable),
            14 => Ok(Self::WorkEnvelope),
            15 => Ok(Self::LiquidityEnvelope),
            16 => Ok(Self::SeriesPlan),
            17 => Ok(Self::SeriesFunding),
            18 => Ok(Self::InstanceDescriptor),
            19 => Ok(Self::DrawdownSummary),
            _ => Err(Error::InvalidParameter),
        }
    }

    /// Exact core-body bytes owned by this account family.
    pub const fn body_len(self) -> usize {
        match self {
            Self::SourcePlaneProgram => SOURCE_PLANE_PROGRAM_BYTES,
            Self::SourceHead => SOURCE_HEAD_BYTES,
            Self::OpenRawPage => OPEN_RAW_PAGE_BYTES,
            Self::RawPage => RAW_PAGE_BYTES,
            Self::WindowSpec => WINDOW_SPEC_BYTES,
            Self::WindowWork => WINDOW_WORK_BYTES,
            Self::WindowClosureReceipt => WINDOW_CLOSURE_RECEIPT_BYTES,
            Self::WindowSeal => WINDOW_SEAL_BYTES,
            Self::SummaryProgram => SUMMARY_PROGRAM_BYTES,
            Self::StatisticKey => STATISTIC_KEY_BYTES,
            Self::StatisticResult => STATISTIC_RESULT_BYTES,
            Self::ProductTemplate => PRODUCT_TEMPLATE_BYTES,
            Self::PayoutTable => PAYOUT_TABLE_BYTES,
            Self::WorkEnvelope => WORK_ENVELOPE_BYTES,
            Self::LiquidityEnvelope => LIQUIDITY_ENVELOPE_BYTES,
            Self::SeriesPlan => SERIES_PLAN_BYTES,
            Self::SeriesFunding => SERIES_FUNDING_BYTES,
            Self::InstanceDescriptor => INSTANCE_DESCRIPTOR_BYTES,
            Self::DrawdownSummary => DRAWDOWN_SUMMARY_BYTES,
        }
    }
}

/// Associate one semantic-core codec with exactly one adapter account family.
pub trait AccountBodyV3: FixedCodec {
    /// Sole account family allowed to wrap this core body.
    const FAMILY: AccountFamilyV3;
}

macro_rules! account_body {
    ($type:ty, $family:ident) => {
        impl AccountBodyV3 for $type {
            const FAMILY: AccountFamilyV3 = AccountFamilyV3::$family;
        }
    };
}

account_body!(SourcePlaneProgramV3, SourcePlaneProgram);
account_body!(SourceHeadV3, SourceHead);
account_body!(OpenRawPageV3, OpenRawPage);
account_body!(RawPageV3, RawPage);
account_body!(WindowSpecV3, WindowSpec);
account_body!(WindowWorkV3, WindowWork);
account_body!(WindowClosureReceiptV3, WindowClosureReceipt);
account_body!(WindowSealV3, WindowSeal);
account_body!(SummaryProgramV3, SummaryProgram);
account_body!(StatisticKeyV3, StatisticKey);
account_body!(StatisticResultV3, StatisticResult);
account_body!(ProductTemplateV3, ProductTemplate);
account_body!(PayoutTableV3, PayoutTable);
account_body!(WorkEnvelopeV3, WorkEnvelope);
account_body!(LiquidityEnvelopeV3, LiquidityEnvelope);
account_body!(SeriesPlanV3, SeriesPlan);
account_body!(SeriesFundingV3, SeriesFunding);
account_body!(InstanceDescriptorV3, InstanceDescriptor);
account_body!(DrawdownSummaryV3, DrawdownSummary);

/// Fixed adapter-owned prefix. Semantic facts live only in the following core
/// body; this prefix owns family dispatch, the PDA bump, and shared terminal
/// rent/donation identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccountHeaderV3 {
    /// Exact account family.
    pub family: AccountFamilyV3,
    /// Stored PDA bump; excluded from semantic content identities.
    pub bump: u8,
    /// Uniform principal/donation/generation owner.
    pub terminal: TerminalIdentityV1,
}

impl AccountHeaderV3 {
    /// Validate family/body registration and terminal identity.
    pub fn validate<T: AccountBodyV3>(&self, neutral_sink: Id) -> Result<()> {
        if self.family != T::FAMILY || self.family.body_len() != T::ENCODED_LEN {
            return Err(Error::WrongAccountFamily);
        }
        self.terminal.validate(neutral_sink)?;
        Ok(())
    }
}

/// Encode `header || core-body` into one exact fixed account image.
pub fn encode_account<T: AccountBodyV3>(
    header: AccountHeaderV3,
    body: &T,
    neutral_sink: Id,
    output: &mut [u8],
) -> Result<()> {
    header.validate::<T>(neutral_sink)?;
    let expected = ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Error::ArithmeticOverflow)?;
    if output.len() != expected {
        return Err(Error::WrongLength);
    }
    output.fill(0);
    output[..8].copy_from_slice(&ACCOUNT_MAGIC);
    output[8..10].copy_from_slice(&ACCOUNT_LAYOUT_VERSION.to_le_bytes());
    output[10..12].copy_from_slice(&header.family.word().to_le_bytes());
    output[12] = header.bump;
    // 13 is flags and 14..16 are reserved; all remain zero.
    let terminal = header.terminal.encode(neutral_sink)?;
    output[16..ACCOUNT_HEADER_BYTES].copy_from_slice(&terminal);
    body.encode_into(&mut output[ACCOUNT_HEADER_BYTES..])?;
    Ok(())
}

/// Hostile-decode one exact fixed account image and its sole semantic body.
pub fn decode_account<T: AccountBodyV3>(
    input: &[u8],
    neutral_sink: Id,
) -> Result<(AccountHeaderV3, T)> {
    let expected = ACCOUNT_HEADER_BYTES
        .checked_add(T::ENCODED_LEN)
        .ok_or(Error::ArithmeticOverflow)?;
    if input.len() != expected {
        return Err(Error::WrongLength);
    }
    if input[..8] != ACCOUNT_MAGIC {
        return Err(Error::WrongMagic);
    }
    if le_u16(&input[8..10]) != ACCOUNT_LAYOUT_VERSION {
        return Err(Error::BadVersion);
    }
    let family = AccountFamilyV3::decode(le_u16(&input[10..12]))?;
    if input[13..16].iter().any(|byte| *byte != 0) {
        return Err(Error::NonCanonicalPadding);
    }
    let terminal = TerminalIdentityV1::decode(&input[16..ACCOUNT_HEADER_BYTES], neutral_sink)?;
    let header = AccountHeaderV3 {
        family,
        bump: input[12],
        terminal,
    };
    header.validate::<T>(neutral_sink)?;
    let body = T::decode(&input[ACCOUNT_HEADER_BYTES..])?;
    Ok((header, body))
}

/// Digest the exact canonical account image without allocating an intermediate
/// `header || body` buffer. Hostile bytes must first pass [`decode_account`];
/// there is deliberately no raw-byte hashing API that can bless an unknown
/// version, trailing bytes, or a noncanonical body.
pub fn canonical_account_state_digest<const N: usize, T: AccountBodyV3>(
    header: AccountHeaderV3,
    body: &T,
    neutral_sink: Id,
) -> Result<ContentId> {
    header.validate::<T>(neutral_sink)?;
    if N != T::ENCODED_LEN {
        return Err(Error::WrongLength);
    }
    let terminal = header.terminal.encode(neutral_sink)?;
    let mut core = [0; N];
    body.encode_into(&mut core)?;

    let mut hasher = Sha256::new();
    hasher.update(ACCOUNT_DIGEST_DOMAIN);
    hasher.update(ACCOUNT_MAGIC);
    hasher.update(ACCOUNT_LAYOUT_VERSION.to_le_bytes());
    hasher.update(header.family.word().to_le_bytes());
    hasher.update([header.bump, 0, 0, 0]);
    hasher.update(terminal);
    hasher.update(core);
    Ok(ContentId::from_bytes(hasher.finalize().into()))
}

fn le_u16(input: &[u8]) -> u16 {
    let mut word = [0; 2];
    word.copy_from_slice(input);
    u16::from_le_bytes(word)
}
