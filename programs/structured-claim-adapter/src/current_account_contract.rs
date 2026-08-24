//! Exact account and Token-2022 contracts for implemented current actions.
//!
//! Implementation is not release admission. The six entries below let the
//! wrapper, base program, capability manifest, and client tooling share one
//! closed description. The default adapter remains disabled; the named
//! successor development profile admits exactly this set.

use crate::runtime_contract::StructuredClaimActionV1;

/// Canonical label committing the exact current action/count/token-effect set.
pub const STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1: &str =
    "dragons-clutch/structured-claim/current-account-contract/v1/product=root-v3+link-v3+registry-v4+bundle-v7+attachment-v6;a1=36:init-mint+recipe-set;a3=32:mint;a5=32:burn;a6=32:dispose-hoard-surplus;a7=33:burn;a8=34:revoke-mint-authority";
/// SHA-256 identity of [`STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1`].
pub const STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1: [u8; 32] = [
    0x69, 0xb8, 0x2f, 0x81, 0x5b, 0xa0, 0xeb, 0x72, 0xbb, 0x01, 0xcf, 0x8b, 0x0c, 0x1f, 0xf0, 0x7c,
    0x67, 0x17, 0x16, 0xbb, 0xb5, 0x2b, 0x50, 0x51, 0x4e, 0xc3, 0x59, 0x9e, 0x18, 0xd1, 0xac, 0x4a,
];

/// Exact account count for action 1.
pub const STRUCTURED_CREATE_ACCOUNT_COUNT_V1: usize = 36;
/// Exact account count for actions 3 and 5.
pub const STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1: usize = 32;
/// Exact account count for action 6.
pub const STRUCTURED_COMPACTION_ACCOUNT_COUNT_V1: usize = 32;
/// Exact account count for action 7.
pub const STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT_V1: usize = 33;
/// Exact account count for action 8.
pub const STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT_V1: usize = 34;

/// Implemented current action bits. This is not an executable capability mask.
pub const IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1: u16 =
    (1_u16 << StructuredClaimActionV1::CreateDescriptor.tag())
        | (1_u16 << StructuredClaimActionV1::WrapFull.tag())
        | (1_u16 << StructuredClaimActionV1::UnwrapFull.tag())
        | (1_u16 << StructuredClaimActionV1::CompactDonation.tag())
        | (1_u16 << StructuredClaimActionV1::RedeemTerminal.tag())
        | (1_u16 << StructuredClaimActionV1::RetireDescriptor.tag());

/// Token-side effect owned by one exact current action contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CurrentStructuredTokenEffectV1 {
    /// Create the permanent zero-supply wrapper mint under its exact PDA.
    InitializeWrapper = 1,
    /// Mint exact wrapper quantity after full backing enters custody.
    MintWrapper = 2,
    /// Burn exact wrapper quantity before backing leaves custody.
    BurnWrapper = 3,
    /// Optionally transfer only destroyed cash liability Hoard-to-neutral.
    DisposeHoardSurplus = 4,
    /// Revoke authority from the already zero-supply permanent mint.
    RevokeMintAuthority = 5,
}

/// One closed implemented action/account contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentStructuredActionContractV1 {
    /// Canonical family-local action.
    pub action: StructuredClaimActionV1,
    /// Exact outer and base account count.
    pub account_count: u8,
    /// Exact Token-2022-side effect.
    pub token_effect: CurrentStructuredTokenEffectV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CurrentStructuredAccountRoleV1 {
    VaultAuthority,
    Payer,
    SystemProgram,
    RentSysvar,
    Realm,
    CollateralProfile,
    CollateralPolicy,
    CollateralTokenProgram,
    CollateralTokenProgramData,
    MarketBinding,
    MarketRuntime,
    PositionV3,
    ReplayV3,
    SourcePositionV3,
    SourceReplayV3,
    DestinationPositionV3,
    DestinationReplayV3,
    Actor,
    Descriptor,
    WrapperMint,
    WrapperProgram,
    WrapperProgramData,
    BaseProgram,
    BaseProgramData,
    Token2022Program,
    Token2022ProgramData,
    NativeClaimBasis,
    MarketInstance,
    HoardV2,
    ClaimLedgerV3,
    HolderToken,
    MintAuthority,
    CollateralMint,
    HoardToken,
    HoardAuthority,
    NeutralToken,
    StructuredRoot,
    ProductRootV3,
    SeriesLinkV3,
    CompilerBundleV7,
    AttachmentV6,
    WrapperRecipeSetV1,
    FundingTermsV2,
    SeriesRegistryV4,
    RegistryReleaseV2,
    CapabilityProfileV4,
    WrapperReleaseV2,
    BaseReleaseV2,
    TokenReleaseV2,
    ResolutionV5,
    RefundOwner,
    NeutralLamportSink,
}

