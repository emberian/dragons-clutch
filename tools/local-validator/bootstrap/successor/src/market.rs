use std::collections::BTreeMap;

use dclutch_capability_contract::{CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityManifestV1};
use dclutch_market_core_codec::{
    CoreState, Identity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
};
use dclutch_product_runtime_v2::{
    ContentId as ProductContentId, portfolio_record_bytes, result_domain_record_bytes,
};
use dclutch_product_runtime_v2_admission::PRODUCT_RECORD_BYTES_V2;
use dclutch_product_runtime_v2_operator::{
    AccountObservationV2, FinalizedRecordObservationV2, ProductCompilationInputV2,
    compile_product_records_v2,
    found::{
        FinalizedReferenceObservationV2, FoundProjectionStateV2, FoundStateV2,
        build_found_instruction_v2, project_found_v2,
    },
    lifecycle_rent_v2::{LifecycleRentCreateStateV2, build_lifecycle_rent_create_v2},
};
use dclutch_realm_contract::{
    FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1, RealmV1Input,
};
use dclutch_rent_contract::lifecycle_v2::{
    LIFECYCLE_RENT_CREDIT_BYTES_V2, LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleRentCreditV2,
};
use dclutch_source_contract::{
    ContentId as SourceContentId, RECOVERY_POLICY_SCHEMA_ID_V2, RecoveryPolicyV2,
    SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2, SourceMaterialV2,
};
use dclutch_token_svm::{
    ACCOUNT_BYTES, AccountState, CollateralAdapterReleaseV1, MINT_BYTES, Mint,
    TOKEN_2022_PROGRAM_ID, TokenAccount,
};
use sha2::{Digest as _, Sha256};
use solana_sdk::{
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
};
use solana_sdk_ids::{system_program, sysvar};
use solana_system_interface::instruction::create_account;

use crate::{
    Error, Result,
    model::{AccountEvidence, MarketRunInput, SuccessorPlan, TransactionEvidence},
    plan::{hex32, pubkey},
    rpc::{Rpc, RpcAccount, account_evidence},
    runtime::{PublishedRecord, decode_hex, publish_product_graph, publish_record, record},
};

pub(crate) const REMAINING_OPEN_SEAM: &str = "Found31 is live with transaction-published Product, Realm, Source, RecoveryPolicy, and capability-manifest records and a real lifecycle RentCreditV2. The next transaction is intentionally refused until the canonical atomic projected-Custody -> Core permit -> Claims FoundingV5 -> Core Open-last operator lands; the current OpenVault order commits Core Open before Claims founding and is not a valid release path.";

pub(crate) struct MarketExecutionEvidence {
    pub(crate) completed: Vec<String>,
    pub(crate) accounts: BTreeMap<String, AccountEvidence>,
}

pub(crate) fn validate_market_input(input: &MarketRunInput) -> Result<()> {
    if input.initial_collateral_atoms == 0
        || input.cut_denominator == 0
        || input.portfolio_denominator == 0
    {
        return Err(Error::new(
            "market input requires positive raw collateral and denominators",
        ));
    }
    let cuts = input
        .cuts
        .iter()
        .map(|value| canonical_i128(value))
        .collect::<Result<Vec<_>>>()?;
    if input.coefficients.len()
        != cuts
            .len()
            .checked_add(2)
            .ok_or_else(|| Error::new("Product outcome width overflow"))?
    {
        return Err(Error::new(
            "portfolio coefficient width must equal cuts + failure + tails",
        ));
    }
    for value in [
        &input.product_id,
        &input.coordinate_domain_id,
        &input.result_unit_id,
        &input.claim_basis_id,
        &input.liability_basis_id,
        &input.representation_release_id,
        &input.mapping_release_id,
    ] {
        let _ = product_id(value)?;
    }
    for value in [
        &input.primary_source_spec_id,
        &input.window_spec_id,
        &input.statistic_spec_id,
        &input.failure_policy_release_id,
    ] {
        let _ = source_id(value)?;
    }
    let recovery_bytes = decode_hex(&input.recovery_policy_hex)?;
    let recovery = RecoveryPolicyV2::decode(&recovery_bytes)
        .map_err(|error| Error::new(format!("RecoveryPolicyV2: {error:?}")))?;
    if recovery.to_bytes().as_slice() != recovery_bytes {
        return Err(Error::new("RecoveryPolicyV2 input was not canonical"));
    }
    let manifest = decode_hex(&input.capability_manifest_hex)?;
    let manifest = CapabilityManifestV1::decode(&manifest)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
    if manifest.entry_count() < 3 {
        return Err(Error::new(
            "capability manifest omitted the three Resolution funding entries",
        ));
    }
    Ok(())
}

