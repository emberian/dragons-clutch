//! Real Registry ELF finalization above the former one-transaction hash ceiling.
//! Genesis supplies a valid padded Loader deployment; publication and subsequent
//! deployment changes run through Registry and the native Loader respectively.
use dclutch_core_contract::ContentId;
use dclutch_registry::artifact_code_commitment_v2::{
    CODE_COMMITMENT_CHUNK_BYTES_V2, code_commitment_v2,
};
use dclutch_registry::record::{
    ARTIFACT_STAGING_CURSOR_BYTES_V2, AbortRecordV1, AppendPageV1, BeginRecordV1,
    CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1, ContentDigest, FinalizeRecordV1,
    RAW_RECORD_PDA_SEED_V1, RecordKeyV1, STAGING_CURSOR_PDA_SEED_V1, SchemaReleaseId,
    artifact_verification_progress_v2,
};
use dclutch_registry::release_set::ProgramIdentityV1;
use dclutch_registry::{ARTIFACT_RELEASE_SCHEMA_ID_V2, ArtifactReleaseV2, ArtifactUpgradePolicyV1};
use dclutch_registry_sbf::RegistryError;
use solana_account::Account;
use solana_loader_v3_interface::{instruction, state::UpgradeableLoaderState};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    hash::hash,
    instruction::{AccountMeta, Instruction, InstructionError},
    pubkey::Pubkey,
    signature::Signer,
    transaction::TransactionError,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
use solana_transaction::Transaction;

async fn send(ctx: &mut ProgramTestContext, ix: Instruction) -> Result<(), TransactionError> {
    let blockhash = ctx
        .get_new_latest_blockhash()
        .await
        .expect("fresh blockhash");
    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&ctx.payer.pubkey()),
        &[&ctx.payer],
        blockhash,
    );
    let result = ctx
        .banks_client
        .process_transaction_with_metadata(tx)
        .await
        .expect("runtime result");
    let metadata = result.metadata.expect("actual execution metadata");
    println!(
        "ArtifactV2 runtime result={:?} CU={} logs={:?}",
        result.result, metadata.compute_units_consumed, metadata.log_messages
    );
    assert!(metadata.compute_units_consumed <= 1_400_000);
    result.result
}
async fn read(ctx: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
    ctx.banks_client
        .get_account(key)
        .await
        .expect("finalized runtime account")
}
fn instruction_for(registry: Pubkey, accounts: Vec<AccountMeta>, data: Vec<u8>) -> Instruction {
    Instruction {
        program_id: registry,
        accounts,
        data,
    }
}
async fn publish(
    ctx: &mut ProgramTestContext,
    registry: Pubkey,
    release: ArtifactReleaseV2,
) -> (Pubkey, Pubkey) {
    let body = release.to_bytes();
    let digest = hash(&body).to_bytes();
    let key = RecordKeyV1::new(
        SchemaReleaseId::new(ARTIFACT_RELEASE_SCHEMA_ID_V2).expect("schema"),
        ContentDigest::new(digest).expect("content"),
    );
    let raw = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V2,
            &digest,
        ],
        &registry,
    )
    .0;
    let cursor = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            &ARTIFACT_RELEASE_SCHEMA_ID_V2,
            &digest,
        ],
        &registry,
    )
    .0;
    let profile = CANONICAL_RECORD_DEPLOYMENT_PROFILE_V1;
    let bounty =
        solana_sdk::rent::Rent::default().minimum_balance(ARTIFACT_STAGING_CURSOR_BYTES_V2);
    let begin = BeginRecordV1::new(
        key,
        body.len() as u64,
        profile.page_envelope().expect("envelope"),
        profile
            .staging_liveness_policy(bounty)
            .expect("policy")
            .policy_id(),
        100_000,
        bounty,
    )
    .expect("begin");
    let payer = ctx.payer.pubkey();
    send(
        ctx,
        instruction_for(
            registry,
            vec![
                AccountMeta::new(payer, true),
                AccountMeta::new(raw, false),
                AccountMeta::new(cursor, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
            ],
            begin.to_bytes().to_vec(),
        ),
    )
    .await
    .expect("native Begin");
    let staging = read(ctx, cursor).await.expect("allocated cursor");
    assert_eq!(staging.owner, registry);
    assert_eq!(staging.data.len(), ARTIFACT_STAGING_CURSOR_BYTES_V2);
    assert_eq!(artifact_verification_progress_v2(&staging.data), Ok(None));
    let append = AppendPageV1::new(0, 0, &body).expect("append");
    let mut bytes = vec![0; append.encoded_len().expect("encoded width")];
    append.encode(&mut bytes).expect("encode");
    send(
        ctx,
        instruction_for(
            registry,
            vec![
                AccountMeta::new_readonly(payer, true),
                AccountMeta::new(raw, false),
                AccountMeta::new(cursor, false),
            ],
            bytes,
        ),
    )
    .await
    .expect("native Append");
    assert_eq!(read(ctx, raw).await.expect("uploaded raw").data, body);
    (raw, cursor)
}
fn finalize(
    registry: Pubkey,
    raw: Pubkey,
    cursor: Pubkey,
    payer: Pubkey,
    program: Pubkey,
    programdata: Pubkey,
) -> Instruction {
    instruction_for(
        registry,
        vec![
            AccountMeta::new_readonly(raw, false),
            AccountMeta::new(cursor, false),
            AccountMeta::new(payer, false),
            AccountMeta::new_readonly(program, false),
            AccountMeta::new_readonly(programdata, false),
        ],
        FinalizeRecordV1.to_bytes().to_vec(),
    )
}
async fn abort(ctx: &mut ProgramTestContext, registry: Pubkey, raw: Pubkey, cursor: Pubkey) {
    let payer = ctx.payer.pubkey();
    let raw_balance = read(ctx, raw).await.expect("raw prestate").lamports;
    let cursor_balance = read(ctx, cursor).await.expect("cursor prestate").lamports;
    let payer_before = read(ctx, payer).await.expect("payer prestate").lamports;
    send(
        ctx,
        instruction_for(
            registry,
            vec![
                AccountMeta::new(raw, false),
                AccountMeta::new(cursor, false),
                AccountMeta::new(payer, true),
                AccountMeta::new(payer, true),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
            ],
            AbortRecordV1.to_bytes().to_vec(),
        ),
    )
    .await
    .expect("native partial verification Abort");
    assert!(read(ctx, raw).await.is_none());
    assert!(read(ctx, cursor).await.is_none());
    assert_eq!(
        read(ctx, payer).await.expect("payer poststate").lamports,
        payer_before + raw_balance + cursor_balance - 5_000
    );
}