impl CurrentStructuredAccountRoleV1 {
    /// Stable human-readable role selected only by the semantic owner.
    pub const fn label(self) -> &'static str {
        match self {
            Self::VaultAuthority => "vault-authority",
            Self::Payer => "payer",
            Self::SystemProgram => "system-program",
            Self::RentSysvar => "rent-sysvar",
            Self::Realm => "realm",
            Self::CollateralProfile => "collateral-profile",
            Self::CollateralPolicy => "collateral-policy",
            Self::CollateralTokenProgram => "collateral-token-program",
            Self::CollateralTokenProgramData => "collateral-token-programdata",
            Self::MarketBinding => "market-binding",
            Self::MarketRuntime => "market-runtime",
            Self::PositionV3 => "position-v3",
            Self::ReplayV3 => "replay-v3",
            Self::SourcePositionV3 => "source-position-v3",
            Self::SourceReplayV3 => "source-replay-v3",
            Self::DestinationPositionV3 => "destination-position-v3",
            Self::DestinationReplayV3 => "destination-replay-v3",
            Self::Actor => "actor",
            Self::Descriptor => "structured-descriptor-v2",
            Self::WrapperMint => "wrapper-mint",
            Self::WrapperProgram => "structured-wrapper-program",
            Self::WrapperProgramData => "structured-wrapper-programdata",
            Self::BaseProgram => "clutch-base-program",
            Self::BaseProgramData => "clutch-base-programdata",
            Self::Token2022Program => "token-2022-program",
            Self::Token2022ProgramData => "token-2022-programdata",
            Self::NativeClaimBasis => "native-claim-basis",
            Self::MarketInstance => "market-instance-v2",
            Self::HoardV2 => "hoard-v2",
            Self::ClaimLedgerV3 => "claim-ledger-v3",
            Self::HolderToken => "wrapper-holder-token",
            Self::MintAuthority => "wrapper-mint-authority",
            Self::CollateralMint => "collateral-mint",
            Self::HoardToken => "hoard-token",
            Self::HoardAuthority => "hoard-authority",
            Self::NeutralToken => "realm-neutral-token",
            Self::StructuredRoot => "structured-root",
            Self::ProductRootV3 => "product-market-root-v3",
            Self::SeriesLinkV3 => "series-market-link-v3",
            Self::CompilerBundleV7 => "compiler-bundle-v7",
            Self::AttachmentV6 => "attachment-v6",
            Self::WrapperRecipeSetV1 => "wrapper-recipe-set-v1",
            Self::FundingTermsV2 => "funding-terms-v2",
            Self::SeriesRegistryV4 => "series-registry-v4",
            Self::RegistryReleaseV2 => "registry-release-v2",
            Self::CapabilityProfileV4 => "capability-profile-v4",
            Self::WrapperReleaseV2 => "structured-wrapper-release-v2",
            Self::BaseReleaseV2 => "clutch-base-release-v2",
            Self::TokenReleaseV2 => "token-2022-release-v2",
            Self::ResolutionV5 => "resolution-v5",
            Self::RefundOwner => "rent-refund-owner",
            Self::NeutralLamportSink => "neutral-lamport-sink",
        }
    }
}

/// Exact role and privileges for one current outer-wrapper account position.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CurrentStructuredAccountMetaV1 {
    /// Stable role label selected inside this semantic-owner module.
    pub label: &'static str,
    /// Whether the outer transaction must provide a signature.
    pub signer: bool,
    /// Whether the outer instruction must grant write privilege.
    pub writable: bool,
    /// Whether the account must be executable.
    pub executable: bool,
}