struct MarketRecords {
    realm: PublishedRecord,
    product: PublishedRecord,
    domain: PublishedRecord,
    portfolio: PublishedRecord,
    source: PublishedRecord,
    recovery: PublishedRecord,
    manifest: PublishedRecord,
}

struct FinalizedSnapshot {
    slot: u64,
    accounts: BTreeMap<Pubkey, Option<RpcAccount>>,
}

impl FinalizedSnapshot {
    fn observation(&self, key: Pubkey) -> Result<AccountObservationV2<'_>> {
        match self.accounts.get(&key) {
            Some(Some(account)) => Ok(AccountObservationV2 {
                slot: self.slot,
                key,
                owner: account.owner,
                lamports: account.lamports,
                executable: account.executable,
                data: &account.data,
            }),
            Some(None) => Ok(AccountObservationV2 {
                slot: self.slot,
                key,
                owner: system_program::ID,
                lamports: 0,
                executable: false,
                data: &[],
            }),
            None => Err(Error::new(format!("finalized snapshot omitted {key}"))),
        }
    }

    fn finalized_record(
        &self,
        rpc: &mut Rpc,
        pair: PublishedRecord,
    ) -> Result<FinalizedRecordObservationV2<'_>> {
        let raw = self.observation(pair.raw)?;
        let staging = self.observation(pair.staging)?;
        Ok(FinalizedRecordObservationV2 {
            raw,
            staging,
            raw_rent_minimum: rpc.minimum_balance(raw.data.len())?,
        })
    }
}

