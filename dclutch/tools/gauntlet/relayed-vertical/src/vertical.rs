//! The vertical itself: found → observe → seal → consume → terminalize, and
//! the silent-relayer sibling that pays a walker.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use sha2::{Digest as _, Sha256};
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};

use dclutch_capability_contract::{
    CapabilityFundingLedgerDerivationV2, CapabilityManifestV1, ContentId as CapabilityContentId,
    FundingLedgerStatusV2, FundingLedgerV2, derive_funded_rent_rate_v2, funding_ledger_bytes_v2,
};
use dclutch_market_core_codec::{CoreState, Phase};
use dclutch_product_runtime_v2::ResultDomainV2;
use dclutch_product_runtime_v2_admission::{
    PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_SCHEMA_ID_V2, RESULT_DOMAIN_SCHEMA_ID_V2,
};
use dclutch_registry_contract::ARTIFACT_RELEASE_SCHEMA_ID_V1;
use dclutch_relay_contract::{
    RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1, RELAYED_FAMILY_RELEASE_ID_V1,
    RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
    record::{RelayedObservationRecordViewV1, RelayedRecordPhaseV1},
};
use dclutch_resolution_codec::{RESOLUTION_CONTROLLER_RELEASE_ID_V7, ResolutionCertificateKindV2};
use dclutch_resolution_core_v3_operator::{
    ObservedAccount, ResolutionAdmitTerminalSnapshotV3, build_resolution_admit_terminal_v3,
    validate_resolution_admit_terminal_report_v3,
};
use dclutch_source_contract::{
    MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, PROVIDER_RELEASE_SCHEMA_ID_V1,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
    SOURCE_SPEC_SCHEMA_ID_V1, STATISTIC_SPEC_SCHEMA_ID_V1, SourceMaterialV3,
    SourcePrincipalPolicyV1, SourceResolutionPhaseV1, WINDOW_SPEC_SCHEMA_ID_V1,
};
use solana_sdk_ids::{system_program, sysvar};

use crate::daemon;
use crate::funding_readiness::{
    FundingReadinessCoordinatesV1, FundingReadinessPlanV1, FundingReadinessRecordCoordinatesV1,
    plan_funding_readiness_from_rpc_v1,
};
use crate::input::{
    self, DISCLOSED_FAILURE_CONFLATION, RelayedMarketFactsV1, WALK_BOUNTY_LAMPORTS,
};
use crate::ledger::{ClassClaimV1, ConservationLedgerV1, LamportClaimV1};
use crate::plan::pubkey;
use crate::relayworld::{
    self, RESOLUTION_FAILURE_KIND, RESOLUTION_SUCCESS_KIND, RecordPairV1, RelayAddressBookV1,
};
use crate::rpc::Rpc;
use crate::runtime::publish_record;
use crate::substrate::{self, SubstrateRequestV1};
use crate::twin;
use crate::{Error, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum WalkV1 {
    Success,
    Failure,
}

pub(crate) struct VerticalRequestV1 {
    pub(crate) walk: WalkV1,
    pub(crate) transcript: PathBuf,
    pub(crate) relayer_bin: PathBuf,
    pub(crate) work: PathBuf,
    /// The successor validator's RPC port; the twin allocates around it.
    pub(crate) rpc_port: u16,
    /// The checked release gate the substrate is prepared from, with its
    /// pinned identity — the same four facts the lifecycle driver demands.
    pub(crate) checked_release_gate: PathBuf,
    pub(crate) expected_gate_sha256: String,
    pub(crate) expected_source_revision: String,
    pub(crate) expected_source_tree_sha256: String,
    /// The prepare seed (64 lowercase hex): loopback-only determinism for the
    /// disposable role keys.
    pub(crate) seed: String,
}

/// A live campaign session over the checked-mutable substrate.
///
/// The shape `found_through_open` used to return, rebuilt from the campaign
/// evidence instead: the validator is this driver's own child, the plan is
/// the checked-mutable plan, and the authority is the retained upgrade
/// authority whose key file the prepare stage wrote. Unlike the tier-1
/// session, the keys here ARE persisted (disposable, loopback-only, under the
/// prepare work dir), and the evidence says so instead of claiming otherwise.
pub(crate) struct RelaySessionV1 {
    /// Kept solely to own the child's lifetime; `Drop` kills the validator.
    #[allow(dead_code)]
    pub(crate) validator: substrate::ValidatorGuardV1,
    pub(crate) rpc: Rpc,
    pub(crate) plan: crate::model::SuccessorPlan,
    pub(crate) plan_sha256: String,
    pub(crate) authority: Keypair,
    pub(crate) output: PathBuf,
    pub(crate) transactions: Vec<crate::model::TransactionEvidence>,
    pub(crate) accounts: std::collections::BTreeMap<String, crate::model::AccountEvidence>,
    pub(crate) completed: Vec<String>,
    pub(crate) founding_custody_context: String,
    pub(crate) direct_selected_manifest_entry_index: u16,
}

impl RelaySessionV1 {
    fn evidence_json(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "schema": "dclutch-relayed-vertical-session-evidence-v1",
            "rpc_url": self.rpc.url(),
            "plan_sha256": self.plan_sha256,
            "authority_pubkey": self.authority.pubkey().to_string(),
            "private_key_persisted": true,
            "keypair_derivation": "prepared-disposable-loopback-roles",
            "founding_custody_context": self.founding_custody_context,
            "direct_selected_manifest_entry_index": self.direct_selected_manifest_entry_index,
            "completed": self.completed,
            "transactions": serde_json::to_value(&self.transactions)?,
            "accounts": serde_json::to_value(&self.accounts)?,
        }))
    }
}

#[derive(Serialize)]
struct StageV1 {
    stage: String,
    outcome: String,
    note: String,
}

fn now_unix() -> Result<i64> {
    let elapsed = SystemTime::now().duration_since(UNIX_EPOCH)?;
    i64::try_from(elapsed.as_secs()).map_err(|_| Error::new("wall clock out of range"))
}

