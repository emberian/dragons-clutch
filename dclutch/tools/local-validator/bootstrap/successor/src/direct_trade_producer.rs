//! Typed producers for ordinary Direct sessions.
//!
//! This module consumes the campaign and participant parsers owned by their
//! respective exteriors. It never parses either report into a parallel DTO.
//! The owned-loopback fixture opens seeded keys only after deriving all public
//! coordinates. The devnet producer instead verifies caller-owned portable
//! tickets and records only the runtime payer keypair path; it never opens a
//! wallet, signs a packet, or submits a transaction.

use std::{
    collections::BTreeMap,
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1;
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
};
use dclutch_capability_program_contract::{
    set_v2::CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2, v4::CapabilityProgramV4,
};
use dclutch_capability_seal_contract::CAPABILITY_SEAL_PDA_DOMAIN_V1;
use dclutch_claims_svm::{
    liability_basis_state_v2::{LiabilityBasisMarketViewV2, LiabilityBasisPositionViewV2},
    protocol_position_v2::ProtocolPositionSeedsV2,
};
use dclutch_custody_contract::{
    CallerRoleV1, CustodyAuthoritySeedsV1, CustodyReplaySeedsV1, CustodyReplayV1,
};
use dclutch_direct_codec::{
    execution_v3::DirectExecutionActionV3,
    intent_v2::CompactIntentV2,
    replay_setup_v1::DirectReplaySetupRequestV1,
    successor::{
        DIRECT_MAKER_REPLAY_BYTES_V1, DirectCoordinatesV1, DirectExecutionConfigV1,
        DirectRootPhaseV1, DirectRootStateV1, MakerReplayRootV1, MakerReplaySeedsV1,
    },
    token_setup_v1::{
        DirectTokenAccountRoleV1, DirectTokenAccountSeedsV1, DirectTokenSetupRequestV1,
    },
};
use dclutch_market_core_codec::{CoreState, Phase as CorePhase};
use dclutch_operator::{
    direct_inline_route_v3::derive_direct_inline_child_authorities_v3,
    direct_inline_v3::{SignedDirectIntentV3, compile_direct_inline_request_v3},
};
use dclutch_product_payoff_v2_codec::registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3;
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_realm_contract::REALM_SCHEMA_RELEASE_ID_V1;
use dclutch_record_contract::{ContentDigest, RecordKeyV1, RecordPdaSeedsV1, SchemaReleaseId};
use dclutch_rent_contract::lifecycle_v2::LifecycleRentCreditV2;
use dclutch_token_svm::{ACCOUNT_BYTES, AccountState, COption, TokenAccount};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest as _, Sha256};
use solana_program::{hash::hash, rent::Rent};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signature, Signer as _},
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result, campaign,
    cluster::{
        ClusterOriginV1, DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH, ExpectedClusterV1,
    },
    direct_ticket::{parse_portable_direct_ticket_v1, sign_direct_intent_v1 as signed_intent_v1},
    direct_trade::{
        AuthenticatedDevnetDirectSessionSourceV1, authenticate_devnet_direct_session_source_v1,
    },
    local_mutable,
    model::{MarketRunInput, ProgramPin, RecordPair, SuccessorPlan},
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, WritePolicyV1, parse_json_without_duplicate_keys_v1},
    terminal_lifecycle::finalized_snapshot,
    user_position_admission,
};

pub(crate) const OWNED_PUBLIC_MANIFEST_SCHEMA_V1: &str =
    "dclutch-owned-loopback-direct-trade-public-manifest-v1";
pub(crate) const OWNED_PRIVATE_SESSION_SCHEMA_V1: &str =
    "dclutch-owned-loopback-direct-trade-private-session-v1";
pub(crate) const OWNED_PRODUCER_RECEIPT_SCHEMA_V1: &str =
    "dclutch-owned-loopback-direct-trade-producer-receipt-v1";
pub(crate) const DEVNET_PRIVATE_SESSION_SCHEMA_V1: &str =
    "dclutch-devnet-direct-trade-private-session-v1";
pub(crate) const DEVNET_SESSION_PRODUCER_JOURNAL_SCHEMA_V1: &str =
    "dclutch-devnet-direct-trade-session-producer-journal-v1";
pub(crate) const DEVNET_SESSION_PRODUCER_COMMAND_V1: &str =
    "devnet-direct-trade-session-produce-v1";
pub(crate) const DEVNET_DIRECT_PRODUCER_COMMAND_V1: &str = "devnet-direct-trade-produce-v1";
const DEVNET_PUBLIC_MANIFEST_SCHEMA_V1: &str = "dclutch-devnet-direct-trade-public-manifest-v1";
const DEVNET_DIRECT_PRODUCER_JOURNAL_SCHEMA_V1: &str =
    "dclutch-devnet-direct-trade-producer-journal-v1";

