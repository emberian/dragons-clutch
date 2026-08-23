//! Real-SBF bank evidence for resumable typed artifact upload and sealing.
//!
//! The main case begins a Terms upload from an absent stage PDA, commits only
//! three chunks, reloads the uploader and stage account images into a fresh
//! ProgramTest bank, commits the remaining chunks, and seals the exact raw
//! Terms bytes at the canonical content-derived PDA.  This is a bank-state
//! rehydration restart, not a validator-ledger replay claim.

use {
    clutch_sbf::{error::ClutchError, seeds},
    clutch_solana_layout::{
        account_len,
        artifact::{decode_stage, ArtifactKind, ARTIFACT_CHUNK_BYTES, ARTIFACT_STAGE_HEADER_BYTES},
        collateral::{self, ParentProfile},
        Hash32, Intent, PriceGridAccount, TermsAccount, MAX_GRID_TICKS,
    },
    clutch_svm_fixture::{
        fixture_policy, fixture_terms, layout_request, PROGRAM_ID, RENT_SYSVAR, SYSTEM_PROGRAM,
    },
    solana_account::Account,
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_keypair::Keypair,
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_rent::Rent,
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

#[cfg(feature = "non-production-product-series-lab")]
use clutch_product_series::{
    CompiledProductSeriesBundleV1, ComponentDebitV1, ContentId, EvidenceOnlyRecoveryPolicyV1,
    FixedCodec, MarketGenesisProfileV2, NativeClaimBasisV1, PriceMeasurePolicyV1,
    ProductTemplateV4, RecoveryAttemptFundingV1, RecoveryAttemptV1, SeriesAttachmentPlanV1,
    SeriesFundingQuoteV1, SeriesFundingTermsV2, SeriesPlanV5, SeriesPlanV5Id, BASIS_BYTES,
    MAX_OUTCOMES as PRODUCT_MAX_OUTCOMES, MAX_PAYOUTS as PRODUCT_MAX_PAYOUTS,
    MAX_RECOVERY_ATTEMPTS, PAYOUT_MAP_UNUSED, RECOVERY_POLICY_DOMAIN, UNIFORM_SPACING_NONE,
};
#[cfg(feature = "non-production-product-series-lab")]
use sha2::{Digest, Sha256};

const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
const UPLOADER_LAMPORTS: u64 = 2_000_000_000;

fn empty_system_account(lamports: u64) -> Account {
    Account {
        lamports,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    }
}

fn uploader() -> Keypair {
    Keypair::new_from_array([
        0x21, 0x7a, 0x49, 0x03, 0x99, 0x51, 0xd2, 0x8b, 0xe0, 0x0d, 0x73, 0x42, 0xaf, 0x14, 0x57,
        0x6c, 0x92, 0x05, 0xbb, 0x18, 0x81, 0x6e, 0x3a, 0x44, 0x09, 0xcf, 0x62, 0xf1, 0x35, 0x77,
        0xa0, 0x5d,
    ])
}

fn reaper() -> Keypair {
    Keypair::new_from_array([
        0x47, 0x11, 0xb0, 0x92, 0x63, 0xad, 0x05, 0x5e, 0x2c, 0x74, 0x8d, 0x3a, 0xf1, 0x09, 0xc0,
        0x36, 0x6a, 0x99, 0x20, 0xe2, 0x15, 0x7b, 0x5c, 0x88, 0x41, 0xde, 0x03, 0x67, 0xfa, 0x24,
        0x59, 0x9c,
    ])
}

fn derive_stage(
    funder: Address,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
) -> (Address, u8) {
    Address::find_program_address(
        &[
            seeds::SEED_ARTIFACT_STAGE,
            funder.as_ref(),
            &[kind.byte()],
            &context.bytes(),
            &digest.bytes(),
        ],
        &PROGRAM_ID,
    )
}

fn derive_final(kind: ArtifactKind, context: Hash32, digest: Hash32) -> (Address, u8) {
    let prefix = match kind {
        ArtifactKind::CollateralPolicy => seeds::SEED_POLICY,
        ArtifactKind::PriceGrid => seeds::SEED_GRID,
        ArtifactKind::Terms => seeds::SEED_TERMS,
        ArtifactKind::BatchPolicy => seeds::SEED_BATCH_POLICY,
        ArtifactKind::DirectBatchPolicyV3 => seeds::SEED_DIRECT_BATCH_POLICY_V3,
        kind @ (ArtifactKind::NativeClaimBasisV1
        | ArtifactKind::EvidenceOnlyRecoveryPolicyV1
        | ArtifactKind::ProductTemplateV4
        | ArtifactKind::PriceMeasurePolicyV1
        | ArtifactKind::MarketGenesisProfileV2
        | ArtifactKind::SeriesFundingQuoteV1
        | ArtifactKind::SeriesAttachmentPlanV1
        | ArtifactKind::SeriesPlanV5
        | ArtifactKind::SeriesFundingTermsV2
        | ArtifactKind::ProductCapabilityRegistryV2
        | ArtifactKind::CompiledProductSeriesBundleV1) => {
            return Address::find_program_address(
                &[
                    seeds::SEED_PRODUCT_ARTIFACT_V1,
                    &[kind.byte()],
                    &digest.bytes(),
                ],
                &PROGRAM_ID,
            );
        }
    };
    Address::find_program_address(&[prefix, &context.bytes(), &digest.bytes()], &PROGRAM_ID)
}

fn new_bank(extra: &[(Address, Account)]) -> ProgramTest {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    for (address, account) in extra {
        test.add_account(*address, account.clone());
    }
    test
}

fn begin_ix(
    funder: Address,
    stage: Address,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
    expires_slot: u64,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::BeginArtifact {
                kind,
                context,
                digest,
                exact_len: kind.exact_len() as u16,
                expires_slot,
            },
        ),
        vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn write_ix(
    funder: Address,
    stage: Address,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
    cursor: usize,
    body: &[u8],
) -> Instruction {
    let remaining = body.len() - cursor;
    let chunk_len = remaining.min(ARTIFACT_CHUNK_BYTES);
    let mut chunk = [0; ARTIFACT_CHUNK_BYTES];
    chunk[..chunk_len].copy_from_slice(&body[cursor..cursor + chunk_len]);
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::WriteArtifact {
                kind,
                context,
                digest,
                cursor: cursor as u16,
                chunk_len: chunk_len as u16,
                chunk,
            },
        ),
        vec![
            AccountMeta::new_readonly(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn seal_ix(
    funder: Address,
    stage: Address,
    final_account: Address,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::SealArtifact {
                kind,
                context,
                digest,
                exact_len: kind.exact_len() as u16,
            },
        ),
        vec![
            AccountMeta::new(funder, true),
            AccountMeta::new(stage, false),
            AccountMeta::new(final_account, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

fn abort_ix(
    caller: Address,
    stage: Address,
    funder: Address,
    kind: ArtifactKind,
    context: Hash32,
    digest: Hash32,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::AbortArtifact {
                kind,
                context,
                digest,
            },
        ),
        vec![
            AccountMeta::new_readonly(caller, true),
            AccountMeta::new(stage, false),
            AccountMeta::new(funder, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

async fn send(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signer: &Keypair,
) -> Result<(), TransactionError> {
    send_measured(context, instruction, signer).await.0
}

async fn send_measured(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    signer: &Keypair,
) -> (Result<(), TransactionError>, u64) {
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, signer],
        blockhash,
    );
    let outcome = context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap();
    let units = outcome
        .metadata
        .map(|metadata| metadata.compute_units_consumed)
        .unwrap_or_default();
    (outcome.result, units)
}

fn custom(result: Result<(), TransactionError>) -> u32 {
    match result {
        Err(TransactionError::InstructionError(_, InstructionError::Custom(code))) => code,
        other => panic!("expected custom refusal, got {other:?}"),
    }
}

async fn account(context: &mut ProgramTestContext, address: Address) -> Option<Account> {
    context.banks_client.get_account(address).await.unwrap()
}

async fn upload_all(
    context: &mut ProgramTestContext,
    author: &Keypair,
    kind: ArtifactKind,
    binding_context: Hash32,
    digest: Hash32,
    body: &[u8],
) -> (Address, Address) {
    let (stage, _) = derive_stage(author.pubkey(), kind, binding_context, digest);
    let (final_account, _) = derive_final(kind, binding_context, digest);
    send(
        context,
        begin_ix(author.pubkey(), stage, kind, binding_context, digest, 1_000),
        author,
    )
    .await
    .expect("typed artifact begin");
    let mut cursor = 0;
    while cursor < body.len() {
        send(
            context,
            write_ix(
                author.pubkey(),
                stage,
                kind,
                binding_context,
                digest,
                cursor,
                body,
            ),
            author,
        )
        .await
        .expect("typed artifact write");
        cursor += ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
    }
    send(
        context,
        seal_ix(
            author.pubkey(),
            stage,
            final_account,
            kind,
            binding_context,
            digest,
        ),
        author,
    )
    .await
    .expect("typed artifact seal");
    (stage, final_account)
}

async fn write_all_chunks(
    context: &mut ProgramTestContext,
    author: &Keypair,
    stage: Address,
    kind: ArtifactKind,
    binding_context: Hash32,
    digest: Hash32,
    body: &[u8],
) {
    let mut cursor = 0;
    while cursor < body.len() {
        send(
            context,
            write_ix(
                author.pubkey(),
                stage,
                kind,
                binding_context,
                digest,
                cursor,
                body,
            ),
            author,
        )
        .await
        .expect("typed artifact write");
        cursor += ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
    }
}

fn encode_terms(mut terms: TermsAccount) -> (Vec<u8>, Hash32, u8) {
    let (address, bump) = derive_final(ArtifactKind::Terms, terms.realm, terms.terms);
    let _ = address;
    terms.stored_bump = bump;
    let mut body = vec![0; account_len::TERMS];
    assert_eq!(terms.encode(&mut body), Ok(account_len::TERMS));
    (body, terms.terms, bump)
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_basis() -> NativeClaimBasisV1 {
    let mut payout_weights = [[0; PRODUCT_MAX_OUTCOMES]; PRODUCT_MAX_PAYOUTS];
    let mut index = 0_usize;
    while index < 4 {
        payout_weights[index][index] = 1_000;
        index += 1;
    }
    let mut payout_map = [PAYOUT_MAP_UNUSED; PRODUCT_MAX_OUTCOMES];
    payout_map[..4].copy_from_slice(&[0, 1, 2, 3]);
    let mut knots = [0; PRODUCT_MAX_OUTCOMES];
    knots[..3].copy_from_slice(&[100, 200, 300]);
    NativeClaimBasisV1 {
        basis_degree: 0,
        outcome_count: 4,
        payout_count: 4,
        knot_count: 3,
        uniform_log2_spacing: UNIFORM_SPACING_NONE,
        ambiguity_policy_registry_value: 1,
        edge_policy_registry_value: 1,
        denominator: 1_000,
        payout_weights,
        payout_map,
        knots,
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_basis_body() -> (Vec<u8>, Hash32) {
    let basis = product_basis();
    let mut body = vec![0_u8; BASIS_BYTES];
    basis.encode_into(&mut body).expect("canonical basis");
    (body, Hash32::from_bytes(basis.id().unwrap().bytes()))
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_recovery() -> EvidenceOnlyRecoveryPolicyV1 {
    let mut attempts = [RecoveryAttemptV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    attempts[0] = RecoveryAttemptV1 {
        repair_generation_delta: 0,
        opens_after_primary_maturity_buckets: 0,
        closes_after_primary_maturity_buckets: 2,
    };
    attempts[1] = RecoveryAttemptV1 {
        repair_generation_delta: 1,
        opens_after_primary_maturity_buckets: 2,
        closes_after_primary_maturity_buckets: 5,
    };
    EvidenceOnlyRecoveryPolicyV1 {
        attempt_count: 2,
        attempts,
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_template() -> ProductTemplateV4 {
    ProductTemplateV4 {
        source_plane_contract_id: product_id(1),
        source_spec_id: product_id(2),
        summary_program_id: product_id(3),
        native_claim_basis_id: product_basis().id().unwrap(),
        evidence_only_recovery_policy_id: product_recovery().id().unwrap(),
        compiler_release_id: product_id(4),
        statistic_registry_value: 11,
        coverage_policy_registry_value: 12,
        window_span_buckets: 4,
        primary_maturity_grace_buckets: 2,
        base_repair_generation: 10,
        coverage_policy_parameter: 0,
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_price_policy() -> PriceMeasurePolicyV1 {
    PriceMeasurePolicyV1 {
        checker_release_id: product_id(30),
        checker_version: 3,
        quantized_semantics_version: 1,
        minimum_basis_degree: 0,
        maximum_basis_degree: 3,
        maximum_outcome_count: 16,
        maximum_atom_count: 16,
        maximum_payout_denominator: u64::MAX,
        maximum_witness_denominator: u64::MAX,
        maximum_price_scale: u64::MAX / 16,
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_genesis() -> MarketGenesisProfileV2 {
    MarketGenesisProfileV2 {
        realm_id: product_id(20),
        profile_id: product_id(21),
        price_grid_id: product_id(22),
        price_measure_policy_id: product_price_policy().id().unwrap(),
        fee_policy_id: product_id(23),
        relation_policy_id: product_id(24),
        score_policy_id: product_id(25),
        candidate_lifecycle_policy_id: product_id(26),
        candidate_liveness_policy_id: product_id(27),
        retirement_policy_id: product_id(28),
        capability_profile_id: product_id(29),
        terminal_disposition_registry_value: 7,
        native_bearer_lot: 1_000,
        coordinate_domain_min: 0,
        coordinate_domain_max: 400,
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_quote() -> SeriesFundingQuoteV1 {
    let mut attempts = [RecoveryAttemptFundingV1::ZERO; MAX_RECOVERY_ATTEMPTS];
    attempts[0] = RecoveryAttemptFundingV1 {
        max_progress_units: 3,
        lamports_per_progress_unit: 5,
    };
    attempts[1] = RecoveryAttemptFundingV1 {
        max_progress_units: 2,
        lamports_per_progress_unit: 7,
    };
    SeriesFundingQuoteV1 {
        evidence_only_recovery_policy_id: product_recovery().id().unwrap(),
        market_core: ComponentDebitV1 {
            lamports: 10,
            collateral_atoms: 0,
        },
        failure_root_rent_principal_lamports: 3,
        failure_replay_tombstone_rent_principal_lamports: 2,
        recovery_reserve: ComponentDebitV1 {
            lamports: 40,
            collateral_atoms: 0,
        },
        source_work: ComponentDebitV1 {
            lamports: 30,
            collateral_atoms: 0,
        },
        liquidity_facility: ComponentDebitV1 {
            lamports: 40,
            collateral_atoms: 100,
        },
        wrapper_set: ComponentDebitV1 {
            lamports: 50,
            collateral_atoms: 10,
        },
        recovery_attempt_count: 2,
        recovery_attempt_funding: attempts,
        recovery_rent_principal_lamports: 11,
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_attachment() -> SeriesAttachmentPlanV1 {
    SeriesAttachmentPlanV1 {
        funding_quote_id: product_quote().id().unwrap(),
        liquidity_facility_plan_id: product_id(41),
        wrapper_recipe_set_id: product_id(42),
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_series() -> SeriesPlanV5 {
    SeriesPlanV5 {
        product_template_id: product_template().id().unwrap(),
        market_genesis_profile_id: product_genesis().id().unwrap(),
        attachment_plan_id: product_attachment().id().unwrap(),
        first_start_bucket: 100,
        stride_buckets: 10,
        instance_count: 3,
        creation_lead_buckets: 5,
        market_collateral_cap: 1_000,
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn product_funding_terms() -> SeriesFundingTermsV2 {
    SeriesFundingTermsV2 {
        series_plan_id: SeriesPlanV5Id::from_bytes(product_series().id().unwrap().bytes()),
        lamport_principal_refund: product_id(50),
        collateral_principal_refund_token_account: product_id(51),
        neutral_collateral_disposition_token_account: product_id(52),
        neutral_lamport_sink: product_id(55),
        collateral_mint: product_id(53),
        token_program: product_id(54),
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn compiled_product_series_bundle() -> CompiledProductSeriesBundleV1 {
    let template = product_template();
    CompiledProductSeriesBundleV1 {
        registry_release_id: product_id(60),
        capability_profile_id: product_genesis().capability_profile_id,
        source_release_manifest_id: product_id(61),
        source_plane_contract_id: template.source_plane_contract_id,
        source_spec_id: template.source_spec_id,
        summary_program_id: template.summary_program_id,
        product_compiler_release_id: template.compiler_release_id,
        native_claim_basis_id: product_basis().id().unwrap(),
        evidence_only_recovery_policy_id: product_recovery().id().unwrap(),
        product_template_id: template.id().unwrap(),
        price_measure_policy_id: product_price_policy().id().unwrap(),
        market_genesis_profile_id: product_genesis().id().unwrap(),
        funding_quote_id: product_quote().id().unwrap(),
        attachment_plan_id: product_attachment().id().unwrap(),
        series_plan_id: product_series().id().unwrap(),
        funding_terms_id: product_funding_terms().id().unwrap(),
    }
}

#[cfg(feature = "non-production-product-series-lab")]
fn other_product_artifact_bodies() -> Vec<(ArtifactKind, Vec<u8>, Hash32)> {
    let mut cases = Vec::new();
    macro_rules! push {
        ($kind:expr, $value:expr) => {{
            let value = $value;
            let mut body = vec![0_u8; $kind.exact_len()];
            value.encode_into(&mut body).unwrap();
            cases.push(($kind, body, Hash32::from_bytes(value.id().unwrap().bytes())));
        }};
    }
    push!(
        ArtifactKind::EvidenceOnlyRecoveryPolicyV1,
        product_recovery()
    );
    push!(ArtifactKind::ProductTemplateV4, product_template());
    push!(ArtifactKind::PriceMeasurePolicyV1, product_price_policy());
    push!(ArtifactKind::MarketGenesisProfileV2, product_genesis());
    push!(ArtifactKind::SeriesFundingQuoteV1, product_quote());
    push!(ArtifactKind::SeriesAttachmentPlanV1, product_attachment());
    push!(ArtifactKind::SeriesPlanV5, product_series());
    push!(ArtifactKind::SeriesFundingTermsV2, product_funding_terms());
    push!(
        ArtifactKind::CompiledProductSeriesBundleV1,
        compiled_product_series_bundle()
    );
    cases
}

#[cfg(feature = "non-production-product-series-lab")]
fn typed_digest(domain: &[u8], body: &[u8]) -> Hash32 {
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(body);
    Hash32::from_bytes(hasher.finalize().into())
}

#[cfg(feature = "non-production-product-series-lab")]
#[tokio::test]
async fn product_series_artifact_catalog_is_real_resumable_and_fail_closed() {
    let author = uploader();
    let kind = ArtifactKind::NativeClaimBasisV1;
    let binding_context = Hash32::ZERO;
    let (body, digest) = product_basis_body();
    assert_eq!(body.len(), 12 * ARTIFACT_CHUNK_BYTES + 48);
    let (stage, _) = derive_stage(author.pubkey(), kind, binding_context, digest);
    let (final_account, _) = derive_final(kind, binding_context, digest);
    let mut context = new_bank(&[(author.pubkey(), empty_system_account(UPLOADER_LAMPORTS))])
        .start_with_context()
        .await;

    // Product artifacts are global typed content. A Realm-like transport
    // context is not harmless metadata and refuses before a stage exists.
    // Start from a canonically encoded zero-context request and mutate only
    // the hostile wire field because the trusted encoder correctly refuses to
    // construct this request for us.
    let hostile_context = Hash32::from_bytes([0x91; 32]);
    let (hostile_stage, _) = derive_stage(author.pubkey(), kind, hostile_context, digest);
    let mut hostile_begin = begin_ix(
        author.pubkey(),
        hostile_stage,
        kind,
        binding_context,
        digest,
        1_000,
    );
    const REQUEST_ENVELOPE_BYTES: usize = 13;
    const ARTIFACT_CONTEXT_OFFSET: usize = REQUEST_ENVELOPE_BYTES + 2 + 1;
    hostile_begin.data[ARTIFACT_CONTEXT_OFFSET..ARTIFACT_CONTEXT_OFFSET + 32]
        .copy_from_slice(&hostile_context.bytes());
    assert!(send(&mut context, hostile_begin, &author).await.is_err());
    assert!(account(&mut context, hostile_stage).await.is_none());

    let mut replay_begin = begin_ix(author.pubkey(), stage, kind, binding_context, digest, 1_000);
    replay_begin.data[2..10].copy_from_slice(&1_u64.to_le_bytes());
    assert_eq!(
        custom(send(&mut context, replay_begin, &author).await),
        ClutchError::Replay as u32
    );
    assert!(account(&mut context, stage).await.is_none());

    // Keep the body fully canonical while changing its economics: denominator
    // and each live diagonal payout weight move together from 1,000 to 999.
    // The owning decoder accepts this second basis, but sealing it under the
    // first basis's typed digest must fail at the native SHA boundary. The
    // complete stage and absent final prove the late refusal is atomic.
    let mut stale_digest_body = body.clone();
    stale_digest_body[24..32].copy_from_slice(&999_u64.to_le_bytes());
    for index in 0..4 {
        let weight_offset = 32 + (index * PRODUCT_MAX_OUTCOMES + index) * 8;
        stale_digest_body[weight_offset..weight_offset + 8].copy_from_slice(&999_u64.to_le_bytes());
    }
    let stale_basis = NativeClaimBasisV1::decode(&stale_digest_body).expect("second valid basis");
    assert_ne!(stale_basis.id().unwrap().bytes(), digest.bytes());
    send(
        &mut context,
        begin_ix(author.pubkey(), stage, kind, binding_context, digest, 1_000),
        &author,
    )
    .await
    .expect("begin stale-digest basis stage");
    write_all_chunks(
        &mut context,
        &author,
        stage,
        kind,
        binding_context,
        digest,
        &stale_digest_body,
    )
    .await;
    let complete_stale_stage = account(&mut context, stage).await.unwrap();
    assert_eq!(
        custom(
            send(
                &mut context,
                seal_ix(
                    author.pubkey(),
                    stage,
                    final_account,
                    kind,
                    binding_context,
                    digest,
                ),
                &author,
            )
            .await
        ),
        ClutchError::EvidenceBufferMismatch as u32
    );
    assert_eq!(
        account(&mut context, stage).await.unwrap(),
        complete_stale_stage
    );
    assert!(account(&mut context, final_account).await.is_none());
    send(
        &mut context,
        abort_ix(
            author.pubkey(),
            stage,
            author.pubkey(),
            kind,
            binding_context,
            digest,
        ),
        &author,
    )
    .await
    .expect("funder aborts refused stale-digest stage");
    assert!(account(&mut context, stage).await.is_none());

    send(
        &mut context,
        begin_ix(author.pubkey(), stage, kind, binding_context, digest, 1_000),
        &author,
    )
    .await
    .expect("begin basis stage");
    let initial_stage = account(&mut context, stage).await.unwrap();

    // A skipped first chunk and an incomplete seal both roll back exactly.
    assert!(send(
        &mut context,
        write_ix(
            author.pubkey(),
            stage,
            kind,
            binding_context,
            digest,
            ARTIFACT_CHUNK_BYTES,
            &body,
        ),
        &author,
    )
    .await
    .is_err());
    assert_eq!(account(&mut context, stage).await.unwrap(), initial_stage);
    assert!(send(
        &mut context,
        seal_ix(
            author.pubkey(),
            stage,
            final_account,
            kind,
            binding_context,
            digest,
        ),
        &author,
    )
    .await
    .is_err());
    assert_eq!(account(&mut context, stage).await.unwrap(), initial_stage);
    assert!(account(&mut context, final_account).await.is_none());

    send(
        &mut context,
        write_ix(
            author.pubkey(),
            stage,
            kind,
            binding_context,
            digest,
            0,
            &body,
        ),
        &author,
    )
    .await
    .expect("first ordered basis chunk");
    let after_first = account(&mut context, stage).await.unwrap();
    assert!(send(
        &mut context,
        write_ix(
            author.pubkey(),
            stage,
            kind,
            binding_context,
            digest,
            0,
            &body,
        ),
        &author,
    )
    .await
    .is_err());
    assert_eq!(account(&mut context, stage).await.unwrap(), after_first);

    let mut cursor = ARTIFACT_CHUNK_BYTES;
    while cursor < body.len() {
        send(
            &mut context,
            write_ix(
                author.pubkey(),
                stage,
                kind,
                binding_context,
                digest,
                cursor,
                &body,
            ),
            &author,
        )
        .await
        .expect("ordered basis chunk");
        cursor += ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
    }
    let complete_stage = account(&mut context, stage).await.unwrap();
    let mut wrong_digest_bytes = digest.bytes();
    wrong_digest_bytes[0] ^= 1;
    let wrong_digest = Hash32::from_bytes(wrong_digest_bytes);
    let (wrong_final, _) = derive_final(kind, binding_context, wrong_digest);
    assert!(send(
        &mut context,
        seal_ix(
            author.pubkey(),
            stage,
            wrong_final,
            kind,
            binding_context,
            wrong_digest,
        ),
        &author,
    )
    .await
    .is_err());
    assert_eq!(account(&mut context, stage).await.unwrap(), complete_stage);
    assert!(account(&mut context, wrong_final).await.is_none());

    let (seal_result, seal_units) = send_measured(
        &mut context,
        seal_ix(
            author.pubkey(),
            stage,
            final_account,
            kind,
            binding_context,
            digest,
        ),
        &author,
    )
    .await;
    seal_result.expect("native-hashed basis seal");
    assert!(seal_units > 0 && seal_units <= 200_000);
    eprintln!("product_series_basis_seal_compute_units={seal_units}");
    assert!(account(&mut context, stage).await.is_none());
    let sealed = account(&mut context, final_account).await.unwrap();
    assert_eq!(sealed.owner, PROGRAM_ID);
    assert_eq!(sealed.data, body);
    assert_eq!(
        sealed.lamports,
        Rent::default().minimum_balance(BASIS_BYTES)
    );

    // A second complete upload converges on the exact existing final instead
    // of inventing another account or treating publication as exclusive.
    let (_, converged_final) =
        upload_all(&mut context, &author, kind, binding_context, digest, &body).await;
    assert_eq!(converged_final, final_account);
    assert_eq!(account(&mut context, final_account).await.unwrap(), sealed);

    // The eight smaller Product/Series kinds take distinct target-only codec
    // and domain-dispatch arms. Drive each through this same real ELF rather
    // than projecting host validation onto those arms.
    for (other_kind, other_body, other_digest) in other_product_artifact_bodies() {
        let (other_stage, other_final) = upload_all(
            &mut context,
            &author,
            other_kind,
            Hash32::ZERO,
            other_digest,
            &other_body,
        )
        .await;
        assert!(account(&mut context, other_stage).await.is_none());
        let published = account(&mut context, other_final).await.unwrap();
        assert_eq!(published.owner, PROGRAM_ID, "{other_kind:?}");
        assert_eq!(published.data, other_body, "{other_kind:?}");
        assert_eq!(
            published.lamports,
            Rent::default().minimum_balance(other_kind.exact_len()),
            "{other_kind:?}"
        );
    }

    // A digest alone is not admission. Give the generic target path a
    // correctly self-hashed Recovery body whose first reserved byte is
    // nonzero. The owning codec must refuse before a final account exists.
    let malformed_kind = ArtifactKind::EvidenceOnlyRecoveryPolicyV1;
    let mut malformed_body = vec![0_u8; malformed_kind.exact_len()];
    product_recovery().encode_into(&mut malformed_body).unwrap();
    malformed_body[11] = 1;
    assert!(EvidenceOnlyRecoveryPolicyV1::decode(&malformed_body).is_err());
    let malformed_digest = typed_digest(RECOVERY_POLICY_DOMAIN, &malformed_body);
    let (malformed_stage, _) = derive_stage(
        author.pubkey(),
        malformed_kind,
        Hash32::ZERO,
        malformed_digest,
    );
    let (malformed_final, _) = derive_final(malformed_kind, Hash32::ZERO, malformed_digest);
    send(
        &mut context,
        begin_ix(
            author.pubkey(),
            malformed_stage,
            malformed_kind,
            Hash32::ZERO,
            malformed_digest,
            1_000,
        ),
        &author,
    )
    .await
    .expect("begin self-hashed malformed recovery");
    write_all_chunks(
        &mut context,
        &author,
        malformed_stage,
        malformed_kind,
        Hash32::ZERO,
        malformed_digest,
        &malformed_body,
    )
    .await;
    let malformed_before = account(&mut context, malformed_stage).await.unwrap();
    assert_eq!(
        custom(
            send(
                &mut context,
                seal_ix(
                    author.pubkey(),
                    malformed_stage,
                    malformed_final,
                    malformed_kind,
                    Hash32::ZERO,
                    malformed_digest,
                ),
                &author,
            )
            .await
        ),
        clutch_sbf::error::codec_code(clutch_solana_layout::CodecError::MismatchedBinding)
    );
    assert_eq!(
        account(&mut context, malformed_stage).await.unwrap(),
        malformed_before
    );
    assert!(account(&mut context, malformed_final).await.is_none());
}

#[tokio::test]
async fn every_admitted_artifact_kind_lands_as_its_exact_raw_codec() {
    let author = uploader();
    let author_genesis = Account {
        lamports: UPLOADER_LAMPORTS,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    };
    let mut context = new_bank(&[(author.pubkey(), author_genesis)])
        .start_with_context()
        .await;

    let policy = fixture_policy([0x91; 32]);
    let policy_digest = policy.digest().expect("policy digest");
    let profile = ParentProfile::from_policy(&policy)
        .and_then(|parent| parent.identity())
        .expect("parent Profile identity");
    let policy_body = policy.canonical_bytes().expect("policy bytes");
    assert_eq!(policy_body.len(), collateral::COLLATERAL_POLICY_BYTES);
    let (policy_stage, policy_final) = upload_all(
        &mut context,
        &author,
        ArtifactKind::CollateralPolicy,
        profile,
        policy_digest,
        &policy_body,
    )
    .await;
    assert!(account(&mut context, policy_stage).await.is_none());
    assert_eq!(
        account(&mut context, policy_final).await.unwrap().data,
        policy_body
    );

    let realm = Hash32::from_bytes([0x41; 32]);
    let mut ticks = [0; MAX_GRID_TICKS];
    ticks[..3].copy_from_slice(&[2_500, 5_000, 7_500]);
    let mut grid = PriceGridAccount {
        grid: Hash32::ZERO,
        realm,
        price_scale: 10_000,
        tick_count: 3,
        ticks,
        stored_bump: 0,
        flags: 0,
    };
    grid.grid = grid.recomputed_grid_id().expect("grid digest");
    let (_, grid_bump) = derive_final(ArtifactKind::PriceGrid, realm, grid.grid);
    grid.stored_bump = grid_bump;
    let mut grid_body = vec![0; account_len::PRICE_GRID];
    assert_eq!(grid.encode(&mut grid_body), Ok(account_len::PRICE_GRID));
    let (grid_stage, grid_final) = upload_all(
        &mut context,
        &author,
        ArtifactKind::PriceGrid,
        realm,
        grid.grid,
        &grid_body,
    )
    .await;
    assert!(account(&mut context, grid_stage).await.is_none());
    assert_eq!(
        account(&mut context, grid_final).await.unwrap().data,
        grid_body
    );
}

#[tokio::test]
async fn one_lamport_stage_and_final_prefunds_are_topped_up_by_exact_shortfalls() {
    let author = uploader();
    let kind = ArtifactKind::CollateralPolicy;
    let policy = fixture_policy([0xa1; 32]);
    let digest = policy.digest().expect("policy digest");
    let profile = ParentProfile::from_policy(&policy)
        .and_then(|parent| parent.identity())
        .expect("profile identity");
    let body = policy.canonical_bytes().expect("policy bytes");
    let (stage, _) = derive_stage(author.pubkey(), kind, profile, digest);
    let (final_account, _) = derive_final(kind, profile, digest);
    let stage_minimum = Rent::default()
        .minimum_balance(ARTIFACT_STAGE_HEADER_BYTES + body.len())
        .max(1);
    let final_minimum = Rent::default().minimum_balance(body.len()).max(1);

    let mut context = new_bank(&[
        (author.pubkey(), empty_system_account(UPLOADER_LAMPORTS)),
        (stage, empty_system_account(1)),
        (final_account, empty_system_account(1)),
    ])
    .start_with_context()
    .await;
    let before_begin = account(&mut context, author.pubkey())
        .await
        .unwrap()
        .lamports;
    send(
        &mut context,
        begin_ix(author.pubkey(), stage, kind, profile, digest, 1_000),
        &author,
    )
    .await
    .expect("one-lamport stage prefund cannot squat BeginArtifact");
    let staged = account(&mut context, stage).await.expect("allocated stage");
    assert_eq!(staged.owner, PROGRAM_ID);
    assert_eq!(staged.lamports, stage_minimum);
    assert_eq!(
        account(&mut context, author.pubkey())
            .await
            .unwrap()
            .lamports,
        before_begin - (stage_minimum - 1),
        "Begin debits exactly the rent shortfall"
    );

    write_all_chunks(&mut context, &author, stage, kind, profile, digest, &body).await;
    let before_seal = account(&mut context, author.pubkey())
        .await
        .unwrap()
        .lamports;
    send(
        &mut context,
        seal_ix(author.pubkey(), stage, final_account, kind, profile, digest),
        &author,
    )
    .await
    .expect("one-lamport final prefund cannot squat SealArtifact");
    assert!(account(&mut context, stage).await.is_none());
    let final_state = account(&mut context, final_account)
        .await
        .expect("allocated final");
    assert_eq!(final_state.owner, PROGRAM_ID);
    assert_eq!(final_state.data, body);
    assert_eq!(final_state.lamports, final_minimum);
    assert_eq!(
        account(&mut context, author.pubkey())
            .await
            .unwrap()
            .lamports,
        before_seal - (final_minimum - 1) + stage_minimum,
        "Seal debits only the final rent shortfall and returns the whole stage"
    );
}

#[tokio::test]
async fn excess_prefunds_are_donations_and_never_squatting_authority() {
    let author = uploader();
    let kind = ArtifactKind::CollateralPolicy;
    let policy = fixture_policy([0xb1; 32]);
    let digest = policy.digest().expect("policy digest");
    let profile = ParentProfile::from_policy(&policy)
        .and_then(|parent| parent.identity())
        .expect("profile identity");
    let body = policy.canonical_bytes().expect("policy bytes");
    let (stage, _) = derive_stage(author.pubkey(), kind, profile, digest);
    let (final_account, _) = derive_final(kind, profile, digest);
    let stage_donation = Rent::default()
        .minimum_balance(ARTIFACT_STAGE_HEADER_BYTES + body.len())
        .max(1)
        + 37;
    let final_donation = Rent::default().minimum_balance(body.len()).max(1) + 53;

    let mut context = new_bank(&[
        (author.pubkey(), empty_system_account(UPLOADER_LAMPORTS)),
        (stage, empty_system_account(stage_donation)),
        (final_account, empty_system_account(final_donation)),
    ])
    .start_with_context()
    .await;
    let before_begin = account(&mut context, author.pubkey())
        .await
        .unwrap()
        .lamports;
    send(
        &mut context,
        begin_ix(author.pubkey(), stage, kind, profile, digest, 1_000),
        &author,
    )
    .await
    .expect("overfunded stage remains creatable");
    assert_eq!(
        account(&mut context, author.pubkey())
            .await
            .unwrap()
            .lamports,
        before_begin,
        "an overfunded target causes no payer debit"
    );
    assert_eq!(
        account(&mut context, stage).await.unwrap().lamports,
        stage_donation
    );

    write_all_chunks(&mut context, &author, stage, kind, profile, digest, &body).await;
    let before_seal = account(&mut context, author.pubkey())
        .await
        .unwrap()
        .lamports;
    send(
        &mut context,
        seal_ix(author.pubkey(), stage, final_account, kind, profile, digest),
        &author,
    )
    .await
    .expect("overfunded final remains creatable");
    assert_eq!(
        account(&mut context, final_account).await.unwrap().lamports,
        final_donation,
        "the persistent final retains its unsolicited excess"
    );
    assert_eq!(
        account(&mut context, author.pubkey())
            .await
            .unwrap()
            .lamports,
        before_seal + stage_donation,
        "the transient stage keeps one close destination, including donations"
    );
}

#[tokio::test]
async fn native_terms_hash_rejects_a_semantically_valid_body_with_a_stale_digest() {
    let author = uploader();
    let realm = Hash32::from_bytes([0x81; 32]);
    let profile = Hash32::from_bytes([0x82; 32]);
    let feed = Hash32::from_bytes([0x83; 32]);
    let (mut body, digest, _) = encode_terms(fixture_terms(realm, profile, feed));
    // The final eight body bytes before the seven reserved bytes and the
    // two-byte bump/flags trailer are the collateral cap. Changing its low
    // bit preserves every structural rule while invalidating only the frozen
    // terms digest, directly exercising the SBF native-SHA boundary.
    let collateral_cap_offset = body.len() - 2 - 7 - 8;
    body[collateral_cap_offset] ^= 1;
    assert_eq!(
        TermsAccount::decode(&body),
        Err(clutch_solana_layout::CodecError::NonCanonicalIdentity)
    );

    let (stage, _) = derive_stage(author.pubkey(), ArtifactKind::Terms, realm, digest);
    let (final_account, _) = derive_final(ArtifactKind::Terms, realm, digest);
    let author_genesis = Account {
        lamports: UPLOADER_LAMPORTS,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    };
    let final_prefund = empty_system_account(1);
    let mut context = new_bank(&[
        (author.pubkey(), author_genesis),
        (final_account, final_prefund.clone()),
    ])
    .start_with_context()
    .await;
    send(
        &mut context,
        begin_ix(
            author.pubkey(),
            stage,
            ArtifactKind::Terms,
            realm,
            digest,
            1_000,
        ),
        &author,
    )
    .await
    .expect("tampered body still has a typed stage");
    let mut cursor = 0;
    while cursor < body.len() {
        send(
            &mut context,
            write_ix(
                author.pubkey(),
                stage,
                ArtifactKind::Terms,
                realm,
                digest,
                cursor,
                &body,
            ),
            &author,
        )
        .await
        .expect("transport does not interpret partial bytes");
        cursor += ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
    }
    let before = account(&mut context, stage).await.unwrap();
    assert_eq!(
        custom(
            send(
                &mut context,
                seal_ix(
                    author.pubkey(),
                    stage,
                    final_account,
                    ArtifactKind::Terms,
                    realm,
                    digest,
                ),
                &author,
            )
            .await
        ),
        clutch_sbf::error::codec_code(clutch_solana_layout::CodecError::NonCanonicalIdentity)
    );
    assert_eq!(
        account(&mut context, stage).await.unwrap(),
        before,
        "digest refusal rolls the complete stage back byte-exactly"
    );
    assert_eq!(
        account(&mut context, final_account).await.unwrap(),
        final_prefund,
        "late semantic refusal rolls a prefunded final back byte-exactly"
    );
}

#[tokio::test]
async fn terms_upload_resumes_after_bank_rehydration_and_seals_atomically() {
    let author = uploader();
    let realm = Hash32::from_bytes([0x31; 32]);
    let profile = Hash32::from_bytes([0x42; 32]);
    let feed = Hash32::from_bytes([0x53; 32]);
    let (body, digest, final_bump) = encode_terms(fixture_terms(realm, profile, feed));
    let kind = ArtifactKind::Terms;
    let (stage, stage_bump) = derive_stage(author.pubkey(), kind, realm, digest);
    let (final_account, observed_bump) = derive_final(kind, realm, digest);
    assert_eq!(observed_bump, final_bump);

    let author_genesis = Account {
        lamports: UPLOADER_LAMPORTS,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    };
    let mut first = new_bank(&[(author.pubkey(), author_genesis)])
        .start_with_context()
        .await;
    send(
        &mut first,
        begin_ix(author.pubkey(), stage, kind, realm, digest, 1_000),
        &author,
    )
    .await
    .expect("BeginArtifact");

    let stage_account = account(&mut first, stage).await.expect("stage exists");
    assert_eq!(stage_account.owner, PROGRAM_ID);
    assert_eq!(
        stage_account.data.len(),
        ARTIFACT_STAGE_HEADER_BYTES + body.len()
    );
    let decoded = decode_stage(&stage_account.data).expect("stage header");
    assert_eq!(decoded.stored_bump, stage_bump);
    assert_eq!(decoded.cursor, 0);

    for cursor in [0, ARTIFACT_CHUNK_BYTES, 2 * ARTIFACT_CHUNK_BYTES] {
        send(
            &mut first,
            write_ix(author.pubkey(), stage, kind, realm, digest, cursor, &body),
            &author,
        )
        .await
        .expect("ordered chunk");
    }

    let before_early = account(&mut first, stage).await.expect("stage remains");
    assert_eq!(
        custom(
            send(
                &mut first,
                seal_ix(author.pubkey(), stage, final_account, kind, realm, digest),
                &author,
            )
            .await
        ),
        ClutchError::ArtifactIncomplete as u32
    );
    assert_eq!(
        account(&mut first, stage).await.expect("stage remains"),
        before_early,
        "early seal rolls back byte-exactly"
    );
    assert!(account(&mut first, final_account).await.is_none());

    let author_checkpoint = account(&mut first, author.pubkey())
        .await
        .expect("author checkpoint");
    let stage_checkpoint = account(&mut first, stage).await.expect("stage checkpoint");
    drop(first);

    let mut restarted = new_bank(&[
        (author.pubkey(), author_checkpoint),
        (stage, stage_checkpoint),
    ])
    .start_with_context()
    .await;
    assert_eq!(
        decode_stage(&account(&mut restarted, stage).await.unwrap().data)
            .unwrap()
            .cursor,
        (3 * ARTIFACT_CHUNK_BYTES) as u16,
        "fresh bank reload preserves the only upload cursor"
    );

    let before_duplicate = account(&mut restarted, stage).await.unwrap();
    assert_eq!(
        custom(
            send(
                &mut restarted,
                write_ix(
                    author.pubkey(),
                    stage,
                    kind,
                    realm,
                    digest,
                    2 * ARTIFACT_CHUNK_BYTES,
                    &body,
                ),
                &author,
            )
            .await
        ),
        clutch_sbf::error::codec_code(clutch_solana_layout::CodecError::MismatchedBinding)
    );
    assert_eq!(
        account(&mut restarted, stage).await.unwrap(),
        before_duplicate
    );

    let mut cursor = 3 * ARTIFACT_CHUNK_BYTES;
    while cursor < body.len() {
        send(
            &mut restarted,
            write_ix(author.pubkey(), stage, kind, realm, digest, cursor, &body),
            &author,
        )
        .await
        .expect("resumed ordered chunk");
        cursor += ARTIFACT_CHUNK_BYTES.min(body.len() - cursor);
    }
    assert!(
        decode_stage(&account(&mut restarted, stage).await.unwrap().data)
            .unwrap()
            .is_complete()
    );

    send(
        &mut restarted,
        seal_ix(author.pubkey(), stage, final_account, kind, realm, digest),
        &author,
    )
    .await
    .expect("SealArtifact");
    assert!(account(&mut restarted, stage).await.is_none());
    let final_state = account(&mut restarted, final_account)
        .await
        .expect("final artifact");
    assert_eq!(final_state.owner, PROGRAM_ID);
    assert_eq!(final_state.data, body);
    let terms = TermsAccount::decode(&final_state.data).expect("sealed terms decode");
    assert_eq!(terms.terms, digest);
    assert_eq!(terms.realm, realm);
    assert_eq!(terms.stored_bump, final_bump);

    let author_before_repeat = account(&mut restarted, author.pubkey())
        .await
        .unwrap()
        .lamports;
    let (repeat_stage, repeat_final) =
        upload_all(&mut restarted, &author, kind, realm, digest, &body).await;
    assert_eq!(repeat_final, final_account);
    assert!(account(&mut restarted, repeat_stage).await.is_none());
    assert_eq!(
        account(&mut restarted, final_account).await.unwrap().data,
        body,
        "idempotent seal admits only the same exact immutable bytes"
    );
    assert_eq!(
        account(&mut restarted, author.pubkey())
            .await
            .unwrap()
            .lamports,
        author_before_repeat,
        "an already-present final charges no second rent and stage rent returns"
    );
}

#[tokio::test]
async fn expired_public_abort_refunds_the_recorded_funder_not_the_reaper() {
    let author = uploader();
    let janitor = reaper();
    let kind = ArtifactKind::Terms;
    let realm = Hash32::from_bytes([0x61; 32]);
    let profile = Hash32::from_bytes([0x62; 32]);
    let feed = Hash32::from_bytes([0x63; 32]);
    let (_body, digest, _) = encode_terms(fixture_terms(realm, profile, feed));
    let (stage, _) = derive_stage(author.pubkey(), kind, realm, digest);
    let base = |lamports| Account {
        lamports,
        data: Vec::new(),
        owner: SYSTEM_PROGRAM,
        executable: false,
        rent_epoch: 0,
    };
    let mut context = new_bank(&[
        (author.pubkey(), base(UPLOADER_LAMPORTS)),
        (janitor.pubkey(), base(10_000_000)),
    ])
    .start_with_context()
    .await;
    send(
        &mut context,
        begin_ix(author.pubkey(), stage, kind, realm, digest, 20),
        &author,
    )
    .await
    .expect("short bounded BeginArtifact");
    let staged = account(&mut context, stage).await.unwrap();
    let author_before = account(&mut context, author.pubkey())
        .await
        .unwrap()
        .lamports;
    let reaper_before = account(&mut context, janitor.pubkey())
        .await
        .unwrap()
        .lamports;

    assert_eq!(
        custom(
            send(
                &mut context,
                abort_ix(
                    janitor.pubkey(),
                    stage,
                    author.pubkey(),
                    kind,
                    realm,
                    digest
                ),
                &janitor,
            )
            .await
        ),
        ClutchError::UnauthorizedActor as u32
    );
    assert_eq!(account(&mut context, stage).await.unwrap(), staged);

    context.warp_to_slot(21).expect("warp beyond expiry");
    send(
        &mut context,
        abort_ix(
            janitor.pubkey(),
            stage,
            author.pubkey(),
            kind,
            realm,
            digest,
        ),
        &janitor,
    )
    .await
    .expect("public expired abort");
    assert!(account(&mut context, stage).await.is_none());
    assert_eq!(
        account(&mut context, author.pubkey())
            .await
            .unwrap()
            .lamports,
        author_before + staged.lamports,
        "all stage rent returns to the persisted funder"
    );
    assert!(
        account(&mut context, janitor.pubkey())
            .await
            .unwrap()
            .lamports
            <= reaper_before,
        "the reaper receives no stage rent (and may pay a fee)"
    );
}
