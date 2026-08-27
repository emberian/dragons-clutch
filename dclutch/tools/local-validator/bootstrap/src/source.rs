//! Chain-derived local dClutch creation and real-Pyth Source composition.

use std::collections::BTreeMap;

use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1,
    MANIFEST_HEADER_BYTES, MARKET_OPENING_READINESS_PDA_DOMAIN, MAX_DEPENDENCIES_PER_CAPABILITY,
    MarketOpeningReadinessV1,
    readiness_instruction::{AdvanceMarketOpeningReadinessV1, BeginMarketOpeningReadinessV1},
};
use dclutch_collateral_contract::{COLLATERAL_CUSTODY_PDA_DOMAIN, COLLATERAL_VAULT_PDA_DOMAIN};
use dclutch_core_contract::Phase;
use dclutch_market_contract::market::{CategoricalMarketV1, decode_market_outcome_count};
use dclutch_operator::foundation::{
    CreationRecordKindV1, CreationRecordObligationV1, FOUNDATION_GENERATION, FinalizedRecordProof,
    FoundMarketState, ObservedVacancy, OpenCollateralVaultState, ReleaseBoundCreationPlanV1,
    TerminalPythArtifactInputV1, TerminalPythCreationInputV1, build_found_market_and_fund_v1,
    build_open_collateral_vault_v1, compile_terminal_pyth_artifacts_v1,
    compile_terminal_pyth_creation_v1,
};
use dclutch_operator::source_resolution::{
    SourceAcceptPrimaryInlineState, SourceCreateResolutionState,
    build_source_accept_primary_inline_v1, build_source_create_resolution_v1,
};
use dclutch_operator::{Finality, Observation, ObservedAccount};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::{
        CapacityEnvelope as ProductCapacityEnvelope, CapacityProfileV1, CapacityProfileV1Input,
    },
    result_domain::FiniteResultDomainV1,
};
use dclutch_pyth_svm::{
    ProgramDataV3View, ProgramV3View, PythReleaseV1, ReceiverConfigV2View,
    local_validator_release_v1,
};
use dclutch_realm_contract::{FreezeAuthorityPolicy, MintAuthorityPolicy, RealmV1, RealmV1Input};
use dclutch_record_contract::{
    AppendPageV1, BeginRecordV1, CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1, ContentDigest,
    FinalizeRecordV1, RecordKeyV1, STAGING_CURSOR_BYTES_V1, SchemaReleaseId,
};
use dclutch_source_contract::{
    CapacityEnvelope as SourceCapacityEnvelope, ContentId as SourceContentId,
    NORMALIZED_EVIDENCE_BYTES, PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1, ProviderReleaseV1,
    PythAdapterConfigV1, SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1, SourceCapacityProfileV1,
    SourceMaterialViewV1, SourceResolutionPhaseV1, SourceResolutionStateV1,
};
use serde::Serialize;
use sha2::Digest;
use solana_program::hash::hash;
use solana_program::{account_info::AccountInfo, rent::Rent, sysvar::SysvarSerialize};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};

use super::{
    AccountEvidence, AnyResult, BootstrapError, Rpc, RpcAccount, TransactionEvidence,
    account_evidence, fail, system_create_account,
};

const FRESHNESS_MARGIN_SECONDS: u32 = 300;
const RESOLUTION_BOUNTY_LAMPORTS: u64 = 100_000;

pub(super) struct LocalProviderFacts {
    pub(super) release: PythReleaseV1,
    pub(super) release_id: [u8; 32],
    pub(super) receiver_slot: u64,
    pub(super) router_slot: u64,
}

pub(super) struct OpenedFoundation {
    pub(super) market: Pubkey,
    pub(super) fund: Pubkey,
    pub(super) material: Pubkey,
    pub(super) material_cursor: Pubkey,
    pub(super) custody: Pubkey,
    pub(super) vault: Pubkey,
    pub(super) child_count: u64,
    pub(super) record_evidence: BTreeMap<String, FinalizedRecordEvidence>,
}

#[derive(Serialize)]
pub(super) struct SourceBootstrapEvidence {
    pub(super) chain_derived_provider_release: bool,
    pub(super) provider_release_sha256: String,
    pub(super) receiver_deployment_slot: u64,
    pub(super) router_deployment_slot: u64,
    pub(super) local_clock_unix_timestamp: i64,
    pub(super) fixture_publish_time: i64,
    pub(super) configured_max_age_seconds: u32,
    pub(super) immutable_records: BTreeMap<String, FinalizedRecordEvidence>,
    pub(super) market: AccountEvidence,
    pub(super) fund: AccountEvidence,
    pub(super) collateral_mint: AccountEvidence,
    pub(super) collateral_custody: AccountEvidence,
    pub(super) collateral_vault: AccountEvidence,
    pub(super) source_state: AccountEvidence,
    pub(super) source_update: String,
    pub(super) primary_inline_funding_boundary: &'static str,
    pub(super) market_resolved: bool,
    pub(super) source_terminal: bool,
    pub(super) source_update_reclaimed: bool,
}

