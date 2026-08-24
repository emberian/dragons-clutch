//! Frozen non-production wire and account layouts for recurring Product/Series.
//!
//! This module owns bytes only. It does not authenticate a registry release,
//! collateral profile, source runtime, failure admission, Clock, account owner,
//! PDA, rent, or token balance. Those are SBF adapter obligations. In
//! particular, decoding [`RegisterSeriesIntentV1`] cannot turn its registry
//! fields into authority, and decoding either funding wrapper cannot prove
//! that its physical custody compartments hold the balances in its pure state.
//!
//! The six Series local action tags `13..=18` are allocated by
//! [`crate::registry`]. SourcePlane V3 exclusively owns this shared family's
//! tags `1..=12`; no former Series coordinate remains as an alias. Every Series
//! action remains runtime-disabled until its complete account/receipt join is
//! wired.

use clutch_product_series::{
    CompiledProductSeriesBundleV5Id, ContentId, DirectGlobalLivenessV1, FixedCodec,
    MarketInstanceV2Id, MarketLifecycleReplayReceiptV1, MarketLifecycleRootV1,
    SeriesFundingComponentV1, SeriesFundingStateV1, SeriesFundingStateV2,
    SeriesFundingTermsV2Id, SeriesLifecycleReplayV1, SeriesMarketLinkV1, SeriesPlanV5Id,
    SourceOccurrenceV1Id, DIRECT_GLOBAL_LIVENESS_BYTES_V1,
    MARKET_LIFECYCLE_REPLAY_RECEIPT_BYTES_V1, MARKET_LIFECYCLE_ROOT_BYTES_V1,
    SERIES_COLLATERAL_VAULT_COUNT_V2, SERIES_FUNDING_COMPONENT_COUNT,
    SERIES_FUNDING_STATE_BYTES, SERIES_FUNDING_STATE_BYTES_V2,
    SERIES_LIFECYCLE_REPLAY_BYTES_V1, SERIES_MARKET_LINK_BYTES_V1,
};

use crate::{digest, is_zero, registry, CodecError, Hash32, Result, HASH_BYTES};

const SERIES_MARKET_LINK_AUTHENTICATION_DOMAIN_V1: &[u8] =
    b"dragons-clutch/series-market-link-account-authentication/v1";

/// Canonical SeriesRegistry PDA prefix shared by SBF and untrusted indexers.
/// V2 intentionally retains the address to make V1 presence a replay refusal,
/// never an alias or an in-place schema rewrite.
pub const SERIES_REGISTRY_PDA_PREFIX_V1: &[u8] = b"dc:series-registry:v1";
/// Canonical SeriesFunding PDA prefix. FundingV2 retains this address so a
/// historical V1 account permanently prevents successor recreation.
pub const SERIES_FUNDING_PDA_PREFIX_V1: &[u8] = b"dc:series-funding:v1";

/// Exact immutable registered-Series account width.
pub const SERIES_REGISTRY_ACCOUNT_BYTES_V1: usize = 168;
/// Exact current registered-Series account width. There is no reserved tail:
/// every byte belongs to one named, authenticated fact.
pub const SERIES_REGISTRY_ACCOUNT_BYTES_V2: usize = 172;
/// Exact mutable Series-funding account width.
pub const SERIES_FUNDING_ACCOUNT_BYTES_V1: usize =
    4 + 8 + (8 * SERIES_FUNDING_COMPONENT_COUNT) + SERIES_FUNDING_STATE_BYTES;
/// Exact current Series funding wrapper width. SeriesAdmission is lamport-only,
/// so the five release-selected collateral-vault rent principals remain
/// separate from the six-component semantic state.
pub const SERIES_FUNDING_ACCOUNT_BYTES_V2: usize =
    4 + 8 + (8 * SERIES_COLLATERAL_VAULT_COUNT_V2) + SERIES_FUNDING_STATE_BYTES_V2;
/// Exact common header before one Product market/link semantic body.
pub const PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1: usize = 16;
/// Exact framed shared MarketLifecycleRoot account width.
pub const MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1: usize =
    PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1 + MARKET_LIFECYCLE_ROOT_BYTES_V1;
/// Exact permanent Product Market-lifecycle replay account width.
pub const MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V1: usize =
    PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1 + MARKET_LIFECYCLE_REPLAY_RECEIPT_BYTES_V1;
/// Exact framed per-Series SeriesMarketLink account width.
pub const SERIES_MARKET_LINK_ACCOUNT_BYTES_V1: usize =
    PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1 + SERIES_MARKET_LINK_BYTES_V1;
/// Canonical Product-owned Direct global-liveness PDA prefix.
pub const PRODUCT_DIRECT_GLOBAL_LIVENESS_PDA_PREFIX_V1: &[u8] =
    b"dc:product-direct-live:v1";
/// Exact framed Product Direct global-liveness account width.
pub const PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1: usize =
    PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1 + DIRECT_GLOBAL_LIVENESS_BYTES_V1;

const _: () = assert!(PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1 == 1_024);

/// Recompute the sole shared authentication identity for one exact Product
/// SeriesMarketLink account. Product and standalone attachment adapters must
/// call this function rather than duplicating its preimage assembly.
pub fn series_market_link_authentication_id_v1(
    account: [u8; HASH_BYTES],
    owner_program: [u8; HASH_BYTES],
    framed_data_id: [u8; HASH_BYTES],
    semantic_id: [u8; HASH_BYTES],
    market_root: [u8; HASH_BYTES],
    observed_lamports: u64,
) -> Hash32 {
    digest(
        SERIES_MARKET_LINK_AUTHENTICATION_DOMAIN_V1,
        &[
            &account,
            &owner_program,
            &framed_data_id,
            &semantic_id,
            &market_root,
            &observed_lamports.to_le_bytes(),
        ],
    )
}

/// Exact `RegisterSeries` payload width.
pub const REGISTER_SERIES_PAYLOAD_BYTES_V1: usize = 4 * HASH_BYTES;
/// Exact `ActivateFunding` payload width.
pub const ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1: usize = HASH_BYTES;
/// Exact `AdvanceOccurrence` payload width.
pub const ADVANCE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1: usize =
    HASH_BYTES + 4 + 4 + HASH_BYTES + HASH_BYTES;
/// Exact `LapseOccurrence` payload width.
pub const LAPSE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1: usize = HASH_BYTES + 4 + 4;
/// Exact `ObserveDonation` payload width.
pub const OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1: usize = HASH_BYTES + 1 + 1 + 6;
/// Exact `CloseFunding` payload width.
pub const CLOSE_SERIES_FUNDING_PAYLOAD_BYTES_V1: usize = HASH_BYTES;

/// Exact current physical FundingV2 activation account count.
pub const ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2: usize = 43;
/// First of six ordered System-owned lamport vaults.
pub const ACTIVATE_SERIES_LAMPORT_VAULT_START_V2: usize = 3;
/// Exclusive end of the six ordered System-owned lamport vaults.
pub const ACTIVATE_SERIES_LAMPORT_VAULT_END_V2: usize = 9;
/// First of five ordered Realm-selected collateral vaults.
pub const ACTIVATE_SERIES_COLLATERAL_VAULT_START_V2: usize = 21;
/// Exclusive end of the five ordered Realm-selected collateral vaults.
pub const ACTIVATE_SERIES_COLLATERAL_VAULT_END_V2: usize = 26;
/// First of nine ordered immutable Series artifacts.
pub const ACTIVATE_SERIES_ARTIFACT_START_V2: usize = 34;
/// Exclusive end of the nine ordered immutable Series artifacts.
pub const ACTIVATE_SERIES_ARTIFACT_END_V2: usize = 43;

