//! Actual native Loader transactions behind decision 0012's digest reuse.
//!
//! Supply `DCLUTCH_LOADER_TEST_ELF` with a valid SBF ELF (the SDK noop fixture
//! suffices). ProgramTest runs its locked native Loader, not a mirrored verifier.
//! Genesis supplies one deployment; every subsequent mutation below is a signed
//! Loader instruction. No test writes ProgramData behind the Loader's back.

use dclutch_registry::activation_auth_v1::{
    ActivationAuthErrorV1, cached_role_deployment_observation_v1,
};
use dclutch_registry::release_set::ProgramIdentityV1;
use dclutch_registry::svm::ProgramDataV3View;
use dclutch_registry::{ArtifactReleaseV1, ArtifactUpgradePolicyV1};
use solana_account::Account;
use solana_loader_v3_interface::{instruction, state::UpgradeableLoaderState};
use solana_program::{account_info::AccountInfo, clock::Clock, hash::hash};
use solana_program_test::{ProgramTest, ProgramTestContext};
use solana_sdk::{
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::TransactionError,
};
use solana_sdk_ids::bpf_loader_upgradeable;
use solana_transaction::Transaction;

const PIN: u64 = 20;

async fn send(
    context: &mut ProgramTestContext,
    instruction: Instruction,
    authority: &Keypair,
) -> Result<(), TransactionError> {
    let blockhash = context
        .banks_client
        .get_latest_blockhash()
        .await
        .expect("blockhash");
    let authority_required = instruction
        .accounts
        .iter()
        .any(|meta| meta.pubkey == authority.pubkey() && meta.is_signer);
    let mut signers = vec![&context.payer];
    if authority_required {
        signers.push(authority);
    }
    let tx = Transaction::new_signed_with_payer(
        &[instruction],
        Some(&context.payer.pubkey()),
        &signers,
        blockhash,
    );
    let output = context
        .banks_client
        .process_transaction_with_metadata(tx)
        .await
        .expect("native Loader transaction response");
    let metadata = output.metadata.expect("native Loader execution metadata");
    println!(
        "native Loader result={:?}, CU={}, logs={:?}",
        output.result, metadata.compute_units_consumed, metadata.log_messages
    );
    if matches!(
        output.result,
        Err(TransactionError::InstructionError(
            _,
            InstructionError::InvalidArgument
        ))
    ) {
        assert!(
            metadata.log_messages.iter().any(|line| line
                .ends_with("Program was deployed in this block already")
                || line.ends_with("Program was extended in this block already")),
            "InvalidArgument must be the native same-slot refusal"
        );
    }
    output.result
}

async fn read(context: &mut ProgramTestContext, key: Pubkey) -> Account {
    context
        .banks_client
        .get_account(key)
        .await
        .expect("account read")
        .expect("account exists")
}

fn observe(
    program_key: Pubkey,
    mut program: Account,
    data_key: Pubkey,
    mut data: Account,
    release: ArtifactReleaseV1,
) -> Result<(), ActivationAuthErrorV1> {
    let program_info = AccountInfo::new(
        &program_key,
        false,
        false,
        &mut program.lamports,
        &mut program.data,
        &program.owner,
        program.executable,
    );
    let data_info = AccountInfo::new(
        &data_key,
        false,
        false,
        &mut data.lamports,
        &mut data.data,
        &data.owner,
        data.executable,
    );
    cached_role_deployment_observation_v1(&program_info, &data_info, release).map(|_| ())
}