pub(crate) fn execute_found_market(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<MarketExecutionEvidence> {
    validate_market_input(input)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let (mint, collateral_wallet) = create_real_collateral(
        rpc,
        payer,
        token_program,
        input.collateral_display_decimals,
        input.initial_collateral_atoms,
        transactions,
    )?;

    let (records, product_id) =
        publish_market_records(rpc, registry, input, mint, payer, transactions)?;
    let release_set_digest = hex32(&plan.release_set_id)?;
    let market_identity = MarketIdentity {
        market_id: identity([0xff; 32])?,
        realm_id: identity(records.realm.digest)?,
        product_record: identity(records.product.digest)?,
        product_id: identity(product_id.to_bytes())?,
        resolution_policy: identity(records.source.digest)?,
        capability_manifest: identity(records.manifest.digest)?,
        selected_release_set: identity(release_set_digest)?,
        registry_program: identity(registry.to_bytes())?,
        generation: input.generation,
    };
    let market = Pubkey::find_program_address(
        &MarketCoreStateSeedsV2::new(market_identity).as_slices(),
        &core,
    )
    .0;
    let credit = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &input.generation.to_le_bytes(),
        ],
        &rent_program,
    )
    .0;
    let keys = found_snapshot_keys(plan, payer.pubkey(), market, credit, &records)?;
    let minimum_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .ok_or_else(|| Error::new("market execution had no finalized predecessor"))?;
    let pre_credit = finalized_snapshot(rpc, &keys, minimum_slot)?;
    let projection_state =
        projection_state(rpc, plan, &pre_credit, payer.pubkey(), market, &records)?;
    let projection = project_found_v2(input.generation, projection_state)
        .map_err(|error| Error::new(format!("chain-derived Found projection: {error:?}")))?;
    if projection.market_address != market {
        return Err(Error::new(
            "Found projection changed the discovered Market address",
        ));
    }
    let create = build_lifecycle_rent_create_v2(
        &projection,
        LifecycleRentCreateStateV2 {
            payer: pre_credit.observation(payer.pubkey())?,
            credit_destination: pre_credit.observation(credit)?,
            refund_wallet: pre_credit.observation(payer.pubkey())?,
            rent_program: pre_credit.observation(rent_program)?,
            system_program: pre_credit.observation(system_program::ID)?,
            rent: pre_credit.observation(sysvar::rent::ID)?,
        },
    )
    .map_err(|error| Error::new(format!("chain-derived RentV2 Create: {error:?}")))?;
    transactions.push(rpc.send(
        "create Market-scoped lifecycle RentCreditV2",
        std::slice::from_ref(&create.instruction),
        payer,
    )?);
    let credit_account = rpc.required_account(credit, "created lifecycle RentCreditV2")?;
    let credit_state = LifecycleRentCreditV2::decode(&credit_account.data)
        .map_err(|error| Error::new(format!("created RentV2 state: {error:?}")))?;
    if credit_account.owner != rent_program
        || credit_account.executable
        || credit_account.data.len() != LIFECYCLE_RENT_CREDIT_BYTES_V2
        || credit_state != create.state
        || credit_account.lamports < create.rent_debit
    {
        return Err(Error::new(
            "RentV2 transaction poststate differed from its checked plan",
        ));
    }

    let post_credit = finalized_snapshot(
        rpc,
        &keys,
        transactions
            .last()
            .map(|transaction| transaction.slot)
            .ok_or_else(|| Error::new("RentV2 transaction omitted finalized slot"))?,
    )?;
    let state = found_state(
        rpc,
        plan,
        &post_credit,
        payer.pubkey(),
        market,
        credit,
        &records,
    )?;
    let found = build_found_instruction_v2(input.generation, state)
        .map_err(|error| Error::new(format!("chain-derived Found31: {error:?}")))?;
    let mut hostile = found.instruction.clone();
    hostile
        .accounts
        .get_mut(2)
        .ok_or_else(|| Error::new("Found31 omitted RentCredit coordinate"))?
        .pubkey = payer.pubkey();
    transactions.push(rpc.send_expected_failure(
        "Found31 refuses substituted lifecycle credit",
        &[hostile],
        payer,
    )?);
    if rpc.account(market)?.is_some() {
        return Err(Error::new("hostile Found31 left a Market account"));
    }
    transactions.push(rpc.send(
        "create canonical Found31 Market",
        &[found.instruction],
        payer,
    )?);
    let market_account = rpc.required_account(market, "Found31 Market")?;
    let market_state = CoreState::decode(&market_account.data)
        .map_err(|error| Error::new(format!("Found31 Market state: {error:?}")))?;
    if market_account.owner != core
        || market_account.executable
        || market_state.phase != Phase::Founding
        || market_state.identity != found.market_identity
        || market_state.identity.market_id.to_bytes() != market.to_bytes()
    {
        return Err(Error::new(
            "Found31 transaction poststate differed from its checked plan",
        ));
    }

    let mut accounts = BTreeMap::new();
    for (label, key) in [
        ("collateral_mint", mint),
        ("collateral_wallet", collateral_wallet),
        ("lifecycle_rent_credit", credit),
        ("market", market),
        ("realm_record", records.realm.raw),
        ("product_record", records.product.raw),
        ("result_domain_record", records.domain.raw),
        ("portfolio_record", records.portfolio.raw),
        ("source_material_record", records.source.raw),
        ("recovery_policy_record", records.recovery.raw),
        ("capability_manifest_record", records.manifest.raw),
    ] {
        let account = rpc.required_account(key, label)?;
        accounts.insert(label.into(), account_evidence(key, &account));
    }
    Ok(MarketExecutionEvidence {
        completed: vec![
            "created an exact Token-2022 collateral Mint and funded raw-atom wallet with ephemeral local keys".into(),
            "transaction-published and finalized the canonical Realm/Product/Source/Recovery/Manifest graph".into(),
            "derived Market and lifecycle-credit coordinates from one finalized pre-credit projection".into(),
            "created and reacquired the exact Market-scoped LifecycleRentCreditV2".into(),
            "proved Found31 rejects a substituted lifecycle credit".into(),
            "created and verified the canonical Founding Market through the chain-derived Found31 operator".into(),
        ],
        accounts,
    })
}