/// Semantic role of one ordered current FundingV2 activation account.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActivateSeriesFundingAccountRoleV2 {
    /// Writable persistent Series registry/replay owner.
    Registry,
    /// Writable absent FundingV2 PDA.
    Funding,
    /// FundingTerms-bound lamport payer and refund identity.
    Payer,
    /// MarketCore lamport custody PDA.
    LamportVaultMarketCore,
    /// SeriesAdmission lamport custody PDA.
    LamportVaultSeriesAdmission,
    /// RecoveryReserve lamport custody PDA.
    LamportVaultRecoveryReserve,
    /// SourceWork lamport custody PDA.
    LamportVaultSourceWork,
    /// LiquidityFacility lamport custody PDA.
    LamportVaultLiquidityFacility,
    /// WrapperSet lamport custody PDA.
    LamportVaultWrapperSet,
    /// Holder collateral source account.
    PayerCollateralSource,
    /// Signer owning the holder collateral source.
    PayerTokenAuthority,
    /// FundingTerms-bound receive-only collateral principal refund account.
    CollateralPrincipalRefund,
    /// FundingTerms-bound receive-only neutral collateral disposition account.
    NeutralCollateralDisposition,
    /// FundingTerms-bound System-owned neutral lamport sink.
    NeutralLamportSink,
    /// Canonical Series collateral-vault authority PDA.
    CollateralAuthority,
    /// Immutable Realm account.
    Realm,
    /// Immutable ProfileV2 account.
    Profile,
    /// Exact collateral-policy artifact.
    CollateralPolicy,
    /// Realm-selected collateral mint.
    CollateralMint,
    /// Realm-selected collateral token program.
    CollateralTokenProgram,
    /// Exact linked collateral ProgramData account.
    CollateralTokenProgramData,
    /// MarketCore collateral vault.
    CollateralVaultMarketCore,
    /// RecoveryReserve collateral vault.
    CollateralVaultRecoveryReserve,
    /// SourceWork collateral vault.
    CollateralVaultSourceWork,
    /// LiquidityFacility collateral vault.
    CollateralVaultLiquidityFacility,
    /// WrapperSet collateral vault.
    CollateralVaultWrapperSet,
    /// System Program.
    SystemProgram,
    /// Rent sysvar.
    RentSysvar,
    /// Executing Clutch program account.
    ExecutingProgram,
    /// Linked Clutch ProgramData account.
    ExecutingProgramData,
    /// Exact RegistryRelease artifact.
    RegistryRelease,
    /// Exact CapabilityProfile artifact.
    CapabilityProfile,
    /// Exact Source release account.
    SourceRelease,
    /// Exact compiled BundleV5 artifact.
    CompilerBundle,
    /// SeriesPlanV5 artifact.
    SeriesPlan,
    /// SeriesFundingTermsV2 artifact.
    FundingTerms,
    /// ProductTemplateV4 artifact.
    ProductTemplate,
    /// NativeClaimBasisV1 artifact.
    NativeClaimBasis,
    /// EvidenceOnlyRecoveryPolicyV1 artifact.
    RecoveryPolicy,
    /// PriceMeasurePolicyV1 artifact.
    PricePolicy,
    /// MarketGenesisProfileV2 artifact.
    MarketGenesis,
    /// SeriesFundingQuoteV4 artifact.
    FundingQuote,
    /// SeriesAttachmentPlanV4 artifact.
    AttachmentPlan,
}

impl ActivateSeriesFundingAccountRoleV2 {
    /// Exact index in the current FundingV2 activation account list.
    pub const fn index(self) -> usize {
        match self {
            Self::Registry => 0,
            Self::Funding => 1,
            Self::Payer => 2,
            Self::LamportVaultMarketCore => 3,
            Self::LamportVaultSeriesAdmission => 4,
            Self::LamportVaultRecoveryReserve => 5,
            Self::LamportVaultSourceWork => 6,
            Self::LamportVaultLiquidityFacility => 7,
            Self::LamportVaultWrapperSet => 8,
            Self::PayerCollateralSource => 9,
            Self::PayerTokenAuthority => 10,
            Self::CollateralPrincipalRefund => 11,
            Self::NeutralCollateralDisposition => 12,
            Self::NeutralLamportSink => 13,
            Self::CollateralAuthority => 14,
            Self::Realm => 15,
            Self::Profile => 16,
            Self::CollateralPolicy => 17,
            Self::CollateralMint => 18,
            Self::CollateralTokenProgram => 19,
            Self::CollateralTokenProgramData => 20,
            Self::CollateralVaultMarketCore => 21,
            Self::CollateralVaultRecoveryReserve => 22,
            Self::CollateralVaultSourceWork => 23,
            Self::CollateralVaultLiquidityFacility => 24,
            Self::CollateralVaultWrapperSet => 25,
            Self::SystemProgram => 26,
            Self::RentSysvar => 27,
            Self::ExecutingProgram => 28,
            Self::ExecutingProgramData => 29,
            Self::RegistryRelease => 30,
            Self::CapabilityProfile => 31,
            Self::SourceRelease => 32,
            Self::CompilerBundle => 33,
            Self::SeriesPlan => 34,
            Self::FundingTerms => 35,
            Self::ProductTemplate => 36,
            Self::NativeClaimBasis => 37,
            Self::RecoveryPolicy => 38,
            Self::PricePolicy => 39,
            Self::MarketGenesis => 40,
            Self::FundingQuote => 41,
            Self::AttachmentPlan => 42,
        }
    }
}

/// Required effective Solana privileges for one activation role.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivateSeriesFundingAccountMetaV2 {
    /// Exact semantic role at this index.
    pub role: ActivateSeriesFundingAccountRoleV2,
    /// Required effective signer bit after allowed privilege union.
    pub signer: bool,
    /// Required effective writable bit after allowed privilege union.
    pub writable: bool,
    /// Required executable bit.
    pub executable: bool,
}

/// Observed runtime key and privileges without a Solana SDK dependency.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObservedActivateSeriesFundingAccountMetaV2 {
    /// Exact runtime account key. The all-zero System Program key is admitted
    /// only at the SystemProgram role.
    pub key: [u8; HASH_BYTES],
    /// Effective transaction signer bit.
    pub signer: bool,
    /// Effective transaction writable bit.
    pub writable: bool,
    /// Runtime executable bit.
    pub executable: bool,
}

const fn activation_meta(
    role: ActivateSeriesFundingAccountRoleV2,
    signer: bool,
    writable: bool,
    executable: bool,
) -> ActivateSeriesFundingAccountMetaV2 {
    ActivateSeriesFundingAccountMetaV2 {
        role,
        signer,
        writable,
        executable,
    }
}

/// Frozen full account order for current FundingV2 physical activation.
pub const ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2:
    [ActivateSeriesFundingAccountMetaV2; ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2] = [
    activation_meta(ActivateSeriesFundingAccountRoleV2::Registry, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::Funding, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::Payer, true, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::LamportVaultMarketCore, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::LamportVaultSeriesAdmission, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::LamportVaultRecoveryReserve, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::LamportVaultSourceWork, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::LamportVaultLiquidityFacility, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::LamportVaultWrapperSet, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::PayerCollateralSource, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::PayerTokenAuthority, true, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralPrincipalRefund, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::NeutralCollateralDisposition, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::NeutralLamportSink, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralAuthority, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::Realm, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::Profile, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralPolicy, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralMint, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralTokenProgram, false, false, true),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralTokenProgramData, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralVaultMarketCore, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralVaultRecoveryReserve, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralVaultSourceWork, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralVaultLiquidityFacility, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CollateralVaultWrapperSet, false, true, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::SystemProgram, false, false, true),
    activation_meta(ActivateSeriesFundingAccountRoleV2::RentSysvar, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::ExecutingProgram, false, false, true),
    activation_meta(ActivateSeriesFundingAccountRoleV2::ExecutingProgramData, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::RegistryRelease, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CapabilityProfile, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::SourceRelease, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::CompilerBundle, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::SeriesPlan, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::FundingTerms, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::ProductTemplate, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::NativeClaimBasis, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::RecoveryPolicy, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::PricePolicy, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::MarketGenesis, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::FundingQuote, false, false, false),
    activation_meta(ActivateSeriesFundingAccountRoleV2::AttachmentPlan, false, false, false),
];

fn activation_alias_allowed(
    left: ActivateSeriesFundingAccountRoleV2,
    right: ActivateSeriesFundingAccountRoleV2,
) -> bool {
    matches!(
        (left, right),
        (
            ActivateSeriesFundingAccountRoleV2::Payer,
            ActivateSeriesFundingAccountRoleV2::PayerTokenAuthority
        ) | (
            ActivateSeriesFundingAccountRoleV2::PayerTokenAuthority,
            ActivateSeriesFundingAccountRoleV2::Payer
        )
    )
}

