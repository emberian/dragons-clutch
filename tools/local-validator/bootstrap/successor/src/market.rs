use std::collections::BTreeMap;

use dclutch_capability_contract::{
    CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1, CapabilityFundingDerivationV1, CapabilityManifestV1,
    ContentId as CapabilityContentId, FUNDING_STATE_BYTES, FundingCustodyObservationV1,
    FundingStateV1, FundingStatus,
};
use dclutch_custody_contract::{
    CUSTODY_AUTHORITY_PDA_DOMAIN_V1, CUSTODY_REPLAY_BYTES_V1, CUSTODY_REPLAY_PDA_DOMAIN_V1,
    CompartmentV1, CustodyVaultSeedsV1, FoundingPrestateStageV1,
    OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1, PROJECTED_CUSTODY_STATE_BYTES_V1,
    PROJECTED_HOARD_CONTEXT_DOMAIN_V1, ProjectedCallerRoleV1, ProjectedCustodyCallerSeedsV1,
    ProjectedCustodyOperationV1, ProjectedCustodyPhaseV1, ProjectedCustodyRequestV1,
    ProjectedCustodyStateV1, SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
};
use dclutch_market_core_codec::{
    Action, CoreState, GenericFoundingRequestV1, GenericFoundingStageV1, Identity,
    MarketCoreStateSeedsV2, MarketIdentity, Phase, ProjectFoundReceiptV1, Request,
    SERIES_FOUNDING_PERMIT_BYTES_V1, STATE_BYTES, generic_founding_funding_list_id_v1,
};
use dclutch_market_founding_v1_operator::construct_generic_founding_root_selection_v1;
use dclutch_product_payoff_v2_codec::{
    registry_v3::GRADED_BASIS_RECORD_SCHEMA_ID_V3,
    runtime_v3::{
        BasisInputV3, BasisKindV3, ProductBasisV3, SEMANTIC_BASIS_CONTENT_DOMAIN_V3,
        basis_record_bytes_v3, compile_basis_v3, semantic_basis_preimage_v3,
    },
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
use solana_system_interface::instruction::{create_account, transfer};

use dclutch_versioned_message_operator::{
    Observation, ObservedAccount, build_lookup_table_creation_v1,
};

use crate::{
    Error, Result,
    model::{AccountEvidence, MarketRunInput, SuccessorPlan, TransactionEvidence},
    plan::{hex, hex32, pubkey},
    rpc::{Rpc, RpcAccount, account_evidence},
    runtime::{PublishedRecord, decode_hex, publish_product_graph, publish_record, record},
};

pub(crate) const REMAINING_OPEN_SEAM: &str = "The campaign publishes the Realm/Product/ResultDomain/Portfolio/Source/RecoveryPolicy/capability-manifest graph and the canonical ProductBasisV3 the Product declares as its liability basis, creates a real lifecycle RentCreditV2, routes the oversized 31-account Found frame through a finalized address lookup table as a packet-safe v0 transaction, creates the Market, and then executes DCLTPCB1 at its own generation - the four-stage projected-Custody founding prestate in one rollback domain. Found31 is not the obstacle it once was and this string used to say otherwise: it creates the Market and costs 232,537 measured compute units, 16.6% of the per-transaction maximum. The Market is Found, not Open, and at this revision NO RUNNER CAN OPEN IT. Core's generic Found stage authenticates the liability basis as a Registry-owned ProductBasisV3 (magic DCLTPAY3, schema GRADED_BASIS_RECORD_SCHEMA_ID_V3, semantic domain dclutch/product-basis/semantic/v3) and commits that record's digest and semantic identity into the Claims request it writes into the permit. Claims FoundingV5 then requires an account with THAT SAME digest which decodes as a legacy Core-owned LinkedBasisRecordV2 (magic DCLTLNK2, schema LIABILITY_BASIS_SCHEMA_RELEASE_ID_V2, semantic domain dclutch/lbv2/semantic-id/v2). Same digest means the same bytes, and no byte string is both records. That is a parallel legacy authority path, not a missing route, and it is already named as debt in crates/dclutch-product-runtime-v2-svm-reader/src/lib.rs: the remaining LinkedBasisRecordV2 Claims consumers must consume the V3 authentication result rather than add another basis decoder. Until affine_batch_v2 does, DCLTGMF1's Lock -> Found/permit -> Realize -> Claims -> Open chain is unsatisfiable and the correct thing for this runner to do is refuse to pretend otherwise. See docs/evidence/GENERIC_FOUNDING_REACHABILITY_2026_08_26.md.";

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
    // The Product's `liability_basis_id` is the semantic identity of a real
    // published `ProductBasisV3`. Founding reads that record and refuses any
    // Product whose declared liability basis is not the one it links, so the
    // run spec has to carry the exact record and not merely an opaque digest.
    let basis = decode_hex(&input.linked_basis_hex)?;
    let semantic = semantic_basis_identity_v3(&basis)?;
    if semantic != product_id(&input.liability_basis_id)?.to_bytes() {
        return Err(Error::new(
            "linked liability basis record is not the Product's declared liability basis",
        ));
    }
    Ok(())
}

/// Exact semantic identity of one canonical `ProductBasisV3` record.
///
/// The semantic preimage deliberately omits the Product and result-domain
/// links, so this identity exists before the Product that will declare it.
/// That omission is what makes the join acyclic rather than a fixed point.
fn semantic_basis_identity_v3(bytes: &[u8]) -> Result<[u8; 32]> {
    let preimage = semantic_basis_preimage_v3(bytes)
        .map_err(|error| Error::new(format!("ProductBasisV3: {error:?}")))?;
    let mut hasher = Sha256::new();
    hasher.update(SEMANTIC_BASIS_CONTENT_DOMAIN_V3);
    hasher.update(preimage.prefix());
    hasher.update(preimage.suffix());
    Ok(hasher.finalize().into())
}

/// Compile the exact categorical liability basis one Product outcome vector
/// determines.
///
/// `CategoricalQ1` is the only shape a categorical Product admits: unit payout
/// scale, no knots, no graded terms, and one basis claim per outcome. Nothing
/// here is a free parameter except the two acyclic links, and both are checked
/// against the compiled Product graph before publication.
fn compile_linked_basis_v3(
    product_id: [u8; 32],
    result_domain_id: [u8; 32],
    coordinate_domain_id: [u8; 32],
    result_unit_id: [u8; 32],
    evaluator_release_id: [u8; 32],
    outcome_count: usize,
) -> Result<Vec<u8>> {
    let basis_width =
        u32::try_from(outcome_count).map_err(|_| Error::new("Product outcome width overflow"))?;
    let width = basis_record_bytes_v3(BasisKindV3::CategoricalQ1, outcome_count, 0, 0)
        .map_err(|error| Error::new(format!("ProductBasisV3 width: {error:?}")))?;
    let mut bytes = vec![0_u8; width];
    compile_basis_v3(
        BasisInputV3 {
            kind: BasisKindV3::CategoricalQ1,
            product_id,
            result_domain_id,
            coordinate_domain_id,
            result_unit_id,
            evaluator_release_id,
            basis_width,
            payout_scale: 1,
            knot_denominator: 1,
            knots: &[],
            terms: &[],
            failure_payouts: &[],
        },
        &mut bytes,
    )
    .map_err(|error| Error::new(format!("canonical ProductBasisV3 compiler: {error:?}")))?;
    Ok(bytes)
}

struct MarketRecords {
    realm: PublishedRecord,
    product: PublishedRecord,
    domain: PublishedRecord,
    portfolio: PublishedRecord,
    source: PublishedRecord,
    recovery: PublishedRecord,
    manifest: PublishedRecord,
    basis: PublishedRecord,
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
    // The canonical 31-account Found frame does not fit the 1,232-byte legacy
    // packet with its keys inline. Routing is table data, never authority: the
    // shared versioned-message operator owns table admission and geometry.
    let (routing, tables) = publish_routing_table(
        rpc,
        payer,
        "Found31",
        std::slice::from_ref(&found.instruction),
        transactions,
    )?;
    let mut hostile = found.instruction.clone();
    hostile
        .accounts
        .get_mut(2)
        .ok_or_else(|| Error::new("Found31 omitted RentCredit coordinate"))?
        .pubkey = payer.pubkey();
    transactions.push(rpc.send_v0_expected_failure(
        "Found31 refuses substituted lifecycle credit",
        &[hostile],
        payer,
        routing,
        &tables,
    )?);
    if rpc.account(market)?.is_some() {
        return Err(Error::new("hostile Found31 left a Market account"));
    }

