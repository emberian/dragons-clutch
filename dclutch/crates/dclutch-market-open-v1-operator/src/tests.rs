use dclutch_custody_contract::{CompartmentV1, ContextV1};
use dclutch_market_core_codec::Identity;
use dclutch_registry_contract::{
    ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1, ArtifactReleaseV1,
    ArtifactUpgradePolicyV1, ExecutionReleaseActivationInputsV1, activate_execution_release_set_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};

use super::*;
use crate::{Finality, ObservedAccount};

const GENERATION: u64 = 4;

fn bytes(seed: u8) -> [u8; 32] {
    [seed; 32]
}

fn observation() -> Observation {
    Observation {
        slot: 93,
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
    state: RegistryOpenMarketContinuationStateV1,
    release_set: ContentId,
    instruction: Instruction,
}

impl Fixture {
    fn new(operation: OperationV1) -> Self {
        let registry = Pubkey::new_from_array(bytes(7));
        let registry_programdata =
            Pubkey::find_program_address(&[registry.as_ref()], &bpf_loader_upgradeable::ID).0;
        let core = role(11, 70);
        let custody = role(12, 71);
        let core_binding = ExecutionRoleBindingV1::new(core.release.program(), core.artifact);
        let custody_binding =
            ExecutionRoleBindingV1::new(custody.release.program(), custody.artifact);
        let release_set = ExecutionReleaseSetV1::new(
            core_binding,
            core_binding,
            core_binding,
            core_binding,
            custody_binding,
        )
        .expect("release set");
        let release_set_id =
            ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release set ID");
        let core_input = ArtifactActivationInputV1::new(
            core.artifact,
            core.release,
            super::deployment_observation(&core.program, &core.programdata)
                .expect("Core deployment"),
        );
        let custody_input = ArtifactActivationInputV1::new(
            custody.artifact,
            custody.release,
            super::deployment_observation(&custody.program, &custody.programdata)
                .expect("Custody deployment"),
        );
        let activated = activate_execution_release_set_v1(
            release_set_id,
            &release_set,
            &ExecutionReleaseActivationInputsV1::new(
                core_input,
                core_input,
                core_input,
                core_input,
                custody_input,
            ),
        )
        .expect("activated release set");
        let cache = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            &registry,
        )
        .0;
        let state = RegistryOpenMarketContinuationStateV1 {
            registry_program: observed(
                registry,
                bpf_loader_upgradeable::ID,
                true,
                loader_program_bytes(registry_programdata),
            ),
            activation_cache: observed(cache, registry, false, activated.to_bytes().to_vec()),
            core_program: core.program,
            core_programdata: core.programdata,
            custody_program: custody.program,
            custody_programdata: custody.programdata,
        };
        let market = Pubkey::new_from_array(bytes(81));
        let realm = bytes(82);
        let payer = Pubkey::new_from_array(bytes(83));
        let rent_refund = bytes(84);
        let core_request = Request::administrative(
            Action::OpenMarket,
            GENERATION,
            Identity::new(market.to_bytes()).expect("Market"),
        );
        let core_bytes = core_request.encode().expect("Core request");
        let mut custody_request = CustodyRequestV1 {
            operation,
            caller_role: CallerRoleV1::Core,
            source_compartment: CompartmentV1::None,
            destination_compartment: if operation == OperationV1::OpenVault {
                CompartmentV1::HoardPrincipal
            } else {
                CompartmentV1::None
            },
            release_set: release_set_id.to_bytes(),
            market: market.to_bytes(),
            realm,
            context: market.to_bytes(),
            caller_program: state.core_program.key.to_bytes(),
            semantic: ContextV1 {
                candidate: [0; 32],
                source_owner: [0; 32],
                destination_owner: [0; 32],
                order: [0; 32],
                parent_request_digest: hash(&core_bytes).to_bytes(),
                order_nonce: 0,
                generation: GENERATION,
                page_index: 0,
                execution_index: 0,
                transfer_index: 0,
            },
            source: [0; 32],
            destination: [0; 32],
            source_vault_context: [0; 32],
            destination_vault_context: if operation == OperationV1::OpenVault {
                market.to_bytes()
            } else {
                [0; 32]
            },
            mint: if operation == OperationV1::OpenVault {
                bytes(85)
            } else {
                [0; 32]
            },
            token_program: if operation == OperationV1::OpenVault {
                bytes(86)
            } else {
                [0; 32]
            },
            payer: payer.to_bytes(),
            rent_refund,
            expected_revision: u64::from(operation == OperationV1::OpenVault),
            resulting_revision: if operation == OperationV1::OpenVault {
                2
            } else {
                1
            },
            amount: 0,
            rent_lamports: 1_000,
        };
        if operation == OperationV1::OpenVault {
            custody_request.destination = Pubkey::find_program_address(
                &CustodyVaultSeedsV1::from_request(custody_request, false).as_slices(),
                &state.custody_program.key,
            )
            .0
            .to_bytes();
        }
        let custody_bytes = custody_request.to_bytes().expect("Custody request");
        let authority = Pubkey::find_program_address(
            &CallerAuthoritySeedsV1::new(
                release_set_id,
                market.to_bytes(),
                ExecutionRoleV1::Core,
                market.to_bytes(),
                hash(&custody_bytes).to_bytes(),
            )
            .expect("Core authority seeds")
            .as_slices(),
            &state.core_program.key,
        )
        .0;
        let realm_raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &REALM_SCHEMA_RELEASE_ID_V1, &realm],
            &registry,
        )
        .0;
        let realm_staging = Pubkey::find_program_address(
            &[
                STAGING_CURSOR_PDA_SEED_V1,
                &REALM_SCHEMA_RELEASE_ID_V1,
                &realm,
            ],
            &registry,
        )
        .0;
        let replay = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::from_request(custody_request).as_slices(),
            &state.custody_program.key,
        )
        .0;
        let mut accounts = vec![
            AccountMeta::new_readonly(authority, false),
            AccountMeta::new(market, false),
            AccountMeta::new_readonly(cache, false),
            AccountMeta::new_readonly(registry, false),
            AccountMeta::new_readonly(state.core_program.key, false),
            AccountMeta::new_readonly(state.core_programdata.key, false),
            AccountMeta::new_readonly(state.custody_program.key, false),
            AccountMeta::new_readonly(state.custody_programdata.key, false),
            AccountMeta::new_readonly(realm_raw, false),
            AccountMeta::new_readonly(realm_staging, false),
            AccountMeta::new(replay, false),
        ];
        if operation == OperationV1::InitializeReplay {
            accounts.extend([
                AccountMeta::new(payer, true),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
            ]);
        } else {
            let vault = Pubkey::new_from_array(custody_request.destination);
            let custody_authority = Pubkey::find_program_address(
                &CustodyAuthoritySeedsV1::from_request(custody_request).as_slices(),
                &state.custody_program.key,
            )
            .0;
            accounts.extend([
                AccountMeta::new_readonly(Pubkey::new_from_array(custody_request.mint), false),
                AccountMeta::new(vault, false),
                AccountMeta::new_readonly(custody_authority, false),
                AccountMeta::new_readonly(
                    Pubkey::new_from_array(custody_request.token_program),
                    false,
                ),
                AccountMeta::new(payer, true),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
            ]);
        }
        let mut data = Vec::from(core_bytes);
        data.extend_from_slice(&custody_bytes);
        let core_program = state.core_program.key;
        Self {
            state,
            release_set: release_set_id,
            instruction: Instruction {
                program_id: core_program,
                accounts,
                data,
            },
        }
    }
}