/// Validate exact count, order-derived effective privileges, executability,
/// live keys, and the sole payer/token-authority alias exception.
///
/// The accessor keeps this owner allocation-free and lets the SBF adapter pass
/// its live `AccountInfo` slice without creating a second role table.
pub fn validate_activate_series_funding_account_metas_v2<F>(
    observed_len: usize,
    mut observed_at: F,
) -> Result<()>
where
    F: FnMut(usize) -> Option<ObservedActivateSeriesFundingAccountMetaV2>,
{
    if observed_len < ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2 {
        return Err(CodecError::Truncated);
    }
    if observed_len > ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2 {
        return Err(CodecError::TrailingBytes);
    }
    let mut index = 0usize;
    while index < ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2 {
        let observed = observed_at(index).ok_or(CodecError::InvalidCount)?;
        let requirement = ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2[index];
        if (is_zero(&observed.key)
            && requirement.role != ActivateSeriesFundingAccountRoleV2::SystemProgram)
            || observed.executable != requirement.executable
        {
            return Err(CodecError::MismatchedBinding);
        }
        let mut effective_signer = requirement.signer;
        let mut effective_writable = requirement.writable;
        let mut other_index = 0usize;
        while other_index < ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2 {
            if other_index != index {
                let other = observed_at(other_index).ok_or(CodecError::InvalidCount)?;
                if observed.key == other.key {
                    let other_requirement =
                        ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2[other_index];
                    if !activation_alias_allowed(requirement.role, other_requirement.role) {
                        return Err(CodecError::MismatchedBinding);
                    }
                    effective_signer |= other_requirement.signer;
                    effective_writable |= other_requirement.writable;
                }
            }
            other_index += 1;
        }
        if observed.signer != effective_signer || observed.writable != effective_writable {
            return Err(CodecError::MismatchedBinding);
        }
        index += 1;
    }
    Ok(())
}

const SERIES_REGISTRY_RESERVED_BYTES_V1: usize = 28;

fn require_exact(input: &[u8], exact: usize) -> Result<()> {
    if input.len() < exact {
        Err(CodecError::Truncated)
    } else if input.len() > exact {
        Err(CodecError::TrailingBytes)
    } else {
        Ok(())
    }
}

fn require_live(bytes: [u8; HASH_BYTES]) -> Result<()> {
    if is_zero(&bytes) {
        Err(CodecError::ZeroIdentity)
    } else {
        Ok(())
    }
}

fn map_product_error(error: clutch_product_series::Error) -> CodecError {
    match error {
        clutch_product_series::Error::Truncated => CodecError::Truncated,
        clutch_product_series::Error::TrailingBytes => CodecError::TrailingBytes,
        clutch_product_series::Error::BadMagic => CodecError::WrongTag,
        clutch_product_series::Error::BadVersion => CodecError::WrongVersion,
        clutch_product_series::Error::NonCanonicalReserved
        | clutch_product_series::Error::NonCanonicalPadding => CodecError::NonCanonicalPadding,
        clutch_product_series::Error::ZeroIdentity => CodecError::ZeroIdentity,
        clutch_product_series::Error::ArithmeticOverflow => CodecError::ArithmeticOverflow,
        clutch_product_series::Error::MismatchedArtifact
        | clutch_product_series::Error::InvalidComponentStatus
        | clutch_product_series::Error::InsufficientPrepayment
        | clutch_product_series::Error::UnauthenticatedAuthority => CodecError::MismatchedBinding,
        clutch_product_series::Error::InvalidParameter
        | clutch_product_series::Error::InvalidSchedule
        | clutch_product_series::Error::WrongOrdinal
        | clutch_product_series::Error::SeriesNotActive
        | clutch_product_series::Error::OutsideCreationWindow
        | clutch_product_series::Error::SeriesNotClosed => CodecError::InvalidCount,
        clutch_product_series::Error::LegacyNumericFallback
        | clutch_product_series::Error::UnsupportedCapability => CodecError::InvalidEnum,
        clutch_product_series::Error::IntervalTooWide
        | clutch_product_series::Error::WorkLimitExceeded
        | clutch_product_series::Error::IntervalPayoutDisagreement
        | clutch_product_series::Error::WorkIncomplete
        | clutch_product_series::Error::WorkAlreadyComplete
        | clutch_product_series::Error::WorkStateMismatch
        | clutch_product_series::Error::RuntimeCapabilityDisabled => CodecError::InvalidCount,
    }
}

fn put_id(out: &mut [u8], at: &mut usize, bytes: [u8; HASH_BYTES]) {
    out[*at..*at + HASH_BYTES].copy_from_slice(&bytes);
    *at += HASH_BYTES;
}

fn take_id(input: &[u8], at: &mut usize) -> [u8; HASH_BYTES] {
    let mut bytes = [0; HASH_BYTES];
    bytes.copy_from_slice(&input[*at..*at + HASH_BYTES]);
    *at += HASH_BYTES;
    bytes
}

fn put_u64(out: &mut [u8], at: &mut usize, value: u64) {
    out[*at..*at + 8].copy_from_slice(&value.to_le_bytes());
    *at += 8;
}

fn take_u64(input: &[u8], at: &mut usize) -> u64 {
    let value = u64::from_le_bytes([
        input[*at],
        input[*at + 1],
        input[*at + 2],
        input[*at + 3],
        input[*at + 4],
        input[*at + 5],
        input[*at + 6],
        input[*at + 7],
    ]);
    *at += 8;
    value
}

fn require_reserved(input: &[u8]) -> Result<()> {
    if input.iter().any(|byte| *byte != 0) {
        Err(CodecError::NonCanonicalPadding)
    } else {
        Ok(())
    }
}

/// Persistent proof-carrying selection and replay anchor for one V5 Series
/// under one registry release/profile pair.
///
/// The account intentionally stores references rather than a copied registry
/// projection. The central registry remains the sole owner of selector
/// semantics; every value-bearing consumer must reauthenticate the exact
/// `registry_release_id` and `capability_profile_id` and reconstruct the
/// complete projection from its authoritative accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRegistryAccountV1 {
    /// Exact registered recurring Series artifact.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact immutable funding/refund ownership artifact.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Exact central registry release authenticated when this account is made.
    pub registry_release_id: ContentId,
    /// Exact registry capability profile selected through Genesis V2.
    pub capability_profile_id: ContentId,
    /// Exact payer-owned rent principal locked at account creation.
    pub rent_principal_lamports: u64,
    /// Canonical account PDA bump.
    pub stored_bump: u8,
    /// Whether the one permitted funding activation has been consumed.
    pub activation_consumed: bool,
}

impl SeriesRegistryAccountV1 {
    /// Validate the canonical shape without claiming registry authenticity.
    pub fn validate(&self) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        self.funding_terms_id
            .validate()
            .map_err(map_product_error)?;
        require_live(self.registry_release_id.bytes())?;
        require_live(self.capability_profile_id.bytes())?;
        if self.rent_principal_lamports == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }

    /// Encode exactly [`SERIES_REGISTRY_ACCOUNT_BYTES_V1`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() < SERIES_REGISTRY_ACCOUNT_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > SERIES_REGISTRY_ACCOUNT_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        out.fill(0);
        out[0] = registry::SOURCE_SERIES_REGISTRY_ACCOUNT_TAG;
        out[1] = registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V1;
        out[2] = self.stored_bump;
        out[3] = u8::from(self.activation_consumed);
        let mut at = 4;
        put_u64(out, &mut at, self.rent_principal_lamports);
        put_id(out, &mut at, self.series_plan_id.bytes());
        put_id(out, &mut at, self.funding_terms_id.bytes());
        put_id(out, &mut at, self.registry_release_id.bytes());
        put_id(out, &mut at, self.capability_profile_id.bytes());
        at += SERIES_REGISTRY_RESERVED_BYTES_V1;
        if at != SERIES_REGISTRY_ACCOUNT_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(())
    }

    /// Decode an exact hostile account body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, SERIES_REGISTRY_ACCOUNT_BYTES_V1)?;
        if input[0] != registry::SOURCE_SERIES_REGISTRY_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V1 {
            return Err(CodecError::WrongVersion);
        }
        let stored_bump = input[2];
        let activation_consumed = match input[3] {
            0 => false,
            1 => true,
            _ => return Err(CodecError::InvalidEnum),
        };
        let mut at = 4;
        let rent_principal_lamports = take_u64(input, &mut at);
        let series_plan_id = SeriesPlanV5Id::from_bytes(take_id(input, &mut at));
        let funding_terms_id = SeriesFundingTermsV2Id::from_bytes(take_id(input, &mut at));
        let registry_release_id = ContentId::from_bytes(take_id(input, &mut at));
        let capability_profile_id = ContentId::from_bytes(take_id(input, &mut at));
        require_reserved(&input[at..at + SERIES_REGISTRY_RESERVED_BYTES_V1])?;
        at += SERIES_REGISTRY_RESERVED_BYTES_V1;
        if at != input.len() {
            return Err(CodecError::TrailingBytes);
        }
        let value = Self {
            series_plan_id,
            funding_terms_id,
            registry_release_id,
            capability_profile_id,
            rent_principal_lamports,
            stored_bump,
            activation_consumed,
        };
        value.validate()?;
        Ok(value)
    }
}

