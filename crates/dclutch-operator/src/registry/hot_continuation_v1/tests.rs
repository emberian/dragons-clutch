use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, ExecutionReleaseActivationInputsV1, activate_execution_release_set_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

use super::*;
use crate::{Finality, ObservedAccount};

fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn observation() -> Observation {
    Observation {
        slot: 91,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

fn observed(key: Pubkey, owner: Pubkey, executable: bool, data: Vec<u8>) -> ObservedAccount {
    ObservedAccount {
        observation: observation(),
        key,
        owner,
        lamports: 1,
        executable,
        data,
    }
}

fn loader_program_bytes(programdata: Pubkey) -> Vec<u8> {
    let mut output = vec![0; 36];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&2_u32.to_le_bytes());
    output
        .get_mut(4..)
        .expect("ProgramData")
        .copy_from_slice(programdata.as_ref());
    output
}

fn immutable_programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
    let mut output = vec![0; 45 + elf.len()];
    output
        .get_mut(..4)
        .expect("variant")
        .copy_from_slice(&3_u32.to_le_bytes());
    output
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&slot.to_le_bytes());
    output.get_mut(45..).expect("ELF").copy_from_slice(elf);
    output
}

#[derive(Clone)]
struct RoleFixture {
    release: ArtifactReleaseV1,
    artifact: ArtifactReleaseIdV1,
    program: ObservedAccount,
    programdata: ObservedAccount,
}

fn role(seed: u8, slot: u64) -> RoleFixture {
    let program = Pubkey::new_from_array(bytes(seed));
    let programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    let elf = vec![seed; 128];
    let release = ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("program"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
        programdata.to_bytes(),
        ContentId::new(bytes(seed + 20)).expect("semantic release"),
        hash(&elf).to_bytes(),
        slot,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release");
    RoleFixture {
        release,
        artifact: ArtifactReleaseIdV1::new(bytes(seed + 40)).expect("artifact ID"),
        program: observed(
            program,
            bpf_loader_upgradeable::ID,
            true,
            loader_program_bytes(programdata),
        ),
        programdata: observed(
            programdata,
            bpf_loader_upgradeable::ID,
            false,
            immutable_programdata_bytes(slot, &elf),
        ),
    }
}

struct Fixture {
    state: RegistryHotContinuationStateV1,
    release_set: ContentId,
    hot: Instruction,
}

impl Fixture {
    fn new() -> Self {
        let registry = Pubkey::new_from_array(bytes(7));
        let registry_programdata =
            Pubkey::find_program_address(&[registry.as_ref()], &bpf_loader_upgradeable::ID).0;
        let core = role(11, 70);
        let trading = role(12, 71);
        let core_binding = ExecutionRoleBindingV1::new(core.release.program(), core.artifact);
        let trading_binding =
            ExecutionRoleBindingV1::new(trading.release.program(), trading.artifact);
        let release_set = ExecutionReleaseSetV1::new(
            core_binding,
            core_binding,
            trading_binding,
            core_binding,
            core_binding,
        )
        .expect("release set");
        let release_set_id =
            ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set ID");
        let core_input = ArtifactActivationInputV1::new(
            core.artifact,
            core.release,
            crate::registry::deployment_observation(&core.program, &core.programdata, core.release)
                .expect("Core deployment"),
        );
        let trading_input = ArtifactActivationInputV1::new(
            trading.artifact,
            trading.release,
            crate::registry::deployment_observation(
                &trading.program,
                &trading.programdata,
                trading.release,
            )
            .expect("Trading deployment"),
        );
        let activated = activate_execution_release_set_v1(
            release_set_id,
            &release_set,
            &ExecutionReleaseActivationInputsV1::new(
                core_input,
                core_input,
                trading_input,
                core_input,
                core_input,
            ),
        )
        .expect("activated release set");
        let cache = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            &registry,
        )
        .0;
        let trading_program_key = trading.program.key;
        let state = RegistryHotContinuationStateV1 {
            registry_program: observed(
                registry,
                bpf_loader_upgradeable::ID,
                true,
                loader_program_bytes(registry_programdata),
            ),
            activation_cache: observed(cache, registry, false, activated.to_bytes().to_vec()),
            core_program: core.program,
            core_programdata: core.programdata,
            trading_program: trading.program,
            trading_programdata: trading.programdata,
        };
        let mut accounts = (0..HOT_FIXED_ACCOUNT_COUNT_V3)
            .map(|index| {
                AccountMeta::new_readonly(Pubkey::new_from_array(bytes(100 + index as u8)), false)
            })
            .collect::<Vec<_>>();
        for (index, key) in [
            (HOT_ACTIVATION_CACHE_ACCOUNT_V3, state.activation_cache.key),
            (HOT_CORE_PROGRAM_ACCOUNT_V3, state.core_program.key),
            (HOT_CORE_PROGRAMDATA_ACCOUNT_V3, state.core_programdata.key),
            (HOT_TRADING_PROGRAM_ACCOUNT_V3, state.trading_program.key),
            (
                HOT_TRADING_PROGRAMDATA_ACCOUNT_V3,
                state.trading_programdata.key,
            ),
            (HOT_REGISTRY_PROGRAM_ACCOUNT_V3, state.registry_program.key),
        ] {
            *accounts.get_mut(index).expect("fixed Hot meta") =
                AccountMeta::new_readonly(key, false);
        }
        let envelope =
            HotExecutionEnvelopeV3::new(3, release_set_id.to_bytes(), bytes(81), 9, bytes(82))
                .expect("Hot envelope");
        let mut data = Vec::from(envelope.to_bytes());
        data.extend_from_slice(b"hot");
        Self {
            state,
            release_set: release_set_id,
            hot: Instruction {
                program_id: trading_program_key,
                accounts,
                data,
            },
        }
    }
}