/// The whole vertical, in the order the park banner prescribed: prepare the
/// checked substrate, authenticate it live, compile the market against it,
/// found, then relay.
pub(crate) fn execute(request: VerticalRequestV1) -> Result<serde_json::Value> {
    std::fs::create_dir_all(&request.work)?;
    let mut stages: Vec<StageV1> = Vec::new();
    let start_unix = now_unix()?;

    // ---------------------------------------------------------- 1. the twin
    // The successor validator's base is chosen by the runner but not yet
    // bound; the twin's allocator must not take that block.
    let twin = twin::start(&request.work, start_unix, Some(request.rpc_port))?;
    stages.push(StageV1 {
        stage: "mainnet twin".into(),
        outcome: "running".into(),
        note: format!(
            "A loopback validator at {} (genesis {}) carries the synthetic-of-real DBC world: the \
             real mainnet program and ProgramData ADDRESSES and the real 424-byte VirtualPool \
             LAYOUT, with a synthetic ELF tail, deployment slot, upgrade authority and pool. The \
             CS dossier never read those four facts from mainnet and this lane makes no public \
             reads, so every value the campaign pins from here is labelled synthetic-of-real.",
            twin.rpc_url, twin.genesis_hash_base58
        ),
    });

    // ----------------------------------- 2. the checked-mutable substrate
    // prepare-mutable -> spawn -> authenticate -> campaign activation. Only
    // after this does a live loopback deployment exist for the deployment-
    // bound Direct compiler to observe.
    let substrate_dir = request.work.join("substrate");
    std::fs::create_dir_all(&substrate_dir)?;
    let checked = substrate::bring_up(&SubstrateRequestV1 {
        work: &substrate_dir,
        checked_release_gate: &request.checked_release_gate,
        expected_gate_sha256: &request.expected_gate_sha256,
        expected_source_revision: &request.expected_source_revision,
        expected_source_tree_sha256: &request.expected_source_tree_sha256,
        seed: &request.seed,
        rpc_port: request.rpc_port,
    })?;
    stages.push(StageV1 {
        stage: "checked-mutable substrate".into(),
        outcome: "executed".into(),
        note: format!(
            "local-mutable-prepare-v1 derived the seven-role mutable substrate from the checked \
             release gate ({}), a fresh solana-test-validator booted the prepared account \
             directory (no --upgradeable-program, so the retained tag-0 authority survives), the \
             plan re-authenticated against the gate on disk, and the administration campaign \
             published, initialized and activated through the retained authority.",
            request.expected_gate_sha256
        ),
    });

    // ------------------------------------------- 3. keys and the market
    // The relayer's attestation key is FOUNDING CONTENT (the key-set record's
    // digest seeds half the graph), so it is generated before the market
    // compiles and disclosed through the records rather than any wallet store.
    let attestation = Keypair::new();
    let fee_payer = Keypair::new();
    let keys_dir = request.work.join("relayer-keys");
    let attestation_path = daemon::write_keypair_file(&keys_dir, "attestation.json", &attestation)?;
    let fee_payer_path = daemon::write_keypair_file(&keys_dir, "fee-payer.json", &fee_payer)?;

    let registry = pubkey(&checked.plan.registry.program_id)?;
    let window_choice = input::window_choice(start_unix, request.walk == WalkV1::Success);
    // The deployment-bound compiler, observing the LIVE checked substrate —
    // the exact ordering the park banner demanded. The fee recipient is a
    // fresh disposable identity; the scenario's 50 bps mirrors the published
    // graduation fixture.
    let fee_recipient = Keypair::new();
    let direct = crate::direct_market::DirectMarketCompilerOwnedV1::load_local(
        &checked.plan_path,
        &checked.rpc_url,
        registry,
        Some(50),
        Some(fee_recipient.pubkey()),
    )?;
    let facts = input::relayed_market_input(
        registry,
        attestation.pubkey().to_bytes(),
        &window_choice,
        direct.compiler(),
    )?;

    // The graduation wrapper, exactly as the shipped `graduation-market`
    // subcommand emits it; the founding campaign authenticates the whole
    // envelope back into the inner source graph.
    let market_path = request.work.join("market.json");
    let wrapper = serde_json::json!({
        "schema": "dclutch-graduation-market-input-v1",
        "market": facts.input,
        "account_set_id": hex(&facts.account_set_id),
        "relayer_attestation": attestation.pubkey().to_string(),
        "relayer_key_set_hex": hex(&facts.relayer_key_set_bytes),
        "relayer_key_set_digest": hex(&facts.relayer_key_set_digest),
        "venue_release_digest": hex(&facts.venue_release_digest),
        "relayed_adapter_config_digest": hex(&facts.relayed_adapter_config_digest),
        "source_spec_digest": hex(&facts.source_spec_digest),
        "window": {
            "start_unix_seconds": window_choice.start_unix_seconds,
            "end_unix_seconds": window_choice.end_unix_seconds,
            "max_age_seconds": window_choice.max_age_seconds,
        },
        "walk_bounty_lamports": WALK_BOUNTY_LAMPORTS,
        "admitted_principal_atoms": facts.admitted_principal_atoms.to_string(),
        "admitted_principal_cap_atoms": facts.admitted_principal_cap_atoms.to_string(),
        "disclosed_failure_conflation": DISCLOSED_FAILURE_CONFLATION,
    });
    std::fs::write(&market_path, serde_json::to_vec_pretty(&wrapper)?)?;

    // ------------------------------------------------------- 4. the founding
    let mut rpc = Rpc::connect(&checked.rpc_url)?;
    let founding = substrate::found_market(
        &checked,
        &mut rpc,
        &market_path,
        &request.work.join("founding-evidence.json"),
    )?;
    let authority_keypair = substrate::authority_keypair(&checked)?;
    let mut session = RelaySessionV1 {
        validator: checked.validator,
        rpc,
        plan: checked.plan,
        plan_sha256: checked.plan_sha256,
        authority: authority_keypair,
        output: request.work.join("session-evidence.json"),
        transactions: founding.transactions,
        accounts: founding.market.accounts,
        completed: founding.market.completed,
        founding_custody_context: founding.market.founding_custody_context,
        direct_selected_manifest_entry_index: founding.market.direct_selected_manifest_entry_index,
    };
    stages.push(StageV1 {
        stage: "founding through Open".into(),
        outcome: "executed".into(),
        note: format!(
            "campaign --founding-only over the live checked substrate, transaction-only record \
             publication, founding the zero-cut graduation Product with NO recovery policy. {} \
             transactions to here. Disclosed at founding: {DISCLOSED_FAILURE_CONFLATION}",
            session.transactions.len()
        ),
    });

    let evidence_market = session
        .accounts
        .get("founding_market")
        .ok_or_else(|| Error::new("the founding's evidence names no founding_market"))?;
    let market = pubkey(&evidence_market.address)?;
    let mint = pubkey(
        &session
            .accounts
            .get("collateral_mint")
            .ok_or_else(|| Error::new("no collateral_mint evidence"))?
            .address,
    )?;
    let hoard = pubkey(
        &session
            .accounts
            .get("founding_hoard_vault_open")
            .ok_or_else(|| Error::new("no founding_hoard_vault_open evidence"))?
            .address,
    )?;
    let aggregate = pubkey(
        &session
            .accounts
            .get("claims_aggregate")
            .ok_or_else(|| Error::new("no claims_aggregate evidence"))?
            .address,
    )?;

    let core_program = pubkey(&session.plan.core.program_id)?;
    let registry_program = pubkey(&session.plan.registry.program_id)?;
    let resolution_program = pubkey(&session.plan.resolution.program_id)?;
    let activation = pubkey(&session.plan.activation)?;
    let market_state =
        CoreState::decode(&session.rpc.required_account(market, "founded Market")?.data)
            .map_err(|error| Error::new(format!("founded Market: {error:?}")))?;
    let generation = market_state.identity.generation;
    let rent_beneficiary = Pubkey::new_from_array(market_state.rent_beneficiary.to_bytes());
    // The Market's resolution policy is the digest of the source material the
    // campaign PUBLISHED, and that is deliberately not the digest `relayed.rs`
    // compiled. The market compiler rebuilds the manipulation floor with the
    // market's own coordinates, and one of them is the collateral mint --
    // created at run time, unknowable to any compiler. So a market whose floor
    // is bounded can never publish the compiler's `material_digest`, and
    // comparing the two could never hold for this family. Every record
    // identity below is therefore read off the FOUNDED MARKET, and the ties
    // back to what this campaign compiled are made on the fields the market
    // compiler does not rewrite: the source spec, and the absence of a
    // recovery policy.
    let published_material = market_state.identity.resolution_policy.to_bytes();
    // The floor the market PUBLISHED. The compiler proposes a template; the
    // market compiler keeps its basis, derivation release and atoms and
    // replaces the source spec, adapter config and collateral unit with the
    // market's own, so the published record is the only one that exists.
    let published_floor = crate::market::record_identity(
        &session
            .rpc
            .required_account(
                pubkey(
                    &session
                        .accounts
                        .get("manipulation_floor_record")
                        .ok_or_else(|| Error::new("no manipulation_floor_record evidence"))?
                        .address,
                )?,
                "published ManipulationFloorV1 record",
            )?
            .data,
    );

    let founder_wallet = pubkey(
        &session
            .accounts
            .get("collateral_wallet")
            .ok_or_else(|| Error::new("no collateral_wallet evidence"))?
            .address,
    )?;
    let founder_position = pubkey(
        &session
            .accounts
            .get("founder_position")
            .ok_or_else(|| Error::new("no founder_position evidence"))?
            .address,
    )?;
    let mut ledger = ConservationLedgerV1::new(mint, session.authority.pubkey());
    ledger.admit_founding(hoard, aggregate, 1);
    // The whole collateral partition, or L1 reads honest movement as a leak:
    // the founding leaves half the supply in the Hoard and half in the
    // founder's wallet, and the founder's Position carries what the aggregate
    // owes.
    ledger.track_token_account("founder_wallet", founder_wallet);
    ledger.track_position("founder_position", founder_position);
    // L1 is only as total as the set it names, so the rest of the collateral
    // partition is DISCOVERED, exactly as the journey does it: every address
    // the founding recorded is re-read live, and any live Token-2022 account
    // of this Mint joins the partition (the tier-1 campaign leaves collateral
    // in more wallets than the founder's — the abort lane's refund wallet
    // held half the supply on the first executed walk).
    {
        let token_program = Pubkey::new_from_array(dclutch_token_svm::TOKEN_2022_PROGRAM_ID);
        let recorded: Vec<(String, Pubkey)> = session
            .accounts
            .iter()
            .map(|(label, evidence)| Ok((label.clone(), pubkey(&evidence.address)?)))
            .collect::<Result<_>>()?;
        for (label, address) in recorded {
            if address == founder_wallet || address == hoard {
                continue;
            }
            let Some(account) = session.rpc.account(address)? else {
                continue;
            };
            let is_collateral = account.owner == token_program
                && dclutch_token_svm::TokenAccount::parse(&account.data)
                    .map(|parsed| parsed.mint == mint.to_bytes())
                    .unwrap_or(false);
            if is_collateral {
                ledger.track_token_account(&label, address);
            }
        }
    }

    // Keep the campaign payer comfortably funded for the publications and the
    // prepaid outputs; this is fee-side lamports, not market collateral.
    session.transactions.push(session.rpc.airdrop(
        "relayed vertical: top up the campaign payer",
        session.authority.pubkey(),
        10_000_000_000,
    )?);

    // ------------------------- 4. the two campaign-owned relayed records
    // The producer published every record the material names directly; the
    // relayer key set and the relayed adapter configuration are named from
    // inside the provider release, and publishing them is the keeper's act.
    // An owned copy of the session's own authority: the forge would issue a
    // NEW indexed key for the role, which is a different signer entirely.
    let authority = Keypair::try_from(session.authority.to_bytes().as_slice())
        .map_err(|error| Error::new(format!("authority keypair copy: {error}")))?;
    let key_set_record = publish_record(
        &mut session.rpc,
        registry_program,
        &authority,
        RELAYER_KEY_SET_SCHEMA_RELEASE_ID_V1,
        &facts.relayer_key_set_bytes,
        None,
        &mut session.transactions,
    )?;
    let adapter_config_record = publish_record(
        &mut session.rpc,
        registry_program,
        &authority,
        RELAYED_ADAPTER_CONFIG_SCHEMA_RELEASE_ID_V1,
        &facts.relayed_adapter_config_bytes,
        None,
        &mut session.transactions,
    )?;
    // The venue's manipulation floor, published beside the source graph: the
    // record KAPPA's founding guard binds to the Source and the collateral
    // unit. The founding already refused or admitted UNDER kappa host-side
    // (input.rs); this makes the floor it admitted against a finalized record
    // anyone can re-derive the bound from.
    // Nothing on chain consumes the floor yet (the Core+Claims closure is
    // queued behind W1b), so the published pair is evidence, not an input.
    let _manipulation_floor_record = publish_record(
        &mut session.rpc,
        registry_program,
        &authority,
        MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
        &facts.manipulation_floor_bytes,
        None,
        &mut session.transactions,
    )?;
    stages.push(StageV1 {
        stage: "relayed release records".into(),
        outcome: "executed".into(),
        note: format!(
            "Published the RelayerKeySetV1 (n=1, m=1, key {}), the RelayedAdapterConfigV1 \
             (account_set_id {}) and the ManipulationFloorV1 (record {}, curve-derived floor \
             {} lamports) as finalized Registry records. With the producer's own publications \
             this completes the section-12.8 record set on the rehearsal chain. The founding \
             was admitted UNDER kappa = 1/4: principal {} of the {}-atom cap.",
            attestation.pubkey(),
            hex(&facts.account_set_id),
            hex(&published_floor),
            dclutch_source_contract::BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
            facts.admitted_principal_atoms,
            facts.admitted_principal_cap_atoms,
        ),
    });
    ledger.observe(
        &mut session.rpc,
        "relayed records published",
        0,
        0,
        LamportClaimV1::inapplicable(
            "record publication spends fees and rent from the campaign payer; no collateral moves",
        ),
        // This campaign admits no Custody namespace, so `class_of` cannot tell
        // its own Hoard from an ordinary wallet and every account censuses as
        // one undifferentiated class. Claiming `unchanged()` would report an
        // L8 green that looked like compartment-level assurance and was not.
        ClassClaimV1::inapplicable(
            "this campaign admits no Custody namespace, so it cannot classify even its own \
             Hoard; L8 has nothing to attribute a movement to and says so rather than \
             reporting one undifferentiated class as a compartment",
        ),
    )?;

    // ------------------------------------ 5. resolution funding, no-recovery
    // The PUBLISHED manifest and the PUBLISHED material digest. The market
    // compiler rewrote the Source entry's config id from the floor template's
    // digest to the compiled material's, so selecting the no-recovery entries
    // out of the DECLARED manifest picks the right indexes for a market that
    // was never founded -- and the FundingLedgerV2 address derived from that
    // mask does not exist, which the chain reports only as `Funding`.
    let manifest_record = pubkey(
        &session
            .accounts
            .get("capability_manifest_record")
            .ok_or_else(|| Error::new("no capability_manifest_record evidence"))?
            .address,
    )?;
    let published_manifest_bytes = session
        .rpc
        .required_account(manifest_record, "published CapabilityManifestV1 record")?
        .data;
    let manifest_view = CapabilityManifestV1::decode(&published_manifest_bytes)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;
    let funding_entry_indices = select_no_recovery_entries(manifest_view, published_material)?;
    let manifest_id =
        CapabilityContentId::new(market_state.identity.capability_manifest.to_bytes())
            .map_err(|error| Error::new(format!("manifest identity: {error:?}")))?;
    let selected_mask =
        funding_entry_indices
            .into_iter()
            .try_fold(0_u16, |mask, entry_index| {
                1_u16
                    .checked_shl(u32::from(entry_index))
                    .map(|bit| mask | bit)
                    .ok_or_else(|| Error::new("funding entry index exceeds the subset-ledger mask"))
            })?;
    let mut funding_ledger_bytes = vec![
        0_u8;
        funding_ledger_bytes_v2(3).map_err(|error| Error::new(
            format!("FundingLedgerV2 width: {error:?}")
        ))?
    ];
    // The rate THIS chain charges, read from the same connection the founding
    // ran on, so the rebuilt ledger is byte-identical to the one on chain.
    let funded_rent_rate = derive_funded_rent_rate_v2(
        session.rpc.minimum_balance(0)?,
        funding_ledger_bytes.len(),
        session.rpc.minimum_balance(funding_ledger_bytes.len())?,
    )
    .map_err(|error| Error::new(format!("funded rent rate: {error:?}")))?;
    FundingLedgerV2::initialize(
        &mut funding_ledger_bytes,
        manifest_id,
        manifest_view,
        selected_mask,
        funded_rent_rate,
    )
    .map_err(|error| Error::new(format!("pending FundingLedgerV2: {error:?}")))?;
    let pending_funding = FundingLedgerV2::decode(&funding_ledger_bytes)
        .map_err(|error| Error::new(format!("pending FundingLedgerV2: {error:?}")))?;
    let funding_derivation = CapabilityFundingLedgerDerivationV2::new(
        resolution_program.to_bytes(),
        market.to_bytes(),
        generation,
        manifest_id,
        pending_funding,
    )
    .map_err(|error| Error::new(format!("funding-ledger derivation: {error:?}")))?;
    let funding =
        Pubkey::find_program_address(&funding_derivation.seed_components(), &resolution_program).0;
    let source_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;

    let material_pair = RecordPairV1::derive(
        registry_program,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
        published_material,
    );
    let material_account = session
        .rpc
        .required_account(material_pair.raw, "SourceMaterialV3 record")?;
    let material = SourceMaterialV3::decode(&material_account.data)
        .map_err(|error| Error::new(format!("SourceMaterialV3: {error:?}")))?;
    // The two facts the market compiler does NOT rewrite, so they still tie
    // the founded market back to the graph this campaign compiled.
    if material.primary_source_spec().to_bytes() != facts.source_spec_digest {
        return Err(Error::new(
            "the founded Market's source material does not name the relayed source spec",
        ));
    }
    if material.recovery_policy().is_some() {
        return Err(Error::new(
            "the relayed vertical requires a no-recovery SourceMaterialV3",
        ));
    }
    match material.principal_policy() {
        SourcePrincipalPolicyV1::BoundedByFloor(floor) if floor.to_bytes() == published_floor => {}
        _ => {
            return Err(Error::new(
                "the relayed SourceMaterialV3 does not select the published manipulation floor",
            ));
        }
    }
    let manifest_pair = RecordPairV1::derive(
        registry_program,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        market_state.identity.capability_manifest.to_bytes(),
    );

    // The founding drives this ladder; this campaign authenticates the result.
    //
    // `found_through_open` ends with `execute_funding_readiness_suffix_v1`, so
    // CreateFund, ActivateFund and VerifyFundReady are already executed and
    // finalized by the time the relay begins. Asking the canonical builders to
    // construct the same three mutations again is not a second check, it is a
    // second author: `build_resolution_create_fund_v3` refuses at
    // `authenticate_vacant_destination` because the Source destination it is
    // asked to create is the account the founding created, and that arrives
    // here as a bare `Funding` naming nothing.
    //
    // So ask the readiness planner the founding itself drives, and require a
    // plan the founding's own completion check accepts. Anything else is a
    // founding defect this campaign reports rather than papers over.
    let readiness_activation_receipt = Pubkey::find_program_address(
        &[
            dclutch_resolution_codec::FUNDING_ACTIVATION_RECEIPT_PDA_DOMAIN_V1,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &resolution_program,
    )
    .0;
    let readiness = plan_funding_readiness_from_rpc_v1(
        &mut session.rpc,
        &session.plan,
        FundingReadinessCoordinatesV1 {
            market,
            source_material: FundingReadinessRecordCoordinatesV1 {
                raw: material_pair.raw,
                staging: material_pair.staging,
            },
            capability_manifest: FundingReadinessRecordCoordinatesV1 {
                raw: manifest_pair.raw,
                staging: manifest_pair.staging,
            },
            // The no-recovery material publishes no policy record.
            recovery_policy: None,
            source_state,
            funding_ledger: funding,
            beneficiary: rent_beneficiary,
            activation_receipt: readiness_activation_receipt,
        },
        0,
    )?;
    // The suffix's own completion assertion is
    // `authenticate_funding_readiness_route_v1(.., "accept")`: VerifyFundReady
    // stays buildable after it lands, so `Accept` names the finished ladder.
    // `ConsumedByFounding` is the other terminal — an atomic founding that
    // consumed the staged readiness, leaving no adjacent route at all. Neither
    // plan alone proves the accept was submitted (that is the founding
    // journal's fact, not the relay's), so the chain facts the relay actually
    // depends on are authenticated below.
    if !matches!(
        readiness,
        FundingReadinessPlanV1::Accept(_) | FundingReadinessPlanV1::ConsumedByFounding
    ) {
        return Err(Error::new(format!(
            "the founding left Resolution funding readiness at `{}`, not at a terminal route: \
             this campaign relays a Market whose funding its own founding drives, and it will \
             not drive it a second time",
            readiness.route_name(),
        )));
    }
    // ActivateFund's receipt is the account this campaign never creates and
    // cannot relay without, so its existence is named rather than assumed.
    session.rpc.required_account(
        readiness_activation_receipt,
        "the Resolution funding activation receipt the founding's suffix created",
    )?;
    // The ledger address this campaign derives from the PUBLISHED manifest and
    // the mask it selects must be the very account the founding created; the
    // planner authenticated its contents, and this names the seam.
    let funding_account = session.rpc.required_account(
        funding,
        "the Resolution funding ledger derived from the published manifest",
    )?;
    let active_funding = FundingLedgerV2::decode(&funding_account.data)
        .and_then(|ledger| ledger.authenticate(manifest_id, manifest_view))
        .map_err(|error| Error::new(format!("Resolution funding ledger: {error:?}")))?;
    for entry_index in funding_entry_indices {
        let status = active_funding
            .slot(entry_index)
            .map_err(|error| {
                Error::new(format!(
                    "Resolution funding ledger entry {entry_index}: {error:?}"
                ))
            })?
            .status();
        if status != FundingLedgerStatusV2::Active {
            return Err(Error::new(format!(
                "Resolution funding ledger entry {entry_index} is {status:?}, not Active"
            )));
        }
    }
    relayworld::require_source_phase(
        &mut session.rpc,
        source_state,
        SourceResolutionPhaseV1::Primary,
    )?;
    stages.push(StageV1 {
        stage: "resolution funding (no recovery)".into(),
        outcome: "authenticated".into(),
        note: "The founding's own post-Open readiness suffix executed CreateFund, ActivateFund \
               and VerifyFundReady over the short no-recovery frame (the two RecoveryPolicyV2 \
               tail positions absent); this campaign authenticates that result instead of \
               driving it a second time. The readiness planner reports consumed-by-founding, \
               the ledger this campaign derives from the PUBLISHED manifest is the account the \
               founding created, every selected entry is Active, and the Source state is in its \
               Primary phase. One Resolution-owned subset ledger carries the failure compartment \
               configured by the market's Source material and its two prepaid, refundable \
               companions."
            .into(),
    });
    ledger.watch("resolution_source_state", source_state);
    ledger.watch("resolution_funding_ledger", funding);
    ledger.watch("resolution_rent_beneficiary", rent_beneficiary);
    ledger.observe(
        &mut session.rpc,
        "resolution funding active",
        0,
        0,
        LamportClaimV1::inapplicable(
            "the funding ladder moves prepaid rent and quotes between campaign-owned accounts",
        ),
        // This campaign admits no Custody namespace, so `class_of` cannot tell
        // its own Hoard from an ordinary wallet and every account censuses as
        // one undifferentiated class. Claiming `unchanged()` would report an
        // L8 green that looked like compartment-level assurance and was not.
        ClassClaimV1::inapplicable(
            "this campaign admits no Custody namespace, so it cannot classify even its own \
             Hoard; L8 has nothing to attribute a movement to and says so rather than \
             reporting one undifferentiated class as a compartment",
        ),
    )?;

    // -------------------------------------------------- 6. the address book
    // The book's worker is the CAMPAIGN's own payer: every frame the campaign
    // submits (create, consume, and the prepaid probes) is signed by the
    // authority, while the daemon derives its own frames — with its own fee
    // payer as worker — inside its own process. The failure walk overrides
    // the worker with the walker.
    let authority_worker = authority.pubkey();
    let book_template = |record: Pubkey, record_bump: u8| RelayAddressBookV1 {
        worker: authority_worker,
        market,
        core_program,
        activation,
        resolution_program,
        record,
        record_bump,
        material: material_pair,
        spec: RecordPairV1::derive(
            registry_program,
            SOURCE_SPEC_SCHEMA_ID_V1,
            facts.source_spec_digest,
        ),
        provider: RecordPairV1::derive(
            registry_program,
            PROVIDER_RELEASE_SCHEMA_ID_V1,
            crate::market::record_identity(&hex_decode(&facts.input.provider_release_hex)),
        ),
        window: RecordPairV1::derive(
            registry_program,
            WINDOW_SPEC_SCHEMA_ID_V1,
            facts.window_digest,
        ),
        statistic: RecordPairV1::derive(
            registry_program,
            STATISTIC_SPEC_SCHEMA_ID_V1,
            facts.statistic_digest,
        ),
        key_set: RecordPairV1 {
            raw: key_set_record.raw,
            staging: key_set_record.staging,
        },
        config: RecordPairV1 {
            raw: adapter_config_record.raw,
            staging: adapter_config_record.staging,
        },
        venue: RecordPairV1::derive(
            registry_program,
            ARTIFACT_RELEASE_SCHEMA_ID_V1,
            facts.venue_release_digest,
        ),
        product: RecordPairV1::derive(
            registry_program,
            PRODUCT_RECORD_SCHEMA_ID_V2,
            facts.product_record_digest,
        ),
        result_domain: RecordPairV1::derive(
            registry_program,
            RESULT_DOMAIN_SCHEMA_ID_V2,
            facts.result_domain_digest,
        ),
        portfolio: RecordPairV1::derive(
            registry_program,
            PORTFOLIO_SCHEMA_ID_V2,
            facts.portfolio_digest,
        ),
        manifest: manifest_pair,
        rent_beneficiary,
        source_state,
        failure_funding: funding,
    };

    // Prepay both certificate destinations: success and failure are different
    // addresses for one sequence, so neither can overwrite the other.
    {
        let probe = book_template(Pubkey::new_unique(), 0);
        for kind in [RESOLUTION_SUCCESS_KIND, RESOLUTION_FAILURE_KIND] {
            relayworld::prepay_certificate(
                &mut session.rpc,
                &authority,
                probe.certificate_of(kind),
                &mut session.transactions,
            )?;
        }
    }

    let walk_result = match request.walk {
        WalkV1::Success => success_walk(
            &request,
            &twin,
            &mut session,
            &facts,
            published_material,
            &authority,
            &fee_payer,
            &attestation_path,
            &fee_payer_path,
            book_template,
            generation,
            &mut ledger,
            &mut stages,
        )?,
        WalkV1::Failure => failure_walk(
            &mut session,
            &facts,
            &authority,
            book_template,
            generation,
            &mut ledger,
            &mut stages,
        )?,
    };

    // ------------------------------------------------------- 7. the verdict
    let violations = ledger.violations();
    let conserved = violations.is_empty();
    let evidence = session.evidence_json()?;
    std::fs::write(&session.output, serde_json::to_vec_pretty(&evidence)?)?;

    let transcript = serde_json::json!({
        "schema": "dclutch-relayed-vertical-transcript-v1",
        "walk": request.walk,
        "disclosed_failure_conflation": DISCLOSED_FAILURE_CONFLATION,
        "synthetic_of_real": {
            "real": [
                "the venue Program and ProgramData addresses (Meteora DBC mainnet)",
                "the 424-byte VirtualPool layout, discriminator and field offsets",
                "the Loader V3 ProgramData prefix layout",
            ],
            "synthetic": [
                "the venue ELF tail and its digest",
                "the deployment slot and upgrade authority",
                "the pool address and every pool value",
                "the twin cluster itself (attestations CLAIM mainnet under the rehearsal-twin label)",
            ],
        },
        "twin_genesis": twin.genesis_hash_base58,
        "account_set_id": hex(&facts.account_set_id),
        "market": market.to_string(),
        "walk_bounty_lamports": WALK_BOUNTY_LAMPORTS,
        "founding_admission": {
            "kappa_numerator": dclutch_source_contract::CHAIN_STATE_DEFAULT_KAPPA_NUMERATOR_V1,
            "kappa_denominator": dclutch_source_contract::CHAIN_STATE_DEFAULT_KAPPA_DENOMINATOR_V1,
            "manipulation_floor_lamports":
                dclutch_source_contract::BONDING_CURVE_GRADUATION_FLOOR_LAMPORTS_V1,
            "manipulation_floor_record": hex(&published_floor),
            "principal_atoms": facts.admitted_principal_atoms.to_string(),
            "principal_cap_atoms": facts.admitted_principal_cap_atoms.to_string(),
        },
        "stages": stages,
        "walk_detail": walk_result,
        "conservation_verdict": if conserved { "conserved" } else { "violated" },
        "conservation_violations": violations,
        "observations": ledger.observations(),
    });
    std::fs::write(&request.transcript, serde_json::to_vec_pretty(&transcript)?)?;
    if !conserved {
        return Err(Error::new(format!(
            "the conservation ledger reported violations; see {}",
            request.transcript.display()
        )));
    }
    Ok(transcript)
}

/// The success walk: the daemon observes, the keeper creates, the daemon
/// submits, the keeper consumes, the market terminalizes.
#[allow(clippy::too_many_arguments)]
fn success_walk(
    request: &VerticalRequestV1,
    twin: &twin::MainnetTwinV1,
    session: &mut RelaySessionV1,
    facts: &RelayedMarketFactsV1,
    // The source-material identity the founded Market NAMES, which is not the
    // one the compiler produced: the market compiler rebuilds the manipulation
    // floor with the market's own collateral unit, so only the published
    // identity exists on chain for the relay to select.
    published_material: [u8; 32],
    authority: &Keypair,
    fee_payer: &Keypair,
    attestation_path: &std::path::Path,
    fee_payer_path: &std::path::Path,
    book_template: impl Fn(Pubkey, u8) -> RelayAddressBookV1,
    generation: u64,
    ledger: &mut ConservationLedgerV1,
    stages: &mut Vec<StageV1>,
) -> Result<serde_json::Value> {
    let entries = input::account_set_entries();
    let positions: Vec<(Pubkey, Pubkey, u16, Vec<u32>)> = entries
        .iter()
        .map(|entry| {
            (
                Pubkey::new_from_array(entry.key),
                Pubkey::new_from_array(entry.expected_owner),
                entry.inline_len,
                Vec::new(),
            )
        })
        .collect();
    let submit_genesis = daemon::genesis_hash(session.rpc.url())?;

    // Fund the daemon's fee payer on the DEVNET side.
    session.transactions.push(session.rpc.airdrop(
        "relayed vertical: fund the daemon's fee payer",
        fee_payer.pubkey(),
        2_000_000_000,
    )?);

    // 1. Observe the twin: the REAL daemon, dry run, rehearsal-twin labelled.
    let market = book_template(Pubkey::new_unique(), 0).market;
    let dry_config = daemon::render_config(
        &request.work,
        &twin.rpc_url,
        &twin.genesis_hash_base58,
        "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
        attestation_path,
        fee_payer_path,
        session.rpc.url(),
        &submit_genesis,
        book_template(Pubkey::new_unique(), 0).resolution_program,
        market,
        generation,
        book_template(Pubkey::new_unique(), 0).key_set.raw,
        book_template(Pubkey::new_unique(), 0).key_set.staging,
        None,
        None,
        RELAYED_FAMILY_RELEASE_ID_V1,
        facts.relayed_adapter_config_digest,
        &positions,
    )?;
    let artifacts = daemon::observe_dry_run(&request.relayer_bin, &request.work, &dry_config)?;
    // The artifacts MUST carry the rehearsal label: dropping it would convert
    // a rehearsal into a claim about mainnet. Refuse to continue without it.
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(artifacts.slot_dir.join("manifest.json"))?)?;
    if manifest
        .get("rehearsal_twin")
        .is_none_or(serde_json::Value::is_null)
    {
        return Err(Error::new(
            "the dry-run manifest carries no rehearsal_twin label; refusing to treat unlabelled              attestations as rehearsal evidence",
        ));
    }
    stages.push(StageV1 {
        stage: "daemon observes the twin".into(),
        outcome: "executed".into(),
        note: format!(
            "dclutch-relayer run --dry-run observed the pinned four-account set at twin slot {} \
             and signed four attestations and one seal, labelled rehearsal-twin (reading genesis \
             {} while attesting the cluster the adapter release pins).",
            artifacts.observed_slot, twin.genesis_hash_base58
        ),
    });

    // 2. The keeper creates the slot-seeded record.
    let (record, record_bump) = RelayAddressBookV1::record_for_slot(
        book_template(Pubkey::new_unique(), 0).resolution_program,
        market,
        generation,
        facts.account_set_id,
        artifacts.observed_slot,
    );
    let book = book_template(record, record_bump);
    let create = relayworld::create_record_instruction(
        &book,
        generation,
        artifacts.observed_slot,
        u16::try_from(entries.len()).map_err(|_| Error::new("set width overflow"))?,
        published_material,
        facts.source_spec_digest,
    )?;
    session.transactions.push(session.rpc.send(
        "relayed vertical: create the observation record for the observed slot",
        &[create],
        authority,
    )?);
    let accepted_create = session
        .transactions
        .last()
        .ok_or_else(|| Error::new("record creation produced no accepted transaction receipt"))?;
    let accepted_receipt_path = request.work.join("accepted-relay-record-caller.json");
    let accepted_receipt = serde_json::to_vec_pretty(&serde_json::json!({
        "schema": "dclutch.relay.accepted-caller-receipt.v1",
        "operation": "create-relayed-observation-record",
        "signature": accepted_create.signature,
        "finalized_slot": accepted_create.slot,
        "market": market.to_string(),
        "generation": generation,
        "record": record.to_string(),
        "account_set_id_hex": hex(&facts.account_set_id),
        "observed_slot": artifacts.observed_slot,
    }))?;
    std::fs::write(&accepted_receipt_path, &accepted_receipt)?;
    let accepted_receipt_sha256: [u8; 32] = Sha256::digest(&accepted_receipt).into();

    // 3. The Market's routing table, covering the append/seal and consumption
    //    frames — the family's two known over-packet wires ride it.
    let append_probe = solana_program::instruction::Instruction {
        program_id: book.resolution_program,
        accounts: book.frame_metas(
            dclutch_relay_contract::frame::RelayFrameKindV1::AppendObservation,
            None,
        )?,
        data: Vec::new(),
    };
    let consume_probe = relayworld::consume_record_instruction(
        &book,
        generation,
        artifacts.observed_slot,
        published_material,
        facts.source_spec_digest,
        &entries,
    )?;
    let (table, routed_addresses, observation, tables) = relayworld::publish_routing_table(
        &mut session.rpc,
        authority,
        "relay",
        &[append_probe, consume_probe.clone()],
        &mut session.transactions,
    )?;

    // 4. The daemon submits the RECORDED observation: append x4 over the
    //    table (the full-body VirtualPool append does not fit a bare packet),
    //    then the seal.
    let submit_config = daemon::render_config(
        &request.work.join("submit"),
        &twin.rpc_url,
        &twin.genesis_hash_base58,
        "5eykt4UsFv8P8NJdTREpY1vzqKqZKvdpKuc147dw2N9d",
        attestation_path,
        fee_payer_path,
        session.rpc.url(),
        &submit_genesis,
        book.resolution_program,
        market,
        generation,
        book.key_set.raw,
        book.key_set.staging,
        Some((table, &routed_addresses)),
        Some(daemon::LaunchCapabilityV1 {
            relay_program_data: pubkey(&session.plan.resolution.programdata_id)?,
            relay_program_deployment_slot: session.plan.resolution.deployment_slot,
            market_owner: pubkey(&session.plan.core.program_id)?,
            source_material_id: published_material,
            provider_release_id: crate::market::record_identity(&hex_decode(
                &facts.input.provider_release_hex,
            )),
            // The record binds the key set by IDENTITY; the config's
            // `relayer_key_set` is the record ACCOUNT the frame passes, and the
            // two are different 32-byte values.
            relayer_key_set_id: crate::market::record_identity(&facts.relayer_key_set_bytes),
            accepted_caller_receipt_path: &accepted_receipt_path,
            accepted_caller_receipt_sha256: accepted_receipt_sha256,
        }),
        RELAYED_FAMILY_RELEASE_ID_V1,
        facts.relayed_adapter_config_digest,
        &positions,
    )?;
    let submit_stdout = daemon::submit_artifacts(
        &request.relayer_bin,
        &request.work,
        &submit_config,
        &artifacts.slot_dir,
    )?;
    // §4.11's publication half, executed: the signed bytes push to a local
    // static-serve directory a verifier can poll. Serving it publicly stays
    // the separately authorized act.
    let public_dir = request.work.join("public");
    let publish_stdout = daemon::publish_log(
        &request.relayer_bin,
        &request.work,
        &dry_config,
        &public_dir,
    )?;
    let latest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(public_dir.join("LATEST.json"))?)?;
    let published_lines = latest
        .get("lines")
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    if published_lines < 5 {
        return Err(Error::new(format!(
            "the publication push carries {published_lines} lines; four attestations and one              seal were signed, so at least five must be public"
        )));
    }
    let _ = publish_stdout;

    let record_view_bytes = session
        .rpc
        .required_account(record, "observation record")?
        .data;
    let record_view = RelayedObservationRecordViewV1::decode(&record_view_bytes)
        .map_err(|error| Error::new(format!("observation record: {error:?}")))?;
    if record_view.phase() != Ok(RelayedRecordPhaseV1::Sealed) {
        return Err(Error::new(format!(
            "the observation record is {:?} after submit-artifacts, not Sealed",
            record_view.phase()
        )));
    }
    stages.push(StageV1 {
        stage: "daemon submits the recorded observation".into(),
        outcome: "executed".into(),
        note: format!(
            "submit-artifacts re-verified and submitted the recorded bytes: four appends and one \
             seal, v0 over the Market table {table}. The record is Sealed. Daemon output tail: {}",
            submit_stdout
                .lines()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .join(" | ")
        ),
    });
    ledger.observe(
        &mut session.rpc,
        "record sealed",
        0,
        0,
        LamportClaimV1::inapplicable("append and seal spend the daemon fee payer's lamports only"),
        // This campaign admits no Custody namespace, so `class_of` cannot tell
        // its own Hoard from an ordinary wallet and every account censuses as
        // one undifferentiated class. Claiming `unchanged()` would report an
        // L8 green that looked like compartment-level assurance and was not.
        ClassClaimV1::inapplicable(
            "this campaign admits no Custody namespace, so it cannot classify even its own \
             Hoard; L8 has nothing to attribute a movement to and says so rather than \
             reporting one undifferentiated class as a compartment",
        ),
    )?;

    // 5. Consume: the sealed graduation resolves the market through the
    //    Product's own domain, as a packet-safe v0 transaction.
    session.transactions.push(session.rpc.send_v0(
        "relayed vertical: a sealed graduation resolves the market",
        &[consume_probe],
        authority,
        observation,
        &tables,
    )?);
    let source = relayworld::require_source_phase(
        &mut session.rpc,
        book.source_state,
        SourceResolutionPhaseV1::Resolved,
    )?;
    let certificate = relayworld::read_certificate(
        &mut session.rpc,
        book.certificate_of(RESOLUTION_SUCCESS_KIND),
        ResolutionCertificateKindV2::ResolutionSuccess,
    )?;
    let domain = ResultDomainV2::decode(&facts.result_domain_bytes)
        .map_err(|error| Error::new(format!("ResultDomainV2: {error:?}")))?;
    if certificate.selector == domain.failure_selector() {
        return Err(Error::new(
            "the graduation resolved onto the failure selector",
        ));
    }
    let consumed_view_bytes = session
        .rpc
        .required_account(record, "observation record")?
        .data;
    let consumed_view = RelayedObservationRecordViewV1::decode(&consumed_view_bytes)
        .map_err(|error| Error::new(format!("observation record: {error:?}")))?;
    if consumed_view.phase() != Ok(RelayedRecordPhaseV1::Consumed) {
        return Err(Error::new(format!(
            "the observation record is {:?} after consumption, not Consumed",
            consumed_view.phase()
        )));
    }
    stages.push(StageV1 {
        stage: "consume and terminalize".into(),
        outcome: "executed".into(),
        note: format!(
            "The 28-account consumption rode the Market table as a v0 transaction; the Source \
             resolution state is Resolved at terminal sequence {}, the record is Consumed, and \
             the ResolutionSuccess certificate selects ordinary cell {} of the zero-cut domain \
             (failure cell {}).",
            relayworld::TERMINAL_SEQUENCE,
            certificate.selector,
            domain.failure_selector(),
        ),
    });
    ledger.observe(
        &mut session.rpc,
        "market terminalized (success)",
        0,
        0,
        LamportClaimV1::inapplicable(
            "consumption allocates the certificate from its own prepaid rent; no collateral moves",
        ),
        // This campaign admits no Custody namespace, so `class_of` cannot tell
        // its own Hoard from an ordinary wallet and every account censuses as
        // one undifferentiated class. Claiming `unchanged()` would report an
        // L8 green that looked like compartment-level assurance and was not.
        ClassClaimV1::inapplicable(
            "this campaign admits no Custody namespace, so it cannot classify even its own \
             Hoard; L8 has nothing to attribute a movement to and says so rather than \
             reporting one undifferentiated class as a compartment",
        ),
    )?;
    let _ = source;
    Ok(serde_json::json!({
        "observed_slot": artifacts.observed_slot,
        "record": record.to_string(),
        "routing_table": table.to_string(),
        "certificate": book.certificate_of(RESOLUTION_SUCCESS_KIND).to_string(),
        "certificate_selector": certificate.selector,
        "failure_selector": domain.failure_selector(),
        "publication_lines": published_lines,
    }))
}