/// Current persistent Series registration and replay anchor.
///
/// Unlike the historical V1 account, this body retains the exact BundleV5
/// identity selected by registration. Every later value-bearing adapter must
/// reopen that content-addressed bundle and its current QuoteV4/AttachmentV4
/// graph before it may interpret any Series state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesRegistryAccountV2 {
    /// Exact registered recurring Series artifact.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact immutable funding/refund ownership artifact.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Exact loader-authenticated RegistryProgramReleaseV2 artifact.
    pub registry_release_id: ContentId,
    /// Exact RegistryCapabilityProfileV4 artifact.
    pub capability_profile_id: ContentId,
    /// Exact current compiler output; this transitively retains the Source
    /// release, QuoteV4, AttachmentV4, and all immutable Product identities.
    pub compiler_bundle_id: CompiledProductSeriesBundleV5Id,
    /// Exact payer-owned rent principal locked at account creation.
    pub rent_principal_lamports: u64,
    /// Canonical account PDA bump.
    pub stored_bump: u8,
    /// Whether the one permitted successor funding activation was consumed.
    pub activation_consumed: bool,
}

impl SeriesRegistryAccountV2 {
    /// Validate canonical typed identities without claiming account authority.
    pub fn validate(&self) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        self.funding_terms_id
            .validate()
            .map_err(map_product_error)?;
        require_live(self.registry_release_id.bytes())?;
        require_live(self.capability_profile_id.bytes())?;
        self.compiler_bundle_id
            .validate()
            .map_err(map_product_error)?;
        if self.rent_principal_lamports == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }

    /// Encode the exact current 0x7f/version2 body.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() < SERIES_REGISTRY_ACCOUNT_BYTES_V2 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > SERIES_REGISTRY_ACCOUNT_BYTES_V2 {
            return Err(CodecError::TrailingBytes);
        }
        out.fill(0);
        out[0] = registry::SOURCE_SERIES_REGISTRY_ACCOUNT_TAG;
        out[1] = registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2;
        out[2] = self.stored_bump;
        out[3] = u8::from(self.activation_consumed);
        let mut at = 4;
        put_u64(out, &mut at, self.rent_principal_lamports);
        put_id(out, &mut at, self.series_plan_id.bytes());
        put_id(out, &mut at, self.funding_terms_id.bytes());
        put_id(out, &mut at, self.registry_release_id.bytes());
        put_id(out, &mut at, self.capability_profile_id.bytes());
        put_id(out, &mut at, self.compiler_bundle_id.bytes());
        if at != SERIES_REGISTRY_ACCOUNT_BYTES_V2 {
            return Err(CodecError::OutputTooSmall);
        }
        Ok(())
    }

    /// Hostile-decode the exact current body, refusing V1 and all trailing data.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, SERIES_REGISTRY_ACCOUNT_BYTES_V2)?;
        if input[0] != registry::SOURCE_SERIES_REGISTRY_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2 {
            return Err(CodecError::WrongVersion);
        }
        let activation_consumed = match input[3] {
            0 => false,
            1 => true,
            _ => return Err(CodecError::InvalidEnum),
        };
        let mut at = 4;
        let rent_principal_lamports = take_u64(input, &mut at);
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(take_id(input, &mut at)),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes(take_id(input, &mut at)),
            registry_release_id: ContentId::from_bytes(take_id(input, &mut at)),
            capability_profile_id: ContentId::from_bytes(take_id(input, &mut at)),
            compiler_bundle_id: CompiledProductSeriesBundleV5Id::from_bytes(take_id(input, &mut at)),
            rent_principal_lamports,
            stored_bump: input[2],
            activation_consumed,
        };
        if at != input.len() {
            return Err(CodecError::TrailingBytes);
        }
        value.validate()?;
        Ok(value)
    }
}

/// Program-owned framing for the pure 324-byte Series funding state.
///
/// The embedded [`SeriesFundingStateV1`] is the sole semantic owner of cursor,
/// payer-principal, donation, and allocation-consumption facts. The wrapper
/// adds global account discrimination, the canonical PDA bump, exact
/// refundable state-account rent principal, and the five separately
/// refundable release-selected collateral-vault rent principals.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingAccountV1 {
    /// Exact pure funding/lifecycle state.
    pub state: SeriesFundingStateV1,
    /// Exact payer-owned rent principal locked at account creation.
    pub rent_principal_lamports: u64,
    /// Exact payer-owned rent principal in each collateral component vault.
    pub collateral_vault_rent_principal_lamports: [u64; SERIES_FUNDING_COMPONENT_COUNT],
    /// Canonical account PDA bump.
    pub stored_bump: u8,
    /// Reserved flags; must be zero.
    pub flags: u8,
}

impl SeriesFundingAccountV1 {
    /// Validate the complete pure state and account framing.
    pub fn validate(&self) -> Result<()> {
        self.state.validate().map_err(map_product_error)?;
        if self.rent_principal_lamports == 0
            || self
                .collateral_vault_rent_principal_lamports
                .iter()
                .any(|principal| *principal == 0)
        {
            return Err(CodecError::ZeroValue);
        }
        if self.flags != 0 {
            return Err(CodecError::InvalidEnum);
        }
        Ok(())
    }

    /// Encode exactly [`SERIES_FUNDING_ACCOUNT_BYTES_V1`] bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() < SERIES_FUNDING_ACCOUNT_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > SERIES_FUNDING_ACCOUNT_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        out[0] = registry::SOURCE_SERIES_FUNDING_ACCOUNT_TAG;
        out[1] = registry::SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V1;
        out[2] = self.stored_bump;
        out[3] = self.flags;
        out[4..12].copy_from_slice(&self.rent_principal_lamports.to_le_bytes());
        let mut at = 12;
        for principal in self.collateral_vault_rent_principal_lamports {
            put_u64(out, &mut at, principal);
        }
        self.state
            .encode_into(&mut out[at..])
            .map_err(map_product_error)
    }

    /// Decode an exact hostile account body and the embedded pure state.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, SERIES_FUNDING_ACCOUNT_BYTES_V1)?;
        if input[0] != registry::SOURCE_SERIES_FUNDING_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V1 {
            return Err(CodecError::WrongVersion);
        }
        let mut at = 12;
        let mut collateral_vault_rent_principal_lamports = [0; SERIES_FUNDING_COMPONENT_COUNT];
        for principal in &mut collateral_vault_rent_principal_lamports {
            *principal = take_u64(input, &mut at);
        }
        let value = Self {
            state: SeriesFundingStateV1::decode(&input[at..]).map_err(map_product_error)?,
            rent_principal_lamports: u64::from_le_bytes(
                input[4..12].try_into().map_err(|_| CodecError::Truncated)?,
            ),
            collateral_vault_rent_principal_lamports,
            stored_bump: input[2],
            flags: input[3],
        };
        value.validate()?;
        Ok(value)
    }
}

/// Program-owned current Series funding wrapper.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesFundingAccountV2 {
    /// Sole current six-component semantic state.
    pub state: SeriesFundingStateV2,
    /// Refundable payer-owned funding-account rent principal.
    pub rent_principal_lamports: u64,
    /// Refundable payer-owned rent for the five collateral-capable vaults.
    pub collateral_vault_rent_principal_lamports: [u64; SERIES_COLLATERAL_VAULT_COUNT_V2],
    /// Canonical account PDA bump.
    pub stored_bump: u8,
}

