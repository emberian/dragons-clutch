//! Canonical logical-to-physical account routing for ordinary Direct V3.
//!
//! The public trade boundary names semantic accounts. It never accepts the
//! fixed, strategy, or runtime arrays which are serialized into the Hot
//! instruction. This module is the sole host-side owner of that projection:
//! child FrameSpecs own their order, AccountProfile owns aliases, physical
//! packing, privileges, and data geometry, and the common Hot ABI owns its
//! fixed prefix.

use crate::{
    Observation, ObservedAccount,
    direct_inline_v3::{
        AuthenticatedDirectHotChainV4, CheckedHotOuterReleaseV3,
        DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1, DirectInlineHotReportV3, DirectInlineHotStateV3,
        DirectInlineHotTransactionPlanV3, ObservedAccountMetaV3, SignedDirectIntentV3,
        authenticate_direct_hot_chain_v4, compile_direct_inline_request_v3,
        validate_direct_hot_instruction_sequence_v4,
    },
    observation::{FinalizedRecordProof, authenticate_finalized_record, decode_rent},
};
use dclutch_vm::account_profile::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};
use dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1;
use dclutch_market::capability_program::hot_v3::{
    HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
    HOT_ACTIVATION_CACHE_ACCOUNT_V3, HOT_CAPABILITY_SEAL_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3,
    HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_CORE_PROGRAMDATA_ACCOUNT_V3,
    HOT_DESCRIPTOR_RAW_ACCOUNT_V3, HOT_DESCRIPTOR_STAGING_ACCOUNT_V3, HOT_EFFECT_RAW_ACCOUNT_V3,
    HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
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
    HOT_TRANSITION_STAGING_ACCOUNT_V3,
};
use dclutch_market::capability_program::v4::{
    CapabilityProgramV4, SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_vm::capability_seal::{
    CAPABILITY_SEAL_BYTES_V1, CapabilitySealKeyV1, CapabilitySealRequestV1,
    SealedDescriptorClosureV1, SealedRecordRowV1, SealedRoleV1,
};
use dclutch_claims::frame_spec_v1::{
    ClaimsFrameRoleV1, SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1, SparseNativeTransferFrameSpecV1,
};
use dclutch_claims::{
    liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_SEED_V2, LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2,
    },
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_custody::{
    CallerRoleV1, CustodyAuthoritySeedsV1, CustodyFrameRoleV1, CustodyFrameSpecV1,
    CustodyReplaySeedsV1, CustodyReplayV1, OperationV1, TRANSFER_ACCOUNT_COUNT_V1,
};
use dclutch_trading::{
    direct_finalization_v3::{
        DIRECT_INLINE_POSTSTATE_COUNT_V3, DirectFinalizationErrorV3, DirectInlineAccountPrestateV3,
        DirectInlineAccountPrestatesV3, DirectInlineFinalizationInputV3,
        DirectInlineFinalizationProgramsV3, DirectInlineFinalizationV3,
        DirectInlinePoststateCommitmentV3, DirectInlinePoststateRoleV3, HotExecutionAckInputV3,
        HotExecutionArtifactFactsV3, prepare_direct_inline_finalization_v3,
        project_direct_inline_account_poststate_v3,
    },
    execution_v3::{DirectExecutionRequestV3, DirectInlineOrdinaryRequestV3},
    inline_candidate_v2::{
        DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3, DirectExternalCollateralV2,
        DirectExternalDebitV2, DirectInlineCandidateContextV2, DirectInlineCandidateErrorV2,
        DirectInlineCollateralFrameV2, prepare_and_verify_inline_effect_partition_v2,
    },
    ordinary_effect_artifacts_v3::{
        DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3, DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3,
        DIRECT_INLINE_FEE_CONTINUATION_ACCOUNT_START_V3, DIRECT_INLINE_FEE_SOLE_ACCOUNT_START_V3,
        DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
        DIRECT_INLINE_SELLER_INTERMEDIATE_ACCOUNT_START_V3,
        DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3,
    },
    ordinary_route_projection_v3::{
        DirectInlineOrdinaryChildProjectionErrorV3,
        project_direct_inline_ordinary_child_requests_v3,
    },
    ordinary_v3::DirectOrdinaryAuthenticatedContextV3,
    state_artifacts_v3::{
        DIRECT_BUYER_MAKER_ACCOUNT_V3, DIRECT_LIFECYCLE_RENT_CREDIT_ACCOUNT_V3,
        DIRECT_MAKER_PAYER_ACCOUNT_V3, DIRECT_MAKER_PAYER_ROUTE_ALIAS_ACCOUNT_V3,
        DIRECT_SELLER_MAKER_ACCOUNT_V3,
    },
    successor::{
        AuthenticatedCompactIntentV2, DIRECT_MAKER_REPLAY_BYTES_V1, DirectCoordinatesV1,
        DirectExecutionConfigV1, DirectRootPhaseV1, DirectRootStateV1, InlineExecutionV2,
        InlineOrdinaryInputV2, InlineParticipantV2, MakerReplayFirstUseV1,
        MakerReplayObservationV1, MakerReplayRootV1, MakerReplaySeedsV1, MakerReplayVacancyV1,
    },
};
use dclutch_market::execution_strategy::v2::{ExecutionStrategyProgramV2, StrategyDispositionV2};
use dclutch_market::CoreState;
use dclutch_market::realm::{REALM_SCHEMA_RELEASE_ID_V1, RealmV1};
use dclutch_registry::ActivatedExecutionReleaseSetViewV1;
use dclutch_registry::release_set::{ArtifactReleaseIdV1, CallerAuthoritySeedsV1, ExecutionRoleV1};
use dclutch_release_tool::{CHECKED_MULTIPROGRAM_BYTES_V1, CheckedExecutionReleaseSetV1};
use dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_custody::token_svm::{AccountState, COption, Mint, TokenAccount};
use solana_address_lookup_table_interface::{
    instruction::{create_lookup_table, extend_lookup_table, freeze_lookup_table},
    program as lookup_table_program,
    state::AddressLookupTable,
};
use solana_hash::Hash;
use solana_program::{
    hash::hash,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
};
use solana_sdk_ids::bpf_loader_upgradeable;

use core::fmt;

use crate::versioned::compile_v0_message;

/// One finalized Registry record and its vacant staging coordinate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FinalizedRecordRouteV3 {
    /// Finalized raw record.
    pub raw: ObservedAccount,
    /// Vacant staging cursor paired with `raw`.
    pub staging: ObservedAccount,
}

/// Semantic accounts in the common Hot fixed prefix.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectHotFixedRouteV3 {
    /// Core Market.
    pub market: ObservedAccount,
    /// Mutable Direct capability root.
    pub root: ObservedAccount,
    /// Selected CapabilityManifest record pair.
    pub manifest: FinalizedRecordRouteV3,
    /// Selected CapabilityProgramSet record pair.
    pub program_set: FinalizedRecordRouteV3,
    /// Selected ordinary Direct descriptor record pair.
    pub descriptor: FinalizedRecordRouteV3,
    /// Selected Direct config record pair.
    pub config: FinalizedRecordRouteV3,
    /// Selected AccountProfile record pair.
    pub account_profile: FinalizedRecordRouteV3,
    /// Selected RequestProfile record pair.
    pub request_profile: FinalizedRecordRouteV3,
    /// Selected Transition record pair.
    pub transition: FinalizedRecordRouteV3,
    /// Selected Effect record pair.
    pub effect: FinalizedRecordRouteV3,
    /// Selected lifecycle-policy record pair.
    pub lifecycle: FinalizedRecordRouteV3,
    /// Selected execution-strategy record pair.
    pub strategy: FinalizedRecordRouteV3,
    /// Current Registry activation cache.
    pub activation_cache: ObservedAccount,
    /// Current Core program.
    pub core_program: ObservedAccount,
    /// Current Core ProgramData.
    pub core_programdata: ObservedAccount,
    /// Current Trading program.
    pub trading_program: ObservedAccount,
    /// Current Trading ProgramData.
    pub trading_programdata: ObservedAccount,
    /// Registry program.
    pub registry_program: ObservedAccount,
    /// Rent sysvar.
    pub rent_sysvar: ObservedAccount,
    /// Instructions sysvar.
    pub instructions_sysvar: ObservedAccount,
    /// Product graph-root record pair.
    pub product: FinalizedRecordRouteV3,
    /// Product result-domain record pair.
    pub result_domain: FinalizedRecordRouteV3,
    /// Product portfolio record pair.
    pub portfolio: FinalizedRecordRouteV3,
    /// Product-linked liability-basis record pair.
    pub linked_basis: FinalizedRecordRouteV3,
    /// Trading validated-artifact seal.
    pub capability_seal: ObservedAccount,
}

/// Named accounts in the Claims sparse-native-transfer child frame.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectClaimsRouteV3 {
    /// Trading child caller-authority PDA derived from the exact Claims request.
    pub caller_authority: ObservedAccount,
    /// Claims aggregate.
    pub aggregate: ObservedAccount,
    /// Claims program.
    pub claims_program: ObservedAccount,
    /// Claims ProgramData.
    pub claims_programdata: ObservedAccount,
    /// Seller Claims Position.
    pub seller_position: ObservedAccount,
    /// Buyer Claims Position.
    pub buyer_position: ObservedAccount,
}

/// Accounts shared by every ordinary Direct Custody transfer route.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectCustodyRouteV3 {
    /// Four route-specific Trading caller-authority PDAs in Effect route order.
    pub caller_authorities: [ObservedAccount; 4],
    /// Finalized Realm record pair.
    pub realm: FinalizedRecordRouteV3,
    /// Trading-role Custody replay rooted at the buyer maker replay.
    pub replay: ObservedAccount,
    /// Realm collateral mint.
    pub mint: ObservedAccount,
    /// Buyer source token account named by the buy intent.
    pub buyer_token: ObservedAccount,
    /// Seller destination token account named by the sell intent.
    pub seller_token: ObservedAccount,
    /// Fee destination token account owned by the configured fee recipient.
    pub fee_token: ObservedAccount,
    /// Canonical Custody transfer authority.
    pub custody_authority: ObservedAccount,
    /// Realm-selected token program.
    pub token_program: ObservedAccount,
    /// Current Custody program.
    pub custody_program: ObservedAccount,
    /// Current Custody ProgramData, observed beside the routed program account.
    /// It is release evidence and is deliberately not appended to the Hot
    /// AccountProfile, whose child frame only routes the executable program.
    pub custody_programdata: ObservedAccount,
}

/// Named account closure for one ordinary Direct trade.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineOrdinaryRouteV3 {
    /// Common Hot prefix.
    pub fixed: DirectHotFixedRouteV3,
    /// Seller maker replay root.
    pub seller_maker: ObservedAccount,
    /// Sole transaction/rent payer, repeated only through an authenticated alias.
    pub payer: ObservedAccount,
    /// Market lifecycle RentCredit.
    pub lifecycle_rent_credit: ObservedAccount,
    /// Buyer maker replay root.
    pub buyer_maker: ObservedAccount,
    /// Rent program owning the lifecycle credit.
    pub rent_program: ObservedAccount,
    /// System program.
    pub system_program: ObservedAccount,
    /// Claims route accounts.
    pub claims: DirectClaimsRouteV3,
    /// Custody route accounts.
    pub custody: DirectCustodyRouteV3,
}

/// Canonically assembled physical accounts for `DirectInlineHotStateV3`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlinePhysicalRouteV3 {
    /// Exact common Hot prefix.
    pub fixed_accounts: Vec<ObservedAccountMetaV3>,
    /// Exact interpreted strategy suffix. Ordinary Direct requires this empty.
    pub strategy_accounts: Vec<ObservedAccountMetaV3>,
    /// AccountProfile-packed physical runtime representatives.
    pub runtime_accounts: Vec<ObservedAccountMetaV3>,
    /// Semantic-owner address class for every fixed coordinate.
    pub fixed_classes: Vec<DirectInlineAddressClassV3>,
    /// Semantic-owner address class for every physical runtime representative.
    pub runtime_classes: Vec<DirectInlineAddressClassV3>,
    /// One finalized observation shared by every account.
    pub observation: Observation,
}

/// Explicit placement class for one Direct trade account.
///
/// This classification is assigned from the account's semantic role before
/// alias packing. It is never inferred from a public key, signer bit, or
/// executable bit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineAddressClassV3 {
    /// Immutable market/release coordinates eligible for the frozen table.
    LookupStable,
    /// Explicit transaction signer, always static-inline.
    InlineSigner,
    /// Actual top-level message program ID, always static-inline.
    InlineProgram,
    /// Request, nonce, or child-request-derived coordinate. It may only enter
    /// the one request-specific frozen table built after both signatures and
    /// every selected artifact are fixed; it is never part of a shared table.
    InlineRequestBound,
}

/// Canonical message-level key/meta/class closure for one assembled route.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineAddressPlacementV3 {
    /// Exact account address.
    pub address: Pubkey,
    /// Unioned signer privilege across every semantic coordinate.
    pub is_signer: bool,
    /// Unioned writable privilege across every semantic coordinate.
    pub is_writable: bool,
    /// Explicit semantic-owner placement class.
    pub class: DirectInlineAddressClassV3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClassifiedAccountV3 {
    account: ObservedAccount,
    class: DirectInlineAddressClassV3,
}

/// Stable refusal from semantic Direct route assembly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineRouteErrorV3 {
    /// An input account came from another observation or was repeated where distinctness is required.
    Observation,
    /// The fixed Hot prefix could not be assembled exactly.
    FixedFrame,
    /// A child FrameSpec refused its semantic role.
    ChildFrame,
    /// AccountProfile width, alias, privilege, or physical geometry refused.
    Profile,
    /// The canonical write-once capability seal closure refused.
    Seal,
    /// The exact ordinary Direct finalizer or complete poststate projection
    /// refused, at the named clause.
    Finalization(DirectInlineFinalizationRefusalV3),
    /// `dclutch_operator` refused; the cause is its own.
    DirectInline(crate::direct_inline_v3::Error),
    /// `dclutch_trading` refused; the cause is its own.
    DirectExecutionRequest(dclutch_trading::execution_v3::DirectExecutionRequestErrorV3),
    /// `dclutch_trading` refused; the cause is its own.
    DirectInlineOrdinaryChildProjection(dclutch_trading::ordinary_route_projection_v3::DirectInlineOrdinaryChildProjectionErrorV3),
    /// `dclutch_registry` refused; the cause is its own.
    Registry(dclutch_registry::Error),
    /// `dclutch_registry::release_set` refused; the cause is its own.
    ReleaseSet(dclutch_registry::release_set::Error),
    /// `dclutch_market::capability_program` refused; the cause is its own.
    CapabilityProgram(dclutch_market::capability_program::Error),
    /// `dclutch_vm::capability_seal` refused; the cause is its own.
    CapabilitySeal(dclutch_vm::capability_seal::Error),
    /// `dclutch_operator` refused; the cause is its own.
    ObservationError(crate::observation::ObservationError),
    /// `dclutch_trading` refused; the cause is its own.
    Successor(dclutch_trading::successor::SuccessorError),
    /// `dclutch_market` refused; the cause is its own.
    MarketCore(dclutch_market::Error),
    /// `dclutch_market::realm` refused; the cause is its own.
    Realm(dclutch_market::realm::Error),
    /// `dclutch_market::rent` refused; the cause is its own.
    LifecycleRent(dclutch_market::rent::lifecycle_v2::LifecycleRentErrorV2),
    /// `dclutch_claims` refused; the cause is its own.
    LiabilityBasisState(dclutch_claims::liability_basis_state_v2::LiabilityBasisStateErrorV2),
    /// `dclutch_custody` refused; the cause is its own.
    Custody(dclutch_custody::Error),
    /// `dclutch_custody::token_svm` refused; the cause is its own.
    Token(dclutch_custody::token_svm::Error),
    /// `dclutch_claims` refused; the cause is its own.
    ProtocolPosition(dclutch_claims::protocol_position_v2::ProtocolPositionErrorV2),
    /// `dclutch_vm::account_profile` refused; the cause is its own.
    AccountProfile(dclutch_vm::account_profile::v2::Error),
    /// `dclutch_claims` refused; the cause is its own.
    FrameSpec(dclutch_claims::frame_spec_v1::FrameSpecErrorV1),
    /// `dclutch_custody` refused; the cause is its own.
    CustodyFrameSpec(dclutch_custody::CustodyFrameSpecErrorV1),
}

impl fmt::Display for DirectInlineRouteErrorV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation => f.write_str(
                "an input account came from another observation or repeated where distinctness is required",
            ),
            Self::FixedFrame => f.write_str("the fixed Hot prefix could not be assembled exactly"),
            Self::ChildFrame => f.write_str("a child FrameSpec refused its semantic role"),
            Self::Profile => {
                f.write_str("AccountProfile width, alias, privilege, or physical geometry refused")
            }
            Self::Seal => {
                f.write_str("the canonical write-once capability seal closure refused")
            }
            Self::Finalization(refusal) => write!(f, "{refusal}"),
            // Every carrying variant prints the cause its authority named.
            cause => write!(f, "{cause:?}"),
        }
    }
}

/// Which side of the trade a participant clause refused on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineParticipantSideV3 {
    /// The seller half.
    Seller,
    /// The buyer half.
    Buyer,
}

impl fmt::Display for DirectInlineParticipantSideV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Seller => "seller",
            Self::Buyer => "buyer",
        })
    }
}

/// Which of the three collateral token accounts a clause refused on.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineCollateralRoleV3 {
    /// The buyer's debited collateral source.
    Buyer,
    /// The seller's credited collateral destination.
    Seller,
    /// The fee recipient's credited destination.
    Fee,
}

impl fmt::Display for DirectInlineCollateralRoleV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Buyer => "buyer",
            Self::Seller => "seller",
            Self::Fee => "fee",
        })
    }
}

/// The flattened shape of a sealed-execution-report projection refusal.
///
/// [`project_direct_inline_sealed_execution_report_v3`] returns
/// [`DirectInlineRoutedTransactionErrorV3`], which carries a
/// [`DirectInlineRouteErrorV3`] of its own; embedding it here would make this
/// type recursive. The discriminant is what a caller can act on, so only the
/// discriminant crosses.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineSealedReportProjectionRefusalV3 {
    /// The route failed its own canonical semantic projection.
    Route,
    /// Report, route, payer, or table observations did not form one snapshot.
    Snapshot,
    /// The report's exact instruction sequence differed from the assembled route.
    Instruction,
    /// The supplied table was not the exact activated frozen stable-only table.
    LookupTable,
    /// Message-required signers differed from the semantic route.
    Signer,
    /// The complete static-plus-loaded key set exceeded devnet's active limit.
    AccountLocks,
    /// Versioned-message construction or packet admission refused.
    Routing,
}

impl DirectInlineSealedReportProjectionRefusalV3 {
    fn from_transaction_error(error: &DirectInlineRoutedTransactionErrorV3) -> Self {
        match error {
            DirectInlineRoutedTransactionErrorV3::Route(_) => Self::Route,
            DirectInlineRoutedTransactionErrorV3::Snapshot => Self::Snapshot,
            DirectInlineRoutedTransactionErrorV3::Instruction => Self::Instruction,
            DirectInlineRoutedTransactionErrorV3::LookupTable => Self::LookupTable,
            DirectInlineRoutedTransactionErrorV3::Signer => Self::Signer,
            DirectInlineRoutedTransactionErrorV3::AccountLocks => Self::AccountLocks,
            DirectInlineRoutedTransactionErrorV3::Routing(_) => Self::Routing,
            DirectInlineRoutedTransactionErrorV3::DirectInlineTransaction(_) => Self::Instruction,
        }
    }
}

impl fmt::Display for DirectInlineSealedReportProjectionRefusalV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Route => "the route failed its own semantic projection",
            Self::Snapshot => "report, route, payer and table were not one snapshot",
            Self::Instruction => "the report's instruction sequence differed from the route",
            Self::LookupTable => "the table was not the activated frozen stable-only table",
            Self::Signer => "message-required signers differed from the semantic route",
            Self::AccountLocks => "the static-plus-loaded key set exceeded the active limit",
            Self::Routing => "versioned-message construction or packet admission refused",
        })
    }
}

/// Every sealed-report fact that disagreed with the authenticated chain.
///
/// All of them are collected rather than the first, because an operator who
/// fixes one and re-runs has learned almost nothing. Same reason as
/// `refusing_ticket_half_clauses_v1` in the Direct trade producer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectInlineSealedReportFactsRefusalV3 {
    /// The selected execution program differed.
    pub selected_program: bool,
    /// The outcome count differed, observed then authenticated.
    pub outcome_count: Option<(u32, u32)>,
    /// The product record digest differed.
    pub product_record: bool,
    /// The Trading artifact release differed.
    pub trading_artifact_release: bool,
    /// The checked manifest digest differed.
    pub checked_manifest_digest: bool,
}

impl DirectInlineSealedReportFactsRefusalV3 {
    fn refuses(&self) -> bool {
        self.selected_program
            || self.outcome_count.is_some()
            || self.product_record
            || self.trading_artifact_release
            || self.checked_manifest_digest
    }
}

impl fmt::Display for DirectInlineSealedReportFactsRefusalV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut clauses: Vec<String> = Vec::new();
        if self.selected_program {
            clauses.push("selected program".to_owned());
        }
        if let Some((observed, expected)) = self.outcome_count {
            clauses.push(format!(
                "outcome count {observed} is not the authenticated {expected}"
            ));
        }
        if self.product_record {
            clauses.push("product record digest".to_owned());
        }
        if self.trading_artifact_release {
            clauses.push("Trading artifact release".to_owned());
        }
        if self.checked_manifest_digest {
            clauses.push("checked manifest digest".to_owned());
        }
        write!(
            f,
            "the projected sealed report disagrees with the authenticated chain on: {}",
            clauses.join(", ")
        )
    }
}

/// Every descriptor/strategy closure clause that refused.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectInlineStrategyClosureRefusalV3 {
    /// The strategy disposition was not `Interpreted`.
    pub disposition: bool,
    /// The strategy named a certificate program.
    pub certificate_program: bool,
    /// The strategy named an admission program.
    pub admission_program: bool,
    /// The strategy's Transition program is not the descriptor's.
    pub transition_program: bool,
    /// The descriptor's strategy program is not the hash of the strategy bytes.
    pub strategy_digest: bool,
}

impl DirectInlineStrategyClosureRefusalV3 {
    fn refuses(&self) -> bool {
        self.disposition
            || self.certificate_program
            || self.admission_program
            || self.transition_program
            || self.strategy_digest
    }
}

impl fmt::Display for DirectInlineStrategyClosureRefusalV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut clauses: Vec<&str> = Vec::new();
        if self.disposition {
            clauses.push("the strategy disposition is not Interpreted");
        }
        if self.certificate_program {
            clauses.push("the strategy names a certificate program");
        }
        if self.admission_program {
            clauses.push("the strategy names an admission program");
        }
        if self.transition_program {
            clauses.push("the strategy Transition program is not the descriptor's");
        }
        if self.strategy_digest {
            clauses.push("the descriptor's strategy program is not the strategy bytes' digest");
        }
        write!(
            f,
            "the descriptor/strategy closure refuses: {}",
            clauses.join(", ")
        )
    }
}

/// Every clause on which the decoded family request disagreed with the
/// authenticated signed intents.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectInlineRequestIntentRefusalV3 {
    /// The request's seller maker is not the authenticated seller.
    pub seller_maker: bool,
    /// The request's seller intent is not the authenticated signed intent.
    pub seller_intent: bool,
    /// The request's buyer maker is not the authenticated buyer.
    pub buyer_maker: bool,
    /// The request's buyer intent is not the authenticated signed intent.
    pub buyer_intent: bool,
    /// The request fill differed, observed then authenticated.
    pub fill: Option<(u64, u64)>,
    /// The request execution price differed, observed then authenticated.
    pub execution_price: Option<(u64, u64)>,
}

impl DirectInlineRequestIntentRefusalV3 {
    fn refuses(&self) -> bool {
        self.seller_maker
            || self.seller_intent
            || self.buyer_maker
            || self.buyer_intent
            || self.fill.is_some()
            || self.execution_price.is_some()
    }
}

impl fmt::Display for DirectInlineRequestIntentRefusalV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut clauses: Vec<String> = Vec::new();
        if self.seller_maker {
            clauses.push("seller maker".to_owned());
        }
        if self.seller_intent {
            clauses.push("seller signed intent".to_owned());
        }
        if self.buyer_maker {
            clauses.push("buyer maker".to_owned());
        }
        if self.buyer_intent {
            clauses.push("buyer signed intent".to_owned());
        }
        if let Some((observed, expected)) = self.fill {
            clauses.push(format!(
                "fill {observed} is not the authenticated {expected}"
            ));
        }
        if let Some((observed, expected)) = self.execution_price {
            clauses.push(format!(
                "execution price {observed} is not the authenticated {expected}"
            ));
        }
        write!(
            f,
            "the decoded family request disagrees with the authenticated intents on: {}",
            clauses.join(", ")
        )
    }
}

/// Every clause on which a first-use maker replay was not a fundable vacancy.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DirectInlineVacancyRefusalV3 {
    /// The account is not System-owned.
    pub owner: bool,
    /// The account is executable.
    pub executable: bool,
    /// The account carries data, observed width.
    pub data_len: Option<usize>,
    /// The declared rent beneficiary is the zero address.
    pub rent_beneficiary: bool,
    /// The declared rent principal is zero.
    pub rent_principal: bool,
}

impl DirectInlineVacancyRefusalV3 {
    fn refuses(&self) -> bool {
        self.owner
            || self.executable
            || self.data_len.is_some()
            || self.rent_beneficiary
            || self.rent_principal
    }
}

impl fmt::Display for DirectInlineVacancyRefusalV3 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut clauses: Vec<String> = Vec::new();
        if self.owner {
            clauses.push("its owner is not the System program".to_owned());
        }
        if self.executable {
            clauses.push("it is executable".to_owned());
        }
        if let Some(observed) = self.data_len {
            clauses.push(format!("it carries {observed} data bytes, not 0"));
        }
        if self.rent_beneficiary {
            clauses.push("the rent beneficiary is the zero address".to_owned());
        }
        if self.rent_principal {
            clauses.push("the rent principal is 0".to_owned());
        }
        write!(f, "{}", clauses.join(", "))
    }
}

/// Which clause of the exterior Hot finalization refused, and what it observed.
///
/// Every refusing site in [`prepare_direct_inline_hot_finalization_v3`] and its
/// three helpers gets its own variant. Before this existed, all of them shared
/// the bare `Finalization` unit, so the driver could print only "Direct Hot
/// finalization: Finalization" and an operator had to instrument the binary to
/// find out which of two dozen clauses had fired. That is the same defect class
/// as the Direct producer's owner-or-width sentence and the ticket's twelve-way
/// blanket OR, both of which this repository has already paid to remove.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineFinalizationRefusalV3 {
    /// The distinct-account report did not project onto the authenticated route.
    SealedReportProjection(DirectInlineSealedReportProjectionRefusalV3),
    /// The projected sealed report disagreed with the authenticated chain.
    SealedReportFacts(DirectInlineSealedReportFactsRefusalV3),
    /// The signed family request did not decode at this outcome count.
    FamilyRequestDecode {
        /// Outcome count the decode was attempted at.
        outcome_count: u32,
        /// Complete family request width.
        request_bytes: usize,
    },
    /// The family request decoded as another Direct execution shape.
    FamilyRequestNotInlineOrdinary,
    /// Transition/Effect child-request projection refused.
    ChildRequestProjection(DirectInlineOrdinaryChildProjectionErrorV3),
    /// The assembled child request bank was not the exact ordinary width.
    RequestBankWidth {
        /// Width assembled from the projected Claims and Custody requests.
        observed: usize,
        /// Exact ordinary bank width.
        expected: usize,
    },
    /// The route's capability descriptor did not decode as `CapabilityProgramV4`.
    DescriptorDecode {
        /// Raw descriptor account width.
        descriptor_bytes: usize,
    },
    /// The route's strategy artifact did not decode as `ExecutionStrategyProgramV2`.
    StrategyDecode {
        /// Raw strategy account width.
        strategy_bytes: usize,
    },
    /// The descriptor/strategy closure refused.
    StrategyClosure(DirectInlineStrategyClosureRefusalV3),
    /// The canonical Direct finalizer refused.
    Finalizer {
        /// The finalizer's own refusal.
        error: DirectFinalizationErrorV3,
        /// The candidate partition's own refusal, when the finalizer's was the
        /// `Candidate` collapse. See
        /// [`rederive_direct_inline_candidate_refusal_v3`].
        candidate: Option<DirectInlineCandidateErrorV2>,
    },
    /// A commitment index had no canonical poststate role.
    PoststateRoleIndex {
        /// Ordered commitment index.
        index: usize,
    },
    /// A commitment carried another role than its ordered index requires.
    PoststateRoleOrder {
        /// Ordered commitment index.
        index: usize,
        /// Role the commitment declared.
        observed: DirectInlinePoststateRoleV3,
        /// Role the ordered index requires.
        expected: DirectInlinePoststateRoleV3,
    },
    /// A commitment's declared width did not fit host addressing.
    PoststateWidth {
        /// Ordered commitment index.
        index: usize,
        /// Declared width.
        data_len: u32,
    },
    /// Materializing one exact poststate refused.
    PoststateProjection {
        /// Ordered commitment index.
        index: usize,
        /// Role being materialized.
        role: DirectInlinePoststateRoleV3,
        /// The finalizer's own refusal.
        error: DirectFinalizationErrorV3,
        /// The candidate partition's own refusal, when the finalizer's was the
        /// `Candidate` collapse.
        candidate: Option<DirectInlineCandidateErrorV2>,
    },
    /// A materialized poststate did not hash to its own commitment.
    PoststateDigest {
        /// Ordered commitment index.
        index: usize,
        /// Role whose bytes disagreed.
        role: DirectInlinePoststateRoleV3,
    },
    /// The Direct root account was shorter than the capability root header.
    RootHeaderWidth {
        /// Complete root account width.
        observed: usize,
        /// Capability root header width.
        header: usize,
    },
    /// The Direct root tail did not decode as `DirectRootStateV1`.
    RootStateDecode {
        /// Width of the tail handed to the decoder.
        tail_bytes: usize,
    },
    /// The decoded request disagreed with the authenticated signed intents.
    RequestIntents(DirectInlineRequestIntentRefusalV3),
    /// One participant's adjacent-ed25519 intent did not authenticate.
    ParticipantIntent {
        /// Side whose intent refused.
        side: DirectInlineParticipantSideV3,
    },
    /// A first-use participant's maker replay was not a fundable vacancy.
    ParticipantVacancy {
        /// Side whose replay refused.
        side: DirectInlineParticipantSideV3,
        /// Every vacancy clause that refused.
        clauses: DirectInlineVacancyRefusalV3,
    },
    /// An existing participant's maker replay did not decode.
    ParticipantMakerReplayDecode {
        /// Side whose replay refused.
        side: DirectInlineParticipantSideV3,
        /// Observed replay account width.
        data_bytes: usize,
        /// Exact maker replay width.
        expected_bytes: usize,
    },
    /// A collateral token account did not parse as a Token account.
    CollateralTokenParse {
        /// Which of the three token accounts refused.
        role: DirectInlineCollateralRoleV3,
        /// Observed account width.
        data_bytes: usize,
    },
    /// The buyer's collateral account carried no delegate.
    ///
    /// Direct debits the buyer through a delegated allowance; an undelegated
    /// source account has nothing for Custody to spend.
    BuyerCollateralDelegateAbsent {
        /// The buyer collateral account that carries no delegate.
        account: Pubkey,
        /// Its token owner.
        owner: Pubkey,
    },
}