#[tokio::test]
async fn four_mib_native_finalization_is_bounded_contiguous_and_reclaims_partial_verification() {
    let mut elf =
        std::fs::read(std::env::var_os("DCLUTCH_LOADER_TEST_ELF").expect("real Loader ELF path"))
            .expect("ELF");
    assert_eq!(elf.get(..4), Some(&b"\x7fELF"[..]));
    assert!(elf.len() < CODE_COMMITMENT_CHUNK_BYTES_V2);
    elf.resize(4 * CODE_COMMITMENT_CHUNK_BYTES_V2, 0);
    let commitment = code_commitment_v2(&elf).expect("complete padded code commitment");
    let registry = Pubkey::new_unique();
    let program = Pubkey::new_unique();
    let programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    let mut test = ProgramTest::default();
    test.prefer_bpf(true);
    test.set_compute_max_units(1_400_000);
    test.deactivate_feature(agave_feature_set::disable_sbpf_v0_v1_v2_deployment::id());
    test.add_upgradeable_program_to_genesis("dclutch_registry_sbf", &registry);
    // Fixed native authority; the permissionless Extend route below does not
    // need its signature and still updates the Loader deployment slot.
    let authority = Pubkey::new_unique();
    let mut data = bincode::serialize(&UpgradeableLoaderState::ProgramData {
        slot: 0,
        upgrade_authority_address: Some(authority),
    })
    .expect("native metadata");
    data.resize(UpgradeableLoaderState::size_of_programdata_metadata(), 0);
    data.extend_from_slice(&elf);
    for (key, data, executable) in [
        (
            program,
            bincode::serialize(&UpgradeableLoaderState::Program {
                programdata_address: programdata,
            })
            .expect("native Program"),
            true,
        ),
        (programdata, data, false),
    ] {
        test.add_account(
            key,
            Account {
                lamports: 100_000_000_000,
                data,
                owner: bpf_loader_upgradeable::ID,
                executable,
                rent_epoch: 0,
            },
        );
    }
    let mut ctx = test.start_with_context().await;
    let payer = ctx.payer.pubkey();
    let release = |semantic, commitment| {
        ArtifactReleaseV2::new(
            ProgramIdentityV1::new(program.to_bytes()).expect("program"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
            programdata.to_bytes(),
            ContentId::new([semantic; 32]).expect("semantic"),
            commitment,
            0,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(authority.to_bytes()),
        )
        .expect("release")
    };
    let accepted = release(1, commitment);
    let (raw, cursor) = publish(&mut ctx, registry, accepted).await;
    let cursor_balance = read(&mut ctx, cursor).await.expect("cursor").lamports;
    for index in 1..=4_u64 {
        let wallet_before = read(&mut ctx, payer).await.expect("wallet").lamports;
        send(
            &mut ctx,
            finalize(registry, raw, cursor, payer, program, programdata),
        )
        .await
        .expect("bounded native Finalize");
        assert_eq!(
            read(&mut ctx, raw).await.expect("raw retained").data,
            accepted.to_bytes()
        );
        if index < 4 {
            let state = read(&mut ctx, cursor).await.expect("not yet finalized");
            let progress = artifact_verification_progress_v2(&state.data)
                .expect("native cursor")
                .expect("partial progress");
            assert_eq!(
                progress.next_offset(),
                index * CODE_COMMITMENT_CHUNK_BYTES_V2 as u64
            );
            assert_eq!(progress.total_length(), elf.len() as u64);
            assert_eq!(state.lamports, cursor_balance);
        } else {
            assert!(read(&mut ctx, cursor).await.is_none());
            assert_eq!(
                read(&mut ctx, payer).await.expect("refund").lamports,
                wallet_before + cursor_balance - 5_000
            );
        }
    }
    // Native bytes are unchanged: only the untrusted proposed record commits
    // an altered final byte. Its last step refuses and preserves all prestates.
    *elf.last_mut().expect("nonempty") = 1;
    let hostile = release(2, code_commitment_v2(&elf).expect("altered commitment"));
    let (bad_raw, bad_cursor) = publish(&mut ctx, registry, hostile).await;
    for _ in 0..3 {
        send(
            &mut ctx,
            finalize(registry, bad_raw, bad_cursor, payer, program, programdata),
        )
        .await
        .expect("proper prefix");
    }
    let before = read(&mut ctx, bad_cursor).await.expect("partial prestate");
    assert_eq!(
        send(
            &mut ctx,
            finalize(registry, bad_raw, bad_cursor, payer, program, programdata)
        )
        .await,
        Err(TransactionError::InstructionError(
            0,
            InstructionError::Custom(RegistryError::ArtifactReleaseCodeCommitmentMismatch as u32)
        ))
    );
    assert_eq!(read(&mut ctx, bad_cursor).await.expect("rollback"), before);
    abort(&mut ctx, registry, bad_raw, bad_cursor).await;
    // Extend does not require the upgrade authority in the supported native
    // Loader, but does change the deployment slot and exact payload length.
    let (stale_raw, stale_cursor) = publish(&mut ctx, registry, release(3, commitment)).await;
    send(
        &mut ctx,
        finalize(
            registry,
            stale_raw,
            stale_cursor,
            payer,
            program,
            programdata,
        ),
    )
    .await
    .expect("first verified chunk");
    let before = read(&mut ctx, stale_cursor)
        .await
        .expect("partial prestate");
    send(
        &mut ctx,
        instruction::extend_program(
            &program,
            Some(&payer),
            instruction::MINIMUM_EXTEND_PROGRAM_BYTES,
        ),
    )
    .await
    .expect("actual native Loader Extend");
    assert_eq!(
        send(
            &mut ctx,
            finalize(
                registry,
                stale_raw,
                stale_cursor,
                payer,
                program,
                programdata
            )
        )
        .await,
        Err(TransactionError::InstructionError(
            0,
            InstructionError::Custom(RegistryError::ReleaseSuperseded as u32)
        ))
    );
    assert_eq!(
        read(&mut ctx, stale_cursor).await.expect("rollback"),
        before
    );
    abort(&mut ctx, registry, stale_raw, stale_cursor).await;
}