#[tokio::test]
async fn native_loader_blocks_same_slot_mutations_and_supersedes_after_extend_and_upgrade() {
    let elf = std::fs::read(
        std::env::var_os("DCLUTCH_LOADER_TEST_ELF")
            .expect("DCLUTCH_LOADER_TEST_ELF must name a real SBF ELF"),
    )
    .expect("read Loader control ELF");
    assert_eq!(elf.get(..4), Some(&b"\x7fELF"[..]));
    println!(
        "loader control ELF sha256={:?}, bytes={}",
        hash(&elf),
        elf.len()
    );
    let authority = Keypair::new();
    let program_key = Pubkey::new_unique();
    let data_key =
        Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID).0;
    let buffer = Pubkey::new_unique();
    let spill = Pubkey::new_unique();
    let mut program_data = bincode::serialize(&UpgradeableLoaderState::ProgramData {
        slot: PIN,
        upgrade_authority_address: Some(authority.pubkey()),
    })
    .expect("Loader ProgramData encoding");
    program_data.resize(UpgradeableLoaderState::size_of_programdata_metadata(), 0);
    program_data.extend_from_slice(&elf);
    let mut buffer_data = bincode::serialize(&UpgradeableLoaderState::Buffer {
        authority_address: Some(authority.pubkey()),
    })
    .expect("Loader Buffer encoding");
    buffer_data.resize(UpgradeableLoaderState::size_of_buffer_metadata(), 0);
    buffer_data.extend_from_slice(&elf);
    let program_bytes = bincode::serialize(&UpgradeableLoaderState::Program {
        programdata_address: data_key,
    })
    .expect("Loader Program encoding");
    let release = ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program_key.to_bytes()).expect("program"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
        data_key.to_bytes(),
        dclutch_core_contract::ContentId::new([1; 32]).expect("semantic id"),
        hash(&elf).to_bytes(),
        PIN,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(authority.pubkey().to_bytes()),
    )
    .expect("pinned release");
    let mut test = ProgramTest::default();
    // The SDK fixture is SBPFv0, as is the supported 3.1 local-validator cohort.
    // Disable only the future deployment-version ban; all Loader mutation/slot
    // rules remain the locked native runtime's implementation.
    test.deactivate_feature(agave_feature_set::disable_sbpf_v0_v1_v2_deployment::id());
    for (key, data, executable) in [
        (program_key, program_bytes, true),
        (data_key, program_data, false),
        (buffer, buffer_data, false),
    ] {
        test.add_account(
            key,
            Account {
                lamports: 10_000_000_000,
                data,
                owner: bpf_loader_upgradeable::ID,
                executable,
                rent_epoch: 0,
            },
        );
    }
    for key in [authority.pubkey(), spill] {
        test.add_account(
            key,
            Account {
                lamports: 10_000_000,
                ..Account::default()
            },
        );
    }
    let mut context = test.start_with_context().await;
    context.warp_to_slot(PIN).expect("initial deployment slot");
    let clock: Clock = context.banks_client.get_sysvar().await.expect("Clock");
    assert_eq!(clock.slot, PIN);
    let before = read(&mut context, data_key).await;
    let program = read(&mut context, program_key).await;
    assert_eq!(
        observe(
            program_key,
            program.clone(),
            data_key,
            before.clone(),
            release
        ),
        Ok(())
    );
    let upgrade = instruction::upgrade(&program_key, &buffer, &authority.pubkey(), &spill);
    let extend = instruction::extend_program(
        &program_key,
        None,
        instruction::MINIMUM_EXTEND_PROGRAM_BYTES,
    );
    let close = instruction::close_any(
        &data_key,
        &spill,
        Some(&authority.pubkey()),
        Some(&program_key),
    );
    for (name, ix) in [
        ("Upgrade", upgrade.clone()),
        ("Extend", extend.clone()),
        ("Close", close),
    ] {
        assert_eq!(
            send(&mut context, ix, &authority).await,
            Err(TransactionError::InstructionError(
                0,
                InstructionError::InvalidArgument
            )),
            "{name}"
        );
        assert_eq!(
            read(&mut context, data_key).await,
            before,
            "{name} changed ProgramData"
        );
    }
    context.warp_to_slot(PIN + 1).expect("later extension slot");
    send(&mut context, extend, &authority)
        .await
        .expect("actual Loader extension");
    let extended = read(&mut context, data_key).await;
    assert_eq!(
        extended.data.len(),
        before.data.len()
            + usize::try_from(instruction::MINIMUM_EXTEND_PROGRAM_BYTES).expect("extension width")
    );
    assert_eq!(
        ProgramDataV3View::parse(&extended.data)
            .expect("extended data")
            .deployment_slot(),
        PIN + 1
    );
    assert_eq!(
        observe(
            program_key,
            program.clone(),
            data_key,
            extended.clone(),
            release
        ),
        Err(ActivationAuthErrorV1::ReleaseSuperseded)
    );
    assert_eq!(
        send(&mut context, upgrade.clone(), &authority).await,
        Err(TransactionError::InstructionError(
            0,
            InstructionError::InvalidArgument
        ))
    );
    assert_eq!(read(&mut context, data_key).await, extended);
    context.warp_to_slot(PIN + 2).expect("later upgrade slot");
    send(&mut context, upgrade, &authority)
        .await
        .expect("actual Loader upgrade");
    let upgraded = read(&mut context, data_key).await;
    assert_eq!(
        ProgramDataV3View::parse(&upgraded.data)
            .expect("upgraded data")
            .deployment_slot(),
        PIN + 2
    );
    assert_eq!(
        observe(program_key, program, data_key, upgraded, release),
        Err(ActivationAuthErrorV1::ReleaseSuperseded)
    );
}