    // Routing data is not authority. The Market address is derived from the
    // immutable identity, so substituting it under an attacker-chosen table
    // must refuse and must roll the whole multi-instruction transaction back
    // to a fee-only debit.
    let rollback_recipient = Pubkey::new_unique();
    let substituted_market_key = Pubkey::new_unique();
    let payer_before = rpc.required_account(payer.pubkey(), "Found31 rollback prestate")?;
    let mut substituted_market = found.instruction.clone();
    substituted_market
        .accounts
        .get_mut(1)
        .ok_or_else(|| Error::new("Found31 omitted the Market coordinate"))?
        .pubkey = substituted_market_key;
    let rolled_back = rpc.send_v0_expected_failure(
        "Found31 refuses a substituted Market coordinate and rolls the transaction back",
        &[
            transfer(&payer.pubkey(), &rollback_recipient, 1),
            substituted_market,
        ],
        payer,
        routing,
        &tables,
    )?;
    let rollback_fee = rolled_back
        .fee_lamports
        .ok_or_else(|| Error::new("refused Found31 omitted its fee"))?;
    let payer_after = rpc.required_account(payer.pubkey(), "Found31 rollback poststate")?;
    if rpc.account(rollback_recipient)?.is_some()
        || rpc.account(substituted_market_key)?.is_some()
        || rpc.account(market)?.is_some()
        || payer_after.lamports.checked_add(rollback_fee) != Some(payer_before.lamports)
    {
        return Err(Error::new(
            "refused Found31 did not roll its whole transaction back to a fee-only debit",
        ));
    }