impl fmt::Display for DirectInlineFinalizationRefusalV3 {
    #[allow(clippy::too_many_lines)]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SealedReportProjection(refusal) => write!(
                f,
                "the distinct-account report does not project onto the authenticated route: {refusal}"
            ),
            Self::SealedReportFacts(refusal) => write!(f, "{refusal}"),
            Self::FamilyRequestDecode {
                outcome_count,
                request_bytes,
            } => write!(
                f,
                "the {request_bytes}-byte family request does not decode as a Direct execution request at outcome count {outcome_count}"
            ),
            Self::FamilyRequestNotInlineOrdinary => f.write_str(
                "the family request decodes as another Direct execution shape, not InlineOrdinary",
            ),
            Self::ChildRequestProjection(error) => write!(
                f,
                "child-request projection through the authenticated Transition/Effect refused: {error:?}"
            ),
            Self::RequestBankWidth { observed, expected } => write!(
                f,
                "the projected child request bank is {observed} bytes, not the exact ordinary {expected}"
            ),
            Self::DescriptorDecode { descriptor_bytes } => write!(
                f,
                "the {descriptor_bytes}-byte capability descriptor does not decode as CapabilityProgramV4"
            ),
            Self::StrategyDecode { strategy_bytes } => write!(
                f,
                "the {strategy_bytes}-byte strategy artifact does not decode as ExecutionStrategyProgramV2"
            ),
            Self::StrategyClosure(refusal) => write!(f, "{refusal}"),
            Self::Finalizer {
                error,
                candidate: Some(candidate),
            } => write!(
                f,
                "the canonical Direct finalizer refused: {error:?}, and the candidate partition re-run on the same inputs refuses at {candidate:?}"
            ),
            Self::Finalizer {
                error,
                candidate: None,
            } => write!(f, "the canonical Direct finalizer refused: {error:?}"),
            Self::PoststateRoleIndex { index } => write!(
                f,
                "commitment index {index} has no canonical poststate role"
            ),
            Self::PoststateRoleOrder {
                index,
                observed,
                expected,
            } => write!(
                f,
                "commitment {index} declares role {observed:?}, but that ordered index is {expected:?}"
            ),
            Self::PoststateWidth { index, data_len } => write!(
                f,
                "commitment {index} declares a {data_len}-byte poststate, which does not fit host addressing"
            ),
            Self::PoststateProjection {
                index,
                role,
                error,
                candidate: Some(candidate),
            } => write!(
                f,
                "materializing the {role:?} poststate at commitment {index} refused: {error:?}, and the candidate partition re-run on the same inputs refuses at {candidate:?}"
            ),
            Self::PoststateProjection {
                index,
                role,
                error,
                candidate: None,
            } => write!(
                f,
                "materializing the {role:?} poststate at commitment {index} refused: {error:?}"
            ),
            Self::PoststateDigest { index, role } => write!(
                f,
                "the materialized {role:?} poststate at commitment {index} does not hash to its own commitment digest"
            ),
            Self::RootHeaderWidth { observed, header } => write!(
                f,
                "the Direct root account is {observed} bytes, shorter than the {header}-byte capability root header"
            ),
            Self::RootStateDecode { tail_bytes } => write!(
                f,
                "the {tail_bytes}-byte Direct root tail does not decode as DirectRootStateV1"
            ),
            Self::RequestIntents(refusal) => write!(f, "{refusal}"),
            Self::ParticipantIntent { side } => write!(
                f,
                "the {side} adjacent-ed25519 signed intent does not authenticate"
            ),
            Self::ParticipantVacancy { side, clauses } => write!(
                f,
                "the {side} maker replay is declared first-use but is not a fundable vacancy: {clauses}"
            ),
            Self::ParticipantMakerReplayDecode {
                side,
                data_bytes,
                expected_bytes,
            } => write!(
                f,
                "the existing {side} maker replay is {data_bytes} bytes and does not decode as the {expected_bytes}-byte MakerReplayRootV1"
            ),
            Self::CollateralTokenParse { role, data_bytes } => write!(
                f,
                "the {role} collateral account is {data_bytes} bytes and does not parse as a Token account"
            ),
            Self::BuyerCollateralDelegateAbsent { account, owner } => write!(
                f,
                "buyer collateral account {account} owned by {owner} carries no delegate, so Custody has no allowance to spend"
            ),
        }
    }
}

/// Refusal from route-aware immutable-ALT compilation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectInlineRoutedTransactionErrorV3 {
    /// The route itself failed its canonical semantic projection.
    Route(DirectInlineRouteErrorV3),
    /// Report, route, payer, or table observations did not form one snapshot.
    Snapshot,
    /// The report's exact instruction sequence differed from the assembled route.
    Instruction,
    /// The supplied table was not the exact activated frozen stable-only table.
    LookupTable,
    /// Message-required signers differed from the semantic route.
    Signer,
    /// The complete static-plus-loaded key set exceeded devnet's active limit.
    AccountLocks,
    /// Versioned-message construction or packet admission refused.
    Routing(crate::versioned::Error),
    /// `dclutch_operator` refused; the cause is its own.
    DirectInlineTransaction(crate::direct_inline_v3::DirectInlineTransactionErrorV3),
}

/// Exact child-route authorities derived from Transition/Effect output.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineChildAuthoritiesV3 {
    /// Canonical complete family request consumed by the Hot outer.
    pub family_request: [u8; crate::direct_inline_v3::DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3],
    /// Claims child request digest.
    pub claims_request_digest: [u8; 32],
    /// Claims child caller-authority PDA.
    pub claims_authority: Pubkey,
    /// Custody child request digests in FIXED SLOT order.
    ///
    /// The slots are the four request shapes a Direct inline execution can
    /// take -- seller terminal, seller intermediate, fee continuation, fee
    /// sole -- and NOT the order the Effect executes them in. Which one
    /// actually runs is `DirectInlineEffectDispatchV2::custody_slots`, and
    /// exactly one of the four does.
    pub custody_request_digests: [[u8; 32]; 4],
    /// Custody child caller-authority PDAs in FIXED SLOT order, as above.
    pub custody_authorities: [Pubkey; 4],
    /// Canonical bump of `claims_authority`, mined here so Trading need not.
    ///
    /// This projection already ran the search to produce the address above; the
    /// bump it threw away is the one byte that lets the on-chain walk reproduce
    /// the same address with `create_program_address` instead of searching for
    /// it again. It is a hint and never an authority -- Trading rebuilds the
    /// seeds itself and refuses unless the address reproduces the account at
    /// coordinate 0. See `HotBumpHintsV1`.
    pub claims_authority_bump: u8,
    /// Canonical bumps of `custody_authorities`, in the same FIXED SLOT order.
    ///
    /// Indexing this array with a literal is a bug. The slot that runs depends
    /// on the settlement: `expected_custody_slots` picks slot 0 only when
    /// `total_fee_transfer == 0`, and slot 1 whenever a fee is transferred.
    /// Use `child_caller_bumps`, which is already ordered the way the on-chain
    /// walk assigns its hint slots.
    pub custody_authority_bumps: [u8; 4],
    /// The two `HotBumpHintsV1::child_caller` slots, ready to hand to the hot
    /// envelope with no indexing at the call site.
    ///
    /// The on-chain walk numbers its hint slots route-major over child
    /// INVOCATIONS (`hot_v3::child_caller_hint_v1` reads `child_caller[ordinal]`),
    /// so for InlineOrdinary slot 0 is the Claims route and slot 1 is the ONE
    /// enabled Custody route. That route is named by
    /// `DirectInlineEffectDispatchV2::custody_slots` and is slot 1 of the fixed
    /// four for every fee-bearing fill, so a caller that reached into
    /// `custody_authority_bumps[0]` mined the ZERO-FEE route's bump and the
    /// program refused `Release` reproducing it. This field exists so that
    /// mistake has nowhere left to live.
    ///
    /// A hint is a memo and never an authority: a wrong one reproduces a
    /// different address and refuses, and a zero searches.
    pub child_caller_bumps: [u8; 2],
    /// Which of the four fixed Custody slots this execution actually runs.
    ///
    /// Reported because it is the fact `child_caller_bumps[1]` turns on, and a
    /// bump byte is too coarse to test against: two Custody routes routinely
    /// share one. `None` means no Custody child runs.
    pub enabled_custody_slot: Option<u8>,
}

/// Release-authenticated program and ProgramData coordinates used by Direct.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineCheckedProgramAccountsV3 {
    /// Current Core program.
    pub core_program: Pubkey,
    /// Current Core ProgramData.
    pub core_programdata: Pubkey,
    /// Current Trading program.
    pub trading_program: Pubkey,
    /// Current Trading ProgramData.
    pub trading_programdata: Pubkey,
    /// Exact hostile-decoded user-checked five-role release manifest. The
    /// Trading artifact release and manifest digest are derived from these
    /// bytes after joining them to the same finalized activation cache.
    pub checked_execution_release_set: [u8; CHECKED_MULTIPROGRAM_BYTES_V1],
    /// Current Registry program.
    pub registry_program: Pubkey,
    /// Current Claims program.
    pub claims_program: Pubkey,
    /// Current Claims ProgramData.
    pub claims_programdata: Pubkey,
    /// Current Custody program.
    pub custody_program: Pubkey,
    /// Current Rent program.
    pub rent_program: Pubkey,
    /// Realm-selected current token program.
    pub token_program: Pubkey,
}

/// Signed request and authenticated context required by production route assembly.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DirectInlineRouteAuthenticationV3 {
    /// Seller intent and detached signature.
    pub seller: SignedDirectIntentV3,
    /// Buyer intent and detached signature.
    pub buyer: SignedDirectIntentV3,
    /// Exact positive fill.
    pub fill: u64,
    /// Exact execution price at the selected config scale.
    pub execution_price: u64,
    /// Chain-authenticated semantic context projected by the same artifacts.
    pub context: DirectOrdinaryAuthenticatedContextV3,
    /// Exact current release and Realm-selected program coordinates.
    pub programs: DirectInlineCheckedProgramAccountsV3,
}

/// Authenticated semantic route plus its canonical physical projection.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineAuthenticatedRouteV3 {
    /// Exact family and child requests with their derived caller PDAs.
    pub child_authorities: DirectInlineChildAuthoritiesV3,
    /// AccountProfile-packed physical route admitted for Hot construction.
    pub physical: DirectInlinePhysicalRouteV3,
    /// Hot-execution projection with all six seal-covered staging coordinates
    /// canonically aliased to their matching raw records.
    pub sealed_execution_physical: DirectInlinePhysicalRouteV3,
    /// Full Market/release/artifact/Product authentication for this request.
    pub chain: AuthenticatedDirectHotChainV4,
    /// Exact checked five-role manifest digest authenticated against Activation.
    pub checked_manifest_digest: [u8; 32],
    /// Sole authenticated transaction/rent payer and lookup-table authority.
    pub payer: Pubkey,
}

/// Exact unsigned lifecycle for the one request-specific Direct lookup table.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineLookupTableProvisionV3 {
    /// Address derived by the official lookup-table program.
    pub lookup_table: Pubkey,
    /// Sole table authority and trade payer.
    pub authority: Pubkey,
    /// Finalized slot used by the official table-address derivation.
    pub creation_slot: u64,
    /// Exact first-semantic-use union of stable and request-bound coordinates.
    pub addresses: Vec<Pubkey>,
    /// Official create instruction.
    pub create: Instruction,
    /// Official bounded extensions in exact order.
    pub extensions: Vec<Instruction>,
    /// Official irreversible freeze instruction.
    pub freeze: Instruction,
}

/// Exact unsigned materialization plan for the ordinary Direct capability seal.
///
/// The seal address and body are pure projections of finalized Registry bytes,
/// the selected action, and the chain-authenticated Trading semantic release.
/// The caller supplies no seal bytes, PDA, or rent quote.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineCapabilitySealPlanV3 {
    /// Canonical content-derived seal key.
    pub key: CapabilitySealKeyV1,
    /// Canonical PDA under the current Trading program.
    pub seal: Pubkey,
    /// Exact finalized observation used to construct the instruction.
    pub observation: Observation,
    /// Lamports already parked at the vacant PDA.
    pub initial_lamports: u64,
    /// Rent minimum decoded from the finalized Rent sysvar.
    pub rent_minimum_lamports: u64,
    /// Exact lamports expected after successful materialization.
    pub expected_final_lamports: u64,
    /// Exact canonical persisted body expected after successful materialization.
    pub expected_body: Vec<u8>,
    /// The finalized observation already carries the exact materialized seal.
    pub already_materialized: bool,
    /// Sole payer and signer, already authenticated by the named route.
    pub payer: Pubkey,
    /// Exact request-specific ALT contents required to route this outer.
    pub lookup_addresses: Vec<Pubkey>,
    /// Exact permissionless Trading instruction. The payer is the route payer.
    pub instruction: Instruction,
}

/// One exact materialized writable-account poststate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineExpectedPoststateV3 {
    /// Canonical role/address/owner/lamport/width/digest commitment.
    pub commitment: DirectInlinePoststateCommitmentV3,
    /// Complete expected account bytes after successful Hot execution.
    pub data: Vec<u8>,
}

/// Exact exterior finalization plan for one chain-authenticated ordinary trade.
///
/// The named route is authenticated again inside the constructor. The six
/// execution-only seal aliases are then projected into the Hot report, and the
/// same pure finalizer consumed by Trading owns the acknowledgement and all ten
/// complete writable poststates. Callers cannot supply acknowledgement bytes,
/// child receipts, or poststate digests.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectInlineHotFinalizationPlanV3 {
    /// Exact Hot report after the six CapabilitySeal execution aliases.
    pub sealed_report: DirectInlineHotReportV3,
    /// Canonical economic candidate, child transcript, commitments, and ACK.
    pub finalization: DirectInlineFinalizationV3,
    /// Complete expected account bytes in the finalizer's canonical ten-role order.
    pub poststates: [DirectInlineExpectedPoststateV3; DIRECT_INLINE_POSTSTATE_COUNT_V3],
}

/// Project every child request and derive its exact Trading caller authority.
#[allow(clippy::too_many_arguments)]
pub fn derive_direct_inline_child_authorities_v3(
    seller: SignedDirectIntentV3,
    buyer: SignedDirectIntentV3,
    fill: u64,
    execution_price: u64,
    context: DirectOrdinaryAuthenticatedContextV3,
    account_profile_bytes: &[u8],
    transition_bytes: &[u8],
    effect_bytes: &[u8],
) -> Result<DirectInlineChildAuthoritiesV3, DirectInlineRouteErrorV3> {
    let family_request = compile_direct_inline_request_v3(seller, buyer, fill, execution_price)
        .map_err(DirectInlineRouteErrorV3::DirectInline)?;
    let parent_digest = hash(&family_request).to_bytes();
    if context.parent_request_digest != parent_digest
        || context.release_set == [0; 32]
        || context.market == [0; 32]
        || context.trading_program == [0; 32]
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let request = match DirectExecutionRequestV3::decode(&family_request, context.outcome_count)
        .map_err(DirectInlineRouteErrorV3::DirectExecutionRequest)?
    {
        DirectExecutionRequestV3::InlineOrdinary(request) => request,
        _ => return Err(DirectInlineRouteErrorV3::ChildFrame),
    };
    let projected = project_direct_inline_ordinary_child_requests_v3(
        request,
        context,
        account_profile_bytes,
        transition_bytes,
        effect_bytes,
    )
    .map_err(DirectInlineRouteErrorV3::DirectInlineOrdinaryChildProjection)?;
    derive_child_authorities(context, family_request, request, projected)
}