/// Return the semantic-owner account projection for one current action.
/// `product_link_writable` selects the inseparable Product RootV3+LinkV3
/// mutation pair only for action 1 first admission and action 8 final-family
/// retirement; all other callers must pass false.
pub const fn current_structured_account_meta_v1(
    action: StructuredClaimActionV1,
    index: usize,
    product_link_writable: bool,
) -> Option<CurrentStructuredAccountMetaV1> {
    if product_link_writable
        && !matches!(
            action,
            StructuredClaimActionV1::CreateDescriptor
                | StructuredClaimActionV1::RetireDescriptor
        )
    {
        return None;
    }
    let role = match match action {
        StructuredClaimActionV1::CreateDescriptor => create_role(index),
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => full_vector_role(action, index),
        StructuredClaimActionV1::CompactDonation => compaction_role(index),
        StructuredClaimActionV1::RetireDescriptor => retirement_role(index),
    } {
        Some(role) => role,
        None => return None,
    };
    let signer = match action {
        StructuredClaimActionV1::CreateDescriptor => index == 1,
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => index == 12,
        StructuredClaimActionV1::CompactDonation
        | StructuredClaimActionV1::RetireDescriptor => false,
    };
    let writable = match action {
        StructuredClaimActionV1::CreateDescriptor => {
            matches!(index, 1 | 11 | 12 | 13 | 14 | 25)
                || (matches!(index, 26 | 35) && product_link_writable)
        }
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => {
            matches!(index, 8 | 9 | 10 | 11 | 22 | 23 | 24 | 25)
        }
        StructuredClaimActionV1::CompactDonation => {
            matches!(index, 8 | 9 | 19 | 20 | 23 | 25)
        }
        StructuredClaimActionV1::RetireDescriptor => {
            matches!(index, 8 | 9 | 10 | 21 | 23 | 27 | 28)
                || (matches!(index, 24 | 33) && product_link_writable)
        }
    };
    let executable = match action {
        StructuredClaimActionV1::CreateDescriptor => matches!(index, 2 | 7 | 15 | 17 | 19),
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => matches!(index, 4 | 14 | 16 | 18),
        StructuredClaimActionV1::CompactDonation => matches!(index, 4 | 11 | 13 | 15),
        StructuredClaimActionV1::RetireDescriptor => matches!(index, 4 | 11 | 13 | 15 | 32),
    };
    Some(CurrentStructuredAccountMetaV1 {
        label: role.label(),
        signer,
        writable,
        executable,
    })
}

/// The sole intentional outer-account alias: collateral and wrapper token
/// programs may be the same exact Token-2022 release, as may their ProgramData.
pub const fn current_structured_alias_allowed_v1(
    action: StructuredClaimActionV1,
    left: usize,
    right: usize,
) -> bool {
    let (collateral_program, collateral_data, token_program, token_data) = match action {
        StructuredClaimActionV1::CreateDescriptor => (7, 8, 19, 20),
        StructuredClaimActionV1::WrapFull
        | StructuredClaimActionV1::UnwrapFull
        | StructuredClaimActionV1::RedeemTerminal => (4, 5, 18, 19),
        StructuredClaimActionV1::CompactDonation => (4, 5, 15, 16),
        StructuredClaimActionV1::RetireDescriptor => (4, 5, 15, 16),
    };
    (left == collateral_program && right == token_program)
        || (left == collateral_data && right == token_data)
        || (right == collateral_program && left == token_program)
        || (right == collateral_data && left == token_data)
}

