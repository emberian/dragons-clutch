//! Typed producer for one owned-loopback ordinary Direct session.
//!
//! This module consumes the campaign and participant parsers owned by their
//! respective exteriors. It never parses either report into a parallel DTO.
//! All public coordinates are derived before any private key file is opened;
//! key access is the final step and is used only to prove the four expected
//! identities and sign the two exact `CompactIntentV2` preimages.

use std::{
    fs::{self, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
    v4::SCHEMA_RELEASE_ID as CAPABILITY_PROGRAM_SCHEMA_ID_V4,
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
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest as _, Sha256};
use solana_program::{hash::hash, rent::Rent};
use solana_sdk::{
    pubkey::Pubkey,
    signature::{Keypair, Signer as _},
};
use solana_sdk_ids::{system_program, sysvar};

use crate::{
    Error, Result, campaign,
    cluster::{ClusterOriginV1, ExpectedClusterV1},
    local_mutable,
    model::{MarketRunInput, RecordPair, SuccessorPlan},
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

#[derive(Clone)]
struct PreparedPublicFactsV1 {
    plan_sha256: String,
    market_sha256: String,
    campaign_sha256: String,
    participant_sha256: String,
    genesis_hash: String,
    market: Pubkey,
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
    )?;

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
        generation: market_input.generation,
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
        generation: market_input.generation,
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
    let context = dclutch_direct_codec::ordinary_v3::DirectOrdinaryAuthenticatedContextV3 {
        parent_request_digest: hash(
            &compile_direct_inline_request_v3(
                seller_signed,
                buyer_signed,
                FILL_ATOMS_V1,
                EXECUTION_PRICE_V1,
            )
            .map_err(|error| Error::new(format!("Direct request: {error:?}")))?,
        )
        .to_bytes(),
        config_content_id: hash(&public.config_bytes).to_bytes(),
        config: public.config,
        market: public.market.to_bytes(),
        generation: market_input.generation,
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
        FILL_ATOMS_V1,
        EXECUTION_PRICE_V1,
        context,
        &public.account_profile_bytes,
        &public.transition_bytes,
        &public.effect_bytes,
    )
    .map_err(|error| Error::new(format!("Direct child authority projection: {error:?}")))?;
    let mut route = public.route_without_children;
    route.claims.caller_authority = child.claims_authority.to_string();
    route.custody.caller_authorities = child.custody_authorities.map(|key| key.to_string());

    let token_setup = public.token_setup.clone();
    let public_manifest = ProducedDirectTradePublicManifestV1 {
        schema: OWNED_PUBLIC_MANIFEST_SCHEMA_V1.into(),
        cluster: ExpectedClusterV1::OwnedLoopback.evidence_label().into(),
        genesis_hash: public.genesis_hash,
        plan_sha256: public.plan_sha256.clone(),
        market_input_sha256: public.market_sha256.clone(),
        market: public.market.to_string(),
        payer: public.payer.to_string(),
        fill: FILL_ATOMS_V1,
        execution_price: EXECUTION_PRICE_V1,
        fee_basis_points: FEE_BASIS_POINTS_V1,
        fee_recipient: public.fee_recipient.to_string(),
        checked_execution_release_set_base64: BASE64.encode(&public.checked_release),
        seller: signed_manifest_v1(seller_signed)?,
        buyer: signed_manifest_v1(buyer_signed)?,
        route,
        context: ProducedDirectContextHintsV1 {
            generation: market_input.generation,
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
        token_setup: token_setup.clone(),
    };
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

    let receipt = OwnedLoopbackDirectProducerReceiptV1 {
        schema: OWNED_PRODUCER_RECEIPT_SCHEMA_V1.into(),
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
    };
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
) -> Result<PreparedPublicFactsV1> {
    local_mutable::authenticate_checked_local_mutable_plan_v1(plan)?;
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
    let root = campaign_address_v1(campaign, "direct_capability_root")?;
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
    let payer = plan
        .core
        .upgrade_authority
        .as_deref()
        .ok_or_else(|| refusal("owned-loopback Direct plan revoked its retained payer"))
        .and_then(|value| pubkey(value))?;
    let registry = pubkey(&plan.registry.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let claims = pubkey(&plan.claims.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let release_set = hex32(&plan.release_set_id)?;

    let config_bytes = decode_hex_v1(&direct.execution_config_hex, "Direct execution config")?;
    require_market_body_matches_record_v1(
        plan,
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
            campaign,
            label,
            &decode_hex_v1(body_hex, label)?,
        )?;
    }
    let config_digest = hash(&config_bytes).to_bytes();
    let config =
        DirectExecutionConfigV1::decode_selected(config_digest, config_digest, &config_bytes)
            .map_err(|error| Error::new(format!("Direct execution config: {error:?}")))?;
    if config.fee_basis_points() != FEE_BASIS_POINTS_V1
        || config.price_scale() != EXPECTED_PRICE_SCALE_V1
    {
        return Err(refusal(
            "owned-loopback Direct producer requires the exact 1,000,000 scale and 50-bps config",
        ));
    }
    let fee_recipient = Pubkey::new_from_array(config.fee_recipient());
    let checked_release = local_mutable::checked_execution_release_set_bytes_v1(plan)?.to_vec();

    let snapshot = finalized_snapshot(
        rpc,
        &[
            market,
            root,
            aggregate,
            seller_position,
            participant.position,
            mint,
            lifecycle_rent_credit,
            sysvar::rent::ID,
        ],
    )?;
    let market_account = snapshot.account(market)?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Direct Market state: {error:?}")))?;
    if market_account.owner != pubkey(&plan.core.program_id)?
        || market_account.executable
        || market_state.phase != CorePhase::Open
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
        || market_state.identity.generation != market_input.generation
        || market_state.identity.selected_release_set.to_bytes() != release_set
    {
        return Err(refusal(
            "Direct producer requires the exact finalized Open founding Market",
        ));
    }
    let market_rent_beneficiary = Pubkey::new_from_array(market_state.rent_beneficiary.to_bytes());
    let root_account = snapshot.account(root)?;
    if root_account.owner != trading
        || root_account.executable
        || root_account.data.len()
            != CAPABILITY_ROOT_HEADER_BYTES_V1
                .checked_add(dclutch_direct_codec::successor::DIRECT_ROOT_STATE_BYTES_V1)
                .ok_or_else(|| refusal("Direct root width overflowed"))?
    {
        return Err(refusal("Direct root owner or width changed"));
    }
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
        || root_header.generation() != market_input.generation
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
        || aggregate_view.generation != market_input.generation
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
    let outcome = first_funded_outcome_v1(
        seller_position_view,
        &seller_position_account.data,
        FILL_ATOMS_V1,
    )?;
    let gross = exact_quote_v1(FILL_ATOMS_V1, EXECUTION_PRICE_V1, config.price_scale())?;
    let fee = fee_floor_v1(gross, FEE_BASIS_POINTS_V1)?;
    let required_buyer_collateral = gross
        .checked_add(fee)
        .ok_or_else(|| refusal("Direct buyer reserve overflowed"))?;
    if participant.collateral_quantity_atoms < required_buyer_collateral {
        return Err(refusal(format!(
            "Direct participant has {} collateral atoms; this exact trade requires {required_buyer_collateral}",
            participant.collateral_quantity_atoms
        )));
    }
    let rent_account = snapshot.account(sysvar::rent::ID)?;
    let rent: Rent = bincode::deserialize(&rent_account.data)
        .map_err(|error| Error::new(format!("Direct Rent sysvar: {error}")))?;
    let coordinates = DirectCoordinatesV1::new(market.to_bytes(), market_input.generation)
        .map_err(|error| Error::new(format!("Direct coordinates: {error:?}")))?;
    let seller_seeds = MakerReplaySeedsV1::new(coordinates, seller.to_bytes())
        .map_err(|error| Error::new(format!("seller maker seeds: {error:?}")))?;
    let buyer_seeds = MakerReplaySeedsV1::new(coordinates, participant.owner.to_bytes())
        .map_err(|error| Error::new(format!("buyer maker seeds: {error:?}")))?;
    let (seller_maker, seller_bump) =
        Pubkey::find_program_address(&seller_seeds.as_slices(), &trading);
    let (buyer_maker, buyer_bump) =
        Pubkey::find_program_address(&buyer_seeds.as_slices(), &trading);
    let seller_facts = maker_facts_v1(
        rpc,
        seller_maker,
        seller_bump,
        trading,
        market,
        market_input.generation,
        seller,
        payer,
        &rent,
    )?;
    let buyer_facts = maker_facts_v1(
        rpc,
        buyer_maker,
        buyer_bump,
        trading,
        market,
        market_input.generation,
        participant.owner,
        payer,
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
        market_input.generation,
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
        market_input.generation,
        seller.to_bytes(),
        DirectTokenAccountRoleV1::Seller,
    )
    .map_err(|error| Error::new(format!("seller Direct token seeds: {error:?}")))?;
    let fee_token_seeds = DirectTokenAccountSeedsV1::new(
        market.to_bytes(),
        market_input.generation,
        fee_recipient.to_bytes(),
        DirectTokenAccountRoleV1::Fee,
    )
    .map_err(|error| Error::new(format!("fee Direct token seeds: {error:?}")))?;
    let seller_token = Pubkey::find_program_address(&seller_token_seeds.as_slices(), &trading).0;
    let fee_token = Pubkey::find_program_address(&fee_token_seeds.as_slices(), &trading).0;
    require_distinct_v1(&[
        seller,
        participant.owner,
        payer,
        fee_recipient,
        seller_token,
        fee_token,
        participant.collateral_account,
        seller_maker,
        buyer_maker,
        custody_replay,
        custody_authority,
        market_rent_beneficiary,
    ])?;
    for (address, label) in [
        (seller_token, "seller Token-2022 destination"),
        (fee_token, "fee Token-2022 destination"),
    ] {
        if rpc.account(address)?.is_some_and(|account| {
            account.owner != system_program::ID || account.executable || !account.data.is_empty()
        }) {
            return Err(refusal(format!(
                "{label} was not a System-owned data-empty PDA prestate"
            )));
        }
    }

    let descriptor = record_coordinates_v1(plan, campaign, "direct_ordinary_descriptor_record")?;
    let descriptor_pair = required_record_v1(plan, "direct_ordinary_descriptor_record")?;
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
            manifest: record_coordinates_v1(plan, campaign, "capability_manifest_record")?,
            program_set: record_coordinates_v1(plan, campaign, "direct_program_set_record")?,
            descriptor,
            config: record_coordinates_v1(plan, campaign, "direct_execution_config_record")?,
            account_profile: record_coordinates_v1(
                plan,
                campaign,
                "direct_ordinary_account_profile_record",
            )?,
            request_profile: record_coordinates_v1(
                plan,
                campaign,
                "direct_ordinary_request_profile_record",
            )?,
            transition: record_coordinates_v1(plan, campaign, "direct_ordinary_transition_record")?,
            effect: record_coordinates_v1(plan, campaign, "direct_ordinary_effect_record")?,
            lifecycle: record_coordinates_v1(
                plan,
                campaign,
                "direct_ordinary_lifecycle_policy_record",
            )?,
            strategy: record_coordinates_v1(plan, campaign, "direct_ordinary_strategy_record")?,
            activation_cache: plan.activation.clone(),
            core_program: plan.core.program_id.clone(),
            core_programdata: plan.core.programdata_id.clone(),
            trading_program: plan.trading.program_id.clone(),
            trading_programdata: plan.trading.programdata_id.clone(),
            registry_program: plan.registry.program_id.clone(),
            product: record_coordinates_v1(plan, campaign, "product_record")?,
            result_domain: record_coordinates_v1(plan, campaign, "result_domain_record")?,
            portfolio: record_coordinates_v1(plan, campaign, "portfolio_record")?,
            linked_basis: record_coordinates_v1(plan, campaign, "linked_liability_basis_record")?,
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
            realm: record_coordinates_v1(plan, campaign, "realm_record")?,
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
        generation: market_input.generation,
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
        generation: market_input.generation,
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
    let account_profile_bytes =
        record_body_v1(plan, campaign, "direct_ordinary_account_profile_record")?;
    let transition_bytes = record_body_v1(plan, campaign, "direct_ordinary_transition_record")?;
    let effect_bytes = record_body_v1(plan, campaign, "direct_ordinary_effect_record")?;
    let product_digest = record_digest_v1(plan, campaign, "product_record")?;
    let realm_digest = record_digest_v1(plan, campaign, "realm_record")?;
    let linked_basis_digest = record_digest_v1(plan, campaign, "linked_liability_basis_record")?;
    let linked_basis_semantic = crate::market::semantic_basis_identity_v3(&decode_hex_v1(
        &market_input.linked_basis_hex,
        "linked liability basis",
    )?)?;
    if product_digest != aggregate_view.product_instance_id
        || realm_digest != aggregate_view.realm_id
        || linked_basis_semantic != hex32(&market_input.liability_basis_id)?
        || aggregate_view.basis_id != linked_basis_semantic
    {
        return Err(refusal(
            "Direct Product/Realm/semantic-basis/linked-basis closure changed",
        ));
    }
    let genesis_hash = rpc
        .call("getGenesisHash", &json!([]))?
        .as_str()
        .ok_or_else(|| refusal("owned-loopback genesis hash was not a string"))?
        .to_owned();
    Ok(PreparedPublicFactsV1 {
        plan_sha256,
        market_sha256,
        campaign_sha256,
        participant_sha256,
        genesis_hash,
        market,
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
fn maker_facts_v1(
    rpc: &mut Rpc,
    address: Pubkey,
    bump: u8,
    trading: Pubkey,
    market: Pubkey,
    generation: u64,
    maker: Pubkey,
    payer: Pubkey,
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
            // Current Direct operator first-use semantics assign maker-root rent
            // to the authenticated payer. If the lifecycle lane moves this to
            // RentCredit, this is the single producer coordinate to change.
            rent_beneficiary: payer,
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
            rent_beneficiary: payer,
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

fn signed_intent_v1(keypair: &Keypair, intent: CompactIntentV2) -> Result<SignedDirectIntentV3> {
    let preimage = intent
        .signed_preimage()
        .map_err(|error| Error::new(format!("Direct intent preimage: {error:?}")))?;
    let signature = keypair.sign_message(&preimage);
    if !signature.verify(keypair.pubkey().as_ref(), &preimage) {
        return Err(refusal("fresh Direct signature did not verify"));
    }
    Ok(SignedDirectIntentV3 {
        maker: keypair.pubkey(),
        signature: signature.as_ref().try_into().map_err(|_| {
            refusal("fresh Direct Ed25519 signature did not have the exact 64-byte width")
        })?,
        intent,
    })
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

fn record_coordinates_v1(
    plan: &SuccessorPlan,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<ProducedRecordPairCoordinatesV1> {
    let pair = required_record_v1(plan, label)?;
    authenticate_campaign_record_v1(campaign, label, pair, &plan.registry.program_id)?;
    Ok(ProducedRecordPairCoordinatesV1 {
        raw: pair.raw.clone(),
        staging: pair.staging.clone(),
    })
}

fn record_body_v1(
    plan: &SuccessorPlan,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<Vec<u8>> {
    let pair = required_record_v1(plan, label)?;
    authenticate_campaign_record_v1(campaign, label, pair, &plan.registry.program_id)?;
    let body = decode_hex_v1(&pair.body_hex, label)?;
    if sha256_hex(&body) != pair.content_sha256 {
        return Err(refusal(format!("checked plan {label} body digest changed")));
    }
    Ok(body)
}

fn record_digest_v1(
    plan: &SuccessorPlan,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
) -> Result<[u8; 32]> {
    let pair = required_record_v1(plan, label)?;
    authenticate_campaign_record_v1(campaign, label, pair, &plan.registry.program_id)?;
    hex32(&pair.content_sha256)
}

fn require_market_body_matches_record_v1(
    plan: &SuccessorPlan,
    campaign: &campaign::CampaignTerminalEvidenceV1,
    label: &str,
    bytes: &[u8],
) -> Result<()> {
    if record_body_v1(plan, campaign, label)? != bytes {
        return Err(refusal(format!(
            "Direct Market input and checked plan {label} bodies differ"
        )));
    }
    Ok(())
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

fn required_record_v1<'a>(plan: &'a SuccessorPlan, label: &str) -> Result<&'a RecordPair> {
    plan.records
        .get(label)
        .ok_or_else(|| refusal(format!("checked plan omitted {label}")))
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

fn require_distinct_v1(keys: &[Pubkey]) -> Result<()> {
    if keys.iter().any(|key| *key == Pubkey::default())
        || keys
            .iter()
            .enumerate()
            .any(|(index, key)| keys.iter().skip(index + 1).any(|other| other == key))
    {
        return Err(refusal("Direct participant/setup coordinates alias"));
    }
    Ok(())
}

fn read_keypair_v1(path: &Path, label: &str) -> Result<Keypair> {
    Ok(Keypair::new_from_array(campaign::read_keypair_file(
        path, label,
    )?))
}

fn private_session_sha256_v1(session: &ProducedDirectTradePrivateSessionV1) -> Result<String> {
    let mut canonical = session.clone();
    canonical.session_sha256.clear();
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

fn decode_hex_v1(value: &str, label: &str) -> Result<Vec<u8>> {
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

    use super::{
        CAPABILITY_SEAL_PDA_DOMAIN_V1, EXECUTION_PRICE_V1, FEE_BASIS_POINTS_V1, FILL_ATOMS_V1,
        ProducedDirectTradePrivateSessionV1, derive_capability_seal_v1, exact_quote_v1,
        fee_floor_v1, private_session_sha256_v1, require_distinct_v1, signed_intent_v1,
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
    fn aliases_and_zero_coordinates_are_refused() {
        let a = Pubkey::new_unique();
        let b = Pubkey::new_unique();
        assert!(require_distinct_v1(&[a, b]).is_ok());
        assert!(require_distinct_v1(&[a, a]).is_err());
        assert!(require_distinct_v1(&[a, Pubkey::default()]).is_err());
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
}
