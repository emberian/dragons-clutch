#![cfg(feature = "non-production-mock-source")]

//! NON-PRODUCTION local-bank evidence for the mock authenticated-source ELF.
//!
//! This test must be run only after building `clutch-sbf` with
//! `--features non-production-mock-source`. That feature changes the ELF and
//! registers a deterministic laboratory parser. No live provider ABI or
//! default-artifact capability follows from this result. Terms are installed
//! as a canonical program-owned prerequisite; SourceSpec, Feed and
//! SourceArchive begin truly absent (apart from explicit System-owned SOL
//! prefunding) and are created by wallet transactions.

use {
    clutch_sbf::{
        error::ClutchError,
        instructions::source_ingest,
        seeds,
        source::{
            SourceSpecFieldsV1, SourceSpecV1, ORIENTATION_QUOTE_PER_BASE,
            SELECTION_FINALIZED_BUCKET_RECORD,
        },
        source_archive::{
            canonical_window_id, verify_recorded_sealed_archive, verify_source_spec_account,
            ArchiveAccountViewV1, CoveragePolicy, FeedIdentity, Grid, SourceSpecAccountViewV1,
            WindowDomain, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES, SOURCE_SPEC_ACCOUNT_V1_BYTES,
        },
    },
    clutch_solana_layout::{account_len, FeedAccount, Hash32, Intent},
    clutch_svm_fixture::{
        compute_unit_limit_data, fixture_terms, layout_request, COMPUTE_BUDGET, PROGRAM_ID,
        RENT_SYSVAR, SYSTEM_PROGRAM,
    },
    solana_account::{Account, AccountSharedData},
    solana_address::Address,
    solana_instruction::{error::InstructionError, AccountMeta, Instruction},
    solana_program_test::{tokio, ProgramTest, ProgramTestContext},
    solana_signer::Signer,
    solana_transaction::Transaction,
    solana_transaction_error::TransactionError,
};

const CLOCK_SYSVAR: Address = Address::new_from_array([
    6, 167, 213, 23, 24, 199, 116, 201, 40, 86, 99, 152, 105, 29, 94, 182, 139, 94, 184, 163, 155,
    75, 109, 92, 115, 85, 91, 33, 0, 0, 0, 0,
]);
const MOCK_ADAPTER: [u8; 32] = [0xa1; 32];
const MOCK_PROGRAM: Address = Address::new_from_array([0xb2; 32]);
const MOCK_PROGRAM_OWNER: Address = Address::new_from_array([0xb3; 32]);
const MOCK_DEPLOYMENT: Address = Address::new_from_array([0xd4; 32]);
const MOCK_DEPLOYMENT_OWNER: Address = Address::new_from_array([0xd5; 32]);
const MOCK_SOURCE: Address = Address::new_from_array([0xc3; 32]);
const SUBSTITUTE_DEPLOYMENT: Address = Address::new_from_array([0xe4; 32]);
const SUBSTITUTE_SOURCE: Address = Address::new_from_array([0xe3; 32]);
const DEPLOYMENT_GENERATION: u64 = 19;

fn h(byte: u8) -> Hash32 {
    Hash32::from_bytes([byte; 32])
}

fn derive(seeds: &[&[u8]]) -> (Address, u8) {
    Address::find_program_address(seeds, &PROGRAM_ID)
}

fn budget() -> Instruction {
    Instruction::new_with_bytes(COMPUTE_BUDGET, &compute_unit_limit_data(1_400_000), vec![])
}

async fn send(
    bank: &mut ProgramTestContext,
    instructions: &[Instruction],
) -> (Result<(), TransactionError>, u64) {
    let blockhash = bank.banks_client.get_latest_blockhash().await.unwrap();
    let transaction = Transaction::new_signed_with_payer(
        instructions,
        Some(&bank.payer.pubkey()),
        &[&bank.payer],
        blockhash,
    );
    let outcome = bank
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap();
    (
        outcome.result,
        outcome
            .metadata
            .map(|metadata| metadata.compute_units_consumed)
            .unwrap_or_default(),
    )
}

async fn succeed(bank: &mut ProgramTestContext, instructions: &[Instruction]) -> u64 {
    let (result, units) = send(bank, instructions).await;
    result.expect("source transaction should succeed");
    units
}

async fn get(bank: &mut ProgramTestContext, address: Address) -> Option<Account> {
    bank.banks_client.get_account(address).await.unwrap()
}