const fn create_role(index: usize) -> Option<CurrentStructuredAccountRoleV1> {
    use CurrentStructuredAccountRoleV1 as R;
    Some(match index {
        0 => R::VaultAuthority, 1 => R::Payer, 2 => R::SystemProgram, 3 => R::RentSysvar,
        4 => R::Realm, 5 => R::CollateralProfile, 6 => R::CollateralPolicy,
        7 => R::CollateralTokenProgram, 8 => R::CollateralTokenProgramData,
        9 => R::MarketBinding, 10 => R::MarketRuntime, 11 => R::PositionV3,
        12 => R::ReplayV3, 13 => R::Descriptor, 14 => R::WrapperMint,
        15 => R::WrapperProgram, 16 => R::WrapperProgramData, 17 => R::BaseProgram,
        18 => R::BaseProgramData, 19 => R::Token2022Program, 20 => R::Token2022ProgramData,
        21 => R::NativeClaimBasis, 22 => R::MarketInstance, 23 => R::HoardV2,
        24 => R::ClaimLedgerV3, 25 => R::StructuredRoot, 26 => R::SeriesLinkV3,
        27 => R::CompilerBundleV7, 28 => R::AttachmentV6, 29 => R::WrapperRecipeSetV1,
        30 => R::SeriesRegistryV4, 31 => R::RegistryReleaseV2, 32 => R::CapabilityProfileV4,
        33 => R::WrapperReleaseV2, 34 => R::TokenReleaseV2, 35 => R::ProductRootV3,
        _ => return None,
    })
}

const fn full_vector_role(
    action: StructuredClaimActionV1,
    index: usize,
) -> Option<CurrentStructuredAccountRoleV1> {
    use CurrentStructuredAccountRoleV1 as R;
    Some(match index {
        0 => R::VaultAuthority, 1 => R::Realm, 2 => R::CollateralProfile,
        3 => R::CollateralPolicy, 4 => R::CollateralTokenProgram,
        5 => R::CollateralTokenProgramData, 6 => R::MarketBinding, 7 => R::MarketRuntime,
        8 => R::SourcePositionV3, 9 => R::SourceReplayV3,
        10 => R::DestinationPositionV3, 11 => R::DestinationReplayV3, 12 => R::Actor,
        13 => R::Descriptor, 14 => R::WrapperProgram, 15 => R::WrapperProgramData,
        16 => R::BaseProgram, 17 => R::BaseProgramData, 18 => R::Token2022Program,
        19 => R::Token2022ProgramData, 20 => R::NativeClaimBasis,
        21 => R::MarketInstance, 22 => R::HoardV2, 23 => R::ClaimLedgerV3,
        24 => R::WrapperMint, 25 => R::HolderToken, 26 => R::MintAuthority,
        27 => R::CollateralMint, 28 => R::HoardToken, 29 => R::WrapperReleaseV2,
        30 => R::BaseReleaseV2, 31 => R::TokenReleaseV2,
        32 if matches!(action, StructuredClaimActionV1::RedeemTerminal) => R::ResolutionV5,
        _ => return None,
    })
}

const fn compaction_role(index: usize) -> Option<CurrentStructuredAccountRoleV1> {
    use CurrentStructuredAccountRoleV1 as R;
    Some(match index {
        0 => R::VaultAuthority, 1 => R::Realm, 2 => R::CollateralProfile,
        3 => R::CollateralPolicy, 4 => R::CollateralTokenProgram,
        5 => R::CollateralTokenProgramData, 6 => R::MarketBinding, 7 => R::MarketRuntime,
        8 => R::PositionV3, 9 => R::ReplayV3, 10 => R::Descriptor,
        11 => R::WrapperProgram, 12 => R::WrapperProgramData, 13 => R::BaseProgram,
        14 => R::BaseProgramData, 15 => R::Token2022Program, 16 => R::Token2022ProgramData,
        17 => R::NativeClaimBasis, 18 => R::MarketInstance, 19 => R::HoardV2,
        20 => R::ClaimLedgerV3, 21 => R::WrapperMint, 22 => R::CollateralMint,
        23 => R::HoardToken, 24 => R::HoardAuthority, 25 => R::NeutralToken,
        26 => R::StructuredRoot, 27 => R::SeriesLinkV3, 28 => R::FundingTermsV2,
        29 => R::WrapperReleaseV2, 30 => R::BaseReleaseV2, 31 => R::TokenReleaseV2,
        _ => return None,
    })
}