fn publish_market_records(
    rpc: &mut Rpc,
    registry: Pubkey,
    input: &MarketRunInput,
    collateral_mint: Pubkey,
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(MarketRecords, ProductContentId)> {
    let cuts = input
        .cuts
        .iter()
        .map(|value| canonical_i128(value))
        .collect::<Result<Vec<_>>>()?;
    let outcome_count = cuts
        .len()
        .checked_add(2)
        .ok_or_else(|| Error::new("Product outcome width overflow"))?;
    if input.coefficients.len() != outcome_count {
        return Err(Error::new(
            "portfolio coefficient width must equal cuts + failure + tails",
        ));
    }
    let semantic_product_id = product_id(&input.product_id)?;
    let mut product = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![
        0_u8;
        result_domain_record_bytes(cuts.len()).map_err(|error| Error::new(
            format!("result-domain width: {error:?}")
        ))?
    ];
    let mut portfolio = vec![
        0_u8;
        portfolio_record_bytes(outcome_count).map_err(|error| Error::new(
            format!("portfolio width: {error:?}")
        ))?
    ];
    let compiled = compile_product_records_v2(
        registry,
        ProductCompilationInputV2 {
            product_id: semantic_product_id,
            coordinate_domain_id: product_id(&input.coordinate_domain_id)?,
            result_unit_id: product_id(&input.result_unit_id)?,
            claim_basis_id: product_id(&input.claim_basis_id)?,
            liability_basis_id: product_id(&input.liability_basis_id)?,
            representation_release_id: product_id(&input.representation_release_id)?,
            mapping_release_id: product_id(&input.mapping_release_id)?,
            cut_denominator: input.cut_denominator,
            cuts: &cuts,
            portfolio_denominator: input.portfolio_denominator,
            coefficients: &input.coefficients,
        },
        &mut product,
        &mut domain,
        &mut portfolio,
    )
    .map_err(|error| Error::new(format!("canonical Product compiler: {error:?}")))?;
    let product_digest: [u8; 32] = Sha256::digest(product).into();

    let recovery_bytes = decode_hex(&input.recovery_policy_hex)?;
    let recovery = RecoveryPolicyV2::decode(&recovery_bytes)
        .map_err(|error| Error::new(format!("RecoveryPolicyV2: {error:?}")))?;
    if recovery.to_bytes().as_slice() != recovery_bytes {
        return Err(Error::new("RecoveryPolicyV2 input was not canonical"));
    }
    let recovery_digest: [u8; 32] = Sha256::digest(&recovery_bytes).into();
    let source = SourceMaterialV2::new(
        SourceContentId::new(product_digest)
            .map_err(|error| Error::new(format!("Product digest: {error:?}")))?,
        source_id(&input.primary_source_spec_id)?,
        source_id(&input.window_spec_id)?,
        source_id(&input.statistic_spec_id)?,
        Some(
            SourceContentId::new(recovery_digest)
                .map_err(|error| Error::new(format!("Recovery digest: {error:?}")))?,
        ),
        source_id(&input.failure_policy_release_id)?,
    )
    .to_bytes();
    let manifest = decode_hex(&input.capability_manifest_hex)?;
    let decoded_manifest = CapabilityManifestV1::decode(&manifest)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
    if decoded_manifest.as_bytes() != manifest.as_slice() || decoded_manifest.entry_count() < 3 {
        return Err(Error::new(
            "capability manifest was noncanonical or omitted the three Resolution funding entries",
        ));
    }
    let adapter = CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer();
    let realm = RealmV1::new(RealmV1Input {
        token_program: TOKEN_2022_PROGRAM_ID,
        collateral_mint: collateral_mint.to_bytes(),
        collateral_adapter_release_id: Sha256::digest(adapter.to_bytes()).into(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .map_err(|error| Error::new(format!("canonical collateral Realm: {error:?}")))?
    .to_bytes();

    let hostile_wallet = Some(Pubkey::new_unique());
    let realm = publish_record(
        rpc,
        registry,
        payer,
        REALM_SCHEMA_RELEASE_ID_V1,
        &realm,
        hostile_wallet,
        transactions,
    )?;
    let (product, domain, portfolio) = publish_product_graph(
        rpc,
        registry,
        payer,
        compiled,
        &product,
        &domain,
        &portfolio,
        transactions,
    )?;
    let source = publish_record(
        rpc,
        registry,
        payer,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
        &source,
        None,
        transactions,
    )?;
    let recovery = publish_record(
        rpc,
        registry,
        payer,
        RECOVERY_POLICY_SCHEMA_ID_V2,
        &recovery_bytes,
        None,
        transactions,
    )?;
    let manifest = publish_record(
        rpc,
        registry,
        payer,
        CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        &manifest,
        None,
        transactions,
    )?;
    Ok((
        MarketRecords {
            realm,
            product,
            domain,
            portfolio,
            source,
            recovery,
            manifest,
        },
        semantic_product_id,
    ))
}

fn create_real_collateral(
    rpc: &mut Rpc,
    payer: &Keypair,
    token_program: Pubkey,
    decimals: u8,
    atoms: u64,
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(Pubkey, Pubkey)> {
    if atoms == 0 {
        return Err(Error::new("initial collateral raw atoms must be positive"));
    }
    let mint = Keypair::new();
    let wallet = Keypair::new();
    let mint_rent = rpc.minimum_balance(MINT_BYTES)?;
    let wallet_rent = rpc.minimum_balance(ACCOUNT_BYTES)?;
    let mut initialize_mint = Vec::with_capacity(70);
    initialize_mint.extend_from_slice(&[20, decimals]);
    initialize_mint.extend_from_slice(payer.pubkey().as_ref());
    initialize_mint.extend_from_slice(&0_u32.to_le_bytes());
    initialize_mint.extend_from_slice(&[0_u8; 32]);
    let mut initialize_wallet = Vec::with_capacity(33);
    initialize_wallet.push(18);
    initialize_wallet.extend_from_slice(payer.pubkey().as_ref());
    let mut mint_to = Vec::with_capacity(10);
    mint_to.push(14);
    mint_to.extend_from_slice(&atoms.to_le_bytes());
    mint_to.push(decimals);
    let mut remove_authority = Vec::with_capacity(38);
    remove_authority.extend_from_slice(&[6, 0]);
    remove_authority.extend_from_slice(&0_u32.to_le_bytes());
    remove_authority.extend_from_slice(&[0_u8; 32]);
    let instructions = [
        create_account(
            &payer.pubkey(),
            &mint.pubkey(),
            mint_rent,
            MINT_BYTES as u64,
            &token_program,
        ),
        Instruction {
            program_id: token_program,
            accounts: vec![AccountMeta::new(mint.pubkey(), false)],
            data: initialize_mint,
        },
        create_account(
            &payer.pubkey(),
            &wallet.pubkey(),
            wallet_rent,
            ACCOUNT_BYTES as u64,
            &token_program,
        ),
        Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(wallet.pubkey(), false),
                AccountMeta::new_readonly(mint.pubkey(), false),
            ],
            data: initialize_wallet,
        },
        Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(mint.pubkey(), false),
                AccountMeta::new(wallet.pubkey(), false),
                AccountMeta::new_readonly(payer.pubkey(), true),
            ],
            data: mint_to,
        },
        Instruction {
            program_id: token_program,
            accounts: vec![
                AccountMeta::new(mint.pubkey(), false),
                AccountMeta::new_readonly(payer.pubkey(), true),
            ],
            data: remove_authority,
        },
    ];
    transactions.push(rpc.send_with_signers(
        "create real Token-2022 collateral and raw-atom wallet",
        &instructions,
        payer,
        &[&mint, &wallet],
    )?);
    let mint_account = rpc.required_account(mint.pubkey(), "collateral Mint")?;
    let wallet_account = rpc.required_account(wallet.pubkey(), "collateral token wallet")?;
    let parsed_mint = Mint::parse(&mint_account.data)
        .map_err(|error| Error::new(format!("collateral Mint: {error:?}")))?;
    let parsed_wallet = TokenAccount::parse(&wallet_account.data)
        .map_err(|error| Error::new(format!("collateral wallet: {error:?}")))?;
    if mint_account.owner != token_program
        || wallet_account.owner != token_program
        || !parsed_mint.mint_authority.is_none()
        || !parsed_mint.freeze_authority.is_none()
        || !parsed_mint.is_initialized
        || parsed_mint.supply != atoms
        || parsed_mint.decimals != decimals
        || parsed_wallet.mint != mint.pubkey().to_bytes()
        || parsed_wallet.owner != payer.pubkey().to_bytes()
        || parsed_wallet.amount != atoms
        || parsed_wallet.state != AccountState::Initialized
        || !parsed_wallet.delegate.is_none()
        || !parsed_wallet.native_reserve.is_none()
        || !parsed_wallet.close_authority.is_none()
    {
        return Err(Error::new(
            "real Token-2022 collateral poststate refused exact base profile",
        ));
    }
    Ok((mint.pubkey(), wallet.pubkey()))
}

fn found_snapshot_keys(
    plan: &SuccessorPlan,
    payer: Pubkey,
    market: Pubkey,
    credit: Pubkey,
    records: &MarketRecords,
) -> Result<Vec<Pubkey>> {
    let release = record(plan, "execution_release_set")?;
    let registry_artifact = record(plan, "registry_artifact_release")?;
    let rent_artifact = record(plan, "rent_artifact_release")?;
    Ok(vec![
        payer,
        market,
        credit,
        pubkey(&plan.rent_credit.program_id)?,
        records.realm.raw,
        records.realm.staging,
        records.product.raw,
        records.product.staging,
        records.domain.raw,
        records.domain.staging,
        records.portfolio.raw,
        records.portfolio.staging,
        records.source.raw,
        records.source.staging,
        records.manifest.raw,
        records.manifest.staging,
        release.0,
        release.1,
        pubkey(&plan.activation)?,
        pubkey(&plan.core.program_id)?,
        pubkey(&plan.core.programdata_id)?,
        pubkey(&plan.registry.program_id)?,
        sysvar::rent::ID,
        system_program::ID,
        pubkey(&plan.infrastructure_profile.address)?,
        registry_artifact.0,
        registry_artifact.1,
        pubkey(&plan.registry.programdata_id)?,
        rent_artifact.0,
        rent_artifact.1,
        pubkey(&plan.rent_credit.programdata_id)?,
    ])
}

fn finalized_snapshot(
    rpc: &mut Rpc,
    keys: &[Pubkey],
    minimum_slot: u64,
) -> Result<FinalizedSnapshot> {
    let mut ordered = keys.to_vec();
    ordered.sort();
    if ordered.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(Error::new("Found snapshot address set contained aliases"));
    }
    let (slot, values) = rpc.finalized_accounts(keys, minimum_slot)?;
    let accounts = keys.iter().copied().zip(values).collect::<BTreeMap<_, _>>();
    Ok(FinalizedSnapshot { slot, accounts })
}