impl SeriesFundingAccountV2 {
    /// Validate exact semantic state and rent ownership.
    pub fn validate(&self) -> Result<()> {
        if self.rent_principal_lamports == 0
            || self
                .collateral_vault_rent_principal_lamports
                .iter()
                .any(|principal| *principal == 0)
        {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }

    /// Encode exact 0x80/version2 bytes.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() < SERIES_FUNDING_ACCOUNT_BYTES_V2 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > SERIES_FUNDING_ACCOUNT_BYTES_V2 {
            return Err(CodecError::TrailingBytes);
        }
        out.fill(0);
        out[0] = registry::SOURCE_SERIES_FUNDING_ACCOUNT_TAG;
        out[1] = registry::SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V2;
        out[2] = self.stored_bump;
        let mut at = 4;
        put_u64(out, &mut at, self.rent_principal_lamports);
        for principal in self.collateral_vault_rent_principal_lamports {
            put_u64(out, &mut at, principal);
        }
        self.state
            .encode_into(&mut out[at..])
            .map_err(map_product_error)
    }

    /// Hostile-decode the exact current wrapper and semantic body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, SERIES_FUNDING_ACCOUNT_BYTES_V2)?;
        if input[0] != registry::SOURCE_SERIES_FUNDING_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V2 {
            return Err(CodecError::WrongVersion);
        }
        require_reserved(&input[3..4])?;
        let mut at = 4;
        let rent_principal_lamports = take_u64(input, &mut at);
        let mut collateral_vault_rent_principal_lamports =
            [0; SERIES_COLLATERAL_VAULT_COUNT_V2];
        for principal in &mut collateral_vault_rent_principal_lamports {
            *principal = take_u64(input, &mut at);
        }
        let value = Self {
            state: SeriesFundingStateV2::decode(&input[at..]).map_err(map_product_error)?,
            rent_principal_lamports,
            collateral_vault_rent_principal_lamports,
            stored_bump: input[2],
        };
        value.validate()?;
        Ok(value)
    }
}

/// Program-owned frame for the shared Product MarketLifecycleRoot.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLifecycleRootAccountV1 {
    /// Sole semantic lifecycle body.
    pub state: MarketLifecycleRootV1,
    /// Exact refundable payer-owned account rent principal.
    pub rent_principal_lamports: u64,
    /// Canonical MarketLifecycleRoot PDA bump.
    pub stored_bump: u8,
}

impl MarketLifecycleRootAccountV1 {
    /// Invalid storage used only as an out-parameter decode target.
    pub const fn decode_buffer() -> Self {
        Self {
            state: MarketLifecycleRootV1::decode_buffer(),
            rent_principal_lamports: 0,
            stored_bump: 0,
        }
    }

    /// Encode the exact hostile account frame and semantic body.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        Self::encode_parts(
            &self.state,
            self.rent_principal_lamports,
            self.stored_bump,
            output,
        )
    }

    /// Encode from borrowed state without copying the 2,448-byte semantic root.
    pub fn encode_parts(
        state: &MarketLifecycleRootV1,
        rent_principal_lamports: u64,
        stored_bump: u8,
        output: &mut [u8],
    ) -> Result<()> {
        require_exact(output, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1)?;
        if rent_principal_lamports == 0 {
            return Err(CodecError::ZeroValue);
        }
        output.fill(0);
        output[0] = registry::PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_TAG;
        output[1] = registry::PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_VERSION;
        output[2] = stored_bump;
        output[8..16].copy_from_slice(&rent_principal_lamports.to_le_bytes());
        state
            .encode_into(&mut output[PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..])
            .map_err(map_product_error)
    }

    /// Decode the exact frame and fully validate the embedded lifecycle owner.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut value = Self::decode_buffer();
        Self::decode_into(input, &mut value)?;
        Ok(value)
    }

    /// Hostile-decode into caller-owned storage for frame-bounded adapters.
    pub fn decode_into(input: &[u8], output: &mut Self) -> Result<()> {
        require_exact(input, MARKET_LIFECYCLE_ROOT_ACCOUNT_BYTES_V1)?;
        if input[0] != registry::PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::PRODUCT_MARKET_LIFECYCLE_ROOT_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        require_reserved(&input[3..8])?;
        MarketLifecycleRootV1::decode_into(
            &input[PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..],
            &mut output.state,
        )
        .map_err(map_product_error)?;
        output.rent_principal_lamports =
            u64::from_le_bytes(input[8..16].try_into().map_err(|_| CodecError::Truncated)?);
        output.stored_bump = input[2];
        if output.rent_principal_lamports == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(())
    }
}

/// Program-owned frame for the Product Direct global-liveness manifest.
///
/// The embedded state owns no physical work balance. Its rent field is the
/// exact separately refundable principal locked in this `0xba/v1` account;
/// any lamports above it are neutral-sink donation and may never discount the
/// payer's required debit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductDirectGlobalLivenessAccountV1 {
    /// Sole Product semantic manifest/allocation lifecycle.
    pub state: DirectGlobalLivenessV1,
    /// Exact refundable payer-funded account rent principal.
    pub rent_principal_lamports: u64,
    /// Canonical Product Direct global-liveness PDA bump.
    pub stored_bump: u8,
}

impl ProductDirectGlobalLivenessAccountV1 {
    /// Encode the exact `0xba/v1` hostile account frame.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        Self::encode_parts(
            &self.state,
            self.rent_principal_lamports,
            self.stored_bump,
            output,
        )
    }

    /// Encode from borrowed state without an additional 1,008-byte copy.
    pub fn encode_parts(
        state: &DirectGlobalLivenessV1,
        rent_principal_lamports: u64,
        stored_bump: u8,
        output: &mut [u8],
    ) -> Result<()> {
        require_exact(output, PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1)?;
        state.validate().map_err(map_product_error)?;
        if rent_principal_lamports == 0
            || rent_principal_lamports != state.manifest_rent_principal_lamports()
        {
            return Err(if rent_principal_lamports == 0 {
                CodecError::ZeroValue
            } else {
                CodecError::MismatchedBinding
            });
        }
        output.fill(0);
        output[0] = registry::PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_TAG;
        output[1] = registry::PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_VERSION;
        output[2] = stored_bump;
        output[8..16].copy_from_slice(&rent_principal_lamports.to_le_bytes());
        state
            .encode_into(&mut output[PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..])
            .map_err(map_product_error)
    }

    /// Hostile-decode the exact tag/version/width and complete semantic body.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_BYTES_V1)?;
        if input[0] != registry::PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::PRODUCT_DIRECT_GLOBAL_LIVENESS_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        require_reserved(&input[3..8])?;
        let rent_principal_lamports = u64::from_le_bytes(
            input[8..16]
                .try_into()
                .map_err(|_| CodecError::Truncated)?,
        );
        let state = DirectGlobalLivenessV1::decode(
            &input[PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..],
        )
        .map_err(map_product_error)?;
        if rent_principal_lamports == 0
            || rent_principal_lamports != state.manifest_rent_principal_lamports()
        {
            return Err(if rent_principal_lamports == 0 {
                CodecError::ZeroValue
            } else {
                CodecError::MismatchedBinding
            });
        }
        Ok(Self {
            state,
            rent_principal_lamports,
            stored_bump: input[2],
        })
    }
}

/// Program-owned frame for the compact permanent Product Market replay anchor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MarketLifecycleReplayAccountV1 {
    /// Sole field-level semantic owner.
    pub receipt: MarketLifecycleReplayReceiptV1,
    /// Exact permanent replay-account rent principal.
    pub permanent_rent_principal_lamports: u64,
    /// Canonical replay PDA bump.
    pub stored_bump: u8,
}

impl MarketLifecycleReplayAccountV1 {
    /// Encode the exact 16-byte frame plus Product-owned semantic body.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        require_exact(output, MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V1)?;
        if self.permanent_rent_principal_lamports == 0 {
            return Err(CodecError::ZeroValue);
        }
        output.fill(0);
        output[0] = registry::PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_TAG;
        output[1] = registry::PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_VERSION;
        output[2] = self.stored_bump;
        output[8..16].copy_from_slice(&self.permanent_rent_principal_lamports.to_le_bytes());
        self.receipt
            .encode_into(&mut output[PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..])
            .map_err(map_product_error)
    }

    /// Hostile-decode the exact frame and complete Product semantic receipt.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, MARKET_LIFECYCLE_REPLAY_ACCOUNT_BYTES_V1)?;
        if input[0] != registry::PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::PRODUCT_MARKET_LIFECYCLE_REPLAY_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        require_reserved(&input[3..8])?;
        let value = Self {
            receipt: MarketLifecycleReplayReceiptV1::decode(
                &input[PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..],
            )
            .map_err(map_product_error)?,
            permanent_rent_principal_lamports: u64::from_le_bytes(
                input[8..16].try_into().map_err(|_| CodecError::Truncated)?,
            ),
            stored_bump: input[2],
        };
        if value.permanent_rent_principal_lamports == 0 {
            return Err(CodecError::ZeroValue);
        }
        Ok(value)
    }
}

