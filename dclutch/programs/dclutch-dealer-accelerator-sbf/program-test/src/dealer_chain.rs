//! Canonical full-chain fixture for Dealer selector 9.
//!
//! This fixture starts from the shared Product graph generator used by the
//! executable Direct campaign, then rebuilds every Market, Claims, Dealer,
//! capability, strategy, and physical-account fact that is owned by selector
//! 9.  Its output is one ordinary Trading Hot instruction; the real Trading
//! ELF must authenticate the frame and CPI the real Dealer accelerator ELF.
#![expect(
    dead_code,
    unused_imports,
    reason = "staged for the joined dealer chain campaign"
)]

use std::vec::Vec;

use dclutch_account_profile_contract::v2::AccountProfileV2;
use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
    MAX_DEPENDENCIES_PER_CAPABILITY,
};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
        HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3, HOT_CONFIG_STAGING_ACCOUNT_V3,
        HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
        HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
        HOT_EFFECT_RAW_ACCOUNT_V3, HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_EXECUTION_ENVELOPE_BYTES_V3,
        HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
        HOT_LIFECYCLE_RAW_ACCOUNT_V3, HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
        HOT_LINKED_BASIS_RAW_ACCOUNT_V3, HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
        HOT_MANIFEST_RAW_ACCOUNT_V3, HOT_MANIFEST_STAGING_ACCOUNT_V3, HOT_MARKET_ACCOUNT_V3,
        HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PORTFOLIO_STAGING_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_PRODUCT_STAGING_ACCOUNT_V3, HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
        HOT_PROGRAM_SET_STAGING_ACCOUNT_V3, HOT_REGISTRY_PROGRAM_ACCOUNT_V3,
        HOT_RENT_SYSVAR_ACCOUNT_V3, HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
        HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3, HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
        HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3, HOT_ROOT_ACCOUNT_V3, HOT_STRATEGY_RAW_ACCOUNT_V3,
        HOT_STRATEGY_STAGING_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
        HOT_TRADING_PROGRAMDATA_ACCOUNT_V3, HOT_TRANSITION_RAW_ACCOUNT_V3,
        HOT_TRANSITION_STAGING_ACCOUNT_V3, HotExecutionEnvelopeV3,
    },
    set_v1::{CapabilityProgramSetV1, CapabilityProgramSetV1 as _},
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
    v3::{
        CAPABILITY_PROGRAM_V3_BYTES, CapabilityProgramV3,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V3,
    },
    v4::{
        CAPABILITY_PROGRAM_V4_BYTES, CapabilityProgramV4,
        SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_RELEASE_ID_V4,
    },
};
use dclutch_claims_svm::{
    frame_spec_v1::{ClaimsFrameRoleV1, SignedDeltaFrameSpecV3},
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_MARKET_SEED_V2,
        LIABILITY_BASIS_POSITION_HEADER_BYTES_V2, LiabilityBasisMarketInputV2,
        LiabilityBasisMarketViewV2, LiabilityBasisPositionInputV2, LiabilityBasisPositionViewV2,
        encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_core_contract::{ContentId, MarketRoot};
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CallerRoleV1, CompartmentV1, ContextV1,
    CustodyAuthoritySeedsV1, CustodyFrameRoleV1, CustodyFrameSpecV1, CustodyReplayV1,
    CustodyRequestV1, CustodyVaultSeedsV1, DelegatedCustodyRequestV2, OperationV1,
};
use dclutch_dealer_codec::{
    config_v4::DealerConfigV4, root_tail::ROOT_TAIL_BYTES, scenario::ClaimsInventoryObservation,
};
use dclutch_direct_hot_program_test_support::{
    DirectHotDeploymentWidthsV5,
    chain::DirectHotInstallAccountV5,
    fixture::{DirectHotChainInputV5, build_direct_hot_chain_fixture_v5},
};
use dclutch_execution_strategy_contract::v2::{
    ACCELERATOR_ACK_SCHEMA_ID_V2, ACCELERATOR_REQUEST_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_ADMISSION_SCHEMA_ID_V2, EXECUTION_STRATEGY_CERTIFICATE_SCHEMA_ID_V2,
    EXECUTION_STRATEGY_PROGRAM_SCHEMA_ID_V2, ExecutionStrategyAdmissionV2,
    ExecutionStrategyCertificateV2, ExecutionStrategyProgramV2, StrategyDispositionV2,
};
use dclutch_market_core_codec::{
    CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
};
use dclutch_product_runtime_v2_admission::ProductRecordV2;
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
use dclutch_registry_contract::{ArtifactReleaseV1, ArtifactUpgradePolicyV1};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionRoleV1, ProgramIdentityV1,
};
use dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID;
use dclutch_trading_sbf::{
    admitted_composition_v3::admitted_caller_authority_count_v3,
    dealer::{
        DEALER_KIND_PREIMAGE_V2, DEALER_ROOT_SCHEMA_PREIMAGE_V2,
        v3_composer::{ScenarioCollateralFrameV3, ScenarioComposerContextV3},
        v3_multi_lp::MultiLpCustodyRequestV3,
        v3_obligation::{
            DEALER_OBLIGATION_PDA_DOMAIN_V3, DealerObligationProjectionV3,
            ObligationAccountObservationV3, ObligationOpenInputV3, obligation_account_bytes_v3,
            prepare_obligation_open_v3,
        },
        v3_release::{
            DEALER_GLOBAL_SELECTOR_COUNT_V3, DEALER_SCENARIO_TRADE_REQUEST_SCHEMA_PREIMAGE_V3,
            dealer_request_schema_v3,
        },
        v3_trade::{
            ScenarioTradeChainProjectionV3, ScenarioTradeDirectionV3, ScenarioTradeIntentV3,
            build_scenario_trade_request_v3, scenario_trade_max_request_bytes_v3,
        },
        v3_trade_artifacts::{
            DEALER_SCENARIO_COMMON_IDENTITY_COUNT_V4, DEALER_SCENARIO_COMMON_SCALAR_COUNT_V4,
            DEALER_SCENARIO_REQUEST_PROFILE_BYTES_V4, DEALER_SCENARIO_TRANSITION_BYTES_V4,
            dealer_scenario_base_effect_program_bytes_v4, dealer_scenario_effect_program_bytes_v4,
            encode_dealer_scenario_base_effect_program_v4,
            encode_dealer_scenario_effect_program_v4, encode_dealer_scenario_request_profile_v4,
            encode_dealer_scenario_transition_v4, project_dealer_scenario_hot_registers_v4,
        },
        v3_trade_profile::{
            DEALER_SCENARIO_ACCOUNT_PROFILE_BYTES_V4, DealerScenarioAccountProfileInputV4,
            dealer_scenario_logical_frame_v4, encode_dealer_scenario_account_profile_v4_atomic,
        },
        v4_scenario_release::{
            DEALER_GLOBAL_PROGRAM_SET_BYTES_V4, DEALER_SCENARIO_EMPTY_LIFECYCLE_BYTES_V5,
            DealerDescriptorRecordV4, DealerScenarioFinalizedArtifactsV4,
            encode_dealer_global_program_set_v4, encode_dealer_scenario_empty_lifecycle_v5,
            finalize_dealer_scenario_descriptor_v4,
        },
    },
};
use solana_account::Account;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