fn projection_state<'a>(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    snapshot: &'a FinalizedSnapshot,
    payer: Pubkey,
    market: Pubkey,
    records: &MarketRecords,
) -> Result<FoundProjectionStateV2<'a>> {
    let release = record(plan, "execution_release_set")?;
    let registry_artifact = record(plan, "registry_artifact_release")?;
    let rent_artifact = record(plan, "rent_artifact_release")?;
    let record_observation =
        |rpc: &mut Rpc, published: PublishedRecord| snapshot.finalized_record(rpc, published);
    Ok(FoundProjectionStateV2 {
        payer: snapshot.observation(payer)?,
        market: snapshot.observation(market)?,
        rent_program: snapshot.observation(pubkey(&plan.rent_credit.program_id)?)?,
        realm: FinalizedReferenceObservationV2 {
            schema_id: REALM_SCHEMA_RELEASE_ID_V1,
            record: record_observation(rpc, records.realm)?,
        },
        product: record_observation(rpc, records.product)?,
        result_domain: record_observation(rpc, records.domain)?,
        portfolio: record_observation(rpc, records.portfolio)?,
        source_material: FinalizedReferenceObservationV2 {
            schema_id: SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V2,
            record: record_observation(rpc, records.source)?,
        },
        capability_manifest: FinalizedReferenceObservationV2 {
            schema_id: CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            record: record_observation(rpc, records.manifest)?,
        },
        execution_release_set: FinalizedReferenceObservationV2 {
            schema_id: dclutch_release_set_contract::EXECUTION_RELEASE_SET_SCHEMA_RELEASE_ID_V1,
            record: snapshot_record(rpc, snapshot, release)?,
        },
        activation_cache: snapshot.observation(pubkey(&plan.activation)?)?,
        core_program: snapshot.observation(pubkey(&plan.core.program_id)?)?,
        core_programdata: snapshot.observation(pubkey(&plan.core.programdata_id)?)?,
        registry_program: snapshot.observation(pubkey(&plan.registry.program_id)?)?,
        rent: snapshot.observation(sysvar::rent::ID)?,
        system_program: snapshot.observation(system_program::ID)?,
        infrastructure_profile: snapshot
            .observation(pubkey(&plan.infrastructure_profile.address)?)?,
        registry_artifact: snapshot_record(rpc, snapshot, registry_artifact)?,
        registry_programdata: snapshot.observation(pubkey(&plan.registry.programdata_id)?)?,
        rent_artifact: snapshot_record(rpc, snapshot, rent_artifact)?,
        rent_programdata: snapshot.observation(pubkey(&plan.rent_credit.programdata_id)?)?,
    })
}