/// Program-owned frame for one Series/ordinal admission link.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SeriesMarketLinkAccountV1 {
    /// Sole semantic SeriesMarketLink body.
    pub state: SeriesMarketLinkV1,
    /// Canonical SeriesMarketLink PDA bump.
    pub stored_bump: u8,
}

impl SeriesMarketLinkAccountV1 {
    /// Invalid storage used only as an out-parameter decode target.
    pub fn decode_buffer() -> Self {
        Self {
            state: SeriesMarketLinkV1::decode_buffer(),
            stored_bump: 0,
        }
    }

    /// Encode the exact hostile account frame and semantic body.
    pub fn encode(&self, output: &mut [u8]) -> Result<()> {
        Self::encode_parts(&self.state, self.stored_bump, output)
    }

    /// Encode from borrowed state without copying the 1,232-byte link body.
    pub fn encode_parts(
        state: &SeriesMarketLinkV1,
        stored_bump: u8,
        output: &mut [u8],
    ) -> Result<()> {
        require_exact(output, SERIES_MARKET_LINK_ACCOUNT_BYTES_V1)?;
        output.fill(0);
        output[0] = registry::PRODUCT_SERIES_MARKET_LINK_ACCOUNT_TAG;
        output[1] = registry::PRODUCT_SERIES_MARKET_LINK_ACCOUNT_VERSION;
        output[2] = stored_bump;
        state
            .encode_into(&mut output[PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..])
            .map_err(map_product_error)
    }

    /// Decode the exact frame and fully validate the embedded link owner.
    pub fn decode(input: &[u8]) -> Result<Self> {
        let mut value = Self::decode_buffer();
        Self::decode_into(input, &mut value)?;
        Ok(value)
    }

    /// Hostile-decode into caller-owned storage for frame-bounded adapters.
    pub fn decode_into(input: &[u8], output: &mut Self) -> Result<()> {
        require_exact(input, SERIES_MARKET_LINK_ACCOUNT_BYTES_V1)?;
        if input[0] != registry::PRODUCT_SERIES_MARKET_LINK_ACCOUNT_TAG {
            return Err(CodecError::WrongTag);
        }
        if input[1] != registry::PRODUCT_SERIES_MARKET_LINK_ACCOUNT_VERSION {
            return Err(CodecError::WrongVersion);
        }
        require_reserved(&input[3..8])?;
        SeriesMarketLinkV1::decode_into(
            &input[PRODUCT_MARKET_ACCOUNT_HEADER_BYTES_V1..],
            &mut output.state,
        )
        .map_err(map_product_error)?;
        output.stored_bump = input[2];
        Ok(())
    }
}

/// Exact Source/Series registration payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegisterSeriesIntentV1 {
    /// Exact Series artifact expected at its content-derived PDA.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact funding-ownership artifact expected at its content-derived PDA.
    pub funding_terms_id: SeriesFundingTermsV2Id,
    /// Registry release that must authenticate the complete projection.
    pub registry_release_id: ContentId,
    /// Capability profile selected by the Series' Genesis V2 artifact.
    pub capability_profile_id: ContentId,
}

impl RegisterSeriesIntentV1 {
    /// Validate nonzero typed identities without accepting them as authority.
    pub fn validate(&self) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        self.funding_terms_id
            .validate()
            .map_err(map_product_error)?;
        require_live(self.registry_release_id.bytes())?;
        require_live(self.capability_profile_id.bytes())
    }

    /// Encode the exact action-owned payload, excluding extension envelope.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() < REGISTER_SERIES_PAYLOAD_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > REGISTER_SERIES_PAYLOAD_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        let mut at = 0;
        put_id(out, &mut at, self.series_plan_id.bytes());
        put_id(out, &mut at, self.funding_terms_id.bytes());
        put_id(out, &mut at, self.registry_release_id.bytes());
        put_id(out, &mut at, self.capability_profile_id.bytes());
        Ok(())
    }

    /// Decode an exact action-owned payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, REGISTER_SERIES_PAYLOAD_BYTES_V1)?;
        let mut at = 0;
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(take_id(input, &mut at)),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes(take_id(input, &mut at)),
            registry_release_id: ContentId::from_bytes(take_id(input, &mut at)),
            capability_profile_id: ContentId::from_bytes(take_id(input, &mut at)),
        };
        value.validate()?;
        Ok(value)
    }
}

/// Exact activation payload. All amounts are derived from authenticated
/// artifacts and observed transfers, never caller supplied on wire.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActivateSeriesFundingIntentV1 {
    /// Registered Series whose deterministic funding PDA is activated.
    pub series_plan_id: SeriesPlanV5Id,
}

impl ActivateSeriesFundingIntentV1 {
    /// Encode the exact action-owned payload.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        if out.len() < ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        out.copy_from_slice(&self.series_plan_id.bytes());
        Ok(())
    }

    /// Decode the exact action-owned payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1)?;
        let mut at = 0;
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(take_id(input, &mut at)),
        };
        value.series_plan_id.validate().map_err(map_product_error)?;
        Ok(value)
    }
}

/// Exact next-occurrence payload. Component debit amounts and present/absent
/// status are deliberately absent: the adapter must derive them from exact
/// artifacts and authenticated runtime receipt accounts.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AdvanceSeriesOccurrenceIntentV1 {
    /// Exact registered Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact expected next ordinal.
    pub ordinal: u32,
    /// Exact immutable SourcePlane provenance record.
    pub source_occurrence_id: SourceOccurrenceV1Id,
    /// Full-width economic instance identity.
    pub market_instance_id: MarketInstanceV2Id,
}

impl AdvanceSeriesOccurrenceIntentV1 {
    /// Encode the exact action-owned payload.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.validate()?;
        if out.len() < ADVANCE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > ADVANCE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        out.fill(0);
        let mut at = 0;
        put_id(out, &mut at, self.series_plan_id.bytes());
        out[at..at + 4].copy_from_slice(&self.ordinal.to_le_bytes());
        at += 8;
        put_id(out, &mut at, self.source_occurrence_id.bytes());
        put_id(out, &mut at, self.market_instance_id.bytes());
        Ok(())
    }

    /// Decode the exact action-owned payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, ADVANCE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1)?;
        let mut at = 0;
        let series_plan_id = SeriesPlanV5Id::from_bytes(take_id(input, &mut at));
        let ordinal = u32::from_le_bytes(
            input[at..at + 4]
                .try_into()
                .map_err(|_| CodecError::Truncated)?,
        );
        at += 4;
        require_reserved(&input[at..at + 4])?;
        at += 4;
        let value = Self {
            series_plan_id,
            ordinal,
            source_occurrence_id: SourceOccurrenceV1Id::from_bytes(take_id(input, &mut at)),
            market_instance_id: MarketInstanceV2Id::from_bytes(take_id(input, &mut at)),
        };
        value.validate()?;
        Ok(value)
    }

    fn validate(&self) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        self.source_occurrence_id
            .validate()
            .map_err(map_product_error)?;
        self.market_instance_id
            .validate()
            .map_err(map_product_error)
    }
}

/// Exact free-lapse payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LapseSeriesOccurrenceIntentV1 {
    /// Exact registered Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// Exact expected next ordinal.
    pub ordinal: u32,
}

impl LapseSeriesOccurrenceIntentV1 {
    /// Encode the exact action-owned payload.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        if out.len() < LAPSE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > LAPSE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        out.fill(0);
        out[..HASH_BYTES].copy_from_slice(&self.series_plan_id.bytes());
        out[HASH_BYTES..HASH_BYTES + 4].copy_from_slice(&self.ordinal.to_le_bytes());
        Ok(())
    }

    /// Decode the exact action-owned payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, LAPSE_SERIES_OCCURRENCE_PAYLOAD_BYTES_V1)?;
        require_reserved(&input[HASH_BYTES + 4..])?;
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(
                input[..HASH_BYTES]
                    .try_into()
                    .map_err(|_| CodecError::Truncated)?,
            ),
            ordinal: u32::from_le_bytes(
                input[HASH_BYTES..HASH_BYTES + 4]
                    .try_into()
                    .map_err(|_| CodecError::Truncated)?,
            ),
        };
        value.series_plan_id.validate().map_err(map_product_error)?;
        Ok(value)
    }
}

