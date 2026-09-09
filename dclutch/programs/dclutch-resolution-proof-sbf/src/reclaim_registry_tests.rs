//! The same lifecycle-owned Registry pin protects consumed and abandoned updates.
use super::*;
use alloc::{boxed::Box, format, vec};
use dclutch_core_contract::ContentId;
use dclutch_registry::release_set::ProgramIdentityV1;
use dclutch_registry::{
    ArtifactUpgradePolicyV1,
    record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1},
};
use solana_sdk_ids::bpf_loader_upgradeable;

const SLOT: u64 = 77;
const AUTHORITY: [u8; 32] = [71; 32];

fn info(
    key: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
    executable: bool,
    lamports: u64,
) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        false,
        false,
        Box::leak(Box::new(lamports)),
        Box::leak(data.into_boxed_slice()),
        Box::leak(Box::new(owner)),
        executable,
    )
}

#[derive(Clone, Copy, Debug)]
enum Case {
    Accepted,
    LaterSlot,
    EarlierSlot,
    Authority,
    Artifact,
    Staging,
    ProgramOwner,
    DataOwner,
    ProgramLink,
}

fn fixture(case: Case, consumed: bool) -> (Vec<AccountInfo<'static>>, ProviderUpdateLifecycleV4) {
    let registry = Pubkey::new_from_array([61; 32]);
    let programdata =
        Pubkey::find_program_address(&[registry.as_ref()], &bpf_loader_upgradeable::ID).0;
    let release = ArtifactReleaseV2::new(
        ProgramIdentityV1::new(registry.to_bytes()).unwrap(),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).unwrap(),
        programdata.to_bytes(),
        ContentId::new([62; 32]).unwrap(),
        dclutch_registry::artifact_code_commitment_v2::code_commitment_v2(&[7; 96]).unwrap(),
        SLOT,
        ArtifactUpgradePolicyV1::ExactAuthority,
        Some(AUTHORITY),
    )
    .unwrap();
    let digest = hash(&release.to_bytes()).to_bytes();
    let request = ProviderSubmitRequestV3 {
        generation: 1,
        reclaim_after_unix_seconds: 100,
        market: [1; 32],
        source_state: [2; 32],
        lifecycle: [3; 32],
        source_material: [4; 32],
        provider_release: [5; 32],
        update_account: [6; 32],
        provider_submitter: [7; 32],
        refund_recipient: [8; 32],
        release_set: [9; 32],
        registry_program: registry.to_bytes(),
        encoded_vaa: [10; 32],
        post_body_digest: [11; 32],
    };
    let mut lifecycle = ProviderUpdateLifecycleV4::submitted(
        request,
        1,
        [12; 32],
        registry.to_bytes(),
        digest,
        [13; 32],
        99,
        2,
        1000,
        1,
    )
    .unwrap();
    if consumed {
        lifecycle.consume(1, [14; 32], [15; 32]).unwrap();
    }
    let mut program = vec![0; 36];
    program[..4].copy_from_slice(&2u32.to_le_bytes());
    program[4..].copy_from_slice(programdata.as_ref());
    let mut data = vec![7; 45 + 96];
    data[..4].copy_from_slice(&3u32.to_le_bytes());
    data[4..12].copy_from_slice(&SLOT.to_le_bytes());
    data[12] = 1;
    data[13..45].copy_from_slice(&AUTHORITY);
    let mut frame: Vec<_> = (0..PROVIDER_RECLAIM_ACCOUNT_COUNT_V3)
        .map(|_| info(Pubkey::new_unique(), system_program::ID, vec![], false, 0))
        .collect();
    frame[7] = info(registry, bpf_loader_upgradeable::ID, program, true, 1);
    frame[8] = info(programdata, bpf_loader_upgradeable::ID, data, false, 1);
    frame[18] = info(
        Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &ARTIFACT_RELEASE_SCHEMA_ID_V2,
                &digest,
            ],
            &registry,
        )
        .0,
        registry,
        release.to_bytes().to_vec(),
        false,
        1,
    );
    frame[19] = info(
        Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &ARTIFACT_RELEASE_SCHEMA_ID_V2,
                &digest,
            ],
            &registry,
        )
        .0,
        system_program::ID,
        vec![],
        false,
        0,
    );
    match case {
        Case::Accepted => {}
        Case::LaterSlot => frame[8].try_borrow_mut_data().unwrap()[4..12]
            .copy_from_slice(&(SLOT + 1).to_le_bytes()),
        Case::EarlierSlot => frame[8].try_borrow_mut_data().unwrap()[4..12]
            .copy_from_slice(&(SLOT - 1).to_le_bytes()),
        Case::Authority => frame[8].try_borrow_mut_data().unwrap()[13..45].fill(72),
        Case::Artifact => frame[18].try_borrow_mut_data().unwrap()[144] ^= 1,
        Case::Staging => frame[19].key = Box::leak(Box::new(Pubkey::new_unique())),
        Case::ProgramOwner => frame[7].owner = Box::leak(Box::new(system_program::ID)),
        Case::DataOwner => frame[8].owner = Box::leak(Box::new(system_program::ID)),
        Case::ProgramLink => frame[7].try_borrow_mut_data().unwrap()[4..36].fill(73),
    }
    (frame, lifecycle)
}

#[test]
fn reclaim_and_abandon_authenticate_the_submitted_registry_artifact() {
    let cases = [
        Case::Accepted,
        Case::LaterSlot,
        Case::EarlierSlot,
        Case::Authority,
        Case::Artifact,
        Case::Staging,
        Case::ProgramOwner,
        Case::DataOwner,
        Case::ProgramLink,
    ];
    let mut actual = Vec::new();
    let mut expected = Vec::new();
    for consumed in [false, true] {
        for case in cases {
            let (accounts, lifecycle) = fixture(case, consumed);
            let frame = ReclaimFrameV3 {
                accounts: &accounts,
            };
            let result = authenticate_reclaim_release(
                &Pubkey::new_unique(),
                lifecycle.release_set,
                frame,
                lifecycle,
            );
            // Empty activation is a downstream sentinel: a correct mutable
            // Registry pin must reach it, and a changed pin must never reach it.
            let refusal = match case {
                Case::Accepted => ResolutionError::ActivationCache,
                Case::LaterSlot => ResolutionError::ReleaseSuperseded,
                Case::Artifact | Case::Staging => ResolutionError::FinalizedRecord,
                _ => ResolutionError::ResolutionDeployment,
            };
            actual.push((consumed, format!("{case:?}"), result));
            expected.push((consumed, format!("{case:?}"), Err(refusal.into())));
        }
    }
    assert_eq!(actual, expected);
}

#[test]
fn submitted_registry_pin_accepts_current_mutable_deployment() {
    let (accounts, lifecycle) = fixture(Case::Accepted, false);
    assert_eq!(
        authenticate_reclaim_registry(
            ReclaimFrameV3 {
                accounts: &accounts
            },
            lifecycle
        ),
        Ok(())
    );
}