/// Authenticate every named account join and assemble the production route.
///
/// Child caller addresses are derived from the exact signed request and
/// selected Transition/Effect bytes. They are never accepted as routing hints.
/// This bridge also closes the owner and distinctness facts FrameSpecs do not
/// own: a FrameSpec orders roles and privileges, while the selected child
/// programs own their state bytes and this host boundary owns which named
/// external account is assigned to each role.
pub fn assemble_authenticated_direct_inline_ordinary_route_v3(
    route: DirectInlineOrdinaryRouteV3,
    outcome_count: u32,
    authentication: DirectInlineRouteAuthenticationV3,
) -> Result<DirectInlineAuthenticatedRouteV3, DirectInlineRouteErrorV3> {
    if outcome_count != authentication.context.outcome_count {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    authenticate_named_route_v3(&route, authentication)?;
    let checked_release = authenticate_checked_direct_release_v3(&route, authentication)?;
    let child_authorities = derive_direct_inline_child_authorities_v3(
        authentication.seller,
        authentication.buyer,
        authentication.fill,
        authentication.execution_price,
        authentication.context,
        &route.fixed.account_profile.raw.data,
        &route.fixed.transition.raw.data,
        &route.fixed.effect.raw.data,
    )?;
    if route.claims.caller_authority.key != child_authorities.claims_authority
        || route
            .custody
            .caller_authorities
            .iter()
            .map(|account| account.key)
            .ne(child_authorities.custody_authorities)
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let physical = assemble_direct_inline_ordinary_route_v3(route.clone(), outcome_count)?;
    if has_duplicate(
        &physical
            .fixed_accounts
            .iter()
            .map(|entry| entry.account.key)
            .collect::<Vec<_>>(),
    ) {
        return Err(DirectInlineRouteErrorV3::FixedFrame);
    }
    let chain = authenticate_direct_hot_chain_v4(
        &DirectInlineHotStateV3 {
            fixed_accounts: physical.fixed_accounts.clone(),
            strategy_accounts: physical.strategy_accounts.clone(),
            runtime_accounts: physical.runtime_accounts.clone(),
            release_set: authentication.context.release_set,
            generation: authentication.context.generation,
            clock_slot: authentication.context.slot,
            minimum_finalized_slot: physical.observation.slot,
            hot_outer: Some(CheckedHotOuterReleaseV3 {
                trading_program: authentication.programs.trading_program,
                artifact_release: checked_release.trading_artifact_release,
                checked_manifest_digest: checked_release.checked_manifest_digest,
            }),
        },
        &child_authorities.family_request,
    )
    .map_err(DirectInlineRouteErrorV3::DirectInline)?;
    if chain.release_set != authentication.context.release_set
        || chain.market.to_bytes() != authentication.context.market
        || chain.registry_program != authentication.programs.registry_program
        || chain.outcome_count != outcome_count
        || chain.product_record != authentication.context.product_record_digest
    {
        return Err(DirectInlineRouteErrorV3::Seal);
    }
    authenticate_direct_child_releases_v3(&route, authentication)?;
    authenticate_direct_ordinary_context_v3(&route, authentication, chain)?;
    let sealed_execution_physical = project_direct_inline_sealed_execution_physical_v3(&physical)?;
    Ok(DirectInlineAuthenticatedRouteV3 {
        child_authorities,
        physical,
        sealed_execution_physical,
        chain,
        checked_manifest_digest: checked_release.checked_manifest_digest,
        payer: route.payer.key,
    })
}

/// Active devnet account-lock limit. The exact devnet genesis admission in the
/// exterior caller owns when this profile is applicable.
pub const DIRECT_INLINE_DEVNET_ACCOUNT_LOCK_LIMIT_V3: usize = 64;

/// The six fixed staging coordinates whose finalized observations are owned by
/// the write-once CapabilitySeal before ordinary Direct execution.
///
/// The pairs themselves are the ABI's -- they are the same six for every family
/// that submits the shape, and this crate spelled them a second time.
pub const DIRECT_INLINE_SEALED_EXECUTION_ALIASES_V3: [(usize, usize); 6] =
    dclutch_market::capability_program::hot_v3::SEALED_EXECUTION_FIXED_ALIASES_V3;

/// Project the execution-only fixed aliases after the distinct named route has
/// authenticated every real raw/staging pair. No account may be added, removed,
/// reordered, or receive broader privileges.
pub fn project_direct_inline_sealed_execution_physical_v3(
    physical: &DirectInlinePhysicalRouteV3,
) -> Result<DirectInlinePhysicalRouteV3, DirectInlineRouteErrorV3> {
    if physical.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || physical.fixed_classes.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || has_duplicate(
            &physical
                .fixed_accounts
                .iter()
                .map(|entry| entry.account.key)
                .collect::<Vec<_>>(),
        )
    {
        return Err(DirectInlineRouteErrorV3::FixedFrame);
    }
    let mut projected = physical.clone();
    for (raw, staging) in DIRECT_INLINE_SEALED_EXECUTION_ALIASES_V3 {
        let raw_meta = physical
            .fixed_accounts
            .get(raw)
            .ok_or(DirectInlineRouteErrorV3::FixedFrame)?;
        let staging_meta = physical
            .fixed_accounts
            .get(staging)
            .ok_or(DirectInlineRouteErrorV3::FixedFrame)?;
        if raw_meta.is_signer
            || raw_meta.is_writable
            || staging_meta.is_signer
            || staging_meta.is_writable
            || raw_meta.account.key == staging_meta.account.key
            || physical.fixed_classes.get(raw) != Some(&DirectInlineAddressClassV3::LookupStable)
            || physical.fixed_classes.get(staging)
                != Some(&DirectInlineAddressClassV3::LookupStable)
        {
            return Err(DirectInlineRouteErrorV3::FixedFrame);
        }
        *projected
            .fixed_accounts
            .get_mut(staging)
            .ok_or(DirectInlineRouteErrorV3::FixedFrame)? = raw_meta.clone();
    }
    require_direct_inline_sealed_execution_shape_v3(&projected)?;
    Ok(projected)
}

fn require_direct_inline_sealed_execution_shape_v3(
    physical: &DirectInlinePhysicalRouteV3,
) -> Result<(), DirectInlineRouteErrorV3> {
    if physical.fixed_accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3
        || physical.fixed_classes.len() != HOT_FIXED_ACCOUNT_COUNT_V3
    {
        return Err(DirectInlineRouteErrorV3::FixedFrame);
    }
    for (raw, staging) in DIRECT_INLINE_SEALED_EXECUTION_ALIASES_V3 {
        if physical.fixed_accounts.get(raw) != physical.fixed_accounts.get(staging)
            || physical.fixed_classes.get(raw) != physical.fixed_classes.get(staging)
        {
            return Err(DirectInlineRouteErrorV3::FixedFrame);
        }
    }
    for (left, entry) in physical.fixed_accounts.iter().enumerate() {
        for (offset, other) in physical
            .fixed_accounts
            .get(left.saturating_add(1)..)
            .ok_or(DirectInlineRouteErrorV3::FixedFrame)?
            .iter()
            .enumerate()
        {
            let right = left
                .checked_add(offset)
                .and_then(|value| value.checked_add(1))
                .ok_or(DirectInlineRouteErrorV3::FixedFrame)?;
            if other.account.key == entry.account.key
                && !DIRECT_INLINE_SEALED_EXECUTION_ALIASES_V3.contains(&(left, right))
            {
                return Err(DirectInlineRouteErrorV3::FixedFrame);
            }
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CheckedDirectReleaseV3 {
    trading_artifact_release: [u8; 32],
    checked_manifest_digest: [u8; 32],
}

fn authenticate_checked_direct_release_v3(
    route: &DirectInlineOrdinaryRouteV3,
    authentication: DirectInlineRouteAuthenticationV3,
) -> Result<CheckedDirectReleaseV3, DirectInlineRouteErrorV3> {
    let checked = CheckedExecutionReleaseSetV1::decode(
        &authentication.programs.checked_execution_release_set,
    )
    .map_err(|_| DirectInlineRouteErrorV3::ChildFrame)?;
    if checked
        .execution_release_set_id()
        .map_err(|_| DirectInlineRouteErrorV3::ChildFrame)?
        .to_bytes()
        != authentication.context.release_set
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let cache = ActivatedExecutionReleaseSetViewV1::decode(&route.fixed.activation_cache.data)
        .map_err(DirectInlineRouteErrorV3::Registry)?;
    let artifacts = checked.artifacts();
    for (role, artifact) in [
        ExecutionRoleV1::Core,
        ExecutionRoleV1::Claims,
        ExecutionRoleV1::Trading,
        ExecutionRoleV1::Resolution,
        ExecutionRoleV1::Custody,
    ]
    .into_iter()
    .zip(artifacts)
    {
        let selected = cache
            .role(role)
            .map_err(DirectInlineRouteErrorV3::Registry)?;
        if selected.release().to_bytes() != artifact.to_bytes() {
            return Err(DirectInlineRouteErrorV3::ChildFrame);
        }
    }
    let [core, claims, trading, _resolution, custody] = artifacts;
    if core.program().to_bytes() != authentication.programs.core_program.to_bytes()
        || claims.program().to_bytes() != authentication.programs.claims_program.to_bytes()
        || trading.program().to_bytes() != authentication.programs.trading_program.to_bytes()
        || custody.program().to_bytes() != authentication.programs.custody_program.to_bytes()
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let trading_artifact_release = ArtifactReleaseIdV1::new(hash(&trading.to_bytes()).to_bytes())
        .map_err(DirectInlineRouteErrorV3::ReleaseSet)?
        .to_bytes();
    let checked_manifest_digest = checked
        .checked_execution_release_set_id()
        .map_err(|_| DirectInlineRouteErrorV3::ChildFrame)?
        .to_bytes();
    Ok(CheckedDirectReleaseV3 {
        trading_artifact_release,
        checked_manifest_digest,
    })
}

/// Build the existing permissionless Trading seal outer from finalized evidence.
///
/// This authenticates the complete named Direct route first, then authenticates
/// each of the six Registry records at its content-derived raw PDA with a vacant
/// staging cursor. The selected descriptor supplies every companion schema and
/// digest. The only mutable input is the payer already authenticated by the
/// route; no caller-provided seal body, address, or rent value is accepted.
pub fn build_direct_inline_capability_seal_v3(
    route: DirectInlineOrdinaryRouteV3,
    outcome_count: u32,
    authentication: DirectInlineRouteAuthenticationV3,
) -> Result<DirectInlineCapabilitySealPlanV3, DirectInlineRouteErrorV3> {
    let authenticated = assemble_authenticated_direct_inline_ordinary_route_v3(
        route.clone(),
        outcome_count,
        authentication,
    )?;
    let fixed = &route.fixed;
    let descriptor_digest = hash(&fixed.descriptor.raw.data).to_bytes();
    let descriptor = CapabilityProgramV4::decode(&fixed.descriptor.raw.data)
        .map_err(DirectInlineRouteErrorV3::CapabilityProgram)?;
    let action = dclutch_trading::execution_v3::DirectExecutionActionV3::InlineOrdinary as u32;
    let key = CapabilitySealKeyV1::new(
        CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        descriptor_digest,
        action,
        authenticated.chain.trading_semantic_release,
        fixed.registry_program.key.to_bytes(),
    )
    .map_err(DirectInlineRouteErrorV3::CapabilitySeal)?;
    let (seal, seal_bump) = Pubkey::find_program_address(
        &key.seeds().as_slices(),
        &authentication.programs.trading_program,
    );
    if fixed.capability_seal.key != seal || fixed.capability_seal.executable {
        return Err(DirectInlineRouteErrorV3::Seal);
    }

    let rent =
        decode_rent(&fixed.rent_sysvar).map_err(DirectInlineRouteErrorV3::ObservationError)?;
    let rows = [
        seal_row(
            SealedRoleV1::Descriptor,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4,
            descriptor_digest,
            &fixed.descriptor,
            fixed.registry_program.key,
        )?,
        seal_row(
            SealedRoleV1::LifecyclePolicy,
            descriptor.lifecycle().schema().to_bytes(),
            descriptor.lifecycle().program().to_bytes(),
            &fixed.lifecycle,
            fixed.registry_program.key,
        )?,
        seal_row(
            SealedRoleV1::AccountProfile,
            descriptor.account_profile().schema().to_bytes(),
            descriptor.account_profile().program().to_bytes(),
            &fixed.account_profile,
            fixed.registry_program.key,
        )?,
        seal_row(
            SealedRoleV1::RequestProfile,
            descriptor.request_profile().schema().to_bytes(),
            descriptor.request_profile().program().to_bytes(),
            &fixed.request_profile,
            fixed.registry_program.key,
        )?,
        seal_row(
            SealedRoleV1::TransitionProgram,
            descriptor.transition().schema().to_bytes(),
            descriptor.transition().program().to_bytes(),
            &fixed.transition,
            fixed.registry_program.key,
        )?,
        seal_row(
            SealedRoleV1::EffectProgram,
            descriptor.effect().schema().to_bytes(),
            descriptor.effect().program().to_bytes(),
            &fixed.effect,
            fixed.registry_program.key,
        )?,
    ];
    let mut expected_body = vec![0_u8; CAPABILITY_SEAL_BYTES_V1];
    SealedDescriptorClosureV1::encode(key, rows, seal_bump, &mut expected_body)
        .map_err(DirectInlineRouteErrorV3::CapabilitySeal)?;

    let mut accounts = authenticated
        .physical
        .fixed_accounts
        .iter()
        .map(|entry| AccountMeta {
            pubkey: entry.account.key,
            is_signer: false,
            is_writable: false,
        })
        .collect::<Vec<_>>();
    accounts
        .get_mut(HOT_CAPABILITY_SEAL_ACCOUNT_V3)
        .ok_or(DirectInlineRouteErrorV3::FixedFrame)?
        .is_writable = true;
    accounts.push(AccountMeta::new(route.payer.key, true));
    accounts.push(AccountMeta::new_readonly(
        solana_sdk_ids::system_program::ID,
        false,
    ));
    if accounts.len() != HOT_FIXED_ACCOUNT_COUNT_V3 + 2
        || has_duplicate(
            &accounts
                .iter()
                .map(|entry| entry.pubkey)
                .collect::<Vec<_>>(),
        )
    {
        return Err(DirectInlineRouteErrorV3::Seal);
    }
    let request = CapabilitySealRequestV1::new(action, descriptor_digest)
        .map_err(DirectInlineRouteErrorV3::CapabilitySeal)?;
    let rent_minimum_lamports = rent.minimum_balance(CAPABILITY_SEAL_BYTES_V1);
    let initial_lamports = fixed.capability_seal.lamports;
    let vacant = fixed.capability_seal.owner == solana_sdk_ids::system_program::ID
        && fixed.capability_seal.data.is_empty();
    let already_materialized = fixed.capability_seal.owner
        == authentication.programs.trading_program
        && fixed.capability_seal.data == expected_body
        && initial_lamports >= rent_minimum_lamports;
    if !vacant && !already_materialized {
        return Err(DirectInlineRouteErrorV3::Seal);
    }
    let (_, lookup_addresses) =
        classify_direct_inline_ordinary_route_v3(&authenticated.sealed_execution_physical)?;
    Ok(DirectInlineCapabilitySealPlanV3 {
        key,
        seal,
        observation: authenticated.sealed_execution_physical.observation,
        initial_lamports,
        rent_minimum_lamports,
        expected_final_lamports: initial_lamports.max(rent_minimum_lamports),
        expected_body,
        already_materialized,
        payer: route.payer.key,
        lookup_addresses,
        instruction: Instruction {
            program_id: authentication.programs.trading_program,
            accounts,
            data: request.to_bytes().to_vec(),
        },
    })
}

/// Route the seal outer through the already-frozen, already-active table that
/// the exact signed Direct request will later use for Hot execution.
///
/// The table may name the still-vacant seal PDA: a lookup-table entry is an
/// address, not an existence claim. A second table and a seal-before-table
/// transaction are both refused.
pub fn compile_direct_inline_capability_seal_routed_v0_v3(
    plan: &DirectInlineCapabilitySealPlanV3,
    recent_blockhash: Hash,
    provision: &DirectInlineLookupTableProvisionV3,
    lookup_table: &ObservedAccount,
) -> Result<crate::versioned::VersionedMessagePlanV0, DirectInlineRoutedTransactionErrorV3> {
    if plan.observation.finality != crate::Finality::Finalized
        || lookup_table.observation != plan.observation
        || lookup_table.owner != lookup_table_program::id()
        || lookup_table.executable
        || provision.lookup_table != lookup_table.key
        || provision.authority != plan.payer
        || provision.creation_slot >= plan.observation.slot
        || provision.addresses != plan.lookup_addresses
    {
        return Err(DirectInlineRoutedTransactionErrorV3::LookupTable);
    }
    let expected = lookup_provision_for_addresses(
        plan.payer,
        provision.creation_slot,
        plan.lookup_addresses.clone(),
    )
    .map_err(DirectInlineRoutedTransactionErrorV3::Route)?;
    if provision != &expected {
        return Err(DirectInlineRoutedTransactionErrorV3::LookupTable);
    }
    let table = AddressLookupTable::deserialize(&lookup_table.data)
        .map_err(|_| DirectInlineRoutedTransactionErrorV3::LookupTable)?;
    if table.meta.authority.is_some()
        || table.meta.deactivation_slot != u64::MAX
        || table.meta.last_extended_slot >= plan.observation.slot
        || table.meta.last_extended_slot < provision.creation_slot
        || table.addresses.as_ref() != plan.lookup_addresses.as_slice()
    {
        return Err(DirectInlineRoutedTransactionErrorV3::LookupTable);
    }
    // The seal walks every role's record - authenticating the finalized
    // registry proof and hashing the body of each - before it writes the
    // closure, so like Hot it does not fit Solana's default allocation and
    // must declare its own.
    //
    // ALLOCATION IS TWO GRANTS, AND THIS SITE USED TO SEND ONE. The sentence
    // above says allocation, which is the HEAP; the list below asked only for
    // compute units, so every seal write this compiler produced arrived with
    // the runtime's default 32 KiB ceiling. Trading's adapter declares the seal
    // outer's extended heap profile and then refuses by name when the grant
    // does not arrive -- `TradingSbfError::HeapFrame`, 0x4008, whose own
    // documentation names this exact remedy -- so the first locally executed
    // Direct trade died at its seal stage having consumed 24,033 CU of the
    // 1,399,850 it had asked for and none of the heap it had not. The
    // instructions sysvar the adapter reads to see this grant is already in the
    // frame: it is `HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3` of the Hot fixed
    // prefix, which the seal reuses whole.
    let instructions = [
        solana_compute_budget_interface::ComputeBudgetInstruction::set_compute_unit_limit(
            crate::direct_inline_v3::DIRECT_SEAL_COMPUTE_UNIT_LIMIT_V1,
        ),
        solana_compute_budget_interface::ComputeBudgetInstruction::request_heap_frame(
            dclutch_market::capability_program::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1,
        ),
        plan.instruction.clone(),
    ];
    let message = compile_v0_message(
        plan.payer,
        &instructions,
        recent_blockhash,
        plan.observation,
        core::slice::from_ref(lookup_table),
    )
    .map_err(DirectInlineRoutedTransactionErrorV3::Routing)?;
    require_direct_inline_devnet_account_locks_v3(
        &message,
        &instructions,
        plan.payer,
        lookup_table.key,
        &table,
    )?;
    if message.required_signatures != 1 {
        return Err(DirectInlineRoutedTransactionErrorV3::Signer);
    }
    Ok(message)
}

/// Authenticate a finalized post-transaction seal observation against its plan.
///
/// Transaction-level callers must additionally require the exact submitted
/// message and signature, success, and `meta.returnData == null` before calling
/// this account verifier.
pub fn verify_direct_inline_capability_seal_v3(
    plan: &DirectInlineCapabilitySealPlanV3,
    observed: &ObservedAccount,
) -> Result<(), DirectInlineRouteErrorV3> {
    if observed.observation.finality != crate::Finality::Finalized
        || observed.observation.slot < plan.observation.slot
        || observed.key != plan.seal
        || observed.owner != plan.instruction.program_id
        || observed.executable
        || observed.lamports != plan.expected_final_lamports
        || observed.data != plan.expected_body
    {
        return Err(DirectInlineRouteErrorV3::Seal);
    }
    let closure = SealedDescriptorClosureV1::decode(&observed.data)
        .map_err(DirectInlineRouteErrorV3::CapabilitySeal)?;
    closure
        .require_key(plan.key)
        .map_err(DirectInlineRouteErrorV3::CapabilitySeal)
}

fn seal_row(
    role: SealedRoleV1,
    schema: [u8; 32],
    digest: [u8; 32],
    record: &FinalizedRecordRouteV3,
    registry: Pubkey,
) -> Result<SealedRecordRowV1, DirectInlineRouteErrorV3> {
    if hash(&record.raw.data).to_bytes() != digest
        || record.raw.observation != record.staging.observation
    {
        return Err(DirectInlineRouteErrorV3::Seal);
    }
    authenticate_finalized_record(
        registry,
        &record.raw,
        &FinalizedRecordProof {
            schema_release_id: schema,
            staging_cursor: record.staging.clone(),
        },
    )
    .map_err(DirectInlineRouteErrorV3::ObservationError)?;
    SealedRecordRowV1::new(
        role,
        u32::try_from(record.raw.data.len()).map_err(|_| DirectInlineRouteErrorV3::Seal)?,
        schema,
        digest,
        record.raw.key.to_bytes(),
        record.staging.key.to_bytes(),
    )
    .map_err(DirectInlineRouteErrorV3::CapabilitySeal)
}

fn authenticate_named_route_v3(
    route: &DirectInlineOrdinaryRouteV3,
    authentication: DirectInlineRouteAuthenticationV3,
) -> Result<(), DirectInlineRouteErrorV3> {
    let fixed = &route.fixed;
    let claims = &route.claims;
    let custody = &route.custody;
    let trading = fixed.trading_program.key;
    let claims_program = claims.claims_program.key;
    let custody_program = custody.custody_program.key;
    let token_program = custody.token_program.key;
    let programs = authentication.programs;
    let system = solana_sdk_ids::system_program::ID;
    if authentication.context.outcome_count == 0
        || authentication.seller.intent.outcome >= authentication.context.outcome_count
        || authentication.buyer.intent.outcome >= authentication.context.outcome_count
        || authentication.context.market != fixed.market.key.to_bytes()
        || authentication.context.trading_program != trading.to_bytes()
        || authentication.context.config_content_id != hash(&fixed.config.raw.data).to_bytes()
        || authentication.context.product_record_digest != hash(&fixed.product.raw.data).to_bytes()
        || authentication.context.linked_basis_record_digest
            != hash(&fixed.linked_basis.raw.data).to_bytes()
        || authentication.context.seller_maker_root != route.seller_maker.key.to_bytes()
        || authentication.context.buyer_maker_root != route.buyer_maker.key.to_bytes()
        || authentication.context.system_program != system.to_bytes()
        || authentication.context.system_program != route.system_program.key.to_bytes()
        || authentication.context.custody_authority != custody.custody_authority.key.to_bytes()
        || authentication.context.mint != custody.mint.key.to_bytes()
        || authentication.context.token_program != token_program.to_bytes()
        || authentication.context.seller_token_account != custody.seller_token.key.to_bytes()
        || authentication.context.buyer_token_account != custody.buyer_token.key.to_bytes()
        || authentication.context.fee_token_account != custody.fee_token.key.to_bytes()
        || authentication.context.seller_native_signer != authentication.seller.maker.to_bytes()
        || authentication.context.buyer_native_signer != authentication.buyer.maker.to_bytes()
        || programs.core_program != fixed.core_program.key
        || programs.core_programdata != fixed.core_programdata.key
        || programs.trading_program != trading
        || programs.trading_programdata != fixed.trading_programdata.key
        || programs.registry_program != fixed.registry_program.key
        || programs.claims_program != claims_program
        || programs.claims_programdata != claims.claims_programdata.key
        || programs.custody_program != custody_program
        || programs.rent_program != route.rent_program.key
        || programs.token_program != token_program
        || fixed.market.owner != fixed.core_program.key
        || fixed.root.owner != trading
        || claims.aggregate.owner != claims_program
        || claims.seller_position.owner != claims_program
        || claims.buyer_position.owner != claims_program
        || custody.replay.owner != custody_program
        || custody.mint.owner != token_program
        || custody.buyer_token.owner != token_program
        || custody.seller_token.owner != token_program
        || custody.fee_token.owner != token_program
        || route.lifecycle_rent_credit.owner != route.rent_program.key
        || route.system_program.key != system
        || route.payer.owner != system
        || route.payer.executable
        || !route.payer.data.is_empty()
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    for maker in [&route.seller_maker, &route.buyer_maker] {
        let live = maker.owner == trading && !maker.data.is_empty();
        let vacant = maker.owner == system && maker.data.is_empty() && !maker.executable;
        if !live && !vacant {
            return Err(DirectInlineRouteErrorV3::ChildFrame);
        }
    }
    let program_keys = [
        fixed.core_program.key,
        trading,
        fixed.registry_program.key,
        claims_program,
        custody_program,
        token_program,
        route.rent_program.key,
        route.system_program.key,
    ];
    if has_duplicate(&program_keys)
        || [
            &fixed.core_program,
            &fixed.trading_program,
            &fixed.registry_program,
            &claims.claims_program,
            &custody.custody_program,
            &custody.token_program,
            &route.rent_program,
            &route.system_program,
        ]
        .iter()
        .any(|program| !program.executable)
        || [
            &fixed.core_program,
            &fixed.trading_program,
            &fixed.registry_program,
            &claims.claims_program,
            &custody.custody_program,
            &custody.token_program,
            &route.rent_program,
        ]
        .iter()
        .any(|program| program.owner != bpf_loader_upgradeable::ID)
        || !upgradeable_pair(&fixed.core_program, &fixed.core_programdata)
        || !upgradeable_pair(&fixed.trading_program, &fixed.trading_programdata)
        || !upgradeable_pair(&claims.claims_program, &claims.claims_programdata)
        || !upgradeable_pair(&custody.custody_program, &custody.custody_programdata)
        || custody.custody_programdata.observation != fixed.market.observation
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    for authority in core::iter::once(&claims.caller_authority).chain(&custody.caller_authorities) {
        if authority.owner != system || authority.executable || !authority.data.is_empty() {
            return Err(DirectInlineRouteErrorV3::ChildFrame);
        }
    }
    if authentication.seller.maker.to_bytes() == authentication.buyer.maker.to_bytes()
        || authentication.seller.intent.market != fixed.market.key.to_bytes()
        || authentication.buyer.intent.market != fixed.market.key.to_bytes()
        || authentication.seller.intent.generation != authentication.context.generation
        || authentication.buyer.intent.generation != authentication.context.generation
        || authentication.seller.intent.collateral_account != custody.seller_token.key.to_bytes()
        || authentication.buyer.intent.collateral_account != custody.buyer_token.key.to_bytes()
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let required_distinct = [
        fixed.root.key,
        route.seller_maker.key,
        route.buyer_maker.key,
        claims.aggregate.key,
        claims.seller_position.key,
        claims.buyer_position.key,
        custody.replay.key,
        custody.mint.key,
        custody.buyer_token.key,
        route.payer.key,
        route.lifecycle_rent_credit.key,
    ];
    if has_duplicate(&required_distinct)
        || custody.buyer_token.key == custody.seller_token.key
        || custody.buyer_token.key == custody.fee_token.key
        || [custody.seller_token.key, custody.fee_token.key]
            .iter()
            .any(|key| required_distinct.contains(key))
        || (custody.seller_token.key == custody.fee_token.key
            && custody.seller_token != custody.fee_token)
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let caller_keys = core::iter::once(claims.caller_authority.key)
        .chain(custody.caller_authorities.iter().map(|account| account.key))
        .collect::<Vec<_>>();
    if has_duplicate(&caller_keys)
        || caller_keys.iter().any(|key| {
            required_distinct.contains(key)
                || program_keys.contains(key)
                || *key == custody.seller_token.key
                || *key == custody.fee_token.key
        })
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    Ok(())
}

fn authenticate_direct_child_releases_v3(
    route: &DirectInlineOrdinaryRouteV3,
    authentication: DirectInlineRouteAuthenticationV3,
) -> Result<(), DirectInlineRouteErrorV3> {
    let cache = ActivatedExecutionReleaseSetViewV1::decode(&route.fixed.activation_cache.data)
        .map_err(DirectInlineRouteErrorV3::Registry)?;
    if cache
        .execution_release_set_id()
        .map_err(DirectInlineRouteErrorV3::Registry)?
        .to_bytes()
        != authentication.context.release_set
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let claims = crate::direct_inline_v3::authenticate_direct_role_deployment_v4(
        cache,
        ExecutionRoleV1::Claims,
        &route.claims.claims_program,
        &route.claims.claims_programdata,
    )
    .map_err(DirectInlineRouteErrorV3::DirectInline)?;
    let custody = crate::direct_inline_v3::authenticate_direct_role_deployment_v4(
        cache,
        ExecutionRoleV1::Custody,
        &route.custody.custody_program,
        &route.custody.custody_programdata,
    )
    .map_err(DirectInlineRouteErrorV3::DirectInline)?;
    if claims.release().program().to_bytes() != authentication.programs.claims_program.to_bytes()
        || custody.release().program().to_bytes()
            != authentication.programs.custody_program.to_bytes()
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct DirectMakerRouteFactsV3 {
    created: bool,
    bump_observation: u8,
    bump: u8,
    rent_principal_observation: u64,
    rent_principal: u64,
    rent_beneficiary_observation: [u8; 32],
    rent_beneficiary: [u8; 32],
    next_nonce: u64,
}

/// Authenticate one maker replay root, or project the one the chain will create.
///
/// `rent_beneficiary` is the wallet the Trading program will record as the
/// root's rent owner when it creates it, and it must be the founding lifecycle
/// RentCredit's `refund_wallet()` — never the payer. `hot_v3.rs` builds
/// `MakerReplayFirstUseV1` with `rent_owner: plan.beneficiary`, and that
/// beneficiary comes from the RentCredit, because a maker replay root is a
/// shared structure of the MARKET: if its rent followed whoever paid, a stranger
/// paying their own fees would walk away owning the rent of something the market
/// depends on, and this route deliberately admits that stranger as the payer.
///
/// This projection previously answered `payer`, and that was the same stale
/// model `9d4935d2` corrected in the producer's `maker_facts_v1` — a
/// hand-written duplicate the producer's fix did not reach.
///
/// It stayed hidden for the least comfortable reason: BOTH sides were wrong
/// together. A market's first fill is exactly where this branch runs, because
/// the Hot action is what creates the maker roots (`direct_replay_setup_v1`
/// creates the Custody replay, not these). While the producer also answered
/// `payer`, the two agreed, the equality below passed, and the error survived
/// as far as the poststate check that FILL-2 measured. Correcting only the
/// producer broke the agreement and turned a landed-but-wrong fill into a
/// `ChildFrame` refusal on these 32 bytes, with every other field of the
/// authenticated context already equal. Half a duplicate is worse than none.
fn authenticate_direct_maker_route_v3(
    account: &ObservedAccount,
    trading: Pubkey,
    market: [u8; 32],
    generation: u64,
    maker: Pubkey,
    rent_beneficiary: Pubkey,
    rent: &solana_program::rent::Rent,
) -> Result<DirectMakerRouteFactsV3, DirectInlineRouteErrorV3> {
    let coordinates = DirectCoordinatesV1::new(market, generation)
        .map_err(DirectInlineRouteErrorV3::Successor)?;
    let seeds = MakerReplaySeedsV1::new(coordinates, maker.to_bytes())
        .map_err(DirectInlineRouteErrorV3::Successor)?;
    let (expected, bump) = Pubkey::find_program_address(&seeds.as_slices(), &trading);
    if account.key != expected || account.executable {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    if account.owner == solana_sdk_ids::system_program::ID && account.data.is_empty() {
        return Ok(DirectMakerRouteFactsV3 {
            created: true,
            bump_observation: 0,
            bump,
            rent_principal_observation: 0,
            rent_principal: rent.minimum_balance(DIRECT_MAKER_REPLAY_BYTES_V1),
            rent_beneficiary_observation: [0; 32],
            rent_beneficiary: rent_beneficiary.to_bytes(),
            next_nonce: 0,
        });
    }
    if account.owner != trading {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let replay =
        MakerReplayRootV1::decode(&account.data).map_err(DirectInlineRouteErrorV3::Successor)?;
    if replay.market() != market
        || replay.generation() != generation
        || replay.maker() != maker.to_bytes()
        || replay.bump() != bump
        || account.lamports < replay.rent_principal()
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    Ok(DirectMakerRouteFactsV3 {
        created: false,
        bump_observation: replay.bump(),
        bump: replay.bump(),
        rent_principal_observation: replay.rent_principal(),
        rent_principal: replay.rent_principal(),
        rent_beneficiary_observation: replay.rent_owner(),
        rent_beneficiary: replay.rent_owner(),
        next_nonce: replay.next_nonce(),
    })
}

fn authenticate_direct_ordinary_context_v3(
    route: &DirectInlineOrdinaryRouteV3,
    authentication: DirectInlineRouteAuthenticationV3,
    chain: AuthenticatedDirectHotChainV4,
) -> Result<(), DirectInlineRouteErrorV3> {
    let fixed = &route.fixed;
    let rent =
        decode_rent(&fixed.rent_sysvar).map_err(DirectInlineRouteErrorV3::ObservationError)?;
    let config_digest = hash(&fixed.config.raw.data).to_bytes();
    let config = DirectExecutionConfigV1::decode_selected(
        config_digest,
        config_digest,
        &fixed.config.raw.data,
    )
    .map_err(DirectInlineRouteErrorV3::Successor)?;
    let core =
        CoreState::decode(&fixed.market.data).map_err(DirectInlineRouteErrorV3::MarketCore)?;
    if core.identity.product_id.to_bytes() != chain.product_id {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let realm_digest = hash(&route.custody.realm.raw.data).to_bytes();
    authenticate_finalized_record(
        fixed.registry_program.key,
        &route.custody.realm.raw,
        &FinalizedRecordProof {
            schema_release_id: REALM_SCHEMA_RELEASE_ID_V1,
            staging_cursor: route.custody.realm.staging.clone(),
        },
    )
    .map_err(DirectInlineRouteErrorV3::ObservationError)?;
    let realm =
        RealmV1::decode(&route.custody.realm.raw.data).map_err(DirectInlineRouteErrorV3::Realm)?;
    if realm_digest != core.identity.realm_id.to_bytes()
        || realm.collateral_mint() != &route.custody.mint.key.to_bytes()
        || realm.token_program() != &route.custody.token_program.key.to_bytes()
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }

    // The rent owner a created maker root will carry comes from the founding
    // lifecycle RentCredit the chain itself reads, not from who is paying for
    // this one trade. `authenticate_named_route_v3` has already proven this
    // account is owned by the route's rent program.
    let maker_rent_beneficiary = Pubkey::new_from_array(
        LifecycleRentCreditV2::decode(&route.lifecycle_rent_credit.data)
            .map_err(DirectInlineRouteErrorV3::LifecycleRent)?
            .refund_wallet()
            .to_bytes(),
    );
    let seller = authenticate_direct_maker_route_v3(
        &route.seller_maker,
        fixed.trading_program.key,
        fixed.market.key.to_bytes(),
        core.identity.generation,
        authentication.seller.maker,
        maker_rent_beneficiary,
        &rent,
    )?;
    let buyer = authenticate_direct_maker_route_v3(
        &route.buyer_maker,
        fixed.trading_program.key,
        fixed.market.key.to_bytes(),
        core.identity.generation,
        authentication.buyer.maker,
        maker_rent_beneficiary,
        &rent,
    )?;

    let aggregate = LiabilityBasisMarketViewV2::decode(&route.claims.aggregate.data)
        .map_err(DirectInlineRouteErrorV3::LiabilityBasisState)?;
    let aggregate_key = Pubkey::find_program_address(
        &[LIABILITY_BASIS_MARKET_SEED_V2, fixed.market.key.as_ref()],
        &route.claims.claims_program.key,
    )
    .0;
    if route.claims.aggregate.key != aggregate_key
        || aggregate.logical_market != fixed.market.key.to_bytes()
        || aggregate.release_set != chain.release_set
        || aggregate.registry_program != fixed.registry_program.key.to_bytes()
        || aggregate.product_instance_id != chain.product_id
        || aggregate.basis_id != chain.semantic_basis
        || aggregate.realm_id != realm_digest
        || aggregate.generation != core.identity.generation
        || aggregate.claim_count != chain.outcome_count
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let seller_position = authenticate_direct_position_v3(
        &route.claims.seller_position,
        route.claims.claims_program.key,
        aggregate_key,
        authentication.seller.maker,
        aggregate,
    )?;
    let buyer_position = authenticate_direct_position_v3(
        &route.claims.buyer_position,
        route.claims.claims_program.key,
        aggregate_key,
        authentication.buyer.maker,
        aggregate,
    )?;

    let replay = CustodyReplayV1::decode(&route.custody.replay.data)
        .map_err(DirectInlineRouteErrorV3::Custody)?;
    let replay_seeds = CustodyReplaySeedsV1::new(
        fixed.market.key.to_bytes(),
        chain.release_set,
        CallerRoleV1::Trading,
        route.buyer_maker.key.to_bytes(),
    );
    let replay_key = Pubkey::find_program_address(
        &replay_seeds.as_slices(),
        &route.custody.custody_program.key,
    )
    .0;
    let authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(fixed.market.key.to_bytes(), chain.release_set).as_slices(),
        &route.custody.custody_program.key,
    )
    .0;
    if route.custody.replay.key != replay_key
        || replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != chain.release_set
        || replay.market != fixed.market.key.to_bytes()
        || replay.realm != realm_digest
        || replay.context != route.buyer_maker.key.to_bytes()
        || replay.caller_program != fixed.trading_program.key.to_bytes()
        || replay.generation != core.identity.generation
        || route.custody.custody_authority.key != authority
        || route.custody.custody_authority.owner != solana_sdk_ids::system_program::ID
        || route.custody.custody_authority.executable
        || !route.custody.custody_authority.data.is_empty()
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }

    let mint = Mint::parse(&route.custody.mint.data).map_err(DirectInlineRouteErrorV3::Token)?;
    let buyer_token = TokenAccount::parse(&route.custody.buyer_token.data)
        .map_err(DirectInlineRouteErrorV3::Token)?;
    let seller_token = TokenAccount::parse(&route.custody.seller_token.data)
        .map_err(DirectInlineRouteErrorV3::Token)?;
    let fee_token = TokenAccount::parse(&route.custody.fee_token.data)
        .map_err(DirectInlineRouteErrorV3::Token)?;
    if !mint.is_initialized
        || buyer_token.mint != route.custody.mint.key.to_bytes()
        || seller_token.mint != route.custody.mint.key.to_bytes()
        || fee_token.mint != route.custody.mint.key.to_bytes()
        || buyer_token.owner != authentication.buyer.maker.to_bytes()
        || seller_token.owner != authentication.seller.maker.to_bytes()
        || fee_token.owner != config.fee_recipient()
        || buyer_token.delegate != COption::Some(authority.to_bytes())
        || buyer_token.native_reserve != COption::None
        || seller_token.native_reserve != COption::None
        || fee_token.native_reserve != COption::None
        || buyer_token.state != AccountState::Initialized
        || seller_token.state != AccountState::Initialized
        || fee_token.state != AccountState::Initialized
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }

    let root_tail = fixed
        .root
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(DirectInlineRouteErrorV3::ChildFrame)?;
    let root = DirectRootStateV1::decode(root_tail).map_err(DirectInlineRouteErrorV3::Successor)?;
    let root_phase = match root.phase() {
        DirectRootPhaseV1::Open => 0,
        DirectRootPhaseV1::Retiring => 1,
    };
    let expected = DirectOrdinaryAuthenticatedContextV3 {
        parent_request_digest: hash(
            &compile_direct_inline_request_v3(
                authentication.seller,
                authentication.buyer,
                authentication.fill,
                authentication.execution_price,
            )
            .map_err(DirectInlineRouteErrorV3::DirectInline)?,
        )
        .to_bytes(),
        config_content_id: config_digest,
        config,
        market: fixed.market.key.to_bytes(),
        generation: core.identity.generation,
        outcome_count: chain.outcome_count,
        slot: fixed.market.observation.slot,
        root_phase,
        seller_next_nonce: seller.next_nonce,
        buyer_next_nonce: buyer.next_nonce,
        root_open_maker_count: root.open_maker_root_count(),
        seller_created: seller.created,
        seller_bump_observation: seller.bump_observation,
        seller_bump: seller.bump,
        seller_rent_principal_observation: seller.rent_principal_observation,
        seller_rent_principal: seller.rent_principal,
        buyer_created: buyer.created,
        buyer_bump_observation: buyer.bump_observation,
        buyer_bump: buyer.bump,
        buyer_rent_principal_observation: buyer.rent_principal_observation,
        buyer_rent_principal: buyer.rent_principal,
        claims_market_revision: aggregate.revision,
        seller_position_revision: seller_position.revision,
        buyer_position_revision: buyer_position.revision,
        custody_revision: replay.next_revision,
        release_set: chain.release_set,
        product_record_digest: chain.product_record,
        semantic_basis: chain.semantic_basis,
        linked_basis_record_digest: chain.linked_basis_record,
        trading_program: fixed.trading_program.key.to_bytes(),
        realm: realm_digest,
        mint: route.custody.mint.key.to_bytes(),
        token_program: route.custody.token_program.key.to_bytes(),
        seller_maker_root: route.seller_maker.key.to_bytes(),
        buyer_maker_root: route.buyer_maker.key.to_bytes(),
        system_program: solana_sdk_ids::system_program::ID.to_bytes(),
        custody_authority: authority.to_bytes(),
        seller_rent_beneficiary: seller.rent_beneficiary,
        seller_rent_beneficiary_observation: seller.rent_beneficiary_observation,
        buyer_rent_beneficiary: buyer.rent_beneficiary,
        buyer_rent_beneficiary_observation: buyer.rent_beneficiary_observation,
        fee_token_account: route.custody.fee_token.key.to_bytes(),
        seller_token_account: route.custody.seller_token.key.to_bytes(),
        buyer_token_account: route.custody.buyer_token.key.to_bytes(),
        seller_native_signer: authentication.seller.maker.to_bytes(),
        buyer_native_signer: authentication.buyer.maker.to_bytes(),
    };
    if expected != authentication.context {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    Ok(())
}

fn authenticate_direct_position_v3(
    account: &ObservedAccount,
    claims_program: Pubkey,
    aggregate: Pubkey,
    owner: Pubkey,
    market: LiabilityBasisMarketViewV2,
) -> Result<LiabilityBasisPositionViewV2, DirectInlineRouteErrorV3> {
    let seeds = ProtocolPositionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes())
        .map_err(DirectInlineRouteErrorV3::ProtocolPosition)?;
    let expected = Pubkey::find_program_address(&seeds.as_slices(), &claims_program).0;
    let position = LiabilityBasisPositionViewV2::decode(&account.data)
        .map_err(DirectInlineRouteErrorV3::LiabilityBasisState)?;
    if account.key != expected
        || account.owner != claims_program
        || account.executable
        || position.market_account != aggregate.to_bytes()
        || position.owner != owner.to_bytes()
        || position.basis_id != market.basis_id
        || position.claim_count != market.claim_count
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    Ok(position)
}

fn upgradeable_pair(program: &ObservedAccount, programdata: &ObservedAccount) -> bool {
    program.owner == bpf_loader_upgradeable::ID
        && program.executable
        && programdata.owner == bpf_loader_upgradeable::ID
        && !programdata.executable
        && programdata.key
            == Pubkey::find_program_address(&[program.key.as_ref()], &bpf_loader_upgradeable::ID).0
}

fn has_duplicate(keys: &[Pubkey]) -> bool {
    keys.iter()
        .enumerate()
        .any(|(index, key)| keys.iter().take(index).any(|earlier| earlier == key))
}

/// Order the two `child_caller` hint slots the way the on-chain walk reads them.
///
/// `hot_v3::child_caller_hint_v1` indexes `HotBumpHintsV1::child_caller` by the
/// child INVOCATION ordinal, route-major, so for InlineOrdinary slot 0 is the
/// Claims route and slot 1 is the one enabled Custody route. `custody_slots`
/// names that route; the four-slot arrays are in FIXED shape order and indexing
/// them with a literal is the bug this function exists to make unrepresentable.
///
/// Returns the hint block and which Custody slot it followed.
fn child_caller_hint_slots_v1(
    claims_authority_bump: u8,
    custody_authority_bumps: [u8; 4],
    dispatch: dclutch_trading::inline_candidate_v2::DirectInlineEffectDispatchV2,
) -> Result<([u8; 2], Option<u8>), DirectInlineRouteErrorV3> {
    let enabled = dispatch
        .custody_slots
        .get(..usize::from(dispatch.custody_count))
        .and_then(<[u8]>::first)
        .copied();
    match enabled {
        Some(slot) => Ok((
            [
                claims_authority_bump,
                *custody_authority_bumps
                    .get(usize::from(slot))
                    .ok_or(DirectInlineRouteErrorV3::ChildFrame)?,
            ],
            Some(slot),
        )),
        // No Custody child runs, so the second slot is ABSENT and the walk
        // searches for whatever it does reach: correct, and merely slower.
        None => Ok(([claims_authority_bump, 0], None)),
    }
}

fn derive_child_authorities(
    context: DirectOrdinaryAuthenticatedContextV3,
    family_request: [u8; crate::direct_inline_v3::DIRECT_INLINE_ORDINARY_REQUEST_BYTES_V3],
    _request: DirectInlineOrdinaryRequestV3,
    projected: dclutch_trading::ordinary_route_projection_v3::DirectInlineOrdinaryChildRequestsV3,
) -> Result<DirectInlineChildAuthoritiesV3, DirectInlineRouteErrorV3> {
    let release_set = dclutch_core_contract::ContentId::new(context.release_set)
        .map_err(|_| DirectInlineRouteErrorV3::ChildFrame)?;
    let trading = Pubkey::new_from_array(context.trading_program);
    let claims_request_digest = hash(&projected.claims).to_bytes();
    let claims_seeds = CallerAuthoritySeedsV1::new(
        release_set,
        context.market,
        ExecutionRoleV1::Trading,
        context.parent_request_digest,
        claims_request_digest,
    )
    .map_err(DirectInlineRouteErrorV3::ReleaseSet)?;
    let (claims_authority, claims_authority_bump) =
        Pubkey::find_program_address(&claims_seeds.as_slices(), &trading);
    let mut custody_request_digests = [[0_u8; 32]; 4];
    let mut custody_authorities = [Pubkey::default(); 4];
    let mut custody_authority_bumps = [0_u8; 4];
    for (index, request) in projected.custody.iter().enumerate() {
        let digest = hash(request).to_bytes();
        let seeds = CallerAuthoritySeedsV1::new(
            release_set,
            context.market,
            ExecutionRoleV1::Trading,
            context.buyer_maker_root,
            digest,
        )
        .map_err(DirectInlineRouteErrorV3::ReleaseSet)?;
        *custody_request_digests
            .get_mut(index)
            .ok_or(DirectInlineRouteErrorV3::ChildFrame)? = digest;
        let (authority, bump) = Pubkey::find_program_address(&seeds.as_slices(), &trading);
        *custody_authorities
            .get_mut(index)
            .ok_or(DirectInlineRouteErrorV3::ChildFrame)? = authority;
        *custody_authority_bumps
            .get_mut(index)
            .ok_or(DirectInlineRouteErrorV3::ChildFrame)? = bump;
    }
    if custody_authorities
        .iter()
        .enumerate()
        .any(|(index, value)| {
            custody_authorities
                .iter()
                .take(index)
                .any(|earlier| earlier == value)
        })
    {
        return Err(DirectInlineRouteErrorV3::ChildFrame);
    }
    let (child_caller_bumps, enabled_custody_slot) = child_caller_hint_slots_v1(
        claims_authority_bump,
        custody_authority_bumps,
        projected.dispatch,
    )?;
    Ok(DirectInlineChildAuthoritiesV3 {
        family_request,
        claims_request_digest,
        claims_authority,
        custody_request_digests,
        custody_authorities,
        claims_authority_bump,
        custody_authority_bumps,
        child_caller_bumps,
        enabled_custody_slot,
    })
}

/// Assemble the exact fixed and AccountProfile-packed route from named accounts.
pub fn assemble_direct_inline_ordinary_route_v3(
    route: DirectInlineOrdinaryRouteV3,
    outcome_count: u32,
) -> Result<DirectInlinePhysicalRouteV3, DirectInlineRouteErrorV3> {
    let observation = route.fixed.market.observation;
    let (fixed_accounts, fixed_classes) = fixed_accounts(&route.fixed, observation)?;
    let profile = AccountProfileV2::decode(&route.fixed.account_profile.raw.data)
        .map_err(DirectInlineRouteErrorV3::AccountProfile)?;
    let logical = logical_accounts(&route, observation)?;
    let (runtime_accounts, runtime_classes) = pack_runtime(profile, outcome_count, &logical)?;
    if runtime_accounts.first() != fixed_accounts.get(HOT_ROOT_ACCOUNT_V3)
        || runtime_accounts.get(1) != fixed_accounts.get(HOT_CONFIG_RAW_ACCOUNT_V3)
        || runtime_accounts.get(2) != fixed_accounts.get(HOT_PRODUCT_RAW_ACCOUNT_V3)
        || runtime_accounts.get(3) != fixed_accounts.get(HOT_PORTFOLIO_RAW_ACCOUNT_V3)
        || runtime_accounts.get(4) != fixed_accounts.get(HOT_LINKED_BASIS_RAW_ACCOUNT_V3)
    {
        return Err(DirectInlineRouteErrorV3::Profile);
    }
    Ok(DirectInlinePhysicalRouteV3 {
        fixed_accounts,
        strategy_accounts: Vec::new(),
        runtime_accounts,
        fixed_classes,
        runtime_classes,
        observation,
    })
}

/// Produce the complete ordered key/meta/class closure and exact
/// request-specific frozen-LUT subset. The first semantic occurrence owns
/// ordering; later aliases may only union privileges and must carry the
/// identical explicit class. Signers and invoked programs remain inline.
pub fn classify_direct_inline_ordinary_route_v3(
    route: &DirectInlinePhysicalRouteV3,
) -> Result<(Vec<DirectInlineAddressPlacementV3>, Vec<Pubkey>), DirectInlineRouteErrorV3> {
    if route.fixed_accounts.len() != route.fixed_classes.len()
        || route.runtime_accounts.len() != route.runtime_classes.len()
        || !route.strategy_accounts.is_empty()
    {
        return Err(DirectInlineRouteErrorV3::Profile);
    }
    let mut closure = Vec::new();
    for (meta, class) in route
        .fixed_accounts
        .iter()
        .zip(&route.fixed_classes)
        .chain(route.runtime_accounts.iter().zip(&route.runtime_classes))
    {
        merge_placement(
            &mut closure,
            DirectInlineAddressPlacementV3 {
                address: meta.account.key,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
                class: *class,
            },
        )?;
    }
    let lookup = closure
        .iter()
        .filter(|entry| {
            matches!(
                entry.class,
                DirectInlineAddressClassV3::LookupStable
                    | DirectInlineAddressClassV3::InlineRequestBound
            )
        })
        .map(|entry| entry.address)
        .collect();
    Ok((closure, lookup))
}

/// Replace exactly the six seal-covered staging metas in an already
/// chain-authenticated ordinary Hot report. The report is first required to
/// match the fully distinct physical route and then required to match the
/// canonical execution projection; callers cannot supply either shape.
pub fn project_direct_inline_sealed_execution_report_v3(
    report: &DirectInlineHotReportV3,
    route: &DirectInlineAuthenticatedRouteV3,
) -> Result<DirectInlineHotReportV3, DirectInlineRoutedTransactionErrorV3> {
    authenticate_report_route(report, &route.physical, route.payer)?;
    let mut projected = report.clone();
    // The TRADING instruction, whose account list the seal aliases rewrite --
    // not the evidence instruction that now sits at index 2. This is indexed
    // by the same constant the builder and validator use, because a bare index
    // here silently rewrites the wrong instruction's accounts.
    let trading = projected
        .instructions
        .get_mut(usize::from(DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1))
        .ok_or(DirectInlineRoutedTransactionErrorV3::Instruction)?;
    for (raw, staging) in DIRECT_INLINE_SEALED_EXECUTION_ALIASES_V3 {
        let raw = trading
            .accounts
            .get(raw)
            .cloned()
            .ok_or(DirectInlineRoutedTransactionErrorV3::Instruction)?;
        *trading
            .accounts
            .get_mut(staging)
            .ok_or(DirectInlineRoutedTransactionErrorV3::Instruction)? = raw;
    }
    authenticate_report_route(&projected, &route.sealed_execution_physical, route.payer)?;
    Ok(projected)
}

/// Recompute the complete exterior Hot finalization from one authenticated
/// named route and the canonical distinct-account report.
///
/// The finalizer input is assembled only from the signed intents, the exact
/// selected Transition/Effect output, and account bytes already authenticated
/// by [`assemble_authenticated_direct_inline_ordinary_route_v3`]. This is a
/// bridge, not a second settlement implementation: all candidate arithmetic,
/// child receipt projection, acknowledgement bytes, and poststate digests are
/// produced by `dclutch-trading::direct_finalization_v3`.
pub fn prepare_direct_inline_hot_finalization_v3(
    named_route: DirectInlineOrdinaryRouteV3,
    outcome_count: u32,
    authentication: DirectInlineRouteAuthenticationV3,
    distinct_report: &DirectInlineHotReportV3,
) -> Result<DirectInlineHotFinalizationPlanV3, DirectInlineRouteErrorV3> {
    let authenticated = assemble_authenticated_direct_inline_ordinary_route_v3(
        named_route.clone(),
        outcome_count,
        authentication,
    )?;
    let sealed_report =
        project_direct_inline_sealed_execution_report_v3(distinct_report, &authenticated).map_err(
            |error| {
                DirectInlineRouteErrorV3::Finalization(
                    DirectInlineFinalizationRefusalV3::SealedReportProjection(
                        DirectInlineSealedReportProjectionRefusalV3::from_transaction_error(&error),
                    ),
                )
            },
        )?;
    let sealed_report_facts = DirectInlineSealedReportFactsRefusalV3 {
        selected_program: sealed_report.selected_program != authenticated.chain.selected_program,
        outcome_count: (sealed_report.outcome_count != authenticated.chain.outcome_count)
            .then_some((
                sealed_report.outcome_count,
                authenticated.chain.outcome_count,
            )),
        product_record: sealed_report.product_record != authenticated.chain.product_record,
        trading_artifact_release: sealed_report.trading_artifact_release
            != authenticated.chain.trading_artifact_release,
        checked_manifest_digest: sealed_report.checked_manifest_digest
            != authenticated.checked_manifest_digest,
    };
    if sealed_report_facts.refuses() {
        return Err(DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::SealedReportFacts(sealed_report_facts),
        ));
    }

    let family_request = authenticated.child_authorities.family_request;
    let ordinary_request = match DirectExecutionRequestV3::decode(&family_request, outcome_count)
        .map_err(|_| {
            DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::FamilyRequestDecode {
                    outcome_count,
                    request_bytes: family_request.len(),
                },
            )
        })? {
        DirectExecutionRequestV3::InlineOrdinary(request) => request,
        _ => {
            return Err(DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::FamilyRequestNotInlineOrdinary,
            ));
        }
    };
    let projected = project_direct_inline_ordinary_child_requests_v3(
        ordinary_request,
        authentication.context,
        &named_route.fixed.account_profile.raw.data,
        &named_route.fixed.transition.raw.data,
        &named_route.fixed.effect.raw.data,
    )
    .map_err(|error| {
        DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::ChildRequestProjection(error),
        )
    })?;
    let mut request_bank = Vec::with_capacity(DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3);
    request_bank.extend_from_slice(&projected.claims);
    for request in &projected.custody {
        request_bank.extend_from_slice(request);
    }
    if request_bank.len() != DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3 {
        return Err(DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::RequestBankWidth {
                observed: request_bank.len(),
                expected: DIRECT_INLINE_ORDINARY_REQUEST_BANK_BYTES_V3,
            },
        ));
    }

    let direct =
        direct_inline_finalization_input_v3(&named_route, authentication, ordinary_request)?;
    let context = direct_inline_finalization_context_v3(authentication.context);
    let collateral = direct_inline_finalization_collateral_v3(&named_route)?;
    let accounts = direct_inline_finalization_prestates_v3(&named_route);
    let descriptor =
        CapabilityProgramV4::decode(&named_route.fixed.descriptor.raw.data).map_err(|_| {
            DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::DescriptorDecode {
                    descriptor_bytes: named_route.fixed.descriptor.raw.data.len(),
                },
            )
        })?;
    let strategy = ExecutionStrategyProgramV2::decode(&named_route.fixed.strategy.raw.data)
        .map_err(|_| {
            DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::StrategyDecode {
                    strategy_bytes: named_route.fixed.strategy.raw.data.len(),
                },
            )
        })?;
    let strategy_closure = DirectInlineStrategyClosureRefusalV3 {
        disposition: strategy.disposition() != StrategyDispositionV2::Interpreted,
        certificate_program: strategy.certificate_program().is_some(),
        admission_program: strategy.admission_program().is_some(),
        transition_program: strategy.transition_program().to_bytes()
            != descriptor.transition().program().to_bytes(),
        strategy_digest: descriptor.strategy().program().to_bytes()
            != hash(&named_route.fixed.strategy.raw.data).to_bytes(),
    };
    if strategy_closure.refuses() {
        return Err(DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::StrategyClosure(strategy_closure),
        ));
    }
    let ack = HotExecutionAckInputV3 {
        release_set: authenticated.chain.release_set,
        market: authenticated.chain.market.to_bytes(),
        generation: authentication.context.generation,
        root: named_route.fixed.root.key.to_bytes(),
        request_digest: authentication.context.parent_request_digest,
        root_prestate_digest: hash(&named_route.fixed.root.data).to_bytes(),
        artifacts: HotExecutionArtifactFactsV3 {
            selected_program: authenticated.chain.selected_program,
            account_profile_program: descriptor.account_profile().program().to_bytes(),
            request_profile_program: descriptor.request_profile().program().to_bytes(),
            strategy_program: descriptor.strategy().program().to_bytes(),
            strategy_transition_program: strategy.transition_program().to_bytes(),
            effect_program: descriptor.effect().program().to_bytes(),
            derivation_policy: descriptor.derivation_policy().to_bytes(),
            config: authentication.context.config_content_id,
            product_record: authenticated.chain.product_record,
            linked_basis_record_digest: authenticated.chain.linked_basis_record,
            semantic_basis_id: authenticated.chain.semantic_basis,
            outcome_count: authenticated.chain.outcome_count,
            // The authenticated Strategy is explicitly Interpreted above, so
            // no accelerator transcript participates in this execution.
            strategy_execution_digest: [0; 32],
        },
    };
    let input = DirectInlineFinalizationInputV3 {
        direct: &direct,
        context: &context,
        product_id: authenticated.chain.product_id,
        collateral: &collateral,
        request_bank: &request_bank,
        dispatch: projected.dispatch,
        family_request: &family_request,
        accounts: &accounts,
        programs: DirectInlineFinalizationProgramsV3 {
            trading: authentication.programs.trading_program.to_bytes(),
            claims: authentication.programs.claims_program.to_bytes(),
            custody: authentication.programs.custody_program.to_bytes(),
            token: authentication.programs.token_program.to_bytes(),
        },
        ack: &ack,
    };
    let finalization = prepare_direct_inline_finalization_v3(&input).map_err(|error| {
        DirectInlineRouteErrorV3::Finalization(DirectInlineFinalizationRefusalV3::Finalizer {
            error,
            candidate: rederive_direct_inline_candidate_refusal_v3(&input, error),
        })
    })?;
    let mut poststates: [DirectInlineExpectedPoststateV3; DIRECT_INLINE_POSTSTATE_COUNT_V3] =
        finalization
            .poststates
            .map(|commitment| DirectInlineExpectedPoststateV3 {
                commitment,
                data: Vec::new(),
            });
    for (index, expected) in poststates.iter_mut().enumerate() {
        let role = DirectInlinePoststateRoleV3::from_index(index).map_err(|_| {
            DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::PoststateRoleIndex { index },
            )
        })?;
        if role != expected.commitment.role {
            return Err(DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::PoststateRoleOrder {
                    index,
                    observed: expected.commitment.role,
                    expected: role,
                },
            ));
        }
        expected.data.resize(
            usize::try_from(expected.commitment.data_len).map_err(|_| {
                DirectInlineRouteErrorV3::Finalization(
                    DirectInlineFinalizationRefusalV3::PoststateWidth {
                        index,
                        data_len: expected.commitment.data_len,
                    },
                )
            })?,
            0,
        );
        project_direct_inline_account_poststate_v3(&input, role, &mut expected.data).map_err(
            |error| {
                DirectInlineRouteErrorV3::Finalization(
                    DirectInlineFinalizationRefusalV3::PoststateProjection {
                        index,
                        role,
                        error,
                        candidate: rederive_direct_inline_candidate_refusal_v3(&input, error),
                    },
                )
            },
        )?;
        if hash(&expected.data).to_bytes() != expected.commitment.data_digest {
            return Err(DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::PoststateDigest { index, role },
            ));
        }
    }
    Ok(DirectInlineHotFinalizationPlanV3 {
        sealed_report,
        finalization,
        poststates,
    })
}