/// Physical value kind whose balance surplus is being observed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum SeriesFundingAssetV1 {
    /// Native lamports in the component's zero-data custody PDA.
    Lamports = 1,
    /// Collateral atoms in the component's authenticated Token-2022 vault.
    Collateral = 2,
}

impl SeriesFundingAssetV1 {
    /// Stable hostile-wire byte without an unchecked enum cast.
    pub const fn byte(self) -> u8 {
        match self {
            Self::Lamports => 1,
            Self::Collateral => 2,
        }
    }

    fn decode(value: u8) -> Result<Self> {
        match value {
            1 => Ok(Self::Lamports),
            2 => Ok(Self::Collateral),
            _ => Err(CodecError::InvalidEnum),
        }
    }
}

/// Exact donation-observation payload. The amount is the authenticated surplus
/// between physical custody and accounted state, not a wire field.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObserveSeriesDonationIntentV1 {
    /// Exact registered Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// One of the five historical quote-owned components.
    pub component: SeriesFundingComponentV1,
    /// Physical asset balance being observed.
    pub asset: SeriesFundingAssetV1,
}

/// Current donation intent with explicit V2 component ordering.
///
/// Byte 34 is schema `2`, making this disjoint from the historical V1 payload
/// whose entire six-byte tail had to be zero.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ObserveSeriesDonationIntentV2 {
    /// Exact registered Series.
    pub series_plan_id: SeriesPlanV5Id,
    /// One of the six QuoteV4 components.
    pub component: clutch_product_series::SeriesFundingComponentV2,
    /// Physical asset balance being observed.
    pub asset: SeriesFundingAssetV1,
}

impl ObserveSeriesDonationIntentV2 {
    /// Encode the exact current action payload.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        if out.len() < OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        out.fill(0);
        out[..HASH_BYTES].copy_from_slice(&self.series_plan_id.bytes());
        out[HASH_BYTES] = match self.component {
            clutch_product_series::SeriesFundingComponentV2::MarketCore => 0,
            clutch_product_series::SeriesFundingComponentV2::SeriesAdmission => 1,
            clutch_product_series::SeriesFundingComponentV2::RecoveryReserve => 2,
            clutch_product_series::SeriesFundingComponentV2::SourceWork => 3,
            clutch_product_series::SeriesFundingComponentV2::LiquidityFacility => 4,
            clutch_product_series::SeriesFundingComponentV2::WrapperSet => 5,
        };
        out[HASH_BYTES + 1] = self.asset.byte();
        out[HASH_BYTES + 2] = 2;
        Ok(())
    }

    /// Hostile-decode only the explicit current schema.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1)?;
        if input[HASH_BYTES + 2] != 2 {
            return Err(CodecError::WrongVersion);
        }
        require_reserved(&input[HASH_BYTES + 3..])?;
        let component = match input[HASH_BYTES] {
            0 => clutch_product_series::SeriesFundingComponentV2::MarketCore,
            1 => clutch_product_series::SeriesFundingComponentV2::SeriesAdmission,
            2 => clutch_product_series::SeriesFundingComponentV2::RecoveryReserve,
            3 => clutch_product_series::SeriesFundingComponentV2::SourceWork,
            4 => clutch_product_series::SeriesFundingComponentV2::LiquidityFacility,
            5 => clutch_product_series::SeriesFundingComponentV2::WrapperSet,
            _ => return Err(CodecError::InvalidEnum),
        };
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(
                input[..HASH_BYTES]
                    .try_into()
                    .map_err(|_| CodecError::Truncated)?,
            ),
            component,
            asset: SeriesFundingAssetV1::decode(input[HASH_BYTES + 1])?,
        };
        value.series_plan_id.validate().map_err(map_product_error)?;
        Ok(value)
    }
}

impl ObserveSeriesDonationIntentV1 {
    /// Encode the exact action-owned payload.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        if out.len() < OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        out.fill(0);
        out[..HASH_BYTES].copy_from_slice(&self.series_plan_id.bytes());
        out[HASH_BYTES] = self.component.byte();
        out[HASH_BYTES + 1] = self.asset.byte();
        Ok(())
    }

    /// Decode the exact action-owned payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, OBSERVE_SERIES_DONATION_PAYLOAD_BYTES_V1)?;
        require_reserved(&input[HASH_BYTES + 2..])?;
        let component = match input[HASH_BYTES] {
            0 => SeriesFundingComponentV1::MarketCore,
            1 => SeriesFundingComponentV1::RecoveryReserve,
            2 => SeriesFundingComponentV1::SourceWork,
            3 => SeriesFundingComponentV1::LiquidityFacility,
            4 => SeriesFundingComponentV1::WrapperSet,
            _ => return Err(CodecError::InvalidEnum),
        };
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(
                input[..HASH_BYTES]
                    .try_into()
                    .map_err(|_| CodecError::Truncated)?,
            ),
            component,
            asset: SeriesFundingAssetV1::decode(input[HASH_BYTES + 1])?,
        };
        value.series_plan_id.validate().map_err(map_product_error)?;
        Ok(value)
    }
}

/// Exact terminal funding payload. Destinations and amounts remain owned by
/// FundingTerms V2 and the funding state rather than being caller supplied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CloseSeriesFundingIntentV1 {
    /// Exact registered Series.
    pub series_plan_id: SeriesPlanV5Id,
}

impl CloseSeriesFundingIntentV1 {
    /// Encode the exact action-owned payload.
    pub fn encode(&self, out: &mut [u8]) -> Result<()> {
        self.series_plan_id.validate().map_err(map_product_error)?;
        if out.len() < CLOSE_SERIES_FUNDING_PAYLOAD_BYTES_V1 {
            return Err(CodecError::OutputTooSmall);
        }
        if out.len() > CLOSE_SERIES_FUNDING_PAYLOAD_BYTES_V1 {
            return Err(CodecError::TrailingBytes);
        }
        out.copy_from_slice(&self.series_plan_id.bytes());
        Ok(())
    }