#[test]
fn builder_wraps_both_exact_open_operations() {
    for operation in [OperationV1::InitializeReplay, OperationV1::OpenVault] {
        let fixture = Fixture::new(operation);
        let report =
            build_registry_open_market_continuation_v1(&fixture.state, &fixture.instruction)
                .expect("Registry open continuation");
        assert_eq!(report.operation, operation);
        assert_eq!(report.release_set_id, fixture.release_set);
        assert_eq!(
            report.instruction.program_id,
            fixture.state.registry_program.key
        );
        assert_eq!(
            report.instruction.data.len(),
            REGISTRY_CONTINUATION_REQUEST_BYTES_V1 + fixture.instruction.data.len()
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
                .last()
                .expect("nested admission")
                .pubkey,
            report.admission
        );
        assert_eq!(report.continuation.role_count(), 2);
        assert_eq!(report.continuation.role(0), Some(ExecutionRoleV1::Core));
        assert_eq!(report.continuation.role(1), Some(ExecutionRoleV1::Custody));
    }
}

#[test]
fn release_order_privilege_and_alias_substitutions_refuse() {
    let fixture = Fixture::new(OperationV1::OpenVault);

    let mut reordered = fixture.instruction.clone();
    reordered.accounts.swap(CORE_PROGRAM, CUSTODY_PROGRAM);
    assert_eq!(
        build_registry_open_market_continuation_v1(&fixture.state, &reordered),
        Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)
    );

    let mut privilege = fixture.instruction.clone();
    privilege.accounts.get_mut(15).expect("payer").is_signer = false;
    assert_eq!(
        build_registry_open_market_continuation_v1(&fixture.state, &privilege),
        Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)
    );

    let mut alias = fixture.instruction.clone();
    let system = alias.accounts.get(16).expect("System program").pubkey;
    alias.accounts.get_mut(17).expect("Rent sysvar").pubkey = system;
    assert_eq!(
        build_registry_open_market_continuation_v1(&fixture.state, &alias),
        Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)
    );

    let mut hostile_release = fixture.instruction.clone();
    *hostile_release
        .data
        .get_mut(REQUEST_BYTES + 16)
        .expect("release byte") ^= 1;
    assert_eq!(
        build_registry_open_market_continuation_v1(&fixture.state, &hostile_release),
        Err(RegistryOpenMarketContinuationErrorV1::InvalidCoreInstruction)
    );
}

#[test]
fn nonfinal_or_changed_current_deployment_refuses() {
    let fixture = Fixture::new(OperationV1::InitializeReplay);
    let mut nonfinal = fixture.state.clone();
    nonfinal.custody_program.observation.finality = Finality::Confirmed;
    assert_eq!(
        build_registry_open_market_continuation_v1(&nonfinal, &fixture.instruction),
        Err(RegistryOpenMarketContinuationErrorV1::Registry(
            RegistryOpenMarketObservationErrorV1::ObservationNotFinalized
        ))
    );

    let mut changed = fixture.state.clone();
    *changed
        .custody_programdata
        .data
        .last_mut()
        .expect("Custody ELF") ^= 1;
    assert_eq!(
        build_registry_open_market_continuation_v1(&changed, &fixture.instruction),
        Err(RegistryOpenMarketContinuationErrorV1::Registry(
            RegistryOpenMarketObservationErrorV1::InvalidDeployment
        ))
    );
}
