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
    let blockhash = context.banks_client.get_latest_blockhash().await.unwrap();
    let transaction = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &[&context.payer, signer],
        blockhash,
    );
    context
        .banks_client
        .process_transaction_with_metadata(transaction)
        .await
        .unwrap()
        .result
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