const fn retirement_role(index: usize) -> Option<CurrentStructuredAccountRoleV1> {
    use CurrentStructuredAccountRoleV1 as R;
    Some(match index {
        0 => R::VaultAuthority, 1 => R::Realm, 2 => R::CollateralProfile,
        3 => R::CollateralPolicy, 4 => R::CollateralTokenProgram,
        5 => R::CollateralTokenProgramData, 6 => R::MarketBinding, 7 => R::MarketRuntime,
        8 => R::PositionV3, 9 => R::ReplayV3, 10 => R::Descriptor,
        11 => R::WrapperProgram, 12 => R::WrapperProgramData, 13 => R::BaseProgram,
        14 => R::BaseProgramData, 15 => R::Token2022Program, 16 => R::Token2022ProgramData,
        17 => R::NativeClaimBasis, 18 => R::MarketInstance, 19 => R::HoardV2,
        20 => R::ClaimLedgerV3, 21 => R::WrapperMint, 22 => R::MintAuthority,
        23 => R::StructuredRoot, 24 => R::SeriesLinkV3, 25 => R::CompilerBundleV7,
        26 => R::AttachmentV6, 27 => R::RefundOwner, 28 => R::NeutralLamportSink,
        29 => R::WrapperReleaseV2, 30 => R::BaseReleaseV2, 31 => R::TokenReleaseV2,
        32 => R::SystemProgram, 33 => R::ProductRootV3, _ => return None,
    })
}

/// Complete implemented current action set. Withdrawn canonical actions 2 and
/// 4 intentionally have no current account contract and cannot be admitted.
pub const CURRENT_STRUCTURED_ACTION_CONTRACTS_V1: [CurrentStructuredActionContractV1; 6] = [
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::CreateDescriptor,
        account_count: STRUCTURED_CREATE_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::InitializeWrapper,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::WrapFull,
        account_count: STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::MintWrapper,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::UnwrapFull,
        account_count: STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::BurnWrapper,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::CompactDonation,
        account_count: STRUCTURED_COMPACTION_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::DisposeHoardSurplus,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::RedeemTerminal,
        account_count: STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::BurnWrapper,
    },
    CurrentStructuredActionContractV1 {
        action: StructuredClaimActionV1::RetireDescriptor,
        account_count: STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT_V1 as u8,
        token_effect: CurrentStructuredTokenEffectV1::RevokeMintAuthority,
    },
];

/// Return the sole implemented current contract for an action.
pub const fn current_structured_action_contract_v1(
    action: StructuredClaimActionV1,
) -> Option<CurrentStructuredActionContractV1> {
    match action {
        StructuredClaimActionV1::CreateDescriptor => {
            Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[0])
        }
        StructuredClaimActionV1::WrapFull => Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[1]),
        StructuredClaimActionV1::UnwrapFull => Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[2]),
        StructuredClaimActionV1::CompactDonation => {
            Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[3])
        }
        StructuredClaimActionV1::RedeemTerminal => {
            Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[4])
        }
        StructuredClaimActionV1::RetireDescriptor => {
            Some(CURRENT_STRUCTURED_ACTION_CONTRACTS_V1[5])
        }
    }
}

