//! Native admission regressions: a caller PDA survives a program upgrade.
use super::*;
use alloc::{boxed::Box, vec};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ArtifactActivationInputV1, ArtifactReleaseV2,
    ArtifactUpgradePolicyV1, DeploymentObservationV2, activate_execution_role_into_v1,
    initialize_activation_cache_v1,
};
use solana_sdk_ids::bpf_loader_upgradeable;

const PIN: u64 = 77;
const AUTHORITY: [u8; 32] = [0x71; 32];

pub(super) fn info(
    key: Pubkey,
    owner: Pubkey,
    data: Vec<u8>,
    executable: bool,
) -> AccountInfo<'static> {
    AccountInfo::new(
        Box::leak(Box::new(key)),
        false,
        false,
        Box::leak(Box::new(1)),
        Box::leak(data.into_boxed_slice()),
        Box::leak(Box::new(owner)),
        executable,
    )
}

pub(super) struct Fixture {
    pub(super) cache: AccountInfo<'static>,
    pub(super) registry: AccountInfo<'static>,
    pub(super) program: AccountInfo<'static>,
    pub(super) programdata: AccountInfo<'static>,
    pub(super) release_set: [u8; 32],
}

impl Fixture {
    pub(super) fn new() -> Self {
        let program_key = Pubkey::new_from_array([0x61; 32]);
        let data_key =
            Pubkey::find_program_address(&[program_key.as_ref()], &bpf_loader_upgradeable::ID).0;
        let registry_key = Pubkey::new_from_array([0x62; 32]);
        let commitment =
            dclutch_registry::artifact_code_commitment_v2::code_commitment_v2(&[7; 96])
                .expect("valid continuity fixture");
        let release = ArtifactReleaseV2::new(
            ProgramIdentityV1::new(program_key.to_bytes()).expect("valid continuity fixture"),
            ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes())
                .expect("valid continuity fixture"),
            data_key.to_bytes(),
            ContentId::new([0x63; 32]).expect("valid continuity fixture"),
            commitment,
            PIN,
            ArtifactUpgradePolicyV1::ExactAuthority,
            Some(AUTHORITY),
        )
        .expect("valid continuity fixture");
        let artifact = ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes())
            .expect("valid continuity fixture");
        let binding = ExecutionRoleBindingV1::new(release.program(), artifact);
        let set = ExecutionReleaseSetV1::new(binding, binding, binding, binding, binding)
            .expect("valid continuity fixture");
        let release_set = hash(&set.to_bytes()).to_bytes();
        let content = ContentId::new(release_set).expect("valid continuity fixture");
        let observation = DeploymentObservationV2::new(
            program_key.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            data_key.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            data_key.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            PIN,
            commitment,
            Some(AUTHORITY),
        )
        .expect("valid continuity fixture");
        let input = ArtifactActivationInputV1::new(artifact, release, observation);
        let mut cache_data = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        initialize_activation_cache_v1(&mut cache_data, content).expect("valid continuity fixture");
        for role in [
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Trading,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ] {
            activate_execution_role_into_v1(&mut cache_data, content, &set, role, &input)
                .expect("valid continuity fixture");
        }
        let cache_key =
            Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &registry_key)
                .0;
        let mut program_bytes = vec![0; 36];
        program_bytes
            .get_mut(..4)
            .expect("Loader field")
            .copy_from_slice(&2_u32.to_le_bytes());
        program_bytes
            .get_mut(4..)
            .expect("Loader field")
            .copy_from_slice(data_key.as_ref());
        let mut data_bytes = vec![0; 45 + 96];
        data_bytes
            .get_mut(..4)
            .expect("Loader field")
            .copy_from_slice(&3_u32.to_le_bytes());
        data_bytes
            .get_mut(4..12)
            .expect("Loader field")
            .copy_from_slice(&PIN.to_le_bytes());
        *data_bytes.get_mut(12).expect("authority option") = 1;
        data_bytes
            .get_mut(13..45)
            .expect("Loader field")
            .copy_from_slice(&AUTHORITY);
        data_bytes.get_mut(45..).expect("Loader field").fill(7);
        Self {
            cache: info(cache_key, registry_key, cache_data, false),
            registry: info(
                registry_key,
                solana_sdk_ids::native_loader::ID,
                vec![],
                true,
            ),
            program: info(program_key, bpf_loader_upgradeable::ID, program_bytes, true),
            programdata: info(data_key, bpf_loader_upgradeable::ID, data_bytes, false),
            release_set,
        }
    }

    pub(super) fn mutate(&mut self, case: Case) {
        match case {
            Case::Accepted => {}
            Case::Slot => self
                .programdata
                .try_borrow_mut_data()
                .expect("valid continuity fixture")
                .get_mut(4..12)
                .expect("Loader field")
                .copy_from_slice(&(PIN + 1).to_le_bytes()),
            Case::Authority => self
                .programdata
                .try_borrow_mut_data()
                .expect("valid continuity fixture")
                .get_mut(13..45)
                .expect("Loader field")
                .fill(0x72),
            Case::ProgramOwner => self.program.owner = Box::leak(Box::new(system_program::ID)),
            Case::ProgramDataOwner => {
                self.programdata.owner = Box::leak(Box::new(system_program::ID))
            }
            Case::ProgramDataLink => self
                .program
                .try_borrow_mut_data()
                .expect("valid continuity fixture")
                .get_mut(4..36)
                .expect("Loader field")
                .fill(0x73),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    Accepted,
    Slot,
    Authority,
    ProgramOwner,
    ProgramDataOwner,
    ProgramDataLink,
}
pub(super) const CASES: [Case; 6] = [
    Case::Accepted,
    Case::Slot,
    Case::Authority,
    Case::ProgramOwner,
    Case::ProgramDataOwner,
    Case::ProgramDataLink,
];

fn request(fixture: &Fixture) -> CustodyRequestV1 {
    CustodyRequestV1 {
        operation: OperationV1::InitializeReplay,
        caller_role: CallerRoleV1::Trading,
        source_compartment: CompartmentV1::None,
        destination_compartment: CompartmentV1::None,
        release_set: fixture.release_set,
        market: [0x12; 32],
        realm: [0x13; 32],
        context: [0x14; 32],
        caller_program: fixture.program.key.to_bytes(),
        semantic: dclutch_custody::ContextV1 {
            candidate: [0x16; 32],
            source_owner: [0; 32],
            destination_owner: [0; 32],
            order: [0x14; 32],
            parent_request_digest: [0x17; 32],
            order_nonce: 9,
            generation: 3,
            page_index: 0,
            execution_index: 9,
            transfer_index: 0,
        },
        source: [0; 32],
        destination: [0; 32],
        source_vault_context: [0; 32],
        destination_vault_context: [0; 32],
        mint: [0; 32],
        token_program: [0; 32],
        payer: [0x18; 32],
        rent_refund: [0x19; 32],
        expected_revision: 0,
        resulting_revision: 1,
        amount: 0,
        rent_lamports: 1,
    }
}

#[test]
fn ordinary_calling_release_requires_live_loader_continuity() {
    let mut outcomes = Vec::new();
    let mut expected_outcomes = Vec::new();
    for case in CASES {
        let mut fixture = Fixture::new();
        fixture.mutate(case);
        let request = request(&fixture);
        request.validate().expect("valid continuity fixture");
        let bytes = fixture
            .cache
            .try_borrow_data()
            .expect("valid continuity fixture");
        let activated =
            ActivatedExecutionReleaseSetViewV1::decode(&bytes).expect("valid continuity fixture");
        authenticate_activation_cache_identity_v1(
            &fixture.registry,
            &fixture.cache,
            &fixture.release_set,
            activated,
        )
        .expect("valid continuity fixture");
        let dummy = info(Pubkey::new_unique(), system_program::ID, vec![], false);
        let accounts = [
            dummy.clone(),
            dummy,
            fixture.cache.clone(),
            fixture.registry.clone(),
            fixture.program.clone(),
            fixture.programdata.clone(),
        ];
        let expected = match case {
            Case::Accepted => Ok(()),
            Case::Slot => Err(CustodySbfError::ReleaseSuperseded.into()),
            _ => Err(CustodySbfError::Release.into()),
        };
        outcomes.push(authenticate_calling_release(
            fixture.program.key,
            &accounts,
            request,
            None,
            activated,
        ));
        expected_outcomes.push(expected);
    }
    assert_eq!(outcomes, expected_outcomes, "case order: {CASES:?}");
}