const FILL_ATOMS_V1: u64 = 100_000_000;
const EXECUTION_PRICE_V1: u64 = 500_000;
const FEE_BASIS_POINTS_V1: u16 = 50;
const EXPECTED_PRICE_SCALE_V1: u64 = 1_000_000;
const INTENT_LIFETIME_SLOTS_V1: u64 = 432_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OwnedLoopbackDirectProducerArgumentsV1 {
    pub(crate) rpc_url: String,
    pub(crate) plan: PathBuf,
    pub(crate) market_input: PathBuf,
    pub(crate) campaign_report: PathBuf,
    pub(crate) participant_report: PathBuf,
    pub(crate) key_dir: PathBuf,
    pub(crate) output_dir: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DevnetDirectSessionProducerArgumentsV1 {
    public_manifest: PathBuf,
    expected_public_manifest_sha256: String,
    plan: PathBuf,
    expected_plan_sha256: String,
    market_input: PathBuf,
    expected_market_input_sha256: String,
    seller_participant: PathBuf,
    expected_seller_participant_sha256: String,
    buyer_participant: PathBuf,
    expected_buyer_participant_sha256: String,
    payer_keypair: PathBuf,
    journal_dir: PathBuf,
    evidence_file: PathBuf,
    session: PathBuf,
    producer_journal: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DevnetDirectProducerArgumentsV1 {
    rpc_url: String,
    plan: PathBuf,
    expected_plan_sha256: String,
    market_input: PathBuf,
    expected_market_input_sha256: String,
    campaign_report: PathBuf,
    expected_campaign_report_sha256: String,
    buyer_participant: PathBuf,
    expected_buyer_participant_sha256: String,
    checked_execution_release: PathBuf,
    expected_checked_execution_release_sha256: String,
    seller_ticket: PathBuf,
    expected_seller_ticket_sha256: String,
    buyer_ticket: PathBuf,
    expected_buyer_ticket_sha256: String,
    payer: Pubkey,
    payer_keypair: PathBuf,
    output_dir: PathBuf,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DevnetDirectProducerPhaseV1 {
    Prepared,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DevnetDirectProducerJournalV1 {
    schema: String,
    phase: DevnetDirectProducerPhaseV1,
    cluster: String,
    genesis_hash: String,
    plan: String,
    plan_sha256: String,
    market_input: String,
    market_input_sha256: String,
    campaign_report: String,
    campaign_report_sha256: String,
    buyer_participant: String,
    buyer_participant_sha256: String,
    checked_execution_release: String,
    checked_execution_release_sha256: String,
    seller_ticket: String,
    seller_ticket_sha256: String,
    buyer_ticket: String,
    buyer_ticket_sha256: String,
    payer: String,
    payer_keypair: String,
    observation_slot: u64,
    public_manifest: String,
    public_manifest_sha256: String,
    public_manifest_base64: String,
    private_session: String,
    private_session_sha256: String,
    private_session_base64: String,
    journal_dir: String,
    evidence_file: String,
    previous_state_sha256: Option<String>,
    state_sha256: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum DevnetDirectSessionProducerPhaseV1 {
    Prepared,
    Finalized,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct DevnetDirectParticipantSourceV1 {
    report: String,
    report_sha256: String,
    owner: String,
    position: String,
    collateral: String,
    collateral_quantity_atoms: u64,
    replay: String,
    nonce: u64,
    admission_signature: String,
    admission_slot: u64,
    collateral_signature: String,
    collateral_slot: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DevnetDirectSessionProducerJournalV1 {
    schema: String,
    phase: DevnetDirectSessionProducerPhaseV1,
    cluster: String,
    genesis_hash: String,
    public_manifest: String,
    public_manifest_sha256: String,
    plan: String,
    plan_sha256: String,
    market_input: String,
    market_input_sha256: String,
    checked_execution_release_sha256: String,
    checked_binaries: BTreeMap<String, ProgramPin>,
    payer: String,
    payer_keypair: String,
    seller: DevnetDirectParticipantSourceV1,
    buyer: DevnetDirectParticipantSourceV1,
    seller_ticket_sha256: String,
    buyer_ticket_sha256: String,
    journal_dir: String,
    evidence_file: String,
    private_session: String,
    private_session_sha256: Option<String>,
    previous_state_sha256: Option<String>,
    state_sha256: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedSignedIntentManifestV1 {
    pub(crate) maker: String,
    pub(crate) intent_base64: String,
    pub(crate) signature_base64: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedRecordPairCoordinatesV1 {
    pub(crate) raw: String,
    pub(crate) staging: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedDirectFixedCoordinatesV1 {
    pub(crate) market: String,
    pub(crate) root: String,
    pub(crate) manifest: ProducedRecordPairCoordinatesV1,
    pub(crate) program_set: ProducedRecordPairCoordinatesV1,
    pub(crate) descriptor: ProducedRecordPairCoordinatesV1,
    pub(crate) config: ProducedRecordPairCoordinatesV1,
    pub(crate) account_profile: ProducedRecordPairCoordinatesV1,
    pub(crate) request_profile: ProducedRecordPairCoordinatesV1,
    pub(crate) transition: ProducedRecordPairCoordinatesV1,
    pub(crate) effect: ProducedRecordPairCoordinatesV1,
    pub(crate) lifecycle: ProducedRecordPairCoordinatesV1,
    pub(crate) strategy: ProducedRecordPairCoordinatesV1,
    pub(crate) activation_cache: String,
    pub(crate) core_program: String,
    pub(crate) core_programdata: String,
    pub(crate) trading_program: String,
    pub(crate) trading_programdata: String,
    pub(crate) registry_program: String,
    pub(crate) product: ProducedRecordPairCoordinatesV1,
    pub(crate) result_domain: ProducedRecordPairCoordinatesV1,
    pub(crate) portfolio: ProducedRecordPairCoordinatesV1,
    pub(crate) linked_basis: ProducedRecordPairCoordinatesV1,
    pub(crate) capability_seal: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedDirectClaimsCoordinatesV1 {
    pub(crate) caller_authority: String,
    pub(crate) aggregate: String,
    pub(crate) claims_program: String,
    pub(crate) claims_programdata: String,
    pub(crate) seller_position: String,
    pub(crate) buyer_position: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedDirectCustodyCoordinatesV1 {
    pub(crate) caller_authorities: [String; 4],
    pub(crate) realm: ProducedRecordPairCoordinatesV1,
    pub(crate) replay: String,
    pub(crate) mint: String,
    pub(crate) buyer_token: String,
    pub(crate) seller_token: String,
    pub(crate) fee_token: String,
    pub(crate) custody_authority: String,
    pub(crate) token_program: String,
    pub(crate) custody_program: String,
    pub(crate) custody_programdata: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedDirectRouteCoordinatesV1 {
    pub(crate) fixed: ProducedDirectFixedCoordinatesV1,
    pub(crate) seller_maker: String,
    pub(crate) payer: String,
    pub(crate) lifecycle_rent_credit: String,
    pub(crate) buyer_maker: String,
    pub(crate) rent_program: String,
    pub(crate) claims: ProducedDirectClaimsCoordinatesV1,
    pub(crate) custody: ProducedDirectCustodyCoordinatesV1,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedDirectContextHintsV1 {
    pub(crate) generation: u64,
    pub(crate) outcome_count: u32,
    pub(crate) root_phase: u8,
    pub(crate) seller_next_nonce: u64,
    pub(crate) buyer_next_nonce: u64,
    pub(crate) root_open_maker_count: u64,
    pub(crate) seller_created: bool,
    pub(crate) seller_bump_observation: u8,
    pub(crate) seller_bump: u8,
    pub(crate) seller_rent_principal_observation: u64,
    pub(crate) seller_rent_principal: u64,
    pub(crate) buyer_created: bool,
    pub(crate) buyer_bump_observation: u8,
    pub(crate) buyer_bump: u8,
    pub(crate) buyer_rent_principal_observation: u64,
    pub(crate) buyer_rent_principal: u64,
    pub(crate) claims_market_revision: u64,
    pub(crate) seller_position_revision: u64,
    pub(crate) buyer_position_revision: u64,
    pub(crate) custody_revision: u64,
    pub(crate) release_set: String,
    pub(crate) semantic_basis: String,
    pub(crate) seller_rent_beneficiary: String,
    pub(crate) seller_rent_beneficiary_observation: String,
    pub(crate) buyer_rent_beneficiary: String,
    pub(crate) buyer_rent_beneficiary_observation: String,
}

/// Wire-compatible with `direct_trade::DirectTradePublicManifestV1`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedDirectTradePublicManifestV1 {
    pub(crate) schema: String,
    pub(crate) cluster: String,
    pub(crate) genesis_hash: String,
    pub(crate) plan_sha256: String,
    pub(crate) market_input_sha256: String,
    pub(crate) market: String,
    pub(crate) payer: String,
    pub(crate) fill: u64,
    pub(crate) execution_price: u64,
    pub(crate) fee_basis_points: u16,
    pub(crate) fee_recipient: String,
    pub(crate) checked_execution_release_set_base64: String,
    pub(crate) seller: ProducedSignedIntentManifestV1,
    pub(crate) buyer: ProducedSignedIntentManifestV1,
    pub(crate) route: ProducedDirectRouteCoordinatesV1,
    pub(crate) context: ProducedDirectContextHintsV1,
    pub(crate) replay_setup: ProducedReplaySetupV1,
    pub(crate) token_setup: ProducedTokenSetupV1,
}

/// Wire-compatible with `direct_trade::DirectTradePrivateSessionV1`.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedDirectTradePrivateSessionV1 {
    pub(crate) schema: String,
    pub(crate) public_manifest: String,
    pub(crate) public_manifest_sha256: String,
    pub(crate) plan: String,
    pub(crate) market_input: String,
    pub(crate) payer_keypair: String,
    pub(crate) journal_dir: String,
    pub(crate) evidence_file: String,
    pub(crate) session_sha256: String,
}

/// Separate non-economic Token-2022 setup coordinates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedTokenSetupV1 {
    pub(crate) request_base64: String,
    pub(crate) request_sha256: String,
    pub(crate) seller_token: String,
    pub(crate) seller_owner: String,
    pub(crate) fee_token: String,
    pub(crate) fee_recipient: String,
    pub(crate) payer: String,
    pub(crate) rent_refund: String,
    pub(crate) mint: String,
    pub(crate) token_program: String,
    pub(crate) trading_program: String,
}

/// Separate non-economic Trading-mediated Custody replay setup coordinates.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct ProducedReplaySetupV1 {
    pub(crate) request_base64: String,
    pub(crate) request_sha256: String,
    pub(crate) maker: String,
    pub(crate) maker_root: String,
    pub(crate) custody_replay: String,
    pub(crate) payer: String,
    pub(crate) rent_refund: String,
    pub(crate) expected_initial_revision: u64,
    pub(crate) expected_resulting_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct OwnedLoopbackDirectProducerReceiptV1 {
    pub(crate) schema: String,
    pub(crate) status: String,
    pub(crate) producer_receipt: String,
    pub(crate) public_manifest: String,
    pub(crate) public_manifest_sha256: String,
    pub(crate) private_session: String,
    pub(crate) private_session_sha256: String,
    pub(crate) plan_sha256: String,
    pub(crate) market_input_sha256: String,
    pub(crate) campaign_report_sha256: String,
    pub(crate) participant_report_sha256: String,
    pub(crate) participant_admission_signature: String,
    pub(crate) participant_admission_slot: u64,
    pub(crate) participant_collateral_signature: String,
    pub(crate) participant_collateral_slot: u64,
    pub(crate) replay_setup: ProducedReplaySetupV1,
    pub(crate) token_setup: ProducedTokenSetupV1,
    pub(crate) receipt_sha256: String,
}

#[derive(Clone, Copy)]
struct MakerFactsV1 {
    created: bool,
    bump_observation: u8,
    bump: u8,
    next_nonce: u64,
    rent_principal_observation: u64,
    rent_principal: u64,
    rent_beneficiary_observation: Pubkey,
    rent_beneficiary: Pubkey,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct DirectTradeTermsV1 {
    pub(crate) outcome: u32,
    pub(crate) fill: u64,
    pub(crate) execution_price: u64,
    pub(crate) fee_basis_points: u16,
}

/// Which of the two prestates the chain admits a Direct token destination to be
/// in, as observed on chain before this trade is produced.
///
/// THE CHAIN ADMITS TWO PRESTATES AND THIS PRODUCER USED TO ADMIT ONE, which is
/// why no market in this tree could ever trade twice. The two are owned by two
/// different instructions and neither is optional:
///
/// - [`Vacant`](Self::Vacant) is what `direct_token_setup_v1` requires in order
///   to CREATE the account. `authenticate_setup` refuses on
///   `seller_account.owner != &system_program::ID || seller_account.data_len()
///   != 0` (`programs/dclutch-trading-sbf/src/direct_token_setup_v1.rs`), so a
///   destination that already exists cannot be set up again, ever.
/// - [`Initialized`](Self::Initialized) is what the TRADE requires in order to
///   EXECUTE against the account. `project_tokens_v3` in
///   `dclutch-direct-codec` refuses on owner, length, state, native profile,
///   Mint and token-owner (`crates/dclutch-direct-codec/src/direct_finalization_v3.rs`),
///   so a vacant destination cannot be paid.
///
/// A market's FIRST trade sees `Vacant` and runs token setup. Every later trade
/// sees `Initialized` and must skip it. Admitting only the first is a producer
/// that refuses every market it has already traded on -- the same defect WALL4
/// dissolved in the TypeScript panel on 2026-08-31, which the Rust producer
/// never got the pass for.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DirectTokenDestinationPrestateV1 {
    /// System-owned and data-empty. Token setup will create it.
    Vacant,
    /// The initialized Token-2022 account for this Realm Mint and this owner.
    /// Token setup has already run; the trade executes against it.
    Initialized,
}

#[derive(Clone)]
struct PreparedPublicFactsV1 {
    plan_sha256: String,
    market_sha256: String,
    campaign_sha256: String,
    participant_sha256: String,
    genesis_hash: String,
    market: Pubkey,
    /// The Open Market's PDA-authenticated identity generation.
    generation: u64,
    payer: Pubkey,
    fee_recipient: Pubkey,
    seller: Pubkey,
    buyer: Pubkey,
    mint: Pubkey,
    token_program: Pubkey,
    seller_token: Pubkey,
    fee_token: Pubkey,
    seller_maker: Pubkey,
    buyer_maker: Pubkey,
    custody_authority: Pubkey,
    seller_facts: MakerFactsV1,
    buyer_facts: MakerFactsV1,
    aggregate_view: LiabilityBasisMarketViewV2,
    seller_position_view: LiabilityBasisPositionViewV2,
    buyer_position_view: LiabilityBasisPositionViewV2,
    root_state: DirectRootStateV1,
    observation_slot: u64,
    outcome: u32,
    config: DirectExecutionConfigV1,
    config_bytes: Vec<u8>,
    account_profile_bytes: Vec<u8>,
    transition_bytes: Vec<u8>,
    effect_bytes: Vec<u8>,
    product_digest: [u8; 32],
    realm_digest: [u8; 32],
    linked_basis_digest: [u8; 32],
    checked_release: Vec<u8>,
    route_without_children: ProducedDirectRouteCoordinatesV1,
    replay_setup: ProducedReplaySetupV1,
    token_setup: ProducedTokenSetupV1,
    participant: user_position_admission::FinalizedDirectParticipantEvidenceV1,
}

/// The finalized facts each caller-owned ticket half must match exactly.
///
/// Lifted out of [`assemble_public_manifest_v1`] so the ticket gate is
/// reachable without seven hundred lines of chain reads behind it: a refusal
/// nobody can reproduce offline is a refusal nobody can diagnose.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FinalizedTicketExpectationV1 {
    seller: Pubkey,
    buyer: Pubkey,
    market: Pubkey,
    generation: u64,
    observation_slot: u64,
    seller_next_nonce: u64,
    buyer_next_nonce: u64,
    /// The Direct token PDA `direct_token_setup_v1` CREATES for the seller, not
    /// an admission-created participant collateral account: the seller half of
    /// this route has no admission, and the producer separately requires this
    /// address to be a System-owned data-empty prestate.
    seller_collateral: Pubkey,
    /// The buyer's own admission-created, Custody-delegated collateral account.
    buyer_collateral: Pubkey,
    terms: DirectTradeTermsV1,
}

impl FinalizedTicketExpectationV1 {
    fn of(public: &PreparedPublicFactsV1, terms: DirectTradeTermsV1) -> Self {
        Self {
            seller: public.seller,
            buyer: public.buyer,
            market: public.market,
            generation: public.generation,
            observation_slot: public.observation_slot,
            seller_next_nonce: public.seller_facts.next_nonce,
            buyer_next_nonce: public.buyer_facts.next_nonce,
            seller_collateral: public.seller_token,
            buyer_collateral: public.participant.collateral_account,
            terms,
        }
    }
}

/// Every clause of one ticket half that refuses, each naming what the ticket
/// carries and what the finalized chain requires.
///
/// This returns ALL of them rather than the first, and the caller prints them.
/// The single-line refusal this replaced could not distinguish a stale nonce
/// from a wrong collateral account from an expired validity window, and the
/// operator who hit it had to instrument the binary to find out which.
fn refusing_ticket_half_clauses_v1(
    label: &str,
    signed: SignedDirectIntentV3,
    maker: Pubkey,
    side: u8,
    next_nonce: u64,
    collateral: Pubkey,
    expected: &FinalizedTicketExpectationV1,
) -> Vec<String> {
    let intent = signed.intent;
    let terms = expected.terms;
    let mut refusing = Vec::new();
    if signed.maker != maker {
        refusing.push(format!(
            "{label} ticket maker {} is not the finalized {label} {maker}",
            signed.maker
        ));
    }
    if intent.side != side {
        refusing.push(format!("{label} ticket side {} is not {side}", intent.side));
    }
    if intent.lifecycle != 0 {
        refusing.push(format!(
            "{label} ticket lifecycle {} is not fill-or-kill (0)",
            intent.lifecycle
        ));
    }
    if intent.outcome != terms.outcome {
        refusing.push(format!(
            "{label} ticket outcome {} is not the trade outcome {}",
            intent.outcome, terms.outcome
        ));
    }
    if intent.market != expected.market.to_bytes() {
        refusing.push(format!(
            "{label} ticket Market {} is not the finalized Market {}",
            Pubkey::new_from_array(intent.market),
            expected.market
        ));
    }
    if intent.generation != expected.generation {
        refusing.push(format!(
            "{label} ticket generation {} is not the Open Market generation {}",
            intent.generation, expected.generation
        ));
    }
    if intent.nonce != next_nonce {
        refusing.push(format!(
            "{label} ticket nonce {} is not the finalized next nonce {next_nonce}",
            intent.nonce
        ));
    }
    if intent.valid_from > expected.observation_slot {
        refusing.push(format!(
            "{label} ticket validFrom {} is after the observation slot {}",
            intent.valid_from, expected.observation_slot
        ));
    }
    if intent.valid_through < expected.observation_slot {
        refusing.push(format!(
            "{label} ticket validThrough {} is before the observation slot {}",
            intent.valid_through, expected.observation_slot
        ));
    }
    if intent.maximum_fill != terms.fill {
        refusing.push(format!(
            "{label} ticket maximumFill {} is not the exact fill {}",
            intent.maximum_fill, terms.fill
        ));
    }
    if intent.limit_price != terms.execution_price {
        refusing.push(format!(
            "{label} ticket limitPrice {} is not the execution price {}",
            intent.limit_price, terms.execution_price
        ));
    }
    if intent.fee_basis_points != terms.fee_basis_points {
        refusing.push(format!(
            "{label} ticket feeBasisPoints {} is not the config fee {}",
            intent.fee_basis_points, terms.fee_basis_points
        ));
    }
    if intent.collateral_account != collateral.to_bytes() {
        refusing.push(format!(
            "{label} ticket collateralAccount {} is not the finalized {label} collateral account {collateral}",
            Pubkey::new_from_array(intent.collateral_account)
        ));
    }
    refusing
}

/// Every clause on which the finalized buyer collateral account cannot fund
/// this exact trade, each naming what the chain holds and what it requires.
///
/// THE ALLOWANCE IS AN EQUALITY, NOT A FLOOR, and this function exists because
/// the producer used to model it as a floor over the wrong number entirely. It
/// tested `participant.collateral_quantity_atoms < required_buyer_collateral`
/// -- one `<` against the admission REPORT -- while `validate_collateral` in
/// `dclutch-direct-codec`, which is what the Trading program actually runs,
/// tests two different things about the finalized TOKEN ACCOUNT:
///
/// - `balance < buyer_collateral_debit` refuses. A floor, as expected.
/// - `delegated_amount != buyer_collateral_debit` refuses. An EQUALITY. The
///   delegation is a single-use authorization for exactly this trade, spent to
///   zero by the Custody effect; an allowance larger than the debit is not
///   generous, it is a different trade's authorization.
///
/// So an admission funded with MORE than the trade needs was accepted here and
/// refused on chain, and the chain said `Candidate`, and the exterior said
/// `Finalization`, and nothing anywhere named the number. That is the whole
/// distance between a produced session and a landed fill.
fn refusing_buyer_collateral_clauses_v1(
    account_data: &[u8],
    admitted_quantity_atoms: u64,
    custody_authority: Pubkey,
    required_buyer_collateral: u64,
) -> Result<Vec<String>> {
    let token = TokenAccount::parse(account_data)
        .map_err(|error| Error::new(format!("buyer collateral token account: {error:?}")))?;
    let mut refusing = Vec::new();
    if token.amount < required_buyer_collateral {
        refusing.push(format!(
            "its balance {} is below the {required_buyer_collateral} atoms this trade debits",
            token.amount
        ));
    }
    match token.delegate {
        COption::Some(delegate) if delegate == custody_authority.to_bytes() => {}
        COption::Some(delegate) => refusing.push(format!(
            "its delegate {} is not the Custody authority {custody_authority}",
            Pubkey::new_from_array(delegate)
        )),
        COption::None => {
            refusing.push("it carries no delegate, so Custody has no allowance to spend".to_owned())
        }
    }
    if token.delegated_amount != required_buyer_collateral {
        refusing.push(format!(
            "its delegated allowance {} is not exactly the {required_buyer_collateral} atoms this trade debits -- the allowance authorizes one trade and is spent to zero, so more is as refused as less",
            token.delegated_amount
        ));
    }
    // The admission REPORT's quantity is deliberately not a clause. The chain
    // reads this token account and nothing else at trade time, and adding a
    // rule the chain does not have is the same drift as missing one -- it just
    // fails in the safe direction, which is how a producer ends up refusing
    // trades the validator would accept. It is carried only to be NAMED beside
    // the allowance when they disagree, because that disagreement is the single
    // most useful sentence for an operator holding an oversized admission.
    if !refusing.is_empty() && admitted_quantity_atoms != token.delegated_amount {
        refusing.push(format!(
            "for context, the admission record says {admitted_quantity_atoms} atoms while the account delegates {}",
            token.delegated_amount
        ));
    }
    Ok(refusing)
}

/// Classify a Direct token destination against the two prestates the chain
/// admits, or name every clause on which it is neither.
///
/// Returns `Some(prestate)` with no clauses, or `None` with at least one. The
/// clauses are ALL of them, not the first, on PAIRFIX's
/// `refusing_ticket_half_clauses_v1` grounds: an operator who fixes one clause
/// and re-runs has learned almost nothing.
///
/// WHAT THIS DELIBERATELY DOES NOT REQUIRE, and each omission is a wall that
/// mirroring the wrong instruction would have built:
///
/// - **not `amount == 0`.** `direct_token_setup_v1`'s POSTSTATE is
///   `TokenAccount::initialized_base_bytes`, whose balance is zero, and mirroring
///   that here would admit a destination only until the first time it was paid --
///   a market that could trade exactly twice instead of exactly once. The trade
///   reads `seller.amount` as an observation and binds it to its own snapshot;
///   it never requires a particular value.
/// - **not `lamports == exact_rent`.** That clause belongs to token setup's rent
///   normalization, which is settling who paid for the account. The trade does
///   not read the destination's lamports at all.
/// - **not the delegate or close authority.** `project_tokens_v3` reads those
///   for the BUYER source, where the allowance is the authorization; for the
///   seller and fee destinations it reads neither, and a producer that invents
///   a rule the chain does not have fails in the safe direction, which is how a
///   producer ends up refusing trades the validator would accept.
///
/// What it DOES require is exactly `project_tokens_v3`'s own conjunction over a
/// destination: the token program owns it, it is `ACCOUNT_BYTES` long, it parses,
/// it is `Initialized`, it carries no native reserve, and its Mint and owner are
/// this trade's.
fn classify_direct_token_destination_v1(
    account: Option<&crate::rpc::RpcAccount>,
    mint: Pubkey,
    owner: Pubkey,
    token_program: Pubkey,
) -> (Option<DirectTokenDestinationPrestateV1>, Vec<String>) {
    // A MISSING account and an existing System-owned data-empty one are the same
    // prestate: the runtime renders the first as the second to any instruction
    // that declares it, which is exactly how token setup creates the PDA.
    let Some(account) = account else {
        return (Some(DirectTokenDestinationPrestateV1::Vacant), Vec::new());
    };
    if account.owner == system_program::ID && !account.executable && account.data.is_empty() {
        return (Some(DirectTokenDestinationPrestateV1::Vacant), Vec::new());
    }
    let mut refusing = Vec::new();
    if account.owner != token_program {
        refusing.push(format!(
            "it is owned by {} rather than the Realm token program {token_program}, and it is not the System-owned data-empty prestate token setup would create either",
            account.owner
        ));
    }
    if account.executable {
        refusing.push("it is executable".to_owned());
    }
    if account.data.len() != ACCOUNT_BYTES {
        refusing.push(format!(
            "it holds {} bytes rather than the {ACCOUNT_BYTES} of a Token-2022 account",
            account.data.len()
        ));
    }
    match TokenAccount::parse(&account.data) {
        Err(error) => refusing.push(format!("its bytes are not a token account: {error:?}")),
        Ok(token) => {
            if token.state != AccountState::Initialized {
                refusing.push(format!(
                    "its state is {:?} rather than Initialized",
                    token.state
                ));
            }
            if !token.native_reserve.is_none() {
                refusing
                    .push("it carries a native reserve, so it is a wrapped-SOL account".to_owned());
            }
            if token.mint != mint.to_bytes() {
                refusing.push(format!(
                    "its Mint {} is not this Market's collateral Mint {mint}",
                    Pubkey::new_from_array(token.mint)
                ));
            }
            if token.owner != owner.to_bytes() {
                refusing.push(format!(
                    "its token owner {} is not {owner}",
                    Pubkey::new_from_array(token.owner)
                ));
            }
        }
    }
    if refusing.is_empty() {
        (
            Some(DirectTokenDestinationPrestateV1::Initialized),
            refusing,
        )
    } else {
        (None, refusing)
    }
}

/// Both halves' refusing clauses, seller first.
fn refusing_ticket_clauses_v1(
    expected: &FinalizedTicketExpectationV1,
    seller_signed: SignedDirectIntentV3,
    buyer_signed: SignedDirectIntentV3,
) -> Vec<String> {
    let mut refusing = refusing_ticket_half_clauses_v1(
        "seller",
        seller_signed,
        expected.seller,
        0,
        expected.seller_next_nonce,
        expected.seller_collateral,
        expected,
    );
    refusing.extend(refusing_ticket_half_clauses_v1(
        "buyer",
        buyer_signed,
        expected.buyer,
        1,
        expected.buyer_next_nonce,
        expected.buyer_collateral,
        expected,
    ));
    refusing
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn assemble_public_manifest_v1(
    public: &PreparedPublicFactsV1,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    seller_signed: SignedDirectIntentV3,
    buyer_signed: SignedDirectIntentV3,
    terms: DirectTradeTermsV1,
    schema: &str,
    cluster: ExpectedClusterV1,
) -> Result<ProducedDirectTradePublicManifestV1> {
    let refusing = refusing_ticket_clauses_v1(
        &FinalizedTicketExpectationV1::of(public, terms),
        seller_signed,
        buyer_signed,
    );
    if !refusing.is_empty() {
        return Err(refusal(format!(
            "caller-owned Direct tickets differ from the finalized seller, buyer, nonce, validity, collateral, or exact trade terms: {}",
            refusing.join("; ")
        )));
    }
    let context = dclutch_direct_codec::ordinary_v3::DirectOrdinaryAuthenticatedContextV3 {
        parent_request_digest: hash(
            &compile_direct_inline_request_v3(
                seller_signed,
                buyer_signed,
                terms.fill,
                terms.execution_price,
            )
            .map_err(|error| Error::new(format!("Direct request: {error:?}")))?,
        )
        .to_bytes(),
        config_content_id: hash(&public.config_bytes).to_bytes(),
        config: public.config,
        market: public.market.to_bytes(),
        generation: public.generation,
        outcome_count: public.aggregate_view.claim_count,
        slot: public.observation_slot,
        root_phase: 0,
        seller_next_nonce: public.seller_facts.next_nonce,
        buyer_next_nonce: public.buyer_facts.next_nonce,
        root_open_maker_count: public.root_state.open_maker_root_count(),
        seller_created: public.seller_facts.created,
        seller_bump_observation: public.seller_facts.bump_observation,
        seller_bump: public.seller_facts.bump,
        seller_rent_principal_observation: public.seller_facts.rent_principal_observation,
        seller_rent_principal: public.seller_facts.rent_principal,
        buyer_created: public.buyer_facts.created,
        buyer_bump_observation: public.buyer_facts.bump_observation,
        buyer_bump: public.buyer_facts.bump,
        buyer_rent_principal_observation: public.buyer_facts.rent_principal_observation,
        buyer_rent_principal: public.buyer_facts.rent_principal,
        claims_market_revision: public.aggregate_view.revision,
        seller_position_revision: public.seller_position_view.revision,
        buyer_position_revision: public.buyer_position_view.revision,
        custody_revision: 1,
        release_set: hex32(&plan.release_set_id)?,
        product_record_digest: public.product_digest,
        semantic_basis: public.aggregate_view.basis_id,
        linked_basis_record_digest: public.linked_basis_digest,
        trading_program: pubkey(&plan.trading.program_id)?.to_bytes(),
        realm: public.realm_digest,
        mint: public.mint.to_bytes(),
        token_program: public.token_program.to_bytes(),
        seller_maker_root: public.seller_maker.to_bytes(),
        buyer_maker_root: public.buyer_maker.to_bytes(),
        system_program: system_program::ID.to_bytes(),
        custody_authority: public.custody_authority.to_bytes(),
        seller_rent_beneficiary: public.seller_facts.rent_beneficiary.to_bytes(),
        seller_rent_beneficiary_observation: public
            .seller_facts
            .rent_beneficiary_observation
            .to_bytes(),
        buyer_rent_beneficiary: public.buyer_facts.rent_beneficiary.to_bytes(),
        buyer_rent_beneficiary_observation: public
            .buyer_facts
            .rent_beneficiary_observation
            .to_bytes(),
        fee_token_account: public.fee_token.to_bytes(),
        seller_token_account: public.seller_token.to_bytes(),
        buyer_token_account: public.participant.collateral_account.to_bytes(),
        seller_native_signer: public.seller.to_bytes(),
        buyer_native_signer: public.buyer.to_bytes(),
    };
    let child = derive_direct_inline_child_authorities_v3(
        seller_signed,
        buyer_signed,
        terms.fill,
        terms.execution_price,
        context,
        &public.account_profile_bytes,
        &public.transition_bytes,
        &public.effect_bytes,
    )
    .map_err(|error| Error::new(format!("Direct child authority projection: {error:?}")))?;
    let mut route = public.route_without_children.clone();
    route.claims.caller_authority = child.claims_authority.to_string();
    route.custody.caller_authorities = child.custody_authorities.map(|key| key.to_string());
    Ok(ProducedDirectTradePublicManifestV1 {
        schema: schema.into(),
        cluster: cluster.evidence_label().into(),
        genesis_hash: public.genesis_hash.clone(),
        plan_sha256: public.plan_sha256.clone(),
        market_input_sha256: public.market_sha256.clone(),
        market: public.market.to_string(),
        payer: public.payer.to_string(),
        fill: terms.fill,
        execution_price: terms.execution_price,
        fee_basis_points: terms.fee_basis_points,
        fee_recipient: public.fee_recipient.to_string(),
        checked_execution_release_set_base64: BASE64.encode(&public.checked_release),
        seller: signed_manifest_v1(seller_signed)?,
        buyer: signed_manifest_v1(buyer_signed)?,
        route,
        context: ProducedDirectContextHintsV1 {
            generation: public.generation,
            outcome_count: public.aggregate_view.claim_count,
            root_phase: 0,
            seller_next_nonce: public.seller_facts.next_nonce,
            buyer_next_nonce: public.buyer_facts.next_nonce,
            root_open_maker_count: public.root_state.open_maker_root_count(),
            seller_created: public.seller_facts.created,
            seller_bump_observation: public.seller_facts.bump_observation,
            seller_bump: public.seller_facts.bump,
            seller_rent_principal_observation: public.seller_facts.rent_principal_observation,
            seller_rent_principal: public.seller_facts.rent_principal,
            buyer_created: public.buyer_facts.created,
            buyer_bump_observation: public.buyer_facts.bump_observation,
            buyer_bump: public.buyer_facts.bump,
            buyer_rent_principal_observation: public.buyer_facts.rent_principal_observation,
            buyer_rent_principal: public.buyer_facts.rent_principal,
            claims_market_revision: public.aggregate_view.revision,
            seller_position_revision: public.seller_position_view.revision,
            buyer_position_revision: public.buyer_position_view.revision,
            custody_revision: 1,
            release_set: plan.release_set_id.clone(),
            semantic_basis: hex(&public.aggregate_view.basis_id),
            seller_rent_beneficiary: public.seller_facts.rent_beneficiary.to_string(),
            seller_rent_beneficiary_observation: public
                .seller_facts
                .rent_beneficiary_observation
                .to_string(),
            buyer_rent_beneficiary: public.buyer_facts.rent_beneficiary.to_string(),
            buyer_rent_beneficiary_observation: public
                .buyer_facts
                .rent_beneficiary_observation
                .to_string(),
        },
        replay_setup: public.replay_setup.clone(),
        token_setup: public.token_setup.clone(),
    })
}

/// Produce the two manifests and the explicit setup receipt.
#[allow(clippy::too_many_lines)]
pub(crate) fn produce_owned_loopback_direct_trade_v1(
    arguments: OwnedLoopbackDirectProducerArgumentsV1,
) -> Result<OwnedLoopbackDirectProducerReceiptV1> {
    let validated = validate_paths_v1(arguments)?;
    let plan_bytes = fs::read(&validated.arguments.plan)?;
    let market_bytes = fs::read(&validated.arguments.market_input)?;
    let campaign_bytes = fs::read(&validated.arguments.campaign_report)?;
    let participant_bytes = fs::read(&validated.arguments.participant_report)?;
    require_unique_json_v1(&plan_bytes, "Direct successor plan")?;
    require_unique_json_v1(&market_bytes, "Direct Market input")?;
    let plan: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let market_input: MarketRunInput = serde_json::from_slice(&market_bytes)?;
    let origin = ClusterOriginV1::parse(&validated.arguments.rpc_url, None)?;
    ExpectedClusterV1::OwnedLoopback.authenticate(&origin)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let campaign = campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_bytes,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    let participant = user_position_admission::parse_finalized_direct_participant_evidence_v1(
        &participant_bytes,
        &mut rpc,
    )?;
    local_mutable::authenticate_checked_local_mutable_plan_v1(&plan)?;
    let payer_identity = plan
        .core
        .upgrade_authority
        .as_deref()
        .ok_or_else(|| refusal("owned-loopback Direct plan revoked its retained payer"))
        .and_then(pubkey)?;
    let checked_release = local_mutable::checked_execution_release_set_bytes_v1(&plan)?.to_vec();
    let public = prepare_public_facts_v1(
        &mut rpc,
        &plan,
        &market_input,
        &campaign,
        participant,
        sha256_hex(&plan_bytes),
        sha256_hex(&market_bytes),
        sha256_hex(&campaign_bytes),
        sha256_hex(&participant_bytes),
        payer_identity,
        checked_release,
        None,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    if public.config.price_scale() != EXPECTED_PRICE_SCALE_V1
        || public.config.fee_basis_points() != FEE_BASIS_POINTS_V1
    {
        return Err(refusal(
            "owned-loopback Direct producer requires the exact 1,000,000 scale and 50-bps config",
        ));
    }

    // No secret-bearing file is opened above this line.
    let payer = read_keypair_v1(&validated.payer_keypair, "Direct payer")?;
    let seller = read_keypair_v1(&validated.seller_keypair, "Direct seller")?;
    let buyer = read_keypair_v1(&validated.buyer_keypair, "Direct buyer")?;
    if payer.pubkey() != public.payer
        || seller.pubkey() != public.seller
        || buyer.pubkey() != public.buyer
    {
        return Err(refusal(
            "a private key file did not expand to its evidence-derived public identity",
        ));
    }

    let valid_through = public
        .observation_slot
        .checked_add(INTENT_LIFETIME_SLOTS_V1)
        .ok_or_else(|| refusal("Direct signed-intent validity range overflowed"))?;
    let seller_intent = CompactIntentV2 {
        side: 0,
        lifecycle: 0,
        outcome: public.outcome,
        market: public.market.to_bytes(),
        generation: public.generation,
        nonce: public.seller_facts.next_nonce,
        valid_from: public.observation_slot,
        valid_through,
        maximum_fill: FILL_ATOMS_V1,
        limit_price: EXECUTION_PRICE_V1,
        fee_basis_points: FEE_BASIS_POINTS_V1,
        collateral_account: public.seller_token.to_bytes(),
    };
    let buyer_intent = CompactIntentV2 {
        side: 1,
        lifecycle: 0,
        outcome: public.outcome,
        market: public.market.to_bytes(),
        generation: public.generation,
        nonce: public.buyer_facts.next_nonce,
        valid_from: public.observation_slot,
        valid_through,
        maximum_fill: FILL_ATOMS_V1,
        limit_price: EXECUTION_PRICE_V1,
        fee_basis_points: FEE_BASIS_POINTS_V1,
        collateral_account: public.participant.collateral_account.to_bytes(),
    };
    let seller_signed = signed_intent_v1(&seller, seller_intent)?;
    let buyer_signed = signed_intent_v1(&buyer, buyer_intent)?;
    let terms = DirectTradeTermsV1 {
        outcome: public.outcome,
        fill: FILL_ATOMS_V1,
        execution_price: EXECUTION_PRICE_V1,
        fee_basis_points: FEE_BASIS_POINTS_V1,
    };
    let public_manifest = assemble_public_manifest_v1(
        &public,
        &plan,
        &market_input,
        seller_signed,
        buyer_signed,
        terms,
        OWNED_PUBLIC_MANIFEST_SCHEMA_V1,
        ExpectedClusterV1::OwnedLoopback,
    )?;
    let token_setup = public.token_setup.clone();
    let public_path = validated
        .arguments
        .output_dir
        .join("direct-trade-public.json");
    let session_path = validated
        .arguments
        .output_dir
        .join("direct-trade-session.json");
    let journal_dir = validated.arguments.output_dir.join("direct-trade-journal");
    let setup_journal_dir = journal_dir.join("setup");
    let evidence_path = validated
        .arguments
        .output_dir
        .join("direct-trade-finalized.json");
    let receipt_path = validated
        .arguments
        .output_dir
        .join("direct-trade-produced.json");
    let public_bytes = pretty_json_bytes_v1(&public_manifest)?;
    let public_sha256 = sha256_hex(&public_bytes);
    let mut private_session = ProducedDirectTradePrivateSessionV1 {
        schema: OWNED_PRIVATE_SESSION_SCHEMA_V1.into(),
        public_manifest: public_path.display().to_string(),
        public_manifest_sha256: public_sha256.clone(),
        plan: validated.arguments.plan.display().to_string(),
        market_input: validated.arguments.market_input.display().to_string(),
        payer_keypair: validated.payer_keypair.display().to_string(),
        journal_dir: journal_dir.display().to_string(),
        evidence_file: evidence_path.display().to_string(),
        session_sha256: String::new(),
    };
    private_session.session_sha256 = private_session_sha256_v1(&private_session)?;
    let session_bytes = pretty_json_bytes_v1(&private_session)?;
    let session_sha256 = sha256_hex(&session_bytes);
    fs::create_dir(&journal_dir)?;
    fs::create_dir(&setup_journal_dir)?;
    write_create_new_v1(&public_path, &public_bytes)?;
    write_create_new_v1(&session_path, &session_bytes)?;

    let mut receipt = OwnedLoopbackDirectProducerReceiptV1 {
        schema: OWNED_PRODUCER_RECEIPT_SCHEMA_V1.into(),
        status: "produced".into(),
        producer_receipt: receipt_path.display().to_string(),
        public_manifest: public_path.display().to_string(),
        public_manifest_sha256: public_sha256,
        private_session: session_path.display().to_string(),
        private_session_sha256: session_sha256,
        plan_sha256: public.plan_sha256,
        market_input_sha256: public.market_sha256,
        campaign_report_sha256: public.campaign_sha256,
        participant_report_sha256: public.participant_sha256,
        participant_admission_signature: public.participant.admission_signature,
        participant_admission_slot: public.participant.admission_slot,
        participant_collateral_signature: public.participant.collateral_signature,
        participant_collateral_slot: public.participant.collateral_slot,
        replay_setup: public.replay_setup,
        token_setup,
        receipt_sha256: String::new(),
    };
    receipt.receipt_sha256 = producer_receipt_sha256_v1(&receipt)?;
    write_create_new_v1(&receipt_path, &pretty_json_bytes_v1(&receipt)?)?;
    Ok(receipt)
}

/// CLI integration hook. `main.rs` only needs to dispatch arguments here.
pub(crate) fn run_owned_loopback(arguments: Vec<String>) -> Result<()> {
    let arguments = parse_arguments_v1(arguments)?;
    let receipt = produce_owned_loopback_direct_trade_v1(arguments)?;
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &receipt)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

pub(crate) fn usage() -> &'static str {
    "dclutch-local-successor-bootstrap local-private-validator-direct-trade-produce-v1 \
     --rpc-url http://127.0.0.1:PORT/ --plan ABSOLUTE_PLAN_JSON \
     --market-input ABSOLUTE_MARKET_JSON --campaign-report ABSOLUTE_CAMPAIGN_JSON \
     --participant-report ABSOLUTE_PARTICIPANT_JSON --key-dir ABSOLUTE_KEYS \
     --output-dir ABSOLUTE_EMPTY_OUTPUT_DIRECTORY"
}

pub(crate) fn devnet_session_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-direct-trade-session-produce-v1 \
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
     --public-manifest ABSOLUTE_JSON --expected-public-manifest-sha256 HEX64 \
     --plan ABSOLUTE_JSON --expected-plan-sha256 HEX64 \
     --market-input ABSOLUTE_JSON --expected-market-input-sha256 HEX64 \
     --seller-participant ABSOLUTE_JSON --expected-seller-participant-sha256 HEX64 \
     --buyer-participant ABSOLUTE_JSON --expected-buyer-participant-sha256 HEX64 \
     --payer-keypair ABSOLUTE_RUNTIME_KEYPAIR_JSON --journal-dir ABSOLUTE_DIR \
     --evidence-file ABSOLUTE_JSON --session ABSOLUTE_JSON \
     --producer-journal ABSOLUTE_JSON"
}

pub(crate) fn devnet_direct_usage() -> &'static str {
    "dclutch-local-successor-bootstrap devnet-direct-trade-produce-v1 \
     --rpc-url DEVNET_HTTPS_URL \
     --i-mean-devnet EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG \
     --plan ABSOLUTE_JSON --expected-plan-sha256 HEX64 \
     --market-input ABSOLUTE_JSON --expected-market-input-sha256 HEX64 \
     --campaign-report ABSOLUTE_JSON --expected-campaign-report-sha256 HEX64 \
     --buyer-participant ABSOLUTE_JSON --expected-buyer-participant-sha256 HEX64 \
     --checked-execution-release ABSOLUTE_JSON \
     --expected-checked-execution-release-sha256 HEX64 \
     --seller-ticket ABSOLUTE_JSON --expected-seller-ticket-sha256 HEX64 \
     --buyer-ticket ABSOLUTE_JSON --expected-buyer-ticket-sha256 HEX64 \
     --payer PUBKEY --payer-keypair ABSOLUTE_RUNTIME_KEYPAIR_JSON \
     --output-dir ABSOLUTE_EXISTING_EMPTY_DIRECTORY"
}

/// Build the existing public manifest and private-session wires from one
/// finalized founding seller, one finalized admitted buyer, and two portable
/// caller-owned tickets. This command reads devnet state but never opens the
/// payer keypair, signs a transaction, or submits a packet.
pub(crate) fn run_devnet_direct(arguments: Vec<String>) -> Result<()> {
    let parsed = parse_devnet_direct_arguments_v1(arguments)?;
    let journal = produce_devnet_direct_trade_v1(parsed)?;
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &journal)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

/// Produce only the existing devnet Direct private-session wire. The caller
/// supplies already signed tickets through the immutable public manifest and a
/// freshly created payer-keypair path; this function never opens that path and
/// performs no RPC, signing, or submission.
pub(crate) fn run_devnet_session(arguments: Vec<String>) -> Result<()> {
    let parsed = parse_devnet_session_arguments_v1(arguments)?;
    let journal = produce_devnet_direct_session_v1(parsed)?;
    let mut stdout = std::io::stdout();
    serde_json::to_writer_pretty(&mut stdout, &journal)?;
    stdout.write_all(b"\n")?;
    Ok(())
}

#[allow(clippy::too_many_lines)]
fn produce_devnet_direct_trade_v1(
    arguments: DevnetDirectProducerArgumentsV1,
) -> Result<DevnetDirectProducerJournalV1> {
    let plan = accepted_regular_v1(
        &arguments.plan,
        &arguments.expected_plan_sha256,
        "Direct successor plan",
    )?;
    let market_input = accepted_regular_v1(
        &arguments.market_input,
        &arguments.expected_market_input_sha256,
        "Direct Market input",
    )?;
    let campaign_report = accepted_regular_v1(
        &arguments.campaign_report,
        &arguments.expected_campaign_report_sha256,
        "Direct campaign report",
    )?;
    let buyer_participant = accepted_regular_v1(
        &arguments.buyer_participant,
        &arguments.expected_buyer_participant_sha256,
        "Direct buyer participant report",
    )?;
    let checked_execution_release = accepted_regular_v1(
        &arguments.checked_execution_release,
        &arguments.expected_checked_execution_release_sha256,
        "Direct checked execution release",
    )?;
    let seller_ticket = accepted_regular_v1(
        &arguments.seller_ticket,
        &arguments.expected_seller_ticket_sha256,
        "Direct seller ticket",
    )?;
    let buyer_ticket = accepted_regular_v1(
        &arguments.buyer_ticket,
        &arguments.expected_buyer_ticket_sha256,
        "Direct buyer ticket",
    )?;
    let payer_keypair = canonical_runtime_keypair_path_v1(&arguments.payer_keypair)?;
    let output_dir = canonical_directory_v1(&arguments.output_dir, "Direct output directory")?;
    let producer_journal = output_dir.join("direct-trade-producer.json");
    let public_manifest = output_dir.join("direct-trade-public.json");
    let private_session = output_dir.join("direct-trade-session.json");
    let journal_dir = output_dir.join("direct-trade-journal");
    let setup_journal_dir = journal_dir.join("setup");
    let evidence_file = output_dir.join("direct-trade-finalized.json");

    if let Some(existing) = load_devnet_direct_producer_journal_v1(&producer_journal)? {
        authenticate_devnet_direct_producer_inputs_v1(
            &existing,
            &arguments,
            &plan,
            &market_input,
            &campaign_report,
            &buyer_participant,
            &checked_execution_release,
            &seller_ticket,
            &buyer_ticket,
            &payer_keypair,
            &public_manifest,
            &private_session,
            &journal_dir,
            &evidence_file,
        )?;
        return recover_devnet_direct_producer_v1(
            existing,
            &producer_journal,
            &public_manifest,
            &private_session,
            &journal_dir,
            &setup_journal_dir,
            &plan,
            &market_input,
        );
    }
    for path in [
        &public_manifest,
        &private_session,
        &journal_dir,
        &evidence_file,
    ] {
        if path.exists() {
            return Err(refusal(format!(
                "Direct producer target exists without its write-ahead journal: {}",
                path.display()
            )));
        }
    }

    let plan_bytes = fs::read(&plan)?;
    let market_bytes = fs::read(&market_input)?;
    let campaign_bytes = fs::read(&campaign_report)?;
    let participant_bytes = fs::read(&buyer_participant)?;
    require_unique_json_v1(&plan_bytes, "Direct successor plan")?;
    require_unique_json_v1(&market_bytes, "Direct Market input")?;
    let plan_value: SuccessorPlan = serde_json::from_slice(&plan_bytes)?;
    let market_value: MarketRunInput = serde_json::from_slice(&market_bytes)?;
    let campaign = campaign::parse_campaign_terminal_evidence_with_expected_cluster_v1(
        &campaign_bytes,
        ExpectedClusterV1::Devnet,
    )?;
    let origin = ClusterOriginV1::parse(&arguments.rpc_url, Some(DEVNET_GENESIS_HASH))?;
    ExpectedClusterV1::Devnet.authenticate(&origin)?;
    let mut rpc = Rpc::connect_cluster(&origin, WritePolicyV1::ReadsOnly)?;
    let participant =
        user_position_admission::parse_finalized_direct_participant_evidence_for_cluster_v1(
            &participant_bytes,
            &mut rpc,
            ExpectedClusterV1::Devnet,
        )?;
    let seller_signed = parse_portable_direct_ticket_v1(&fs::read(&seller_ticket)?, "seller")?;
    let buyer_signed = parse_portable_direct_ticket_v1(&fs::read(&buyer_ticket)?, "buyer")?;
    let terms = exact_ticket_pair_terms_v1(&seller_signed, &buyer_signed)?;
    let public = prepare_public_facts_v1(
        &mut rpc,
        &plan_value,
        &market_value,
        &campaign,
        participant,
        arguments.expected_plan_sha256.clone(),
        arguments.expected_market_input_sha256.clone(),
        arguments.expected_campaign_report_sha256.clone(),
        arguments.expected_buyer_participant_sha256.clone(),
        arguments.payer,
        fs::read(&checked_execution_release)?,
        Some(terms),
        ExpectedClusterV1::Devnet,
    )?;
    let public_value = assemble_public_manifest_v1(
        &public,
        &plan_value,
        &market_value,
        seller_signed,
        buyer_signed,
        terms,
        DEVNET_PUBLIC_MANIFEST_SCHEMA_V1,
        ExpectedClusterV1::Devnet,
    )?;
    let public_bytes = pretty_json_bytes_v1(&public_value)?;
    let source =
        authenticate_devnet_direct_session_source_v1(&public_bytes, &plan_bytes, &market_bytes)?;
    if source.payer != arguments.payer
        || source.seller != public.seller
        || source.buyer != public.buyer
        || source.public_manifest_sha256 != sha256_hex(&public_bytes)
        || source.checked_execution_release_sha256
            != arguments.expected_checked_execution_release_sha256
    {
        return Err(refusal(
            "assembled Direct manifest changed payer, founding seller, admitted buyer, or canonical bytes",
        ));
    }
    let public_sha256 = sha256_hex(&public_bytes);
    let mut private_value = ProducedDirectTradePrivateSessionV1 {
        schema: DEVNET_PRIVATE_SESSION_SCHEMA_V1.into(),
        public_manifest: public_manifest.display().to_string(),
        public_manifest_sha256: public_sha256.clone(),
        plan: plan.display().to_string(),
        market_input: market_input.display().to_string(),
        payer_keypair: payer_keypair.display().to_string(),
        journal_dir: journal_dir.display().to_string(),
        evidence_file: evidence_file.display().to_string(),
        session_sha256: String::new(),
    };
    private_value.session_sha256 = private_session_sha256_v1(&private_value)?;
    let private_bytes = pretty_json_bytes_v1(&private_value)?;
    let mut prepared = DevnetDirectProducerJournalV1 {
        schema: DEVNET_DIRECT_PRODUCER_JOURNAL_SCHEMA_V1.into(),
        phase: DevnetDirectProducerPhaseV1::Prepared,
        cluster: ExpectedClusterV1::Devnet.evidence_label().into(),
        genesis_hash: DEVNET_GENESIS_HASH.into(),
        plan: plan.display().to_string(),
        plan_sha256: arguments.expected_plan_sha256,
        market_input: market_input.display().to_string(),
        market_input_sha256: arguments.expected_market_input_sha256,
        campaign_report: campaign_report.display().to_string(),
        campaign_report_sha256: arguments.expected_campaign_report_sha256,
        buyer_participant: buyer_participant.display().to_string(),
        buyer_participant_sha256: arguments.expected_buyer_participant_sha256,
        checked_execution_release: checked_execution_release.display().to_string(),
        checked_execution_release_sha256: arguments.expected_checked_execution_release_sha256,
        seller_ticket: seller_ticket.display().to_string(),
        seller_ticket_sha256: arguments.expected_seller_ticket_sha256,
        buyer_ticket: buyer_ticket.display().to_string(),
        buyer_ticket_sha256: arguments.expected_buyer_ticket_sha256,
        payer: arguments.payer.to_string(),
        payer_keypair: payer_keypair.display().to_string(),
        observation_slot: public.observation_slot,
        public_manifest: public_manifest.display().to_string(),
        public_manifest_sha256: public_sha256,
        public_manifest_base64: BASE64.encode(&public_bytes),
        private_session: private_session.display().to_string(),
        private_session_sha256: sha256_hex(&private_bytes),
        private_session_base64: BASE64.encode(&private_bytes),
        journal_dir: journal_dir.display().to_string(),
        evidence_file: evidence_file.display().to_string(),
        previous_state_sha256: None,
        state_sha256: String::new(),
    };
    prepared.state_sha256 = devnet_direct_producer_state_sha256_v1(&prepared)?;
    write_create_new_durable_v1(&producer_journal, &pretty_json_bytes_v1(&prepared)?)?;
    recover_devnet_direct_producer_v1(
        prepared,
        &producer_journal,
        &public_manifest,
        &private_session,
        &journal_dir,
        &setup_journal_dir,
        &plan,
        &market_input,
    )
}

pub(crate) fn exact_ticket_pair_terms_v1(
    seller: &SignedDirectIntentV3,
    buyer: &SignedDirectIntentV3,
) -> Result<DirectTradeTermsV1> {
    if seller.maker == buyer.maker
        || seller.intent.side != 0
        || buyer.intent.side != 1
        || seller.intent.lifecycle != 0
        || buyer.intent.lifecycle != 0
        || seller.intent.outcome != buyer.intent.outcome
        || seller.intent.market != buyer.intent.market
        || seller.intent.generation != buyer.intent.generation
        || seller.intent.maximum_fill == 0
        || seller.intent.maximum_fill != buyer.intent.maximum_fill
        || seller.intent.limit_price == 0
        || seller.intent.limit_price != buyer.intent.limit_price
        || seller.intent.fee_basis_points != buyer.intent.fee_basis_points
    {
        return Err(refusal(
            "portable Direct tickets do not form one exact, distinct-maker fill-or-kill crossing",
        ));
    }
    Ok(DirectTradeTermsV1 {
        outcome: seller.intent.outcome,
        fill: seller.intent.maximum_fill,
        execution_price: seller.intent.limit_price,
        fee_basis_points: seller.intent.fee_basis_points,
    })
}

fn devnet_direct_producer_state_sha256_v1(
    journal: &DevnetDirectProducerJournalV1,
) -> Result<String> {
    let mut canonical = journal.clone();
    canonical.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn load_devnet_direct_producer_journal_v1(
    path: &Path,
) -> Result<Option<DevnetDirectProducerJournalV1>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    require_unique_json_v1(&bytes, "Direct full producer journal")?;
    let value = parse_json_without_duplicate_keys_v1(&bytes)?;
    let journal: DevnetDirectProducerJournalV1 = serde_json::from_value(value.clone())?;
    if serde_json::to_value(&journal)? != value
        || journal.schema != DEVNET_DIRECT_PRODUCER_JOURNAL_SCHEMA_V1
        || journal.cluster != ExpectedClusterV1::Devnet.evidence_label()
        || journal.genesis_hash != DEVNET_GENESIS_HASH
        || journal.observation_slot == 0
        || journal.state_sha256 != devnet_direct_producer_state_sha256_v1(&journal)?
    {
        return Err(refusal(
            "Direct full producer journal identity, fields, or self digest changed",
        ));
    }
    let mut prepared = journal.clone();
    prepared.phase = DevnetDirectProducerPhaseV1::Prepared;
    prepared.previous_state_sha256 = None;
    prepared.state_sha256.clear();
    let prepared_digest = devnet_direct_producer_state_sha256_v1(&prepared)?;
    match journal.phase {
        DevnetDirectProducerPhaseV1::Prepared if journal.previous_state_sha256.is_some() => {
            return Err(refusal(
                "prepared Direct full producer journal unexpectedly names a predecessor",
            ));
        }
        DevnetDirectProducerPhaseV1::Finalized
            if journal.previous_state_sha256.as_deref() != Some(prepared_digest.as_str()) =>
        {
            return Err(refusal(
                "finalized Direct full producer journal changed its exact prepared predecessor",
            ));
        }
        _ => {}
    }
    Ok(Some(journal))
}

#[allow(clippy::too_many_arguments)]
fn authenticate_devnet_direct_producer_inputs_v1(
    journal: &DevnetDirectProducerJournalV1,
    arguments: &DevnetDirectProducerArgumentsV1,
    plan: &Path,
    market_input: &Path,
    campaign_report: &Path,
    buyer_participant: &Path,
    checked_execution_release: &Path,
    seller_ticket: &Path,
    buyer_ticket: &Path,
    payer_keypair: &Path,
    public_manifest: &Path,
    private_session: &Path,
    journal_dir: &Path,
    evidence_file: &Path,
) -> Result<()> {
    let expected = [
        (
            &journal.plan,
            plan,
            &journal.plan_sha256,
            &arguments.expected_plan_sha256,
        ),
        (
            &journal.market_input,
            market_input,
            &journal.market_input_sha256,
            &arguments.expected_market_input_sha256,
        ),
        (
            &journal.campaign_report,
            campaign_report,
            &journal.campaign_report_sha256,
            &arguments.expected_campaign_report_sha256,
        ),
        (
            &journal.buyer_participant,
            buyer_participant,
            &journal.buyer_participant_sha256,
            &arguments.expected_buyer_participant_sha256,
        ),
        (
            &journal.checked_execution_release,
            checked_execution_release,
            &journal.checked_execution_release_sha256,
            &arguments.expected_checked_execution_release_sha256,
        ),
        (
            &journal.seller_ticket,
            seller_ticket,
            &journal.seller_ticket_sha256,
            &arguments.expected_seller_ticket_sha256,
        ),
        (
            &journal.buyer_ticket,
            buyer_ticket,
            &journal.buyer_ticket_sha256,
            &arguments.expected_buyer_ticket_sha256,
        ),
    ];
    if expected
        .iter()
        .any(|(stored_path, path, stored_digest, digest)| {
            stored_path.as_str() != path.to_string_lossy() || stored_digest != digest
        })
        || journal.payer != arguments.payer.to_string()
        || journal.payer_keypair != payer_keypair.display().to_string()
        || journal.public_manifest != public_manifest.display().to_string()
        || journal.private_session != private_session.display().to_string()
        || journal.journal_dir != journal_dir.display().to_string()
        || journal.evidence_file != evidence_file.display().to_string()
    {
        return Err(refusal(
            "Direct producer recovery arguments differ from the prepared immutable inputs or outputs",
        ));
    }
    let public_bytes = decode_canonical_base64_producer_v1(
        &journal.public_manifest_base64,
        "Direct prepared public manifest",
    )?;
    let private_bytes = decode_canonical_base64_producer_v1(
        &journal.private_session_base64,
        "Direct prepared private session",
    )?;
    if sha256_hex(&public_bytes) != journal.public_manifest_sha256
        || sha256_hex(&private_bytes) != journal.private_session_sha256
    {
        return Err(refusal(
            "Direct producer embedded output bytes changed from their digests",
        ));
    }
    let plan_bytes = fs::read(plan)?;
    let market_bytes = fs::read(market_input)?;
    let source =
        authenticate_devnet_direct_session_source_v1(&public_bytes, &plan_bytes, &market_bytes)?;
    let public: ProducedDirectTradePublicManifestV1 = serde_json::from_slice(&public_bytes)?;
    let seller = parse_portable_direct_ticket_v1(&fs::read(seller_ticket)?, "seller")?;
    let buyer = parse_portable_direct_ticket_v1(&fs::read(buyer_ticket)?, "buyer")?;
    if public.seller != signed_manifest_v1(seller)?
        || public.buyer != signed_manifest_v1(buyer)?
        || BASE64
            .decode(&public.checked_execution_release_set_base64)
            .ok()
            .as_deref()
            != Some(fs::read(checked_execution_release)?.as_slice())
        || source.payer != arguments.payer
        || source.checked_execution_release_sha256
            != arguments.expected_checked_execution_release_sha256
    {
        return Err(refusal(
            "Direct producer recovery tickets, checked release, or payer differ from the embedded public manifest",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn recover_devnet_direct_producer_v1(
    prepared_or_finalized: DevnetDirectProducerJournalV1,
    producer_journal: &Path,
    public_manifest: &Path,
    private_session: &Path,
    journal_dir: &Path,
    setup_journal_dir: &Path,
    plan: &Path,
    market_input: &Path,
) -> Result<DevnetDirectProducerJournalV1> {
    let public_bytes = decode_canonical_base64_producer_v1(
        &prepared_or_finalized.public_manifest_base64,
        "Direct prepared public manifest",
    )?;
    let private_bytes = decode_canonical_base64_producer_v1(
        &prepared_or_finalized.private_session_base64,
        "Direct prepared private session",
    )?;
    authenticate_devnet_direct_session_source_v1(
        &public_bytes,
        &fs::read(plan)?,
        &fs::read(market_input)?,
    )?;
    write_or_authenticate_exact_v1(public_manifest, &public_bytes, "Direct public manifest")?;
    write_or_authenticate_exact_v1(private_session, &private_bytes, "Direct private session")?;
    if prepared_or_finalized.phase == DevnetDirectProducerPhaseV1::Prepared {
        create_or_authenticate_empty_directory_v1(journal_dir, "Direct journal directory")?;
        create_or_authenticate_empty_directory_v1(
            setup_journal_dir,
            "Direct setup journal directory",
        )?;
        let mut finalized = prepared_or_finalized.clone();
        finalized.phase = DevnetDirectProducerPhaseV1::Finalized;
        finalized.previous_state_sha256 = Some(prepared_or_finalized.state_sha256.clone());
        finalized.state_sha256 = devnet_direct_producer_state_sha256_v1(&finalized)?;
        replace_json_durable_v1(producer_journal, &prepared_or_finalized, &finalized)?;
        return Ok(finalized);
    }
    if prepared_or_finalized.previous_state_sha256.is_none()
        || !journal_dir.is_dir()
        || !setup_journal_dir.is_dir()
    {
        return Err(refusal(
            "finalized Direct producer omitted its prepared predecessor or executor journal directories",
        ));
    }
    Ok(prepared_or_finalized)
}

fn decode_canonical_base64_producer_v1(value: &str, label: &str) -> Result<Vec<u8>> {
    let bytes = BASE64
        .decode(value)
        .map_err(|error| Error::new(format!("{label} base64: {error}")))?;
    if BASE64.encode(&bytes) != value {
        return Err(refusal(format!("{label} is not canonical base64")));
    }
    Ok(bytes)
}

fn write_or_authenticate_exact_v1(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    if path.exists() {
        if fs::read(path)? != bytes {
            return Err(refusal(format!(
                "existing {label} differs from its prepared exact bytes"
            )));
        }
        return Ok(());
    }
    write_create_new_durable_v1(path, bytes)
}

fn create_or_authenticate_empty_directory_v1(path: &Path, label: &str) -> Result<()> {
    if path.exists() {
        canonical_directory_v1(path, label)?;
        if fs::read_dir(path)?.next().is_some() {
            return Err(refusal(format!(
                "{label} is not empty before producer finalization"
            )));
        }
        return Ok(());
    }
    fs::create_dir(path)?;
    sync_parent_v1(path)
}

fn produce_devnet_direct_session_v1(
    arguments: DevnetDirectSessionProducerArgumentsV1,
) -> Result<DevnetDirectSessionProducerJournalV1> {
    let public_manifest = accepted_regular_v1(
        &arguments.public_manifest,
        &arguments.expected_public_manifest_sha256,
        "Direct public manifest",
    )?;
    let plan = accepted_regular_v1(
        &arguments.plan,
        &arguments.expected_plan_sha256,
        "Direct successor plan",
    )?;
    let market_input = accepted_regular_v1(
        &arguments.market_input,
        &arguments.expected_market_input_sha256,
        "Direct Market input",
    )?;
    let seller_participant = accepted_regular_v1(
        &arguments.seller_participant,
        &arguments.expected_seller_participant_sha256,
        "Direct seller participant report",
    )?;
    let buyer_participant = accepted_regular_v1(
        &arguments.buyer_participant,
        &arguments.expected_buyer_participant_sha256,
        "Direct buyer participant report",
    )?;
    let payer_keypair = canonical_runtime_keypair_path_v1(&arguments.payer_keypair)?;
    let session = canonical_output_path_v1(&arguments.session, "Direct private session")?;
    let producer_journal =
        canonical_output_path_v1(&arguments.producer_journal, "Direct producer journal")?;
    let journal_dir =
        canonical_directory_target_v1(&arguments.journal_dir, "Direct journal directory")?;
    let evidence_file = canonical_output_path_v1(&arguments.evidence_file, "Direct evidence file")?;
    if session == producer_journal
        || session == evidence_file
        || producer_journal == evidence_file
        || journal_dir == session
        || journal_dir == producer_journal
        || journal_dir == evidence_file
    {
        return Err(refusal("Direct session producer output paths alias"));
    }
    let public_bytes = fs::read(&public_manifest)?;
    let plan_bytes = fs::read(&plan)?;
    let market_bytes = fs::read(&market_input)?;
    let source =
        authenticate_devnet_direct_session_source_v1(&public_bytes, &plan_bytes, &market_bytes)?;
    if source.public_manifest_sha256 != arguments.expected_public_manifest_sha256
        || source.plan_sha256 != arguments.expected_plan_sha256
        || source.market_input_sha256 != arguments.expected_market_input_sha256
    {
        return Err(refusal(
            "Direct session producer source digests changed during authentication",
        ));
    }
    let seller_bytes = fs::read(&seller_participant)?;
    let buyer_bytes = fs::read(&buyer_participant)?;
    let seller = user_position_admission::parse_finalized_direct_participant_evidence_offline_v1(
        &seller_bytes,
        ExpectedClusterV1::Devnet,
    )?;
    let buyer = user_position_admission::parse_finalized_direct_participant_evidence_offline_v1(
        &buyer_bytes,
        ExpectedClusterV1::Devnet,
    )?;
    authenticate_devnet_direct_participant_pair_v1(&source, &seller, &buyer)?;
    let payer = source.payer.to_string();
    let participant = |path: &Path,
                       digest: &str,
                       evidence: &user_position_admission::FinalizedDirectParticipantEvidenceV1,
                       replay: Pubkey,
                       nonce: u64| {
        DevnetDirectParticipantSourceV1 {
            report: path.display().to_string(),
            report_sha256: digest.into(),
            owner: evidence.owner.to_string(),
            position: evidence.position.to_string(),
            collateral: evidence.collateral_account.to_string(),
            collateral_quantity_atoms: evidence.collateral_quantity_atoms,
            replay: replay.to_string(),
            nonce,
            admission_signature: evidence.admission_signature.clone(),
            admission_slot: evidence.admission_slot,
            collateral_signature: evidence.collateral_signature.clone(),
            collateral_slot: evidence.collateral_slot,
        }
    };
    let mut prepared = DevnetDirectSessionProducerJournalV1 {
        schema: DEVNET_SESSION_PRODUCER_JOURNAL_SCHEMA_V1.into(),
        phase: DevnetDirectSessionProducerPhaseV1::Prepared,
        cluster: ExpectedClusterV1::Devnet.evidence_label().into(),
        genesis_hash: DEVNET_GENESIS_HASH.into(),
        public_manifest: public_manifest.display().to_string(),
        public_manifest_sha256: source.public_manifest_sha256.clone(),
        plan: plan.display().to_string(),
        plan_sha256: source.plan_sha256.clone(),
        market_input: market_input.display().to_string(),
        market_input_sha256: source.market_input_sha256.clone(),
        checked_execution_release_sha256: source.checked_execution_release_sha256.clone(),
        checked_binaries: source.checked_binaries.clone(),
        payer,
        payer_keypair: payer_keypair.display().to_string(),
        seller: participant(
            &seller_participant,
            &arguments.expected_seller_participant_sha256,
            &seller,
            source.seller_replay,
            source.seller_nonce,
        ),
        buyer: participant(
            &buyer_participant,
            &arguments.expected_buyer_participant_sha256,
            &buyer,
            source.buyer_replay,
            source.buyer_nonce,
        ),
        seller_ticket_sha256: source.seller_ticket_sha256,
        buyer_ticket_sha256: source.buyer_ticket_sha256,
        journal_dir: journal_dir.display().to_string(),
        evidence_file: evidence_file.display().to_string(),
        private_session: session.display().to_string(),
        private_session_sha256: None,
        previous_state_sha256: None,
        state_sha256: String::new(),
    };
    prepared.state_sha256 = devnet_session_producer_state_sha256_v1(&prepared)?;
    let existing = load_devnet_session_producer_journal_v1(&producer_journal)?;
    match existing {
        None => {
            write_create_new_durable_v1(&producer_journal, &pretty_json_bytes_v1(&prepared)?)?;
        }
        Some(existing) => {
            authenticate_devnet_session_producer_recovery_v1(&prepared, &existing)?;
            if existing.phase == DevnetDirectSessionProducerPhaseV1::Finalized {
                authenticate_devnet_private_session_output_v1(&session, &existing)?;
                return Ok(existing);
            }
        }
    }
    create_or_authenticate_session_journal_directory_v1(&journal_dir)?;
    let mut private = ProducedDirectTradePrivateSessionV1 {
        schema: DEVNET_PRIVATE_SESSION_SCHEMA_V1.into(),
        public_manifest: public_manifest.display().to_string(),
        public_manifest_sha256: source.public_manifest_sha256,
        plan: plan.display().to_string(),
        market_input: market_input.display().to_string(),
        payer_keypair: payer_keypair.display().to_string(),
        journal_dir: journal_dir.display().to_string(),
        evidence_file: evidence_file.display().to_string(),
        session_sha256: String::new(),
    };
    private.session_sha256 = private_session_sha256_v1(&private)?;
    let private_bytes = pretty_json_bytes_v1(&private)?;
    let private_file_sha256 = sha256_hex(&private_bytes);
    if session.exists() {
        let bytes = fs::read(&session)?;
        if bytes != private_bytes {
            return Err(refusal(
                "existing Direct private session differs from the prepared write-ahead intent",
            ));
        }
    } else {
        write_create_new_durable_v1(&session, &private_bytes)?;
    }
    let mut finalized = prepared.clone();
    finalized.phase = DevnetDirectSessionProducerPhaseV1::Finalized;
    finalized.private_session_sha256 = Some(private_file_sha256);
    finalized.previous_state_sha256 = Some(prepared.state_sha256.clone());
    finalized.state_sha256 = devnet_session_producer_state_sha256_v1(&finalized)?;
    replace_json_durable_v1(&producer_journal, &prepared, &finalized)?;
    Ok(finalized)
}

struct ValidatedPathsV1 {
    arguments: OwnedLoopbackDirectProducerArgumentsV1,
    payer_keypair: PathBuf,
    seller_keypair: PathBuf,
    buyer_keypair: PathBuf,
}

fn validate_paths_v1(
    mut arguments: OwnedLoopbackDirectProducerArgumentsV1,
) -> Result<ValidatedPathsV1> {
    arguments.plan = canonical_regular_v1(&arguments.plan, "Direct successor plan")?;
    arguments.market_input = canonical_regular_v1(&arguments.market_input, "Direct Market input")?;
    arguments.campaign_report =
        canonical_regular_v1(&arguments.campaign_report, "Direct campaign report")?;
    arguments.participant_report =
        canonical_regular_v1(&arguments.participant_report, "Direct participant report")?;
    arguments.key_dir = canonical_directory_v1(&arguments.key_dir, "Direct key directory")?;
    arguments.output_dir =
        canonical_directory_v1(&arguments.output_dir, "Direct output directory")?;
    let payer_keypair = canonical_regular_v1(
        &arguments.key_dir.join("core-upgrade-authority.json"),
        "Direct payer keypair",
    )?;
    let seller_keypair = canonical_regular_v1(
        &arguments.key_dir.join("founding-founder.json"),
        "Direct seller keypair",
    )?;
    let buyer_keypair = canonical_regular_v1(
        &arguments.key_dir.join("participant.json"),
        "Direct buyer keypair",
    )?;
    for target in [
        "direct-trade-public.json",
        "direct-trade-session.json",
        "direct-trade-finalized.json",
        "direct-trade-produced.json",
        "direct-trade-journal",
    ] {
        if arguments.output_dir.join(target).try_exists()? {
            return Err(refusal(format!(
                "Direct producer target already exists: {}",
                arguments.output_dir.join(target).display()
            )));
        }
    }
    Ok(ValidatedPathsV1 {
        arguments,
        payer_keypair,
        seller_keypair,
        buyer_keypair,
    })
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn prepare_public_facts_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    participant: user_position_admission::FinalizedDirectParticipantEvidenceV1,
    plan_sha256: String,
    market_sha256: String,
    campaign_sha256: String,
    participant_sha256: String,
    payer: Pubkey,
    checked_release: Vec<u8>,
    requested_terms: Option<DirectTradeTermsV1>,
    expected_cluster: ExpectedClusterV1,
) -> Result<PreparedPublicFactsV1> {
    crate::market::validate_market_input(market_input)?;
    if campaign.plan_sha256 != plan_sha256 || campaign.market_sha256 != market_sha256 {
        return Err(refusal(
            "Direct campaign and checked plan/Market input digests differ",
        ));
    }
    let direct = market_input
        .direct_capability
        .as_ref()
        .ok_or_else(|| refusal("Direct Market input omitted its capability closure"))?;
    if direct.selected_manifest_entry_index != campaign.direct_selected_manifest_entry_index {
        return Err(refusal(
            "Direct selected manifest entry changed after founding",
        ));
    }
    let market = campaign_address_v1(campaign, "founding_market")?;
    if participant.market != market {
        return Err(refusal(
            "Direct participant evidence belongs to another Market",
        ));
    }
    let aggregate = campaign_address_v1(campaign, "claims_aggregate")?;
    if participant.claims_market != aggregate {
        return Err(refusal(
            "Direct participant evidence belongs to another Claims aggregate",
        ));
    }
    let seller_position = campaign_address_v1(campaign, "founder_position")?;
    // The EXECUTION capability root. This is DERIVED, never read from the
    // campaign: the founding's recorded `direct_capability_root` is the
    // founding-permit namespace address (its selection config is the
    // generic-founding preimage digest, decision 0004), where no account can
    // ever exist - both the activation and hot paths force
    // `selection.config == entry.config_id`. The execution root's header is
    // rebuilt here from the sealed manifest body and the live Open Market's
    // own identity, exactly as `authenticate_header` on chain re-derives it.
    let root = derived_direct_execution_root_v1(
        &mut *rpc,
        plan,
        market_input,
        campaign,
        market,
        campaign.direct_selected_manifest_entry_index,
    )?;
    let mint = campaign_address_v1(campaign, "collateral_mint")?;
    if participant.mint != mint {
        return Err(refusal("Direct participant collateral Mint changed"));
    }
    let token_program = participant.token_program;
    let expected_token_program = pubkey(&campaign_account_v1(campaign, "collateral_mint")?.owner)?;
    if token_program != expected_token_program {
        return Err(refusal("Direct participant token program changed"));
    }
    let lifecycle_rent_credit = campaign_address_v1(campaign, "founding_lifecycle_rent_credit")?;
    if payer == Pubkey::default() {
        return Err(refusal("Direct payer is the default public key"));
    }
    let registry = pubkey(&plan.registry.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let release_set = hex32(&plan.release_set_id)?;

    let config_bytes = decode_hex_v1(&direct.execution_config_hex, "Direct execution config")?;
    require_market_body_matches_record_v1(
        plan,
        market_input,
        campaign,
        "direct_execution_config_record",
        &config_bytes,
    )?;
    for (label, body_hex) in [
        (
            "direct_ordinary_account_profile_record",
            direct.ordinary_account_profile_hex.as_str(),
        ),
        (
            "direct_ordinary_lifecycle_policy_record",
            direct.ordinary_lifecycle_policy_hex.as_str(),
        ),
        (
            "direct_ordinary_request_profile_record",
            direct.ordinary_request_profile_hex.as_str(),
        ),
        (
            "direct_ordinary_transition_record",
            direct.ordinary_transition_hex.as_str(),
        ),
        (
            "direct_ordinary_strategy_record",
            direct.ordinary_strategy_hex.as_str(),
        ),
        (
            "direct_ordinary_effect_record",
            direct.ordinary_effect_hex.as_str(),
        ),
        (
            "direct_ordinary_descriptor_record",
            direct.ordinary_descriptor_hex.as_str(),
        ),
        ("direct_program_set_record", direct.program_set_hex.as_str()),
        (
            "capability_manifest_record",
            market_input.capability_manifest_hex.as_str(),
        ),
        (
            "linked_liability_basis_record",
            market_input.linked_basis_hex.as_str(),
        ),
    ] {
        require_market_body_matches_record_v1(
            plan,
            market_input,
            campaign,
            label,
            &decode_hex_v1(body_hex, label)?,
        )?;
    }
    let config_digest = hash(&config_bytes).to_bytes();
    let config =
        DirectExecutionConfigV1::decode_selected(config_digest, config_digest, &config_bytes)
            .map_err(|error| Error::new(format!("Direct execution config: {error:?}")))?;
    if config.price_scale() == 0 {
        return Err(refusal("Direct selected config has a zero price scale"));
    }
    let fee_recipient = Pubkey::new_from_array(config.fee_recipient());

    let snapshot = finalized_snapshot(
        rpc,
        &[
            market,
            root,
            aggregate,
            seller_position,
            participant.position,
            // The buyer's collateral account joins the snapshot because the
            // allowance the chain requires is a fact of THIS account at THIS
            // commitment, not of the admission report that once described it.
            participant.collateral_account,
            mint,
            lifecycle_rent_credit,
            sysvar::rent::ID,
        ],
    )?;
    let market_account = snapshot.account(market)?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Direct Market state: {error:?}")))?;
    // The Market account address is the PDA of its own identity, so a decoded
    // identity generation is chain-authenticated. It must still be the one
    // this exact input names: either the input's own generation (a compiled
    // input carrying the live Open generation) or the founding lane's Open
    // generation for a founding-shaped input (`derive_founding_targets`
    // places the DCLTGMF3 Open Market at that offset).
    let generation = market_state.identity.generation;
    if generation != market_input.generation
        && generation != crate::market::open_market_generation_v1(market_input)?
    {
        return Err(refusal(format!(
            "Direct Open Market generation {generation} is neither the input generation {} nor the founding lane's Open generation",
            market_input.generation,
        )));
    }
    if market_account.owner != pubkey(&plan.core.program_id)?
        || market_account.executable
        || market_state.phase != CorePhase::Open
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
        || market_state.identity.selected_release_set.to_bytes() != release_set
    {
        return Err(refusal(format!(
            "Direct producer requires the exact finalized Open founding Market (owner {} vs core {}, executable {}, phase {:?}, market id {} vs {}, release set {} vs {})",
            market_account.owner,
            plan.core.program_id,
            market_account.executable,
            market_state.phase,
            Pubkey::new_from_array(market_state.identity.market_id.to_bytes()),
            market,
            hex_encode_v1(&market_state.identity.selected_release_set.to_bytes()),
            hex_encode_v1(&release_set),
        )));
    }
    let market_rent_beneficiary = Pubkey::new_from_array(market_state.rent_beneficiary.to_bytes());
    let root_account = snapshot.account(root)?;
    authenticate_direct_execution_root_shape_v1(root, root_account, trading)?;
    let root_header = CapabilityRootHeaderV1::decode(
        root_account
            .data
            .get(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .ok_or_else(|| refusal("Direct root header disappeared"))?,
    )
    .map_err(|error| Error::new(format!("Direct root header: {error:?}")))?;
    let root_state = DirectRootStateV1::decode(
        root_account
            .data
            .get(CAPABILITY_ROOT_HEADER_BYTES_V1..)
            .ok_or_else(|| refusal("Direct root tail disappeared"))?,
    )
    .map_err(|error| Error::new(format!("Direct root tail: {error:?}")))?;
    if root_header.market() != market.to_bytes()
        || root_header.generation() != generation
        || root_header.release_set().to_bytes() != release_set
        || root_state.phase() != DirectRootPhaseV1::Open
    {
        return Err(refusal(
            "Direct root immutable coordinates or phase changed",
        ));
    }
    let aggregate_account = snapshot.account(aggregate)?;
    let aggregate_view = LiabilityBasisMarketViewV2::decode(&aggregate_account.data)
        .map_err(|error| Error::new(format!("Direct Claims aggregate: {error:?}")))?;
    let seller_position_account = snapshot.account(seller_position)?;
    let seller_position_view = LiabilityBasisPositionViewV2::decode(&seller_position_account.data)
        .map_err(|error| Error::new(format!("Direct seller Position: {error:?}")))?;
    let buyer_position_account = snapshot.account(participant.position)?;
    let buyer_position_view = LiabilityBasisPositionViewV2::decode(&buyer_position_account.data)
        .map_err(|error| Error::new(format!("Direct buyer Position: {error:?}")))?;
    if aggregate_account.owner != claims
        || seller_position_account.owner != claims
        || buyer_position_account.owner != claims
        || aggregate_view.logical_market != market.to_bytes()
        || aggregate_view.release_set != release_set
        || aggregate_view.generation != generation
        || seller_position_view.market_account != aggregate.to_bytes()
        || buyer_position_view.market_account != aggregate.to_bytes()
        || buyer_position_view.owner != participant.owner.to_bytes()
        || seller_position_view.basis_id != aggregate_view.basis_id
        || buyer_position_view.basis_id != aggregate_view.basis_id
        || seller_position_view.claim_count != aggregate_view.claim_count
        || buyer_position_view.claim_count != aggregate_view.claim_count
    {
        return Err(refusal(
            "Direct Claims Market/Position/basis/owner closure changed",
        ));
    }
    let expected_seller_position = Pubkey::find_program_address(
        &ProtocolPositionSeedsV2::new(aggregate.to_bytes(), seller_position_view.owner)
            .map_err(|error| Error::new(format!("seller Position seeds: {error:?}")))?
            .as_slices(),
        &claims,
    )
    .0;
    if seller_position != expected_seller_position
        || participant.position != expected_buyer_position_v1(aggregate, participant.owner, claims)?
    {
        return Err(refusal("Direct Position PDA coordinate changed"));
    }
    let seller = Pubkey::new_from_array(seller_position_view.owner);
    let outcome = match requested_terms {
        Some(terms) => {
            if terms.fill == 0
                || terms.execution_price == 0
                || terms.fee_basis_points != config.fee_basis_points()
                || terms.outcome >= aggregate_view.claim_count
                || seller_position_view
                    .balance(&seller_position_account.data, terms.outcome)
                    .map_err(|error| {
                        Error::new(format!("seller claim balance {}: {error:?}", terms.outcome))
                    })?
                    < terms.fill
            {
                return Err(refusal(
                    "caller-owned Direct terms exceed the finalized seller claim balance or selected config",
                ));
            }
            terms.outcome
        }
        None => first_funded_outcome_v1(
            seller_position_view,
            &seller_position_account.data,
            FILL_ATOMS_V1,
        )?,
    };
    let terms = requested_terms.unwrap_or(DirectTradeTermsV1 {
        outcome,
        fill: FILL_ATOMS_V1,
        execution_price: EXECUTION_PRICE_V1,
        fee_basis_points: FEE_BASIS_POINTS_V1,
    });
    let gross = exact_quote_v1(terms.fill, terms.execution_price, config.price_scale())?;
    let fee = fee_floor_v1(gross, terms.fee_basis_points)?;
    let required_buyer_collateral = gross
        .checked_add(fee)
        .ok_or_else(|| refusal("Direct buyer reserve overflowed"))?;
    let buyer_collateral_account = snapshot.account(participant.collateral_account)?;
    let refusing = refusing_buyer_collateral_clauses_v1(
        &buyer_collateral_account.data,
        participant.collateral_quantity_atoms,
        participant.custody_authority,
        required_buyer_collateral,
    )?;
    if !refusing.is_empty() {
        return Err(refusal(format!(
            "the finalized buyer collateral account {} cannot fund this exact trade: {}",
            participant.collateral_account,
            refusing.join("; ")
        )));
    }
    let rent_account = snapshot.account(sysvar::rent::ID)?;
    let rent: Rent = bincode::deserialize(&rent_account.data)
        .map_err(|error| Error::new(format!("Direct Rent sysvar: {error}")))?;
    let coordinates = DirectCoordinatesV1::new(market.to_bytes(), generation)
        .map_err(|error| Error::new(format!("Direct coordinates: {error:?}")))?;
    let seller_seeds = MakerReplaySeedsV1::new(coordinates, seller.to_bytes())
        .map_err(|error| Error::new(format!("seller maker seeds: {error:?}")))?;
    let buyer_seeds = MakerReplaySeedsV1::new(coordinates, participant.owner.to_bytes())
        .map_err(|error| Error::new(format!("buyer maker seeds: {error:?}")))?;
    let (seller_maker, seller_bump) =
        Pubkey::find_program_address(&seller_seeds.as_slices(), &trading);
    let (buyer_maker, buyer_bump) =
        Pubkey::find_program_address(&buyer_seeds.as_slices(), &trading);
    // Read from the RentCredit the chain itself reads, not from who is paying.
    let maker_rent_beneficiary = maker_root_rent_beneficiary_v1(rpc, lifecycle_rent_credit)?;
    let seller_facts = maker_facts_v1(
        rpc,
        seller_maker,
        seller_bump,
        trading,
        market,
        generation,
        seller,
        maker_rent_beneficiary,
        &rent,
    )?;
    let buyer_facts = maker_facts_v1(
        rpc,
        buyer_maker,
        buyer_bump,
        trading,
        market,
        generation,
        participant.owner,
        maker_rent_beneficiary,
        &rent,
    )?;
    let custody_replay = Pubkey::find_program_address(
        &CustodyReplaySeedsV1::new(
            market.to_bytes(),
            release_set,
            CallerRoleV1::Trading,
            buyer_maker.to_bytes(),
        )
        .as_slices(),
        &custody,
    )
    .0;
    authenticate_or_admit_pending_replay_v1(
        rpc,
        custody_replay,
        custody,
        market,
        release_set,
        aggregate_view.realm_id,
        buyer_maker,
        trading,
        generation,
        market_rent_beneficiary,
    )?;
    let custody_authority = Pubkey::find_program_address(
        &CustodyAuthoritySeedsV1::new(market.to_bytes(), release_set).as_slices(),
        &custody,
    )
    .0;
    if participant.custody_authority != custody_authority {
        return Err(refusal("Direct participant Custody authority changed"));
    }
    let seller_token_seeds = DirectTokenAccountSeedsV1::new(
        market.to_bytes(),
        generation,
        seller.to_bytes(),
        DirectTokenAccountRoleV1::Seller,
    )
    .map_err(|error| Error::new(format!("seller Direct token seeds: {error:?}")))?;
    let fee_token_seeds = DirectTokenAccountSeedsV1::new(
        market.to_bytes(),
        generation,
        fee_recipient.to_bytes(),
        DirectTokenAccountRoleV1::Fee,
    )
    .map_err(|error| Error::new(format!("fee Direct token seeds: {error:?}")))?;
    let seller_token = Pubkey::find_program_address(&seller_token_seeds.as_slices(), &trading).0;
    let fee_token = Pubkey::find_program_address(&fee_token_seeds.as_slices(), &trading).0;
    // The payer may be the buyer, and only the buyer.
    //
    // What the distinctness is FOR, at its two on-chain sites, is the
    // transaction-level privilege-and-lamport merge: a key repeated across two
    // coordinates arrives as ONE account whose writability is the union, and
    // both setup instructions close over exact lamport deltas that a merged
    // account would silently satisfy from the wrong side.
    // `direct_replay_setup_v1` states its own hazard set exactly -- payer vs
    // rent refund, and the created maker root vs payer/refund/Market -- and
    // then asserts `payer_before - top_up == payer.lamports()` against
    // `refund_before + refunded_excess == rent_refund.lamports()`.
    // `direct_token_setup_v1` asserts the same shape over PAYER and
    // RENT_REFUND plus a digest over every account it declares immutable.
    //
    // Neither instruction carries the BUYER at any account index: replay setup
    // takes the maker as instruction DATA and derives the root PDA, and token
    // setup is about the seller and the venue fee. In the trade itself both
    // makers authorize with a detached Ed25519 intent signature over the
    // 172-byte compact preimage, never with a transaction signature -- the
    // compiled route names exactly one signer, the payer -- so a buyer who
    // pays holds no privilege the trade did not already give the payer.
    //
    // A stranger paying their own fees is the product, not a hazard: the
    // browser panel compiles only when the route's payer IS the connected
    // wallet, so refusing this pair refused every wallet-paid trade.
    require_distinct_v1(
        &[
            ("seller", seller),
            ("buyer", participant.owner),
            ("payer", payer),
            ("fee recipient", fee_recipient),
            ("seller Direct token account", seller_token),
            ("fee Direct token account", fee_token),
            ("buyer collateral account", participant.collateral_account),
            ("seller maker replay root", seller_maker),
            ("buyer maker replay root", buyer_maker),
            ("Custody replay", custody_replay),
            ("Custody authority", custody_authority),
            ("Market rent beneficiary", market_rent_beneficiary),
        ],
        &[("payer", "buyer")],
    )?;
    // The two destinations are classified independently and then required to
    // AGREE, because a market with one vacant and one initialized destination is
    // a half-run token setup: setup refuses (one is not vacant) and the trade
    // refuses (one is not initialized), so there is no instruction that can move
    // it forward, and saying so here is the only place an operator learns it.
    let mut destination_prestates = Vec::new();
    for (address, label, token_owner) in [
        (seller_token, "seller Token-2022 destination", seller),
        (fee_token, "fee Token-2022 destination", fee_recipient),
    ] {
        let observed = rpc.account(address)?;
        let (prestate, refusing) = classify_direct_token_destination_v1(
            observed.as_ref(),
            mint,
            token_owner,
            token_program,
        );
        match prestate {
            Some(prestate) => destination_prestates.push((label, prestate)),
            None => {
                return Err(refusal(format!(
                    "{label} {address} is neither of the two prestates the chain admits -- not the System-owned data-empty account `direct_token_setup_v1` creates, and not the initialized Token-2022 account the trade pays: {}",
                    refusing.join("; ")
                )));
            }
        }
    }
    let token_setup_prestate = match destination_prestates.as_slice() {
        [(_, seller_prestate), (_, fee_prestate)] if seller_prestate == fee_prestate => {
            *seller_prestate
        }
        [(seller_label, seller_prestate), (fee_label, fee_prestate)] => {
            return Err(refusal(format!(
                "Direct token setup is half run on this Market: the {seller_label} {seller_token} is {seller_prestate:?} while the {fee_label} {fee_token} is {fee_prestate:?}. Token setup creates both in one instruction and refuses unless BOTH are vacant; the trade pays both and refuses unless BOTH are initialized. No instruction can move this Market forward"
            )));
        }
        _ => return Err(refusal("Direct token destinations were not classified")),
    };
    // WHAT REMAINS OF WALL 7, NAMED RATHER THAN DISCOVERED ON CHAIN.
    //
    // The producer now admits both prestates the chain admits, which is the
    // half of this that was wrong: it used to call an initialized destination
    // malformed. But admitting it is only useful if the SETUP STAGE MACHINE
    // then skips a token setup whose accounts already exist, and it does not
    // yet -- `execute_direct_setup_action_v1` selects TokenSetup purely on
    // journal phase, and a fresh session has no journal, so it would send the
    // instruction and `direct_token_setup_v1` would refuse it for not finding
    // the vacancy it needs to create anything.
    //
    // So this refuses HERE, before signing two intents and writing a session
    // that could never advance, and it says which stage owns the gap. That is
    // strictly better than producing the session and reading `Content` off the
    // chain twenty minutes later, and it is honest that the market still cannot
    // trade twice.
    if token_setup_prestate == DirectTokenDestinationPrestateV1::Initialized {
        return Err(refusal(format!(
            "Direct token setup has already run on Market {market} generation {generation}: the seller destination {seller_token} and fee destination {fee_token} are both the initialized Token-2022 accounts this trade would pay. The producer admits that prestate; the setup stage machine cannot yet SKIP a finished token setup, so this session would stall at the token-setup stage. That skip is the remaining half of wall 7"
        )));
    }

    let descriptor = record_coordinates_v1(
        plan,
        market_input,
        campaign,
        "direct_ordinary_descriptor_record",
    )?;
    let descriptor_pair = resolved_record_v1(
        plan,
        market_input,
        campaign,
        "direct_ordinary_descriptor_record",
    )?;
    let descriptor_digest = hex32(&descriptor_pair.content_sha256)?;
    let trading_semantic_release = hex32(&plan.trading.semantic_release_id)?;
    let capability_seal = derive_capability_seal_v1(
        descriptor_digest,
        trading_semantic_release,
        registry,
        trading,
    );
    let route_without_children = ProducedDirectRouteCoordinatesV1 {
        fixed: ProducedDirectFixedCoordinatesV1 {
            market: market.to_string(),
            root: root.to_string(),
            manifest: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "capability_manifest_record",
            )?,
            program_set: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "direct_program_set_record",
            )?,
            descriptor,
            config: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "direct_execution_config_record",
            )?,
            account_profile: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "direct_ordinary_account_profile_record",
            )?,
            request_profile: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "direct_ordinary_request_profile_record",
            )?,
            transition: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "direct_ordinary_transition_record",
            )?,
            effect: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "direct_ordinary_effect_record",
            )?,
            lifecycle: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "direct_ordinary_lifecycle_policy_record",
            )?,
            strategy: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "direct_ordinary_strategy_record",
            )?,
            activation_cache: plan.activation.clone(),
            core_program: plan.core.program_id.clone(),
            core_programdata: plan.core.programdata_id.clone(),
            trading_program: plan.trading.program_id.clone(),
            trading_programdata: plan.trading.programdata_id.clone(),
            registry_program: plan.registry.program_id.clone(),
            product: record_coordinates_v1(plan, market_input, campaign, "product_record")?,
            result_domain: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "result_domain_record",
            )?,
            portfolio: record_coordinates_v1(plan, market_input, campaign, "portfolio_record")?,
            linked_basis: record_coordinates_v1(
                plan,
                market_input,
                campaign,
                "linked_liability_basis_record",
            )?,
            capability_seal: capability_seal.to_string(),
        },
        seller_maker: seller_maker.to_string(),
        payer: payer.to_string(),
        lifecycle_rent_credit: lifecycle_rent_credit.to_string(),
        buyer_maker: buyer_maker.to_string(),
        rent_program: plan.rent_credit.program_id.clone(),
        claims: ProducedDirectClaimsCoordinatesV1 {
            caller_authority: Pubkey::default().to_string(),
            aggregate: aggregate.to_string(),
            claims_program: plan.claims.program_id.clone(),
            claims_programdata: plan.claims.programdata_id.clone(),
            seller_position: seller_position.to_string(),
            buyer_position: participant.position.to_string(),
        },
        custody: ProducedDirectCustodyCoordinatesV1 {
            caller_authorities: core::array::from_fn(|_| Pubkey::default().to_string()),
            realm: record_coordinates_v1(plan, market_input, campaign, "realm_record")?,
            replay: custody_replay.to_string(),
            mint: mint.to_string(),
            buyer_token: participant.collateral_account.to_string(),
            seller_token: seller_token.to_string(),
            fee_token: fee_token.to_string(),
            custody_authority: custody_authority.to_string(),
            token_program: token_program.to_string(),
            custody_program: plan.custody.program_id.clone(),
            custody_programdata: plan.custody.programdata_id.clone(),
        },
    };
    let replay_request = DirectReplaySetupRequestV1 {
        market: market.to_bytes(),
        maker: participant.owner.to_bytes(),
        expected_market_digest: hash(&market_account.data).to_bytes(),
        generation,
    };
    let replay_request_bytes = replay_request
        .to_bytes()
        .map_err(|error| Error::new(format!("Direct replay setup request: {error:?}")))?;
    let replay_setup = ProducedReplaySetupV1 {
        request_base64: BASE64.encode(replay_request_bytes),
        request_sha256: sha256_hex(&replay_request_bytes),
        maker: participant.owner.to_string(),
        maker_root: buyer_maker.to_string(),
        custody_replay: custody_replay.to_string(),
        payer: payer.to_string(),
        rent_refund: market_rent_beneficiary.to_string(),
        expected_initial_revision: 0,
        expected_resulting_revision: 1,
    };
    let token_setup_request = DirectTokenSetupRequestV1 {
        market: market.to_bytes(),
        expected_market_digest: hash(&market_account.data).to_bytes(),
        expected_root_digest: hash(&root_account.data).to_bytes(),
        expected_claims_aggregate_digest: hash(&aggregate_account.data).to_bytes(),
        seller_owner: seller.to_bytes(),
        expected_seller_position_digest: hash(&seller_position_account.data).to_bytes(),
        generation,
    };
    let token_setup_request_bytes = token_setup_request
        .to_bytes()
        .map_err(|error| Error::new(format!("Direct token setup request: {error:?}")))?;
    let token_setup = ProducedTokenSetupV1 {
        request_base64: BASE64.encode(token_setup_request_bytes),
        request_sha256: sha256_hex(&token_setup_request_bytes),
        seller_token: seller_token.to_string(),
        seller_owner: seller.to_string(),
        fee_token: fee_token.to_string(),
        fee_recipient: fee_recipient.to_string(),
        payer: payer.to_string(),
        rent_refund: market_rent_beneficiary.to_string(),
        mint: mint.to_string(),
        token_program: token_program.to_string(),
        trading_program: trading.to_string(),
    };
    let account_profile_bytes = record_body_v1(
        plan,
        market_input,
        campaign,
        "direct_ordinary_account_profile_record",
    )?;
    let transition_bytes = record_body_v1(
        plan,
        market_input,
        campaign,
        "direct_ordinary_transition_record",
    )?;
    let effect_bytes = record_body_v1(
        plan,
        market_input,
        campaign,
        "direct_ordinary_effect_record",
    )?;
    let product_digest = record_digest_v1(plan, market_input, campaign, "product_record")?;
    let realm_digest = record_digest_v1(plan, market_input, campaign, "realm_record")?;
    let linked_basis_digest = record_digest_v1(
        plan,
        market_input,
        campaign,
        "linked_liability_basis_record",
    )?;
    let linked_basis_semantic = crate::market::semantic_basis_identity_v3(&decode_hex_v1(
        &market_input.linked_basis_hex,
        "linked liability basis",
    )?)?;
    // The aggregate stores the SEMANTIC product identity (the market input's
    // product_id, which the product record embeds); product_digest is the
    // record-content digest and belongs to the hot context, where the chain
    // itself compares it against the live record bytes.
    if hex32(&market_input.product_id)? != aggregate_view.product_instance_id
        || realm_digest != aggregate_view.realm_id
        || linked_basis_semantic != hex32(&market_input.liability_basis_id)?
        || aggregate_view.basis_id != linked_basis_semantic
    {
        return Err(refusal(format!(
            "Direct Product/Realm/semantic-basis/linked-basis closure changed (product {} vs aggregate {}, realm {} vs {}, semantic {} vs input {}, aggregate basis {})",
            market_input.product_id,
            hex_encode_v1(&aggregate_view.product_instance_id),
            hex_encode_v1(&realm_digest),
            hex_encode_v1(&aggregate_view.realm_id),
            hex_encode_v1(&linked_basis_semantic),
            market_input.liability_basis_id,
            hex_encode_v1(&aggregate_view.basis_id),
        )));
    }
    let genesis_hash = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| refusal("Direct genesis hash was not a string"))?
        .to_owned();
    if expected_cluster == ExpectedClusterV1::Devnet && genesis_hash != DEVNET_GENESIS_HASH {
        return Err(refusal(
            "Direct devnet producer observed another genesis hash",
        ));
    }
    Ok(PreparedPublicFactsV1 {
        plan_sha256,
        market_sha256,
        campaign_sha256,
        participant_sha256,
        genesis_hash,
        market,
        generation,
        payer,
        fee_recipient,
        seller,
        buyer: participant.owner,
        mint,
        token_program,
        seller_token,
        fee_token,
        seller_maker,
        buyer_maker,
        custody_authority,
        seller_facts,
        buyer_facts,
        aggregate_view,
        seller_position_view,
        buyer_position_view,
        root_state,
        observation_slot: snapshot.observation.slot,
        outcome,
        config,
        config_bytes,
        account_profile_bytes,
        transition_bytes,
        effect_bytes,
        product_digest,
        realm_digest,
        linked_basis_digest,
        checked_release,
        route_without_children,
        replay_setup,
        token_setup,

        participant,
    })
}

#[allow(clippy::too_many_arguments)]
/// The refund wallet the chain will write into a maker root it creates.
///
/// THE LIFECYCLE LANE MOVED THIS AND THE PRODUCER DID NOT FOLLOW. `maker_facts_v1`
/// used to answer `payer` here, under a comment that said so and named itself as
/// "the single producer coordinate to change" if maker-root rent ever moved to
/// RentCredit. It moved. `hot_v3.rs` builds `MakerReplayFirstUseV1` with
/// `rent_owner: plan.beneficiary`, and that beneficiary is
/// `credit.refund_wallet()` of the founding lifecycle RentCredit -- not the
/// wallet that happened to pay for one trade.
///
/// The distinction is the point rather than a detail. A maker replay root is a
/// shared structure of the MARKET; if its rent followed whoever paid, a stranger
/// paying their own fees would walk away owning the rent of something the market
/// depends on. The same route deliberately admits that stranger as the payer.
///
/// Measured 2026-08-31 on a landed fill: the producer projected the payer, the
/// chain wrote the refund wallet, and the driver refused its own fill's
/// poststate on those 32 bytes with every economic cell already agreeing.
fn maker_root_rent_beneficiary_v1(rpc: &mut Rpc, lifecycle_rent_credit: Pubkey) -> Result<Pubkey> {
    let account = rpc
        .account(lifecycle_rent_credit)?
        .ok_or_else(|| refusal("Direct founding lifecycle RentCredit is absent"))?;
    let credit = LifecycleRentCreditV2::decode(&account.data)
        .map_err(|error| Error::new(format!("Direct lifecycle RentCredit: {error:?}")))?;
    Ok(Pubkey::new_from_array(credit.refund_wallet().to_bytes()))
}

fn maker_facts_v1(
    rpc: &mut Rpc,
    address: Pubkey,
    bump: u8,
    trading: Pubkey,
    market: Pubkey,
    generation: u64,
    maker: Pubkey,
    rent_beneficiary: Pubkey,
    rent: &Rent,
) -> Result<MakerFactsV1> {
    let Some(account) = rpc.account(address)? else {
        return Ok(MakerFactsV1 {
            created: true,
            bump_observation: 0,
            bump,
            next_nonce: 0,
            rent_principal_observation: 0,
            rent_principal: rent.minimum_balance(DIRECT_MAKER_REPLAY_BYTES_V1),
            rent_beneficiary_observation: Pubkey::default(),
            rent_beneficiary,
        });
    };
    if account.owner == system_program::ID && account.data.is_empty() {
        return Ok(MakerFactsV1 {
            created: true,
            bump_observation: 0,
            bump,
            next_nonce: 0,
            rent_principal_observation: 0,
            rent_principal: rent.minimum_balance(DIRECT_MAKER_REPLAY_BYTES_V1),
            rent_beneficiary_observation: Pubkey::default(),
            rent_beneficiary,
        });
    }
    if account.owner != trading || account.executable {
        return Err(refusal(
            "Direct maker replay owner or executable bit changed",
        ));
    }
    let replay = MakerReplayRootV1::decode(&account.data)
        .map_err(|error| Error::new(format!("Direct maker replay: {error:?}")))?;
    if replay.market() != market.to_bytes()
        || replay.generation() != generation
        || replay.maker() != maker.to_bytes()
        || replay.bump() != bump
        || account.lamports < replay.rent_principal()
    {
        return Err(refusal("Direct maker replay coordinates changed"));
    }
    Ok(MakerFactsV1 {
        created: false,
        bump_observation: replay.bump(),
        bump: replay.bump(),
        next_nonce: replay.next_nonce(),
        rent_principal_observation: replay.rent_principal(),
        rent_principal: replay.rent_principal(),
        rent_beneficiary_observation: Pubkey::new_from_array(replay.rent_owner()),
        rent_beneficiary: Pubkey::new_from_array(replay.rent_owner()),
    })
}

#[allow(clippy::too_many_arguments)]
fn authenticate_or_admit_pending_replay_v1(
    rpc: &mut Rpc,
    address: Pubkey,
    custody: Pubkey,
    market: Pubkey,
    release_set: [u8; 32],
    realm: [u8; 32],
    context: Pubkey,
    trading: Pubkey,
    generation: u64,
    rent_refund: Pubkey,
) -> Result<()> {
    let Some(account) = rpc.account(address)? else {
        return Ok(());
    };
    if account.owner == system_program::ID && account.data.is_empty() {
        return Ok(());
    }
    if account.owner != custody || account.executable {
        return Err(refusal(
            "Direct Custody replay owner or executable bit changed",
        ));
    }
    let replay = CustodyReplayV1::decode(&account.data)
        .map_err(|error| Error::new(format!("Direct Custody replay: {error:?}")))?;
    if replay.caller_role != CallerRoleV1::Trading
        || replay.release_set != release_set
        || replay.market != market.to_bytes()
        || replay.realm != realm
        || replay.context != context.to_bytes()
        || replay.caller_program != trading.to_bytes()
        || replay.rent_refund != rent_refund.to_bytes()
        || replay.open_vault_count != 0
        || replay.next_revision != 1
        || replay.generation != generation
    {
        return Err(refusal(
            "preexisting Direct Custody replay is not the exact first-use setup poststate",
        ));
    }
    Ok(())
}

fn signed_manifest_v1(value: SignedDirectIntentV3) -> Result<ProducedSignedIntentManifestV1> {
    let encoded = value
        .intent
        .encode()
        .map_err(|error| Error::new(format!("Direct intent encode: {error:?}")))?;
    Ok(ProducedSignedIntentManifestV1 {
        maker: value.maker.to_string(),
        intent_base64: BASE64.encode(encoded),
        signature_base64: BASE64.encode(value.signature),
    })
}

fn derive_capability_seal_v1(
    descriptor_digest: [u8; 32],
    trading_semantic_release: [u8; 32],
    registry: Pubkey,
    trading: Pubkey,
) -> Pubkey {
    let action = (DirectExecutionActionV3::InlineOrdinary as u32).to_le_bytes();
    Pubkey::find_program_address(
        &[
            CAPABILITY_SEAL_PDA_DOMAIN_V1,
            CAPABILITY_PROGRAM_SCHEMA_ID_V4.as_slice(),
            descriptor_digest.as_slice(),
            action.as_slice(),
            trading_semantic_release.as_slice(),
            registry.as_ref(),
        ],
        &trading,
    )
    .0
}

/// Resolve one finalized record pair on either evidence topology.
///
/// A LOCAL checked plan aggregates every market record into `plan.records`
/// with its body. A devnet flagship plan seals only infrastructure records;
/// the market records' bodies are authored by the Market input's own hex
/// fields, their raw addresses by the sealed campaign account rows, and their
/// schemas by the decoded ordinary descriptor plus the protocol's schema
/// constants. Both coordinates are self-verifying: the raw record address is
/// the Registry PDA of (schema, content digest), so a wrong schema or body
/// cannot resolve.
/// Address of one Registry record under the seed order the Registry itself uses.
///
/// The seeds come from [`RecordKeyV1`] rather than being spelled here: a
/// Registry record address is the Registry's fact, and this producer is not a
/// second author of it. `crates/dclutch-record-contract` exports the
/// constructor for exactly this, and `programs/dclutch-registry-sbf`'s own
/// `derive_record_pda` reads the same three segments in the same order.
fn record_address_v1(registry: &Pubkey, seeds: RecordPdaSeedsV1) -> Pubkey {
    Pubkey::find_program_address(
        &[
            seeds.domain(),
            seeds.schema_release_id().as_bytes(),
            seeds.expected_digest().as_bytes(),
        ],
        registry,
    )
    .0
}

pub(crate) fn resolved_record_v1(
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<RecordPair> {
    if let Some(pair) = plan.records.get(label) {
        authenticate_campaign_record_v1(campaign, label, pair, &plan.registry.program_id)?;
        return Ok(pair.clone());
    }
    let row = campaign_account_v1(campaign, label)?;
    if row.owner != plan.registry.program_id {
        return Err(refusal(format!(
            "campaign {label} record is not Registry-owned"
        )));
    }
    let schema = devnet_market_record_schema_v1(market_input, label)?;
    let digest = hex32(&row.data_sha256)?;
    let registry = pubkey(&plan.registry.program_id)?;
    // Going through RecordKeyV1 also buys the owner's zero-identity refusal,
    // which the raw spelling did not have: an all-zero schema or digest used to
    // derive an address the Registry can never mint, and was caught only
    // downstream as an address mismatch that named neither field.
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(schema).map_err(|_| {
            refusal(format!(
                "campaign {label} record schema is the zero identity"
            ))
        })?,
        ContentDigest::new(digest).map_err(|_| {
            refusal(format!(
                "campaign {label} record content digest is the zero identity"
            ))
        })?,
    );
    let raw = record_address_v1(&registry, key.raw_record_pda_seeds());
    if raw.to_string() != row.address {
        return Err(refusal(format!(
            "campaign {label} is not the canonical Registry record coordinate for its schema and content"
        )));
    }
    let staging = record_address_v1(&registry, key.staging_cursor_pda_seeds());
    let body_hex = devnet_market_record_body_hex_v1(market_input, label).unwrap_or_default();
    if !body_hex.is_empty() {
        let body = decode_hex_v1(&body_hex, label)?;
        if sha256_hex(&body) != row.data_sha256 {
            return Err(refusal(format!(
                "Direct Market input {label} body differs from the sealed campaign record content"
            )));
        }
    }
    Ok(RecordPair {
        raw: raw.to_string(),
        staging: staging.to_string(),
        schema_id: hex_encode_v1(&schema),
        content_sha256: row.data_sha256.clone(),
        body_hex,
    })
}

/// The schema each devnet market record was published under; descriptor-owned
/// schemas come from the decoded ordinary descriptor itself.
fn devnet_market_record_schema_v1(market_input: &MarketRunInput, label: &str) -> Result<[u8; 32]> {
    let direct = market_input.direct_capability.as_ref().ok_or_else(|| {
        refusal("devnet Direct record resolution requires the typed Direct capability payload")
    })?;
    Ok(match label {
        "capability_manifest_record" => CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        "direct_program_set_record" => CAPABILITY_PROGRAM_SET_SCHEMA_RELEASE_ID_V2,
        "direct_ordinary_descriptor_record" => CAPABILITY_PROGRAM_SCHEMA_ID_V4,
        "direct_activation_account_profile_record" => {
            dclutch_direct_codec::activation_bundle_v1::direct_activation_account_profile_schema_v1(
            )
        }
        "direct_activation_effect_record" => {
            dclutch_direct_codec::activation_bundle_v1::direct_activation_effect_schema_v1()
        }
        "direct_activation_descriptor_record" => {
            dclutch_direct_codec::activation_bundle_v1::direct_activation_descriptor_schema_v1()
        }
        "product_record" => PRODUCT_RECORD_SCHEMA_ID_V2,
        "result_domain_record" => RESULT_DOMAIN_SCHEMA_ID_V2,
        "portfolio_record" => PORTFOLIO_SCHEMA_ID_V2,
        "linked_liability_basis_record" => GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        "realm_record" => REALM_SCHEMA_RELEASE_ID_V1,
        "direct_execution_config_record"
        | "direct_ordinary_account_profile_record"
        | "direct_ordinary_request_profile_record"
        | "direct_ordinary_transition_record"
        | "direct_ordinary_effect_record"
        | "direct_ordinary_lifecycle_policy_record"
        | "direct_ordinary_strategy_record" => {
            let descriptor_bytes = decode_hex_v1(
                &direct.ordinary_descriptor_hex,
                "direct_ordinary_descriptor_record",
            )?;
            let descriptor = CapabilityProgramV4::decode(&descriptor_bytes)
                .map_err(|error| Error::new(format!("Direct CapabilityProgramV4: {error:?}")))?;
            match label {
                "direct_execution_config_record" => descriptor.config_schema().to_bytes(),
                "direct_ordinary_account_profile_record" => {
                    descriptor.account_profile().schema().to_bytes()
                }
                "direct_ordinary_request_profile_record" => {
                    descriptor.request_profile().schema().to_bytes()
                }
                "direct_ordinary_transition_record" => descriptor.transition().schema().to_bytes(),
                "direct_ordinary_effect_record" => descriptor.effect().schema().to_bytes(),
                "direct_ordinary_lifecycle_policy_record" => {
                    descriptor.lifecycle().schema().to_bytes()
                }
                "direct_ordinary_strategy_record" => descriptor.strategy().schema().to_bytes(),
                _ => unreachable!("the outer match admitted only descriptor-owned labels"),
            }
        }
        other => {
            return Err(refusal(format!(
                "no devnet schema author is registered for record {other}"
            )));
        }
    })
}

/// The Market input field that authors one devnet record body, when one does.
fn devnet_market_record_body_hex_v1(market_input: &MarketRunInput, label: &str) -> Option<String> {
    let direct = market_input.direct_capability.as_ref()?;
    Some(match label {
        "capability_manifest_record" => market_input.capability_manifest_hex.clone(),
        "linked_liability_basis_record" => market_input.linked_basis_hex.clone(),
        "direct_program_set_record" => direct.program_set_hex.clone(),
        "direct_ordinary_descriptor_record" => direct.ordinary_descriptor_hex.clone(),
        "direct_execution_config_record" => direct.execution_config_hex.clone(),
        "direct_ordinary_account_profile_record" => direct.ordinary_account_profile_hex.clone(),
        "direct_ordinary_request_profile_record" => direct.ordinary_request_profile_hex.clone(),
        "direct_ordinary_transition_record" => direct.ordinary_transition_hex.clone(),
        "direct_ordinary_effect_record" => direct.ordinary_effect_hex.clone(),
        "direct_ordinary_lifecycle_policy_record" => direct.ordinary_lifecycle_policy_hex.clone(),
        "direct_ordinary_strategy_record" => direct.ordinary_strategy_hex.clone(),
        "direct_activation_account_profile_record" => direct.activation_account_profile_hex.clone(),
        "direct_activation_effect_record" => direct.activation_effect_hex.clone(),
        "direct_activation_descriptor_record" => direct.activation_descriptor_hex.clone(),
        _ => return None,
    })
}

fn record_coordinates_v1(
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<ProducedRecordPairCoordinatesV1> {
    let pair = resolved_record_v1(plan, market_input, campaign, label)?;
    Ok(ProducedRecordPairCoordinatesV1 {
        raw: pair.raw,
        staging: pair.staging,
    })
}

fn record_body_v1(
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<Vec<u8>> {
    let pair = resolved_record_v1(plan, market_input, campaign, label)?;
    if pair.body_hex.is_empty() {
        return Err(refusal(format!("no body author exists for record {label}")));
    }
    let body = decode_hex_v1(&pair.body_hex, label)?;
    if sha256_hex(&body) != pair.content_sha256 {
        return Err(refusal(format!("checked {label} body digest changed")));
    }
    Ok(body)
}

fn record_digest_v1(
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<[u8; 32]> {
    let pair = resolved_record_v1(plan, market_input, campaign, label)?;
    hex32(&pair.content_sha256)
}

fn require_market_body_matches_record_v1(
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
    bytes: &[u8],
) -> Result<()> {
    if record_body_v1(plan, market_input, campaign, label)? != bytes {
        return Err(refusal(format!(
            "Direct Market input and checked {label} bodies differ"
        )));
    }
    Ok(())
}

/// The Direct EXECUTION capability root's account shape, with each condition
/// said on its own.
///
/// This used to be one disjunction reported as "Direct root owner or width
/// changed", and that sentence is the reason it now has a function of its own.
///
/// **Absence is checked first and named as absence.** The finalized snapshot
/// represents an account that does not exist as a System-owned, zero-lamport,
/// zero-length placeholder rather than refusing
/// (`FinalizedSnapshotV1::from_rpc`), so a root that was never created arrives
/// here looking exactly like a root whose owner changed — and the operator was
/// told the WIDTH changed. It cost a lane a day: a population run read
/// twenty-one refused fills as evidence that a widened market's capability
/// closure had not followed its outcome count
/// (`docs/evidence/SIMULATOR_POPULATION_DRIVEN_2026_08_30.md`). Width was never
/// the question. Both sides of the width comparison are compile-time constants
/// (`CAPABILITY_ROOT_HEADER_BYTES_V1 + DIRECT_ROOT_STATE_BYTES_V1`), the root
/// PDA's own seeds carry no width term, and the roots simply had never been
/// activated — because until the loopback activation endpoint existed, no
/// Direct root could be created on a local validator at all.
fn authenticate_direct_execution_root_shape_v1(
    root: Pubkey,
    account: &dclutch_operator::ObservedAccount,
    trading: Pubkey,
) -> Result<()> {
    let expected_bytes = CAPABILITY_ROOT_HEADER_BYTES_V1
        .checked_add(dclutch_direct_codec::successor::DIRECT_ROOT_STATE_BYTES_V1)
        .ok_or_else(|| refusal("Direct root width overflowed"))?;
    // ABSENCE IS EXCLUSIVE and is answered alone. Unlike the clauses below it
    // is not one fact among several that could be wrong together: when the
    // account does not exist, the owner, the executable flag and the width all
    // belong to the snapshot's placeholder rather than to any root, so
    // reporting them beside this would be reporting three facts about nothing.
    if account.owner == system_program::ID && account.lamports == 0 && account.data.is_empty() {
        return Err(refusal(format!(
            "the Direct execution capability root {root} does not exist. Founding never creates \
             it and no other route can: run {} against an owned loopback validator, or {} against \
             acknowledged devnet, before producing a trade on this Market",
            crate::direct_capability_activation::LOCAL_DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1,
            crate::direct_capability_activation::DIRECT_CAPABILITY_ACTIVATION_COMMAND_V1,
        )));
    }
    // An OCCUPIED root can be wrong in all three ways at once, so all three are
    // collected and reported together, the way `refusing_ticket_half_clauses_v1`
    // reports a ticket's. Returning only the first is what the sentence this
    // replaced did, one disjunct at a time, and an operator staring at a
    // foreign-owned root of the wrong width would have to fix it twice to find
    // that out.
    let mut refusing = Vec::new();
    if account.owner != trading {
        refusing.push(format!(
            "it is owned by {} rather than by Trading {trading}",
            account.owner
        ));
    }
    if account.executable {
        refusing.push("it is executable".to_owned());
    }
    if account.data.len() != expected_bytes {
        refusing.push(format!(
            "it is {} bytes, not the exact {expected_bytes} this release's capability header and \
             Direct tail occupy",
            account.data.len()
        ));
    }
    if refusing.is_empty() {
        return Ok(());
    }
    Err(refusal(format!(
        "the Direct execution capability root {root} is not the account this release activates: {}",
        refusing.join("; ")
    )))
}

fn authenticate_campaign_record_v1(
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
    pair: &RecordPair,
    registry: &str,
) -> Result<()> {
    let account = campaign_account_v1(campaign, label)?;
    if account.address != pair.raw
        || account.data_sha256 != pair.content_sha256
        || account.owner != registry
    {
        return Err(refusal(format!(
            "campaign {label} does not match its checked Registry record pair"
        )));
    }
    Ok(())
}

fn campaign_account_v1<'a>(
    campaign: &'a campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<&'a campaign::CampaignAccountEvidenceV1> {
    campaign
        .accounts
        .get(label)
        .ok_or_else(|| refusal(format!("campaign omitted {label}")))
}

fn campaign_address_v1(
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<Pubkey> {
    pubkey(&campaign_account_v1(campaign, label)?.address)
}

/// Derive the Direct EXECUTION capability root for one Open Market.
///
/// The seeds are the semantic identities the on-chain `authenticate_header`
/// re-derives from a live root's stored header: release set, market, the
/// Open generation, and the manifest-selected entry's kind/release/config.
/// Every input is an author's own value - the manifest body resolves through
/// the sealed record closure, and the generation is the PDA-authenticated
/// Market identity read at finalized commitment.
pub(crate) fn derived_direct_execution_root_v1(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    market_input: &MarketRunInput,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    market: Pubkey,
    entry_index: u16,
) -> Result<Pubkey> {
    let manifest_pair =
        resolved_record_v1(plan, market_input, campaign, "capability_manifest_record")?;
    let manifest_body = decode_hex_v1(&manifest_pair.body_hex, "capability manifest")?;
    if sha256_hex(&manifest_body) != manifest_pair.content_sha256 {
        return Err(refusal("capability manifest body digest changed"));
    }
    let manifest = dclutch_capability_contract::CapabilityManifestV1::decode(&manifest_body)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;
    let entry = manifest
        .entry(entry_index)
        .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
    let market_account = rpc.required_account(market, "Core Market state")?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Core Market state: {error:?}")))?;
    if market_state.identity.capability_manifest.to_bytes() != hash(&manifest_body).to_bytes() {
        return Err(refusal(
            "Market identity selects another capability manifest",
        ));
    }
    let selection = dclutch_release_set_contract::CapabilityExecutionSelectionV1::new(
        entry_index,
        dclutch_core_contract::ContentId::new(hash(&manifest_body).to_bytes())
            .map_err(|_| refusal("manifest identity is zero"))?,
        entry.kind_id(),
        entry.release_id(),
        entry.config_id(),
    )
    .map_err(|error| Error::new(format!("execution selection: {error:?}")))?;
    let header = dclutch_capability_program_contract::CapabilityRootHeaderV1::new(
        dclutch_core_contract::ContentId::new(
            market_state.identity.selected_release_set.to_bytes(),
        )
        .map_err(|_| refusal("release set is zero"))?,
        market.to_bytes(),
        market_state.identity.generation,
        selection,
        dclutch_capability_program_contract::SelectedRecordBumpsV1::default(),
    )
    .map_err(|error| Error::new(format!("root header: {error:?}")))?;
    Ok(Pubkey::find_program_address(
        &header.seeds().as_slices(),
        &pubkey(&plan.trading.program_id)?,
    )
    .0)
}

fn expected_buyer_position_v1(aggregate: Pubkey, owner: Pubkey, claims: Pubkey) -> Result<Pubkey> {
    let seeds = ProtocolPositionSeedsV2::new(aggregate.to_bytes(), owner.to_bytes())
        .map_err(|error| Error::new(format!("buyer Position seeds: {error:?}")))?;
    Ok(Pubkey::find_program_address(&seeds.as_slices(), &claims).0)
}

fn first_funded_outcome_v1(
    view: LiabilityBasisPositionViewV2,
    bytes: &[u8],
    minimum: u64,
) -> Result<u32> {
    for outcome in 0..view.claim_count {
        let balance = view
            .balance(bytes, outcome)
            .map_err(|error| Error::new(format!("seller claim balance {outcome}: {error:?}")))?;
        if balance >= minimum {
            return Ok(outcome);
        }
    }
    Err(refusal(format!(
        "seller Position has no outcome with the required {minimum} claim atoms"
    )))
}

fn exact_quote_v1(quantity: u64, price: u64, scale: u64) -> Result<u64> {
    let product = u128::from(quantity) * u128::from(price);
    let scale = u128::from(scale);
    if scale == 0 || product % scale != 0 {
        return Err(refusal("Direct quote is not exactly representable"));
    }
    u64::try_from(product / scale).map_err(|_| refusal("Direct quote overflowed"))
}

fn fee_floor_v1(gross: u64, basis_points: u16) -> Result<u64> {
    u64::try_from(u128::from(gross) * u128::from(basis_points) / 10_000)
        .map_err(|_| refusal("Direct fee overflowed"))
}

/// Refuse when a coordinate is the default key, or when two coordinates that
/// must stay separate accounts carry the same key.
///
/// Every refusal NAMES the coordinates that collided and reports all of them,
/// not the first: twelve anonymous coordinates refusing as one line is a
/// diagnosis the operator cannot make from outside the process.
///
/// `admitted_alias` lists the coordinate pairs that MAY be one key. Both names
/// in a pair must appear in `coordinates`, so a renamed or misspelled
/// coordinate widens nothing -- it refuses instead.
fn require_distinct_v1(
    coordinates: &[(&str, Pubkey)],
    admitted_alias: &[(&str, &str)],
) -> Result<()> {
    for (name, key) in coordinates {
        if *key == Pubkey::default() {
            return Err(refusal(format!(
                "Direct setup coordinate {name} is the default public key"
            )));
        }
    }
    for (left, right) in admitted_alias {
        if !coordinates.iter().any(|(name, _)| name == left)
            || !coordinates.iter().any(|(name, _)| name == right)
        {
            return Err(Error::new(format!(
                "admitted Direct coordinate alias {left}/{right} names a coordinate this check does not carry"
            )));
        }
    }
    let admitted = |left: &str, right: &str| {
        admitted_alias.iter().any(|(one, other)| {
            (*one == left && *other == right) || (*one == right && *other == left)
        })
    };
    let mut aliased = Vec::new();
    for (index, (name, key)) in coordinates.iter().enumerate() {
        for (other, other_key) in coordinates.iter().skip(index.saturating_add(1)) {
            if other_key == key && !admitted(name, other) {
                aliased.push(format!("{name} and {other} are both {key}"));
            }
        }
    }
    if aliased.is_empty() {
        return Ok(());
    }
    Err(refusal(format!(
        "Direct participant/setup coordinates alias: {}",
        aliased.join("; ")
    )))
}

pub(crate) fn read_keypair_v1(path: &Path, label: &str) -> Result<Keypair> {
    Ok(Keypair::new_from_array(campaign::read_keypair_file(
        path, label,
    )?))
}

fn private_session_sha256_v1(session: &ProducedDirectTradePrivateSessionV1) -> Result<String> {
    let mut canonical = session.clone();
    canonical.session_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn producer_receipt_sha256_v1(receipt: &OwnedLoopbackDirectProducerReceiptV1) -> Result<String> {
    let mut canonical = receipt.clone();
    canonical.receipt_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn pretty_json_bytes_v1(value: &impl Serialize) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn write_create_new_v1(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new().create_new(true).write(true).open(path)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}

fn parse_arguments_v1(arguments: Vec<String>) -> Result<OwnedLoopbackDirectProducerArgumentsV1> {
    let mut rpc_url = None;
    let mut plan = None;
    let mut market_input = None;
    let mut campaign_report = None;
    let mut participant_report = None;
    let mut key_dir = None;
    let mut output_dir = None;
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        let slot = match argument.as_str() {
            "--rpc-url" => &mut rpc_url,
            "--plan" => &mut plan,
            "--market-input" => &mut market_input,
            "--campaign-report" => &mut campaign_report,
            "--participant-report" => &mut participant_report,
            "--key-dir" => &mut key_dir,
            "--output-dir" => &mut output_dir,
            _ => {
                return Err(Error::new(format!(
                    "unknown Direct producer argument: {argument}"
                )));
            }
        };
        if slot.replace(value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    Ok(OwnedLoopbackDirectProducerArgumentsV1 {
        rpc_url: required_v1(rpc_url, "--rpc-url")?,
        plan: PathBuf::from(required_v1(plan, "--plan")?),
        market_input: PathBuf::from(required_v1(market_input, "--market-input")?),
        campaign_report: PathBuf::from(required_v1(campaign_report, "--campaign-report")?),
        participant_report: PathBuf::from(required_v1(participant_report, "--participant-report")?),
        key_dir: PathBuf::from(required_v1(key_dir, "--key-dir")?),
        output_dir: PathBuf::from(required_v1(output_dir, "--output-dir")?),
    })
}

fn parse_devnet_session_arguments_v1(
    arguments: Vec<String>,
) -> Result<DevnetDirectSessionProducerArgumentsV1> {
    let mut values = std::collections::BTreeMap::<String, String>::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if !matches!(
            argument.as_str(),
            DEVNET_ACKNOWLEDGMENT_FLAG
                | "--public-manifest"
                | "--expected-public-manifest-sha256"
                | "--plan"
                | "--expected-plan-sha256"
                | "--market-input"
                | "--expected-market-input-sha256"
                | "--seller-participant"
                | "--expected-seller-participant-sha256"
                | "--buyer-participant"
                | "--expected-buyer-participant-sha256"
                | "--payer-keypair"
                | "--journal-dir"
                | "--evidence-file"
                | "--session"
                | "--producer-journal"
        ) {
            return Err(Error::new(format!(
                "unknown devnet Direct session producer argument: {argument}",
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let take = |label: &str| {
        values
            .get(label)
            .cloned()
            .ok_or_else(|| Error::new(format!("{label} is required")))
    };
    if take(DEVNET_ACKNOWLEDGMENT_FLAG)? != DEVNET_GENESIS_HASH {
        return Err(refusal(
            "devnet Direct session producer requires the exact public-devnet genesis acknowledgment",
        ));
    }
    Ok(DevnetDirectSessionProducerArgumentsV1 {
        public_manifest: PathBuf::from(take("--public-manifest")?),
        expected_public_manifest_sha256: take("--expected-public-manifest-sha256")?,
        plan: PathBuf::from(take("--plan")?),
        expected_plan_sha256: take("--expected-plan-sha256")?,
        market_input: PathBuf::from(take("--market-input")?),
        expected_market_input_sha256: take("--expected-market-input-sha256")?,
        seller_participant: PathBuf::from(take("--seller-participant")?),
        expected_seller_participant_sha256: take("--expected-seller-participant-sha256")?,
        buyer_participant: PathBuf::from(take("--buyer-participant")?),
        expected_buyer_participant_sha256: take("--expected-buyer-participant-sha256")?,
        payer_keypair: PathBuf::from(take("--payer-keypair")?),
        journal_dir: PathBuf::from(take("--journal-dir")?),
        evidence_file: PathBuf::from(take("--evidence-file")?),
        session: PathBuf::from(take("--session")?),
        producer_journal: PathBuf::from(take("--producer-journal")?),
    })
}

fn parse_devnet_direct_arguments_v1(
    arguments: Vec<String>,
) -> Result<DevnetDirectProducerArgumentsV1> {
    let mut values = BTreeMap::<String, String>::new();
    let mut iterator = arguments.into_iter();
    while let Some(argument) = iterator.next() {
        let value = iterator
            .next()
            .ok_or_else(|| Error::new(format!("{argument} requires a value")))?;
        if !matches!(
            argument.as_str(),
            DEVNET_ACKNOWLEDGMENT_FLAG
                | "--rpc-url"
                | "--plan"
                | "--expected-plan-sha256"
                | "--market-input"
                | "--expected-market-input-sha256"
                | "--campaign-report"
                | "--expected-campaign-report-sha256"
                | "--buyer-participant"
                | "--expected-buyer-participant-sha256"
                | "--checked-execution-release"
                | "--expected-checked-execution-release-sha256"
                | "--seller-ticket"
                | "--expected-seller-ticket-sha256"
                | "--buyer-ticket"
                | "--expected-buyer-ticket-sha256"
                | "--payer"
                | "--payer-keypair"
                | "--output-dir"
        ) {
            return Err(Error::new(format!(
                "unknown devnet Direct producer argument: {argument}"
            )));
        }
        if values.insert(argument.clone(), value).is_some() {
            return Err(Error::new(format!("{argument} may be supplied only once")));
        }
    }
    let take = |label: &str| {
        values
            .get(label)
            .cloned()
            .ok_or_else(|| Error::new(format!("{label} is required")))
    };
    if take(DEVNET_ACKNOWLEDGMENT_FLAG)? != DEVNET_GENESIS_HASH {
        return Err(refusal(
            "devnet Direct producer requires the exact public-devnet genesis acknowledgment",
        ));
    }
    Ok(DevnetDirectProducerArgumentsV1 {
        rpc_url: take("--rpc-url")?,
        plan: PathBuf::from(take("--plan")?),
        expected_plan_sha256: take("--expected-plan-sha256")?,
        market_input: PathBuf::from(take("--market-input")?),
        expected_market_input_sha256: take("--expected-market-input-sha256")?,
        campaign_report: PathBuf::from(take("--campaign-report")?),
        expected_campaign_report_sha256: take("--expected-campaign-report-sha256")?,
        buyer_participant: PathBuf::from(take("--buyer-participant")?),
        expected_buyer_participant_sha256: take("--expected-buyer-participant-sha256")?,
        checked_execution_release: PathBuf::from(take("--checked-execution-release")?),
        expected_checked_execution_release_sha256: take(
            "--expected-checked-execution-release-sha256",
        )?,
        seller_ticket: PathBuf::from(take("--seller-ticket")?),
        expected_seller_ticket_sha256: take("--expected-seller-ticket-sha256")?,
        buyer_ticket: PathBuf::from(take("--buyer-ticket")?),
        expected_buyer_ticket_sha256: take("--expected-buyer-ticket-sha256")?,
        payer: pubkey(&take("--payer")?)?,
        payer_keypair: PathBuf::from(take("--payer-keypair")?),
        output_dir: PathBuf::from(take("--output-dir")?),
    })
}

fn authenticate_devnet_direct_participant_pair_v1(
    source: &AuthenticatedDevnetDirectSessionSourceV1,
    seller: &user_position_admission::FinalizedDirectParticipantEvidenceV1,
    buyer: &user_position_admission::FinalizedDirectParticipantEvidenceV1,
) -> Result<()> {
    if source.seller == source.buyer
        || source.seller_replay == source.buyer_replay
        || seller.owner != source.seller
        || buyer.owner != source.buyer
        || seller.market != source.market
        || buyer.market != source.market
        || seller.claims_market != source.claims_market
        || buyer.claims_market != source.claims_market
        || seller.position != source.seller_position
        || buyer.position != source.buyer_position
        || seller.collateral_account != source.seller_collateral
        || buyer.collateral_account != source.buyer_collateral
        || seller.custody_authority != source.custody_authority
        || buyer.custody_authority != source.custody_authority
        || seller.mint != source.mint
        || buyer.mint != source.mint
        || seller.token_program != source.token_program
        || buyer.token_program != source.token_program
        || seller.collateral_quantity_atoms == 0
        || buyer.collateral_quantity_atoms == 0
        || seller.admission_signature.is_empty()
        || buyer.admission_signature.is_empty()
        || seller.collateral_signature.is_empty()
        || buyer.collateral_signature.is_empty()
        || seller.admission_slot == 0
        || buyer.admission_slot == 0
        || seller.collateral_slot < seller.admission_slot
        || buyer.collateral_slot < buyer.admission_slot
    {
        return Err(refusal(
            "Direct public route/tickets and seller/buyer participant snapshots do not form one exact Market, replay, Position, collateral, Mint, and Custody closure",
        ));
    }
    // No admitted alias here: these are the two participants' OWN identities
    // and owned accounts. The payer is not among them, so nothing on this list
    // is the pair the trade site admits.
    require_distinct_v1(
        &[
            ("seller", source.seller),
            ("buyer", source.buyer),
            ("seller maker replay root", source.seller_replay),
            ("buyer maker replay root", source.buyer_replay),
            ("seller Position", source.seller_position),
            ("buyer Position", source.buyer_position),
            ("seller collateral account", source.seller_collateral),
            ("buyer collateral account", source.buyer_collateral),
        ],
        &[],
    )?;
    Ok(())
}

fn accepted_regular_v1(path: &Path, expected_sha256: &str, label: &str) -> Result<PathBuf> {
    hex32(expected_sha256)?;
    let path = canonical_regular_v1(path, label)?;
    if sha256_hex(&fs::read(&path)?) != expected_sha256 {
        return Err(refusal(format!(
            "{label} changed from its accepted SHA-256"
        )));
    }
    Ok(path)
}

fn canonical_runtime_keypair_path_v1(path: &Path) -> Result<PathBuf> {
    // Deliberately metadata-only: session production must not read or parse
    // the runtime-created wallet file. The executor opens it only at its
    // separately authorized signing boundary.
    canonical_regular_v1(path, "Direct runtime payer keypair")
}

fn canonical_output_path_v1(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(refusal(format!("{label} path is not absolute")));
    }
    let parent = path
        .parent()
        .ok_or_else(|| refusal(format!("{label} has no parent directory")))?;
    let canonical_parent = canonical_directory_v1(parent, &format!("{label} parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| refusal(format!("{label} has no file name")))?;
    let canonical = canonical_parent.join(name);
    if canonical != path {
        return Err(refusal(format!("{label} path is not canonical")));
    }
    if let Ok(metadata) = fs::symlink_metadata(&canonical) {
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(refusal(format!("{label} is not one regular file")));
        }
    }
    Ok(canonical)
}

fn canonical_directory_target_v1(path: &Path, label: &str) -> Result<PathBuf> {
    if path.exists() {
        return canonical_directory_v1(path, label);
    }
    if !path.is_absolute() {
        return Err(refusal(format!("{label} path is not absolute")));
    }
    let parent = path
        .parent()
        .ok_or_else(|| refusal(format!("{label} has no parent directory")))?;
    let canonical_parent = canonical_directory_v1(parent, &format!("{label} parent"))?;
    let name = path
        .file_name()
        .ok_or_else(|| refusal(format!("{label} has no directory name")))?;
    let canonical = canonical_parent.join(name);
    if canonical != path {
        return Err(refusal(format!("{label} path is not canonical")));
    }
    Ok(canonical)
}

fn devnet_session_producer_state_sha256_v1(
    journal: &DevnetDirectSessionProducerJournalV1,
) -> Result<String> {
    let mut canonical = journal.clone();
    canonical.state_sha256.clear();
    Ok(sha256_hex(&serde_json::to_vec(&canonical)?))
}

fn load_devnet_session_producer_journal_v1(
    path: &Path,
) -> Result<Option<DevnetDirectSessionProducerJournalV1>> {
    if !path.exists() {
        return Ok(None);
    }
    let bytes = fs::read(path)?;
    require_unique_json_v1(&bytes, "Direct producer journal")?;
    let journal: DevnetDirectSessionProducerJournalV1 = serde_json::from_slice(&bytes)?;
    if journal.schema != DEVNET_SESSION_PRODUCER_JOURNAL_SCHEMA_V1
        || journal.cluster != ExpectedClusterV1::Devnet.evidence_label()
        || journal.genesis_hash != DEVNET_GENESIS_HASH
        || journal.state_sha256 != devnet_session_producer_state_sha256_v1(&journal)?
    {
        return Err(refusal(
            "Direct producer journal identity or state digest changed",
        ));
    }
    Ok(Some(journal))
}

fn authenticate_devnet_session_producer_recovery_v1(
    prepared: &DevnetDirectSessionProducerJournalV1,
    existing: &DevnetDirectSessionProducerJournalV1,
) -> Result<()> {
    let recovered_prepared = match existing.phase {
        DevnetDirectSessionProducerPhaseV1::Prepared => existing.clone(),
        DevnetDirectSessionProducerPhaseV1::Finalized => {
            if existing.previous_state_sha256.as_deref() != Some(prepared.state_sha256.as_str())
                || existing.private_session_sha256.is_none()
            {
                return Err(refusal(
                    "finalized Direct producer journal omitted its exact prepared predecessor or session digest",
                ));
            }
            let mut projected = existing.clone();
            projected.phase = DevnetDirectSessionProducerPhaseV1::Prepared;
            projected.private_session_sha256 = None;
            projected.previous_state_sha256 = None;
            projected.state_sha256 = prepared.state_sha256.clone();
            projected
        }
    };
    if serde_json::to_vec(&recovered_prepared)? != serde_json::to_vec(prepared)? {
        return Err(refusal(
            "existing Direct producer journal belongs to another route, participant pair, ticket, release, payer, or output",
        ));
    }
    Ok(())
}

fn authenticate_devnet_private_session_output_v1(
    path: &Path,
    journal: &DevnetDirectSessionProducerJournalV1,
) -> Result<()> {
    let bytes = fs::read(path).map_err(|error| {
        Error::new(format!(
            "Direct private session {}: {error}",
            path.display()
        ))
    })?;
    require_unique_json_v1(&bytes, "Direct private session")?;
    let session: ProducedDirectTradePrivateSessionV1 = serde_json::from_slice(&bytes)?;
    let file_sha256 = sha256_hex(&bytes);
    if session.schema != DEVNET_PRIVATE_SESSION_SCHEMA_V1
        || session.session_sha256 != private_session_sha256_v1(&session)?
        || journal.private_session_sha256.as_deref() != Some(file_sha256.as_str())
        || session.public_manifest != journal.public_manifest
        || session.public_manifest_sha256 != journal.public_manifest_sha256
        || session.plan != journal.plan
        || session.market_input != journal.market_input
        || session.payer_keypair != journal.payer_keypair
        || session.journal_dir != journal.journal_dir
        || session.evidence_file != journal.evidence_file
        || path.display().to_string() != journal.private_session
    {
        return Err(refusal(
            "Direct private session output differs from its finalized producer journal",
        ));
    }
    Ok(())
}

fn create_or_authenticate_session_journal_directory_v1(path: &Path) -> Result<()> {
    if path.exists() {
        canonical_directory_v1(path, "Direct journal directory")?;
        if fs::read_dir(path)?.next().is_some() {
            return Err(refusal(
                "prepared Direct session journal directory is not empty before session creation",
            ));
        }
        return Ok(());
    }
    fs::create_dir(path)?;
    sync_parent_v1(path)?;
    Ok(())
}

fn sync_parent_v1(path: &Path) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| refusal("durable Direct producer path has no parent"))?;
    OpenOptions::new().read(true).open(parent)?.sync_all()?;
    Ok(())
}

fn write_create_new_durable_v1(path: &Path, bytes: &[u8]) -> Result<()> {
    write_create_new_v1(path, bytes)?;
    sync_parent_v1(path)
}

fn replace_json_durable_v1<T: Serialize>(path: &Path, expected: &T, next: &T) -> Result<()> {
    let expected_bytes = pretty_json_bytes_v1(expected)?;
    if fs::read(path)? != expected_bytes {
        return Err(refusal(
            "Direct producer journal changed before its Finalized transition",
        ));
    }
    let temporary = path.with_extension("direct-session-producer.tmp");
    if temporary.exists() {
        return Err(refusal(
            "Direct producer temporary journal path already exists",
        ));
    }
    write_create_new_v1(&temporary, &pretty_json_bytes_v1(next)?)?;
    fs::rename(&temporary, path)?;
    sync_parent_v1(path)
}

fn required_v1(value: Option<String>, label: &str) -> Result<String> {
    value.ok_or_else(|| Error::new(format!("{label} is required")))
}

fn canonical_regular_v1(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(refusal(format!("{label} path is not absolute")));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(refusal(format!("{label} is not one regular file")));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err(refusal(format!("{label} path is not canonical")));
    }
    Ok(canonical)
}

fn canonical_directory_v1(path: &Path, label: &str) -> Result<PathBuf> {
    if !path.is_absolute() {
        return Err(refusal(format!("{label} path is not absolute")));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| Error::new(format!("{label} {}: {error}", path.display())))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(refusal(format!("{label} is not one directory")));
    }
    let canonical = fs::canonicalize(path)?;
    if canonical != path {
        return Err(refusal(format!("{label} path is not canonical")));
    }
    Ok(canonical)
}

fn require_unique_json_v1(bytes: &[u8], label: &str) -> Result<()> {
    parse_json_without_duplicate_keys_v1(bytes)
        .map_err(|error| Error::new(format!("{label} {error}")))?;
    Ok(())
}

pub(crate) fn decode_hex_v1(value: &str, label: &str) -> Result<Vec<u8>> {
    if value.len() % 2 != 0
        || value
            .bytes()
            .any(|byte| !byte.is_ascii_digit() && !(b'a'..=b'f').contains(&byte))
    {
        return Err(refusal(format!(
            "{label} is not canonical lowercase even-width hex"
        )));
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble_v1(pair[0]).ok_or_else(|| refusal(format!("{label} hex")))?;
            let low = hex_nibble_v1(pair[1]).ok_or_else(|| refusal(format!("{label} hex")))?;
            Ok(high << 4 | low)
        })
        .collect()
}

fn hex_nibble_v1(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        _ => None,
    }
}

fn hex_encode_v1(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut output = String::with_capacity(64);
    for byte in digest {
        use core::fmt::Write as _;
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn refusal(reason: impl AsRef<str>) -> Error {
    Error::new(format!("REFUSED: {}", reason.as_ref()))
}

#[cfg(test)]
mod tests {
    use solana_sdk::{
        pubkey::Pubkey,
        signature::{Keypair, Signer as _},
    };

    use dclutch_token_svm::{TokenAccount, state::TokenAccountLayoutV1};
    use solana_sdk_ids::system_program;

    use crate::rpc::RpcAccount;

    use super::{
        ACCOUNT_BYTES, CAPABILITY_SEAL_PDA_DOMAIN_V1, DEVNET_DIRECT_PRODUCER_COMMAND_V1,
        DEVNET_SESSION_PRODUCER_JOURNAL_SCHEMA_V1, DevnetDirectParticipantSourceV1,
        DevnetDirectSessionProducerJournalV1, DevnetDirectSessionProducerPhaseV1,
        DirectTokenAccountRoleV1, DirectTokenAccountSeedsV1, DirectTokenDestinationPrestateV1,
        DirectTradeTermsV1, EXECUTION_PRICE_V1, FEE_BASIS_POINTS_V1, FILL_ATOMS_V1,
        FinalizedTicketExpectationV1, OwnedLoopbackDirectProducerReceiptV1,
        ProducedDirectTradePrivateSessionV1, ProducedReplaySetupV1, ProducedTokenSetupV1,
        SignedDirectIntentV3, authenticate_devnet_direct_participant_pair_v1,
        authenticate_devnet_session_producer_recovery_v1,
        authenticate_direct_execution_root_shape_v1, classify_direct_token_destination_v1,
        derive_capability_seal_v1, devnet_direct_usage, devnet_session_producer_state_sha256_v1,
        devnet_session_usage, exact_quote_v1, exact_ticket_pair_terms_v1, fee_floor_v1,
        parse_devnet_direct_arguments_v1, parse_devnet_session_arguments_v1,
        private_session_sha256_v1, producer_receipt_sha256_v1, refusing_ticket_clauses_v1,
        require_distinct_v1,
    };
    use crate::{
        cluster::{DEVNET_ACKNOWLEDGMENT_FLAG, DEVNET_GENESIS_HASH},
        direct_ticket::{
            encode_portable_direct_ticket_v1, parse_portable_direct_ticket_v1,
            sign_direct_intent_v1 as signed_intent_v1,
        },
        direct_trade::AuthenticatedDevnetDirectSessionSourceV1,
        user_position_admission::FinalizedDirectParticipantEvidenceV1,
    };
    use dclutch_direct_codec::intent_v2::CompactIntentV2;

    #[test]
    fn exact_demo_arithmetic_is_named_and_integral() -> crate::Result<()> {
        let gross = exact_quote_v1(FILL_ATOMS_V1, EXECUTION_PRICE_V1, 1_000_000)?;
        assert_eq!(gross, 50_000_000);
        assert_eq!(fee_floor_v1(gross, FEE_BASIS_POINTS_V1)?, 250_000);
        assert!(exact_quote_v1(1, EXECUTION_PRICE_V1, 1_000_000).is_err());
        Ok(())
    }

    /// One `ObservedAccount` at an exact shape, built the way the finalized
    /// snapshot builds one.
    fn observed_root_v1(
        key: Pubkey,
        owner: Pubkey,
        lamports: u64,
        executable: bool,
        data: Vec<u8>,
    ) -> dclutch_operator::ObservedAccount {
        dclutch_operator::ObservedAccount {
            observation: dclutch_operator::Observation {
                slot: 1,
                unix_timestamp: 1_800_000_000,
                finality: dclutch_operator::Finality::Finalized,
            },
            key,
            owner,
            lamports,
            executable,
            data,
        }
    }

    /// The regression this whole lane exists for.
    ///
    /// A root that was never activated arrives as the snapshot's System-owned,
    /// zero-lamport, zero-length placeholder. The sentence it produces must say
    /// the root DOES NOT EXIST and must name the route that creates it — and it
    /// must not say "width", because a population lane spent a day reading the
    /// old sentence as a claim about a widened market's outcome count.
    #[test]
    fn an_unactivated_direct_root_refuses_as_absent_and_never_as_a_width() {
        let root = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let error = authenticate_direct_execution_root_shape_v1(
            root,
            &observed_root_v1(root, super::system_program::ID, 0, false, Vec::new()),
            trading,
        )
        .expect_err("an absent root must refuse");
        let text = error.to_string();
        assert!(text.contains(&root.to_string()), "{text}");
        assert!(text.contains("does not exist"), "{text}");
        assert!(
            text.contains("local-private-validator-direct-capability-activation-v1"),
            "the refusal must name the loopback activation endpoint: {text}"
        );
        assert!(
            text.contains("devnet-direct-capability-activation-v1"),
            "the refusal must name the devnet activation endpoint: {text}"
        );
        assert!(
            !text.contains("width") && !text.contains("bytes"),
            "an absent root is not a width finding: {text}"
        );
    }

    /// An occupied root names every clause that failed, not the first, and a
    /// correctly activated root says nothing at all.
    #[test]
    fn an_occupied_direct_root_names_every_clause_that_failed() {
        let root = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let exact = dclutch_capability_program_contract::CAPABILITY_ROOT_HEADER_BYTES_V1
            + dclutch_direct_codec::successor::DIRECT_ROOT_STATE_BYTES_V1;
        // The width both halves of the old comparison were made of. It is a
        // pair of compile-time constants and no Market's outcome count reaches
        // either one.
        assert_eq!(exact, 256);

        authenticate_direct_execution_root_shape_v1(
            root,
            &observed_root_v1(root, trading, 2_672_640, false, vec![0; exact]),
            trading,
        )
        .expect("an activated root of the exact width is admitted");

        let foreign = Pubkey::new_unique();
        let owner_error = authenticate_direct_execution_root_shape_v1(
            root,
            &observed_root_v1(root, foreign, 1, false, vec![0; exact]),
            trading,
        )
        .expect_err("a foreign-owned root must refuse")
        .to_string();
        assert!(owner_error.contains("owned by"), "{owner_error}");
        assert!(owner_error.contains(&foreign.to_string()), "{owner_error}");
        assert!(
            !owner_error.contains("does not exist"),
            "an occupied root is present: {owner_error}"
        );

        let executable_error = authenticate_direct_execution_root_shape_v1(
            root,
            &observed_root_v1(root, trading, 1, true, vec![0; exact]),
            trading,
        )
        .expect_err("an executable root must refuse")
        .to_string();
        assert!(
            executable_error.contains("is executable"),
            "{executable_error}"
        );

        let width_error = authenticate_direct_execution_root_shape_v1(
            root,
            &observed_root_v1(root, trading, 1, false, vec![0; exact - 1]),
            trading,
        )
        .expect_err("a short root must refuse")
        .to_string();
        assert!(width_error.contains("255 bytes"), "{width_error}");
        assert!(width_error.contains("256"), "{width_error}");

        // ALL of them, when an occupied root is wrong in every way at once.
        // Reporting only the first is what the disjunction this replaced did.
        let every_way = authenticate_direct_execution_root_shape_v1(
            root,
            &observed_root_v1(root, foreign, 1, true, vec![0; exact - 1]),
            trading,
        )
        .expect_err("a root wrong in three ways must refuse")
        .to_string();
        for clause in ["owned by", "is executable", "255 bytes"] {
            assert!(
                every_way.contains(clause),
                "every failing clause must be named, {clause} was not: {every_way}"
            );
        }
        assert_eq!(
            every_way.matches("; ").count(),
            2,
            "three clauses join with exactly two separators: {every_way}"
        );

        // A System-owned account that is NOT the snapshot's absence
        // placeholder — it has been paid — is present, not absent.
        let funded_error = authenticate_direct_execution_root_shape_v1(
            root,
            &observed_root_v1(root, super::system_program::ID, 1, false, Vec::new()),
            trading,
        )
        .expect_err("a funded System account at the root coordinate must refuse")
        .to_string();
        assert!(
            !funded_error.contains("does not exist") && funded_error.contains("owned by"),
            "{funded_error}"
        );
    }

    #[test]
    fn session_digest_excludes_only_itself() -> crate::Result<()> {
        let mut session = ProducedDirectTradePrivateSessionV1 {
            schema: "schema".into(),
            public_manifest: "/tmp/public".into(),
            public_manifest_sha256: "11".repeat(32),
            plan: "/tmp/plan".into(),
            market_input: "/tmp/market".into(),
            payer_keypair: "/tmp/payer".into(),
            journal_dir: "/tmp/journal".into(),
            evidence_file: "/tmp/evidence".into(),
            session_sha256: String::new(),
        };
        let digest = private_session_sha256_v1(&session)?;
        session.session_sha256 = digest.clone();
        assert_eq!(private_session_sha256_v1(&session)?, digest);
        session.plan.push('x');
        assert_ne!(private_session_sha256_v1(&session)?, digest);
        Ok(())
    }

    #[test]
    fn producer_completion_digest_binds_status_path_and_both_setup_plans() -> crate::Result<()> {
        let mut receipt = OwnedLoopbackDirectProducerReceiptV1 {
            schema: "dclutch-owned-loopback-direct-trade-producer-receipt-v1".into(),
            status: "produced".into(),
            producer_receipt: "/tmp/direct-trade-produced.json".into(),
            public_manifest: "/tmp/public".into(),
            public_manifest_sha256: "11".repeat(32),
            private_session: "/tmp/private".into(),
            private_session_sha256: "22".repeat(32),
            plan_sha256: "33".repeat(32),
            market_input_sha256: "44".repeat(32),
            campaign_report_sha256: "55".repeat(32),
            participant_report_sha256: "66".repeat(32),
            participant_admission_signature: "admission".into(),
            participant_admission_slot: 7,
            participant_collateral_signature: "collateral".into(),
            participant_collateral_slot: 8,
            replay_setup: ProducedReplaySetupV1 {
                request_base64: "AA==".into(),
                request_sha256: "77".repeat(32),
                maker: "maker".into(),
                maker_root: "maker-root".into(),
                custody_replay: "replay".into(),
                payer: "payer".into(),
                rent_refund: "refund".into(),
                expected_initial_revision: 0,
                expected_resulting_revision: 1,
            },
            token_setup: ProducedTokenSetupV1 {
                request_base64: "AA==".into(),
                request_sha256: "88".repeat(32),
                seller_token: "seller-token".into(),
                seller_owner: "seller".into(),
                fee_token: "fee-token".into(),
                fee_recipient: "fee-recipient".into(),
                payer: "payer".into(),
                rent_refund: "refund".into(),
                mint: "mint".into(),
                token_program: "token-program".into(),
                trading_program: "trading-program".into(),
            },
            receipt_sha256: String::new(),
        };
        let digest = producer_receipt_sha256_v1(&receipt)?;
        receipt.receipt_sha256 = digest.clone();
        assert_eq!(producer_receipt_sha256_v1(&receipt)?, digest);
        for mutate in [
            |receipt: &mut OwnedLoopbackDirectProducerReceiptV1| receipt.status.push('x'),
            |receipt: &mut OwnedLoopbackDirectProducerReceiptV1| {
                receipt.producer_receipt.push('x');
            },
            |receipt: &mut OwnedLoopbackDirectProducerReceiptV1| {
                receipt.replay_setup.request_sha256.push('x');
            },
            |receipt: &mut OwnedLoopbackDirectProducerReceiptV1| {
                receipt.token_setup.request_sha256.push('x');
            },
        ] {
            let mut hostile = receipt.clone();
            mutate(&mut hostile);
            assert_ne!(producer_receipt_sha256_v1(&hostile)?, digest);
        }
        Ok(())
    }

    #[test]
    fn aliases_and_zero_coordinates_are_refused_and_named() {
        let a = Pubkey::new_unique();
        let b = Pubkey::new_unique();
        assert!(require_distinct_v1(&[("payer", a), ("buyer", b)], &[]).is_ok());
        let aliased = require_distinct_v1(&[("payer", a), ("buyer", a)], &[])
            .expect_err("an unadmitted alias refuses");
        let named = format!("{aliased}");
        assert!(named.contains("payer"), "{named}");
        assert!(named.contains("buyer"), "{named}");
        assert!(named.contains(&a.to_string()), "{named}");
        let zero = require_distinct_v1(&[("payer", a), ("buyer", Pubkey::default())], &[])
            .expect_err("a default coordinate refuses");
        assert!(format!("{zero}").contains("buyer"), "{zero}");
    }

    /// Every aliasing pair is reported, not the first one found.
    #[test]
    fn every_aliasing_pair_is_named_in_one_refusal() {
        let a = Pubkey::new_unique();
        let b = Pubkey::new_unique();
        let refused = require_distinct_v1(
            &[
                ("payer", a),
                ("fee recipient", a),
                ("seller", b),
                ("buyer", b),
            ],
            &[],
        )
        .expect_err("two independent aliases refuse");
        let named = format!("{refused}");
        assert!(named.contains("payer and fee recipient"), "{named}");
        assert!(named.contains("seller and buyer"), "{named}");
    }

    /// The wallet-paid trade: the payer IS the buyer, and nothing else may
    /// borrow that key. Neither on-chain setup instruction carries the buyer at
    /// any account index, and both makers authorize with a detached Ed25519
    /// intent signature rather than a transaction signature, so this pair is
    /// the product model rather than a privilege merge.
    #[test]
    fn the_payer_may_be_the_buyer_and_no_other_coordinate() {
        let buyer = Pubkey::new_unique();
        let seller = Pubkey::new_unique();
        let refund = Pubkey::new_unique();
        assert!(
            require_distinct_v1(
                &[
                    ("seller", seller),
                    ("buyer", buyer),
                    ("payer", buyer),
                    ("Market rent beneficiary", refund),
                ],
                &[("payer", "buyer")],
            )
            .is_ok(),
            "a buyer paying their own fees must plan"
        );
        for (label, coordinates) in [
            (
                "seller",
                [
                    ("seller", seller),
                    ("buyer", buyer),
                    ("payer", seller),
                    ("Market rent beneficiary", refund),
                ],
            ),
            (
                "Market rent beneficiary",
                [
                    ("seller", seller),
                    ("buyer", buyer),
                    ("payer", refund),
                    ("Market rent beneficiary", refund),
                ],
            ),
        ] {
            let refused = require_distinct_v1(&coordinates, &[("payer", "buyer")])
                .expect_err("only the buyer pair is admitted");
            assert!(format!("{refused}").contains(label), "{refused}");
        }
        assert!(
            require_distinct_v1(
                &[("seller", seller), ("buyer", seller)],
                &[("payer", "buyer")]
            )
            .is_err(),
            "an admitted pair naming an absent coordinate must not widen the check"
        );
    }

    fn key_v1(text: &str) -> Pubkey {
        core::str::FromStr::from_str(text).expect("canonical base58 address")
    }

    fn ticket_v1(maker: Pubkey, intent: CompactIntentV2) -> SignedDirectIntentV3 {
        SignedDirectIntentV3 {
            maker,
            signature: [1; 64],
            intent,
        }
    }

    /// market19's real coordinates, and the two DIFFERENT addresses the two
    /// derivations produce for the same seller.
    ///
    /// The 2026-08-31 session authored the seller ticket with the PARTICIPANT
    /// collateral derivation -- `create_with_seed(owner, sha256("dclutch:
    /// direct-collateral:v1" | market | owner | release set)[..32], Token-2022)`
    /// -- which is the buyer's shape. The seller half of this route has no
    /// admission and no such account; its collateral destination is the Direct
    /// token PDA `direct_token_setup_v1` creates, which is why the producer
    /// separately requires that address to be a vacant System-owned prestate.
    /// The producer refused and named nothing, and the diagnosis cost a session.
    #[test]
    fn the_seller_direct_token_pda_is_not_the_participant_collateral_address() {
        let market = key_v1("6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4");
        let seller = key_v1("B6qxQCSwVeSfgcFyhNx38mcHs6FrTqRYpuDyQ4TVJ7cs");
        let trading = key_v1("5ywjTNdo6DGTe7bC8p9CgFYWFrBNePx61xeXp8Cdhbkk");
        let seeds = DirectTokenAccountSeedsV1::new(
            market.to_bytes(),
            2,
            seller.to_bytes(),
            DirectTokenAccountRoleV1::Seller,
        )
        .expect("seller Direct token seeds");
        let seller_token = Pubkey::find_program_address(&seeds.as_slices(), &trading).0;
        assert_eq!(
            seller_token.to_string(),
            "2xGo6Cxtfb41HJCrrqWBf73TSaJrbjUmFqr2urDi91q8"
        );
        assert_ne!(
            seller_token.to_string(),
            "HxwXjVqB9aFgNkcxpVHRycB9NqkTvVM3EPx3dxATyDcJ",
            "the participant collateral derivation is not the seller's Direct token account"
        );
    }

    /// The convicted clause, through the gate that convicted it.
    #[test]
    fn the_ticket_gate_names_the_seller_collateral_account_it_refused() {
        let market = key_v1("6WZXJ7jBPPA3eFZPc8hQmmNsf3R4zAZN4DRZzfhcV7a4");
        let seller = key_v1("B6qxQCSwVeSfgcFyhNx38mcHs6FrTqRYpuDyQ4TVJ7cs");
        let buyer = key_v1("3bGyTA4FU2n3hn8Be9rwDiw6ri5ikM6wVTVFVgHCG2Lp");
        let seller_token = key_v1("2xGo6Cxtfb41HJCrrqWBf73TSaJrbjUmFqr2urDi91q8");
        let authored = key_v1("HxwXjVqB9aFgNkcxpVHRycB9NqkTvVM3EPx3dxATyDcJ");
        let buyer_collateral = key_v1("6XUhhHLRrYjrwW7YJU2yqyPJPQgZT8BqBsGDMRV3YyUo");
        let terms = DirectTradeTermsV1 {
            outcome: 0,
            fill: 1_000_000,
            execution_price: 250_000,
            fee_basis_points: 0,
        };
        let expected = FinalizedTicketExpectationV1 {
            seller,
            buyer,
            market,
            generation: 2,
            observation_slot: 490_712_924,
            seller_next_nonce: 0,
            buyer_next_nonce: 0,
            seller_collateral: seller_token,
            buyer_collateral,
            terms,
        };
        let half = |side: u8, collateral: Pubkey| CompactIntentV2 {
            side,
            lifecycle: 0,
            outcome: 0,
            market: market.to_bytes(),
            generation: 2,
            nonce: 0,
            valid_from: 490_000_000,
            valid_through: 506_000_000,
            maximum_fill: 1_000_000,
            limit_price: 250_000,
            fee_basis_points: 0,
            collateral_account: collateral.to_bytes(),
        };
        let good_buyer = ticket_v1(buyer, half(1, buyer_collateral));
        assert!(
            refusing_ticket_clauses_v1(
                &expected,
                ticket_v1(seller, half(0, seller_token)),
                good_buyer,
            )
            .is_empty(),
            "the corrected pair must plan"
        );
        let refusing =
            refusing_ticket_clauses_v1(&expected, ticket_v1(seller, half(0, authored)), good_buyer);
        assert_eq!(refusing.len(), 1, "{refusing:?}");
        let clause = &refusing[0];
        assert!(
            clause.contains("seller ticket collateralAccount"),
            "{clause}"
        );
        assert!(clause.contains(&authored.to_string()), "{clause}");
        assert!(clause.contains(&seller_token.to_string()), "{clause}");
    }

    /// A refusal reports every failing clause, not the first: the single line
    /// this replaced could not tell a stale nonce from an expired window.
    #[test]
    fn the_ticket_gate_names_every_refusing_clause_on_both_halves() {
        let market = Pubkey::new_unique();
        let seller = Pubkey::new_unique();
        let buyer = Pubkey::new_unique();
        let expected = FinalizedTicketExpectationV1 {
            seller,
            buyer,
            market,
            generation: 2,
            observation_slot: 500,
            seller_next_nonce: 3,
            buyer_next_nonce: 4,
            seller_collateral: Pubkey::new_unique(),
            buyer_collateral: Pubkey::new_unique(),
            terms: DirectTradeTermsV1 {
                outcome: 0,
                fill: 1_000_000,
                execution_price: 250_000,
                fee_basis_points: 0,
            },
        };
        let half = |side: u8, nonce: u64, collateral: Pubkey| CompactIntentV2 {
            side,
            lifecycle: 0,
            outcome: 0,
            market: market.to_bytes(),
            generation: 2,
            nonce,
            valid_from: 1,
            valid_through: 1_000,
            maximum_fill: 1_000_000,
            limit_price: 250_000,
            fee_basis_points: 0,
            collateral_account: collateral.to_bytes(),
        };
        let mut stale = half(0, 2, expected.seller_collateral);
        stale.valid_through = 400;
        let mut wrong_buyer = half(1, 4, Pubkey::new_unique());
        wrong_buyer.generation = 3;
        let refusing = refusing_ticket_clauses_v1(
            &expected,
            ticket_v1(seller, stale),
            ticket_v1(buyer, wrong_buyer),
        );
        assert_eq!(refusing.len(), 4, "{refusing:?}");
        for fragment in [
            "seller ticket nonce 2",
            "seller ticket validThrough 400",
            "buyer ticket generation 3",
            "buyer ticket collateralAccount",
        ] {
            assert!(
                refusing.iter().any(|clause| clause.contains(fragment)),
                "{fragment} missing from {refusing:?}"
            );
        }
    }

    #[test]
    fn signatures_bind_side_market_nonce_and_collateral() -> crate::Result<()> {
        let keypair = Keypair::new();
        let intent = CompactIntentV2 {
            side: 0,
            lifecycle: 0,
            outcome: 1,
            market: [3; 32],
            generation: 4,
            nonce: 5,
            valid_from: 6,
            valid_through: 7,
            maximum_fill: FILL_ATOMS_V1,
            limit_price: EXECUTION_PRICE_V1,
            fee_basis_points: FEE_BASIS_POINTS_V1,
            collateral_account: [8; 32],
        };
        let signed = signed_intent_v1(&keypair, intent)?;
        let mut hostile = intent;
        hostile.nonce = hostile.nonce.saturating_add(1);
        assert!(
            !solana_sdk::signature::Signature::from(signed.signature).verify(
                keypair.pubkey().as_ref(),
                &hostile.signed_preimage().expect("hostile preimage")
            )
        );
        Ok(())
    }

    #[test]
    fn capability_seal_projection_uses_the_contract_domain_and_all_six_seeds() {
        assert_eq!(CAPABILITY_SEAL_PDA_DOMAIN_V1, b"dclutch:capability-seal:v1");
        let registry = Pubkey::new_unique();
        let trading = Pubkey::new_unique();
        let first = derive_capability_seal_v1([1; 32], [2; 32], registry, trading);
        assert_ne!(
            first,
            derive_capability_seal_v1([3; 32], [2; 32], registry, trading)
        );
        assert_ne!(
            first,
            derive_capability_seal_v1([1; 32], [4; 32], registry, trading)
        );
        assert_ne!(
            first,
            derive_capability_seal_v1([1; 32], [2; 32], Pubkey::new_unique(), trading)
        );
    }

    fn devnet_pair() -> (
        AuthenticatedDevnetDirectSessionSourceV1,
        FinalizedDirectParticipantEvidenceV1,
        FinalizedDirectParticipantEvidenceV1,
    ) {
        let market = Pubkey::new_unique();
        let claims_market = Pubkey::new_unique();
        let custody_authority = Pubkey::new_unique();
        let mint = Pubkey::new_unique();
        let token_program = Pubkey::new_unique();
        let seller = FinalizedDirectParticipantEvidenceV1 {
            market,
            claims_market,
            position: Pubkey::new_unique(),
            owner: Pubkey::new_unique(),
            collateral_account: Pubkey::new_unique(),
            collateral_quantity_atoms: 700,
            custody_authority,
            mint,
            token_program,
            admission_signature: "seller-admission".into(),
            admission_slot: 10,
            collateral_signature: "seller-collateral".into(),
            collateral_slot: 11,
        };
        let buyer = FinalizedDirectParticipantEvidenceV1 {
            market,
            claims_market,
            position: Pubkey::new_unique(),
            owner: Pubkey::new_unique(),
            collateral_account: Pubkey::new_unique(),
            collateral_quantity_atoms: 800,
            custody_authority,
            mint,
            token_program,
            admission_signature: "buyer-admission".into(),
            admission_slot: 12,
            collateral_signature: "buyer-collateral".into(),
            collateral_slot: 13,
        };
        let source = AuthenticatedDevnetDirectSessionSourceV1 {
            public_manifest_sha256: "11".repeat(32),
            plan_sha256: "22".repeat(32),
            market_input_sha256: "33".repeat(32),
            checked_execution_release_sha256: "44".repeat(32),
            payer: Pubkey::new_unique(),
            market,
            seller: seller.owner,
            buyer: buyer.owner,
            claims_market,
            seller_position: seller.position,
            buyer_position: buyer.position,
            seller_collateral: seller.collateral_account,
            buyer_collateral: buyer.collateral_account,
            custody_authority,
            mint,
            token_program,
            seller_replay: Pubkey::new_unique(),
            buyer_replay: Pubkey::new_unique(),
            seller_nonce: 4,
            buyer_nonce: 5,
            seller_ticket_sha256: "55".repeat(32),
            buyer_ticket_sha256: "66".repeat(32),
            checked_binaries: Default::default(),
        };
        (source, seller, buyer)
    }

    #[test]
    fn devnet_session_pair_joins_both_finalized_participants_to_one_route() {
        let (source, seller, buyer) = devnet_pair();
        assert!(authenticate_devnet_direct_participant_pair_v1(&source, &seller, &buyer).is_ok());
        let mut hostile = buyer.clone();
        hostile.position = seller.position;
        assert!(
            authenticate_devnet_direct_participant_pair_v1(&source, &seller, &hostile).is_err()
        );
        let mut hostile = buyer.clone();
        hostile.collateral_account = Pubkey::new_unique();
        assert!(
            authenticate_devnet_direct_participant_pair_v1(&source, &seller, &hostile).is_err()
        );
        let mut hostile = buyer;
        hostile.collateral_slot = hostile.admission_slot.saturating_sub(1);
        assert!(
            authenticate_devnet_direct_participant_pair_v1(&source, &seller, &hostile).is_err()
        );
    }

    fn devnet_journal() -> DevnetDirectSessionProducerJournalV1 {
        let participant = |name: &str, nonce| DevnetDirectParticipantSourceV1 {
            report: format!("/tmp/{name}.json"),
            report_sha256: "77".repeat(32),
            owner: Pubkey::new_unique().to_string(),
            position: Pubkey::new_unique().to_string(),
            collateral: Pubkey::new_unique().to_string(),
            collateral_quantity_atoms: 9,
            replay: Pubkey::new_unique().to_string(),
            nonce,
            admission_signature: format!("{name}-admission"),
            admission_slot: 7,
            collateral_signature: format!("{name}-collateral"),
            collateral_slot: 8,
        };
        let mut journal = DevnetDirectSessionProducerJournalV1 {
            schema: DEVNET_SESSION_PRODUCER_JOURNAL_SCHEMA_V1.into(),
            phase: DevnetDirectSessionProducerPhaseV1::Prepared,
            cluster: "devnet".into(),
            genesis_hash: DEVNET_GENESIS_HASH.into(),
            public_manifest: "/tmp/public.json".into(),
            public_manifest_sha256: "11".repeat(32),
            plan: "/tmp/plan.json".into(),
            plan_sha256: "22".repeat(32),
            market_input: "/tmp/market.json".into(),
            market_input_sha256: "33".repeat(32),
            checked_execution_release_sha256: "44".repeat(32),
            checked_binaries: Default::default(),
            payer: Pubkey::new_unique().to_string(),
            payer_keypair: "/tmp/runtime-payer.json".into(),
            seller: participant("seller", 4),
            buyer: participant("buyer", 5),
            seller_ticket_sha256: "55".repeat(32),
            buyer_ticket_sha256: "66".repeat(32),
            journal_dir: "/tmp/direct-journal".into(),
            evidence_file: "/tmp/direct-finalized.json".into(),
            private_session: "/tmp/direct-session.json".into(),
            private_session_sha256: None,
            previous_state_sha256: None,
            state_sha256: String::new(),
        };
        journal.state_sha256 = devnet_session_producer_state_sha256_v1(&journal).expect("digest");
        journal
    }

    #[test]
    fn devnet_session_recovery_accepts_only_the_exact_prepared_predecessor() {
        let prepared = devnet_journal();
        assert!(authenticate_devnet_session_producer_recovery_v1(&prepared, &prepared).is_ok());
        let mut finalized = prepared.clone();
        finalized.phase = DevnetDirectSessionProducerPhaseV1::Finalized;
        finalized.private_session_sha256 = Some("88".repeat(32));
        finalized.previous_state_sha256 = Some(prepared.state_sha256.clone());
        finalized.state_sha256 =
            devnet_session_producer_state_sha256_v1(&finalized).expect("digest");
        assert!(authenticate_devnet_session_producer_recovery_v1(&prepared, &finalized).is_ok());
        for mutate in [
            |value: &mut DevnetDirectSessionProducerJournalV1| {
                value.seller.report_sha256 = "99".repeat(32);
            },
            |value: &mut DevnetDirectSessionProducerJournalV1| {
                value.buyer_ticket_sha256 = "aa".repeat(32);
            },
            |value: &mut DevnetDirectSessionProducerJournalV1| {
                value.payer_keypair.push('x');
            },
        ] {
            let mut hostile = prepared.clone();
            mutate(&mut hostile);
            hostile.state_sha256 =
                devnet_session_producer_state_sha256_v1(&hostile).expect("digest");
            assert!(authenticate_devnet_session_producer_recovery_v1(&prepared, &hostile).is_err());
        }
    }

    #[test]
    fn devnet_session_parser_requires_exact_ack_and_refuses_unknown_flags() {
        assert!(
            parse_devnet_session_arguments_v1(vec![
                DEVNET_ACKNOWLEDGMENT_FLAG.into(),
                "another-genesis".into(),
            ])
            .is_err()
        );
        assert!(
            parse_devnet_session_arguments_v1(vec!["--secret-key".into(), "x".into()]).is_err()
        );
    }

    #[test]
    fn devnet_session_usage_exposes_only_runtime_paths_and_exact_acknowledgment() {
        let usage = devnet_session_usage();
        assert!(usage.starts_with(
            "dclutch-local-successor-bootstrap devnet-direct-trade-session-produce-v1 "
        ));
        for flag in [
            DEVNET_ACKNOWLEDGMENT_FLAG,
            "--public-manifest",
            "--seller-participant",
            "--buyer-participant",
            "--payer-keypair",
            "--producer-journal",
            "--session",
        ] {
            assert!(usage.contains(flag), "usage omitted {flag}");
        }
        assert!(usage.contains(DEVNET_GENESIS_HASH));
        for forbidden in ["--seller-keypair", "--buyer-keypair", "--secret-key"] {
            assert!(!usage.contains(forbidden), "usage exposed {forbidden}");
        }
    }

    /// The real writer, not a test-only restatement of it: this is exactly the
    /// text `direct-intent-ticket-author-v1` writes and the browser panel emits.
    fn portable_ticket_bytes(
        signed: &dclutch_operator::direct_inline_v3::SignedDirectIntentV3,
    ) -> Vec<u8> {
        encode_portable_direct_ticket_v1(signed)
            .expect("ticket")
            .into_bytes()
    }

    #[test]
    fn portable_tickets_reopen_exact_sdk_wire_and_form_one_crossing() -> crate::Result<()> {
        let market = Pubkey::new_unique();
        let seller_key = Keypair::new();
        let buyer_key = Keypair::new();
        let intent = |side, collateral| CompactIntentV2 {
            side,
            lifecycle: 0,
            outcome: 3,
            market: market.to_bytes(),
            generation: 7,
            nonce: 9 + u64::from(side),
            valid_from: 11,
            valid_through: 22,
            maximum_fill: 100,
            limit_price: 500_000,
            fee_basis_points: 50,
            collateral_account: collateral,
        };
        let seller = signed_intent_v1(&seller_key, intent(0, Pubkey::new_unique().to_bytes()))?;
        let buyer = signed_intent_v1(&buyer_key, intent(1, Pubkey::new_unique().to_bytes()))?;
        let decoded_seller =
            parse_portable_direct_ticket_v1(&portable_ticket_bytes(&seller), "seller")?;
        let decoded_buyer =
            parse_portable_direct_ticket_v1(&portable_ticket_bytes(&buyer), "buyer")?;
        assert_eq!(decoded_seller, seller);
        assert_eq!(decoded_buyer, buyer);
        let terms = exact_ticket_pair_terms_v1(&decoded_seller, &decoded_buyer)?;
        assert_eq!(terms.outcome, 3);
        assert_eq!(terms.fill, 100);
        assert_eq!(terms.execution_price, 500_000);
        Ok(())
    }

    #[test]
    fn portable_ticket_refuses_unknown_noncanonical_and_changed_signature_fields() {
        let key = Keypair::new();
        let signed = signed_intent_v1(
            &key,
            CompactIntentV2 {
                side: 0,
                lifecycle: 0,
                outcome: 0,
                market: Pubkey::new_unique().to_bytes(),
                generation: 1,
                nonce: 2,
                valid_from: 3,
                valid_through: 4,
                maximum_fill: 5,
                limit_price: 6,
                fee_basis_points: 7,
                collateral_account: Pubkey::new_unique().to_bytes(),
            },
        )
        .expect("signed");
        let bytes = portable_ticket_bytes(&signed);
        let mut unknown: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        unknown.as_object_mut().expect("object").insert(
            "authority".into(),
            serde_json::json!(key.pubkey().to_string()),
        );
        assert!(
            parse_portable_direct_ticket_v1(
                &serde_json::to_vec(&unknown).expect("unknown"),
                "seller"
            )
            .is_err()
        );
        let mut noncanonical: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        noncanonical["intent"]["generation"] = serde_json::json!("01");
        assert!(
            parse_portable_direct_ticket_v1(
                &serde_json::to_vec(&noncanonical).expect("noncanonical"),
                "seller"
            )
            .is_err()
        );
        let mut changed: serde_json::Value = serde_json::from_slice(&bytes).expect("json");
        changed["intent"]["nonce"] = serde_json::json!("3");
        assert!(
            parse_portable_direct_ticket_v1(
                &serde_json::to_vec(&changed).expect("changed"),
                "seller"
            )
            .is_err()
        );
    }

    #[test]
    fn devnet_full_producer_parser_and_usage_expose_no_maker_secret_paths() {
        assert!(
            parse_devnet_direct_arguments_v1(vec![
                DEVNET_ACKNOWLEDGMENT_FLAG.into(),
                "another-genesis".into(),
            ])
            .is_err()
        );
        assert!(
            parse_devnet_direct_arguments_v1(vec!["--seller-keypair".into(), "x".into()]).is_err()
        );
        let usage = devnet_direct_usage();
        assert!(usage.contains(DEVNET_DIRECT_PRODUCER_COMMAND_V1));
        for required in [
            "--campaign-report",
            "--buyer-participant",
            "--checked-execution-release",
            "--seller-ticket",
            "--buyer-ticket",
            "--payer-keypair",
            "--output-dir",
        ] {
            assert!(usage.contains(required), "usage omitted {required}");
        }
        for forbidden in ["--seller-keypair", "--buyer-keypair", "--secret-key"] {
            assert!(!usage.contains(forbidden), "usage exposed {forbidden}");
        }
    }

    /// Both prestates the chain admits are admitted, and every clause of the
    /// third thing is proven red by mutating exactly one field away from a
    /// destination that would otherwise pass.
    ///
    /// This is the red-proof for wall 7. The producer used to admit vacancy
    /// alone, so a market that had run token setup once could never trade
    /// again; the failure it produced named a "System-owned data-empty PDA
    /// prestate", which describes the prestate of the instruction that CREATES
    /// the account and says nothing about the one that PAYS it.
    #[test]
    fn both_admitted_token_destination_prestates_and_every_refusing_clause() {
        let mint = key_v1("2xGo6Cxtfb41HJCrrqWBf73TSaJrbjUmFqr2urDi91q8");
        let owner = Pubkey::new_unique();
        let token_program = Pubkey::new_unique();
        let initialized = TokenAccount::initialized_base_bytes(mint.to_bytes(), owner.to_bytes())
            .expect("initialized base");

        // A MISSING account is the vacant prestate: the runtime renders it that
        // way to the instruction that creates the PDA.
        let (prestate, clauses) =
            classify_direct_token_destination_v1(None, mint, owner, token_program);
        assert_eq!(prestate, Some(DirectTokenDestinationPrestateV1::Vacant));
        assert!(clauses.is_empty(), "{clauses:?}");

        let vacant = RpcAccount {
            lamports: 0,
            owner: system_program::ID,
            executable: false,
            rent_epoch: 0,
            data: Vec::new(),
        };
        let (prestate, clauses) =
            classify_direct_token_destination_v1(Some(&vacant), mint, owner, token_program);
        assert_eq!(prestate, Some(DirectTokenDestinationPrestateV1::Vacant));
        assert!(clauses.is_empty(), "{clauses:?}");

        let good = RpcAccount {
            lamports: 2_039_280,
            owner: token_program,
            executable: false,
            rent_epoch: 0,
            data: initialized.to_vec(),
        };
        let (prestate, clauses) =
            classify_direct_token_destination_v1(Some(&good), mint, owner, token_program);
        assert_eq!(
            prestate,
            Some(DirectTokenDestinationPrestateV1::Initialized),
            "{clauses:?}"
        );
        assert!(clauses.is_empty(), "{clauses:?}");

        // THE BALANCE IS NOT A CLAUSE, and this is the assertion that keeps it
        // from becoming one. `direct_token_setup_v1`'s poststate is the ZERO
        // balance of `initialized_base_bytes`, and mirroring that here would
        // admit a destination only until the first time it was paid -- a market
        // that could trade exactly twice instead of exactly once. The trade
        // reads this balance as an observation, never as a requirement.
        let mut paid = good.clone();
        paid.data = TokenAccount::project_amount_poststate(&initialized, 50_000_000)
            .expect("paid seller destination")
            .to_vec();
        let (prestate, clauses) =
            classify_direct_token_destination_v1(Some(&paid), mint, owner, token_program);
        assert_eq!(
            prestate,
            Some(DirectTokenDestinationPrestateV1::Initialized),
            "a seller destination that has been paid once is still payable: {clauses:?}"
        );

        // Nor are the lamports: that clause belongs to token setup's rent
        // normalization, and the trade does not read this account's lamports.
        let mut underfunded = good.clone();
        underfunded.lamports = 1;
        let (prestate, _) =
            classify_direct_token_destination_v1(Some(&underfunded), mint, owner, token_program);
        assert_eq!(
            prestate,
            Some(DirectTokenDestinationPrestateV1::Initialized)
        );

        // Each mutation below moves exactly one field and must refuse by name.
        let cases: [(&str, Box<dyn Fn(&mut RpcAccount)>, &str); 5] = [
            (
                "a stranger owns it",
                Box::new(|account: &mut RpcAccount| account.owner = Pubkey::new_unique()),
                "Realm token program",
            ),
            (
                "it is executable",
                Box::new(|account: &mut RpcAccount| account.executable = true),
                "executable",
            ),
            (
                "it is the wrong length",
                Box::new(|account: &mut RpcAccount| account.data.truncate(ACCOUNT_BYTES - 1)),
                "bytes rather than the",
            ),
            (
                "it is frozen rather than initialized",
                Box::new(|account: &mut RpcAccount| {
                    account.data[TokenAccountLayoutV1::STATE] = 2;
                }),
                "rather than Initialized",
            ),
            (
                "it carries a native reserve",
                Box::new(|account: &mut RpcAccount| {
                    account.data[TokenAccountLayoutV1::NATIVE_RESERVE] = 1;
                }),
                "native reserve",
            ),
        ];
        for (label, mutate, expected) in cases {
            let mut hostile = good.clone();
            mutate(&mut hostile);
            let (prestate, clauses) =
                classify_direct_token_destination_v1(Some(&hostile), mint, owner, token_program);
            assert_eq!(prestate, None, "{label} was admitted");
            assert!(
                clauses.iter().any(|clause| clause.contains(expected)),
                "{label} refused without naming {expected}: {clauses:?}"
            );
        }

        // The Mint and the token owner are the two identities that make this
        // THIS trade's destination rather than some other market's.
        let other = key_v1("6wLYToyGCRNa39Hjph9L4zgCPE4mjr2wKiHLQSyBDKaK");
        let (prestate, clauses) =
            classify_direct_token_destination_v1(Some(&good), other, owner, token_program);
        assert_eq!(prestate, None);
        assert!(
            clauses
                .iter()
                .any(|clause| clause.contains("collateral Mint")),
            "{clauses:?}"
        );
        let (prestate, clauses) = classify_direct_token_destination_v1(
            Some(&good),
            mint,
            Pubkey::new_unique(),
            token_program,
        );
        assert_eq!(prestate, None);
        assert!(
            clauses.iter().any(|clause| clause.contains("token owner")),
            "{clauses:?}"
        );

        // ALL failing clauses, not the first: an operator who fixes one and
        // re-runs has learned almost nothing.
        let mut doubly = good.clone();
        doubly.executable = true;
        doubly.data[TokenAccountLayoutV1::STATE] = 2;
        let (_, clauses) =
            classify_direct_token_destination_v1(Some(&doubly), mint, owner, token_program);
        assert!(clauses.len() >= 2, "only one clause reported: {clauses:?}");
    }
}