    /// Decode the exact action-owned payload.
    pub fn decode(input: &[u8]) -> Result<Self> {
        require_exact(input, CLOSE_SERIES_FUNDING_PAYLOAD_BYTES_V1)?;
        let value = Self {
            series_plan_id: SeriesPlanV5Id::from_bytes(
                input.try_into().map_err(|_| CodecError::Truncated)?,
            ),
        };
        value.series_plan_id.validate().map_err(map_product_error)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observed_activation_meta(
        index: usize,
        payer_authority_alias: bool,
    ) -> ObservedActivateSeriesFundingAccountMetaV2 {
        let requirement = ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2[index];
        let payer_index = ActivateSeriesFundingAccountRoleV2::Payer.index();
        let authority_index = ActivateSeriesFundingAccountRoleV2::PayerTokenAuthority.index();
        let key_byte = u8::try_from(index.checked_add(1).unwrap()).unwrap();
        let key = if requirement.role == ActivateSeriesFundingAccountRoleV2::SystemProgram {
            [0; HASH_BYTES]
        } else if payer_authority_alias && index == authority_index {
            let payer_byte = u8::try_from(payer_index.checked_add(1).unwrap()).unwrap();
            [payer_byte; HASH_BYTES]
        } else {
            [key_byte; HASH_BYTES]
        };
        let aliased = payer_authority_alias && (index == payer_index || index == authority_index);
        ObservedActivateSeriesFundingAccountMetaV2 {
            key,
            signer: if aliased { true } else { requirement.signer },
            writable: if aliased { true } else { requirement.writable },
            executable: requirement.executable,
        }
    }

    fn registry_v2() -> SeriesRegistryAccountV2 {
        SeriesRegistryAccountV2 {
            series_plan_id: SeriesPlanV5Id::from_bytes([1; HASH_BYTES]),
            funding_terms_id: SeriesFundingTermsV2Id::from_bytes([2; HASH_BYTES]),
            registry_release_id: ContentId::from_bytes([3; HASH_BYTES]),
            capability_profile_id: ContentId::from_bytes([4; HASH_BYTES]),
            compiler_bundle_id: CompiledProductSeriesBundleV5Id::from_bytes([5; HASH_BYTES]),
            rent_principal_lamports: 7,
            stored_bump: 9,
            activation_consumed: false,
        }
    }

    fn funding_v2() -> SeriesFundingAccountV2 {
        SeriesFundingAccountV2 {
            state: SeriesFundingStateV2 {
                series_plan_id: SeriesPlanV5Id::from_bytes([1; HASH_BYTES]),
                funding_terms_id: SeriesFundingTermsV2Id::from_bytes([2; HASH_BYTES]),
                funding_quote_id: clutch_product_series::SeriesFundingQuoteV4Id::from_bytes([
                    3;
                    HASH_BYTES
                ]),
                attachment_plan_id: clutch_product_series::SeriesAttachmentPlanV4Id::from_bytes([
                    4;
                    HASH_BYTES
                ]),
                compiler_bundle_id: CompiledProductSeriesBundleV5Id::from_bytes([5; HASH_BYTES]),
                instance_count: 1,
                next_ordinal: 0,
                lapsed_count: 0,
                transition_sequence: 0,
                phase: clutch_product_series::SeriesFundingPhaseV2::Active,
                pending_disposition: None,
                pending_market_instance_id: ContentId::ZERO,
                pending_source_occurrence_id: ContentId::ZERO,
                pending_series_market_link_id: ContentId::ZERO,
                pending_ordinal: 0,
                pending_reservation_receipt_id: ContentId::ZERO,
                pending_debits: [clutch_product_series::ComponentDebitV1::ZERO;
                    clutch_product_series::SERIES_FUNDING_COMPONENT_COUNT_V2],
                components: [clutch_product_series::SeriesComponentCapitalV2::ZERO;
                    clutch_product_series::SERIES_FUNDING_COMPONENT_COUNT_V2],
            },
            rent_principal_lamports: 7,
            collateral_vault_rent_principal_lamports: [8; SERIES_COLLATERAL_VAULT_COUNT_V2],
            stored_bump: 9,
        }
    }

    #[test]
    fn registry_v2_round_trips_every_owned_byte() {
        let value = registry_v2();
        let mut body = [0; SERIES_REGISTRY_ACCOUNT_BYTES_V2];
        value.encode(&mut body).unwrap();
        assert_eq!(SeriesRegistryAccountV2::decode(&body), Ok(value));
        assert_eq!(body[0], registry::SOURCE_SERIES_REGISTRY_ACCOUNT_TAG);
        assert_eq!(body[1], registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V2);
        assert_eq!(&body[140..172], &[5; HASH_BYTES]);
    }

    #[test]
    fn registry_versions_and_bundle_identity_cannot_alias() {
        let value = registry_v2();
        let mut body = [0; SERIES_REGISTRY_ACCOUNT_BYTES_V2];
        value.encode(&mut body).unwrap();
        body[1] = registry::SOURCE_SERIES_REGISTRY_ACCOUNT_VERSION_V1;
        assert_eq!(
            SeriesRegistryAccountV2::decode(&body),
            Err(CodecError::WrongVersion)
        );
        value.encode(&mut body).unwrap();
        body[140..172].fill(0);
        assert_eq!(
            SeriesRegistryAccountV2::decode(&body),
            Err(CodecError::ZeroIdentity)
        );
    }

    #[test]
    fn funding_v2_round_trips_and_refuses_v1_version() {
        let value = funding_v2();
        let mut body = [0; SERIES_FUNDING_ACCOUNT_BYTES_V2];
        value.encode(&mut body).unwrap();
        assert_eq!(SeriesFundingAccountV2::decode(&body), Ok(value));
        body[1] = registry::SOURCE_SERIES_FUNDING_ACCOUNT_VERSION_V1;
        assert_eq!(
            SeriesFundingAccountV2::decode(&body),
            Err(CodecError::WrongVersion)
        );
    }

    #[test]
    fn activation_account_contract_accepts_only_exact_ordered_roles() {
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2,
                |index| Some(observed_activation_meta(index, false)),
            ),
            Ok(())
        );
        assert_eq!(
            ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2
                [ACTIVATE_SERIES_LAMPORT_VAULT_START_V2]
                .role,
            ActivateSeriesFundingAccountRoleV2::LamportVaultMarketCore
        );
        assert_eq!(
            ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2
                [ACTIVATE_SERIES_COLLATERAL_VAULT_START_V2]
                .role,
            ActivateSeriesFundingAccountRoleV2::CollateralVaultMarketCore
        );
        assert_eq!(
            ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2
                [ActivateSeriesFundingAccountRoleV2::CollateralPrincipalRefund.index()]
                .writable,
            false
        );
        assert_eq!(
            ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2
                [ActivateSeriesFundingAccountRoleV2::NeutralCollateralDisposition.index()]
                .writable,
            false
        );
        assert_eq!(
            ACTIVATE_SERIES_FUNDING_ACCOUNT_METAS_V2[ACTIVATE_SERIES_ARTIFACT_START_V2].role,
            ActivateSeriesFundingAccountRoleV2::SeriesPlan
        );
    }

    #[test]
    fn activation_payer_authority_alias_requires_union_privileges() {
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2,
                |index| Some(observed_activation_meta(index, true)),
            ),
            Ok(())
        );
        let authority_index = ActivateSeriesFundingAccountRoleV2::PayerTokenAuthority.index();
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2,
                |index| {
                    let mut observed = observed_activation_meta(index, true);
                    if index == authority_index {
                        observed.writable = false;
                    }
                    Some(observed)
                },
            ),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn activation_wrong_alias_zero_key_and_executable_refuse() {
        let neutral_index = ActivateSeriesFundingAccountRoleV2::NeutralLamportSink.index();
        let payer_index = ActivateSeriesFundingAccountRoleV2::Payer.index();
        let payer_key = observed_activation_meta(payer_index, false).key;
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2,
                |index| {
                    let mut observed = observed_activation_meta(index, false);
                    if index == neutral_index {
                        observed.key = payer_key;
                    }
                    Some(observed)
                },
            ),
            Err(CodecError::MismatchedBinding)
        );
        let refund_index =
            ActivateSeriesFundingAccountRoleV2::CollateralPrincipalRefund.index();
        let disposition_index =
            ActivateSeriesFundingAccountRoleV2::NeutralCollateralDisposition.index();
        let refund_key = observed_activation_meta(refund_index, false).key;
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2,
                |index| {
                    let mut observed = observed_activation_meta(index, false);
                    if index == disposition_index {
                        observed.key = refund_key;
                    }
                    Some(observed)
                },
            ),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2,
                |index| {
                    let mut observed = observed_activation_meta(index, false);
                    if index == refund_index {
                        observed.writable = true;
                    }
                    Some(observed)
                },
            ),
            Err(CodecError::MismatchedBinding)
        );
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2,
                |index| {
                    let mut observed = observed_activation_meta(index, false);
                    if index == payer_index {
                        observed.key = [0; HASH_BYTES];
                    }
                    Some(observed)
                },
            ),
            Err(CodecError::MismatchedBinding)
        );
        let token_program =
            ActivateSeriesFundingAccountRoleV2::CollateralTokenProgram.index();
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2,
                |index| {
                    let mut observed = observed_activation_meta(index, false);
                    if index == token_program {
                        observed.executable = false;
                    }
                    Some(observed)
                },
            ),
            Err(CodecError::MismatchedBinding)
        );
    }

    #[test]
    fn activation_account_count_is_exact() {
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2
                    .checked_sub(1)
                    .unwrap(),
                |_| None,
            ),
            Err(CodecError::Truncated)
        );
        assert_eq!(
            validate_activate_series_funding_account_metas_v2(
                ACTIVATE_SERIES_FUNDING_ACCOUNT_COUNT_V2
                    .checked_add(1)
                    .unwrap(),
                |_| None,
            ),
            Err(CodecError::TrailingBytes)
        );
    }

    #[test]
    fn activation_payload_is_exact_and_carries_no_caller_amounts() {
        let value = ActivateSeriesFundingIntentV1 {
            series_plan_id: SeriesPlanV5Id::from_bytes([0x51; HASH_BYTES]),
        };
        let mut bytes = [0u8; ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1];
        value.encode(&mut bytes).unwrap();
        assert_eq!(ActivateSeriesFundingIntentV1::decode(&bytes), Ok(value));

        let mut trailing = [0u8; ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1 + 1];
        trailing[..ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1].copy_from_slice(&bytes);
        assert_eq!(
            ActivateSeriesFundingIntentV1::decode(&trailing),
            Err(CodecError::TrailingBytes)
        );
        assert_eq!(
            ActivateSeriesFundingIntentV1::decode(
                &[0u8; ACTIVATE_SERIES_FUNDING_PAYLOAD_BYTES_V1]
            ),
            Err(CodecError::ZeroIdentity)
        );
    }
}