/// Name the clause behind a `Candidate` refusal by re-running the same public
/// candidate partition on the same inputs.
///
/// `DirectFinalizationErrorV3::Candidate` is the next collapse down: six sites
/// in `dclutch-trading` share it, and the largest of them discards a whole
/// nine-variant `DirectInlineCandidateErrorV2` through a `map_err`. That crate
/// compiles into the Trading SBF program, where widening a refusal is a stack
/// frame's worth of on-chain cost for a diagnosis nobody on chain can read, so
/// the discard is defensible THERE and only there. This host is where the
/// operator reads the message, so this host pays for it: on the refusal path
/// only, never on the success path, the exact same public entry point is called
/// with the exact same five arguments the finalizer just used. This is not a
/// second reader of the accounts -- there is no reimplemented rule here to
/// drift -- it is the one reader, asked again with its own answer kept.
///
/// `None` means the candidate partition itself accepts, so the `Candidate` came
/// from one of the other five sites (a settlement encode, or the Custody
/// receipt join) rather than from the partition.
fn rederive_direct_inline_candidate_refusal_v3(
    input: &DirectInlineFinalizationInputV3<'_>,
    error: DirectFinalizationErrorV3,
) -> Option<DirectInlineCandidateErrorV2> {
    if error != DirectFinalizationErrorV3::Candidate {
        return None;
    }
    prepare_and_verify_inline_effect_partition_v2(
        *input.direct,
        *input.context,
        *input.collateral,
        input.request_bank,
        input.dispatch,
    )
    .err()
}

fn direct_inline_finalization_input_v3(
    route: &DirectInlineOrdinaryRouteV3,
    authentication: DirectInlineRouteAuthenticationV3,
    request: DirectInlineOrdinaryRequestV3,
) -> Result<InlineOrdinaryInputV2, DirectInlineRouteErrorV3> {
    let root_tail = route
        .fixed
        .root
        .data
        .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
        .ok_or(DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::RootHeaderWidth {
                observed: route.fixed.root.data.len(),
                header: CAPABILITY_ROOT_HEADER_BYTES_V1,
            },
        ))?;
    let root = DirectRootStateV1::decode(root_tail).map_err(|_| {
        DirectInlineRouteErrorV3::Finalization(DirectInlineFinalizationRefusalV3::RootStateDecode {
            tail_bytes: root_tail.len(),
        })
    })?;
    let seller = direct_inline_finalization_participant_v3(
        DirectInlineParticipantSideV3::Seller,
        &route.seller_maker,
        authentication.seller,
        authentication.context.seller_created,
        authentication.context.seller_bump,
        authentication.context.seller_rent_beneficiary,
        authentication.context.seller_rent_principal,
    )?;
    let buyer = direct_inline_finalization_participant_v3(
        DirectInlineParticipantSideV3::Buyer,
        &route.buyer_maker,
        authentication.buyer,
        authentication.context.buyer_created,
        authentication.context.buyer_bump,
        authentication.context.buyer_rent_beneficiary,
        authentication.context.buyer_rent_principal,
    )?;
    let intents = DirectInlineRequestIntentRefusalV3 {
        seller_maker: request.seller.maker != authentication.seller.maker.to_bytes(),
        seller_intent: request.seller.intent != authentication.seller.intent,
        buyer_maker: request.buyer.maker != authentication.buyer.maker.to_bytes(),
        buyer_intent: request.buyer.intent != authentication.buyer.intent,
        fill: (request.fill != authentication.fill).then_some((request.fill, authentication.fill)),
        execution_price: (request.execution_price != authentication.execution_price)
            .then_some((request.execution_price, authentication.execution_price)),
    };
    if intents.refuses() {
        return Err(DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::RequestIntents(intents),
        ));
    }
    Ok(InlineOrdinaryInputV2 {
        root,
        seller,
        buyer,
        execution: InlineExecutionV2 {
            config: authentication.context.config,
            outcome_count: authentication.context.outcome_count,
            slot: authentication.context.slot,
            fill: request.fill,
            execution_price: request.execution_price,
        },
    })
}

fn direct_inline_finalization_participant_v3(
    side: DirectInlineParticipantSideV3,
    maker_replay: &ObservedAccount,
    signed: SignedDirectIntentV3,
    created: bool,
    bump: u8,
    rent_owner: [u8; 32],
    rent_principal: u64,
) -> Result<InlineParticipantV2, DirectInlineRouteErrorV3> {
    let authenticated =
        AuthenticatedCompactIntentV2::from_adjacent_ed25519(signed.maker.to_bytes(), signed.intent)
            .map_err(|_| {
                DirectInlineRouteErrorV3::Finalization(
                    DirectInlineFinalizationRefusalV3::ParticipantIntent { side },
                )
            })?;
    let (maker_replay, first_use) = if created {
        let vacancy = DirectInlineVacancyRefusalV3 {
            owner: maker_replay.owner != solana_sdk_ids::system_program::ID,
            executable: maker_replay.executable,
            data_len: (!maker_replay.data.is_empty()).then_some(maker_replay.data.len()),
            rent_beneficiary: rent_owner == [0; 32],
            rent_principal: rent_principal == 0,
        };
        if vacancy.refuses() {
            return Err(DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::ParticipantVacancy {
                    side,
                    clauses: vacancy,
                },
            ));
        }
        (
            MakerReplayObservationV1::Vacant(MakerReplayVacancyV1::new(
                bump,
                maker_replay.lamports,
            )),
            Some(MakerReplayFirstUseV1 {
                rent_owner,
                rent_principal,
            }),
        )
    } else {
        (
            MakerReplayObservationV1::Existing(
                MakerReplayRootV1::decode(&maker_replay.data).map_err(|_| {
                    DirectInlineRouteErrorV3::Finalization(
                        DirectInlineFinalizationRefusalV3::ParticipantMakerReplayDecode {
                            side,
                            data_bytes: maker_replay.data.len(),
                            expected_bytes: DIRECT_MAKER_REPLAY_BYTES_V1,
                        },
                    )
                })?,
            ),
            None,
        )
    };
    Ok(InlineParticipantV2 {
        authenticated,
        maker_replay,
        first_use,
    })
}

fn direct_inline_finalization_context_v3(
    context: DirectOrdinaryAuthenticatedContextV3,
) -> DirectInlineCandidateContextV2 {
    DirectInlineCandidateContextV2 {
        release_set: context.release_set,
        market: context.market,
        generation: context.generation,
        outcome_count: context.outcome_count,
        product_record_digest: context.product_record_digest,
        semantic_basis_id: context.semantic_basis,
        linked_basis_record_digest: context.linked_basis_record_digest,
        trading_program: context.trading_program,
        realm: context.realm,
        mint: context.mint,
        token_program: context.token_program,
        buyer_maker_root: context.buyer_maker_root,
        custody_authority: context.custody_authority,
        parent_request_digest: context.parent_request_digest,
        claims_market_revision: context.claims_market_revision,
        seller_position_revision: context.seller_position_revision,
        buyer_position_revision: context.buyer_position_revision,
        custody_revision: context.custody_revision,
    }
}

fn direct_inline_finalization_collateral_v3(
    route: &DirectInlineOrdinaryRouteV3,
) -> Result<DirectInlineCollateralFrameV2, DirectInlineRouteErrorV3> {
    let buyer = TokenAccount::parse(&route.custody.buyer_token.data).map_err(|_| {
        DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::CollateralTokenParse {
                role: DirectInlineCollateralRoleV3::Buyer,
                data_bytes: route.custody.buyer_token.data.len(),
            },
        )
    })?;
    let seller = TokenAccount::parse(&route.custody.seller_token.data).map_err(|_| {
        DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::CollateralTokenParse {
                role: DirectInlineCollateralRoleV3::Seller,
                data_bytes: route.custody.seller_token.data.len(),
            },
        )
    })?;
    let fee = TokenAccount::parse(&route.custody.fee_token.data).map_err(|_| {
        DirectInlineRouteErrorV3::Finalization(
            DirectInlineFinalizationRefusalV3::CollateralTokenParse {
                role: DirectInlineCollateralRoleV3::Fee,
                data_bytes: route.custody.fee_token.data.len(),
            },
        )
    })?;
    let delegate = match buyer.delegate {
        COption::Some(delegate) => delegate,
        COption::None => {
            return Err(DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::BuyerCollateralDelegateAbsent {
                    account: route.custody.buyer_token.key,
                    owner: Pubkey::new_from_array(buyer.owner),
                },
            ));
        }
    };
    Ok(DirectInlineCollateralFrameV2 {
        buyer_source: DirectExternalDebitV2 {
            account: route.custody.buyer_token.key.to_bytes(),
            owner: buyer.owner,
            delegate,
            delegated_amount: buyer.delegated_amount,
            balance: buyer.amount,
        },
        seller_destination: DirectExternalCollateralV2 {
            account: route.custody.seller_token.key.to_bytes(),
            owner: seller.owner,
            balance: seller.amount,
        },
        fee_destination: DirectExternalCollateralV2 {
            account: route.custody.fee_token.key.to_bytes(),
            owner: fee.owner,
            balance: fee.amount,
        },
    })
}

fn direct_inline_finalization_prestates_v3(
    route: &DirectInlineOrdinaryRouteV3,
) -> DirectInlineAccountPrestatesV3<'_> {
    DirectInlineAccountPrestatesV3 {
        root: direct_inline_account_prestate_v3(&route.fixed.root),
        seller_maker_replay: direct_inline_account_prestate_v3(&route.seller_maker),
        buyer_maker_replay: direct_inline_account_prestate_v3(&route.buyer_maker),
        claims_market: direct_inline_account_prestate_v3(&route.claims.aggregate),
        seller_position: direct_inline_account_prestate_v3(&route.claims.seller_position),
        buyer_position: direct_inline_account_prestate_v3(&route.claims.buyer_position),
        custody_replay: direct_inline_account_prestate_v3(&route.custody.replay),
        buyer_token: direct_inline_account_prestate_v3(&route.custody.buyer_token),
        seller_token: direct_inline_account_prestate_v3(&route.custody.seller_token),
        fee_token: direct_inline_account_prestate_v3(&route.custody.fee_token),
    }
}

fn direct_inline_account_prestate_v3(
    account: &ObservedAccount,
) -> DirectInlineAccountPrestateV3<'_> {
    DirectInlineAccountPrestateV3 {
        address: account.key.to_bytes(),
        owner: account.owner.to_bytes(),
        lamports: account.lamports,
        data: &account.data,
    }
}

/// Build the exact create/extend/freeze plan for one authenticated Direct route.
///
/// This function only builds instructions. The caller must durably persist the
/// complete plan before signing or submitting `create`, journal every signed
/// transaction ID before submission, finish after `freeze`, and re-observe the
/// frozen table at a later finalized slot before compiling the trade.
pub fn build_direct_inline_lookup_table_provision_v3(
    route: &DirectInlineAuthenticatedRouteV3,
    authority: Pubkey,
    creation_slot: u64,
) -> Result<DirectInlineLookupTableProvisionV3, DirectInlineRouteErrorV3> {
    if authority != route.payer || creation_slot != route.physical.observation.slot {
        return Err(DirectInlineRouteErrorV3::Observation);
    }
    build_lookup_table_provision(&route.sealed_execution_physical, authority, creation_slot)
}

/// Re-derive an already journaled request-specific table plan at a later
/// finalized observation. The creation slot remains immutable; no persisted
/// address or instruction is trusted as a parallel truth.
pub fn rederive_direct_inline_lookup_table_provision_v3(
    route: &DirectInlineAuthenticatedRouteV3,
    authority: Pubkey,
    creation_slot: u64,
) -> Result<DirectInlineLookupTableProvisionV3, DirectInlineRouteErrorV3> {
    if authority != route.payer
        || creation_slot == 0
        || creation_slot > route.physical.observation.slot
    {
        return Err(DirectInlineRouteErrorV3::Observation);
    }
    build_lookup_table_provision(&route.sealed_execution_physical, authority, creation_slot)
}

fn build_lookup_table_provision(
    route: &DirectInlinePhysicalRouteV3,
    authority: Pubkey,
    creation_slot: u64,
) -> Result<DirectInlineLookupTableProvisionV3, DirectInlineRouteErrorV3> {
    if authority == Pubkey::default()
        || creation_slot == 0
        || route.observation.finality != crate::Finality::Finalized
    {
        return Err(DirectInlineRouteErrorV3::Observation);
    }
    let (closure, addresses) = classify_direct_inline_ordinary_route_v3(route)?;
    if addresses.is_empty()
        || addresses.len() > 256
        || closure.iter().any(|placement| {
            placement.is_signer
                && matches!(
                    placement.class,
                    DirectInlineAddressClassV3::LookupStable
                        | DirectInlineAddressClassV3::InlineRequestBound
                )
        })
    {
        return Err(DirectInlineRouteErrorV3::Profile);
    }
    lookup_provision_for_addresses(authority, creation_slot, addresses)
}

fn lookup_provision_for_addresses(
    authority: Pubkey,
    creation_slot: u64,
    addresses: Vec<Pubkey>,
) -> Result<DirectInlineLookupTableProvisionV3, DirectInlineRouteErrorV3> {
    if authority == Pubkey::default()
        || creation_slot == 0
        || addresses.is_empty()
        || addresses.len() > 256
        || has_duplicate(&addresses)
    {
        return Err(DirectInlineRouteErrorV3::Profile);
    }
    let (create, lookup_table) = create_lookup_table(authority, authority, creation_slot);
    let extensions = addresses
        .chunks(crate::versioned::EXTEND_ADDRESSES_PER_TRANSACTION_V1)
        .map(|page| extend_lookup_table(lookup_table, authority, Some(authority), page.to_vec()))
        .collect();
    Ok(DirectInlineLookupTableProvisionV3 {
        lookup_table,
        authority,
        creation_slot,
        addresses,
        create,
        extensions,
        freeze: freeze_lookup_table(lookup_table, authority),
    })
}

/// Compile an ordinary Direct report through its exact request-specific frozen ALT.
///
/// The route classes are assigned before alias packing. The sole table contains
/// the exact first-use union of stable and request-bound coordinates; signers
/// and program IDs remain inline. The table must already be frozen and active
/// at a later finalized slot. This function never creates, mutates, signs, or
/// submits it.
pub fn compile_direct_inline_routed_v0_v3(
    report: &DirectInlineHotReportV3,
    route: &DirectInlinePhysicalRouteV3,
    payer: Pubkey,
    recent_blockhash: Hash,
    provision: &DirectInlineLookupTableProvisionV3,
    lookup_table: &ObservedAccount,
) -> Result<DirectInlineHotTransactionPlanV3, DirectInlineRoutedTransactionErrorV3> {
    if payer == Pubkey::default()
        || report.observation != route.observation
        || lookup_table.observation != route.observation
        || route.observation.finality != crate::Finality::Finalized
        || route.observation.slot == 0
        || lookup_table.owner != lookup_table_program::id()
        || lookup_table.executable
        || report.trading_artifact_release == [0; 32]
        || report.checked_manifest_digest == [0; 32]
    {
        return Err(DirectInlineRoutedTransactionErrorV3::Snapshot);
    }
    require_direct_inline_sealed_execution_shape_v3(route)
        .map_err(DirectInlineRoutedTransactionErrorV3::Route)?;
    let (closure, lookup) = classify_direct_inline_ordinary_route_v3(route)
        .map_err(DirectInlineRoutedTransactionErrorV3::Route)?;
    if closure.iter().any(|placement| {
        matches!(
            placement.class,
            DirectInlineAddressClassV3::LookupStable
                | DirectInlineAddressClassV3::InlineRequestBound
        ) && placement.is_signer
    }) {
        return Err(DirectInlineRoutedTransactionErrorV3::Signer);
    }
    let expected_provision = build_lookup_table_provision(route, payer, provision.creation_slot)
        .map_err(DirectInlineRoutedTransactionErrorV3::Route)?;
    if provision != &expected_provision
        || provision.addresses != lookup
        || provision.lookup_table != lookup_table.key
        || provision.creation_slot >= route.observation.slot
    {
        return Err(DirectInlineRoutedTransactionErrorV3::LookupTable);
    }

    authenticate_report_route(report, route, payer)?;
    let table = AddressLookupTable::deserialize(&lookup_table.data)
        .map_err(|_| DirectInlineRoutedTransactionErrorV3::LookupTable)?;
    if table.meta.authority.is_some()
        || table.meta.deactivation_slot != u64::MAX
        || table.meta.last_extended_slot >= route.observation.slot
        || table.meta.last_extended_slot < provision.creation_slot
        || table.addresses.as_ref() != lookup.as_slice()
    {
        return Err(DirectInlineRoutedTransactionErrorV3::LookupTable);
    }
    let message = compile_v0_message(
        payer,
        &report.instructions,
        recent_blockhash,
        report.observation,
        core::slice::from_ref(lookup_table),
    )
    .map_err(DirectInlineRoutedTransactionErrorV3::Routing)?;
    require_direct_inline_devnet_account_locks_v3(
        &message,
        &report.instructions,
        payer,
        lookup_table.key,
        &table,
    )?;
    let required_signers = expected_signers(report, payer)?;
    if usize::from(message.required_signatures) != required_signers.len() {
        return Err(DirectInlineRoutedTransactionErrorV3::Signer);
    }
    Ok(DirectInlineHotTransactionPlanV3 {
        message,
        required_signers,
        outcome_count: report.outcome_count,
        selected_program_schema: report.selected_program_schema,
        selected_program: report.selected_program,
        trading_artifact_release: report.trading_artifact_release,
        checked_manifest_digest: report.checked_manifest_digest,
    })
}

fn require_direct_inline_devnet_account_locks_v3(
    message: &crate::versioned::VersionedMessagePlanV0,
    instructions: &[Instruction],
    payer: Pubkey,
    lookup_table: Pubkey,
    table: &AddressLookupTable<'_>,
) -> Result<(), DirectInlineRoutedTransactionErrorV3> {
    let solana_message::VersionedMessage::V0(versioned) = &message.message else {
        return Err(DirectInlineRoutedTransactionErrorV3::AccountLocks);
    };
    let total = versioned
        .account_keys
        .len()
        .checked_add(message.loaded_addresses)
        .ok_or(DirectInlineRoutedTransactionErrorV3::AccountLocks)?;
    admit_direct_inline_devnet_account_lock_count_v3(total)?;

    let required = usize::from(versioned.header.num_required_signatures);
    let readonly_signed = usize::from(versioned.header.num_readonly_signed_accounts);
    let readonly_unsigned = usize::from(versioned.header.num_readonly_unsigned_accounts);
    if required > versioned.account_keys.len()
        || readonly_signed > required
        || readonly_unsigned > versioned.account_keys.len().saturating_sub(required)
    {
        return Err(DirectInlineRoutedTransactionErrorV3::AccountLocks);
    }
    let writable_signed = required.saturating_sub(readonly_signed);
    let writable_unsigned_end = versioned
        .account_keys
        .len()
        .saturating_sub(readonly_unsigned);
    let mut actual = Vec::with_capacity(total);
    for (index, key) in versioned.account_keys.iter().copied().enumerate() {
        actual.push((
            key,
            index < required,
            index < writable_signed || (index >= required && index < writable_unsigned_end),
        ));
    }
    if versioned.address_table_lookups.len() != 1
        || versioned
            .address_table_lookups
            .first()
            .is_none_or(|lookup| lookup.account_key != lookup_table)
    {
        return Err(DirectInlineRoutedTransactionErrorV3::AccountLocks);
    }
    let lookup = versioned
        .address_table_lookups
        .first()
        .ok_or(DirectInlineRoutedTransactionErrorV3::AccountLocks)?;
    for (indexes, writable) in [
        (lookup.writable_indexes.as_slice(), true),
        (lookup.readonly_indexes.as_slice(), false),
    ] {
        for index in indexes {
            let key = table
                .addresses
                .get(usize::from(*index))
                .copied()
                .ok_or(DirectInlineRoutedTransactionErrorV3::AccountLocks)?;
            actual.push((key, false, writable));
        }
    }
    if actual.len() != total
        || has_duplicate(&actual.iter().map(|entry| entry.0).collect::<Vec<_>>())
    {
        return Err(DirectInlineRoutedTransactionErrorV3::AccountLocks);
    }

    let mut expected = vec![(payer, true, true)];
    for instruction in instructions {
        merge_key_privilege_v3(&mut expected, instruction.program_id, false, false);
        for meta in &instruction.accounts {
            merge_key_privilege_v3(&mut expected, meta.pubkey, meta.is_signer, meta.is_writable);
        }
    }
    actual.sort_unstable_by_key(|entry| entry.0.to_bytes());
    expected.sort_unstable_by_key(|entry| entry.0.to_bytes());
    if actual != expected {
        return Err(DirectInlineRoutedTransactionErrorV3::AccountLocks);
    }
    Ok(())
}

fn merge_key_privilege_v3(
    union: &mut Vec<(Pubkey, bool, bool)>,
    key: Pubkey,
    signer: bool,
    writable: bool,
) {
    if let Some(entry) = union.iter_mut().find(|entry| entry.0 == key) {
        entry.1 |= signer;
        entry.2 |= writable;
    } else {
        union.push((key, signer, writable));
    }
}

fn admit_direct_inline_devnet_account_lock_count_v3(
    total: usize,
) -> Result<(), DirectInlineRoutedTransactionErrorV3> {
    if total == 0 || total > DIRECT_INLINE_DEVNET_ACCOUNT_LOCK_LIMIT_V3 {
        return Err(DirectInlineRoutedTransactionErrorV3::AccountLocks);
    }
    Ok(())
}

fn authenticate_report_route(
    report: &DirectInlineHotReportV3,
    route: &DirectInlinePhysicalRouteV3,
    payer: Pubkey,
) -> Result<(), DirectInlineRoutedTransactionErrorV3> {
    validate_direct_hot_instruction_sequence_v4(
        dclutch_trading::execution_v3::DirectExecutionActionV3::InlineOrdinary,
        report.outcome_count,
        &report.hot_instruction_data,
        &report.instructions,
    )
    .map_err(DirectInlineRoutedTransactionErrorV3::DirectInlineTransaction)?;
    let [_compute, _heap, _native, trading] = &report.instructions;
    if trading.data != report.hot_instruction_data {
        return Err(DirectInlineRoutedTransactionErrorV3::Instruction);
    }
    let expected_accounts = route
        .fixed_accounts
        .iter()
        .chain(route.strategy_accounts.iter())
        .chain(route.runtime_accounts.iter().skip(
            dclutch_market::capability_program::hot_v3::HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3,
        ))
        .map(|meta| AccountMeta {
            pubkey: meta.account.key,
            is_signer: meta.is_signer,
            is_writable: meta.is_writable,
        })
        .collect::<Vec<_>>();
    if trading.accounts != expected_accounts
        || route
            .fixed_accounts
            .get(HOT_TRADING_PROGRAM_ACCOUNT_V3)
            .is_none_or(|account| trading.program_id != account.account.key)
    {
        return Err(DirectInlineRoutedTransactionErrorV3::Instruction);
    }
    let expected = expected_accounts.iter().filter(|meta| meta.is_signer).fold(
        Vec::new(),
        |mut signers, meta| {
            if !signers.contains(&meta.pubkey) {
                signers.push(meta.pubkey);
            }
            signers
        },
    );
    if expected != report.required_instruction_signers
        || !expected.contains(&payer)
        || expected.iter().any(|signer| *signer == Pubkey::default())
    {
        return Err(DirectInlineRoutedTransactionErrorV3::Signer);
    }
    Ok(())
}