#[test]
fn builder_wraps_exact_hot_bytes_and_fixed_admission_boundary() {
    let fixture = Fixture::new();
    let report = build_registry_hot_continuation_v1(&fixture.state, &fixture.hot)
        .expect("Registry Hot continuation");
    assert_eq!(
        report.instruction.program_id,
        fixture.state.registry_program.key
    );
    // 44 wrapper+frame accounts plus the validated-artifact seal at fixed hot
    // coordinate 38 (Decision 0005).
    assert_eq!(report.instruction.accounts.len(), 46);
    assert_eq!(
        report.instruction.data.len(),
        REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + fixture.hot.data.len()
    );
    assert_eq!(
        report
            .instruction
            .accounts
            .get(5)
            .expect("prefix admission")
            .pubkey,
        report.admission
    );
    assert_eq!(
        report
            .instruction
            .accounts
            .get(REGISTRY_HOT_CONTINUATION_PREFIX_ACCOUNTS_V1 + HOT_FIXED_ACCOUNT_COUNT_V3)
            .expect("nested admission")
            .pubkey,
        report.admission
    );
    assert_eq!(
        report.continuation.verify_core_trading_hot(
            fixture.release_set,
            report.activation_cache_digest,
            report.hot_instruction_digest,
            u32::try_from(fixture.hot.data.len()).expect("Hot width"),
        ),
        Ok(())
    );
}

#[test]
fn release_meta_digest_and_admission_substitutions_refuse_or_rederive() {
    let fixture = Fixture::new();
    let mut wrong_release = fixture.hot.clone();
    let envelope = HotExecutionEnvelopeV3::new(3, bytes(99), bytes(81), 9, bytes(82))
        .expect("hostile envelope");
    wrong_release.data.clear();
    wrong_release.data.extend_from_slice(&envelope.to_bytes());
    wrong_release.data.extend_from_slice(b"hot");
    assert_eq!(
        build_registry_hot_continuation_v1(&fixture.state, &wrong_release),
        Err(RegistryHotContinuationErrorV1::InvalidHotInstruction)
    );

    let mut reordered = fixture.hot.clone();
    reordered
        .accounts
        .swap(HOT_CORE_PROGRAM_ACCOUNT_V3, HOT_TRADING_PROGRAM_ACCOUNT_V3);
    assert_eq!(
        build_registry_hot_continuation_v1(&fixture.state, &reordered),
        Err(RegistryHotContinuationErrorV1::InvalidHotInstruction)
    );

    let report = build_registry_hot_continuation_v1(&fixture.state, &fixture.hot)
        .expect("baseline continuation");
    let mut aliased = fixture.hot.clone();
    aliased.accounts.get_mut(0).expect("first meta").pubkey = report.admission;
    assert_eq!(
        build_registry_hot_continuation_v1(&fixture.state, &aliased),
        Err(RegistryHotContinuationErrorV1::Admission)
    );

    let mut changed = fixture.hot.clone();
    *changed.data.last_mut().expect("family request") ^= 1;
    let changed = build_registry_hot_continuation_v1(&fixture.state, &changed)
        .expect("new exact continuation");
    assert_ne!(
        changed.hot_instruction_digest,
        report.hot_instruction_digest
    );
    assert_ne!(changed.admission, report.admission);

    let mut stale = fixture.state.clone();
    *stale
        .trading_programdata
        .data
        .last_mut()
        .expect("Trading ELF") ^= 1;
    assert_eq!(
        build_registry_hot_continuation_v1(&stale, &fixture.hot),
        Err(RegistryHotContinuationErrorV1::Registry(
            RegistryError::InvalidDeployment
        ))
    );
}

#[test]
fn nonfinal_snapshot_and_mutable_fixed_release_meta_refuse() {
    let fixture = Fixture::new();
    let mut nonfinal = fixture.state.clone();
    nonfinal.trading_program.observation.finality = Finality::Confirmed;
    assert_eq!(
        build_registry_hot_continuation_v1(&nonfinal, &fixture.hot),
        Err(RegistryHotContinuationErrorV1::Registry(
            RegistryError::ObservationNotFinalized
        ))
    );

    let mut writable = fixture.hot;
    writable
        .accounts
        .get_mut(HOT_ACTIVATION_CACHE_ACCOUNT_V3)
        .expect("cache meta")
        .is_writable = true;
    assert_eq!(
        build_registry_hot_continuation_v1(&fixture.state, &writable),
        Err(RegistryHotContinuationErrorV1::InvalidHotInstruction)
    );

    assert_ne!(fixture.state.registry_program.owner, system_program::ID);
}