/// Execute one complete local-only foundation and terminal Pyth Source route.
///
/// Every signing key is supplied by the caller or generated in this process.
/// Provider identity is authenticated from the localhost validator's Loader V3
/// accounts before it becomes SourceMaterial authority.
#[allow(clippy::too_many_arguments)]
pub(super) fn execute_integrated_source(
    rpc: &mut Rpc,
    program_id: Pubkey,
    payer: &Keypair,
    loader: Pubkey,
    receiver: Pubkey,
    receiver_programdata: Pubkey,
    config: Pubkey,
    encoded_vaa: Pubkey,
    router: Pubkey,
    router_programdata: Pubkey,
    treasury: Pubkey,
    token_program: Pubkey,
    system: Pubkey,
    rent: Pubkey,
    clock: Pubkey,
    compute_budget: Pubkey,
    fixture_publish_time: i64,
    post_update_body: &[u8],
    transactions: &mut Vec<TransactionEvidence>,
) -> AnyResult<SourceBootstrapEvidence> {
    let provider = authenticate_local_provider_release(
        rpc,
        loader,
        receiver,
        receiver_programdata,
        config,
        router,
        router_programdata,
    )?;
    let collateral_mint = create_collateral_mint(rpc, payer, token_program, system, transactions)?;
    let planning = finalized_snapshot(
        rpc,
        &[payer.pubkey(), config, rent, clock],
        last_slot(transactions)?,
        clock,
    )?;
    let rent_value = decode_rent(
        planning
            .accounts
            .get(&rent)
            .and_then(Option::as_ref)
            .ok_or_else(|| BootstrapError("planning snapshot lacks Rent sysvar".into()))?,
        rent,
    )?;
    let config_account = planning.required(config, "receiver config")?;
    let receiver_config = ReceiverConfigV2View::parse(&config_account.data)
        .map_err(|error| BootstrapError(format!("invalid receiver config: {error:?}")))?;
    let max_age_seconds = freshness_age(planning.observation.unix_timestamp, fixture_publish_time)?;

    let product_capacity_profile = CapacityProfileV1::new(CapacityProfileV1Input {
        envelope: ProductCapacityEnvelope::Measured,
        verifier_release_id: product_id(b"dclutch/local-validator/product-verifier/v1")?,
        envelope_basis_id: product_id(b"dclutch/local-validator/product-measurement/v1")?,
        max_artifact_bytes: 256,
        page_payload_bytes: 64,
        max_pages: 4,
        max_partition_cells: 2,
    })
    .map_err(|error| BootstrapError(format!("invalid Product capacity: {error:?}")))?;
    let coordinate_domain = product_id(b"dclutch/local-validator/pyth-price-atoms/v1")?;
    let result_unit = product_id(b"dclutch/local-validator/pyth-price-atom-unit/v1")?;
    let result_domain = FiniteResultDomainV1::new(coordinate_domain, result_unit, 1, &[])
        .map_err(|error| BootstrapError(format!("invalid finite result domain: {error:?}")))?;
    let source_capacity_profile = SourceCapacityProfileV1::new(
        SourceCapacityEnvelope::Measured,
        1,
        0,
        source_id(b"dclutch/local-validator/pyth-source-verifier/v1")?,
        source_id(b"dclutch/local-validator/pyth-source-measurement/v1")?,
        u32::try_from(NORMALIZED_EVIDENCE_BYTES)?,
        0,
    )
    .map_err(|error| BootstrapError(format!("invalid Source capacity: {error:?}")))?;
    let provider_release = ProviderReleaseV1::new(
        source_id(b"dclutch/provider-family/pyth/v1")?,
        SourceContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1)
            .map_err(|error| BootstrapError(format!("invalid Pyth extension ID: {error:?}")))?,
        SourceContentId::new(provider.release_id)
            .map_err(|error| BootstrapError(format!("invalid provider release ID: {error:?}")))?,
        SourceContentId::new(provider.release.price_update_codec_id())
            .map_err(|error| BootstrapError(format!("invalid provider codec ID: {error:?}")))?,
        SourceContentId::new(provider.release.adapter_id())
            .map_err(|error| BootstrapError(format!("invalid provider transport ID: {error:?}")))?,
    );
    let pyth_adapter_config = PythAdapterConfigV1::new([0x2a; 32], -8, 100)
        .map_err(|error| BootstrapError(format!("invalid Pyth adapter config: {error:?}")))?;
    let terms_id = product_id(b"dclutch/local-validator/binary-terminal-price-terms/v1")?;
    let occurrence_id = product_id(b"dclutch/local-validator/captured-vaa-occurrence/v1")?;
    let schedule_id = source_id(b"dclutch/local-validator/captured-terminal-schedule/v1")?;
    let evaluator_release_id = source_id(b"dclutch/local-validator/terminal-sample-evaluator/v1")?;
    let artifact_input = TerminalPythArtifactInputV1 {
        product_capacity_profile,
        terms_id,
        occurrence_id,
        result_domain,
        source_capacity_profile,
        provider_release,
        pyth_adapter_config,
        target_unix_seconds: fixture_publish_time,
        max_age_seconds,
        max_future_skew_seconds: 60,
        schedule_id,
        evaluator_release_id,
    };
    let artifacts = compile_terminal_pyth_artifacts_v1(&artifact_input)
        .map_err(|error| BootstrapError(format!("artifact compiler refused: {error:?}")))?;
    let fund_rent = rent_value.minimum_balance(dclutch_pyth_contract::funding::FUNDING_BYTES);
    let capability_manifest =
        resolution_manifest(&artifacts.source_material, fund_rent, receiver_config.fee())?;
    let adapter_release = dclutch_token_svm::CollateralAdapterReleaseV1::legacy_exact_transfer();
    if adapter_release.token_program() != token_program.to_bytes() {
        return fail("loaded legacy Token program differs from the selected collateral adapter");
    }
    let realm = RealmV1::new(RealmV1Input {
        token_program: token_program.to_bytes(),
        collateral_mint: collateral_mint.pubkey().to_bytes(),
        collateral_adapter_release_id: hash(&adapter_release.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .map_err(|error| BootstrapError(format!("invalid collateral Realm: {error:?}")))?;
    let plan = compile_terminal_pyth_creation_v1(&TerminalPythCreationInputV1 {
        program_id,
        sponsor: payer.pubkey(),
        realm,
        product_capacity_profile,
        terms_id,
        occurrence_id,
        result_domain,
        source_capacity_profile,
        provider_release,
        pyth_adapter_config,
        target_unix_seconds: fixture_publish_time,
        max_age_seconds,
        max_future_skew_seconds: 60,
        schedule_id,
        evaluator_release_id,
        capability_manifest,
        rent: rent_value,
        current_slot: planning.observation.slot,
    })
    .map_err(|error| BootstrapError(format!("creation compiler refused: {error:?}")))?;
    if plan.claim_basis != artifacts.claim_basis
        || plan.product_instance != artifacts.product_instance
        || plan.source_material != artifacts.source_material
    {
        return fail("two-phase semantic compilation did not converge exactly");
    }
    let rent_credit = plan.found.rent_credit_address;
    let foundation = execute_foundation(
        rpc,
        program_id,
        payer,
        rent_credit,
        collateral_mint.pubkey(),
        token_program,
        system,
        rent,
        clock,
        &plan.found,
        transactions,
    )?;
    let (source_state, source_update, market_resolved, source_terminal, source_update_reclaimed) =
        execute_source_resolution(
            rpc,
            program_id,
            payer,
            &foundation,
            rent_credit,
            receiver,
            receiver_programdata,
            config,
            encoded_vaa,
            router,
            router_programdata,
            treasury,
            system,
            rent,
            clock,
            compute_budget,
            post_update_body,
            transactions,
        )?;
    let evidence_account = |rpc: &mut Rpc, key, label| -> AnyResult<AccountEvidence> {
        let account = rpc.required_account(key, label)?;
        Ok(account_evidence(key, &account))
    };
    Ok(SourceBootstrapEvidence {
        chain_derived_provider_release: true,
        provider_release_sha256: super::hex(&provider.release_id),
        receiver_deployment_slot: provider.receiver_slot,
        router_deployment_slot: provider.router_slot,
        local_clock_unix_timestamp: planning.observation.unix_timestamp,
        fixture_publish_time,
        configured_max_age_seconds: max_age_seconds,
        immutable_records: foundation.record_evidence,
        market: evidence_account(rpc, foundation.market, "resolved Market")?,
        fund: evidence_account(rpc, foundation.fund, "resolution Fund")?,
        collateral_mint: evidence_account(rpc, collateral_mint.pubkey(), "collateral Mint")?,
        collateral_custody: evidence_account(rpc, foundation.custody, "collateral custody")?,
        collateral_vault: evidence_account(rpc, foundation.vault, "collateral vault")?,
        source_state: evidence_account(rpc, source_state, "terminal Source state")?,
        source_update: source_update.to_string(),
        primary_inline_funding_boundary: "the current primary-inline Source ABI makes the resolver fund receiver fee/update rent; the canonical Fund remains required and capitalized but is not debited on this route",
        market_resolved,
        source_terminal,
        source_update_reclaimed,
    })
}

fn product_id(preimage: &[u8]) -> AnyResult<ProductContentId> {
    ProductContentId::new(hash(preimage).to_bytes())
        .map_err(|error| Box::new(BootstrapError(format!("invalid Product ID: {error:?}"))) as _)
}

fn source_id(preimage: &[u8]) -> AnyResult<SourceContentId> {
    SourceContentId::new(hash(preimage).to_bytes())
        .map_err(|error| Box::new(BootstrapError(format!("invalid Source ID: {error:?}"))) as _)
}

#[derive(Serialize)]
pub(super) struct FinalizedRecordEvidence {
    schema_release_id_hex: String,
    content_id_hex: String,
    raw_record: AccountEvidence,
    staging_cursor_vacant: bool,
}

pub(super) struct Snapshot {
    pub(super) observation: Observation,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

impl Snapshot {
    pub(super) fn required(&self, key: Pubkey, label: &str) -> AnyResult<ObservedAccount> {
        let account = self
            .accounts
            .get(&key)
            .ok_or_else(|| BootstrapError(format!("snapshot omitted {label} {key}")))?
            .as_ref()
            .ok_or_else(|| BootstrapError(format!("snapshot lacks {label} {key}")))?;
        Ok(observed(self.observation, key, account))
    }

    pub(super) fn vacant_account(&self, key: Pubkey, label: &str) -> AnyResult<ObservedAccount> {
        match self.accounts.get(&key) {
            Some(None) => Ok(ObservedAccount {
                observation: self.observation,
                key,
                owner: Pubkey::default(),
                lamports: 0,
                executable: false,
                data: Vec::new(),
            }),
            Some(Some(account))
                if account.owner == Pubkey::default()
                    && !account.executable
                    && account.data.is_empty() =>
            {
                Ok(observed(self.observation, key, account))
            }
            Some(Some(_)) => fail(format!("{label} {key} is not vacant")),
            None => fail(format!("snapshot omitted {label} {key}")),
        }
    }

    pub(super) fn is_vacant(&self, key: Pubkey) -> bool {
        self.accounts.get(&key).is_some_and(|account| {
            account.as_ref().is_none_or(|account| {
                account.owner == Pubkey::default() && !account.executable && account.data.is_empty()
            })
        })
    }
}

pub(super) fn finalized_snapshot(
    rpc: &mut Rpc,
    keys: &[Pubkey],
    minimum_slot: u64,
    clock_key: Pubkey,
) -> AnyResult<Snapshot> {
    if !keys.contains(&clock_key) {
        return fail("finalized snapshot must include the Clock sysvar");
    }
    let (slot, values) = rpc.finalized_accounts(keys, minimum_slot)?;
    let mut accounts = BTreeMap::new();
    for (key, value) in keys.iter().copied().zip(values) {
        if accounts.insert(key, value).is_some() {
            return fail(format!("duplicate finalized-snapshot key {key}"));
        }
    }
    let clock = accounts
        .get(&clock_key)
        .and_then(Option::as_ref)
        .ok_or_else(|| BootstrapError("finalized snapshot lacks Clock sysvar".into()))?;
    let unix_timestamp = super::i64_le(&clock.data, 32, "Clock.unix_timestamp")?;
    Ok(Snapshot {
        observation: Observation {
            slot,
            unix_timestamp,
            finality: Finality::Finalized,
        },
        accounts,
    })
}

pub(super) fn freshness_age(
    clock_unix_timestamp: i64,
    fixture_publish_time: i64,
) -> AnyResult<u32> {
    let elapsed = clock_unix_timestamp
        .checked_sub(fixture_publish_time)
        .ok_or_else(|| BootstrapError("fixture publish time is in the validator future".into()))?;
    let elapsed = u32::try_from(elapsed)
        .map_err(|_| BootstrapError("fixture age exceeds the V1 u32 window".into()))?;
    elapsed
        .checked_add(FRESHNESS_MARGIN_SECONDS)
        .ok_or_else(|| Box::new(BootstrapError("freshness margin overflow".into())) as _)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn authenticate_local_provider_release(
    rpc: &mut Rpc,
    loader: Pubkey,
    receiver: Pubkey,
    receiver_programdata: Pubkey,
    config: Pubkey,
    router: Pubkey,
    router_programdata: Pubkey,
) -> AnyResult<LocalProviderFacts> {
    let marker = local_validator_release_v1()
        .map_err(|error| BootstrapError(format!("invalid local-provider release: {error:?}")))?;
    let release = *marker.release();
    for (actual, expected, label) in [
        (
            receiver.to_bytes(),
            release.receiver_program(),
            "receiver program",
        ),
        (
            receiver_programdata.to_bytes(),
            release.receiver_programdata(),
            "receiver ProgramData",
        ),
        (
            config.to_bytes(),
            release.receiver_config(),
            "receiver config",
        ),
        (
            router.to_bytes(),
            release.router_program(),
            "router program",
        ),
        (
            router_programdata.to_bytes(),
            release.router_programdata(),
            "router ProgramData",
        ),
    ] {
        if actual != expected {
            return fail(format!("chain address differs from local release: {label}"));
        }
    }
    let receiver_account = rpc.required_account(receiver, "receiver program")?;
    let receiver_data_account =
        rpc.required_account(receiver_programdata, "receiver ProgramData")?;
    let router_account = rpc.required_account(router, "router program")?;
    let router_data_account = rpc.required_account(router_programdata, "router ProgramData")?;
    let config_account = rpc.required_account(config, "receiver config")?;
    let receiver_view = ProgramV3View::parse(&receiver_account.data)
        .map_err(|error| BootstrapError(format!("invalid receiver Program: {error:?}")))?;
    let router_view = ProgramV3View::parse(&router_account.data)
        .map_err(|error| BootstrapError(format!("invalid router Program: {error:?}")))?;
    let receiver_data_view = ProgramDataV3View::parse(&receiver_data_account.data)
        .map_err(|error| BootstrapError(format!("invalid receiver ProgramData: {error:?}")))?;
    let router_data_view = ProgramDataV3View::parse(&router_data_account.data)
        .map_err(|error| BootstrapError(format!("invalid router ProgramData: {error:?}")))?;
    let receiver_config = ReceiverConfigV2View::parse(&config_account.data)
        .map_err(|error| BootstrapError(format!("invalid receiver config: {error:?}")))?;
    let receiver_slot = receiver_data_view.deployment_slot();
    let router_slot = router_data_view.deployment_slot();
    if receiver_account.owner != loader
        || !receiver_account.executable
        || receiver_data_account.owner != loader
        || receiver_data_account.executable
        || router_account.owner != loader
        || !router_account.executable
        || router_data_account.owner != loader
        || router_data_account.executable
        || receiver_view.programdata_key() != receiver_programdata.to_bytes()
        || router_view.programdata_key() != router_programdata.to_bytes()
        || Pubkey::find_program_address(&[receiver.as_ref()], &loader).0 != receiver_programdata
        || Pubkey::find_program_address(&[router.as_ref()], &loader).0 != router_programdata
        || receiver_slot != release.receiver_deployment_slot()
        || router_slot != release.router_deployment_slot()
        || config_account.owner != receiver
        || config_account.executable
        || <[u8; 32]>::from(sha2::Sha256::digest(&config_account.data)) != release.config_digest()
        || receiver_config.router_program() != release.router_program()
    {
        return fail("chain-derived provider facts do not satisfy the local release row");
    }
    Ok(LocalProviderFacts {
        release,
        release_id: hash(&release.to_bytes()).to_bytes(),
        receiver_slot,
        router_slot,
    })
}

pub(super) fn resolution_manifest(
    source_material: &[u8],
    fund_rent: u64,
    provider_fee: u64,
) -> AnyResult<Vec<u8>> {
    if provider_fee == 0 {
        return fail("real receiver configuration exposed a zero provider fee");
    }
    let material = SourceMaterialViewV1::decode(source_material)
        .map_err(|error| BootstrapError(format!("invalid SourceMaterial: {error:?}")))?;
    let (source_capacity_id, _) = material
        .capacity_profile()
        .map_err(|error| BootstrapError(format!("invalid Source capacity: {error:?}")))?;
    let native = |amount| {
        CompartmentFundingV1::native_lamports(amount)
            .map_err(|error| BootstrapError(format!("invalid native funding: {error:?}")))
    };
    let not_applicable = CompartmentFundingV1::not_applicable();
    let amounts = FundingAmountsV1::new(
        native(fund_rent)?,
        not_applicable,
        not_applicable,
        native(provider_fee)?,
        native(RESOLUTION_BOUNTY_LAMPORTS)?,
        not_applicable,
        not_applicable,
    )
    .map_err(|error| BootstrapError(format!("invalid resolution funding: {error:?}")))?;
    let quote = FundingQuoteV1::new(amounts, None)
        .map_err(|error| BootstrapError(format!("invalid resolution quote: {error:?}")))?;
    let capability_id = |preimage: &[u8]| {
        CapabilityContentId::new(hash(preimage).to_bytes())
            .map_err(|error| BootstrapError(format!("invalid capability ID: {error:?}")))
    };
    let entry = CapabilityEntryV1::new(
        capability_id(b"dclutch/local-validator/pyth-resolution-kind/v1")?,
        CapabilityContentId::new(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1)
            .map_err(|error| BootstrapError(format!("invalid Pyth release ID: {error:?}")))?,
        CapabilityContentId::new(hash(source_material).to_bytes())
            .map_err(|error| BootstrapError(format!("invalid material ID: {error:?}")))?,
        CapabilityContentId::new(source_capacity_id.to_bytes())
            .map_err(|error| BootstrapError(format!("invalid capacity ID: {error:?}")))?,
        capability_id(b"dclutch/local-validator/source-state-schema/v1")?,
        capability_id(b"dclutch/local-validator/source-state-derivation/v1")?,
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; MAX_DEPENDENCIES_PER_CAPABILITY],
        quote,
    )
    .map_err(|error| BootstrapError(format!("invalid resolution capability: {error:?}")))?;
    let mut bytes = vec![0; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[entry], &mut bytes)
        .map_err(|error| BootstrapError(format!("manifest encode failed: {error:?}")))?;
    Ok(bytes)
}

pub(super) fn create_collateral_mint(
    rpc: &mut Rpc,
    payer: &Keypair,
    token_program: Pubkey,
    system: Pubkey,
    transactions: &mut Vec<TransactionEvidence>,
) -> AnyResult<Keypair> {
    let mint = Keypair::new();
    let mint_rent = rpc.minimum_balance(dclutch_token_svm::MINT_BYTES)?;
    transactions.push(rpc.send_transaction(
        "create_real_legacy_collateral_mint",
        &[
            system_create_account(
                payer.pubkey(),
                mint.pubkey(),
                mint_rent,
                dclutch_token_svm::MINT_BYTES,
                token_program,
                system,
            )?,
            token_initialize_mint2(token_program, mint.pubkey(), payer.pubkey(), 6),
            token_remove_mint_authority(token_program, mint.pubkey(), payer.pubkey()),
        ],
        payer,
        &[&mint],
    )?);
    let account = rpc.required_account(mint.pubkey(), "collateral Mint")?;
    let parsed = dclutch_token_svm::Mint::parse(&account.data)
        .map_err(|error| BootstrapError(format!("invalid collateral Mint: {error:?}")))?;
    if account.owner != token_program
        || account.executable
        || account.lamports != mint_rent
        || !parsed.is_initialized
        || !parsed.mint_authority.is_none()
        || !parsed.freeze_authority.is_none()
        || parsed.supply != 0
        || parsed.decimals != 6
    {
        return fail("collateral Mint does not satisfy exact legacy-token Realm policy");
    }
    Ok(mint)
}

pub(super) fn decode_rent(account: &RpcAccount, key: Pubkey) -> AnyResult<Rent> {
    let mut lamports = account.lamports;
    let mut data = account.data.clone();
    let info = AccountInfo::new(
        &key,
        false,
        false,
        &mut lamports,
        &mut data,
        &account.owner,
        account.executable,
    );
    Rent::from_account_info(&info)
        .map_err(|error| Box::new(BootstrapError(format!("invalid Rent sysvar: {error:?}"))) as _)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn publish_record(
    rpc: &mut Rpc,
    program_id: Pubkey,
    payer: &Keypair,
    rent_credit: Pubkey,
    system: Pubkey,
    rent: Pubkey,
    clock: Pubkey,
    obligation: &CreationRecordObligationV1,
    transactions: &mut Vec<TransactionEvidence>,
) -> AnyResult<FinalizedRecordEvidence> {
    if rpc.account(obligation.raw_record)?.is_some()
        || rpc.account(obligation.staging_cursor)?.is_some()
    {
        return fail(format!(
            "immutable record destination {} or cursor {} already exists",
            obligation.raw_record, obligation.staging_cursor
        ));
    }
    let current_slot = rpc
        .call("getSlot", serde_json::json!([{"commitment":"confirmed"}]))?
        .as_u64()
        .ok_or_else(|| BootstrapError("getSlot result was not u64".into()))?;
    let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
    let page_bytes = usize::try_from(profile.page_bytes())?;
    let expiry_slot = current_slot
        .checked_add(profile.maximum_staging_lifetime_slots())
        .ok_or_else(|| BootstrapError("record expiry slot overflow".into()))?;
    let cleanup_bounty = rpc.minimum_balance(STAGING_CURSOR_BYTES_V1)?;
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(obligation.schema_release_id)
            .map_err(|error| BootstrapError(format!("invalid record schema: {error:?}")))?,
        ContentDigest::new(obligation.content_id)
            .map_err(|error| BootstrapError(format!("invalid record digest: {error:?}")))?,
    );
    let envelope = profile
        .page_envelope()
        .map_err(|error| BootstrapError(format!("invalid canonical page envelope: {error:?}")))?;
    let liveness = profile
        .staging_liveness_policy(cleanup_bounty)
        .map_err(|error| BootstrapError(format!("invalid canonical liveness: {error:?}")))?;
    let begin = BeginRecordV1::new(
        key,
        u64::try_from(obligation.content.len())?,
        envelope,
        liveness.policy_id(),
        expiry_slot,
        cleanup_bounty,
    )
    .map_err(|error| BootstrapError(format!("invalid Begin record: {error:?}")))?;
    transactions.push(rpc.send_transaction(
        format!("dclutch_record_begin_{:?}", obligation.kind),
        &[Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(obligation.raw_record, false),
                AccountMeta::new(obligation.staging_cursor, false),
                AccountMeta::new_readonly(rent_credit, false),
                AccountMeta::new_readonly(system, false),
                AccountMeta::new_readonly(rent, false),
                AccountMeta::new_readonly(clock, false),
            ],
            data: begin.to_bytes().to_vec(),
        }],
        payer,
        &[],
    )?);
    for (page_index, page) in obligation.content.chunks(page_bytes).enumerate() {
        let offset = page_index
            .checked_mul(page_bytes)
            .ok_or_else(|| BootstrapError("record page offset overflow".into()))?;
        let append = AppendPageV1::new(u64::try_from(page_index)?, u64::try_from(offset)?, page)
            .map_err(|error| BootstrapError(format!("invalid Append page: {error:?}")))?;
        let mut data = vec![
            0;
            append.encoded_len().map_err(|error| {
                BootstrapError(format!("invalid Append encoded length: {error:?}"))
            })?
        ];
        append
            .encode(&mut data)
            .map_err(|error| BootstrapError(format!("Append encode failed: {error:?}")))?;
        transactions.push(rpc.send_transaction(
            format!("dclutch_record_append_{:?}_{page_index}", obligation.kind),
            &[Instruction {
                program_id,
                accounts: vec![
                    AccountMeta::new_readonly(payer.pubkey(), true),
                    AccountMeta::new(obligation.raw_record, false),
                    AccountMeta::new(obligation.staging_cursor, false),
                ],
                data,
            }],
            payer,
            &[],
        )?);
    }
    transactions.push(rpc.send_transaction(
        format!("dclutch_record_finalize_{:?}", obligation.kind),
        &[Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new_readonly(obligation.raw_record, false),
                AccountMeta::new(obligation.staging_cursor, false),
                AccountMeta::new(rent_credit, false),
            ],
            data: FinalizeRecordV1.to_bytes().to_vec(),
        }],
        payer,
        &[],
    )?);
    let raw = rpc.required_account(obligation.raw_record, "finalized immutable record")?;
    if raw.owner != program_id || raw.executable || raw.data != obligation.content {
        return fail(format!(
            "finalized immutable record {} differs from its chain-derived obligation",
            obligation.raw_record
        ));
    }
    if rpc.account(obligation.staging_cursor)?.is_some() {
        return fail(format!(
            "staging cursor {} survived finalization",
            obligation.staging_cursor
        ));
    }
    Ok(FinalizedRecordEvidence {
        schema_release_id_hex: super::hex(&obligation.schema_release_id),
        content_id_hex: super::hex(&obligation.content_id),
        raw_record: account_evidence(obligation.raw_record, &raw),
        staging_cursor_vacant: true,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_foundation(
    rpc: &mut Rpc,
    program_id: Pubkey,
    payer: &Keypair,
    rent_credit: Pubkey,
    collateral_mint: Pubkey,
    token_program: Pubkey,
    system: Pubkey,
    rent: Pubkey,
    clock: Pubkey,
    plan: &ReleaseBoundCreationPlanV1,
    transactions: &mut Vec<TransactionEvidence>,
) -> AnyResult<OpenedFoundation> {
    if plan.rent_credit_address != rent_credit {
        return fail("creation plan selected a different sponsor RentCredit");
    }
    let mut record_evidence = BTreeMap::new();
    for obligation in &plan.records {
        let evidence = publish_record(
            rpc,
            program_id,
            payer,
            rent_credit,
            system,
            rent,
            clock,
            obligation,
            transactions,
        )?;
        record_evidence.insert(format!("{:?}", obligation.kind), evidence);
    }
    let realm = record(plan, CreationRecordKindV1::Realm)?;
    let instance = record(plan, CreationRecordKindV1::ProductInstance)?;
    let claim = record(plan, CreationRecordKindV1::ClaimBasis)?;
    let capacity = record(plan, CreationRecordKindV1::ProductCapacityProfile)?;
    let material = record(plan, CreationRecordKindV1::SourceMaterial)?;
    let manifest = record(plan, CreationRecordKindV1::CapabilityManifest)?;
    let found_keys = vec![
        payer.pubkey(),
        plan.market_address,
        plan.fund_address,
        rent_credit,
        realm.raw_record,
        realm.staging_cursor,
        instance.raw_record,
        instance.staging_cursor,
        claim.raw_record,
        claim.staging_cursor,
        capacity.raw_record,
        capacity.staging_cursor,
        material.raw_record,
        material.staging_cursor,
        manifest.raw_record,
        manifest.staging_cursor,
        system,
        rent,
        clock,
    ];
    let snapshot = finalized_snapshot(rpc, &found_keys, last_slot(transactions)?, clock)?;
    if !snapshot.is_vacant(plan.market_address) || !snapshot.is_vacant(plan.fund_address) {
        return fail("finalized snapshot found occupied Market or Fund destination");
    }
    let found = build_found_market_and_fund_v1(
        program_id,
        &FoundMarketState {
            sponsor: snapshot.required(payer.pubkey(), "sponsor")?,
            market_destination: ObservedVacancy {
                key: plan.market_address,
                observation: snapshot.observation,
            },
            fund_destination: ObservedVacancy {
                key: plan.fund_address,
                observation: snapshot.observation,
            },
            rent_credit: snapshot.required(rent_credit, "RentCredit")?,
            realm: snapshot.required(realm.raw_record, "Realm record")?,
            realm_finalization: finalization_proof(&snapshot, realm)?,
            product_instance: snapshot.required(instance.raw_record, "Product Instance record")?,
            product_instance_finalization: finalization_proof(&snapshot, instance)?,
            claim_basis: snapshot.required(claim.raw_record, "ClaimBasis record")?,
            claim_basis_finalization: finalization_proof(&snapshot, claim)?,
            capacity_profile: snapshot.required(capacity.raw_record, "capacity record")?,
            capacity_profile_finalization: finalization_proof(&snapshot, capacity)?,
            resolution_material: snapshot.required(material.raw_record, "SourceMaterial record")?,
            resolution_material_finalization: finalization_proof(&snapshot, material)?,
            capability_manifest: snapshot.required(manifest.raw_record, "manifest record")?,
            capability_manifest_finalization: finalization_proof(&snapshot, manifest)?,
            system_program: snapshot.required(system, "System Program")?,
            rent_sysvar: snapshot.required(rent, "Rent sysvar")?,
        },
    )
    .map_err(|error| BootstrapError(format!("chain-derived Found builder refused: {error:?}")))?;
    if found.market_address != plan.market_address || found.fund_address != plan.fund_address {
        return fail("chain-derived Found destinations differ from the semantic plan");
    }
    transactions.push(rpc.send_transaction(
        "dclutch_found_market_and_fund",
        &[found.instruction],
        payer,
        &[],
    )?);
    let market_after_found = rpc.required_account(plan.market_address, "founded Market")?;
    let found_child_count = market_child_count(&market_after_found.data)?;
    let generation = found.identity.generation();
    let generation_seed = generation.to_le_bytes();
    let readiness = Pubkey::find_program_address(
        &[
            MARKET_OPENING_READINESS_PDA_DOMAIN,
            plan.market_address.as_ref(),
            generation_seed.as_slice(),
        ],
        &program_id,
    )
    .0;
    let begin = BeginMarketOpeningReadinessV1::new(generation, found_child_count);
    transactions.push(rpc.send_transaction(
        "dclutch_begin_market_opening_readiness",
        &[Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new(plan.market_address, false),
                AccountMeta::new(readiness, false),
                AccountMeta::new_readonly(manifest.raw_record, false),
                AccountMeta::new_readonly(rent_credit, false),
                AccountMeta::new_readonly(system, false),
                AccountMeta::new_readonly(rent, false),
            ],
            data: begin.to_bytes().to_vec(),
        }],
        payer,
        &[],
    )?);
    let readiness_account = rpc.required_account(readiness, "market readiness")?;
    let readiness_value = MarketOpeningReadinessV1::decode(&readiness_account.data)
        .map_err(|error| BootstrapError(format!("invalid readiness state: {error:?}")))?;
    if readiness_value.is_ready() {
        return fail("fresh one-entry readiness unexpectedly began complete");
    }
    let next_entry = readiness_value.next_entry_index();
    let advance = AdvanceMarketOpeningReadinessV1::new(generation, next_entry);
    transactions.push(rpc.send_transaction(
        "dclutch_advance_market_opening_readiness",
        &[Instruction {
            program_id,
            accounts: vec![
                AccountMeta::new(plan.market_address, false),
                AccountMeta::new(readiness, false),
                AccountMeta::new_readonly(manifest.raw_record, false),
                AccountMeta::new_readonly(plan.fund_address, false),
                AccountMeta::new_readonly(rent, false),
            ],
            data: advance.to_bytes().to_vec(),
        }],
        payer,
        &[],
    )?);
    let ready_account = rpc.required_account(readiness, "advanced readiness")?;
    let ready = MarketOpeningReadinessV1::decode(&ready_account.data)
        .map_err(|error| BootstrapError(format!("invalid advanced readiness: {error:?}")))?;
    if !ready.is_ready() || ready.next_entry_index() != ready.entry_count() {
        return fail("chain readiness did not complete the exact manifest");
    }
    let custody = Pubkey::find_program_address(
        &[COLLATERAL_CUSTODY_PDA_DOMAIN, plan.market_address.as_ref()],
        &program_id,
    )
    .0;
    let vault = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, plan.market_address.as_ref()],
        &program_id,
    )
    .0;
    let open_keys = vec![
        payer.pubkey(),
        plan.market_address,
        readiness,
        rent_credit,
        manifest.raw_record,
        manifest.staging_cursor,
        realm.raw_record,
        realm.staging_cursor,
        custody,
        vault,
        collateral_mint,
        token_program,
        system,
        rent,
        clock,
    ];
    let open_snapshot = finalized_snapshot(rpc, &open_keys, last_slot(transactions)?, clock)?;
    let open = build_open_collateral_vault_v1(
        program_id,
        &OpenCollateralVaultState {
            sponsor: open_snapshot.required(payer.pubkey(), "sponsor")?,
            market: open_snapshot.required(plan.market_address, "founded Market")?,
            readiness: open_snapshot.required(readiness, "ready child")?,
            rent_credit: open_snapshot.required(rent_credit, "RentCredit")?,
            capability_manifest: open_snapshot.required(manifest.raw_record, "manifest record")?,
            capability_manifest_finalization: finalization_proof(&open_snapshot, manifest)?,
            realm: open_snapshot.required(realm.raw_record, "Realm record")?,
            realm_finalization: finalization_proof(&open_snapshot, realm)?,
            custody_destination: open_snapshot.vacant_account(custody, "collateral custody")?,
            vault_destination: open_snapshot.vacant_account(vault, "collateral vault")?,
            collateral_mint: open_snapshot.required(collateral_mint, "collateral Mint")?,
            token_program: open_snapshot.required(token_program, "token program")?,
            system_program: open_snapshot.required(system, "System Program")?,
            rent_sysvar: open_snapshot.required(rent, "Rent sysvar")?,
        },
    )
    .map_err(|error| BootstrapError(format!("chain-derived Open builder refused: {error:?}")))?;
    if open.custody_address != custody || open.vault_address != vault {
        return fail("chain-derived Open destinations differ from canonical PDAs");
    }
    transactions.push(rpc.send_transaction(
        "dclutch_open_collateral_vault",
        &[open.instruction],
        payer,
        &[],
    )?);
    if rpc.account(readiness)?.is_some() {
        return fail("readiness child survived atomic Market open");
    }
    let market_after_open = rpc.required_account(plan.market_address, "open Market")?;
    let child_count = market_child_count(&market_after_open.data)?;
    Ok(OpenedFoundation {
        market: plan.market_address,
        fund: plan.fund_address,
        material: material.raw_record,
        material_cursor: material.staging_cursor,
        custody,
        vault,
        child_count,
        record_evidence,
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn execute_source_resolution(
    rpc: &mut Rpc,
    program_id: Pubkey,
    payer: &Keypair,
    foundation: &OpenedFoundation,
    rent_credit: Pubkey,
    receiver: Pubkey,
    receiver_programdata: Pubkey,
    config: Pubkey,
    encoded_vaa: Pubkey,
    router: Pubkey,
    router_programdata: Pubkey,
    treasury: Pubkey,
    system: Pubkey,
    rent: Pubkey,
    clock: Pubkey,
    compute_budget: Pubkey,
    post_update_body: &[u8],
    transactions: &mut Vec<TransactionEvidence>,
) -> AnyResult<(Pubkey, Pubkey, bool, bool, bool)> {
    let generation = FOUNDATION_GENERATION;
    let generation_seed = generation.to_le_bytes();
    let resolution_state = Pubkey::find_program_address(
        &[
            SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V1,
            foundation.market.as_ref(),
            generation_seed.as_slice(),
        ],
        &program_id,
    )
    .0;
    let create_keys = vec![
        payer.pubkey(),
        resolution_state,
        foundation.market,
        foundation.material,
        foundation.material_cursor,
        rent_credit,
        system,
        rent,
        clock,
    ];
    let create_snapshot = finalized_snapshot(rpc, &create_keys, last_slot(transactions)?, clock)?;
    let create = build_source_create_resolution_v1(
        program_id,
        &SourceCreateResolutionState {
            payer: create_snapshot.required(payer.pubkey(), "Source payer")?,
            resolution_state_destination: create_snapshot
                .vacant_account(resolution_state, "Source state destination")?,
            market: create_snapshot.required(foundation.market, "open Market")?,
            resolution_material: create_snapshot.required(foundation.material, "SourceMaterial")?,
            resolution_material_finalization: FinalizedRecordProof {
                schema_release_id: dclutch_source_contract::SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
                staging_cursor: create_snapshot
                    .vacant_account(foundation.material_cursor, "SourceMaterial cursor")?,
            },
            rent_credit: create_snapshot.required(rent_credit, "RentCredit")?,
            system_program: create_snapshot.required(system, "System Program")?,
            rent_sysvar: create_snapshot.required(rent, "Rent sysvar")?,
        },
    )
    .map_err(|error| BootstrapError(format!("chain-derived Source Create refused: {error:?}")))?;
    if create.resolution_state != resolution_state
        || create.expected_market_child_count != foundation.child_count
    {
        return fail("Source Create builder disagreed with the opened foundation");
    }
    transactions.push(rpc.send_transaction(
        "dclutch_create_source_resolution",
        &[create.instruction],
        payer,
        &[],
    )?);
    let created_state = rpc.required_account(resolution_state, "Source resolution state")?;
    let created = SourceResolutionStateV1::decode(&created_state.data)
        .map_err(|error| BootstrapError(format!("invalid created Source state: {error:?}")))?;
    if created.phase() != SourceResolutionPhaseV1::Primary
        || created.market() != foundation.market.to_bytes()
        || created.generation() != generation
    {
        return fail("created Source state does not bind the open Market");
    }
    let source_update = Keypair::new();
    let accept_keys = vec![
        resolution_state,
        foundation.market,
        foundation.material,
        foundation.material_cursor,
        rent,
        clock,
        payer.pubkey(),
        source_update.pubkey(),
        receiver,
        receiver_programdata,
        config,
        encoded_vaa,
        router,
        router_programdata,
        treasury,
        system,
    ];
    let accept_snapshot = finalized_snapshot(rpc, &accept_keys, last_slot(transactions)?, clock)?;
    let accept = build_source_accept_primary_inline_v1(
        program_id,
        &SourceAcceptPrimaryInlineState {
            resolution_state: accept_snapshot.required(resolution_state, "Source state")?,
            market: accept_snapshot.required(foundation.market, "open Market")?,
            resolution_material: accept_snapshot.required(foundation.material, "SourceMaterial")?,
            resolution_material_finalization: FinalizedRecordProof {
                schema_release_id: dclutch_source_contract::SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
                staging_cursor: accept_snapshot
                    .vacant_account(foundation.material_cursor, "SourceMaterial cursor")?,
            },
            rent_sysvar: accept_snapshot.required(rent, "Rent sysvar")?,
            clock_sysvar: accept_snapshot.required(clock, "Clock sysvar")?,
            resolver: accept_snapshot.required(payer.pubkey(), "resolver")?,
            update: accept_snapshot.vacant_account(source_update.pubkey(), "provider update")?,
            receiver_program: accept_snapshot.required(receiver, "receiver program")?,
            receiver_programdata: accept_snapshot
                .required(receiver_programdata, "receiver ProgramData")?,
            receiver_config: accept_snapshot.required(config, "receiver config")?,
            encoded_vaa: accept_snapshot.required(encoded_vaa, "EncodedVAA")?,
            router_program: accept_snapshot.required(router, "router program")?,
            router_programdata: accept_snapshot
                .required(router_programdata, "router ProgramData")?,
            receiver_treasury: accept_snapshot.required(treasury, "receiver treasury")?,
            system_program: accept_snapshot.required(system, "System Program")?,
        },
        post_update_body,
    )
    .map_err(|error| BootstrapError(format!("chain-derived Source Accept refused: {error:?}")))?;
    transactions.push(rpc.send_transaction(
        "dclutch_accept_real_pyth_source_evidence",
        &[
            super::set_compute_unit_limit(compute_budget, 1_400_000),
            accept.instruction,
        ],
        payer,
        &[&source_update],
    )?);
    let source_update_reclaimed = rpc.account(source_update.pubkey())?.is_none();
    let terminal_account = rpc.required_account(resolution_state, "terminal Source state")?;
    let terminal = SourceResolutionStateV1::decode(&terminal_account.data)
        .map_err(|error| BootstrapError(format!("invalid terminal Source state: {error:?}")))?;
    let source_terminal = terminal.phase() == SourceResolutionPhaseV1::Resolved;
    let market_account = rpc.required_account(foundation.market, "resolved Market")?;
    let market_resolved = market_phase(&market_account.data)? == Phase::Resolved;
    if !source_update_reclaimed || !source_terminal || !market_resolved {
        return fail("real Pyth Source action did not reach all atomic postconditions");
    }
    Ok((
        resolution_state,
        source_update.pubkey(),
        market_resolved,
        source_terminal,
        source_update_reclaimed,
    ))
}

pub(super) fn finalization_proof(
    snapshot: &Snapshot,
    obligation: &CreationRecordObligationV1,
) -> AnyResult<FinalizedRecordProof> {
    Ok(FinalizedRecordProof {
        schema_release_id: obligation.schema_release_id,
        staging_cursor: snapshot.vacant_account(obligation.staging_cursor, "staging cursor")?,
    })
}

fn observed(observation: Observation, key: Pubkey, account: &RpcAccount) -> ObservedAccount {
    ObservedAccount {
        observation,
        key,
        owner: account.owner,
        lamports: account.lamports,
        executable: account.executable,
        data: account.data.clone(),
    }
}

fn record(
    plan: &ReleaseBoundCreationPlanV1,
    kind: CreationRecordKindV1,
) -> AnyResult<&CreationRecordObligationV1> {
    plan.records
        .iter()
        .find(|record| record.kind == kind)
        .ok_or_else(|| Box::new(BootstrapError(format!("creation plan omitted {kind:?}"))) as _)
}

fn last_slot(transactions: &[TransactionEvidence]) -> AnyResult<u64> {
    transactions
        .last()
        .map(|transaction| transaction.slot)
        .ok_or_else(|| Box::new(BootstrapError("transaction evidence is empty".into())) as _)
}

pub(super) fn market_child_count(data: &[u8]) -> AnyResult<u64> {
    let outcomes = decode_market_outcome_count(data)
        .map_err(|error| BootstrapError(format!("invalid Market width: {error:?}")))?;
    macro_rules! child_count {
        ($n:literal) => {
            CategoricalMarketV1::<$n>::decode(data)
                .map(|market| market.root().outstanding_children())
                .map_err(|error| {
                    Box::new(BootstrapError(format!("invalid Market body: {error:?}"))) as _
                })
        };
    }
    match outcomes {
        2 => child_count!(2),
        3 => child_count!(3),
        4 => child_count!(4),
        5 => child_count!(5),
        6 => child_count!(6),
        7 => child_count!(7),
        8 => child_count!(8),
        9 => child_count!(9),
        10 => child_count!(10),
        11 => child_count!(11),
        12 => child_count!(12),
        13 => child_count!(13),
        14 => child_count!(14),
        15 => child_count!(15),
        16 => child_count!(16),
        _ => fail(format!("unsupported Market outcome count {outcomes}")),
    }
}

pub(super) fn market_phase(data: &[u8]) -> AnyResult<Phase> {
    let outcomes = decode_market_outcome_count(data)
        .map_err(|error| BootstrapError(format!("invalid Market width: {error:?}")))?;
    macro_rules! phase {
        ($n:literal) => {
            CategoricalMarketV1::<$n>::decode(data)
                .map(|market| market.root().phase())
                .map_err(|error| {
                    Box::new(BootstrapError(format!("invalid Market body: {error:?}"))) as _
                })
        };
    }
    match outcomes {
        2 => phase!(2),
        3 => phase!(3),
        4 => phase!(4),
        5 => phase!(5),
        6 => phase!(6),
        7 => phase!(7),
        8 => phase!(8),
        9 => phase!(9),
        10 => phase!(10),
        11 => phase!(11),
        12 => phase!(12),
        13 => phase!(13),
        14 => phase!(14),
        15 => phase!(15),
        16 => phase!(16),
        _ => fail(format!("unsupported Market outcome count {outcomes}")),
    }
}

pub(super) fn token_initialize_mint2(
    token_program: Pubkey,
    mint: Pubkey,
    authority: Pubkey,
    decimals: u8,
) -> Instruction {
    let mut data = Vec::with_capacity(70);
    data.extend_from_slice(&[20, decimals]);
    data.extend_from_slice(authority.as_ref());
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&[0_u8; 32]);
    Instruction {
        program_id: token_program,
        accounts: vec![AccountMeta::new(mint, false)],
        data,
    }
}

pub(super) fn token_remove_mint_authority(
    token_program: Pubkey,
    mint: Pubkey,
    current_authority: Pubkey,
) -> Instruction {
    let mut data = Vec::with_capacity(38);
    data.extend_from_slice(&[6, 0]);
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(&[0_u8; 32]);
    Instruction {
        program_id: token_program,
        accounts: vec![
            AccountMeta::new(mint, false),
            AccountMeta::new_readonly(current_authority, true),
        ],
        data,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dclutch_record_contract::APPEND_PAGE_HEADER_BYTES_V1;

    #[test]
    fn local_clock_policy_refuses_future_and_overflow() {
        assert_eq!(freshness_age(1_000, 900).expect("bounded age"), 400);
        assert!(freshness_age(899, 900).is_err());
        assert!(freshness_age(i64::MAX, 0).is_err());
    }

    #[test]
    fn token_mint_wires_have_exact_legacy_shapes() {
        let program = Pubkey::new_from_array([1; 32]);
        let mint = Pubkey::new_from_array([2; 32]);
        let authority = Pubkey::new_from_array([3; 32]);
        let initialize = token_initialize_mint2(program, mint, authority, 6);
        assert_eq!(initialize.data.len(), 70);
        assert_eq!(&initialize.data[..2], &[20, 6]);
        assert_eq!(&initialize.data[2..34], authority.as_ref());
        assert_eq!(&initialize.data[34..38], &0_u32.to_le_bytes());
        let remove = token_remove_mint_authority(program, mint, authority);
        assert_eq!(remove.data.len(), 38);
        assert_eq!(&remove.data[..2], &[6, 0]);
        assert_eq!(&remove.data[2..6], &0_u32.to_le_bytes());
        assert!(remove.accounts[1].is_signer);
    }

    #[test]
    fn append_envelope_matches_committed_sbf_profile() {
        let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
        let page_bytes = usize::try_from(profile.page_bytes()).expect("host usize");
        let page = vec![7; page_bytes];
        let append = AppendPageV1::new(1, 768, &page).expect("exact page");
        assert_eq!(
            append.encoded_len().expect("bounded wire"),
            APPEND_PAGE_HEADER_BYTES_V1 + page_bytes
        );
        assert!(profile.validates_page_envelope(profile.page_envelope().expect("envelope")));
        let liveness = profile
            .staging_liveness_policy(1)
            .expect("positive cleanup bounty");
        assert!(profile.validates_staging_liveness_policy(liveness, 1));
    }

    #[test]
    fn source_capacity_measures_normalized_evidence_not_provider_account() {
        assert_eq!(NORMALIZED_EVIDENCE_BYTES, 208);
        assert_ne!(
            NORMALIZED_EVIDENCE_BYTES,
            dclutch_pyth_svm::FULL_PRICE_UPDATE_V2_LEN
        );
    }
}