/// The silent-relayer sibling: nobody observes, the deadline passes, the
/// funded walk pays a walker on a bare legacy transaction.
fn failure_walk(
    session: &mut RelaySessionV1,
    facts: &RelayedMarketFactsV1,
    authority: &Keypair,
    book_template: impl Fn(Pubkey, u8) -> RelayAddressBookV1,
    generation: u64,
    ledger: &mut ConservationLedgerV1,
    stages: &mut Vec<StageV1>,
) -> Result<serde_json::Value> {
    let book = book_template(Pubkey::new_unique(), 0);
    let deadline = facts
        .window
        .end_unix_seconds()
        .checked_add(i64::from(facts.window.max_age_seconds()))
        .ok_or_else(|| Error::new("primary deadline overflow"))?;

    // Wait out the market's own deadline in real time, by the DEVNET clock.
    loop {
        let slot = session.rpc.finalized_slot()?;
        let now = session.rpc.block_time(slot)?;
        if now > deadline {
            break;
        }
        std::thread::sleep(std::time::Duration::from_secs(2));
    }
    stages.push(StageV1 {
        stage: "the relayer stays silent".into(),
        outcome: "executed".into(),
        note: format!(
            "No observation record was ever created; the devnet clock passed the market's own \
             primary deadline (window.end + max_age = {deadline})."
        ),
    });

    // A walker who is nobody: a fresh key, funded only to pay its own fee.
    let walker = Keypair::new();
    session.transactions.push(session.rpc.airdrop(
        "relayed vertical: fund the walker",
        walker.pubkey(),
        1_000_000_000,
    )?);
    let mut walker_book = book;
    walker_book.worker = walker.pubkey();
    let walk = relayworld::deadline_failure_instruction(&walker_book, generation)?;
    let extent = relayworld::legacy_wire_extent(&walk, walker.pubkey());
    if extent > 1_232 {
        return Err(Error::new(format!(
            "the deadline walk serialised to {extent} bytes; it must fit a bare legacy packet"
        )));
    }
    let walker_before = session
        .rpc
        .required_account(walker.pubkey(), "walker")?
        .lamports;
    let funding_before = session
        .rpc
        .required_account(walker_book.failure_funding, "failure Fund")?
        .lamports;
    let evidence = session.rpc.send(
        "relayed vertical: a silent relayer cannot make the market unresolvable",
        &[walk],
        &walker,
    )?;
    let fee = evidence.fee_lamports.unwrap_or(0);
    session.transactions.push(evidence);
    let walker_after = session
        .rpc
        .required_account(walker.pubkey(), "walker")?
        .lamports;
    let funding_after = session
        .rpc
        .required_account(walker_book.failure_funding, "failure Fund")?
        .lamports;
    if walker_after
        != walker_before
            .checked_sub(fee)
            .and_then(|value| value.checked_add(WALK_BOUNTY_LAMPORTS))
            .ok_or_else(|| Error::new("walker balance arithmetic overflow"))?
    {
        return Err(Error::new(format!(
            "the walker moved from {walker_before} to {walker_after} with fee {fee}; the walk \
             must pay exactly the disclosed {WALK_BOUNTY_LAMPORTS}-lamport bounty"
        )));
    }
    if funding_after != funding_before.saturating_sub(WALK_BOUNTY_LAMPORTS) {
        return Err(Error::new(format!(
            "the failure Fund moved from {funding_before} to {funding_after}; it must be debited \
             exactly the bounty"
        )));
    }

    relayworld::require_source_phase(
        &mut session.rpc,
        walker_book.source_state,
        SourceResolutionPhaseV1::FailureCommitted,
    )?;
    let certificate = relayworld::read_certificate(
        &mut session.rpc,
        walker_book.certificate_of(RESOLUTION_FAILURE_KIND),
        ResolutionCertificateKindV2::ResolutionFailure,
    )?;
    let domain = ResultDomainV2::decode(&facts.result_domain_bytes)
        .map_err(|error| Error::new(format!("ResultDomainV2: {error:?}")))?;
    if certificate.selector != domain.failure_selector()
        || certificate.work_paid != WALK_BOUNTY_LAMPORTS
        || certificate.route != [0; 32]
        || certificate.provider_evidence != [0; 32]
    {
        return Err(Error::new(format!(
            "the failure certificate does not carry the walk's own facts: selector {} (failure \
             {}), work_paid {}, route zero {}, evidence zero {}",
            certificate.selector,
            domain.failure_selector(),
            certificate.work_paid,
            certificate.route == [0; 32],
            certificate.provider_evidence == [0; 32],
        )));
    }
    stages.push(StageV1 {
        stage: "the funded walk pays a walker".into(),
        outcome: "executed".into(),
        note: format!(
            "CommitDeadlineFailure on a bare {extent}-byte legacy transaction (limit 1,232; no \
             lookup table — the one route that must work when nobody cooperated never depends on \
             one). The walker was credited exactly the disclosed {WALK_BOUNTY_LAMPORTS}-lamport \
             bounty from the market's own escrow; the certificate is ResolutionFailure at the \
             Product's pre-disclosed failure cell with route and provider evidence zero."
        ),
    });
    // ------------------------- the walked market ENDS TERMINAL, live
    // The exact next instruction the ProgramTest campaign proved
    // (walked_to_failure, resolution_core_v3_lifecycle.rs), now against the
    // live validator: chain-derived, validated, then asserted three ways.
    let admit = build_resolution_admit_terminal_v3(&terminal_snapshot(
        &mut session.rpc,
        &walker_book,
        pubkey(&session.plan.registry.program_id)?,
        pubkey(&session.plan.core.programdata_id)?,
        pubkey(&session.plan.resolution.programdata_id)?,
    )?)
    .map_err(|error| Error::new(format!("chain-derived AdmitTerminal: {error:?}")))?;
    validate_resolution_admit_terminal_report_v3(&admit)
        .map_err(|error| Error::new(format!("AdmitTerminal report: {error:?}")))?;
    session.transactions.push(session.rpc.send(
        "relayed vertical: the walked market ends terminal on its pre-disclosed failure terms",
        std::slice::from_ref(&admit.instruction),
        authority,
    )?);
    let admitted = CoreState::decode(
        &session
            .rpc
            .required_account(walker_book.market, "terminal Market")?
            .data,
    )
    .map_err(|error| Error::new(format!("terminal Market: {error:?}")))?;
    let expected_certificate = walker_book.certificate_of(RESOLUTION_FAILURE_KIND);
    if !matches!(admitted.phase, Phase::Terminal) {
        return Err(Error::new(format!(
            "the walked market did not end Terminal: phase {:?}",
            admitted.phase
        )));
    }
    if admitted.terminal_receipt.map(|value| value.to_bytes())
        != Some(expected_certificate.to_bytes())
    {
        return Err(Error::new(
            "the Market did not commit to the FAILURE certificate's own address — the seat a \
             provider-resolved terminal writes is a different PDA, and neither may occupy the \
             other's",
        ));
    }
    if admitted.terminal_winner != domain.failure_selector() {
        return Err(Error::new(format!(
            "terminal_winner {} is not the Product's pre-disclosed failure selector {}",
            admitted.terminal_winner,
            domain.failure_selector()
        )));
    }
    stages.push(StageV1 {
        stage: "the walked market ends terminal".into(),
        outcome: "executed".into(),
        note: format!(
            "AdmitTerminal, chain-derived from the FailureCommitted Source, executed against the \
             live validator. The Market is Terminal, committed to the failure certificate's own \
             address (a different PDA from a provider-resolved terminal's seat), and \
             terminal_winner {} is the selector the Source's own failure decision carried.",
            admitted.terminal_winner
        ),
    });
    ledger.observe(
        &mut session.rpc,
        "market terminalized (failure walk)",
        0,
        0,
        LamportClaimV1::inapplicable(
            "the walk moves the disclosed bounty from the watched escrow to the walker, and the \
             terminal admission moves no collateral; the stage's own assertions carry the exact \
             lamport deltas",
        ),
        // This campaign admits no Custody namespace, so `class_of` cannot tell
        // its own Hoard from an ordinary wallet and every account censuses as
        // one undifferentiated class. Claiming `unchanged()` would report an
        // L8 green that looked like compartment-level assurance and was not.
        ClassClaimV1::inapplicable(
            "this campaign admits no Custody namespace, so it cannot classify even its own \
             Hoard; L8 has nothing to attribute a movement to and says so rather than \
             reporting one undifferentiated class as a compartment",
        ),
    )?;
    Ok(serde_json::json!({
        "terminal_phase": "Terminal",
        "terminal_winner": admitted.terminal_winner,
        "walk_wire_bytes": extent,
        "walker": walker.pubkey().to_string(),
        "bounty_paid_lamports": WALK_BOUNTY_LAMPORTS,
        "certificate": walker_book.certificate_of(RESOLUTION_FAILURE_KIND).to_string(),
        "certificate_selector": certificate.selector,
    }))
}