const WIDTH: usize = 3;
const WIDTH_U32: u32 = 3;
const GENERATION: u64 = 9;
const CURRENT_SLOT: u64 = 1;
const CLAIMS_REVISION: u64 = 8;
const DEALER_REVISION: u64 = 9;
const COUNTERPARTY_REVISION: u64 = 10;
const CUSTODY_REVISION: u64 = 7;

/// One canonical Dealer chain fixture before Registry continuation wrapping.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DealerScenarioChainFixtureV4 {
    /// Trading Hot instruction which must invoke the accelerator itself.
    pub hot_instruction: Instruction,
    /// All fixed, strategy, and packed runtime account bodies.
    pub accounts: Vec<DirectHotInstallAccountV5>,
    /// Accounts installed externally by the release-waist harness.
    pub externally_installed_keys: Vec<Pubkey>,
    /// Mutable accounts whose state must roll back on any late refusal.
    pub rollback_snapshot_keys: Vec<Pubkey>,
    /// Canonical Dealer child root.
    pub root: Pubkey,
    /// Canonical Trading-owned obligation PDA.
    pub obligation: Pubkey,
}

/// Stable fixture-construction refusal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DealerScenarioChainErrorV4 {
    /// A semantic-owner encoder rejected the requested fixture.
    Encoding,
    /// Finalized artifact generation or selection did not join exactly.
    Artifacts,
    /// Runtime account packing differed from Profile13.
    Profile,
    /// Checked integer or byte arithmetic failed.
    Arithmetic,
}

#[derive(Clone, Debug)]
struct Finalized {
    raw: Pubkey,
    staging: Pubkey,
    bytes: Vec<u8>,
    digest: [u8; 32],
    schema: [u8; 32],
}

#[derive(Clone, Debug)]
struct ChainAccount {
    key: Pubkey,
    account: Account,
    meta: AccountMeta,
    snapshot: bool,
}

impl ChainAccount {
    fn install(self) -> DirectHotInstallAccountV5 {
        DirectHotInstallAccountV5 {
            key: self.key,
            account: self.account,
            snapshot_for_rollback: self.snapshot,
        }
    }
}