fn account(owner: Address, data: Vec<u8>, executable: bool, lamports: u64) -> AccountSharedData {
    AccountSharedData::from(Account {
        lamports,
        data,
        owner,
        executable,
        rent_epoch: 0,
    })
}

fn clock(account: &Account) -> (u64, u64) {
    let slot = u64::from_le_bytes(account.data[0..8].try_into().unwrap());
    let unix = i64::from_le_bytes(account.data[32..40].try_into().unwrap());
    (
        slot,
        u64::try_from(unix).expect("ProgramTest clock is non-negative"),
    )
}

fn spec() -> SourceSpecV1 {
    SourceSpecV1::new(SourceSpecFieldsV1 {
        source_adapter_id: h(0xa1),
        source_adapter_version: 7,
        parser_id: 11,
        parser_version: 3,
        source_program: MOCK_PROGRAM.to_bytes(),
        source_account: MOCK_SOURCE.to_bytes(),
        deployment_generation: DEPLOYMENT_GENERATION,
        base_asset_id: h(1),
        quote_asset_id: h(2),
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 6,
        grid_family_id: 5,
        grid_version: 2,
        bucket_seconds: 1,
        max_staleness_slots: 20,
        max_staleness_seconds: 120,
        max_future_seconds: 2,
        max_confidence_atoms: 10_000,
        max_confidence_bps: 200,
        confidence_multiplier: 2,
        selection_rule: SELECTION_FINALIZED_BUCKET_RECORD,
    })
    .unwrap()
}

fn source_record(bucket: u64, sequence: u64, slot: u64, time: u64) -> Vec<u8> {
    let mut out = vec![0_u8; 77];
    out[..4].copy_from_slice(b"SRC1");
    out[4..12].copy_from_slice(&DEPLOYMENT_GENERATION.to_le_bytes());
    out[12..20].copy_from_slice(&sequence.to_le_bytes());
    out[20..28].copy_from_slice(&slot.to_le_bytes());
    out[28..36].copy_from_slice(&time.to_le_bytes());
    out[36..44].copy_from_slice(&bucket.to_le_bytes());
    out[44..60].copy_from_slice(&1_000_000_u128.to_le_bytes());
    out[60..76].copy_from_slice(&10_u128.to_le_bytes());
    out[76] = 1;
    out
}

fn init_spec_ix(
    payer: Address,
    terms: Hash32,
    spec: SourceSpecV1,
    spec_pda: Address,
    feed_pda: Address,
    terms_pda: Address,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            0,
            Intent::InitSourceSpec {
                terms,
                spec_body: spec.encode_canonical(),
            },
        ),
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(spec_pda, false),
            AccountMeta::new(feed_pda, false),
            AccountMeta::new_readonly(terms_pda, false),
            AccountMeta::new_readonly(MOCK_PROGRAM, false),
            AccountMeta::new_readonly(MOCK_DEPLOYMENT, false),
            AccountMeta::new_readonly(MOCK_SOURCE, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ],
    )
}

fn init_archive_ix(
    payer: Address,
    terms: Hash32,
    spec_pda: Address,
    feed_pda: Address,
    terms_pda: Address,
    archive_pda: Address,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(0, Intent::InitSourceArchive { terms }),
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(spec_pda, false),
            AccountMeta::new_readonly(feed_pda, false),
            AccountMeta::new_readonly(terms_pda, false),
            AccountMeta::new(archive_pda, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM, false),
            AccountMeta::new_readonly(RENT_SYSVAR, false),
        ],
    )
}

#[allow(clippy::too_many_arguments)]
fn mutate_ix(
    terms: Hash32,
    spec_pda: Address,
    feed_pda: Address,
    terms_pda: Address,
    archive_pda: Address,
    sequence: u64,
    seal: bool,
) -> Instruction {
    Instruction::new_with_bytes(
        PROGRAM_ID,
        &layout_request(
            sequence,
            if seal {
                Intent::SealSourceArchive { terms }
            } else {
                Intent::AppendSourceArchive { terms }
            },
        ),
        vec![
            AccountMeta::new_readonly(spec_pda, false),
            if seal {
                AccountMeta::new(feed_pda, false)
            } else {
                AccountMeta::new_readonly(feed_pda, false)
            },
            AccountMeta::new_readonly(terms_pda, false),
            AccountMeta::new(archive_pda, false),
            AccountMeta::new_readonly(MOCK_PROGRAM, false),
            AccountMeta::new_readonly(MOCK_DEPLOYMENT, false),
            AccountMeta::new_readonly(MOCK_SOURCE, false),
            AccountMeta::new_readonly(CLOCK_SYSVAR, false),
        ],
    )
}