/// The no-recovery funding-entry selection, mirroring the operator's own
/// canonical derivation: the unique material-configured controller entry is
/// the failure compartment; the exactly-two other controller entries, in
/// manifest order, are the companions.
fn select_no_recovery_entries(
    manifest: CapabilityManifestV1<'_>,
    material_digest: [u8; 32],
) -> Result<[u16; 3]> {
    let mut failure = None;
    let mut others = Vec::new();
    for entry_index in 0..manifest.entry_count() {
        let entry = manifest
            .entry(entry_index)
            .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
        if entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V7 {
            continue;
        }
        if entry.config_id().to_bytes() == material_digest {
            if failure.replace(entry_index).is_some() {
                return Err(Error::new("two entries configure the Source material"));
            }
        } else {
            others.push(entry_index);
        }
    }
    if others.len() != 2 {
        return Err(Error::new(format!(
            "a no-recovery manifest needs exactly two companion controller entries, found {}",
            others.len()
        )));
    }
    Ok([
        others[0],
        others[1],
        failure.ok_or_else(|| Error::new("no entry configures the Source material"))?,
    ])
}

/// Same-finalized snapshot for the terminal admission of a walked market.
///
/// Every address comes off the campaign's own book; the five staging cursors
/// are vacant by construction on a chain whose records were published in
/// transactions and never staged again.
fn terminal_snapshot(
    rpc: &mut Rpc,
    book: &RelayAddressBookV1,
    registry_program: Pubkey,
    core_programdata: Pubkey,
    resolution_programdata: Pubkey,
) -> Result<ResolutionAdmitTerminalSnapshotV3> {
    let certificate = book.certificate_of(RESOLUTION_FAILURE_KIND);
    let (observation, present) = rpc.finalized_observed_accounts(
        &[
            book.market,
            book.activation,
            registry_program,
            book.core_program,
            core_programdata,
            book.resolution_program,
            resolution_programdata,
            book.material.raw,
            book.manifest.raw,
            book.source_state,
            book.failure_funding,
            certificate,
            sysvar::rent::ID,
            book.product.raw,
            book.result_domain.raw,
            book.portfolio.raw,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ResolutionAdmitTerminalSnapshotV3 {
        market: at(0)?,
        activation_cache: at(1)?,
        registry_program: at(2)?,
        core_program: at(3)?,
        core_programdata: at(4)?,
        resolution_program: at(5)?,
        resolution_programdata: at(6)?,
        source_material: at(7)?,
        source_material_staging: vacant(observation, book.material.staging),
        capability_manifest: at(8)?,
        capability_manifest_staging: vacant(observation, book.manifest.staging),
        source_state: at(9)?,
        funding_ledger: at(10)?,
        certificate: at(11)?,
        rent_sysvar: at(12)?,
        product_raw: at(13)?,
        product_staging: vacant(observation, book.product.staging),
        result_domain_raw: at(14)?,
        result_domain_staging: vacant(observation, book.result_domain.staging),
        portfolio_raw: at(15)?,
        portfolio_staging: vacant(observation, book.portfolio.staging),
    })
}

fn vacant(
    observation: dclutch_resolution_core_v3_operator::Observation,
    key: Pubkey,
) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        lamports: 0,
        owner: system_program::ID,
        executable: false,
        data: Vec::new(),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hex_decode(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .filter_map(|index| u8::from_str_radix(text.get(index..index + 2).unwrap_or(""), 16).ok())
        .collect()
}