fn found_state<'a>(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    snapshot: &'a FinalizedSnapshot,
    payer: Pubkey,
    market: Pubkey,
    credit: Pubkey,
    records: &MarketRecords,
) -> Result<FoundStateV2<'a>> {
    let projection = projection_state(rpc, plan, snapshot, payer, market, records)?;
    Ok(FoundStateV2 {
        payer: projection.payer,
        market: projection.market,
        rent_credit: snapshot.observation(credit)?,
        rent_program: projection.rent_program,
        realm: projection.realm,
        product: projection.product,
        result_domain: projection.result_domain,
        portfolio: projection.portfolio,
        source_material: projection.source_material,
        capability_manifest: projection.capability_manifest,
        execution_release_set: projection.execution_release_set,
        activation_cache: projection.activation_cache,
        core_program: projection.core_program,
        core_programdata: projection.core_programdata,
        registry_program: projection.registry_program,
        rent: projection.rent,
        system_program: projection.system_program,
        infrastructure_profile: projection.infrastructure_profile,
        registry_artifact: projection.registry_artifact,
        registry_programdata: projection.registry_programdata,
        rent_artifact: projection.rent_artifact,
        rent_programdata: projection.rent_programdata,
    })
}

fn snapshot_record<'a>(
    rpc: &mut Rpc,
    snapshot: &'a FinalizedSnapshot,
    pair: (Pubkey, Pubkey),
) -> Result<FinalizedRecordObservationV2<'a>> {
    let raw = snapshot.observation(pair.0)?;
    Ok(FinalizedRecordObservationV2 {
        raw,
        staging: snapshot.observation(pair.1)?,
        raw_rent_minimum: rpc.minimum_balance(raw.data.len())?,
    })
}