fn expected_signers(
    report: &DirectInlineHotReportV3,
    payer: Pubkey,
) -> Result<Vec<Pubkey>, DirectInlineRoutedTransactionErrorV3> {
    let mut required_signers = vec![payer];
    for signer in &report.required_instruction_signers {
        if *signer == Pubkey::default() {
            return Err(DirectInlineRoutedTransactionErrorV3::Signer);
        }
        if !required_signers.contains(signer) {
            required_signers.push(*signer);
        }
    }
    Ok(required_signers)
}

fn merge_placement(
    closure: &mut Vec<DirectInlineAddressPlacementV3>,
    candidate: DirectInlineAddressPlacementV3,
) -> Result<(), DirectInlineRouteErrorV3> {
    if let Some(existing) = closure
        .iter_mut()
        .find(|existing| existing.address == candidate.address)
    {
        if existing.class != candidate.class {
            return Err(DirectInlineRouteErrorV3::Profile);
        }
        existing.is_signer |= candidate.is_signer;
        existing.is_writable |= candidate.is_writable;
    } else {
        closure.push(candidate);
    }
    Ok(())
}

fn fixed_accounts(
    fixed: &DirectHotFixedRouteV3,
    observation: Observation,
) -> Result<(Vec<ObservedAccountMetaV3>, Vec<DirectInlineAddressClassV3>), DirectInlineRouteErrorV3>
{
    let mut accounts = vec![None; HOT_FIXED_ACCOUNT_COUNT_V3];
    let mut classes = vec![None; HOT_FIXED_ACCOUNT_COUNT_V3];
    let mut put = |index: usize,
                   account: &ObservedAccount,
                   writable: bool,
                   class: DirectInlineAddressClassV3| {
        if account.observation != observation {
            return Err(DirectInlineRouteErrorV3::Observation);
        }
        insert_once(
            &mut accounts,
            index,
            meta(account, false, writable),
            DirectInlineRouteErrorV3::FixedFrame,
        )?;
        insert_once(
            &mut classes,
            index,
            class,
            DirectInlineRouteErrorV3::FixedFrame,
        )
    };
    put(
        HOT_MARKET_ACCOUNT_V3,
        &fixed.market,
        false,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    put(
        HOT_ROOT_ACCOUNT_V3,
        &fixed.root,
        true,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    for (raw_index, staging_index, record) in [
        (
            HOT_MANIFEST_RAW_ACCOUNT_V3,
            HOT_MANIFEST_STAGING_ACCOUNT_V3,
            &fixed.manifest,
        ),
        (
            HOT_PROGRAM_SET_RAW_ACCOUNT_V3,
            HOT_PROGRAM_SET_STAGING_ACCOUNT_V3,
            &fixed.program_set,
        ),
        (
            HOT_DESCRIPTOR_RAW_ACCOUNT_V3,
            HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
            &fixed.descriptor,
        ),
        (
            HOT_CONFIG_RAW_ACCOUNT_V3,
            HOT_CONFIG_STAGING_ACCOUNT_V3,
            &fixed.config,
        ),
        (
            HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3,
            HOT_ACCOUNT_PROFILE_STAGING_ACCOUNT_V3,
            &fixed.account_profile,
        ),
        (
            HOT_REQUEST_PROFILE_RAW_ACCOUNT_V3,
            HOT_REQUEST_PROFILE_STAGING_ACCOUNT_V3,
            &fixed.request_profile,
        ),
        (
            HOT_TRANSITION_RAW_ACCOUNT_V3,
            HOT_TRANSITION_STAGING_ACCOUNT_V3,
            &fixed.transition,
        ),
        (
            HOT_EFFECT_RAW_ACCOUNT_V3,
            HOT_EFFECT_STAGING_ACCOUNT_V3,
            &fixed.effect,
        ),
        (
            HOT_LIFECYCLE_RAW_ACCOUNT_V3,
            HOT_LIFECYCLE_STAGING_ACCOUNT_V3,
            &fixed.lifecycle,
        ),
        (
            HOT_STRATEGY_RAW_ACCOUNT_V3,
            HOT_STRATEGY_STAGING_ACCOUNT_V3,
            &fixed.strategy,
        ),
        (
            HOT_PRODUCT_RAW_ACCOUNT_V3,
            HOT_PRODUCT_STAGING_ACCOUNT_V3,
            &fixed.product,
        ),
        (
            HOT_RESULT_DOMAIN_RAW_ACCOUNT_V3,
            HOT_RESULT_DOMAIN_STAGING_ACCOUNT_V3,
            &fixed.result_domain,
        ),
        (
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            HOT_PORTFOLIO_STAGING_ACCOUNT_V3,
            &fixed.portfolio,
        ),
        (
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
            HOT_LINKED_BASIS_STAGING_ACCOUNT_V3,
            &fixed.linked_basis,
        ),
    ] {
        put(
            raw_index,
            &record.raw,
            false,
            DirectInlineAddressClassV3::LookupStable,
        )?;
        put(
            staging_index,
            &record.staging,
            false,
            DirectInlineAddressClassV3::LookupStable,
        )?;
    }
    for (index, account) in [
        (HOT_ACTIVATION_CACHE_ACCOUNT_V3, &fixed.activation_cache),
        (HOT_CORE_PROGRAM_ACCOUNT_V3, &fixed.core_program),
        (HOT_CORE_PROGRAMDATA_ACCOUNT_V3, &fixed.core_programdata),
        (HOT_TRADING_PROGRAM_ACCOUNT_V3, &fixed.trading_program),
        (
            HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
            &fixed.trading_programdata,
        ),
        (HOT_REGISTRY_PROGRAM_ACCOUNT_V3, &fixed.registry_program),
        (HOT_RENT_SYSVAR_ACCOUNT_V3, &fixed.rent_sysvar),
        (
            HOT_INSTRUCTIONS_SYSVAR_ACCOUNT_V3,
            &fixed.instructions_sysvar,
        ),
        (HOT_CAPABILITY_SEAL_ACCOUNT_V3, &fixed.capability_seal),
    ] {
        let class = match index {
            HOT_TRADING_PROGRAM_ACCOUNT_V3 => DirectInlineAddressClassV3::InlineProgram,
            _ => DirectInlineAddressClassV3::LookupStable,
        };
        put(index, account, false, class)?;
    }
    let accounts = accounts
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(DirectInlineRouteErrorV3::FixedFrame)?;
    let classes = classes
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(DirectInlineRouteErrorV3::FixedFrame)?;
    Ok((accounts, classes))
}

fn logical_accounts(
    route: &DirectInlineOrdinaryRouteV3,
    observation: Observation,
) -> Result<Vec<ClassifiedAccountV3>, DirectInlineRouteErrorV3> {
    let logical_count = usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3);
    let mut logical = vec![None; logical_count];
    let mut put = |index: usize, account: &ObservedAccount, class: DirectInlineAddressClassV3| {
        if account.observation != observation {
            return Err(DirectInlineRouteErrorV3::Observation);
        }
        insert_once(
            &mut logical,
            index,
            ClassifiedAccountV3 {
                account: account.clone(),
                class,
            },
            DirectInlineRouteErrorV3::ChildFrame,
        )
    };
    put(
        0,
        &route.fixed.root,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    put(
        1,
        &route.fixed.config.raw,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    put(
        2,
        &route.fixed.product.raw,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    put(
        3,
        &route.fixed.portfolio.raw,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    put(
        4,
        &route.fixed.linked_basis.raw,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    put(
        usize::from(DIRECT_SELLER_MAKER_ACCOUNT_V3),
        &route.seller_maker,
        DirectInlineAddressClassV3::InlineRequestBound,
    )?;
    put(
        usize::from(DIRECT_MAKER_PAYER_ACCOUNT_V3),
        &route.payer,
        DirectInlineAddressClassV3::InlineSigner,
    )?;
    put(
        usize::from(DIRECT_LIFECYCLE_RENT_CREDIT_ACCOUNT_V3),
        &route.lifecycle_rent_credit,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    put(
        usize::from(DIRECT_BUYER_MAKER_ACCOUNT_V3),
        &route.buyer_maker,
        DirectInlineAddressClassV3::InlineRequestBound,
    )?;
    put(
        usize::from(DIRECT_MAKER_PAYER_ROUTE_ALIAS_ACCOUNT_V3),
        &route.payer,
        DirectInlineAddressClassV3::InlineSigner,
    )?;
    put(
        10,
        &route.rent_program,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    put(
        11,
        &route.system_program,
        DirectInlineAddressClassV3::LookupStable,
    )?;

    let claims = SparseNativeTransferFrameSpecV1;
    for local in 0..SPARSE_NATIVE_TRANSFER_ACCOUNT_COUNT_V1 {
        let role = claims
            .account(local)
            .map_err(DirectInlineRouteErrorV3::FrameSpec)?
            .role();
        let (account, class) = claims_account(route, role)?;
        put(
            usize::from(DIRECT_INLINE_CLAIMS_ACCOUNT_START_V3 + local),
            account,
            class,
        )?;
    }

    let custody = CustodyFrameSpecV1::new(OperationV1::Transfer);
    for (route_index, start) in [
        DIRECT_INLINE_SELLER_TERMINAL_ACCOUNT_START_V3,
        DIRECT_INLINE_SELLER_INTERMEDIATE_ACCOUNT_START_V3,
        DIRECT_INLINE_FEE_CONTINUATION_ACCOUNT_START_V3,
        DIRECT_INLINE_FEE_SOLE_ACCOUNT_START_V3,
    ]
    .into_iter()
    .enumerate()
    {
        for local in 0..TRANSFER_ACCOUNT_COUNT_V1 {
            let role = custody
                .account(local)
                .map_err(DirectInlineRouteErrorV3::CustodyFrameSpec)?
                .role();
            let (account, class) = custody_account(route, route_index, role)?;
            put(usize::from(start + local), account, class)?;
        }
    }
    put(
        usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3),
        &route.custody.custody_program,
        DirectInlineAddressClassV3::LookupStable,
    )?;
    logical
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(DirectInlineRouteErrorV3::ChildFrame)
}

fn claims_account(
    route: &DirectInlineOrdinaryRouteV3,
    role: ClaimsFrameRoleV1,
) -> Result<(&ObservedAccount, DirectInlineAddressClassV3), DirectInlineRouteErrorV3> {
    let fixed = &route.fixed;
    let claims = &route.claims;
    match role {
        ClaimsFrameRoleV1::CallerAuthority => Ok((
            &claims.caller_authority,
            DirectInlineAddressClassV3::InlineRequestBound,
        )),
        ClaimsFrameRoleV1::ClaimsMarket => {
            Ok((&claims.aggregate, DirectInlineAddressClassV3::LookupStable))
        }
        ClaimsFrameRoleV1::BasisRecord => Ok((
            &fixed.linked_basis.raw,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::BasisStaging => Ok((
            &fixed.linked_basis.staging,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::ProductRecord => {
            Ok((&fixed.product.raw, DirectInlineAddressClassV3::LookupStable))
        }
        ClaimsFrameRoleV1::ProductStaging => Ok((
            &fixed.product.staging,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::ResultDomainRecord => Ok((
            &fixed.result_domain.raw,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::ResultDomainStaging => Ok((
            &fixed.result_domain.staging,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::PortfolioRecord => Ok((
            &fixed.portfolio.raw,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::PortfolioStaging => Ok((
            &fixed.portfolio.staging,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::RentSysvar => {
            Ok((&fixed.rent_sysvar, DirectInlineAddressClassV3::LookupStable))
        }
        ClaimsFrameRoleV1::CoreMarket => {
            Ok((&fixed.market, DirectInlineAddressClassV3::LookupStable))
        }
        ClaimsFrameRoleV1::ActivationCache => Ok((
            &fixed.activation_cache,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::RegistryProgram => Ok((
            &fixed.registry_program,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::TradingProgram => Ok((
            &fixed.trading_program,
            DirectInlineAddressClassV3::InlineProgram,
        )),
        ClaimsFrameRoleV1::TradingProgramData => Ok((
            &fixed.trading_programdata,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::ClaimsProgram => Ok((
            &claims.claims_program,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::ClaimsProgramData => Ok((
            &claims.claims_programdata,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::CoreProgram => Ok((
            &fixed.core_program,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::CoreProgramData => Ok((
            &fixed.core_programdata,
            DirectInlineAddressClassV3::LookupStable,
        )),
        ClaimsFrameRoleV1::SparseSourcePosition => Ok((
            &claims.seller_position,
            DirectInlineAddressClassV3::InlineRequestBound,
        )),
        ClaimsFrameRoleV1::SparseDestinationPosition => Ok((
            &claims.buyer_position,
            DirectInlineAddressClassV3::InlineRequestBound,
        )),
        _ => Err(DirectInlineRouteErrorV3::ChildFrame),
    }
}

fn custody_account(
    route: &DirectInlineOrdinaryRouteV3,
    route_index: usize,
    role: CustodyFrameRoleV1,
) -> Result<(&ObservedAccount, DirectInlineAddressClassV3), DirectInlineRouteErrorV3> {
    let fixed = &route.fixed;
    let custody = &route.custody;
    match role {
        CustodyFrameRoleV1::CallerAuthority => custody
            .caller_authorities
            .get(route_index)
            .map(|account| (account, DirectInlineAddressClassV3::InlineRequestBound))
            .ok_or(DirectInlineRouteErrorV3::ChildFrame),
        CustodyFrameRoleV1::CoreMarket => {
            Ok((&fixed.market, DirectInlineAddressClassV3::LookupStable))
        }
        CustodyFrameRoleV1::ActivationCache => Ok((
            &fixed.activation_cache,
            DirectInlineAddressClassV3::LookupStable,
        )),
        CustodyFrameRoleV1::RegistryProgram => Ok((
            &fixed.registry_program,
            DirectInlineAddressClassV3::LookupStable,
        )),
        CustodyFrameRoleV1::CallerProgram => Ok((
            &fixed.trading_program,
            DirectInlineAddressClassV3::InlineProgram,
        )),
        CustodyFrameRoleV1::CallerProgramData => Ok((
            &fixed.trading_programdata,
            DirectInlineAddressClassV3::LookupStable,
        )),
        CustodyFrameRoleV1::RealmRecord => {
            Ok((&custody.realm.raw, DirectInlineAddressClassV3::LookupStable))
        }
        CustodyFrameRoleV1::RealmStaging => Ok((
            &custody.realm.staging,
            DirectInlineAddressClassV3::LookupStable,
        )),
        CustodyFrameRoleV1::Replay => Ok((
            &custody.replay,
            DirectInlineAddressClassV3::InlineRequestBound,
        )),
        CustodyFrameRoleV1::Mint => Ok((&custody.mint, DirectInlineAddressClassV3::LookupStable)),
        CustodyFrameRoleV1::TransferSource => Ok((
            &custody.buyer_token,
            DirectInlineAddressClassV3::InlineRequestBound,
        )),
        CustodyFrameRoleV1::TransferDestination if route_index < 2 => Ok((
            &custody.seller_token,
            DirectInlineAddressClassV3::InlineRequestBound,
        )),
        CustodyFrameRoleV1::TransferDestination => Ok((
            &custody.fee_token,
            DirectInlineAddressClassV3::InlineRequestBound,
        )),
        CustodyFrameRoleV1::CustodyAuthority => Ok((
            &custody.custody_authority,
            DirectInlineAddressClassV3::LookupStable,
        )),
        CustodyFrameRoleV1::TokenProgram => Ok((
            &custody.token_program,
            DirectInlineAddressClassV3::LookupStable,
        )),
        _ => Err(DirectInlineRouteErrorV3::ChildFrame),
    }
}

fn pack_runtime(
    profile: AccountProfileV2<'_>,
    outcome_count: u32,
    logical: &[ClassifiedAccountV3],
) -> Result<(Vec<ObservedAccountMetaV3>, Vec<DirectInlineAddressClassV3>), DirectInlineRouteErrorV3>
{
    if logical.len()
        != profile
            .logical_account_count(outcome_count)
            .map_err(DirectInlineRouteErrorV3::AccountProfile)?
    {
        return Err(DirectInlineRouteErrorV3::Profile);
    }
    let physical_count = profile
        .physical_account_count_with_dynamic_spans(outcome_count, &[])
        .map_err(DirectInlineRouteErrorV3::AccountProfile)?;
    let mut physical: Vec<Option<ClassifiedAccountV3>> = vec![None; physical_count];
    for (logical_coordinate, account) in logical.iter().enumerate() {
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(outcome_count, &[], logical_coordinate)
            .map_err(DirectInlineRouteErrorV3::AccountProfile)?;
        let slot = physical
            .get_mut(ordinal)
            .ok_or(DirectInlineRouteErrorV3::Profile)?;
        if slot.as_ref().is_some_and(|existing| existing != account) {
            return Err(DirectInlineRouteErrorV3::Profile);
        }
        if slot.is_none() {
            *slot = Some(account.clone());
        }
    }
    let packed = physical
        .into_iter()
        .enumerate()
        .map(|(ordinal, account)| {
            let classified = account.ok_or(DirectInlineRouteErrorV3::Profile)?;
            let account = classified.account;
            let geometry = profile
                .physical_account_geometry_with_dynamic_spans(outcome_count, &[], ordinal)
                .map_err(DirectInlineRouteErrorV3::AccountProfile)?;
            let privileges = geometry.privileges();
            let data_matches = match geometry.data() {
                PhysicalAccountDataGeometryV2::Exact { bytes } => account.data.len() == bytes,
                PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes } => {
                    account.data.is_empty() || account.data.len() == live_bytes
                }
                PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                    !account.data.is_empty() && account.data.len() >= minimum_bytes
                }
                PhysicalAccountDataGeometryV2::Opaque => true,
            };
            if account.executable != privileges.executable() || !data_matches {
                return Err(DirectInlineRouteErrorV3::Profile);
            }
            Ok((
                ObservedAccountMetaV3 {
                    account,
                    is_signer: privileges.signer(),
                    is_writable: privileges.writable(),
                },
                classified.class,
            ))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (accounts, classes) = packed.into_iter().unzip();
    Ok((accounts, classes))
}

fn meta(account: &ObservedAccount, is_signer: bool, is_writable: bool) -> ObservedAccountMetaV3 {
    ObservedAccountMetaV3 {
        account: account.clone(),
        is_signer,
        is_writable,
    }
}

fn insert_once<T>(
    slots: &mut [Option<T>],
    index: usize,
    value: T,
    error: DirectInlineRouteErrorV3,
) -> Result<(), DirectInlineRouteErrorV3> {
    let slot = slots.get_mut(index).ok_or(error)?;
    if slot.is_some() {
        return Err(error);
    }
    *slot = Some(value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use dclutch_vm::account_profile::v2::{AccountProfileV2, PhysicalAccountDataGeometryV2};
    use dclutch_market::capability_manifest::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
        CompartmentFundingV1, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_market::capability_program::CAPABILITY_ROOT_HEADER_BYTES_V1;
    use dclutch_market::capability_program::hot_v3::{
        HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3, HOT_CONFIG_RAW_ACCOUNT_V3,
        HOT_CONFIG_STAGING_ACCOUNT_V3, HOT_DESCRIPTOR_STAGING_ACCOUNT_V3,
        HOT_EFFECT_STAGING_ACCOUNT_V3, HOT_FIXED_ACCOUNT_COUNT_V3, HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        HOT_MARKET_ACCOUNT_V3, HOT_PORTFOLIO_RAW_ACCOUNT_V3, HOT_PRODUCT_RAW_ACCOUNT_V3,
        HOT_ROOT_ACCOUNT_V3, HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3,
        HotExecutionEnvelopeV3,
    };
    use dclutch_market::capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1};
    use dclutch_claims::liability_basis_state_v2::{
        LIABILITY_BASIS_MARKET_HEADER_BYTES_V2, LIABILITY_BASIS_POSITION_HEADER_BYTES_V2,
        LiabilityBasisMarketInputV2, LiabilityBasisPositionInputV2,
        encode_liability_basis_market_into_v2, encode_liability_basis_position_into_v2,
    };
    use dclutch_claims::protocol_position_v2::ProtocolPositionSeedsV2;
    use dclutch_core_contract::ContentId as CoreContentId;
    use dclutch_custody::CustodyReplayLayoutV1;
    use dclutch_custody::{CallerRoleV1, CustodyReplaySeedsV1, CustodyReplayV1};
    use dclutch_trading::{
        execution_v3::DirectExecutionActionV3,
        intent_v2::CompactIntentV2,
        native_evidence_v3::{
            DirectNativeEvidenceContainerV3, direct_native_evidence_bytes_v3,
            encode_direct_native_evidence_many_v3_atomic,
        },
        ordinary_account_artifacts_v3::DirectInlineOrdinaryAccountProfileInputV3,
        ordinary_bundle_v4::{
            DirectInlineOrdinaryHotBundleInputV4, build_direct_inline_ordinary_hot_bundle_v4,
        },
        ordinary_effect_artifacts_v3::{
            DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3, DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3,
        },
        ordinary_geometry_v3::DirectOrdinaryGeometryV3,
        ordinary_v3::DirectOrdinaryAuthenticatedContextV3,
        program_set_v4::build_direct_inline_ordinary_lifecycle_program_set_v1,
        successor::{
            DIRECT_EXECUTION_CONFIG_BYTES_V1, DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1,
            DIRECT_MAKER_REPLAY_BYTES_V1, DIRECT_ROOT_SCHEMA_ID_V1, DIRECT_ROOT_STATE_BYTES_V1,
            DirectExecutionConfigV1, DirectRootStateV1,
        },
    };
    use dclutch_market::{
        CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase, Readiness,
        STATE_BYTES as CORE_STATE_BYTES,
    };
    use dclutch_product::payoff::{
        registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        runtime_v3::{
            BASIS_WIDTH_OFFSET_V3, BasisInputV3, BasisKindV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
        },
    };
    use dclutch_product::{
        ContentId as ProductContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
        compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
    };
    use dclutch_product::admission::PRODUCT_RECORD_BYTES_V2;
    use dclutch_product::admission::{
        PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, ProductRecordV2,
        RESULT_DOMAIN_SCHEMA_ID_V2,
    };
    use dclutch_market::realm::REALM_BYTES;
    use dclutch_market::realm::{
        FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
        RealmV1Input,
    };
    use dclutch_registry::{
        ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
        ARTIFACT_RELEASE_BYTES_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
        ArtifactUpgradePolicyV1, ExecutionReleaseActivationInputsV1,
        activate_execution_release_set_v1,
    };
    use dclutch_registry::svm::LOADER_V3_PROGRAM_BYTES;
    use dclutch_registry::release_set::{
        ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, EXECUTION_RELEASE_SET_BYTES_V1,
        ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1, ProgramIdentityV1,
    };
    use dclutch_release_tool::{
        CHECKED_MULTIPROGRAM_BYTES_V1, CHECKED_MULTIPROGRAM_MAGIC_V1,
        CHECKED_MULTIPROGRAM_SCHEMA_V1, CheckedExecutionReleaseSetV1,
    };
    use dclutch_market::rent::lifecycle_v2::LIFECYCLE_RENT_CREDIT_BYTES_V2;
    use dclutch_custody::token_svm::{AccountState, TokenAccount, state::TokenAccountLayoutV1};
    use solana_address_lookup_table_interface::state::{AddressLookupTable, LookupTableMeta};
    use solana_compute_budget_interface::ComputeBudgetInstruction;
    use solana_hash::Hash;
    use solana_message::VersionedMessage;
    use solana_program::{
        account_info::AccountInfo,
        hash::{hash, hashv},
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        rent::Rent,
        sysvar::SysvarSerialize,
    };
    use solana_sdk_ids::{
        bpf_loader_upgradeable, compute_budget, ed25519_program, system_program, sysvar,
    };

    use super::{
        DirectClaimsRouteV3, DirectCustodyRouteV3, DirectHotFixedRouteV3,
        DirectInlineAddressClassV3, DirectInlineAddressPlacementV3,
        DirectInlineCheckedProgramAccountsV3, DirectInlineFinalizationRefusalV3,
        DirectInlineLookupTableProvisionV3, DirectInlineOrdinaryRouteV3,
        DirectInlinePhysicalRouteV3, DirectInlineRouteAuthenticationV3, DirectInlineRouteErrorV3,
        DirectInlineRoutedTransactionErrorV3, DirectInlineSealedReportFactsRefusalV3,
        DirectInlineSealedReportProjectionRefusalV3, FinalizedRecordRouteV3,
        admit_direct_inline_devnet_account_lock_count_v3,
        assemble_authenticated_direct_inline_ordinary_route_v3,
        build_direct_inline_capability_seal_v3, build_direct_inline_lookup_table_provision_v3,
        child_caller_hint_slots_v1, compile_direct_inline_capability_seal_routed_v0_v3,
        compile_direct_inline_routed_v0_v3, derive_direct_inline_child_authorities_v3, insert_once,
        merge_placement, prepare_direct_inline_hot_finalization_v3,
        project_direct_inline_sealed_execution_physical_v3,
        project_direct_inline_sealed_execution_report_v3, verify_direct_inline_capability_seal_v3,
    };
    use crate::direct_inline_v3::DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1;
    use crate::{
        Finality, Observation, ObservedAccount,
        direct_inline_v3::{
            DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1, DirectInlineEconomicPreviewV3,
            DirectInlineHotReportV3, ObservedAccountMetaV3, SignedDirectIntentV3,
            compile_direct_inline_request_v3,
        },
    };
    use dclutch_market::capability_program::hot_v3::DIRECT_HOT_HEAP_FRAME_BYTES_V1;
    use dclutch_trading::direct_finalization_v3::DirectFinalizationErrorV3;
    use dclutch_trading::inline_candidate_v2::DirectInlineCandidateErrorV2;
    use dclutch_market::StateBumpsV1;

    fn key(byte: u8) -> Pubkey {
        Pubkey::new_from_array([byte; 32])
    }

    fn observation() -> Observation {
        Observation {
            slot: 77,
            unix_timestamp: 1_800_000_000,
            finality: Finality::Finalized,
        }
    }

    fn observed(byte: u8, executable: bool) -> ObservedAccount {
        ObservedAccount {
            observation: observation(),
            key: key(byte),
            owner: key(240),
            lamports: 1_000_000,
            executable,
            data: vec![byte],
        }
    }

    fn ordinary_logical_lengths() -> Vec<u32> {
        let mut lengths = vec![0_u32; usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)];
        lengths[0] = u32::try_from(CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1)
            .expect("root width");
        lengths[1] = u32::try_from(DIRECT_EXECUTION_CONFIG_BYTES_V1).expect("config width");
        lengths[2] = u32::try_from(PRODUCT_RECORD_BYTES_V2).expect("Product width");
        let geometry = DirectOrdinaryGeometryV3::CANONICAL;
        lengths[3] = geometry.portfolio_record_bytes().expect("portfolio width");
        lengths[4] = u32::try_from(BASIS_WIDTH_OFFSET_V3 + 4).expect("basis prefix");
        for coordinate in [5_usize, 8] {
            lengths[coordinate] = u32::try_from(DIRECT_MAKER_REPLAY_BYTES_V1).expect("maker width");
        }
        lengths[7] = u32::try_from(LIFECYCLE_RENT_CREDIT_BYTES_V2).expect("RentCredit width");
        lengths[10] = u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Rent program width");
        lengths[13] = geometry
            .claims_aggregate_record_bytes()
            .expect("Claims aggregate width");
        lengths[14] = lengths[4];
        lengths[16] = lengths[2];
        lengths[18] = geometry
            .result_domain_record_bytes()
            .expect("result-domain width");
        lengths[20] = lengths[3];
        lengths[22] = 17;
        lengths[23] = u32::try_from(CORE_STATE_BYTES).expect("Core width");
        lengths[24] =
            u32::try_from(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1).expect("activation width");
        for coordinate in [25_usize, 26, 28, 30] {
            lengths[coordinate] = u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("program width");
        }
        for coordinate in [27_usize, 29, 31] {
            lengths[coordinate] = 1_024;
        }
        let position = geometry
            .claims_position_record_bytes()
            .expect("Claims Position width");
        lengths[32] = position;
        lengths[33] = position;
        lengths[40] = u32::try_from(REALM_BYTES).expect("Realm width");
        lengths[42] = u32::try_from(CustodyReplayLayoutV1::BYTES).expect("replay width");
        lengths[43] = 82;
        lengths[44] = 165;
        lengths[45] = 165;
        lengths[47] = 36;
        lengths[73] = 165;
        for (coordinate, representative) in [
            (9, 6),
            (35, 23),
            (36, 24),
            (37, 25),
            (38, 26),
            (39, 27),
            (49, 23),
            (50, 24),
            (51, 25),
            (52, 26),
            (53, 27),
            (54, 40),
            (55, 41),
            (56, 42),
            (57, 43),
            (58, 44),
            (59, 45),
            (60, 46),
            (61, 47),
            (63, 23),
            (64, 24),
            (65, 25),
            (66, 26),
            (67, 27),
            (68, 40),
            (69, 41),
            (70, 42),
            (71, 43),
            (72, 44),
            (74, 46),
            (75, 47),
            (77, 23),
            (78, 24),
            (79, 25),
            (80, 26),
            (81, 27),
            (82, 40),
            (83, 41),
            (84, 42),
            (85, 43),
            (86, 44),
            (87, 73),
            (88, 46),
            (89, 47),
        ] {
            lengths[coordinate] = lengths[representative];
        }
        lengths[usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3)] =
            u32::try_from(LOADER_V3_PROGRAM_BYTES).expect("Custody program width");
        lengths
    }

    fn profiled_account(
        profile: AccountProfileV2<'_>,
        coordinate: usize,
        key: Pubkey,
        owner: Pubkey,
    ) -> ObservedAccount {
        let ordinal = profile
            .physical_account_ordinal_with_dynamic_spans(3, &[], coordinate)
            .expect("physical ordinal");
        let geometry = profile
            .physical_account_geometry_with_dynamic_spans(3, &[], ordinal)
            .expect("physical geometry");
        let data_len = match geometry.data() {
            PhysicalAccountDataGeometryV2::Exact { bytes }
            | PhysicalAccountDataGeometryV2::VacantOrExact { live_bytes: bytes } => bytes,
            PhysicalAccountDataGeometryV2::AdapterAuthenticatedVariable { minimum_bytes } => {
                minimum_bytes
            }
            PhysicalAccountDataGeometryV2::Opaque => 1,
        };
        ObservedAccount {
            observation: observation(),
            key,
            owner,
            lamports: 1_000_000,
            executable: geometry.privileges().executable(),
            data: vec![0; data_len],
        }
    }

    /// The founding RentCredit's refund wallet, deliberately NOT the payer.
    ///
    /// A maker replay root's rent belongs to the market, so the chain records
    /// this wallet as its rent owner however the trade is paid for. Holding it
    /// distinct from the payer is what lets these fixtures tell the two models
    /// apart; while they were the same key, a projection answering `payer` was
    /// indistinguishable from a correct one.
    fn maker_rent_beneficiary() -> Pubkey {
        key(0x5a)
    }

    /// A real founding lifecycle RentCredit, not a zero-filled placeholder.
    fn lifecycle_rent_credit_account(
        profile: AccountProfileV2<'_>,
        key_value: Pubkey,
        rent_program: Pubkey,
    ) -> ObservedAccount {
        let mut account = profiled_account(profile, 7, key_value, rent_program);
        let credit = dclutch_market::rent::lifecycle_v2::LifecycleRentCreditV2::new(
            dclutch_market::rent::RefundAuthority::new(maker_rent_beneficiary().to_bytes())
                .expect("refund wallet"),
            dclutch_market::rent::lifecycle_v2::LifecycleAccountIdV2::new([0x5b; 32])
                .expect("market"),
            dclutch_market::rent::lifecycle_v2::LifecycleAccountIdV2::new([0x5c; 32])
                .expect("release set"),
            7,
            255,
        )
        .expect("lifecycle RentCredit");
        account.data = credit.to_bytes().to_vec();
        account
    }

    fn record(raw: ObservedAccount, staging_key: Pubkey) -> FinalizedRecordRouteV3 {
        FinalizedRecordRouteV3 {
            raw,
            staging: ObservedAccount {
                observation: observation(),
                key: staging_key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: Vec::new(),
            },
        }
    }

    fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
        let mut output = vec![0; LOADER_V3_PROGRAM_BYTES];
        output
            .get_mut(..4)
            .expect("loader variant")
            .copy_from_slice(&2_u32.to_le_bytes());
        output
            .get_mut(4..)
            .expect("ProgramData identity")
            .copy_from_slice(programdata.as_ref());
        output
    }

    fn immutable_programdata_bytes(slot: u64, fill: u8) -> Vec<u8> {
        let mut output = vec![0; 1_024];
        output
            .get_mut(..4)
            .expect("ProgramData variant")
            .copy_from_slice(&3_u32.to_le_bytes());
        output
            .get_mut(4..12)
            .expect("deployment slot")
            .copy_from_slice(&slot.to_le_bytes());
        output.get_mut(45..).expect("ELF").fill(fill);
        output
    }

    #[derive(Clone)]
    struct RoleFixtureV3 {
        release: ArtifactReleaseV1,
        artifact: ArtifactReleaseIdV1,
        program: ObservedAccount,
        programdata: ObservedAccount,
    }

    fn role_fixture(program: Pubkey, semantic_fill: u8, deployment_slot: u64) -> RoleFixtureV3 {
        let programdata =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let programdata_bytes = immutable_programdata_bytes(deployment_slot, semantic_fill);
        let release = ArtifactReleaseV1::new(
            ProgramIdentityV1::new(program.to_bytes()).expect("role program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata.to_bytes(),
            CoreContentId::new([semantic_fill; 32]).expect("semantic release"),
            hash(programdata_bytes.get(45..).expect("ELF")).to_bytes(),
            deployment_slot,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("ArtifactRelease");
        RoleFixtureV3 {
            artifact: ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes())
                .expect("artifact ID"),
            release,
            program: ObservedAccount {
                observation: observation(),
                key: program,
                owner: bpf_loader_upgradeable::ID,
                lamports: 1_000_000,
                executable: true,
                data: loader_program_bytes(programdata),
            },
            programdata: ObservedAccount {
                observation: observation(),
                key: programdata,
                owner: bpf_loader_upgradeable::ID,
                lamports: 1_000_000,
                executable: false,
                data: programdata_bytes,
            },
        }
    }

    fn checked_release_set_bytes(
        release_set: ExecutionReleaseSetV1,
        artifacts: [ArtifactReleaseV1; 5],
    ) -> [u8; CHECKED_MULTIPROGRAM_BYTES_V1] {
        const HEADER: usize = 16;
        let mut output = [0_u8; CHECKED_MULTIPROGRAM_BYTES_V1];
        output
            .get_mut(..8)
            .expect("magic")
            .copy_from_slice(&CHECKED_MULTIPROGRAM_MAGIC_V1);
        output
            .get_mut(8..10)
            .expect("schema")
            .copy_from_slice(&CHECKED_MULTIPROGRAM_SCHEMA_V1.to_le_bytes());
        output
            .get_mut(10..12)
            .expect("role count")
            .copy_from_slice(&5_u16.to_le_bytes());
        output
            .get_mut(HEADER..HEADER + EXECUTION_RELEASE_SET_BYTES_V1)
            .expect("release set")
            .copy_from_slice(&release_set.to_bytes());
        let mut offset = HEADER + EXECUTION_RELEASE_SET_BYTES_V1;
        for (index, artifact) in artifacts.into_iter().enumerate() {
            output
                .get_mut(offset..offset + ARTIFACT_RELEASE_BYTES_V1)
                .expect("artifact")
                .copy_from_slice(&artifact.to_bytes());
            offset += ARTIFACT_RELEASE_BYTES_V1;
            output
                .get_mut(offset..offset + 32)
                .expect("checked release identity")
                .copy_from_slice(&[0xc0 + u8::try_from(index).expect("role"); 32]);
            offset += 32;
        }
        assert_eq!(offset, output.len());
        CheckedExecutionReleaseSetV1::decode(&output).expect("canonical checked release set");
        output
    }

    fn product_graph_bytes() -> (Vec<u8>, Vec<u8>, Vec<u8>, Vec<u8>, [u8; 32], [u8; 32]) {
        let product_id = ProductContentId::new([0xa1; 32]).expect("Product identity");
        let coordinate_domain = ProductContentId::new([0xa2; 32]).expect("coordinate domain");
        let result_unit = ProductContentId::new([0xa3; 32]).expect("result unit");
        let provisional_input = BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id: product_id.to_bytes(),
            result_domain_id: [0xa4; 32],
            coordinate_domain_id: coordinate_domain.to_bytes(),
            result_unit_id: result_unit.to_bytes(),
            evaluator_release_id: [0xa5; 32],
            basis_width: 3,
            payout_scale: 1,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
            // Exempt by proof: degree 0 and 1 need no price gate,
            // and a digest offered alongside one is refused.
            price_gate_certificate_digest: [0_u8; 32],
        };
        let basis_width =
            basis_record_bytes_v3(BasisKindV3::CategoricalQ1, 3, 0, 0).expect("basis width");
        let mut provisional = vec![0_u8; basis_width];
        compile_basis_v3(provisional_input, &mut provisional).expect("provisional basis");
        let semantic = semantic_basis_preimage_v3(&provisional).expect("semantic basis");
        let semantic_basis = hashv(&[
            SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
            semantic.prefix(),
            semantic.suffix(),
        ])
        .to_bytes();
        let cuts = [0_i128];
        let mut domain = vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain width")];
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id,
                coordinate_domain_id: coordinate_domain,
                result_unit_id: result_unit,
                liability_basis_id: ProductContentId::new(semantic_basis).expect("basis ID"),
                representation_release_id: ProductContentId::new([0xa6; 32])
                    .expect("representation"),
                mapping_release_id: ProductContentId::new([0xa7; 32]).expect("mapping"),
                cut_denominator: 1,
                cuts: &cuts,
            },
            &mut domain,
        )
        .expect("domain");
        let domain_digest = hash(&domain).to_bytes();
        let mut basis = vec![0_u8; basis_width];
        compile_basis_v3(
            BasisInputV3 {
                result_domain_id: domain_digest,
                ..provisional_input
            },
            &mut basis,
        )
        .expect("linked basis");
        let coefficients = [7_u64; 3];
        let mut portfolio =
            vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
        compile_portfolio_v2(
            PortfolioInputV2 {
                product_id,
                result_domain_id: ProductContentId::new(domain_digest).expect("domain ID"),
                claim_basis_id: ProductContentId::new([0xa8; 32]).expect("claim basis"),
                liability_basis_id: ProductContentId::new(semantic_basis).expect("basis ID"),
                representation_release_id: ProductContentId::new([0xa6; 32])
                    .expect("representation"),
                denominator: 1,
                coefficients: &coefficients,
            },
            &mut portfolio,
        )
        .expect("portfolio");
        let mut product = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
        ProductRecordV2::new(
            product_id,
            ProductContentId::new(domain_digest).expect("domain"),
            ProductContentId::new(hash(&portfolio).to_bytes()).expect("portfolio"),
        )
        .encode_into(&mut product)
        .expect("Product record");
        (
            product,
            domain,
            portfolio,
            basis,
            product_id.to_bytes(),
            semantic_basis,
        )
    }

    fn token_account_bytes(
        mint: Pubkey,
        owner: Pubkey,
        amount: u64,
        delegate: Option<Pubkey>,
        delegated_amount: u64,
    ) -> Vec<u8> {
        let mut output = TokenAccount::initialized_base_bytes(mint.to_bytes(), owner.to_bytes())
            .expect("base token account");
        output
            .get_mut(TokenAccountLayoutV1::AMOUNT..TokenAccountLayoutV1::AMOUNT + 8)
            .expect("amount")
            .copy_from_slice(&amount.to_le_bytes());
        output
            .get_mut(
                TokenAccountLayoutV1::DELEGATED_AMOUNT..TokenAccountLayoutV1::DELEGATED_AMOUNT + 8,
            )
            .expect("delegated amount")
            .copy_from_slice(&delegated_amount.to_le_bytes());
        if let Some(delegate) = delegate {
            output
                .get_mut(TokenAccountLayoutV1::DELEGATE..TokenAccountLayoutV1::DELEGATE + 4)
                .expect("delegate tag")
                .copy_from_slice(&1_u32.to_le_bytes());
            output
                .get_mut(TokenAccountLayoutV1::DELEGATE + 4..TokenAccountLayoutV1::DELEGATE + 36)
                .expect("delegate")
                .copy_from_slice(delegate.as_ref());
        }
        assert_eq!(
            TokenAccount::parse(&output).map(|value| value.state),
            Ok(AccountState::Initialized)
        );
        output.to_vec()
    }

    fn capability_id(bytes: [u8; 32]) -> dclutch_market::capability_manifest::ContentId {
        dclutch_market::capability_manifest::ContentId::new(bytes).expect("capability identity")
    }

    #[allow(clippy::too_many_lines)]
    fn install_exact_chain_fixture_v3(
        route: &mut DirectInlineOrdinaryRouteV3,
        authentication: &mut DirectInlineRouteAuthenticationV3,
        ordinary: dclutch_trading::ordinary_bundle_v4::DirectInlineOrdinaryHotBundleV4,
    ) {
        let rent = Rent::default();
        route.fixed.rent_sysvar.data = serialize_rent(&rent);
        route.fixed.rent_sysvar.owner = sysvar::ID;
        route.fixed.rent_sysvar.lamports = 1;
        let registry = authentication.programs.registry_program;
        let registry_programdata =
            Pubkey::find_program_address(&[registry.as_ref()], &bpf_loader_upgradeable::ID).0;
        route.fixed.registry_program.data = loader_program_bytes(registry_programdata);
        route.fixed.registry_program.owner = bpf_loader_upgradeable::ID;
        route.fixed.registry_program.executable = true;

        let core = role_fixture(authentication.programs.core_program, 0xb1, 71);
        let claims = role_fixture(authentication.programs.claims_program, 0xb2, 72);
        let trading = role_fixture(authentication.programs.trading_program, 0xb3, 73);
        let resolution = role_fixture(key(53), 0xb4, 74);
        let custody = role_fixture(authentication.programs.custody_program, 0xb5, 75);
        let bindings = [
            (&core, ExecutionRoleV1::Core),
            (&claims, ExecutionRoleV1::Claims),
            (&trading, ExecutionRoleV1::Trading),
            (&resolution, ExecutionRoleV1::Resolution),
            (&custody, ExecutionRoleV1::Custody),
        ]
        .map(|(role, _)| ExecutionRoleBindingV1::new(role.release.program(), role.artifact));
        let [
            core_binding,
            claims_binding,
            trading_binding,
            resolution_binding,
            custody_binding,
        ] = bindings;
        let release_set = ExecutionReleaseSetV1::new(
            core_binding,
            claims_binding,
            trading_binding,
            resolution_binding,
            custody_binding,
        )
        .expect("release set");
        let release_set_digest = hash(&release_set.to_bytes()).to_bytes();
        let release_set_id = CoreContentId::new(release_set_digest).expect("release set ID");
        let activation_input = |role: &RoleFixtureV3| {
            ArtifactActivationInputV1::new(
                role.artifact,
                role.release,
                crate::direct_inline_v3::direct_deployment_observation_v4(
                    &role.program,
                    &role.programdata,
                    role.release,
                )
                .expect("deployment observation"),
            )
        };
        let activated = activate_execution_release_set_v1(
            release_set_id,
            &release_set,
            &ExecutionReleaseActivationInputsV1::new(
                activation_input(&core),
                activation_input(&claims),
                activation_input(&trading),
                activation_input(&resolution),
                activation_input(&custody),
            ),
        )
        .expect("activated release set");
        route.fixed.activation_cache = ObservedAccount {
            observation: observation(),
            key: Pubkey::find_program_address(
                &[ACTIVATION_PDA_DOMAIN_V1, &release_set_digest],
                &registry,
            )
            .0,
            owner: registry,
            lamports: rent.minimum_balance(ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1),
            executable: false,
            data: activated.to_bytes().to_vec(),
        };
        route.fixed.core_program = core.program.clone();
        route.fixed.core_programdata = core.programdata.clone();
        route.fixed.trading_program = trading.program.clone();
        route.fixed.trading_programdata = trading.programdata.clone();
        route.claims.claims_program = claims.program.clone();
        route.claims.claims_programdata = claims.programdata.clone();
        route.custody.custody_program = custody.program.clone();
        route.custody.custody_programdata = custody.programdata.clone();
        authentication.programs.core_programdata = core.programdata.key;
        authentication.programs.trading_programdata = trading.programdata.key;
        authentication.programs.claims_programdata = claims.programdata.key;
        authentication.programs.checked_execution_release_set = checked_release_set_bytes(
            release_set,
            [
                core.release,
                claims.release,
                trading.release,
                resolution.release,
                custody.release,
            ],
        );

        let (product, domain, portfolio, basis, product_id, semantic_basis) = product_graph_bytes();
        route.fixed.product.raw.data = product;
        route.fixed.result_domain.raw.data = domain;
        route.fixed.portfolio.raw.data = portfolio;
        route.fixed.linked_basis.raw.data = basis;
        for (record, schema) in [
            (&mut route.fixed.product, PRODUCT_RECORD_SCHEMA_ID_V2),
            (&mut route.fixed.result_domain, RESULT_DOMAIN_SCHEMA_ID_V2),
            (&mut route.fixed.portfolio, PORTFOLIO_SCHEMA_ID_V2),
            (
                &mut route.fixed.linked_basis,
                GRADED_BASIS_RECORD_SCHEMA_ID_V3,
            ),
        ] {
            let digest = hash(&record.raw.data).to_bytes();
            canonicalize_record(record, registry, &rent, schema, digest);
        }

        let realm = RealmV1::new(RealmV1Input {
            token_program: route.custody.token_program.key.to_bytes(),
            collateral_mint: route.custody.mint.key.to_bytes(),
            collateral_adapter_release_id: [0xd1; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("Realm");
        route.custody.realm.raw.data = realm.to_bytes().to_vec();
        let realm_digest = hash(&route.custody.realm.raw.data).to_bytes();
        canonicalize_record(
            &mut route.custody.realm,
            registry,
            &rent,
            REALM_SCHEMA_RELEASE_ID_V1,
            realm_digest,
        );

        let capacity_profile = [0x91; 32];
        let lifecycle =
            build_direct_inline_ordinary_lifecycle_program_set_v1(ordinary, capacity_profile)
                .expect("lifecycle ProgramSet");
        let config =
            DirectExecutionConfigV1::new(100, 50, key(7).to_bytes()).expect("Direct config");
        let config_bytes = config.encode();
        let config_digest = hash(&config_bytes).to_bytes();
        let amounts = FundingAmountsV1::new(
            CompartmentFundingV1::native_lamports(1).expect("Rent funding"),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
            CompartmentFundingV1::not_applicable(),
        )
        .expect("Funding amounts");
        let entry = CapabilityEntryV1::new(
            capability_id(dclutch_trading::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3),
            capability_id(lifecycle.program_set_id),
            capability_id(config_digest),
            capability_id(capacity_profile),
            capability_id(DIRECT_ROOT_SCHEMA_ID_V1),
            capability_id(hash(&lifecycle.ordinary.lifecycle_policy).to_bytes()),
            ActivationPolicy::PrepaidLazy,
            1_000,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            FundingQuoteV1::new(amounts, None).expect("Funding quote"),
        )
        .expect("manifest entry");
        let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&[entry], &mut manifest).expect("manifest");
        let manifest_digest = hash(&manifest).to_bytes();
        route.fixed.manifest.raw.data = manifest;
        route.fixed.program_set.raw.data = lifecycle.program_set.clone();
        route.fixed.descriptor.raw.data = lifecycle.ordinary.descriptor.to_vec();
        route.fixed.config.raw.data = config_bytes.to_vec();
        route.fixed.account_profile.raw.data = lifecycle.ordinary.account_profile.to_vec();
        route.fixed.request_profile.raw.data = lifecycle.ordinary.request_profile.to_vec();
        route.fixed.transition.raw.data = lifecycle.ordinary.transition.to_vec();
        route.fixed.effect.raw.data = lifecycle.ordinary.effect.to_vec();
        route.fixed.lifecycle.raw.data = lifecycle.ordinary.lifecycle_policy.to_vec();
        route.fixed.strategy.raw.data = lifecycle.ordinary.strategy.to_vec();
        let descriptor = dclutch_market::capability_program::v4::CapabilityProgramV4::decode(
            &route.fixed.descriptor.raw.data,
        )
        .expect("ordinary descriptor");
        for (record, schema) in [
            (
                &mut route.fixed.manifest,
                dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            ),
            (
                &mut route.fixed.program_set,
                dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            ),
            (
                &mut route.fixed.descriptor,
                dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
            ),
            (&mut route.fixed.config, DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1),
            (
                &mut route.fixed.account_profile,
                descriptor.account_profile().schema().to_bytes(),
            ),
            (
                &mut route.fixed.request_profile,
                descriptor.request_profile().schema().to_bytes(),
            ),
            (
                &mut route.fixed.transition,
                descriptor.transition().schema().to_bytes(),
            ),
            (
                &mut route.fixed.effect,
                descriptor.effect().schema().to_bytes(),
            ),
            (
                &mut route.fixed.lifecycle,
                descriptor.lifecycle().schema().to_bytes(),
            ),
            (
                &mut route.fixed.strategy,
                descriptor.strategy().schema().to_bytes(),
            ),
        ] {
            let digest = hash(&record.raw.data).to_bytes();
            canonicalize_record(record, registry, &rent, schema, digest);
        }

        let mut identity = MarketIdentity {
            market_id: Identity::new([1; 32]).expect("temporary Market"),
            realm_id: Identity::new(realm_digest).expect("Realm identity"),
            product_record: Identity::new(hash(&route.fixed.product.raw.data).to_bytes())
                .expect("Product record"),
            product_id: Identity::new(product_id).expect("Product identity"),
            resolution_policy: Identity::new([0xd2; 32]).expect("resolution policy"),
            capability_manifest: Identity::new(manifest_digest).expect("manifest"),
            selected_release_set: Identity::new(release_set_digest).expect("release set"),
            registry_program: Identity::new(registry.to_bytes()).expect("Registry"),
            generation: 7,
        };
        let market_key = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(identity).as_slices(),
            &core.program.key,
        )
        .0;
        identity.market_id = Identity::new(market_key.to_bytes()).expect("canonical Market");
        let market = CoreState {
            phase: Phase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity,
            outstanding_capabilities: 1,
            principal_cap_sets: 100,
            rent_beneficiary: Identity::new(route.payer.key.to_bytes()).expect("beneficiary"),
            terminal_receipt: None,
            bumps: StateBumpsV1::UNRECORDED,
        };
        route.fixed.market.key = market_key;
        route.fixed.market.owner = core.program.key;
        route.fixed.market.executable = false;
        route.fixed.market.data = market.encode().expect("Core Market").to_vec();
        route.fixed.market.lamports = rent.minimum_balance(route.fixed.market.data.len());

        let selection = CapabilityExecutionSelectionV1::new(
            0,
            capability_id(manifest_digest),
            capability_id(dclutch_trading::execution_v3::DIRECT_SUCCESSOR_KIND_ID_V3),
            capability_id(lifecycle.program_set_id),
            capability_id(config_digest),
        )
        .expect("selection");
        let bumps = |schema: [u8; 32], digest: [u8; 32]| {
            (
                Pubkey::find_program_address(
                    &[
                        dclutch_registry::record::RAW_RECORD_PDA_SEED_V1,
                        &schema,
                        &digest,
                    ],
                    &registry,
                )
                .1,
                Pubkey::find_program_address(
                    &[
                        dclutch_registry::record::STAGING_CURSOR_PDA_SEED_V1,
                        &schema,
                        &digest,
                    ],
                    &registry,
                )
                .1,
            )
        };
        let manifest_bumps = bumps(
            dclutch_market::capability_manifest::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            manifest_digest,
        );
        let set_bumps = bumps(
            dclutch_market::capability_program::set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
            lifecycle.program_set_id,
        );
        let config_bumps = bumps(DIRECT_EXECUTION_CONFIG_SCHEMA_ID_V1, config_digest);
        let header = CapabilityRootHeaderV1::new(
            CoreContentId::new(release_set_digest).expect("release set"),
            market_key.to_bytes(),
            identity.generation,
            selection.with_capability_release_record_bumps(set_bumps.0, set_bumps.1),
            SelectedRecordBumpsV1::new(
                manifest_bumps.0,
                manifest_bumps.1,
                config_bumps.0,
                config_bumps.1,
            ),
        )
        .expect("root header");
        let mut root_data = header.to_bytes().to_vec();
        root_data.extend_from_slice(&DirectRootStateV1::new().encode());
        route.fixed.root.key =
            Pubkey::find_program_address(&header.seeds().as_slices(), &trading.program.key).0;
        route.fixed.root.owner = trading.program.key;
        route.fixed.root.data = root_data;
        route.fixed.root.lamports = rent.minimum_balance(route.fixed.root.data.len());

        for intent in [
            &mut authentication.seller.intent,
            &mut authentication.buyer.intent,
        ] {
            intent.market = market_key.to_bytes();
            intent.generation = identity.generation;
            intent.nonce = 0;
            intent.valid_from = 1;
            intent.valid_through = 100;
        }
        let coordinates = dclutch_trading::successor::DirectCoordinatesV1::new(
            market_key.to_bytes(),
            identity.generation,
        )
        .expect("Direct coordinates");
        for (account, maker) in [
            (&mut route.seller_maker, authentication.seller.maker),
            (&mut route.buyer_maker, authentication.buyer.maker),
        ] {
            let seeds = dclutch_trading::successor::MakerReplaySeedsV1::new(
                coordinates,
                maker.to_bytes(),
            )
            .expect("maker seeds");
            account.key = Pubkey::find_program_address(&seeds.as_slices(), &trading.program.key).0;
            account.owner = system_program::ID;
            account.lamports = 1;
            account.executable = false;
            account.data.clear();
        }

        let aggregate_key = Pubkey::find_program_address(
            &[
                dclutch_claims::liability_basis_state_v2::LIABILITY_BASIS_MARKET_SEED_V2,
                market_key.as_ref(),
            ],
            &claims.program.key,
        )
        .0;
        let mut aggregate = vec![0_u8; LIABILITY_BASIS_MARKET_HEADER_BYTES_V2 + 3 * 8];
        encode_liability_basis_market_into_v2(
            LiabilityBasisMarketInputV2 {
                revision: 11,
                logical_market: market_key.to_bytes(),
                release_set: release_set_digest,
                registry_program: registry.to_bytes(),
                product_instance_id: product_id,
                basis_id: semantic_basis,
                realm_id: realm_digest,
                // The aggregate owns the founding's permanent Market/Hoard
                // namespace.  Inline ordinary Direct uses a separate,
                // external-to-external replay at the buyer maker root; those
                // contexts must not be collapsed into one coordinate.
                custody_context: [0x6d; 32],
                generation: identity.generation,
            },
            &[30, 30, 30],
            &mut aggregate,
        )
        .expect("Claims aggregate");
        route.claims.aggregate.key = aggregate_key;
        route.claims.aggregate.owner = claims.program.key;
        route.claims.aggregate.data = aggregate;
        route.claims.aggregate.lamports = rent.minimum_balance(route.claims.aggregate.data.len());
        for (account, owner, revision, balances) in [
            (
                &mut route.claims.seller_position,
                authentication.seller.maker,
                12,
                [0, 0, 30],
            ),
            (
                &mut route.claims.buyer_position,
                authentication.buyer.maker,
                13,
                [0, 0, 0],
            ),
        ] {
            let seeds = ProtocolPositionSeedsV2::new(aggregate_key.to_bytes(), owner.to_bytes())
                .expect("Position seeds");
            account.key = Pubkey::find_program_address(&seeds.as_slices(), &claims.program.key).0;
            account.owner = claims.program.key;
            account.executable = false;
            account.data = vec![0_u8; LIABILITY_BASIS_POSITION_HEADER_BYTES_V2 + 3 * 8];
            encode_liability_basis_position_into_v2(
                LiabilityBasisPositionInputV2 {
                    revision,
                    market_account: aggregate_key.to_bytes(),
                    owner: owner.to_bytes(),
                    basis_id: semantic_basis,
                },
                &balances,
                &mut account.data,
            )
            .expect("Claims Position");
            account.lamports = rent.minimum_balance(account.data.len());
        }

        let custody_authority = Pubkey::find_program_address(
            &dclutch_custody::CustodyAuthoritySeedsV1::new(
                market_key.to_bytes(),
                release_set_digest,
            )
            .as_slices(),
            &custody.program.key,
        )
        .0;
        route.custody.custody_authority.key = custody_authority;
        route.custody.custody_authority.owner = system_program::ID;
        route.custody.custody_authority.executable = false;
        route.custody.custody_authority.data.clear();
        let replay = CustodyReplayV1 {
            caller_role: CallerRoleV1::Trading,
            release_set: release_set_digest,
            market: market_key.to_bytes(),
            realm: realm_digest,
            context: route.buyer_maker.key.to_bytes(),
            caller_program: trading.program.key.to_bytes(),
            rent_refund: route.payer.key.to_bytes(),
            open_vault_count: 1,
            next_revision: 14,
            generation: identity.generation,
            last_request_digest: [0xe1; 32],
            last_poststate_commitment: [0xe2; 32],
        };
        let replay_seeds = CustodyReplaySeedsV1::new(
            market_key.to_bytes(),
            release_set_digest,
            CallerRoleV1::Trading,
            route.buyer_maker.key.to_bytes(),
        );
        route.custody.replay.key =
            Pubkey::find_program_address(&replay_seeds.as_slices(), &custody.program.key).0;
        route.custody.replay.owner = custody.program.key;
        route.custody.replay.data = replay.to_bytes().expect("Custody replay").to_vec();
        route.custody.replay.lamports = rent.minimum_balance(route.custody.replay.data.len());

        route.custody.mint.data = {
            let mut mint = vec![0_u8; 82];
            mint[45] = 1;
            mint
        };
        route.custody.buyer_token.data = token_account_bytes(
            route.custody.mint.key,
            authentication.buyer.maker,
            10_000,
            Some(custody_authority),
            10,
        );
        route.custody.seller_token.data = token_account_bytes(
            route.custody.mint.key,
            authentication.seller.maker,
            0,
            None,
            0,
        );
        route.custody.fee_token.data =
            token_account_bytes(route.custody.mint.key, key(7), 0, None, 0);

        let request = compile_direct_inline_request_v3(
            authentication.seller,
            authentication.buyer,
            authentication.fill,
            authentication.execution_price,
        )
        .expect("family request");
        authentication.context = DirectOrdinaryAuthenticatedContextV3 {
            parent_request_digest: hash(&request).to_bytes(),
            config_content_id: config_digest,
            config,
            market: market_key.to_bytes(),
            generation: identity.generation,
            outcome_count: 3,
            slot: observation().slot,
            root_phase: 0,
            seller_next_nonce: 0,
            buyer_next_nonce: 0,
            root_open_maker_count: 0,
            seller_created: true,
            seller_bump_observation: 0,
            seller_bump: Pubkey::find_program_address(
                &dclutch_trading::successor::MakerReplaySeedsV1::new(
                    coordinates,
                    authentication.seller.maker.to_bytes(),
                )
                .expect("seller seeds")
                .as_slices(),
                &trading.program.key,
            )
            .1,
            seller_rent_principal_observation: 0,
            seller_rent_principal: rent.minimum_balance(DIRECT_MAKER_REPLAY_BYTES_V1),
            buyer_created: true,
            buyer_bump_observation: 0,
            buyer_bump: Pubkey::find_program_address(
                &dclutch_trading::successor::MakerReplaySeedsV1::new(
                    coordinates,
                    authentication.buyer.maker.to_bytes(),
                )
                .expect("buyer seeds")
                .as_slices(),
                &trading.program.key,
            )
            .1,
            buyer_rent_principal_observation: 0,
            buyer_rent_principal: rent.minimum_balance(DIRECT_MAKER_REPLAY_BYTES_V1),
            claims_market_revision: 11,
            seller_position_revision: 12,
            buyer_position_revision: 13,
            custody_revision: 14,
            release_set: release_set_digest,
            product_record_digest: hash(&route.fixed.product.raw.data).to_bytes(),
            semantic_basis,
            linked_basis_record_digest: hash(&route.fixed.linked_basis.raw.data).to_bytes(),
            trading_program: trading.program.key.to_bytes(),
            realm: realm_digest,
            mint: route.custody.mint.key.to_bytes(),
            token_program: route.custody.token_program.key.to_bytes(),
            seller_maker_root: route.seller_maker.key.to_bytes(),
            buyer_maker_root: route.buyer_maker.key.to_bytes(),
            system_program: system_program::ID.to_bytes(),
            custody_authority: custody_authority.to_bytes(),
            seller_rent_beneficiary: maker_rent_beneficiary().to_bytes(),
            seller_rent_beneficiary_observation: [0; 32],
            buyer_rent_beneficiary: maker_rent_beneficiary().to_bytes(),
            buyer_rent_beneficiary_observation: [0; 32],
            fee_token_account: route.custody.fee_token.key.to_bytes(),
            seller_token_account: route.custody.seller_token.key.to_bytes(),
            buyer_token_account: route.custody.buyer_token.key.to_bytes(),
            seller_native_signer: authentication.seller.maker.to_bytes(),
            buyer_native_signer: authentication.buyer.maker.to_bytes(),
        };
        let child = derive_direct_inline_child_authorities_v3(
            authentication.seller,
            authentication.buyer,
            authentication.fill,
            authentication.execution_price,
            authentication.context,
            &route.fixed.account_profile.raw.data,
            &route.fixed.transition.raw.data,
            &route.fixed.effect.raw.data,
        )
        .expect("child authorities");
        route.claims.caller_authority.key = child.claims_authority;
        for (account, key) in route
            .custody
            .caller_authorities
            .iter_mut()
            .zip(child.custody_authorities)
        {
            account.key = key;
        }
    }

    #[allow(clippy::too_many_lines)]
    fn named_fixture() -> (
        DirectInlineHotReportV3,
        DirectInlinePhysicalRouteV3,
        DirectInlineOrdinaryRouteV3,
        DirectInlineRouteAuthenticationV3,
        DirectInlineLookupTableProvisionV3,
        ObservedAccount,
        Pubkey,
    ) {
        let lengths = ordinary_logical_lengths();
        let bundle =
            build_direct_inline_ordinary_hot_bundle_v4(DirectInlineOrdinaryHotBundleInputV4 {
                account_profile: DirectInlineOrdinaryAccountProfileInputV3 {
                    logical_data_lengths: &lengths,
                },
                capacity_profile: [0x91; 32],
            })
            .expect("canonical ordinary bundle");
        let profile = AccountProfileV2::decode(&bundle.account_profile).expect("AccountProfile");
        let registry = key(49);
        let trading = key(35);
        let core_program = key(36);
        let claims_program = key(37);
        let custody_program = key(41);
        let token_program = key(40);
        let rent_program = key(66);
        let payer = key(65);
        let core_programdata =
            Pubkey::find_program_address(&[core_program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let trading_programdata =
            Pubkey::find_program_address(&[trading.as_ref()], &bpf_loader_upgradeable::ID).0;
        let claims_programdata =
            Pubkey::find_program_address(&[claims_program.as_ref()], &bpf_loader_upgradeable::ID).0;
        let custody_programdata =
            Pubkey::find_program_address(&[custody_program.as_ref()], &bpf_loader_upgradeable::ID)
                .0;
        let config =
            DirectExecutionConfigV1::new(100, 50, key(7).to_bytes()).expect("Direct config");
        let config_bytes = config.encode();

        let seller = SignedDirectIntentV3 {
            maker: key(2),
            signature: [0x11; 64],
            intent: CompactIntentV2 {
                side: 0,
                lifecycle: 1,
                outcome: 2,
                market: key(1).to_bytes(),
                generation: 7,
                nonce: 4,
                valid_from: 10,
                valid_through: 30,
                maximum_fill: 25,
                limit_price: 40,
                fee_basis_points: 50,
                collateral_account: key(20).to_bytes(),
            },
        };
        let buyer = SignedDirectIntentV3 {
            maker: key(3),
            signature: [0x22; 64],
            intent: CompactIntentV2 {
                side: 1,
                lifecycle: 1,
                outcome: 2,
                market: key(1).to_bytes(),
                generation: 7,
                nonce: 9,
                valid_from: 5,
                valid_through: 40,
                maximum_fill: 30,
                limit_price: 60,
                fee_basis_points: 50,
                collateral_account: key(21).to_bytes(),
            },
        };
        let family_request =
            compile_direct_inline_request_v3(seller, buyer, 20, 50).expect("family request");
        let context = DirectOrdinaryAuthenticatedContextV3 {
            parent_request_digest: hash(&family_request).to_bytes(),
            config_content_id: hash(&config_bytes).to_bytes(),
            config,
            market: key(1).to_bytes(),
            generation: 7,
            outcome_count: 3,
            slot: 20,
            root_phase: 0,
            seller_next_nonce: 4,
            buyer_next_nonce: 9,
            root_open_maker_count: 2,
            seller_created: false,
            seller_bump_observation: 1,
            seller_bump: 1,
            seller_rent_principal_observation: 100,
            seller_rent_principal: 100,
            buyer_created: false,
            buyer_bump_observation: 2,
            buyer_bump: 2,
            buyer_rent_principal_observation: 100,
            buyer_rent_principal: 100,
            claims_market_revision: 11,
            seller_position_revision: 12,
            buyer_position_revision: 13,
            custody_revision: 14,
            release_set: [0x31; 32],
            product_record_digest: hash(&vec![0; PRODUCT_RECORD_BYTES_V2]).to_bytes(),
            semantic_basis: key(33).to_bytes(),
            linked_basis_record_digest: hash(&vec![0; BASIS_WIDTH_OFFSET_V3 + 4]).to_bytes(),
            trading_program: trading.to_bytes(),
            realm: key(38).to_bytes(),
            mint: key(39).to_bytes(),
            token_program: token_program.to_bytes(),
            seller_maker_root: key(42).to_bytes(),
            buyer_maker_root: key(43).to_bytes(),
            system_program: system_program::ID.to_bytes(),
            custody_authority: key(45).to_bytes(),
            seller_rent_beneficiary: key(74).to_bytes(),
            seller_rent_beneficiary_observation: key(74).to_bytes(),
            buyer_rent_beneficiary: key(75).to_bytes(),
            buyer_rent_beneficiary_observation: key(75).to_bytes(),
            fee_token_account: key(48).to_bytes(),
            seller_token_account: key(20).to_bytes(),
            buyer_token_account: key(21).to_bytes(),
            seller_native_signer: seller.maker.to_bytes(),
            buyer_native_signer: buyer.maker.to_bytes(),
        };
        let mut authentication = DirectInlineRouteAuthenticationV3 {
            seller,
            buyer,
            fill: 20,
            execution_price: 50,
            context,
            programs: DirectInlineCheckedProgramAccountsV3 {
                core_program,
                core_programdata,
                trading_program: trading,
                trading_programdata,
                checked_execution_release_set: [0; CHECKED_MULTIPROGRAM_BYTES_V1],
                registry_program: registry,
                claims_program,
                claims_programdata,
                custody_program,
                rent_program,
                token_program,
            },
        };
        let decoded = match dclutch_trading::execution_v3::DirectExecutionRequestV3::decode(
            &family_request,
            3,
        )
        .expect("decode family request")
        {
            dclutch_trading::execution_v3::DirectExecutionRequestV3::InlineOrdinary(value) => {
                value
            }
            _ => unreachable!("ordinary request"),
        };
        dclutch_trading::ordinary_route_projection_v3::project_direct_inline_ordinary_child_requests_v3(
            decoded,
            context,
            &bundle.account_profile,
            &bundle.transition,
            &bundle.effect,
        )
        .expect("direct child projection");
        let child = derive_direct_inline_child_authorities_v3(
            seller,
            buyer,
            20,
            50,
            context,
            &bundle.account_profile,
            &bundle.transition,
            &bundle.effect,
        )
        .expect("projected child authorities");

        let mut market = profiled_account(profile, 23, key(1), core_program);
        market.owner = core_program;
        let mut root = profiled_account(profile, 0, key(60), trading);
        root.owner = trading;
        let mut config_raw = profiled_account(profile, 1, key(80), registry);
        config_raw.data = config_bytes.to_vec();
        let mut account_profile_raw = observed(81, false);
        account_profile_raw.key = key(81);
        account_profile_raw.owner = registry;
        account_profile_raw.data = bundle.account_profile.to_vec();
        let registry_record = |record_key: u8, staging_key: u8, data: Vec<u8>| {
            record(
                ObservedAccount {
                    observation: observation(),
                    key: key(record_key),
                    owner: registry,
                    lamports: 1_000_000,
                    executable: false,
                    data,
                },
                key(staging_key),
            )
        };
        let mut product = profiled_account(profile, 2, key(32), registry);
        product.owner = registry;
        let mut result_domain = profiled_account(profile, 18, key(82), registry);
        result_domain.owner = registry;
        let mut portfolio = profiled_account(profile, 3, key(83), registry);
        portfolio.owner = registry;
        let mut linked_basis = profiled_account(profile, 4, key(34), registry);
        linked_basis.owner = registry;

        let mut core = profiled_account(profile, 30, core_program, bpf_loader_upgradeable::ID);
        core.owner = bpf_loader_upgradeable::ID;
        let mut trading_account =
            profiled_account(profile, 26, trading, bpf_loader_upgradeable::ID);
        trading_account.owner = bpf_loader_upgradeable::ID;
        let mut registry_account =
            profiled_account(profile, 25, registry, bpf_loader_upgradeable::ID);
        registry_account.owner = bpf_loader_upgradeable::ID;
        let fixed = DirectHotFixedRouteV3 {
            market,
            root,
            manifest: registry_record(84, 184, vec![1]),
            program_set: registry_record(85, 185, vec![2]),
            descriptor: registry_record(86, 186, bundle.descriptor.to_vec()),
            config: record(config_raw, key(187)),
            account_profile: record(account_profile_raw, key(188)),
            request_profile: registry_record(89, 189, bundle.request_profile.to_vec()),
            transition: registry_record(90, 190, bundle.transition.to_vec()),
            effect: registry_record(91, 191, bundle.effect.to_vec()),
            lifecycle: registry_record(92, 192, bundle.lifecycle_policy.to_vec()),
            strategy: registry_record(93, 193, bundle.strategy.to_vec()),
            activation_cache: profiled_account(profile, 24, key(69), registry),
            core_program: core,
            core_programdata: profiled_account(
                profile,
                31,
                core_programdata,
                bpf_loader_upgradeable::ID,
            ),
            trading_program: trading_account,
            trading_programdata: profiled_account(
                profile,
                27,
                trading_programdata,
                bpf_loader_upgradeable::ID,
            ),
            registry_program: registry_account,
            rent_sysvar: ObservedAccount {
                key: sysvar::rent::ID,
                owner: sysvar::ID,
                ..profiled_account(profile, 22, sysvar::rent::ID, sysvar::ID)
            },
            instructions_sysvar: ObservedAccount {
                observation: observation(),
                key: sysvar::instructions::ID,
                owner: sysvar::ID,
                lamports: 1,
                executable: false,
                data: vec![0; 8],
            },
            product: record(product, key(194)),
            result_domain: record(result_domain, key(195)),
            portfolio: record(portfolio, key(196)),
            linked_basis: record(linked_basis, key(197)),
            capability_seal: ObservedAccount {
                observation: observation(),
                key: key(68),
                owner: trading,
                lamports: 1_000_000,
                executable: false,
                data: vec![1; 64],
            },
        };
        let mut claims_authority_account =
            profiled_account(profile, 12, child.claims_authority, system_program::ID);
        claims_authority_account.data.clear();
        let claims = DirectClaimsRouteV3 {
            caller_authority: claims_authority_account,
            aggregate: profiled_account(profile, 13, key(61), claims_program),
            claims_program: profiled_account(
                profile,
                28,
                claims_program,
                bpf_loader_upgradeable::ID,
            ),
            claims_programdata: profiled_account(
                profile,
                29,
                claims_programdata,
                bpf_loader_upgradeable::ID,
            ),
            seller_position: profiled_account(profile, 32, key(62), claims_program),
            buyer_position: profiled_account(profile, 33, key(63), claims_program),
        };
        let caller_authorities = [34_usize, 48, 62, 76].map(|coordinate| {
            let index = match coordinate {
                34 => 0,
                48 => 1,
                62 => 2,
                76 => 3,
                _ => unreachable!("known route"),
            };
            let mut account = profiled_account(
                profile,
                coordinate,
                child.custody_authorities[index],
                system_program::ID,
            );
            account.data.clear();
            account
        });
        let custody = DirectCustodyRouteV3 {
            caller_authorities,
            realm: record(profiled_account(profile, 40, key(73), registry), key(198)),
            replay: profiled_account(profile, 42, key(64), custody_program),
            mint: profiled_account(profile, 43, key(39), token_program),
            buyer_token: profiled_account(profile, 44, key(21), token_program),
            seller_token: profiled_account(profile, 45, key(20), token_program),
            fee_token: profiled_account(profile, 73, key(48), token_program),
            custody_authority: profiled_account(profile, 46, key(45), system_program::ID),
            token_program: profiled_account(profile, 47, token_program, bpf_loader_upgradeable::ID),
            custody_program: profiled_account(
                profile,
                usize::from(DIRECT_INLINE_CUSTODY_PROGRAM_ACCOUNT_V3),
                custody_program,
                bpf_loader_upgradeable::ID,
            ),
            custody_programdata: ObservedAccount {
                observation: observation(),
                key: custody_programdata,
                owner: bpf_loader_upgradeable::ID,
                lamports: 1_000_000,
                executable: false,
                data: vec![0; 1_024],
            },
        };
        let mut route = DirectInlineOrdinaryRouteV3 {
            fixed,
            seller_maker: profiled_account(profile, 5, key(42), trading),
            payer: profiled_account(profile, 6, payer, system_program::ID),
            lifecycle_rent_credit: lifecycle_rent_credit_account(profile, key(67), rent_program),
            buyer_maker: profiled_account(profile, 8, key(43), trading),
            rent_program: profiled_account(profile, 10, rent_program, bpf_loader_upgradeable::ID),
            system_program: profiled_account(profile, 11, system_program::ID, key(200)),
            claims,
            custody,
        };
        install_exact_chain_fixture_v3(&mut route, &mut authentication, bundle);
        let seller = authentication.seller;
        let buyer = authentication.buyer;
        let context = authentication.context;
        let family_request = compile_direct_inline_request_v3(
            seller,
            buyer,
            authentication.fill,
            authentication.execution_price,
        )
        .expect("installed family request");
        let child = derive_direct_inline_child_authorities_v3(
            seller,
            buyer,
            authentication.fill,
            authentication.execution_price,
            context,
            &route.fixed.account_profile.raw.data,
            &route.fixed.transition.raw.data,
            &route.fixed.effect.raw.data,
        )
        .expect("installed child authorities");
        super::authenticate_named_route_v3(&route, authentication)
            .expect("named route key and owner joins");
        let authenticated = assemble_authenticated_direct_inline_ordinary_route_v3(
            route.clone(),
            3,
            authentication,
        )
        .expect("authenticated named route");
        assert_eq!(authenticated.child_authorities, child);
        let same_snapshot_provision = build_direct_inline_lookup_table_provision_v3(
            &authenticated,
            payer,
            observation().slot,
        )
        .expect("durable same-snapshot ALT plan");
        assert_eq!(same_snapshot_provision.creation_slot, observation().slot);
        assert!(!same_snapshot_provision.extensions.is_empty());
        let physical = authenticated.sealed_execution_physical.clone();
        let accounts = authenticated
            .physical
            .fixed_accounts
            .iter()
            .chain(authenticated.physical.runtime_accounts.iter().skip(5))
            .map(|meta| AccountMeta {
                pubkey: meta.account.key,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect::<Vec<_>>();
        let envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(family_request.len()).expect("request width"),
            [0x41; 32],
            context.market,
            context.generation,
            context.release_set,
        )
        .expect("Hot envelope");
        let mut hot_instruction_data = envelope.to_bytes().to_vec();
        hot_instruction_data.extend_from_slice(&family_request);
        let native_bytes =
            direct_native_evidence_bytes_v3(DirectExecutionActionV3::InlineOrdinary, 3)
                .expect("native width");
        let mut scratch = vec![0_u8; native_bytes];
        let mut native_data = vec![0_u8; native_bytes];
        encode_direct_native_evidence_many_v3_atomic(
            DirectNativeEvidenceContainerV3::TradingHot,
            DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
            &hot_instruction_data,
            3,
            &[seller.signature, buyer.signature],
            &mut scratch,
            &mut native_data,
        )
        .expect("native evidence");
        let report = DirectInlineHotReportV3 {
            instructions: [
                ComputeBudgetInstruction::set_compute_unit_limit(DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1),
                ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
                Instruction {
                    program_id: ed25519_program::ID,
                    accounts: Vec::new(),
                    data: native_data,
                },
                Instruction {
                    program_id: trading,
                    accounts,
                    data: hot_instruction_data.clone(),
                },
            ],
            hot_instruction_data,
            observation: observation(),
            selected_program_schema: dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
            selected_program: [0x41; 32],
            outcome_count: 3,
            product_record: key(32).to_bytes(),
            trading_artifact_release: [0x42; 32],
            checked_manifest_digest: [0x43; 32],
            required_instruction_signers: vec![payer],
            preview: DirectInlineEconomicPreviewV3 {
                claim_transfer: 20,
                gross_collateral: 10,
                seller_net_collateral_credit: 10,
                buyer_collateral_debit: 10,
                total_fee_transfer: 0,
            },
        };
        let report = project_direct_inline_sealed_execution_report_v3(&report, &authenticated)
            .expect("sealed execution report projection");
        let provision = super::build_lookup_table_provision(&physical, payer, 76)
            .expect("request-specific ALT plan");
        let (_, lookup) =
            super::classify_direct_inline_ordinary_route_v3(&physical).expect("classify route");
        assert_eq!(lookup, provision.addresses);
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: None,
                last_extended_slot: 76,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(lookup),
        };
        let table = ObservedAccount {
            observation: observation(),
            key: provision.lookup_table,
            owner: solana_address_lookup_table_interface::program::id(),
            lamports: 9_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("ALT bytes"),
        };
        (
            report,
            physical,
            route,
            authentication,
            provision,
            table,
            payer,
        )
    }

    fn fixture() -> (
        DirectInlineHotReportV3,
        DirectInlinePhysicalRouteV3,
        DirectInlineLookupTableProvisionV3,
        ObservedAccount,
        Pubkey,
    ) {
        let payer = key(200);
        let trading_program = key(201);
        let mut fixed_accounts = (1_u8..=u8::try_from(HOT_FIXED_ACCOUNT_COUNT_V3).expect("width"))
            .map(|byte| ObservedAccountMetaV3 {
                account: observed(byte, false),
                is_signer: false,
                is_writable: false,
            })
            .collect::<Vec<_>>();
        fixed_accounts[HOT_TRADING_PROGRAM_ACCOUNT_V3].account = ObservedAccount {
            key: trading_program,
            executable: true,
            ..observed(201, true)
        };
        let market = fixed_accounts[HOT_MARKET_ACCOUNT_V3].account.key;
        let mut fixed_classes =
            vec![DirectInlineAddressClassV3::LookupStable; fixed_accounts.len()];
        fixed_classes[HOT_TRADING_PROGRAM_ACCOUNT_V3] = DirectInlineAddressClassV3::InlineProgram;
        let runtime_accounts = [
            HOT_ROOT_ACCOUNT_V3,
            HOT_CONFIG_RAW_ACCOUNT_V3,
            HOT_PRODUCT_RAW_ACCOUNT_V3,
            HOT_PORTFOLIO_RAW_ACCOUNT_V3,
            HOT_LINKED_BASIS_RAW_ACCOUNT_V3,
        ]
        .into_iter()
        .map(|index| fixed_accounts[index].clone())
        .chain(core::iter::once(ObservedAccountMetaV3 {
            account: ObservedAccount {
                key: payer,
                ..observed(200, false)
            },
            is_signer: true,
            is_writable: true,
        }))
        .collect::<Vec<_>>();
        assert_eq!(HOT_RUNTIME_FIXED_COORDINATE_COUNT_V3, 5);
        let runtime_classes = [DirectInlineAddressClassV3::LookupStable; 5]
            .into_iter()
            .chain(core::iter::once(DirectInlineAddressClassV3::InlineSigner))
            .collect::<Vec<_>>();
        let route = DirectInlinePhysicalRouteV3 {
            fixed_accounts,
            strategy_accounts: Vec::new(),
            runtime_accounts,
            fixed_classes,
            runtime_classes,
            observation: observation(),
        };
        let route = project_direct_inline_sealed_execution_physical_v3(&route)
            .expect("sealed execution projection");
        let accounts = route
            .fixed_accounts
            .iter()
            .chain(route.runtime_accounts.iter().skip(5))
            .map(|meta| AccountMeta {
                pubkey: meta.account.key,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            })
            .collect::<Vec<_>>();
        let seller = SignedDirectIntentV3 {
            maker: key(210),
            signature: [0x11; 64],
            intent: CompactIntentV2 {
                side: 0,
                lifecycle: 1,
                outcome: 1,
                market: market.to_bytes(),
                generation: 9,
                nonce: 0,
                valid_from: 1,
                valid_through: 100,
                maximum_fill: 10,
                limit_price: 40,
                fee_basis_points: 0,
                collateral_account: key(211).to_bytes(),
            },
        };
        let buyer = SignedDirectIntentV3 {
            maker: key(212),
            signature: [0x22; 64],
            intent: CompactIntentV2 {
                side: 1,
                lifecycle: 1,
                outcome: 1,
                market: seller.intent.market,
                generation: 9,
                nonce: 0,
                valid_from: 1,
                valid_through: 100,
                maximum_fill: 10,
                limit_price: 60,
                fee_basis_points: 0,
                collateral_account: key(213).to_bytes(),
            },
        };
        let request =
            compile_direct_inline_request_v3(seller, buyer, 10, 50).expect("canonical request");
        let envelope = HotExecutionEnvelopeV3::new(
            u32::try_from(request.len()).expect("request width"),
            [0x40; 32],
            seller.intent.market,
            9,
            [0x41; 32],
        )
        .expect("Hot envelope");
        let mut hot_instruction_data = envelope.to_bytes().to_vec();
        hot_instruction_data.extend_from_slice(&request);
        let native_bytes =
            direct_native_evidence_bytes_v3(DirectExecutionActionV3::InlineOrdinary, 3)
                .expect("native width");
        let mut scratch = vec![0_u8; native_bytes];
        let mut native_data = vec![0_u8; native_bytes];
        encode_direct_native_evidence_many_v3_atomic(
            DirectNativeEvidenceContainerV3::TradingHot,
            DIRECT_HOT_TRADING_INSTRUCTION_INDEX_V1,
            &hot_instruction_data,
            3,
            &[[0x11; 64], [0x22; 64]],
            &mut scratch,
            &mut native_data,
        )
        .expect("native evidence");
        let report = DirectInlineHotReportV3 {
            instructions: [
                ComputeBudgetInstruction::set_compute_unit_limit(DIRECT_HOT_COMPUTE_UNIT_LIMIT_V1),
                ComputeBudgetInstruction::request_heap_frame(DIRECT_HOT_HEAP_FRAME_BYTES_V1),
                Instruction {
                    program_id: ed25519_program::ID,
                    accounts: Vec::new(),
                    data: native_data,
                },
                Instruction {
                    program_id: trading_program,
                    accounts,
                    data: hot_instruction_data.clone(),
                },
            ],
            hot_instruction_data,
            observation: observation(),
            selected_program_schema: dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
            selected_program: [0x31; 32],
            outcome_count: 3,
            product_record: [0x32; 32],
            trading_artifact_release: [0x33; 32],
            checked_manifest_digest: [0x34; 32],
            required_instruction_signers: vec![payer],
            preview: DirectInlineEconomicPreviewV3 {
                claim_transfer: 1,
                gross_collateral: 1,
                seller_net_collateral_credit: 1,
                buyer_collateral_debit: 1,
                total_fee_transfer: 0,
            },
        };
        let provision = super::build_lookup_table_provision(&route, payer, 76)
            .expect("request-specific ALT plan");
        let (_, lookup) =
            super::classify_direct_inline_ordinary_route_v3(&route).expect("classify");
        assert_eq!(lookup, provision.addresses);
        let table = AddressLookupTable {
            meta: LookupTableMeta {
                authority: None,
                last_extended_slot: 76,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(lookup),
        };
        let table = ObservedAccount {
            observation: observation(),
            key: provision.lookup_table,
            owner: solana_address_lookup_table_interface::program::id(),
            lamports: 9_000_000,
            executable: false,
            data: table.serialize_for_tests().expect("ALT bytes"),
        };
        (report, route, provision, table, payer)
    }

    fn serialize_rent(rent: &Rent) -> Vec<u8> {
        let mut lamports = 1_u64;
        let mut data = vec![0_u8; Rent::size_of()];
        let owner = sysvar::ID;
        let key = sysvar::rent::ID;
        let mut info =
            AccountInfo::new(&key, false, false, &mut lamports, &mut data, &owner, false);
        rent.to_account_info(&mut info).expect("serialize Rent");
        data
    }

    fn canonicalize_record(
        record: &mut FinalizedRecordRouteV3,
        registry: Pubkey,
        rent: &Rent,
        schema: [u8; 32],
        digest: [u8; 32],
    ) {
        assert_eq!(hash(&record.raw.data).to_bytes(), digest);
        record.raw.key = Pubkey::find_program_address(
            &[
                dclutch_registry::record::RAW_RECORD_PDA_SEED_V1,
                &schema,
                &digest,
            ],
            &registry,
        )
        .0;
        record.raw.owner = registry;
        record.raw.lamports = rent.minimum_balance(record.raw.data.len());
        record.raw.executable = false;
        record.staging.key = Pubkey::find_program_address(
            &[
                dclutch_registry::record::STAGING_CURSOR_PDA_SEED_V1,
                &schema,
                &digest,
            ],
            &registry,
        )
        .0;
        record.staging.owner = system_program::ID;
        record.staging.lamports = 0;
        record.staging.executable = false;
        record.staging.data.clear();
    }

    fn seal_fixture() -> (
        DirectInlineOrdinaryRouteV3,
        DirectInlineRouteAuthenticationV3,
        Rent,
    ) {
        let (_, _, mut route, authentication, _, _, _) = named_fixture();
        let rent = Rent::default();
        route.fixed.rent_sysvar.data = serialize_rent(&rent);
        route.fixed.rent_sysvar.lamports = 1;
        let registry = route.fixed.registry_program.key;
        let descriptor_digest = hash(&route.fixed.descriptor.raw.data).to_bytes();
        let descriptor = dclutch_market::capability_program::v4::CapabilityProgramV4::decode(
            &route.fixed.descriptor.raw.data,
        )
        .expect("ordinary descriptor");
        canonicalize_record(
            &mut route.fixed.descriptor,
            registry,
            &rent,
            dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
            descriptor_digest,
        );
        canonicalize_record(
            &mut route.fixed.lifecycle,
            registry,
            &rent,
            descriptor.lifecycle().schema().to_bytes(),
            descriptor.lifecycle().program().to_bytes(),
        );
        canonicalize_record(
            &mut route.fixed.account_profile,
            registry,
            &rent,
            descriptor.account_profile().schema().to_bytes(),
            descriptor.account_profile().program().to_bytes(),
        );
        canonicalize_record(
            &mut route.fixed.request_profile,
            registry,
            &rent,
            descriptor.request_profile().schema().to_bytes(),
            descriptor.request_profile().program().to_bytes(),
        );
        canonicalize_record(
            &mut route.fixed.transition,
            registry,
            &rent,
            descriptor.transition().schema().to_bytes(),
            descriptor.transition().program().to_bytes(),
        );
        canonicalize_record(
            &mut route.fixed.effect,
            registry,
            &rent,
            descriptor.effect().schema().to_bytes(),
            descriptor.effect().program().to_bytes(),
        );
        let key = dclutch_vm::capability_seal::CapabilitySealKeyV1::new(
            dclutch_market::capability_program::v4::SCHEMA_RELEASE_ID,
            descriptor_digest,
            DirectExecutionActionV3::InlineOrdinary as u32,
            CheckedExecutionReleaseSetV1::decode(
                &authentication.programs.checked_execution_release_set,
            )
            .expect("checked release set")
            .artifacts()[2]
                .semantic_release_id()
                .to_bytes(),
            registry.to_bytes(),
        )
        .expect("seal key");
        route.fixed.capability_seal = ObservedAccount {
            observation: observation(),
            key: Pubkey::find_program_address(
                &key.seeds().as_slices(),
                &authentication.programs.trading_program,
            )
            .0,
            owner: system_program::ID,
            lamports: 1,
            executable: false,
            data: Vec::new(),
        };
        (route, authentication, rent)
    }

    #[test]
    fn coordinate_insertion_refuses_overwrite_and_out_of_bounds() {
        let mut coordinates = [None];
        assert_eq!(
            insert_once(
                &mut coordinates,
                0,
                7_u8,
                DirectInlineRouteErrorV3::FixedFrame,
            ),
            Ok(())
        );
        assert_eq!(
            insert_once(
                &mut coordinates,
                0,
                8_u8,
                DirectInlineRouteErrorV3::FixedFrame,
            ),
            Err(DirectInlineRouteErrorV3::FixedFrame)
        );
        assert_eq!(coordinates, [Some(7)]);
        assert_eq!(
            insert_once(
                &mut coordinates,
                1,
                9_u8,
                DirectInlineRouteErrorV3::ChildFrame,
            ),
            Err(DirectInlineRouteErrorV3::ChildFrame)
        );
        assert_eq!(coordinates, [Some(7)]);
    }

    #[test]
    fn seal_plan_is_chain_derived_and_poststate_is_exact() {
        let (route, authentication, rent) = seal_fixture();
        let plan = build_direct_inline_capability_seal_v3(route.clone(), 3, authentication)
            .expect("canonical seal plan");
        assert_eq!(plan.seal, route.fixed.capability_seal.key);
        assert_eq!(
            plan.rent_minimum_lamports,
            rent.minimum_balance(dclutch_vm::capability_seal::CAPABILITY_SEAL_BYTES_V1)
        );
        assert_eq!(plan.expected_final_lamports, plan.rent_minimum_lamports);
        assert_eq!(plan.instruction.program_id, route.fixed.trading_program.key);
        assert_eq!(
            plan.instruction.accounts.len(),
            HOT_FIXED_ACCOUNT_COUNT_V3 + 2
        );
        assert!(!plan.instruction.accounts[HOT_ROOT_ACCOUNT_V3].is_writable);
        assert!(
            plan.instruction.accounts
                [dclutch_market::capability_program::hot_v3::HOT_CAPABILITY_SEAL_ACCOUNT_V3]
                .is_writable
        );
        let payer_meta = plan
            .instruction
            .accounts
            .get(HOT_FIXED_ACCOUNT_COUNT_V3)
            .expect("payer meta");
        assert_eq!(payer_meta.pubkey, route.payer.key);
        assert!(payer_meta.is_signer && payer_meta.is_writable);
        let system_meta = plan
            .instruction
            .accounts
            .get(HOT_FIXED_ACCOUNT_COUNT_V3 + 1)
            .expect("System meta");
        assert_eq!(system_meta.pubkey, system_program::ID);
        assert!(!system_meta.is_signer && !system_meta.is_writable);
        let closure = dclutch_vm::capability_seal::SealedDescriptorClosureV1::decode(
            &plan.expected_body,
        )
        .expect("canonical expected seal");
        closure.require_key(plan.key).expect("exact key");

        let mut materialized_route = route.clone();
        materialized_route.fixed.capability_seal.owner = authentication.programs.trading_program;
        materialized_route.fixed.capability_seal.lamports = plan.expected_final_lamports;
        materialized_route.fixed.capability_seal.data = plan.expected_body.clone();
        let materialized =
            build_direct_inline_capability_seal_v3(materialized_route.clone(), 3, authentication)
                .expect("exact materialized seal is a resumable completed stage");
        assert!(materialized.already_materialized);
        assert_eq!(materialized.seal, plan.seal);
        assert_eq!(materialized.expected_body, plan.expected_body);

        let mut wrong_materialized = materialized_route.clone();
        *wrong_materialized
            .fixed
            .capability_seal
            .data
            .first_mut()
            .expect("materialized seal byte") ^= 1;
        assert_eq!(
            build_direct_inline_capability_seal_v3(wrong_materialized, 3, authentication),
            Err(DirectInlineRouteErrorV3::Seal)
        );
        let mut wrong_materialized = materialized_route.clone();
        wrong_materialized.fixed.capability_seal.owner = system_program::ID;
        assert_eq!(
            build_direct_inline_capability_seal_v3(wrong_materialized, 3, authentication),
            Err(DirectInlineRouteErrorV3::Seal)
        );
        let mut wrong_materialized = materialized_route;
        wrong_materialized.fixed.capability_seal.lamports =
            plan.rent_minimum_lamports.saturating_sub(1);
        assert_eq!(
            build_direct_inline_capability_seal_v3(wrong_materialized, 3, authentication),
            Err(DirectInlineRouteErrorV3::Seal)
        );

        let authenticated = assemble_authenticated_direct_inline_ordinary_route_v3(
            route.clone(),
            3,
            authentication,
        )
        .expect("seal route");
        let provision = super::build_lookup_table_provision(
            &authenticated.sealed_execution_physical,
            route.payer.key,
            observation().slot - 1,
        )
        .expect("request-specific ALT");
        let table_data = AddressLookupTable {
            meta: LookupTableMeta {
                authority: None,
                last_extended_slot: observation().slot - 1,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(provision.addresses.clone()),
        }
        .serialize_for_tests()
        .expect("ALT bytes");
        let table = ObservedAccount {
            observation: observation(),
            key: provision.lookup_table,
            owner: solana_address_lookup_table_interface::program::id(),
            lamports: 9_000_000,
            executable: false,
            data: table_data,
        };
        let routed = compile_direct_inline_capability_seal_routed_v0_v3(
            &plan,
            Hash::new_from_array([0x46; 32]),
            &provision,
            &table,
        )
        .expect("seal routes only after exact frozen ALT activation");
        assert_eq!(routed.required_signatures, 1);
        assert!(routed.wire_bytes <= crate::versioned::PACKET_DATA_BYTES);
        let mut mutable_table = table.clone();
        let mutable_data = AddressLookupTable {
            meta: LookupTableMeta {
                authority: Some(route.payer.key),
                last_extended_slot: observation().slot - 1,
                deactivation_slot: u64::MAX,
                ..LookupTableMeta::default()
            },
            addresses: Cow::Owned(provision.addresses.clone()),
        }
        .serialize_for_tests()
        .expect("mutable ALT bytes");
        mutable_table.data = mutable_data;
        assert_eq!(
            compile_direct_inline_capability_seal_routed_v0_v3(
                &plan,
                Hash::new_from_array([0x46; 32]),
                &provision,
                &mutable_table,
            ),
            Err(DirectInlineRoutedTransactionErrorV3::LookupTable)
        );

        let observed = ObservedAccount {
            observation: Observation {
                slot: observation().slot + 1,
                ..observation()
            },
            key: plan.seal,
            owner: plan.instruction.program_id,
            lamports: plan.expected_final_lamports,
            executable: false,
            data: plan.expected_body.clone(),
        };
        verify_direct_inline_capability_seal_v3(&plan, &observed)
            .expect("exact finalized seal poststate");

        let mut wrong = observed.clone();
        let first = wrong.data.first_mut().expect("seal byte");
        *first ^= 1;
        assert_eq!(
            verify_direct_inline_capability_seal_v3(&plan, &wrong),
            Err(DirectInlineRouteErrorV3::Seal)
        );
        let mut wrong = observed.clone();
        wrong.owner = system_program::ID;
        assert_eq!(
            verify_direct_inline_capability_seal_v3(&plan, &wrong),
            Err(DirectInlineRouteErrorV3::Seal)
        );
        let mut wrong = observed;
        wrong.lamports = wrong.lamports.saturating_add(1);
        assert_eq!(
            verify_direct_inline_capability_seal_v3(&plan, &wrong),
            Err(DirectInlineRouteErrorV3::Seal)
        );

        let (mut route, authentication, _) = seal_fixture();
        route.fixed.descriptor.raw.key = key(223);
        assert_eq!(
            build_direct_inline_capability_seal_v3(route, 3, authentication),
            Err(DirectInlineRouteErrorV3::DirectInline(
                crate::direct_inline_v3::Error::Observation(
                    crate::observation::ObservationError::AddressMismatch
                )
            ))
        );
        let (mut route, authentication, _) = seal_fixture();
        route.fixed.lifecycle.staging.data.push(1);
        assert_eq!(
            build_direct_inline_capability_seal_v3(route, 3, authentication),
            Err(DirectInlineRouteErrorV3::DirectInline(
                crate::direct_inline_v3::Error::Observation(
                    crate::observation::ObservationError::AddressMismatch
                )
            ))
        );
        let (route, mut authentication, _) = seal_fixture();
        authentication.programs.checked_execution_release_set[0] ^= 1;
        assert_eq!(
            build_direct_inline_capability_seal_v3(route, 3, authentication),
            Err(DirectInlineRouteErrorV3::ChildFrame)
        );
        let (mut route, authentication, _) = seal_fixture();
        route.fixed.capability_seal.key = key(224);
        assert_eq!(
            build_direct_inline_capability_seal_v3(route, 3, authentication),
            Err(DirectInlineRouteErrorV3::Seal)
        );
    }

    #[test]
    fn placement_merge_preserves_order_unions_privileges_and_refuses_class_alias() {
        let address = Pubkey::new_unique();
        let mut closure = vec![DirectInlineAddressPlacementV3 {
            address,
            is_signer: false,
            is_writable: false,
            class: DirectInlineAddressClassV3::LookupStable,
        }];
        merge_placement(
            &mut closure,
            DirectInlineAddressPlacementV3 {
                address,
                is_signer: false,
                is_writable: true,
                class: DirectInlineAddressClassV3::LookupStable,
            },
        )
        .expect("same semantic class aliases");
        assert_eq!(closure.len(), 1);
        assert!(closure[0].is_writable);
        assert_eq!(closure[0].address, address);
        assert_eq!(
            merge_placement(
                &mut closure,
                DirectInlineAddressPlacementV3 {
                    address,
                    is_signer: true,
                    is_writable: false,
                    class: DirectInlineAddressClassV3::InlineSigner,
                },
            ),
            Err(DirectInlineRouteErrorV3::Profile)
        );
        assert!(!closure[0].is_signer);
    }

    #[test]
    fn named_route_derives_child_authorities_packs_profile_and_refuses_substitution() {
        let (report, physical, route, authentication, provision, table, payer) = named_fixture();
        let plan = compile_direct_inline_routed_v0_v3(
            &report,
            &physical,
            payer,
            Hash::new_from_array([0x44; 32]),
            &provision,
            &table,
        )
        .expect("complete named route compiles");
        assert_eq!(plan.message.wire_bytes, 1_167);
        assert_eq!(plan.message.loaded_addresses, 57);
        assert_eq!(plan.message.message.static_account_keys().len(), 4);
        let base_lock_count =
            plan.message.message.static_account_keys().len() + plan.message.loaded_addresses;
        assert_eq!(base_lock_count, 61);
        assert_eq!(
            plan.message.message.static_account_keys(),
            &[
                payer,
                compute_budget::ID,
                ed25519_program::ID,
                report.instructions[3].program_id,
            ]
        );
        let VersionedMessage::V0(message) = &plan.message.message else {
            panic!("Direct Hot must compile as v0");
        };
        assert_eq!(message.instructions.len(), 4);
        assert_eq!(message.instructions[3].accounts.len(), 78);
        assert_eq!(physical.runtime_accounts.len(), 44);
        assert_eq!(provision.addresses.len(), 57);
        assert_eq!(message.address_table_lookups.len(), 1);
        assert_eq!(
            message.address_table_lookups[0].writable_indexes.len()
                + message.address_table_lookups[0].readonly_indexes.len(),
            57
        );
        assert_eq!(
            admit_direct_inline_devnet_account_lock_count_v3(base_lock_count + 3),
            Ok(())
        );
        assert_eq!(
            admit_direct_inline_devnet_account_lock_count_v3(base_lock_count + 4),
            Err(DirectInlineRoutedTransactionErrorV3::AccountLocks)
        );
        assert_eq!(
            crate::versioned::PACKET_DATA_BYTES - plan.message.wire_bytes,
            65
        );
        assert_eq!(plan.required_signers, vec![payer]);
        assert_eq!(physical.fixed_accounts.len(), HOT_FIXED_ACCOUNT_COUNT_V3);
        assert!(
            physical.runtime_accounts.len() < usize::from(DIRECT_INLINE_ORDINARY_FIXED_ACCOUNTS_V3)
        );
        assert!(
            physical
                .runtime_classes
                .iter()
                .any(|class| *class == DirectInlineAddressClassV3::InlineRequestBound)
        );

        let compile_shape = |candidate: &DirectInlinePhysicalRouteV3| {
            compile_direct_inline_routed_v0_v3(
                &report,
                candidate,
                payer,
                Hash::new_from_array([0x44; 32]),
                &provision,
                &table,
            )
        };
        let mut partial = physical.clone();
        partial.fixed_accounts[HOT_DESCRIPTOR_STAGING_ACCOUNT_V3].account =
            route.fixed.descriptor.staging.clone();
        assert_eq!(
            compile_shape(&partial),
            Err(DirectInlineRoutedTransactionErrorV3::Route(
                DirectInlineRouteErrorV3::FixedFrame
            ))
        );
        let mut seventh = physical.clone();
        seventh.fixed_accounts[HOT_CONFIG_STAGING_ACCOUNT_V3] =
            seventh.fixed_accounts[HOT_CONFIG_RAW_ACCOUNT_V3].clone();
        assert_eq!(
            compile_shape(&seventh),
            Err(DirectInlineRoutedTransactionErrorV3::Route(
                DirectInlineRouteErrorV3::FixedFrame
            ))
        );
        let mut wrong_pair = physical.clone();
        wrong_pair.fixed_accounts[HOT_DESCRIPTOR_STAGING_ACCOUNT_V3] =
            wrong_pair.fixed_accounts[HOT_ACCOUNT_PROFILE_RAW_ACCOUNT_V3].clone();
        assert_eq!(
            compile_shape(&wrong_pair),
            Err(DirectInlineRoutedTransactionErrorV3::Route(
                DirectInlineRouteErrorV3::FixedFrame
            ))
        );
        let mut escalated = physical.clone();
        escalated.fixed_accounts[HOT_EFFECT_STAGING_ACCOUNT_V3].is_writable = true;
        assert_eq!(
            compile_shape(&escalated),
            Err(DirectInlineRoutedTransactionErrorV3::Route(
                DirectInlineRouteErrorV3::FixedFrame
            ))
        );

        let mut aliased_positions = route.clone();
        aliased_positions.claims.buyer_position = aliased_positions.claims.seller_position.clone();
        assert_eq!(
            assemble_authenticated_direct_inline_ordinary_route_v3(
                aliased_positions,
                3,
                authentication,
            ),
            Err(DirectInlineRouteErrorV3::ChildFrame)
        );

        let mut wrong_replay_owner = route.clone();
        wrong_replay_owner.custody.replay.owner = key(239);
        assert_eq!(
            assemble_authenticated_direct_inline_ordinary_route_v3(
                wrong_replay_owner,
                3,
                authentication,
            ),
            Err(DirectInlineRouteErrorV3::ChildFrame)
        );

        for case in 0..7 {
            let mut substituted_program = route.clone();
            match case {
                0 => substituted_program.fixed.core_program.key = key(220),
                1 => substituted_program.fixed.registry_program.key = key(221),
                2 => substituted_program.claims.claims_program.key = key(222),
                3 => substituted_program.custody.custody_program.key = key(223),
                4 => substituted_program.custody.token_program.key = key(224),
                5 => substituted_program.rent_program.key = key(225),
                _ => substituted_program.system_program.key = key(226),
            }
            assert_eq!(
                assemble_authenticated_direct_inline_ordinary_route_v3(
                    substituted_program,
                    3,
                    authentication,
                ),
                Err(DirectInlineRouteErrorV3::ChildFrame),
                "CPI program substitution case {case}"
            );
        }

        let mut substituted_authority = route;
        substituted_authority.custody.caller_authorities[2].key = key(238);
        assert_eq!(
            assemble_authenticated_direct_inline_ordinary_route_v3(
                substituted_authority,
                3,
                authentication,
            ),
            Err(DirectInlineRouteErrorV3::ChildFrame)
        );
    }

    #[test]
    fn named_route_projects_canonical_ack_and_all_ten_complete_poststates() {
        let (sealed_report, _, route, authentication, _, _, _) = named_fixture();
        let authenticated = assemble_authenticated_direct_inline_ordinary_route_v3(
            route.clone(),
            3,
            authentication,
        )
        .expect("authenticated distinct route");
        let mut report = sealed_report.clone();
        report.selected_program = authenticated.chain.selected_program;
        report.outcome_count = authenticated.chain.outcome_count;
        report.product_record = authenticated.chain.product_record;
        report.trading_artifact_release = authenticated.chain.trading_artifact_release;
        report.checked_manifest_digest = authenticated.checked_manifest_digest;
        for (index, meta) in authenticated.physical.fixed_accounts.iter().enumerate() {
            report.instructions[3].accounts[index] = AccountMeta {
                pubkey: meta.account.key,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            };
        }
        let plan =
            prepare_direct_inline_hot_finalization_v3(route.clone(), 3, authentication, &report)
                .expect("exact exterior finalization");
        assert_eq!(plan.poststates.len(), 10);
        assert_eq!(
            plan.finalization.ack_bytes,
            plan.finalization.ack.to_bytes()
        );
        let addresses = core::array::from_fn(|index| plan.poststates[index].commitment.address);
        assert_eq!(
            addresses,
            [
                route.fixed.root.key.to_bytes(),
                route.seller_maker.key.to_bytes(),
                route.buyer_maker.key.to_bytes(),
                route.claims.aggregate.key.to_bytes(),
                route.claims.seller_position.key.to_bytes(),
                route.claims.buyer_position.key.to_bytes(),
                route.custody.replay.key.to_bytes(),
                route.custody.buyer_token.key.to_bytes(),
                route.custody.seller_token.key.to_bytes(),
                route.custody.fee_token.key.to_bytes(),
            ]
        );
        for poststate in &plan.poststates {
            assert_eq!(
                hash(&poststate.data).to_bytes(),
                poststate.commitment.data_digest
            );
            assert_eq!(
                poststate.data.len(),
                usize::try_from(poststate.commitment.data_len).expect("poststate width")
            );
        }
        let mut expected_sealed = sealed_report;
        expected_sealed.selected_program = authenticated.chain.selected_program;
        expected_sealed.outcome_count = authenticated.chain.outcome_count;
        expected_sealed.product_record = authenticated.chain.product_record;
        expected_sealed.trading_artifact_release = authenticated.chain.trading_artifact_release;
        expected_sealed.checked_manifest_digest = authenticated.checked_manifest_digest;
        assert_eq!(plan.sealed_report, expected_sealed);

        let mut substituted_report = report;
        substituted_report.product_record = [0x99; 32];
        let refused = prepare_direct_inline_hot_finalization_v3(
            route,
            3,
            authentication,
            &substituted_report,
        )
        .expect_err("substituted product record");
        assert_eq!(
            refused,
            DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::SealedReportFacts(
                    DirectInlineSealedReportFactsRefusalV3 {
                        product_record: true,
                        ..DirectInlineSealedReportFactsRefusalV3::default()
                    }
                )
            )
        );
        assert_eq!(
            refused.to_string(),
            "the projected sealed report disagrees with the authenticated chain on: product record digest"
        );
    }

    /// The clause split earns its keep only if a refusal NAMES its site.
    ///
    /// Three independent mutations of the same fixture, each reaching a
    /// different site, each asserted on the exact rendered sentence rather than
    /// on a discriminant. Before the split all three produced the identical
    /// four-word string "Direct Hot finalization: Finalization", which is what
    /// sent the first local Direct fill to a lane with no way to tell a missing
    /// token delegate from a corrupt strategy artifact.
    #[test]
    fn distinct_finalization_clauses_refuse_under_their_own_names() {
        let (sealed_report, _, route, authentication, _, _, _) = named_fixture();
        let authenticated = assemble_authenticated_direct_inline_ordinary_route_v3(
            route.clone(),
            3,
            authentication,
        )
        .expect("authenticated distinct route");
        let mut report = sealed_report;
        report.selected_program = authenticated.chain.selected_program;
        report.outcome_count = authenticated.chain.outcome_count;
        report.product_record = authenticated.chain.product_record;
        report.trading_artifact_release = authenticated.chain.trading_artifact_release;
        report.checked_manifest_digest = authenticated.checked_manifest_digest;
        for (index, meta) in authenticated.physical.fixed_accounts.iter().enumerate() {
            report.instructions[3].accounts[index] = AccountMeta {
                pubkey: meta.account.key,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            };
        }
        prepare_direct_inline_hot_finalization_v3(route.clone(), 3, authentication, &report)
            .expect("the unmutated fixture still finalizes");

        // 1. The caller's report names an account the assembled route does not.
        // Before the split this was indistinguishable from every clause below.
        let mut substituted_instruction = report.clone();
        substituted_instruction.instructions[3].accounts[0].pubkey = key(0xEE);
        let refused = prepare_direct_inline_hot_finalization_v3(
            route.clone(),
            3,
            authentication,
            &substituted_instruction,
        )
        .expect_err("a report instruction the route does not name");
        assert_eq!(
            refused,
            DirectInlineRouteErrorV3::Finalization(
                DirectInlineFinalizationRefusalV3::SealedReportProjection(
                    DirectInlineSealedReportProjectionRefusalV3::Instruction
                )
            )
        );
        assert_eq!(
            refused.to_string(),
            "the distinct-account report does not project onto the authenticated route: the report's instruction sequence differed from the route"
        );

        // 2. The buyer's collateral source cannot pay for the fill. This is the
        // clause an operator hits with an underfunded admission, and it is the
        // one the bare `Finalization` unit hid most expensively: the fix is on
        // the chain, not in the driver, and nothing in the old message said so.
        let buyer_token =
            TokenAccount::parse(&route.custody.buyer_token.data).expect("fixture buyer token");
        let buyer_delegate = match buyer_token.delegate {
            super::COption::Some(delegate) => Pubkey::new_from_array(delegate),
            super::COption::None => panic!("the fixture buyer token is delegated"),
        };
        let mut unfunded = route.clone();
        unfunded.custody.buyer_token.data = token_account_bytes(
            Pubkey::new_from_array(buyer_token.mint),
            Pubkey::new_from_array(buyer_token.owner),
            0,
            Some(buyer_delegate),
            buyer_token.delegated_amount,
        );
        let refused =
            prepare_direct_inline_hot_finalization_v3(unfunded, 3, authentication, &report)
                .expect_err("a buyer who cannot pay for the fill");
        assert_eq!(
            refused,
            DirectInlineRouteErrorV3::Finalization(DirectInlineFinalizationRefusalV3::Finalizer {
                error: DirectFinalizationErrorV3::Candidate,
                candidate: Some(DirectInlineCandidateErrorV2::Binding),
            })
        );
        assert_eq!(
            refused.to_string(),
            "the canonical Direct finalizer refused: Candidate, and the candidate partition re-run on the same inputs refuses at Binding"
        );

        // 3. The buyer can pay but Custody has no allowance to spend. A
        // DIFFERENT chain fact reaching the SAME clause -- recorded that way on
        // purpose: `Binding` is where this chase currently ends, and the pair
        // is the standing evidence of how much further it could usefully go.
        let mut unallowed = route.clone();
        unallowed.custody.buyer_token.data = token_account_bytes(
            Pubkey::new_from_array(buyer_token.mint),
            Pubkey::new_from_array(buyer_token.owner),
            buyer_token.amount,
            Some(buyer_delegate),
            0,
        );
        let refused =
            prepare_direct_inline_hot_finalization_v3(unallowed, 3, authentication, &report)
                .expect_err("a buyer whose delegate may spend nothing");
        assert_eq!(
            refused,
            DirectInlineRouteErrorV3::Finalization(DirectInlineFinalizationRefusalV3::Finalizer {
                error: DirectFinalizationErrorV3::Candidate,
                candidate: Some(DirectInlineCandidateErrorV2::Binding),
            })
        );
    }

    /// Which finalization clauses the assembled route reaches, and which are
    /// backstops behind an earlier gate.
    ///
    /// `assemble_authenticated_direct_inline_ordinary_route_v3` runs first and
    /// refuses in its own vocabulary, so several finalization clauses cannot
    /// fire through this entry point at all. That is not a reason to delete
    /// them -- they are the second reader of the same bytes -- but it IS a fact
    /// an operator reading a refusal needs, because a clause that never fires
    /// is a clause you should stop suspecting. Recorded as a test so it stays
    /// true rather than as a comment that decays.
    #[test]
    fn earlier_gates_own_the_clauses_the_finalizer_never_reaches() {
        let (sealed_report, _, route, authentication, _, _, _) = named_fixture();
        let authenticated = assemble_authenticated_direct_inline_ordinary_route_v3(
            route.clone(),
            3,
            authentication,
        )
        .expect("authenticated distinct route");
        let mut report = sealed_report;
        report.selected_program = authenticated.chain.selected_program;
        report.outcome_count = authenticated.chain.outcome_count;
        report.product_record = authenticated.chain.product_record;
        report.trading_artifact_release = authenticated.chain.trading_artifact_release;
        report.checked_manifest_digest = authenticated.checked_manifest_digest;
        for (index, meta) in authenticated.physical.fixed_accounts.iter().enumerate() {
            report.instructions[3].accounts[index] = AccountMeta {
                pubkey: meta.account.key,
                is_signer: meta.is_signer,
                is_writable: meta.is_writable,
            };
        }
        let buyer_token =
            TokenAccount::parse(&route.custody.buyer_token.data).expect("fixture buyer token");
        let mut undelegated = route.clone();
        undelegated.custody.buyer_token.data = token_account_bytes(
            Pubkey::new_from_array(buyer_token.mint),
            Pubkey::new_from_array(buyer_token.owner),
            buyer_token.amount,
            None,
            0,
        );
        let mut truncated_root = route.clone();
        truncated_root.fixed.root.data.clear();
        let mut grown_strategy = route.clone();
        grown_strategy.fixed.strategy.raw.data.push(0);
        let mut grown_descriptor = route.clone();
        grown_descriptor.fixed.descriptor.raw.data.push(0);
        let mut grown_replay = route.clone();
        grown_replay.seller_maker.data.push(0);
        for (mutated, expected) in [
            // The undelegated buyer source: the child Custody FrameSpec reads
            // the delegate before the finalizer's collateral frame does.
            (undelegated, DirectInlineRouteErrorV3::ChildFrame),
            // An absent root, wearing the zero-length placeholder the finalized
            // snapshot renders for a MISSING account: AccountProfile geometry.
            (truncated_root, DirectInlineRouteErrorV3::Profile),
            // Artifact bytes the sealed descriptor closure does not cover.
            (
                grown_strategy,
                DirectInlineRouteErrorV3::DirectInline(
                    crate::direct_inline_v3::Error::Observation(
                        crate::observation::ObservationError::AddressMismatch,
                    ),
                ),
            ),
            (
                grown_descriptor,
                DirectInlineRouteErrorV3::DirectInline(
                    crate::direct_inline_v3::Error::Observation(
                        crate::observation::ObservationError::AddressMismatch,
                    ),
                ),
            ),
            // A maker replay of the wrong width.
            (grown_replay, DirectInlineRouteErrorV3::ChildFrame),
        ] {
            assert_eq!(
                prepare_direct_inline_hot_finalization_v3(mutated, 3, authentication, &report),
                Err(expected)
            );
        }
    }

    #[test]
    fn devnet_account_lock_admission_accepts_64_and_refuses_65() {
        assert_eq!(admit_direct_inline_devnet_account_lock_count_v3(64), Ok(()));
        assert_eq!(
            admit_direct_inline_devnet_account_lock_count_v3(65),
            Err(DirectInlineRoutedTransactionErrorV3::AccountLocks)
        );
        assert_eq!(
            admit_direct_inline_devnet_account_lock_count_v3(0),
            Err(DirectInlineRoutedTransactionErrorV3::AccountLocks)
        );
    }

    /// The Custody hint slot follows the ENABLED Custody route, never slot 0.
    ///
    /// `expected_custody_slots` picks the terminal slot 0 only when
    /// `total_fee_transfer == 0`, and the non-terminal slot 1 -- the one that
    /// leaves the fee's allowance standing for the settlement transaction --
    /// whenever a fee is transferred. The devnet trade driver handed
    /// `custody_authority_bumps[0]` to the hot envelope, which is right for
    /// every zero-fee fill this protocol had ever assembled and wrong for the
    /// first fee-bearing one: Trading reproduced the terminal route's bump for
    /// the intermediate route's seeds, `create_program_address` refused the
    /// off-curve check, and the fill returned `Release` 771,347 CU in.
    ///
    /// Measured on cohort-8's market22 against the deployed ELF: the enabled
    /// slot's canonical bump was 252 and slot 0's was 255.
    #[test]
    fn the_custody_hint_slot_follows_a_fee_bearing_settlement() {
        let (_, _, route, authentication, _, _, _) = named_fixture();
        let child = derive_direct_inline_child_authorities_v3(
            authentication.seller,
            authentication.buyer,
            authentication.fill,
            authentication.execution_price,
            authentication.context,
            &route.fixed.account_profile.raw.data,
            &route.fixed.transition.raw.data,
            &route.fixed.effect.raw.data,
        )
        .expect("child authorities");

        // Hint slot 0 is the Claims route and is not at issue.
        assert_eq!(child.child_caller_bumps[0], child.claims_authority_bump);

        // The hint follows whatever slot the projection enabled -- never a
        // literal index.
        let slot = child.enabled_custody_slot.expect("an enabled Custody slot");
        assert_eq!(
            child.child_caller_bumps[1],
            child.custody_authority_bumps[usize::from(slot)],
        );

        // And a standing note on this fixture, so nobody reads the assertion
        // above as covering the bug. Its intents fill 20 at price 50 against a
        // price scale of 100, so gross is 10 and 50 bps of that FLOORS TO
        // ZERO. It is nominally fee-bearing and economically not, so it takes
        // the terminal slot 0 -- which is exactly the byte the old call site
        // hardcoded. Every fixture in this crate is in the same position, and
        // that is why indexing `[0]` survived here for as long as it did. The
        // discriminating coverage is the unit test below.
        assert_eq!(slot, 0, "this fixture's 50 bps floors to a zero fee");
    }

    /// The Custody hint slot follows the ENABLED Custody route, never slot 0.
    ///
    /// `expected_custody_slots` picks the terminal slot 0 only when
    /// `total_fee_transfer == 0`, and the non-terminal slot 1 -- the one that
    /// leaves the fee's allowance standing for the settlement transaction --
    /// whenever a fee is actually transferred. That selection is proven
    /// two-sidedly in the codec by
    /// `a_zero_fee_fill_closes_the_delegation_and_a_fee_bearing_one_does_not`.
    ///
    /// What no test covered was whether the hint handed to the hot envelope
    /// FOLLOWS that selection. It did not: the devnet trade driver passed
    /// `custody_authority_bumps[0]`, which is right for every zero-fee fill
    /// this protocol had ever assembled and wrong for the first fee-bearing
    /// one. Trading reproduced the terminal route's bump against the
    /// intermediate route's seeds, `create_program_address` refused the
    /// off-curve check, and the fill returned `Release` 771,347 CU in.
    ///
    /// Measured on cohort-8's market22 against the deployed ELF: the enabled
    /// slot's canonical bump was 252 and slot 0's was 255.
    #[test]
    fn the_child_caller_hint_follows_the_enabled_custody_slot() {
        // Four distinct bumps, so picking the wrong slot cannot coincidentally
        // reproduce the right byte and no assertion below can go vacuous.
        let bumps = [251_u8, 252, 253, 254];
        let claims = 250_u8;
        let dispatch = |slot: u8, count: u8| {
            dclutch_trading::inline_candidate_v2::DirectInlineEffectDispatchV2 {
                custody_slots: [slot],
                custody_count: count,
                child_dispatch_writable: [false; 4],
            }
        };

        // A zero-fee fill takes the terminal route at slot 0.
        assert_eq!(
            child_caller_hint_slots_v1(claims, bumps, dispatch(0, 1)),
            Ok(([claims, 251], Some(0))),
        );
        // A fee-bearing fill takes the intermediate route at slot 1, and the
        // hint must move with it. This is the case the old call site got wrong.
        assert_eq!(
            child_caller_hint_slots_v1(claims, bumps, dispatch(1, 1)),
            Ok(([claims, 252], Some(1))),
        );
        // The remaining shapes are not reachable from an inline fill today, but
        // the mapping must not silently hand back a neighbour's byte if one
        // ever becomes reachable.
        assert_eq!(
            child_caller_hint_slots_v1(claims, bumps, dispatch(2, 1)),
            Ok(([claims, 253], Some(2))),
        );
        assert_eq!(
            child_caller_hint_slots_v1(claims, bumps, dispatch(3, 1)),
            Ok(([claims, 254], Some(3))),
        );

        // No Custody child: the slot is ABSENT and the walk searches.
        assert_eq!(
            child_caller_hint_slots_v1(claims, bumps, dispatch(0, 0)),
            Ok(([claims, 0], None)),
        );

        // A slot outside the fixed four is refused rather than wrapped or
        // silently dropped.
        assert_eq!(
            child_caller_hint_slots_v1(claims, bumps, dispatch(4, 1)),
            Err(DirectInlineRouteErrorV3::ChildFrame),
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn complete_route_refuses_release_graph_participant_and_custody_substitutions() {
        let (_, _, route, authentication, _, _, _) = named_fixture();
        assert_ne!(
            route.fixed.product.raw.key.to_bytes(),
            hash(&route.fixed.product.raw.data).to_bytes(),
            "a Registry raw PDA is not its content digest"
        );
        assert_ne!(
            route.fixed.linked_basis.raw.key.to_bytes(),
            hash(&route.fixed.linked_basis.raw.data).to_bytes(),
            "linked-basis address and body digest remain distinct facts"
        );

        let mut hostile = route.clone();
        hostile.fixed.instructions_sysvar.key = key(230);
        assert!(
            assemble_authenticated_direct_inline_ordinary_route_v3(hostile, 3, authentication,)
                .is_err()
        );

        let mut hostile = route.clone();
        hostile.fixed.activation_cache.data[20] ^= 1;
        assert!(
            assemble_authenticated_direct_inline_ordinary_route_v3(hostile, 3, authentication,)
                .is_err()
        );

        for case in 0..4 {
            let mut hostile = route.clone();
            match case {
                0 => hostile.fixed.core_programdata.data[50] ^= 1,
                1 => hostile.fixed.trading_programdata.data[50] ^= 1,
                2 => hostile.claims.claims_programdata.data[50] ^= 1,
                _ => hostile.custody.custody_programdata.data[50] ^= 1,
            }
            assert!(
                assemble_authenticated_direct_inline_ordinary_route_v3(hostile, 3, authentication,)
                    .is_err(),
                "current deployment substitution case {case}"
            );
        }

        for case in 0..6 {
            let mut hostile = route.clone();
            match case {
                0 => hostile.fixed.manifest.raw.data[20] ^= 1,
                1 => hostile.fixed.program_set.raw.data[20] ^= 1,
                2 => hostile.fixed.config.raw.data[20] ^= 1,
                3 => hostile.fixed.strategy.raw.data[20] ^= 1,
                4 => hostile.fixed.product.raw.key = key(231),
                _ => hostile.fixed.linked_basis.staging.key = key(232),
            }
            assert!(
                assemble_authenticated_direct_inline_ordinary_route_v3(hostile, 3, authentication,)
                    .is_err(),
                "finalized graph/artifact substitution case {case}"
            );
        }

        let mut hostile = route.clone();
        hostile.fixed.market.data[120] ^= 1;
        assert!(
            assemble_authenticated_direct_inline_ordinary_route_v3(hostile, 3, authentication,)
                .is_err()
        );

        for case in 0..5 {
            let mut hostile = route.clone();
            match case {
                0 => hostile.seller_maker.key = key(233),
                1 => hostile.claims.aggregate.data[16] ^= 1,
                2 => hostile.claims.seller_position.data[56] ^= 1,
                3 => hostile.claims.buyer_position.key = key(234),
                _ => hostile.custody.replay.data[16] ^= 1,
            }
            assert!(
                assemble_authenticated_direct_inline_ordinary_route_v3(hostile, 3, authentication,)
                    .is_err(),
                "participant/replay substitution case {case}"
            );
        }

        for case in 0..7 {
            let mut hostile = route.clone();
            match case {
                0 => hostile.custody.realm.raw.data[20] ^= 1,
                1 => hostile.custody.mint.key = key(235),
                2 => hostile.custody.custody_authority.key = key(236),
                3 => hostile.custody.buyer_token.data[32] ^= 1,
                4 => hostile.custody.buyer_token.data[76] ^= 1,
                5 => hostile.custody.seller_token.data[32] ^= 1,
                _ => hostile.custody.fee_token.data[32] ^= 1,
            }
            assert!(
                assemble_authenticated_direct_inline_ordinary_route_v3(hostile, 3, authentication,)
                    .is_err(),
                "Realm/token/authority substitution case {case}"
            );
        }

        let mut hostile_context = authentication;
        hostile_context.context.custody_revision =
            hostile_context.context.custody_revision.saturating_add(1);
        assert_eq!(
            assemble_authenticated_direct_inline_ordinary_route_v3(
                route.clone(),
                3,
                hostile_context,
            ),
            Err(DirectInlineRouteErrorV3::ChildFrame)
        );

        let mut aliased_fixed = route.clone();
        aliased_fixed.fixed.strategy.raw.key = aliased_fixed.fixed.lifecycle.raw.key;
        assert_eq!(
            assemble_authenticated_direct_inline_ordinary_route_v3(
                aliased_fixed,
                3,
                authentication,
            ),
            Err(DirectInlineRouteErrorV3::FixedFrame)
        );

        let authenticated =
            assemble_authenticated_direct_inline_ordinary_route_v3(route, 3, authentication)
                .expect("complete route");
        assert_eq!(
            build_direct_inline_lookup_table_provision_v3(
                &authenticated,
                key(237),
                observation().slot,
            ),
            Err(DirectInlineRouteErrorV3::Observation)
        );
    }

    #[test]
    fn routed_compiler_loads_exact_route_union_and_keeps_payer_inline() {
        let (report, route, provision, table, payer) = fixture();
        let plan = compile_direct_inline_routed_v0_v3(
            &report,
            &route,
            payer,
            Hash::new_from_array([0x44; 32]),
            &provision,
            &table,
        )
        .expect("routed v0");
        assert_eq!(plan.required_signers, vec![payer]);
        let VersionedMessage::V0(message) = plan.message.message else {
            panic!("ordinary Direct must compile as v0");
        };
        assert!(message.account_keys.contains(&payer));
        assert!(message.address_table_lookups.iter().any(|lookup| {
            !lookup.writable_indexes.is_empty() || !lookup.readonly_indexes.is_empty()
        }));
    }

    #[test]
    fn routed_compiler_refuses_mutable_stale_or_substituted_tables() {
        let (report, route, provision, table, payer) = fixture();
        let decode = |account: &ObservedAccount| -> AddressLookupTable<'static> {
            let decoded = AddressLookupTable::deserialize(&account.data).expect("table");
            AddressLookupTable {
                meta: decoded.meta,
                addresses: Cow::Owned(decoded.addresses.to_vec()),
            }
        };

        let mut mutable = table.clone();
        let mut decoded = decode(&mutable);
        decoded.meta.authority = Some(payer);
        mutable.data = decoded.serialize_for_tests().expect("mutable table");
        assert_eq!(
            compile_direct_inline_routed_v0_v3(
                &report,
                &route,
                payer,
                Hash::new_from_array([0x44; 32]),
                &provision,
                &mutable,
            ),
            Err(DirectInlineRoutedTransactionErrorV3::LookupTable)
        );

        let mut stale = table.clone();
        let mut decoded = decode(&stale);
        decoded.meta.last_extended_slot = observation().slot;
        stale.data = decoded.serialize_for_tests().expect("stale table");
        assert_eq!(
            compile_direct_inline_routed_v0_v3(
                &report,
                &route,
                payer,
                Hash::new_from_array([0x44; 32]),
                &provision,
                &stale,
            ),
            Err(DirectInlineRoutedTransactionErrorV3::LookupTable)
        );

        let mut substituted = table.clone();
        let mut decoded = decode(&substituted);
        decoded.addresses.to_mut()[0] = key(239);
        substituted.data = decoded.serialize_for_tests().expect("substituted table");
        assert_eq!(
            compile_direct_inline_routed_v0_v3(
                &report,
                &route,
                payer,
                Hash::new_from_array([0x44; 32]),
                &provision,
                &substituted,
            ),
            Err(DirectInlineRoutedTransactionErrorV3::LookupTable)
        );
    }

    #[test]
    fn routed_compiler_refuses_instruction_or_class_substitution() {
        let (report, route, provision, table, payer) = fixture();
        let mut substituted = report.clone();
        substituted.instructions[3].accounts[0].is_writable ^= true;
        assert_eq!(
            compile_direct_inline_routed_v0_v3(
                &substituted,
                &route,
                payer,
                Hash::new_from_array([0x44; 32]),
                &provision,
                &table,
            ),
            Err(DirectInlineRoutedTransactionErrorV3::Instruction)
        );

        let mut reclassified = route.clone();
        reclassified.runtime_classes[5] = DirectInlineAddressClassV3::LookupStable;
        assert_eq!(
            compile_direct_inline_routed_v0_v3(
                &report,
                &reclassified,
                payer,
                Hash::new_from_array([0x44; 32]),
                &provision,
                &table,
            ),
            Err(DirectInlineRoutedTransactionErrorV3::Signer)
        );
    }
}
