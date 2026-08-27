//! The vertical itself: found → observe → seal → consume → terminalize, and
//! the silent-relayer sibling that pays a walker.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::Serialize;
use solana_sdk::pubkey::Pubkey;
use solana_sdk::signature::{Keypair, Signer};

use dclutch_capability_contract::{
    CapabilityFundingDerivationV1, CapabilityManifestV1, ContentId as CapabilityContentId,
    FUNDING_STATE_BYTES, FundingCustodyObservationV1, FundingStateV1, FundingStatus,
};
use dclutch_market_core_codec::CoreState;
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
use dclutch_resolution_codec::{RESOLUTION_CONTROLLER_RELEASE_ID_V4, ResolutionCertificateKindV2};
use dclutch_resolution_core_v3_operator::{
    ObservedAccount, ResolutionCreateFundSnapshotV3, ResolutionVerifyFundReadySnapshotV3,
    build_resolution_create_fund_v3, build_resolution_verify_fund_ready_v3,
    validate_resolution_create_fund_report_v3, validate_resolution_verify_fund_ready_report_v3,
};
use dclutch_source_contract::{
    PROVIDER_RELEASE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
    SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2, SOURCE_SPEC_SCHEMA_ID_V1, SourceResolutionPhaseV1,
    WINDOW_SPEC_SCHEMA_ID_V1,
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::transfer;

use crate::daemon;
use crate::input::{
    self, DISCLOSED_FAILURE_CONFLATION, RelayedMarketFactsV1, WALK_BOUNTY_LAMPORTS,
};
use crate::ledger::{ConservationLedgerV1, LamportClaimV1};
use crate::plan::pubkey;
use crate::relayworld::{
    self, RESOLUTION_FAILURE_KIND, RESOLUTION_SUCCESS_KIND, RecordPairV1, RelayAddressBookV1,
};
use crate::rpc::Rpc;
use crate::runtime::{OpenMarketSessionV1, found_through_open, publish_record};
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
    pub(crate) spec_template: PathBuf,
    pub(crate) transcript: PathBuf,
    pub(crate) relayer_bin: PathBuf,
    pub(crate) work: PathBuf,
    pub(crate) keypair_seed: Option<String>,
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

/// The whole vertical.
pub(crate) fn execute(request: VerticalRequestV1) -> Result<serde_json::Value> {
    std::fs::create_dir_all(&request.work)?;
    let mut stages: Vec<StageV1> = Vec::new();
    let start_unix = now_unix()?;

    // ---------------------------------------------------------- 1. the twin
    // The successor validator's base is allocated by the runner but not yet
    // bound; the twin's allocator must not take that block.
    let template_probe: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&request.spec_template)?)?;
    let successor_base = template_probe
        .get("rpc_url")
        .and_then(|value| value.as_str())
        .and_then(|url| url.rsplit(':').next())
        .and_then(|tail| tail.trim_end_matches('/').parse::<u16>().ok());
    let twin = twin::start(&request.work, start_unix, successor_base)?;
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

    // ---------------------------------------------- 2. keys and the market
    // The relayer's attestation key is FOUNDING CONTENT (the key-set record's
    // digest seeds half the graph), so it is generated before the spec exists
    // and disclosed through the records rather than through any wallet store.
    let attestation = Keypair::new();
    let fee_payer = Keypair::new();
    let keys_dir = request.work.join("relayer-keys");
    let attestation_path = daemon::write_keypair_file(&keys_dir, "attestation.json", &attestation)?;
    let fee_payer_path = daemon::write_keypair_file(&keys_dir, "fee-payer.json", &fee_payer)?;

    let template: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&request.spec_template)?)?;
    let registry = pubkey(
        template
            .get("registry")
            .and_then(|role| role.get("program_id"))
            .and_then(|value| value.as_str())
            .ok_or_else(|| Error::new("the spec template names no registry program id"))?,
    )?;
    let window_choice = input::window_choice(start_unix, request.walk == WalkV1::Success);
    let facts =
        input::relayed_market_input(registry, attestation.pubkey().to_bytes(), &window_choice)?;

    let mut spec_value = template;
    spec_value["market"] = serde_json::to_value(&facts.input)?;
    spec_value["record_publication"] = serde_json::Value::String("transaction".into());
    let spec_path = request.work.join("spec.json");
    std::fs::write(&spec_path, serde_json::to_vec_pretty(&spec_value)?)?;

    // ------------------------------------------------------- 3. the founding
    let mut session = found_through_open(&spec_path, request.keypair_seed.as_deref())?;
    stages.push(StageV1 {
        stage: "founding through Open".into(),
        outcome: "executed".into(),
        note: format!(
            "The tier-1 producer's own campaign, transaction-only record publication, founding \
             the zero-cut graduation Product with NO recovery policy. {} transactions to here. \
             Disclosed at founding: {DISCLOSED_FAILURE_CONFLATION}",
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
    if market_state.identity.resolution_policy.to_bytes() != facts.material_digest {
        return Err(Error::new(
            "the founded Market's resolution policy is not the compiled relayed material",
        ));
    }

    let mut ledger = ConservationLedgerV1::new(mint, session.authority.pubkey());
    ledger.admit_founding(hoard, aggregate, 1);

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
    stages.push(StageV1 {
        stage: "relayed release records".into(),
        outcome: "executed".into(),
        note: format!(
            "Published the RelayerKeySetV1 (n=1, m=1, key {}) and the RelayedAdapterConfigV1 \
             (account_set_id {}) as finalized Registry records. With the producer's own \
             publications this completes the section-12.8 record set on the rehearsal chain.",
            attestation.pubkey(),
            hex(&facts.account_set_id),
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
    )?;

    // ------------------------------------ 5. resolution funding, no-recovery
    let manifest_view = CapabilityManifestV1::decode(&facts.manifest_bytes)
        .map_err(|error| Error::new(format!("capability manifest: {error:?}")))?;
    let funding_entry_indices = select_no_recovery_entries(manifest_view, facts.material_digest)?;
    let manifest_id =
        CapabilityContentId::new(market_state.identity.capability_manifest.to_bytes())
            .map_err(|error| Error::new(format!("manifest identity: {error:?}")))?;
    let funding_state_rent = session.rpc.minimum_balance(FUNDING_STATE_BYTES)?;
    let mut funding = [Pubkey::default(); 3];
    for (slot, entry_index) in funding_entry_indices.into_iter().enumerate() {
        let entry = manifest_view
            .entry(entry_index)
            .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?;
        let target = funding_state_rent
            .checked_add(entry.funding_quote().amounts().native_lamports_total())
            .ok_or_else(|| Error::new("funding target overflow"))?;
        let custody = FundingCustodyObservationV1::native_only(target, funding_state_rent)
            .map_err(|error| Error::new(format!("funding custody: {error:?}")))?;
        let state = FundingStateV1::new(manifest_id, manifest_view, entry_index, custody)
            .map_err(|error| Error::new(format!("pending FundingState: {error:?}")))?;
        let derivation = CapabilityFundingDerivationV1::new(
            market.to_bytes(),
            generation,
            manifest_id,
            manifest_view,
            state,
        )
        .map_err(|error| Error::new(format!("funding derivation: {error:?}")))?;
        funding[slot] =
            Pubkey::find_program_address(&derivation.seed_components(), &resolution_program).0;
    }
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
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        facts.material_digest,
    );
    let manifest_pair = RecordPairV1::derive(
        registry_program,
        dclutch_capability_contract::CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        crate::market::record_identity(&facts.manifest_bytes),
    );

    let create_snapshot = fund_snapshot(
        &mut session.rpc,
        market,
        activation,
        registry_program,
        core_program,
        pubkey(&session.plan.core.programdata_id)?,
        resolution_program,
        pubkey(&session.plan.resolution.programdata_id)?,
        material_pair,
        manifest_pair,
        source_state,
        funding,
    )?;
    let create = build_resolution_create_fund_v3(&create_snapshot)
        .map_err(|error| Error::new(format!("chain-derived CreateFund: {error:?}")))?;
    validate_resolution_create_fund_report_v3(&create)
        .map_err(|error| Error::new(format!("CreateFund report: {error:?}")))?;
    let mut prepay = Vec::new();
    if create.source_top_up_lamports > 0 {
        prepay.push(transfer(
            &authority.pubkey(),
            &source_state,
            create.source_top_up_lamports,
        ));
    }
    for (destination, top_up) in funding.into_iter().zip(create.funding_top_up_lamports) {
        if top_up > 0 {
            prepay.push(transfer(&authority.pubkey(), &destination, top_up));
        }
    }
    if !prepay.is_empty() {
        session.transactions.push(session.rpc.send(
            "relayed vertical: prepay the Source state and the three Resolution Funds",
            &prepay,
            &authority,
        )?);
    }
    let create = build_resolution_create_fund_v3(&fund_snapshot(
        &mut session.rpc,
        market,
        activation,
        registry_program,
        core_program,
        pubkey(&session.plan.core.programdata_id)?,
        resolution_program,
        pubkey(&session.plan.resolution.programdata_id)?,
        material_pair,
        manifest_pair,
        source_state,
        funding,
    )?)
    .map_err(|error| Error::new(format!("chain-derived CreateFund after prepay: {error:?}")))?;
    session.transactions.push(session.rpc.send(
        "relayed vertical: a no-recovery Market creates its own Resolution funding",
        std::slice::from_ref(&create.instruction),
        &authority,
    )?);
    let verify = build_resolution_verify_fund_ready_v3(&verify_snapshot(
        &mut session.rpc,
        market,
        activation,
        registry_program,
        core_program,
        pubkey(&session.plan.core.programdata_id)?,
        resolution_program,
        pubkey(&session.plan.resolution.programdata_id)?,
        material_pair,
        manifest_pair,
        source_state,
        funding,
        rent_beneficiary,
    )?)
    .map_err(|error| Error::new(format!("chain-derived VerifyFundReady: {error:?}")))?;
    validate_resolution_verify_fund_ready_report_v3(&verify)
        .map_err(|error| Error::new(format!("VerifyFundReady report: {error:?}")))?;
    session.transactions.push(session.rpc.send(
        "relayed vertical: activate the no-recovery Resolution funding",
        std::slice::from_ref(&verify.instruction),
        &authority,
    )?);
    for (label, address) in [
        ("recovery companion Fund", funding[0]),
        ("exhaustion companion Fund", funding[1]),
        ("failure Fund", funding[2]),
    ] {
        let status = FundingStateV1::decode(&session.rpc.required_account(address, label)?.data)
            .map_err(|error| Error::new(format!("{label}: {error:?}")))?
            .status();
        if status != FundingStatus::Active {
            return Err(Error::new(format!("{label} is {status:?}, not Active")));
        }
    }
    relayworld::require_source_phase(
        &mut session.rpc,
        source_state,
        SourceResolutionPhaseV1::Primary,
    )?;
    stages.push(StageV1 {
        stage: "resolution funding (no recovery)".into(),
        outcome: "executed".into(),
        note: "CreateFund and VerifyFundReady over the SHORT no-recovery frame (the two \
               RecoveryPolicyV2 tail positions absent), the first execution of the e5b6923 \
               admission on a live validator. The failure compartment is configured by the \
               market's own Source material; the two companions are prepaid and refundable."
            .into(),
    });
    ledger.watch("resolution_source_state", source_state);
    ledger.watch("resolution_funding_failure", funding[2]);
    ledger.watch("resolution_rent_beneficiary", rent_beneficiary);
    ledger.observe(
        &mut session.rpc,
        "resolution funding active",
        0,
        0,
        LamportClaimV1::inapplicable(
            "the funding ladder moves prepaid rent and quotes between campaign-owned accounts",
        ),
    )?;

    // -------------------------------------------------- 6. the address book
    let book_template = |record: Pubkey, record_bump: u8| RelayAddressBookV1 {
        worker: fee_payer.pubkey(),
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
        failure_funding: funding[2],
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
    let evidence = session.evidence();
    std::fs::write(
        &session.spec.output,
        serde_json::to_vec_pretty(&serde_json::to_value(&evidence)?)?,
    )?;

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
    session: &mut OpenMarketSessionV1,
    facts: &RelayedMarketFactsV1,
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
        book_template(Pubkey::new_unique(), 0).resolution_program,
        market,
        generation,
        book_template(Pubkey::new_unique(), 0).key_set.raw,
        book_template(Pubkey::new_unique(), 0).key_set.staging,
        None,
        RELAYED_FAMILY_RELEASE_ID_V1,
        facts.relayed_adapter_config_digest,
        &positions,
    )?;
    let artifacts = daemon::observe_dry_run(&request.relayer_bin, &request.work, &dry_config)?;
    // The artifacts MUST carry the rehearsal label: dropping it would convert
    // a rehearsal into a claim about mainnet. Refuse to continue without it.
    let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(
        artifacts.slot_dir.join("manifest.json"),
    )?)?;
    if manifest.get("rehearsal_twin").is_none_or(serde_json::Value::is_null) {
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
        facts.material_digest,
        facts.source_spec_digest,
    )?;
    session.transactions.push(session.rpc.send(
        "relayed vertical: create the observation record for the observed slot",
        &[create],
        authority,
    )?);

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
        facts.material_digest,
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
        book.resolution_program,
        market,
        generation,
        book.key_set.raw,
        book.key_set.staging,
        Some((table, &routed_addresses)),
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
    )?;
    let _ = source;
    Ok(serde_json::json!({
        "observed_slot": artifacts.observed_slot,
        "record": record.to_string(),
        "routing_table": table.to_string(),
        "certificate": book.certificate_of(RESOLUTION_SUCCESS_KIND).to_string(),
        "certificate_selector": certificate.selector,
        "failure_selector": domain.failure_selector(),
    }))
}

/// The silent-relayer sibling: nobody observes, the deadline passes, the
/// funded walk pays a walker on a bare legacy transaction.
fn failure_walk(
    session: &mut OpenMarketSessionV1,
    facts: &RelayedMarketFactsV1,
    _authority: &Keypair,
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
    ledger.observe(
        &mut session.rpc,
        "market terminalized (failure walk)",
        0,
        0,
        LamportClaimV1::inapplicable(
            "the walk moves the disclosed bounty from the watched escrow to the walker; the \
             stage's own assertions carry the exact lamport deltas",
        ),
    )?;
    Ok(serde_json::json!({
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
        if entry.release_id().to_bytes() != RESOLUTION_CONTROLLER_RELEASE_ID_V4 {
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

#[allow(clippy::too_many_arguments)]
fn fund_snapshot(
    rpc: &mut Rpc,
    market: Pubkey,
    activation: Pubkey,
    registry_program: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
    resolution_program: Pubkey,
    resolution_programdata: Pubkey,
    material: RecordPairV1,
    manifest: RecordPairV1,
    source_state: Pubkey,
    funding: [Pubkey; 3],
) -> Result<ResolutionCreateFundSnapshotV3> {
    let (observation, present) = rpc.finalized_observed_accounts(
        &[
            market,
            activation,
            registry_program,
            core_program,
            core_programdata,
            resolution_program,
            resolution_programdata,
            material.raw,
            manifest.raw,
            sysvar::rent::ID,
            system_program::ID,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ResolutionCreateFundSnapshotV3 {
        market: at(0)?,
        activation_cache: at(1)?,
        registry_program: at(2)?,
        core_program: at(3)?,
        core_programdata: at(4)?,
        resolution_program: at(5)?,
        resolution_programdata: at(6)?,
        source_material: at(7)?,
        source_material_staging: vacant(observation, material.staging),
        capability_manifest: at(8)?,
        capability_manifest_staging: vacant(observation, manifest.staging),
        source_destination: observed_or_vacant(rpc, observation, source_state)?,
        recovery_destination: observed_or_vacant(rpc, observation, funding[0])?,
        exhaustion_destination: observed_or_vacant(rpc, observation, funding[1])?,
        failure_destination: observed_or_vacant(rpc, observation, funding[2])?,
        rent_sysvar: at(9)?,
        system_program: at(10)?,
        // The no-recovery material has no policy record; per the operator's
        // own None rule the two policy positions re-present the material pair.
        recovery_policy: at(7)?,
        recovery_policy_staging: vacant(observation, material.staging),
    })
}

#[allow(clippy::too_many_arguments)]
fn verify_snapshot(
    rpc: &mut Rpc,
    market: Pubkey,
    activation: Pubkey,
    registry_program: Pubkey,
    core_program: Pubkey,
    core_programdata: Pubkey,
    resolution_program: Pubkey,
    resolution_programdata: Pubkey,
    material: RecordPairV1,
    manifest: RecordPairV1,
    source_state: Pubkey,
    funding: [Pubkey; 3],
    rent_beneficiary: Pubkey,
) -> Result<ResolutionVerifyFundReadySnapshotV3> {
    let (observation, present) = rpc.finalized_observed_accounts(
        &[
            market,
            activation,
            registry_program,
            core_program,
            core_programdata,
            resolution_program,
            resolution_programdata,
            material.raw,
            manifest.raw,
            source_state,
            funding[0],
            funding[1],
            funding[2],
            rent_beneficiary,
            sysvar::rent::ID,
            sysvar::clock::ID,
        ],
        0,
    )?;
    let at = |index: usize| -> Result<ObservedAccount> {
        present
            .get(index)
            .cloned()
            .ok_or_else(|| Error::new("finalized observation lost an account"))
    };
    Ok(ResolutionVerifyFundReadySnapshotV3 {
        market: at(0)?,
        activation_cache: at(1)?,
        registry_program: at(2)?,
        core_program: at(3)?,
        core_programdata: at(4)?,
        resolution_program: at(5)?,
        resolution_programdata: at(6)?,
        source_material: at(7)?,
        source_material_staging: vacant(observation, material.staging),
        capability_manifest: at(8)?,
        capability_manifest_staging: vacant(observation, manifest.staging),
        source_state: at(9)?,
        recovery_funding: at(10)?,
        exhaustion_funding: at(11)?,
        failure_funding: at(12)?,
        beneficiary: at(13)?,
        rent_sysvar: at(14)?,
        clock_sysvar: at(15)?,
        recovery_policy: at(7)?,
        recovery_policy_staging: vacant(observation, material.staging),
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

fn observed_or_vacant(
    rpc: &mut Rpc,
    observation: dclutch_resolution_core_v3_operator::Observation,
    key: Pubkey,
) -> Result<ObservedAccount> {
    Ok(match rpc.account(key)? {
        None => vacant(observation, key),
        Some(account) => ObservedAccount {
            observation,
            key,
            lamports: account.lamports,
            owner: account.owner,
            executable: account.executable,
            data: account.data,
        },
    })
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