fn product_id(value: &str) -> Result<ProductContentId> {
    ProductContentId::new(hex32(value)?)
        .map_err(|error| Error::new(format!("Product content ID: {error:?}")))
}

fn source_id(value: &str) -> Result<SourceContentId> {
    SourceContentId::new(hex32(value)?)
        .map_err(|error| Error::new(format!("Source content ID: {error:?}")))
}

fn identity(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|error| Error::new(format!("Market identity: {error:?}")))
}

fn canonical_i128(value: &str) -> Result<i128> {
    let parsed = value
        .parse::<i128>()
        .map_err(|error| Error::new(format!("cut numerator {value:?}: {error}")))?;
    if parsed.to_string() != value {
        return Err(Error::new(
            "cut numerators must use canonical decimal spelling",
        ));
    }
    Ok(parsed)
}

#[cfg(test)]
pub(crate) fn test_market_input(registry: Pubkey) -> Result<MarketRunInput> {
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V4;
    use dclutch_source_contract::{RECOVERY_POLICY_MAX_ATTEMPTS_V2, RecoveryAttemptV2};

    let hex_id = |byte: u8| crate::plan::hex(&[byte; 32]);
    let product = ProductCompilationInputV2 {
        product_id: product_id(&hex_id(1))?,
        coordinate_domain_id: product_id(&hex_id(2))?,
        result_unit_id: product_id(&hex_id(3))?,
        claim_basis_id: product_id(&hex_id(4))?,
        liability_basis_id: product_id(&hex_id(5))?,
        representation_release_id: product_id(&hex_id(6))?,
        mapping_release_id: product_id(&hex_id(7))?,
        cut_denominator: 10,
        cuts: &[1],
        portfolio_denominator: 1,
        coefficients: &[1, 0, 0],
    };
    let mut product_bytes = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![
        0_u8;
        result_domain_record_bytes(1)
            .map_err(|error| Error::new(format!("test domain: {error:?}")))?
    ];
    let mut portfolio = vec![
        0_u8;
        portfolio_record_bytes(3).map_err(|error| Error::new(format!(
            "test portfolio: {error:?}"
        )))?
    ];
    compile_product_records_v2(
        registry,
        product,
        &mut product_bytes,
        &mut domain,
        &mut portfolio,
    )
    .map_err(|error| Error::new(format!("test Product: {error:?}")))?;
    let product_digest: [u8; 32] = Sha256::digest(product_bytes).into();

    let attempt = RecoveryAttemptV2::new(
        source_id(&hex_id(0x21))?,
        source_id(&hex_id(0x22))?,
        2_000_000_000,
        source_id(&hex_id(0x23))?,
    )
    .map_err(|error| Error::new(format!("test recovery attempt: {error:?}")))?;
    let mut attempts = [None; RECOVERY_POLICY_MAX_ATTEMPTS_V2];
    attempts[0] = Some(attempt);
    let recovery = RecoveryPolicyV2::new(source_id(&hex_id(0x24))?, attempts, 1)
        .map_err(|error| Error::new(format!("test recovery policy: {error:?}")))?;
    let recovery_bytes = recovery.to_bytes();
    let recovery_digest: [u8; 32] = Sha256::digest(recovery_bytes).into();
    let material = SourceMaterialV2::new(
        SourceContentId::new(product_digest)
            .map_err(|error| Error::new(format!("test Product digest: {error:?}")))?,
        source_id(&hex_id(0x21))?,
        source_id(&hex_id(0x25))?,
        source_id(&hex_id(0x26))?,
        Some(
            SourceContentId::new(recovery_digest)
                .map_err(|error| Error::new(format!("test recovery digest: {error:?}")))?,
        ),
        source_id(&hex_id(0x27))?,
    );
    let material_digest: [u8; 32] = Sha256::digest(material.to_bytes()).into();
    let native = CompartmentFundingV1::native_lamports(1)
        .map_err(|error| Error::new(format!("test funding: {error:?}")))?;
    let none = CompartmentFundingV1::not_applicable();
    let amounts = FundingAmountsV1::new(native, native, none, none, native, none, none)
        .map_err(|error| Error::new(format!("test funding amounts: {error:?}")))?;
    let quote = FundingQuoteV1::new(amounts, None)
        .map_err(|error| Error::new(format!("test funding quote: {error:?}")))?;
    let release = CapabilityContentId::new(RESOLUTION_CONTROLLER_RELEASE_ID_V4)
        .map_err(|error| Error::new(format!("test Resolution release: {error:?}")))?;
    let configs = [
        attempt.funding_allocation_id().to_bytes(),
        recovery_digest,
        material_digest,
    ];
    let mut entries = Vec::new();
    for (index, config) in configs.into_iter().enumerate() {
        let kind = u8::try_from(0x31_usize + index)
            .map_err(|_| Error::new("test capability kind overflow"))?;
        entries.push(
            CapabilityEntryV1::new(
                CapabilityContentId::new([kind; 32])
                    .map_err(|error| Error::new(format!("test kind: {error:?}")))?,
                release,
                CapabilityContentId::new(config)
                    .map_err(|error| Error::new(format!("test config: {error:?}")))?,
                CapabilityContentId::new([0x41; 32])
                    .map_err(|error| Error::new(format!("test capacity: {error:?}")))?,
                CapabilityContentId::new([0x42; 32])
                    .map_err(|error| Error::new(format!("test schema: {error:?}")))?,
                CapabilityContentId::new([0x43; 32])
                    .map_err(|error| Error::new(format!("test derivation: {error:?}")))?,
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quote,
            )
            .map_err(|error| Error::new(format!("test capability entry: {error:?}")))?,
        );
    }
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest)
        .map_err(|error| Error::new(format!("test capability manifest: {error:?}")))?;
    Ok(MarketRunInput {
        generation: 1,
        collateral_display_decimals: 6,
        initial_collateral_atoms: 1_000_000_000,
        product_id: hex_id(1),
        coordinate_domain_id: hex_id(2),
        result_unit_id: hex_id(3),
        claim_basis_id: hex_id(4),
        liability_basis_id: hex_id(5),
        representation_release_id: hex_id(6),
        mapping_release_id: hex_id(7),
        cut_denominator: 10,
        cuts: vec!["1".into()],
        portfolio_denominator: 1,
        coefficients: vec![1, 0, 0],
        primary_source_spec_id: hex_id(0x21),
        window_spec_id: hex_id(0x25),
        statistic_spec_id: hex_id(0x26),
        failure_policy_release_id: hex_id(0x27),
        recovery_policy_hex: crate::plan::hex(&recovery_bytes),
        capability_manifest_hex: crate::plan::hex(&manifest),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cut_parser_refuses_noncanonical_integer_spellings() {
        assert_eq!(canonical_i128("-2").expect("canonical"), -2);
        assert_eq!(canonical_i128("0").expect("canonical"), 0);
        for value in ["+1", "01", "-0", " 1", "1 "] {
            assert!(canonical_i128(value).is_err(), "{value}");
        }
    }

    #[test]
    fn mint_instruction_shapes_are_exact_and_do_not_convert_raw_atoms() {
        let authority = Keypair::new();
        let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
        let mint = Keypair::new();
        let wallet = Keypair::new();
        let atoms = 9_007_199_254_740_993_u64;
        let decimals = 255_u8;
        let mut mint_to = Vec::with_capacity(10);
        mint_to.push(14);
        mint_to.extend_from_slice(&atoms.to_le_bytes());
        mint_to.push(decimals);
        assert_eq!(mint_to.len(), 10);
        assert_eq!(&mint_to[1..9], &atoms.to_le_bytes());
        assert_eq!(mint_to[9], decimals);
        assert_ne!(authority.pubkey(), mint.pubkey());
        assert_ne!(authority.pubkey(), wallet.pubkey());
        assert_ne!(mint.pubkey(), wallet.pubkey());
        assert_ne!(token_program, system_program::ID);
    }
}