const _: () = assert!(STRUCTURED_CREATE_ACCOUNT_COUNT_V1 <= u8::MAX as usize);
const _: () = assert!(STRUCTURED_FULL_VECTOR_ACCOUNT_COUNT_V1 <= u8::MAX as usize);
const _: () = assert!(STRUCTURED_COMPACTION_ACCOUNT_COUNT_V1 <= u8::MAX as usize);
const _: () = assert!(STRUCTURED_TERMINAL_REDEMPTION_ACCOUNT_COUNT_V1 <= u8::MAX as usize);
const _: () = assert!(STRUCTURED_DESCRIPTOR_RETIREMENT_ACCOUNT_COUNT_V1 <= u8::MAX as usize);

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn implemented_contracts_are_exact_and_withdrawn_routes_have_none() {
        let mut observed_mask = 0_u16;
        for contract in CURRENT_STRUCTURED_ACTION_CONTRACTS_V1 {
            observed_mask |= 1_u16 << contract.action.tag();
            assert_eq!(
                current_structured_action_contract_v1(contract.action),
                Some(contract)
            );
        }
        assert_eq!(observed_mask, IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1);
        assert_eq!(
            <[u8; 32]>::from(
                Sha256::digest(STRUCTURED_CURRENT_ACCOUNT_CONTRACT_LABEL_V1.as_bytes())
            ),
            STRUCTURED_CURRENT_ACCOUNT_CONTRACT_ID_V1,
        );
        for withdrawn_tag in [2_u8, 4_u8] {
            assert_eq!(
                StructuredClaimActionV1::from_tag(withdrawn_tag),
                Err(crate::runtime_contract::Error::UnknownAction),
            );
            assert_eq!(
                IMPLEMENTED_CURRENT_STRUCTURED_ACTION_MASK_V1 & (1_u16 << withdrawn_tag),
                0,
            );
        }
    }

    #[test]
    fn role_projection_pins_counts_privileges_and_dynamic_product_link() {
        for contract in CURRENT_STRUCTURED_ACTION_CONTRACTS_V1 {
            let count = usize::from(contract.account_count);
            for index in 0..count {
                assert!(current_structured_account_meta_v1(contract.action, index, false).is_some());
            }
            assert!(current_structured_account_meta_v1(contract.action, count, false).is_none());
        }
        let create_link = current_structured_account_meta_v1(
            StructuredClaimActionV1::CreateDescriptor,
            26,
            true,
        )
        .unwrap();
        let terminal_link = current_structured_account_meta_v1(
            StructuredClaimActionV1::RetireDescriptor,
            24,
            true,
        )
        .unwrap();
        let create_root = current_structured_account_meta_v1(
            StructuredClaimActionV1::CreateDescriptor,
            35,
            true,
        )
        .unwrap();
        let terminal_root = current_structured_account_meta_v1(
            StructuredClaimActionV1::RetireDescriptor,
            33,
            true,
        )
        .unwrap();
        assert!(create_link.writable);
        assert!(terminal_link.writable);
        assert!(create_root.writable);
        assert!(terminal_root.writable);
        assert!(!current_structured_account_meta_v1(
            StructuredClaimActionV1::CreateDescriptor,
            35,
            false,
        )
        .unwrap()
        .writable);
        assert!(!current_structured_account_meta_v1(
            StructuredClaimActionV1::RetireDescriptor,
            33,
            false,
        )
        .unwrap()
        .writable);
        assert!(current_structured_account_meta_v1(
            StructuredClaimActionV1::WrapFull,
            0,
            true,
        )
        .is_none());
        assert!(current_structured_account_meta_v1(
            StructuredClaimActionV1::CompactDonation,
            0,
            true,
        )
        .is_none());
    }

    #[test]
    fn only_correlated_collateral_and_wrapper_token_release_roles_may_alias() {
        for action in [
            StructuredClaimActionV1::CreateDescriptor,
            StructuredClaimActionV1::WrapFull,
            StructuredClaimActionV1::UnwrapFull,
            StructuredClaimActionV1::CompactDonation,
            StructuredClaimActionV1::RedeemTerminal,
            StructuredClaimActionV1::RetireDescriptor,
        ] {
            let (collateral_program, collateral_data, token_program, token_data) = match action {
                StructuredClaimActionV1::CreateDescriptor => (7, 8, 19, 20),
                StructuredClaimActionV1::WrapFull
                | StructuredClaimActionV1::UnwrapFull
                | StructuredClaimActionV1::RedeemTerminal => (4, 5, 18, 19),
                StructuredClaimActionV1::CompactDonation
                | StructuredClaimActionV1::RetireDescriptor => (4, 5, 15, 16),
            };
            assert!(current_structured_alias_allowed_v1(
                action,
                collateral_program,
                token_program,
            ));
            assert!(current_structured_alias_allowed_v1(
                action,
                token_data,
                collateral_data,
            ));
            assert!(!current_structured_alias_allowed_v1(action, 0, 1));
            assert!(!current_structured_alias_allowed_v1(
                action,
                collateral_program,
                token_data,
            ));
        }
    }
}