#[tokio::test]
async fn mock_elf_constructs_appends_and_seals_from_absent_source_state() {
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.add_program("clutch_sbf", PROGRAM_ID, None);
    test.add_account(
        MOCK_PROGRAM,
        account(
            MOCK_PROGRAM_OWNER,
            b"MOCK-PROVIDER-V1".to_vec(),
            true,
            10_000_000,
        )
        .into(),
    );
    let mut deployment = b"DEP1".to_vec();
    deployment.extend_from_slice(&DEPLOYMENT_GENERATION.to_le_bytes());
    test.add_account(
        MOCK_DEPLOYMENT,
        account(MOCK_DEPLOYMENT_OWNER, deployment, false, 10_000_000).into(),
    );
    test.add_account(
        MOCK_SOURCE,
        account(MOCK_PROGRAM, vec![0; 77], false, 10_000_000).into(),
    );
    test.add_account(
        SUBSTITUTE_DEPLOYMENT,
        account(
            MOCK_DEPLOYMENT_OWNER,
            b"DEP1\x13\0\0\0\0\0\0\0".to_vec(),
            false,
            10_000_000,
        )
        .into(),
    );
    test.add_account(
        SUBSTITUTE_SOURCE,
        account(MOCK_PROGRAM, vec![0; 77], false, 10_000_000).into(),
    );
    let mut bank = test.start_with_context().await;
    let payer = bank.payer.pubkey();
    let (slot, unix) = clock(&get(&mut bank, CLOCK_SYSVAR).await.unwrap());
    let start = unix.saturating_sub(2);
    let end = unix;

    let spec = spec();
    let mut terms = fixture_terms(h(0x31), h(0x32), spec.feed_id());
    terms.grid_family_id = 5;
    terms.grid_version = 2;
    terms.bucket_seconds = 1;
    terms.expected_start_bucket = start;
    terms.expected_end_bucket_exclusive = end;
    terms.maturity_horizon_buckets = 3;
    terms.coverage_policy_id = 1;
    terms.coverage_policy_parameter = 0;
    terms.repair_policy_id = 1;
    terms.repair_generation = 0;
    terms.source_version = 7;
    terms.evaluator_version = 4;
    terms.source_adapter_id = h(0xa1);
    terms.terms = terms.recomputed_terms_digest().unwrap();
    let (terms_pda, terms_bump) = derive(&[
        seeds::SEED_TERMS,
        &terms.realm.bytes(),
        &terms.terms.bytes(),
    ]);
    terms.stored_bump = terms_bump;
    let rent = bank.banks_client.get_rent().await.unwrap();
    let mut terms_data = vec![0; account_len::TERMS];
    terms.encode(&mut terms_data).unwrap();
    bank.set_account(
        &terms_pda,
        &account(
            PROGRAM_ID,
            terms_data,
            false,
            rent.minimum_balance(account_len::TERMS),
        ),
    );

    let (spec_pda, spec_bump) = derive(&[seeds::SEED_SOURCE_SPEC, &spec.feed_id().bytes()]);
    let (feed_pda, _) = derive(&[seeds::SEED_FEED, &spec.feed_id().bytes()]);
    let feed_identity = FeedIdentity::new(MOCK_ADAPTER, spec.feed_id().bytes(), 7, 4).unwrap();
    let window = WindowDomain::new(
        feed_identity,
        Grid::new(5, 2, 1).unwrap(),
        start,
        end,
        end + 1,
        0,
        CoveragePolicy::COMPLETE_REQUIRED,
    )
    .unwrap();
    let window_id = canonical_window_id(window);
    let (archive_pda, archive_bump) = derive(&[
        seeds::SEED_SOURCE_ARCHIVE,
        &spec.feed_id().bytes(),
        &window_id.bytes(),
    ]);
    assert!(get(&mut bank, spec_pda).await.is_none());
    assert!(get(&mut bank, feed_pda).await.is_none());
    assert!(get(&mut bank, archive_pda).await.is_none());

    /* A one-lamport System-owned PDA cannot be produced by an ordinary
     * transaction under the bank's rent rules, so it is injected solely to
     * exercise that hostile prefund branch. The over-rent Feed and Archive
     * prefunds below are ordinary public transfers. */
    bank.set_account(&spec_pda, &account(SYSTEM_PROGRAM, vec![], false, 1));
    let feed_excess = rent.minimum_balance(account_len::FEED) + 4_321;
    succeed(
        &mut bank,
        &[solana_system_interface::instruction::transfer(
            &payer,
            &feed_pda,
            feed_excess,
        )],
    )
    .await;

    bank.set_account(
        &MOCK_SOURCE,
        &account(
            MOCK_PROGRAM,
            source_record(start, 1, slot.saturating_sub(2), start),
            false,
            10_000_000,
        ),
    );
    let init_spec_cu = succeed(
        &mut bank,
        &[
            budget(),
            init_spec_ix(payer, terms.terms, spec, spec_pda, feed_pda, terms_pda),
        ],
    )
    .await;
    let spec_account = get(&mut bank, spec_pda).await.unwrap();
    assert_eq!(spec_account.owner, PROGRAM_ID);
    assert_eq!(spec_account.data.len(), SOURCE_SPEC_ACCOUNT_V1_BYTES);
    assert_eq!(
        spec_account.lamports,
        rent.minimum_balance(SOURCE_SPEC_ACCOUNT_V1_BYTES)
    );
    assert_eq!(
        get(&mut bank, feed_pda).await.unwrap().lamports,
        feed_excess
    );

    let archive_excess = rent.minimum_balance(SOURCE_ARCHIVE_ACCOUNT_V1_BYTES) + 7_654;
    succeed(
        &mut bank,
        &[solana_system_interface::instruction::transfer(
            &payer,
            &archive_pda,
            archive_excess,
        )],
    )
    .await;
    let init_archive_cu = succeed(
        &mut bank,
        &[
            budget(),
            init_archive_ix(
                payer,
                terms.terms,
                spec_pda,
                feed_pda,
                terms_pda,
                archive_pda,
            ),
        ],
    )
    .await;
    assert_eq!(
        get(&mut bank, archive_pda).await.unwrap().lamports,
        archive_excess
    );

    let append0_cu = succeed(
        &mut bank,
        &[
            budget(),
            mutate_ix(
                terms.terms,
                spec_pda,
                feed_pda,
                terms_pda,
                archive_pda,
                0,
                false,
            ),
        ],
    )
    .await;
    let after_first = get(&mut bank, archive_pda).await.unwrap().data;
    let (replay, _) = send(
        &mut bank,
        &[
            budget(),
            mutate_ix(
                terms.terms,
                spec_pda,
                feed_pda,
                terms_pda,
                archive_pda,
                0,
                false,
            ),
        ],
    )
    .await;
    assert_eq!(
        replay,
        Err(TransactionError::InstructionError(
            1,
            InstructionError::Custom(ClutchError::Replay as u32)
        ))
    );
    assert_eq!(get(&mut bank, archive_pda).await.unwrap().data, after_first);

    let mut substituted = mutate_ix(
        terms.terms,
        spec_pda,
        feed_pda,
        terms_pda,
        archive_pda,
        1,
        false,
    );
    substituted.accounts[5] = AccountMeta::new_readonly(SUBSTITUTE_DEPLOYMENT, false);
    let (wrong_deployment, _) = send(&mut bank, &[budget(), substituted]).await;
    assert_eq!(
        wrong_deployment,
        Err(TransactionError::InstructionError(
            1,
            InstructionError::Custom(ClutchError::SourceAdmissionFailed as u32)
        ))
    );
    assert_eq!(get(&mut bank, archive_pda).await.unwrap().data, after_first);

    let mut substituted = mutate_ix(
        terms.terms,
        spec_pda,
        feed_pda,
        terms_pda,
        archive_pda,
        1,
        false,
    );
    substituted.accounts[6] = AccountMeta::new_readonly(SUBSTITUTE_SOURCE, false);
    let (wrong_source, _) = send(&mut bank, &[budget(), substituted]).await;
    assert_eq!(
        wrong_source,
        Err(TransactionError::InstructionError(
            1,
            InstructionError::Custom(ClutchError::SourceAdmissionFailed as u32)
        ))
    );
    assert_eq!(get(&mut bank, archive_pda).await.unwrap().data, after_first);

    bank.set_account(
        &MOCK_SOURCE,
        &account(
            MOCK_PROGRAM,
            source_record(start + 1, 2, slot.saturating_sub(1), start + 1),
            false,
            10_000_000,
        ),
    );
    let append1_cu = succeed(
        &mut bank,
        &[
            budget(),
            mutate_ix(
                terms.terms,
                spec_pda,
                feed_pda,
                terms_pda,
                archive_pda,
                1,
                false,
            ),
        ],
    )
    .await;

    let before_bad_seal = get(&mut bank, archive_pda).await.unwrap().data;
    let before_bad_feed = get(&mut bank, feed_pda).await.unwrap().data;
    bank.set_account(
        &MOCK_SOURCE,
        &account(
            MOCK_PROGRAM,
            source_record(end + 1, 3, slot, unix),
            false,
            10_000_000,
        ),
    );
    let (bad_seal, _) = send(
        &mut bank,
        &[
            budget(),
            mutate_ix(
                terms.terms,
                spec_pda,
                feed_pda,
                terms_pda,
                archive_pda,
                2,
                true,
            ),
        ],
    )
    .await;
    assert_eq!(
        bad_seal,
        Err(TransactionError::InstructionError(
            1,
            InstructionError::Custom(ClutchError::SourceAdmissionFailed as u32)
        ))
    );
    assert_eq!(
        get(&mut bank, archive_pda).await.unwrap().data,
        before_bad_seal
    );
    assert_eq!(
        get(&mut bank, feed_pda).await.unwrap().data,
        before_bad_feed
    );

    bank.set_account(
        &MOCK_SOURCE,
        &account(
            MOCK_PROGRAM,
            source_record(end, 3, slot, unix),
            false,
            10_000_000,
        ),
    );
    let seal_cu = succeed(
        &mut bank,
        &[
            budget(),
            mutate_ix(
                terms.terms,
                spec_pda,
                feed_pda,
                terms_pda,
                archive_pda,
                2,
                true,
            ),
        ],
    )
    .await;
    let feed = FeedAccount::decode(&get(&mut bank, feed_pda).await.unwrap().data).unwrap();
    assert_eq!(feed.cursor, end + 1);
    assert_eq!(feed.archive_pages, 1);
    let archive = get(&mut bank, archive_pda).await.unwrap();
    let spec_account = get(&mut bank, spec_pda).await.unwrap();
    let verified_spec = verify_source_spec_account(
        PROGRAM_ID.to_bytes(),
        spec_pda.to_bytes(),
        SourceSpecAccountViewV1::new(
            spec_pda.to_bytes(),
            PROGRAM_ID.to_bytes(),
            false,
            &spec_account.data,
        ),
    )
    .unwrap();
    let receipt = verify_recorded_sealed_archive(
        PROGRAM_ID.to_bytes(),
        archive_pda.to_bytes(),
        ArchiveAccountViewV1::new(
            archive_pda.to_bytes(),
            PROGRAM_ID.to_bytes(),
            false,
            &archive.data,
        ),
        verified_spec,
        window,
    )
    .unwrap();
    assert_eq!(receipt.stored_bump(), archive_bump);
    assert_eq!(verified_spec.stored_bump(), spec_bump);
    assert_eq!(receipt.sealed_feed_cursor(), end + 1);
    assert_eq!(feed.summary, receipt.page_commitment());

    println!(
        "NON-PRODUCTION mock source: spec={}B rent={} CU={}; feed={}B rent/excess={} CU(shared init); archive={}B rent/excess={} init={} CU; append=[{},{}] CU; seal={} CU",
        SOURCE_SPEC_ACCOUNT_V1_BYTES,
        rent.minimum_balance(SOURCE_SPEC_ACCOUNT_V1_BYTES),
        init_spec_cu,
        account_len::FEED,
        feed_excess,
        SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
        archive_excess,
        init_archive_cu,
        append0_cu,
        append1_cu,
        seal_cu,
    );
    assert_eq!(source_ingest::INIT_SOURCE_SPEC_ACCOUNT_COUNT, 9);
    assert_eq!(source_ingest::INIT_SOURCE_ARCHIVE_ACCOUNT_COUNT, 7);
    assert_eq!(source_ingest::MUTATE_SOURCE_ARCHIVE_ACCOUNT_COUNT, 8);
}