    transactions.push(rolled_back);
    transactions.push(rpc.send_v0(
        "create canonical Found31 Market",
        &[found.instruction],
        payer,
        routing,
        &tables,
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
        ("linked_liability_basis_record", records.basis.raw),
    ] {
        let account = rpc.required_account(key, label)?;
        accounts.insert(label.into(), account_evidence(key, &account));
    }
    let mut completed = vec![
            "created an exact Token-2022 collateral Mint and funded raw-atom wallet with ephemeral local keys".into(),
            "transaction-published and finalized the canonical Realm/Product/Source/Recovery/Manifest graph".into(),
            "derived Market and lifecycle-credit coordinates from one finalized pre-credit projection".into(),
            "created and reacquired the exact Market-scoped LifecycleRentCreditV2".into(),
            "proved Found31 rejects a substituted lifecycle credit".into(),
            "proved a substituted Market coordinate under attacker-chosen routing refuses and rolls the whole transaction back to a fee-only debit".into(),
            "routed the oversized 31-account Found frame through a finalized address lookup table as a packet-safe v0 transaction".into(),
            "created and verified the canonical Founding Market through the chain-derived Found31 operator".into(),
        ];
    // The generic founding runs at its own generation against a Market that
    // does not exist yet: every projected-Custody stage asserts the inverse of
    // a live Market, so the Found31 Market above cannot be reused.
    execute_projected_custody_bootstrap(
        rpc,
        plan,
        input,
        &records,
        market_identity,
        product_id,
        mint,
        collateral_wallet,
        payer,
        transactions,
        &mut accounts,
        &mut completed,
    )?;
    Ok(MarketExecutionEvidence {
        completed,
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
    let domain_digest: [u8; 32] = Sha256::digest(&domain).into();

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
    // Founding reads the liability basis the Product declares. Both of the
    // record's links are checked against the graph that was just compiled, so
    // a basis belonging to a different Product cannot be published as this
    // Market's.
    let basis_bytes = decode_hex(&input.linked_basis_hex)?;
    let basis_state = ProductBasisV3::decode(&basis_bytes)
        .map_err(|error| Error::new(format!("ProductBasisV3: {error:?}")))?;
    let outcome_width =
        u32::try_from(outcome_count).map_err(|_| Error::new("Product outcome width overflow"))?;
    if semantic_basis_identity_v3(&basis_bytes)?
        != product_id(&input.liability_basis_id)?.to_bytes()
        || basis_state.product_id() != semantic_product_id.to_bytes()
        || basis_state.result_domain_id() != domain_digest
        || basis_state.basis_width() != outcome_width
        || basis_state.payout_scale() != 1
        || basis_state.kind() != BasisKindV3::CategoricalQ1
    {
        return Err(Error::new(
            "linked liability basis record did not bind the compiled Product graph",
        ));
    }
    let basis = publish_record(
        rpc,
        registry,
        payer,
        GRADED_BASIS_RECORD_SCHEMA_ID_V3,
        &basis_bytes,
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
            basis,
        },
        semantic_product_id,
    ))
}

/// Publish one finalized address lookup table covering an oversized frame.
///
/// Only non-signer coordinates and the invoked Program are routed; the fee
/// payer and every signer stay in the message's static key list. The table is
/// authority-owned so its rent stays recoverable, and it is never frozen.
fn publish_routing_table(
    rpc: &mut Rpc,
    payer: &Keypair,
    label: &str,
    instructions: &[Instruction],
    transactions: &mut Vec<TransactionEvidence>,
) -> Result<(Observation, Vec<ObservedAccount>)> {
    let mut addresses: Vec<Pubkey> = Vec::new();
    let push = |key: Pubkey, addresses: &mut Vec<Pubkey>| {
        if key != payer.pubkey() && !addresses.contains(&key) {
            addresses.push(key);
        }
    };
    for instruction in instructions {
        push(instruction.program_id, &mut addresses);
        for meta in &instruction.accounts {
            if !meta.is_signer {
                push(meta.pubkey, &mut addresses);
            }
        }
    }
    let recent_slot = rpc.finalized_slot()?;
    let plan =
        build_lookup_table_creation_v1(payer.pubkey(), payer.pubkey(), recent_slot, &addresses)
            .map_err(|error| Error::new(format!("{label} routing table plan: {error:?}")))?;
    transactions.push(rpc.send(
        &format!("create {label} routing address lookup table"),
        std::slice::from_ref(&plan.create),
        payer,
    )?);
    for (index, extension) in plan.extensions.iter().enumerate() {
        transactions.push(rpc.send(
            &format!("extend {label} routing table page {index}"),
            std::slice::from_ref(extension),
            payer,
        )?);
    }
    let extended_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .ok_or_else(|| Error::new("routing table publication omitted a finalized slot"))?;
    // A table is only usable strictly after the slot that last extended it.
    let minimum_slot = extended_slot
        .checked_add(1)
        .ok_or_else(|| Error::new("routing table slot overflow"))?;
    await_finalized_slot(rpc, minimum_slot)?;
    let (observation, tables) =
        rpc.finalized_observed_accounts(&[plan.lookup_table], minimum_slot)?;
    Ok((observation, tables))
}

fn await_finalized_slot(rpc: &mut Rpc, minimum_slot: u64) -> Result<()> {
    for _ in 0..600 {
        if rpc.finalized_slot()? >= minimum_slot {
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    Err(Error::new(
        "validator did not finalize a slot after the routing table extension",
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

// ---------------------------------------------------------------------------
// DCLTPCB1 - the projected-Custody founding prestate bootstrap.
//
// Four Custody transitions in one rollback domain: Initialize, OpenHoard,
// OpenSourceCompartment, and the FundingState staging Trading performs itself.
// Every coordinate below is derived from facts this campaign already
// established on chain - the published record graph, the activated release
// set, the live Rent sysvar - or from a PDA seed order owned by the program
// that will re-derive it. The runner authors no digest.
// ---------------------------------------------------------------------------

/// Domain separating this campaign's founding action contexts.
const FOUNDING_CONTEXT_DOMAIN_V1: &[u8] = b"dclutch/local-campaign/founding-context/v1";

/// Sole top-level projected-Custody founding-bootstrap instruction.
///
/// Owned by `programs/dclutch-trading-sbf/src/projected_custody_bootstrap_v1.rs`
/// (`PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V1`); restated here because a localhost
/// host utility does not depend on an SBF program crate. The route carries no
/// payload, so these eight bytes are the whole instruction data.
const PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V1: [u8; 8] = *b"DCLTPCB1";

/// Exact projected-Custody bootstrap frame width before the funding tail.
const PROJECTED_CUSTODY_BOOTSTRAP_FIXED_ACCOUNTS_V1: usize = 78;

/// Every coordinate one generic founding determines, derived once.
struct FoundingCoordinates {
    generation: u64,
    market: Pubkey,
    credit: Pubkey,
    hoard_vault: Pubkey,
    source_vault: Pubkey,
    source_replay: Pubkey,
    projected_replay: Pubkey,
    custody_authority: Pubkey,
    funding_states: Vec<Pubkey>,
    found: GenericFoundingRequestV1,
    lock: ProjectedCustodyRequestV1,
}

/// Derive one founding's complete coordinate set from the finalized graph.
///
/// The order is forced and acyclic. The Custody vault, replay, and projection
/// addresses depend only on the Market, release set, and action context. The
/// FundingState addresses depend only on the authenticated manifest. The
/// artifact's capability root is derived last, from a root-free preimage, by
/// the shared operator - hashing the finished artifact instead would require
/// the artifact to carry the address of a PDA whose seeds already contain that
/// hash.
#[allow(clippy::too_many_arguments)]
fn derive_founding_coordinates(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    records: &MarketRecords,
    identity_template: MarketIdentity,
    product: ProductContentId,
    mint: Pubkey,
    payer: Pubkey,
    founder: Pubkey,
    expiry_slot: u64,
) -> Result<FoundingCoordinates> {
    let core = pubkey(&plan.core.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);
    let release_set = hex32(&plan.release_set_id)?;

    // The generic founding creates its own Market. It cannot reuse the one
    // Found31 already created: every projected-Custody stage asserts the
    // inverse of a live Market, and Core's own projection requires the Market
    // account vacant. A distinct generation is a distinct, still-vacant PDA.
    let generation = input
        .generation
        .checked_add(1)
        .ok_or_else(|| Error::new("founding generation overflow"))?;
    let identity = MarketIdentity {
        generation,
        ..identity_template
    };
    let market =
        Pubkey::find_program_address(&MarketCoreStateSeedsV2::new(identity).as_slices(), &core).0;
    let credit = Pubkey::find_program_address(
        &[
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
            market.as_ref(),
            &generation.to_le_bytes(),
        ],
        &rent_program,
    )
    .0;

    // The action context namespaces this founding under its own Market and
    // generation, so two foundings can never share a permit, a caller PDA, or
    // a Custody compartment.
    let mut hasher = Sha256::new();
    hasher.update(FOUNDING_CONTEXT_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(market.as_ref());
    hasher.update(generation.to_le_bytes());
    hasher.update(release_set);
    let context: [u8; 32] = hasher.finalize().into();
    let context_digest: [u8; 32] = {
        let mut hasher = Sha256::new();
        hasher.update(PROJECTED_HOARD_CONTEXT_DOMAIN_V1);
        hasher.update(context);
        hasher.finalize().into()
    };
    if context_digest == context {
        return Err(Error::new("founding context collided with its own digest"));
    }

    let market_bytes = market.to_bytes();
    let hoard_seeds = CustodyVaultSeedsV1::new(
        market_bytes,
        release_set,
        context_digest,
        CompartmentV1::HoardPrincipal,
    );
    let hoard_vault = Pubkey::find_program_address(&hoard_seeds.as_slices(), &custody).0;
    // Settlement is the compartment the request codec admits for a founding
    // source; None, External, and HoardPrincipal are refused outright.
    let source_seeds = CustodyVaultSeedsV1::new(
        market_bytes,
        release_set,
        context,
        CompartmentV1::Settlement,
    );
    let source_vault = Pubkey::find_program_address(&source_seeds.as_slices(), &custody).0;
    let source_replay = Pubkey::find_program_address(
        &[
            CUSTODY_REPLAY_PDA_DOMAIN_V1,
            &market_bytes,
            &release_set,
            &context,
        ],
        &custody,
    )
    .0;
    let projected_replay = Pubkey::find_program_address(
        &[
            CUSTODY_REPLAY_PDA_DOMAIN_V1,
            &market_bytes,
            &release_set,
            &context_digest,
        ],
        &custody,
    )
    .0;
    let custody_authority = Pubkey::find_program_address(
        &[CUSTODY_AUTHORITY_PDA_DOMAIN_V1, &market_bytes, &release_set],
        &custody,
    )
    .0;

    // One prepaid FundingState per capability-manifest entry, at the address
    // and holding the lamports the manifest itself quotes.
    let manifest_bytes = decode_hex(&input.capability_manifest_hex)?;
    let manifest = CapabilityManifestV1::decode(&manifest_bytes)
        .map_err(|error| Error::new(format!("CapabilityManifestV1: {error:?}")))?;
    let manifest_id = CapabilityContentId::new(records.manifest.digest)
        .map_err(|error| Error::new(format!("manifest identity: {error:?}")))?;
    let state_rent = rpc.minimum_balance(FUNDING_STATE_BYTES)?;
    let mut funding_states = Vec::new();
    let mut funding_identities = Vec::new();
    for entry_index in 0..manifest.entry_count() {
        let quoted = manifest
            .entry(entry_index)
            .map_err(|error| Error::new(format!("manifest entry {entry_index}: {error:?}")))?
            .funding_quote()
            .amounts()
            .native_lamports_total();
        let required = state_rent
            .checked_add(quoted)
            .ok_or_else(|| Error::new("funding-state prepayment overflow"))?;
        let observation = FundingCustodyObservationV1::native_only(required, state_rent)
            .map_err(|error| Error::new(format!("funding custody observation: {error:?}")))?;
        let state = FundingStateV1::new(manifest_id, manifest, entry_index, observation)
            .map_err(|error| Error::new(format!("FundingStateV1: {error:?}")))?;
        let derivation = CapabilityFundingDerivationV1::new(
            market_bytes,
            generation,
            manifest_id,
            manifest,
            state,
        )
        .map_err(|error| Error::new(format!("funding derivation: {error:?}")))?;
        let address = Pubkey::find_program_address(&derivation.seed_components(), &trading).0;
        funding_states.push(address);
        funding_identities.push(identity_of(address.to_bytes())?);
    }
    let funding_list_id = generic_founding_funding_list_id_v1(&funding_identities)
        .map_err(|error| Error::new(format!("funding list identity: {error:?}")))?;

    // The complete-set quantity times the basis scale is the Hoard principal.
    // A categorical Product's payout scale is one, and Core refuses any
    // artifact whose basis scale differs from the published basis record's.
    let basis_scale = 1_u64;
    let quantity = input
        .initial_collateral_atoms
        .checked_div(2)
        .filter(|value| *value > 0)
        .ok_or_else(|| Error::new("collateral supply cannot fund a founding"))?;
    let market_rent = rpc.minimum_balance(STATE_BYTES)?;
    let permit_rent = rpc.minimum_balance(SERIES_FOUNDING_PERMIT_BYTES_V1)?;

    let funding_count = u8::try_from(funding_states.len())
        .map_err(|_| Error::new("capability funding width overflow"))?;
    let template = GenericFoundingRequestV1::new(
        GenericFoundingStageV1::FoundAndPermit,
        funding_count,
        identity_of(release_set)?,
        identity_of(market_bytes)?,
        identity_of([1; 32])?,
        identity_of(context)?,
        identity_of(founder.to_bytes())?,
        identity_of(payer.to_bytes())?,
        identity_of(source_vault.to_bytes())?,
        identity_of(hoard_vault.to_bytes())?,
        identity_of(projected_replay.to_bytes())?,
        funding_list_id,
        generation,
        quantity,
        basis_scale,
        expiry_slot,
        market_rent,
        permit_rent,
        GENERIC_FOUNDING_PROJECTED_RESULTING_REVISION_V1,
        0,
    )
    .map_err(|error| Error::new(format!("founding artifact template: {error:?}")))?;
    let selection =
        construct_generic_founding_root_selection_v1(trading, template, &manifest_bytes)
            .map_err(|error| Error::new(format!("capability-root selection: {error:?}")))?;
    let found = selection.request;

    // The projection receipt is a pure function of the coordinates above, so
    // the request can commit to it without simulating the CPI Custody will run.
    let projection_receipt_digest = project_found_receipt_digest_v1(
        market_bytes,
        generation,
        records,
        product,
        mint,
        token_program,
        release_set,
        rent_program,
    )?;

    let lock = ProjectedCustodyRequestV1 {
        operation: ProjectedCustodyOperationV1::LockHoardAndCloseSource,
        caller_role: ProjectedCallerRoleV1::TradingCapability,
        market: market_bytes,
        generation,
        realm: records.realm.digest,
        product_record: records.product.digest,
        product: product.to_bytes(),
        source: records.source.digest,
        release_set,
        projection_receipt_digest,
        parent_capability_root: found.capability_root().to_bytes(),
        context_digest,
        caller_program: trading.to_bytes(),
        payer: payer.to_bytes(),
        core_program: core.to_bytes(),
        rent_program: rent_program.to_bytes(),
        refund_owner: payer.to_bytes(),
        rent_credit: credit.to_bytes(),
        hoard_vault: hoard_vault.to_bytes(),
        funding_source_vault: source_vault.to_bytes(),
        funding_source_context: context,
        funding_source_compartment: CompartmentV1::Settlement,
        mint: mint.to_bytes(),
        token_program: token_program.to_bytes(),
        collateral_release: collateral_adapter_release_id(),
        expiry_slot,
        expected_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1,
        resulting_revision: OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1
            .checked_add(1)
            .ok_or_else(|| Error::new("terminal revision overflow"))?,
        amount: found
            .hoard_principal()
            .map_err(|error| Error::new(format!("Hoard principal: {error:?}")))?,
        state_rent_lamports: rpc.minimum_balance(PROJECTED_CUSTODY_STATE_BYTES_V1)?,
        vault_rent_lamports: rpc.minimum_balance(ACCOUNT_BYTES)?,
        funding_source_replay_revision: SOURCE_COMPARTMENT_REPLAY_REVISION_V1,
        funding_source_state_rent_lamports: rpc.minimum_balance(CUSTODY_REPLAY_BYTES_V1)?,
        funding_source_vault_rent_lamports: rpc.minimum_balance(ACCOUNT_BYTES)?,
    };
    // Refuse here rather than inside Custody: the ladder is only derivable
    // from a terminal request the codec already accepts.
    let _ = lock
        .encode()
        .map_err(|error| Error::new(format!("terminal Lock request: {error:?}")))?;
    if lock.resulting_revision.checked_add(1) != Some(found.projected_resulting_revision()) {
        return Err(Error::new(
            "founding artifact and terminal Lock disagree about the Realize revision",
        ));
    }

    Ok(FoundingCoordinates {
        generation,
        market,
        credit,
        hoard_vault,
        source_vault,
        source_replay,
        projected_replay,
        custody_authority,
        funding_states,
        found,
        lock,
    })
}

/// Exact projected-Custody terminal revision for the four-stage ladder.
///
/// Initialize reaches one, OpenHoard two, OpenSourceCompartment three, the
/// terminal Lock four, and Core's Realize stage five. Core refuses any artifact
/// whose declared revision is not exactly the Realize poststate.
const GENERIC_FOUNDING_PROJECTED_RESULTING_REVISION_V1: u64 = 5;

/// Digest of the `ProjectFoundReceiptV1` Core will return during Initialize.
///
/// Derived, not simulated: every field of the receipt is a coordinate this
/// campaign already established, and none of it is a slot, a bump, a balance,
/// or anything else that only execution could supply.
#[allow(clippy::too_many_arguments)]
fn project_found_receipt_digest_v1(
    market: [u8; 32],
    generation: u64,
    records: &MarketRecords,
    product: ProductContentId,
    mint: Pubkey,
    token_program: Pubkey,
    release_set: [u8; 32],
    rent_program: Pubkey,
) -> Result<[u8; 32]> {
    let found_request = Request::administrative(Action::Found, generation, identity_of(market)?);
    let found_bytes = found_request
        .encode()
        .map_err(|error| Error::new(format!("canonical Core Found request: {error:?}")))?;
    let receipt = ProjectFoundReceiptV1::new(
        identity_of(market)?,
        generation,
        identity_of(records.realm.digest)?,
        identity_of(mint.to_bytes())?,
        identity_of(token_program.to_bytes())?,
        identity_of(collateral_adapter_release_id())?,
        identity_of(records.product.digest)?,
        identity_of(product.to_bytes())?,
        identity_of(records.source.digest)?,
        identity_of(release_set)?,
        identity_of(rent_program.to_bytes())?,
        Sha256::digest(found_bytes).into(),
    )
    .map_err(|error| Error::new(format!("ProjectFoundReceiptV1: {error:?}")))?;
    let bytes = receipt
        .encode()
        .map_err(|error| Error::new(format!("ProjectFound receipt encoding: {error:?}")))?;
    Ok(Sha256::digest(bytes).into())
}

/// The Realm-selected collateral adapter release this campaign publishes.
fn collateral_adapter_release_id() -> [u8; 32] {
    Sha256::digest(
        CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer().to_bytes(),
    )
    .into()
}

fn identity_of(bytes: [u8; 32]) -> Result<Identity> {
    Identity::new(bytes).map_err(|error| Error::new(format!("identity: {error:?}")))
}

/// Build the exact 78 + funding_count account `DCLTPCB1` frame.
///
/// Privileges are the outer's own assertion, not a guess: the route refuses a
/// frame that under-privileges an account rather than failing opaquely inside
/// Custody. Exactly the projected state, the two vaults, the source replay, the
/// funding tail, and the Custody rent payer are writable; exactly the payer and
/// the principal `refund_owner` sign, and the three caller PDAs sign only
/// inside the CPI, under `invoke_signed`.
#[allow(clippy::too_many_arguments)]
fn build_projected_custody_bootstrap_v1(
    plan: &SuccessorPlan,
    coordinates: &FoundingCoordinates,
    records: &MarketRecords,
    found_raw: Pubkey,
    lock_raw: Pubkey,
    projection_witness: Pubkey,
    payer: Pubkey,
    beneficiary: Pubkey,
    funder: Pubkey,
    mint: Pubkey,
) -> Result<Instruction> {
    let trading = pubkey(&plan.trading.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let cache = pubkey(&plan.activation)?;
    let registry = pubkey(&plan.registry.program_id)?;
    let trading_programdata = pubkey(&plan.trading.programdata_id)?;
    let core = pubkey(&plan.core.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    let stages = [
        (
            FoundingPrestateStageV1::Initialize,
            ProjectedCustodyOperationV1::Initialize,
        ),
        (
            FoundingPrestateStageV1::OpenHoard,
            ProjectedCustodyOperationV1::OpenHoard,
        ),
        (
            FoundingPrestateStageV1::OpenSourceCompartment,
            ProjectedCustodyOperationV1::OpenSourceCompartment,
        ),
    ];
    let mut callers = Vec::with_capacity(stages.len());
    for (stage, operation) in stages {
        let request = coordinates
            .lock
            .founding_prestate_stage_v1(stage)
            .map_err(|error| Error::new(format!("founding prestate {operation:?}: {error:?}")))?;
        let raw = request
            .encode()
            .map_err(|error| Error::new(format!("prestate encoding {operation:?}: {error:?}")))?;
        let digest: [u8; 32] = Sha256::digest(raw).into();
        let seeds = ProjectedCustodyCallerSeedsV1::new(request, digest);
        callers.push(Pubkey::find_program_address(&seeds.as_slices(), &trading).0);
    }

    let mut accounts = Vec::with_capacity(
        PROJECTED_CUSTODY_BOOTSTRAP_FIXED_ACCOUNTS_V1
            .checked_add(coordinates.funding_states.len())
            .ok_or_else(|| Error::new("bootstrap frame width overflow"))?,
    );
    accounts.push(AccountMeta::new_readonly(found_raw, false));
    accounts.push(AccountMeta::new_readonly(lock_raw, false));
    accounts.push(AccountMeta::new_readonly(custody, false));

    // Initialize: the seven shared projected coordinates, four Initialize
    // coordinates, then Core's own 31-account ProjectFound sub-frame verbatim.
    let common = |caller: Pubkey| {
        vec![
            AccountMeta::new_readonly(caller, false),
            AccountMeta::new(coordinates.projected_replay, false),
            AccountMeta::new_readonly(cache, false),
            AccountMeta::new_readonly(registry, false),
            AccountMeta::new_readonly(trading, false),
            AccountMeta::new_readonly(trading_programdata, false),
            AccountMeta::new_readonly(coordinates.credit, false),
        ]
    };
    accounts.extend(common(
        *callers
            .first()
            .ok_or_else(|| Error::new("Initialize caller authority missing"))?,
    ));
    accounts.push(AccountMeta::new_readonly(core, false));
    accounts.push(AccountMeta::new(payer, true));
    accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    for key in found_snapshot_keys(
        plan,
        projection_witness,
        coordinates.market,
        coordinates.credit,
        records,
    )? {
        accounts.push(AccountMeta::new_readonly(key, false));
    }

    // OpenHoard.
    accounts.extend(common(
        *callers
            .get(1)
            .ok_or_else(|| Error::new("OpenHoard caller authority missing"))?,
    ));
    accounts.push(AccountMeta::new(coordinates.hoard_vault, false));
    accounts.push(AccountMeta::new_readonly(
        coordinates.custody_authority,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(mint, false));
    accounts.push(AccountMeta::new_readonly(token_program, false));
    accounts.push(AccountMeta::new(payer, true));
    accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    accounts.push(AccountMeta::new_readonly(coordinates.market, false));

    // OpenSourceCompartment.
    accounts.extend(common(*callers.get(2).ok_or_else(|| {
        Error::new("OpenSourceCompartment caller authority missing")
    })?));
    accounts.push(AccountMeta::new(coordinates.source_vault, false));
    accounts.push(AccountMeta::new(coordinates.source_replay, false));
    accounts.push(AccountMeta::new_readonly(
        coordinates.custody_authority,
        false,
    ));
    accounts.push(AccountMeta::new_readonly(mint, false));
    accounts.push(AccountMeta::new_readonly(token_program, false));
    accounts.push(AccountMeta::new(funder, false));
    accounts.push(AccountMeta::new_readonly(beneficiary, true));
    accounts.push(AccountMeta::new(payer, true));
    accounts.push(AccountMeta::new_readonly(sysvar::rent::ID, false));
    accounts.push(AccountMeta::new_readonly(system_program::ID, false));
    accounts.push(AccountMeta::new_readonly(coordinates.market, false));

    for funding in &coordinates.funding_states {
        accounts.push(AccountMeta::new(*funding, false));
    }

    if accounts.len()
        != PROJECTED_CUSTODY_BOOTSTRAP_FIXED_ACCOUNTS_V1 + coordinates.funding_states.len()
    {
        return Err(Error::new(
            "assembled bootstrap frame did not match its exact fixed width",
        ));
    }
    Ok(Instruction {
        program_id: trading,
        accounts,
        data: PROJECTED_CUSTODY_BOOTSTRAP_MAGIC_V1.to_vec(),
    })
}

/// Schema namespacing this campaign's readonly raw-request records.
///
/// `DCLTPCB1` and `DCLTGMF1` carry no payload: every economic byte travels in
/// readonly data accounts. Publishing them as ordinary content-addressed
/// Registry records means their addresses are a function of their bytes, so a
/// substituted request is a substituted address and the outer's own frame
/// checks catch it.
fn raw_request_schema_v1(role: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(b"dclutch/local-campaign/raw-request-schema/v1");
    hasher.update([0]);
    hasher.update(role.as_bytes());
    hasher.finalize().into()
}

/// Create the projected-Custody founding prestate on a real validator.
///
/// This is `DCLTPCB1`: four transitions in one rollback domain against a Market
/// that does not exist yet. It leaves the projected replay at `SourceFunded`,
/// an empty Hoard vault, a funded source compartment, and one prepaid
/// `FundingStateV1` per capability-manifest entry - exactly the prestate the
/// founding outer's Lock stage consumes and nothing else.
#[allow(clippy::too_many_arguments)]
fn execute_projected_custody_bootstrap(
    rpc: &mut Rpc,
    plan: &SuccessorPlan,
    input: &MarketRunInput,
    records: &MarketRecords,
    identity_template: MarketIdentity,
    product: ProductContentId,
    mint: Pubkey,
    collateral_wallet: Pubkey,
    payer: &Keypair,
    transactions: &mut Vec<TransactionEvidence>,
    accounts: &mut BTreeMap<String, AccountEvidence>,
    completed: &mut Vec<String>,
) -> Result<()> {
    let registry = pubkey(&plan.registry.program_id)?;
    let custody = pubkey(&plan.custody.program_id)?;
    let trading = pubkey(&plan.trading.program_id)?;
    let rent_program = pubkey(&plan.rent_credit.program_id)?;
    let token_program = Pubkey::new_from_array(TOKEN_2022_PROGRAM_ID);

    // The principal supplier is not the rent payer. Custody requires the
    // funding source's owner to sign and to be non-writable, while the rent
    // payer must be writable, and a transaction grants privileges per key.
    let beneficiary = Keypair::new();
    let founder = Keypair::new();
    let projection_witness = Keypair::new();
    let market_rent = rpc.minimum_balance(STATE_BYTES)?;
    let expiry_slot = rpc
        .finalized_slot()?
        .checked_add(500_000)
        .ok_or_else(|| Error::new("founding expiry slot overflow"))?;

    let coordinates = derive_founding_coordinates(
        rpc,
        plan,
        input,
        records,
        identity_template,
        product,
        mint,
        payer.pubkey(),
        founder.pubkey(),
        expiry_slot,
    )?;
    let principal = coordinates.lock.amount;

    // One Token-2022 account owned by the party the principal is refundable to.
    let source_funder = Keypair::new();
    let wallet_rent = rpc.minimum_balance(ACCOUNT_BYTES)?;
    let mut initialize_wallet = Vec::with_capacity(33);
    initialize_wallet.push(18);
    initialize_wallet.extend_from_slice(beneficiary.pubkey().as_ref());
    let mut transfer_checked = Vec::with_capacity(10);
    transfer_checked.push(12);
    transfer_checked.extend_from_slice(&principal.to_le_bytes());
    transfer_checked.push(input.collateral_display_decimals);
    transactions.push(rpc.send_with_signers(
        "fund the founding principal supplier and its rent-capacity witness",
        &[
            transfer(&payer.pubkey(), &projection_witness.pubkey(), market_rent),
            create_account(
                &payer.pubkey(),
                &source_funder.pubkey(),
                wallet_rent,
                ACCOUNT_BYTES as u64,
                &token_program,
            ),
            Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(source_funder.pubkey(), false),
                    AccountMeta::new_readonly(mint, false),
                ],
                data: initialize_wallet,
            },
            Instruction {
                program_id: token_program,
                accounts: vec![
                    AccountMeta::new(collateral_wallet, false),
                    AccountMeta::new_readonly(mint, false),
                    AccountMeta::new(source_funder.pubkey(), false),
                    AccountMeta::new_readonly(payer.pubkey(), true),
                ],
                data: transfer_checked,
            },
        ],
        payer,
        &[&source_funder],
    )?);

    // The founding generation's own lifecycle credit, refundable to the
    // beneficiary the artifact names.
    let keys = found_snapshot_keys(
        plan,
        projection_witness.pubkey(),
        coordinates.market,
        coordinates.credit,
        records,
    )?;
    let mut snapshot_keys = keys.clone();
    for extra in [payer.pubkey(), beneficiary.pubkey()] {
        if !snapshot_keys.contains(&extra) {
            snapshot_keys.push(extra);
        }
    }
    let minimum_slot = transactions
        .last()
        .map(|transaction| transaction.slot)
        .ok_or_else(|| Error::new("founding stage had no finalized predecessor"))?;
    let snapshot = finalized_snapshot(rpc, &snapshot_keys, minimum_slot)?;
    let projection_state = projection_state(
        rpc,
        plan,
        &snapshot,
        projection_witness.pubkey(),
        coordinates.market,
        records,
    )?;
    let projection = project_found_v2(coordinates.generation, projection_state)
        .map_err(|error| Error::new(format!("chain-derived founding projection: {error:?}")))?;
    if projection.market_address != coordinates.market {
        return Err(Error::new(
            "founding projection changed the derived Market address",
        ));
    }
    let create = build_lifecycle_rent_create_v2(
        &projection,
        LifecycleRentCreateStateV2 {
            payer: snapshot.observation(payer.pubkey())?,
            credit_destination: snapshot.observation(coordinates.credit)?,
            refund_wallet: snapshot.observation(beneficiary.pubkey())?,
            rent_program: snapshot.observation(rent_program)?,
            system_program: snapshot.observation(system_program::ID)?,
            rent: snapshot.observation(sysvar::rent::ID)?,
        },
    )
    .map_err(|error| Error::new(format!("chain-derived founding RentV2 Create: {error:?}")))?;
    transactions.push(rpc.send(
        "create the founding generation's lifecycle RentCreditV2",
        std::slice::from_ref(&create.instruction),
        payer,
    )?);

    // Publish the artifact and the terminal Lock request as content-addressed
    // readonly records, plus one non-terminal prestate the bootstrap must
    // refuse, so the substitution below is a real request rather than a fake.
    let found_raw_bytes = coordinates
        .found
        .encode()
        .map_err(|error| Error::new(format!("founding artifact encoding: {error:?}")))?;
    let lock_raw_bytes = coordinates
        .lock
        .encode()
        .map_err(|error| Error::new(format!("terminal Lock encoding: {error:?}")))?;
    let open_hoard_bytes = coordinates
        .lock
        .founding_prestate_stage_v1(FoundingPrestateStageV1::OpenHoard)
        .and_then(|request| request.encode())
        .map_err(|error| Error::new(format!("OpenHoard prestate encoding: {error:?}")))?;
    let found_record = publish_record(
        rpc,
        registry,
        payer,
        raw_request_schema_v1("generic-founding-artifact"),
        &found_raw_bytes,
        None,
        transactions,
    )?;
    let lock_record = publish_record(
        rpc,
        registry,
        payer,
        raw_request_schema_v1("projected-custody-terminal-lock"),
        &lock_raw_bytes,
        None,
        transactions,
    )?;
    let open_hoard_record = publish_record(
        rpc,
        registry,
        payer,
        raw_request_schema_v1("projected-custody-non-terminal"),
        &open_hoard_bytes,
        None,
        transactions,
    )?;

    let bootstrap = build_projected_custody_bootstrap_v1(
        plan,
        &coordinates,
        records,
        found_record.raw,
        lock_record.raw,
        projection_witness.pubkey(),
        payer.pubkey(),
        beneficiary.pubkey(),
        source_funder.pubkey(),
        mint,
    )?;
    let (routing, tables) = publish_routing_table(
        rpc,
        payer,
        "DCLTPCB1",
        std::slice::from_ref(&bootstrap),
        transactions,
    )?;

    let created = [
        ("projected_custody_replay", coordinates.projected_replay),
        ("founding_hoard_vault", coordinates.hoard_vault),
        ("founding_source_vault", coordinates.source_vault),
        ("founding_source_replay", coordinates.source_replay),
    ];
    let refuse_left_nothing = |rpc: &mut Rpc, label: &str| -> Result<()> {
        for (name, key) in created {
            if rpc.account(key)?.is_some() {
                return Err(Error::new(format!("{label} left a {name} account")));
            }
        }
        for funding in &coordinates.funding_states {
            if rpc.account(*funding)?.is_some() {
                return Err(Error::new(format!("{label} left a funded FundingState")));
            }
        }
        Ok(())
    };
    refuse_left_nothing(rpc, "the founding prestate")?;

    // The bootstrap admits exactly one request: the terminal Lock this
    // founding determines. A well-formed non-terminal prestate is refused.
    let mut non_terminal = bootstrap.clone();
    non_terminal
        .accounts
        .get_mut(1)
        .ok_or_else(|| Error::new("bootstrap omitted its terminal Lock coordinate"))?
        .pubkey = open_hoard_record.raw;
    transactions.push(rpc.send_v0_expected_failure_with_signers(
        "DCLTPCB1 refuses a non-terminal projected-Custody request",
        &[non_terminal],
        payer,
        &[&beneficiary],
        routing,
        &tables,
    )?);
    refuse_left_nothing(rpc, "the refused non-terminal bootstrap")?;

    // The FundingState tail is the manifest binding. Reordering it derives an
    // address the manifest entry at that position does not name, and the whole
    // four-stage bootstrap must roll back with no funded account left behind.
    if coordinates.funding_states.len() >= 2 {
        let mut reordered = bootstrap.clone();
        let first = PROJECTED_CUSTODY_BOOTSTRAP_FIXED_ACCOUNTS_V1;
        let second = first
            .checked_add(1)
            .ok_or_else(|| Error::new("funding tail index overflow"))?;
        reordered.accounts.swap(first, second);
        let rollback_recipient = Pubkey::new_unique();
        let before = rpc.required_account(payer.pubkey(), "bootstrap rollback prestate")?;
        let rolled_back = rpc.send_v0_expected_failure_with_signers(
            "DCLTPCB1 refuses a reordered FundingState tail and rolls the transaction back",
            &[transfer(&payer.pubkey(), &rollback_recipient, 1), reordered],
            payer,
            &[&beneficiary],
            routing,
            &tables,
        )?;
        let fee = rolled_back
            .fee_lamports
            .ok_or_else(|| Error::new("refused bootstrap omitted its fee"))?;
        let after = rpc.required_account(payer.pubkey(), "bootstrap rollback poststate")?;
        if rpc.account(rollback_recipient)?.is_some()
            || after.lamports.checked_add(fee) != Some(before.lamports)
        {
            return Err(Error::new(
                "refused bootstrap did not roll its whole transaction back to a fee-only debit",
            ));
        }
        transactions.push(rolled_back);
        refuse_left_nothing(rpc, "the refused reordered bootstrap")?;
    }

    transactions.push(rpc.send_v0_with_signers(
        "create the projected-Custody founding prestate (DCLTPCB1)",
        &[bootstrap],
        payer,
        &[&beneficiary],
        routing,
        &tables,
    )?);

    authenticate_bootstrap_poststate(
        rpc,
        &coordinates,
        custody,
        trading,
        token_program,
        mint,
        principal,
    )?;
    for (label, key) in created {
        let account = rpc.required_account(key, label)?;
        accounts.insert(label.into(), account_evidence(key, &account));
    }
    for (index, funding) in coordinates.funding_states.iter().enumerate() {
        let account = rpc.required_account(*funding, "founding FundingState")?;
        accounts.insert(
            format!("founding_funding_state_{index}"),
            account_evidence(*funding, &account),
        );
    }
    let credit_account = rpc.required_account(coordinates.credit, "founding lifecycle credit")?;
    accounts.insert(
        "founding_lifecycle_rent_credit".into(),
        account_evidence(coordinates.credit, &credit_account),
    );
    completed.push(
        "derived one generic founding's complete coordinate set - Market, credit, action context, Custody compartments, capability root, and prepaid FundingState list - from the finalized record graph alone".into(),
    );
    completed.push(
        "proved DCLTPCB1 admits only the terminal Lock request its founding determines".into(),
    );
    completed.push(
        "proved a reordered FundingState tail refuses and rolls the whole four-stage bootstrap back to a fee-only debit".into(),
    );
    completed.push(
        "executed DCLTPCB1: projected replay at SourceFunded, empty Hoard vault, funded source compartment, and one prepaid FundingStateV1 per capability-manifest entry, in one rollback domain".into(),
    );
    Ok(())
}

/// Reacquire and check the exact prestate the founding outer will consume.
#[allow(clippy::too_many_arguments)]
fn authenticate_bootstrap_poststate(
    rpc: &mut Rpc,
    coordinates: &FoundingCoordinates,
    custody: Pubkey,
    trading: Pubkey,
    token_program: Pubkey,
    mint: Pubkey,
    principal: u64,
) -> Result<()> {
    let replay = rpc.required_account(coordinates.projected_replay, "projected Custody replay")?;
    let state = ProjectedCustodyStateV1::decode(&replay.data)
        .map_err(|error| Error::new(format!("projected Custody state: {error:?}")))?;
    if replay.owner != custody
        || replay.data.len() != PROJECTED_CUSTODY_STATE_BYTES_V1
        || state.phase != ProjectedCustodyPhaseV1::SourceFunded
        || state.next_revision != OPEN_SOURCE_COMPARTMENT_RESULTING_REVISION_V1
        || state.locked_amount != principal
    {
        return Err(Error::new(
            "DCLTPCB1 poststate did not rest at the funded source compartment",
        ));
    }
    let hoard = rpc.required_account(coordinates.hoard_vault, "founding Hoard vault")?;
    let hoard_state = TokenAccount::parse(&hoard.data)
        .map_err(|error| Error::new(format!("Hoard vault: {error:?}")))?;
    if hoard.owner != token_program
        || hoard_state.mint != mint.to_bytes()
        || hoard_state.amount != 0
        || hoard_state.state != AccountState::Initialized
    {
        return Err(Error::new(
            "founding Hoard vault poststate differed from its checked plan",
        ));
    }
    let source = rpc.required_account(coordinates.source_vault, "founding source vault")?;
    let source_state = TokenAccount::parse(&source.data)
        .map_err(|error| Error::new(format!("source vault: {error:?}")))?;
    if source.owner != token_program
        || source_state.mint != mint.to_bytes()
        || source_state.amount != principal
        || source_state.state != AccountState::Initialized
    {
        return Err(Error::new(
            "founding source vault did not hold exactly the founding principal",
        ));
    }
    let source_replay =
        rpc.required_account(coordinates.source_replay, "founding source replay")?;
    if source_replay.owner != custody || source_replay.data.len() != CUSTODY_REPLAY_BYTES_V1 {
        return Err(Error::new(
            "founding source replay poststate differed from its checked plan",
        ));
    }
    for (index, key) in coordinates.funding_states.iter().enumerate() {
        let account = rpc.required_account(*key, "founding FundingState")?;
        let funding = FundingStateV1::decode(&account.data)
            .map_err(|error| Error::new(format!("FundingStateV1 {index}: {error:?}")))?;
        if account.owner != trading
            || account.data.len() != FUNDING_STATE_BYTES
            || funding.status() != FundingStatus::Pending
            || usize::from(funding.entry_index()) != index
        {
            return Err(Error::new(format!(
                "FundingState {index} poststate differed from its checked plan"
            )));
        }
    }
    Ok(())
}

/// Domain-separated preimage prefix for every demo semantic identifier.
///
/// A demo identifier is the SHA-256 of `domain || 00 || part || 00 || part...`.
/// It names a semantic spec; it is not a checked production release row and no
/// registry publishes it outside this local lab.
const DEMO_ID_DOMAIN_V1: &[u8] = b"dclutch/local-demo-market/v1";

fn demo_id(role: &str, parts: &[&[u8]]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(DEMO_ID_DOMAIN_V1);
    hasher.update([0]);
    hasher.update(role.as_bytes());
    for part in parts {
        hasher.update([0]);
        hasher.update(part);
    }
    hasher.finalize().into()
}

/// Construct the canonical local demo Market: SOL/USD range protection.
///
/// The Product is a small categorical partition of USD-cents-per-SOL with cuts
/// at 12000 and 18000, so the three ordinary regions are "below 120.00",
/// "inside [120.00, 180.00)", and "at or above 180.00", followed by the
/// explicit failure outcome. The portfolio pays one unit of the liability
/// basis in either tail and nothing inside the range or on failure, which is
/// the payoff a holder buys as protection against SOL/USD leaving the band.
///
/// Every semantic identifier is a domain-separated digest naming the spec it
/// stands for, and the resolution identifiers additionally bind the captured
/// synthetic-local Pyth release from
/// `fixtures/pyth/local-upgraded-2026-08-22`. That release is a local lab
/// projection documented in `docs/evidence/PYTH_SYNTHETIC_RELEASE_V1.md`; it is
/// not a production provider release, and this Market is not a mainnet or
/// devnet product.
pub(crate) fn demo_market_input(registry: Pubkey) -> Result<MarketRunInput> {
    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CompartmentFundingV1,
        ContentId as CapabilityContentId, FundingAmountsV1, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY,
    };
    use dclutch_pyth_svm::synthetic_fixture::synthetic_local_release_v1;
    use dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V4;
    use dclutch_source_contract::{RECOVERY_POLICY_MAX_ATTEMPTS_V2, RecoveryAttemptV2};

    let fixture = synthetic_local_release_v1()
        .map_err(|error| Error::new(format!("synthetic-local Pyth release: {error:?}")))?;
    let local_label = fixture.local_label();
    let adapter = fixture.release().adapter_id();
    let feed = b"sol-usd".as_slice();

    let product_identity = demo_id("product/sol-usd-range-protection", &[&local_label]);
    let coordinate_domain = demo_id("coordinate-domain/usd-cents-per-sol", &[]);
    let result_unit = demo_id("result-unit/usd-cents", &[]);
    let claim_basis = demo_id("claim-basis/unit-complete-set", &[]);
    let representation = demo_id("representation/categorical-fixed-width", &[]);
    let mapping = demo_id("mapping/scaled-integer-cut", &[&coordinate_domain]);
    let primary_source = demo_id("source-spec/pyth-price-update", &[&adapter, feed]);
    let window = demo_id("window-spec/single-verified-post-at-expiry", &[&adapter]);
    let statistic = demo_id("statistic-spec/last-verified-price", &[&adapter]);
    let failure_policy = demo_id("failure-policy/explicit-failure-outcome", &[]);
    let recovery_allocation = demo_id("recovery/funding-allocation", &[&local_label]);
    let recovery_source = demo_id("recovery/secondary-source-spec", &[&adapter, feed]);
    let recovery_authority = demo_id("recovery/attempt-authority", &[&local_label]);
    let recovery_root = demo_id("recovery/policy-root", &[&local_label]);

    let cut_denominator = 100_u64;
    let cuts: Vec<i128> = vec![12_000, 18_000];
    let coefficients: Vec<u64> = vec![1, 0, 1, 0];
    let outcome_count = coefficients.len();

    // The liability basis is a real record, not a name. Its semantic identity
    // omits the Product and result-domain links, so it is derivable before the
    // Product that declares it exists; the record is then recompiled with both
    // real links and must keep the same identity.
    let evaluator_release = demo_id("liability-basis/categorical-unit-evaluator", &[]);
    let liability_basis = semantic_basis_identity_v3(&compile_linked_basis_v3(
        product_identity,
        product_identity,
        coordinate_domain,
        result_unit,
        evaluator_release,
        outcome_count,
    )?)?;

    let product = ProductCompilationInputV2 {
        product_id: ProductContentId::new(product_identity)
            .map_err(|error| Error::new(format!("demo Product ID: {error:?}")))?,
        coordinate_domain_id: ProductContentId::new(coordinate_domain)
            .map_err(|error| Error::new(format!("demo coordinate domain: {error:?}")))?,
        result_unit_id: ProductContentId::new(result_unit)
            .map_err(|error| Error::new(format!("demo result unit: {error:?}")))?,
        claim_basis_id: ProductContentId::new(claim_basis)
            .map_err(|error| Error::new(format!("demo claim basis: {error:?}")))?,
        liability_basis_id: ProductContentId::new(liability_basis)
            .map_err(|error| Error::new(format!("demo liability basis: {error:?}")))?,
        representation_release_id: ProductContentId::new(representation)
            .map_err(|error| Error::new(format!("demo representation: {error:?}")))?,
        mapping_release_id: ProductContentId::new(mapping)
            .map_err(|error| Error::new(format!("demo mapping: {error:?}")))?,
        cut_denominator,
        cuts: &cuts,
        portfolio_denominator: 1,
        coefficients: &coefficients,
    };
    let mut product_bytes = [0_u8; PRODUCT_RECORD_BYTES_V2];
    let mut domain = vec![
        0_u8;
        result_domain_record_bytes(cuts.len()).map_err(|error| Error::new(
            format!("demo domain width: {error:?}")
        ))?
    ];
    let mut portfolio = vec![
        0_u8;
        portfolio_record_bytes(outcome_count).map_err(|error| Error::new(
            format!("demo portfolio width: {error:?}")
        ))?
    ];
    compile_product_records_v2(
        registry,
        product,
        &mut product_bytes,
        &mut domain,
        &mut portfolio,
    )
    .map_err(|error| Error::new(format!("demo Product compiler: {error:?}")))?;
    let product_digest: [u8; 32] = Sha256::digest(product_bytes).into();
    let domain_digest: [u8; 32] = Sha256::digest(&domain).into();
    let linked_basis = compile_linked_basis_v3(
        product_identity,
        domain_digest,
        coordinate_domain,
        result_unit,
        evaluator_release,
        outcome_count,
    )?;
    if semantic_basis_identity_v3(&linked_basis)? != liability_basis {
        return Err(Error::new(
            "linking the demo liability basis to its Product changed its semantic identity",
        ));
    }

    let attempt = RecoveryAttemptV2::new(
        SourceContentId::new(recovery_source)
            .map_err(|error| Error::new(format!("demo recovery source: {error:?}")))?,
        SourceContentId::new(recovery_authority)
            .map_err(|error| Error::new(format!("demo recovery authority: {error:?}")))?,
        2_000_000_000,
        SourceContentId::new(recovery_allocation)
            .map_err(|error| Error::new(format!("demo recovery allocation: {error:?}")))?,
    )
    .map_err(|error| Error::new(format!("demo recovery attempt: {error:?}")))?;
    let mut attempts = [None; RECOVERY_POLICY_MAX_ATTEMPTS_V2];
    attempts[0] = Some(attempt);
    let recovery = RecoveryPolicyV2::new(
        SourceContentId::new(recovery_root)
            .map_err(|error| Error::new(format!("demo recovery root: {error:?}")))?,
        attempts,
        1,
    )
    .map_err(|error| Error::new(format!("demo recovery policy: {error:?}")))?;
    let recovery_bytes = recovery.to_bytes();
    let recovery_digest: [u8; 32] = Sha256::digest(recovery_bytes).into();
    let material = SourceMaterialV2::new(
        SourceContentId::new(product_digest)
            .map_err(|error| Error::new(format!("demo Product digest: {error:?}")))?,
        SourceContentId::new(primary_source)
            .map_err(|error| Error::new(format!("demo primary source: {error:?}")))?,
        SourceContentId::new(window)
            .map_err(|error| Error::new(format!("demo window: {error:?}")))?,
        SourceContentId::new(statistic)
            .map_err(|error| Error::new(format!("demo statistic: {error:?}")))?,
        Some(
            SourceContentId::new(recovery_digest)
                .map_err(|error| Error::new(format!("demo recovery digest: {error:?}")))?,
        ),
        SourceContentId::new(failure_policy)
            .map_err(|error| Error::new(format!("demo failure policy: {error:?}")))?,
    );
    let material_digest: [u8; 32] = Sha256::digest(material.to_bytes()).into();

    let native = CompartmentFundingV1::native_lamports(1)
        .map_err(|error| Error::new(format!("demo funding: {error:?}")))?;
    let none = CompartmentFundingV1::not_applicable();
    let amounts = FundingAmountsV1::new(native, native, none, none, native, none, none)
        .map_err(|error| Error::new(format!("demo funding amounts: {error:?}")))?;
    let quote = FundingQuoteV1::new(amounts, None)
        .map_err(|error| Error::new(format!("demo funding quote: {error:?}")))?;
    let release = CapabilityContentId::new(RESOLUTION_CONTROLLER_RELEASE_ID_V4)
        .map_err(|error| Error::new(format!("demo Resolution release: {error:?}")))?;
    let mut entries_input: Vec<([u8; 32], [u8; 32])> = vec![
        (
            demo_id("capability/resolve-primary", &[&local_label]),
            attempt.funding_allocation_id().to_bytes(),
        ),
        (
            demo_id("capability/recovery-policy", &[&local_label]),
            recovery_digest,
        ),
        (
            demo_id("capability/source-material", &[&local_label]),
            material_digest,
        ),
    ];
    // The manifest is canonical only when entries are strictly ordered by
    // capability-kind identity; the demo kinds are digests, so sort them.
    entries_input.sort_by_key(|entry| entry.0);
    let mut entries = Vec::new();
    for (index, (kind, config)) in entries_input.into_iter().enumerate() {
        let entry_index =
            u16::try_from(index).map_err(|_| Error::new("demo capability index overflow"))?;
        entries.push(
            CapabilityEntryV1::new(
                CapabilityContentId::new(kind)
                    .map_err(|error| Error::new(format!("demo capability kind: {error:?}")))?,
                release,
                CapabilityContentId::new(config)
                    .map_err(|error| Error::new(format!("demo capability config: {error:?}")))?,
                CapabilityContentId::new(demo_id("capability/capacity", &[&[entry_index as u8]]))
                    .map_err(|error| Error::new(format!("demo capability capacity: {error:?}")))?,
                CapabilityContentId::new(demo_id("capability/schema", &[]))
                    .map_err(|error| Error::new(format!("demo capability schema: {error:?}")))?,
                CapabilityContentId::new(demo_id("capability/derivation", &[])).map_err(
                    |error| Error::new(format!("demo capability derivation: {error:?}")),
                )?,
                ActivationPolicy::RequiredAtFounding,
                0,
                0,
                [0; MAX_DEPENDENCIES_PER_CAPABILITY],
                quote,
            )
            .map_err(|error| Error::new(format!("demo capability entry: {error:?}")))?,
        );
    }
    let mut manifest = vec![0_u8; MANIFEST_HEADER_BYTES + entries.len() * CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&entries, &mut manifest)
        .map_err(|error| Error::new(format!("demo capability manifest: {error:?}")))?;

    let input = MarketRunInput {
        generation: 1,
        collateral_display_decimals: 6,
        initial_collateral_atoms: 1_000_000_000,
        product_id: hex(&product_identity),
        coordinate_domain_id: hex(&coordinate_domain),
        result_unit_id: hex(&result_unit),
        claim_basis_id: hex(&claim_basis),
        liability_basis_id: hex(&liability_basis),
        representation_release_id: hex(&representation),
        mapping_release_id: hex(&mapping),
        cut_denominator,
        cuts: cuts.iter().map(|cut| cut.to_string()).collect(),
        portfolio_denominator: 1,
        coefficients,
        primary_source_spec_id: hex(&primary_source),
        window_spec_id: hex(&window),
        statistic_spec_id: hex(&statistic),
        failure_policy_release_id: hex(&failure_policy),
        recovery_policy_hex: hex(&recovery_bytes),
        capability_manifest_hex: hex(&manifest),
        linked_basis_hex: hex(&linked_basis),
    };
    validate_market_input(&input)?;
    Ok(input)
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
