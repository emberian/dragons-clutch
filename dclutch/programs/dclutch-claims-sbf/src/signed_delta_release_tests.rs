//! The caller PDA survives replacement of its current Loader deployment.
use super::*;
use alloc::{boxed::Box, vec};
use dclutch_claims::signed_delta_v3::{
    PositionDeltaInputV3, PositionDeltaV3, SignedDeltaPlanInputV3, SignedDeltaPositionV3,
    plan_bytes,
};
use dclutch_registry::release_set::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
};
use dclutch_registry::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV2, ArtifactUpgradePolicyV1, DeploymentObservationV2,
    activate_execution_role_into_v1, initialize_activation_cache_v1,
};
use solana_sdk_ids::{bpf_loader_upgradeable, system_program};

const PIN: u64 = 77;
const AUTHORITY: [u8; 32] = [0x71; 32];

fn info(key: Pubkey, owner: Pubkey, data: Vec<u8>, executable: bool) -> AccountInfo<'static> {
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

struct Fixture {
    registry: AccountInfo<'static>,
    cache: AccountInfo<'static>,
    programs: Vec<AccountInfo<'static>>,
    programdata: Vec<AccountInfo<'static>>,
    release_set: [u8; 32],
}

impl Fixture {
    fn new() -> Self {
        let registry_key = Pubkey::new_from_array([0x55; 32]);
        let commitment =
            dclutch_registry::artifact_code_commitment_v2::code_commitment_v2(&[7; 96])
                .expect("code commitment");
        let mut programs = Vec::new();
        let mut programdata = Vec::new();
        let mut inputs = Vec::new();
        let mut bindings = Vec::new();
        for seed in [0x60, 0x61, 0x62, 0x63, 0x64] {
            let key = Pubkey::new_from_array([seed; 32]);
            let data_key =
                Pubkey::find_program_address(&[key.as_ref()], &bpf_loader_upgradeable::ID).0;
            let release = ArtifactReleaseV2::new(
                ProgramIdentityV1::new(key.to_bytes()).expect("program"),
                ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
                data_key.to_bytes(),
                ContentId::new([seed; 32]).expect("semantic"),
                commitment,
                PIN,
                ArtifactUpgradePolicyV1::ExactAuthority,
                Some(AUTHORITY),
            )
            .expect("release");
            let artifact =
                ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact");
            bindings.push(ExecutionRoleBindingV1::new(release.program(), artifact));
            let observation = DeploymentObservationV2::new(
                key.to_bytes(),
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
            .expect("observation");
            inputs.push(ArtifactActivationInputV1::new(
                artifact,
                release,
                observation,
            ));
            let mut program = vec![0; 36];
            program
                .get_mut(..4)
                .expect("variant")
                .copy_from_slice(&2_u32.to_le_bytes());
            program
                .get_mut(4..36)
                .expect("link")
                .copy_from_slice(data_key.as_ref());
            let mut data = vec![0; 45 + 96];
            data.get_mut(..4)
                .expect("variant")
                .copy_from_slice(&3_u32.to_le_bytes());
            data.get_mut(4..12)
                .expect("slot")
                .copy_from_slice(&PIN.to_le_bytes());
            *data.get_mut(12).expect("option") = 1;
            data.get_mut(13..45)
                .expect("authority")
                .copy_from_slice(&AUTHORITY);
            data.get_mut(45..).expect("ELF").fill(7);
            programs.push(info(key, bpf_loader_upgradeable::ID, program, true));
            programdata.push(info(data_key, bpf_loader_upgradeable::ID, data, false));
        }
        let [core, claims, trading, resolution, custody]: [ExecutionRoleBindingV1; 5] =
            bindings.try_into().expect("five roles");
        let set = ExecutionReleaseSetV1::new(core, claims, trading, resolution, custody)
            .expect("release set");
        let release_set = hash(&set.to_bytes()).to_bytes();
        let content = ContentId::new(release_set).expect("set id");
        let mut data = vec![0; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        initialize_activation_cache_v1(&mut data, content).expect("cache");
        for (role, input) in [
            ExecutionRoleV1::Core,
            ExecutionRoleV1::Claims,
            ExecutionRoleV1::Trading,
            ExecutionRoleV1::Resolution,
            ExecutionRoleV1::Custody,
        ]
        .into_iter()
        .zip(inputs)
        {
            activate_execution_role_into_v1(&mut data, content, &set, role, &input)
                .expect("activated role");
        }
        let key =
            Pubkey::find_program_address(&[ACTIVATION_PDA_DOMAIN_V1, &release_set], &registry_key)
                .0;
        Self {
            registry: info(
                registry_key,
                solana_sdk_ids::native_loader::ID,
                vec![],
                true,
            ),
            cache: info(key, registry_key, data, false),
            programs,
            programdata,
            release_set,
        }
    }
}

fn plan_bytes_for(release_set: [u8; 32], role: CallerRole) -> Vec<u8> {
    let delta = SignedDeltaV3::new(DeltaDirectionV3::Debit, 1).expect("debit");
    let positions = [SignedDeltaPositionV3::new([7; 32], 4).expect("position")];
    let rows = [PositionDeltaV3::new(
        PositionDeltaInputV3 {
            position_index: 0,
            outcome: 0,
            delta,
        },
        1,
        1,
    )
    .expect("row")];
    let mut bytes = vec![0; plan_bytes(1, 1, 1).expect("width")];
    SignedDeltaPlanV3::encode_into(
        SignedDeltaPlanInputV3 {
            caller_role: role,
            release_set,
            market: [2; 32],
            request_id: [3; 32],
            product_record_digest: [4; 32],
            semantic_basis_id: [5; 32],
            linked_basis_record_digest: [6; 32],
            expected_market_revision: 3,
            claim_count: 1,
        },
        &positions,
        &[delta],
        &rows,
        &mut bytes,
    )
    .expect("plan");
    bytes
}

#[test]
fn signed_delta_caller_release_requires_live_loader_continuity() {
    let mut observed = Vec::new();
    let mut expected = Vec::new();
    let mut public_observed = Vec::new();
    let mut public_expected = Vec::new();
    for role in [CallerRole::Core, CallerRole::Trading] {
        for mutation in ["accepted", "slot", "authority", "owner", "counterpart_slot"] {
            let fixture = Fixture::new();
            let caller_index = if role == CallerRole::Core { 0 } else { 2 };
            let caller = fixture.programs.get(caller_index).expect("caller");
            let caller_data = fixture.programdata.get(caller_index).expect("caller data");
            let mut supplied_data = caller_data.clone();
            match mutation {
                "slot" => supplied_data
                    .try_borrow_mut_data()
                    .expect("data")
                    .get_mut(4..12)
                    .expect("slot")
                    .copy_from_slice(&(PIN + 1).to_le_bytes()),
                "authority" => supplied_data
                    .try_borrow_mut_data()
                    .expect("data")
                    .get_mut(13..45)
                    .expect("authority")
                    .fill(0x72),
                "owner" => supplied_data.owner = Box::leak(Box::new(system_program::ID)),
                // Claims is a cached state-owner counterpart for either external caller.
                "counterpart_slot" => fixture
                    .programdata
                    .get(1)
                    .expect("Claims data")
                    .try_borrow_mut_data()
                    .expect("data")
                    .get_mut(4..12)
                    .expect("slot")
                    .copy_from_slice(&(PIN + 1).to_le_bytes()),
                _ => {}
            }
            let bytes = plan_bytes_for(fixture.release_set, role);
            let plan = SignedDeltaPlanV3::decode(&bytes).expect("plan");
            let digest = hash(&bytes).to_bytes();
            let seeds = CallerAuthoritySeedsV1::from_bytes(
                fixture.release_set,
                plan.market(),
                execution_role(role),
                plan.request_id(),
                digest,
            )
            .expect("seeds");
            let key = Pubkey::find_program_address(&seeds.as_slices(), caller.key).0;
            let mut authority = info(key, system_program::ID, vec![], false);
            authority.is_signer = true;
            let dummy = info(Pubkey::new_unique(), system_program::ID, vec![], false);
            let core_data = if role == CallerRole::Core {
                &supplied_data
            } else {
                fixture.programdata.first().expect("Core data")
            };
            let accounts = SignedDeltaAccountsV3 {
                all: &[],
                authority: &authority,
                market: &dummy,
                basis_record: &dummy,
                product_record: &dummy,
                rent: &dummy,
                core_market: &dummy,
                cache: &fixture.cache,
                registry: &fixture.registry,
                caller_program: caller,
                caller_programdata: &supplied_data,
                claims_program: fixture.programs.get(1).expect("Claims"),
                claims_programdata: fixture.programdata.get(1).expect("Claims data"),
                core_program: fixture.programs.first().expect("Core"),
                core_programdata: core_data,
                positions: &[],
            };
            authenticate_authority(&accounts, plan, digest)
                .expect("actual caller PDA authorizes exact request");
            observed.push(authenticate_releases(&accounts, plan));
            expected.push(match mutation {
                "slot" => Err(SignedDeltaSbfErrorV3::ReleaseSuperseded.into()),
                "authority" | "owner" => Err(SignedDeltaSbfErrorV3::Release.into()),
                _ => Ok(()),
            });
            // Drive the real public dispatcher through its complete privilege
            // frame. Empty Claims state is a deliberate downstream sentinel:
            // valid release admission reaches ClaimsState, while stale callers
            // must stop at their exact release refusal before that decode.
            let spec = SignedDeltaFrameSpecV3::new(1).expect("frame");
            let mut public_frame = Vec::new();
            for index in 0..spec.account_count().expect("frame count") {
                let entry = spec.account(index).expect("coordinate");
                let mut value = match entry.role() {
                    ClaimsFrameRoleV1::CallerAuthority => authority.clone(),
                    ClaimsFrameRoleV1::ActivationCache => fixture.cache.clone(),
                    ClaimsFrameRoleV1::RegistryProgram => fixture.registry.clone(),
                    ClaimsFrameRoleV1::CallerProgram => caller.clone(),
                    ClaimsFrameRoleV1::CallerProgramData => supplied_data.clone(),
                    ClaimsFrameRoleV1::ClaimsProgram => accounts.claims_program.clone(),
                    ClaimsFrameRoleV1::ClaimsProgramData => accounts.claims_programdata.clone(),
                    ClaimsFrameRoleV1::CoreProgram => accounts.core_program.clone(),
                    ClaimsFrameRoleV1::CoreProgramData => core_data.clone(),
                    ClaimsFrameRoleV1::RentSysvar => {
                        info(sysvar::rent::ID, sysvar::ID, vec![], false)
                    }
                    _ => info(Pubkey::new_unique(), system_program::ID, vec![], false),
                };
                value.is_signer = entry.privileges().signer();
                value.is_writable = entry.privileges().writable();
                value.executable = entry.privileges().executable();
                public_frame.push(value);
            }
            public_observed.push(crate::process_instruction(
                accounts.claims_program.key,
                &public_frame,
                &bytes,
            ));
            public_expected.push(match mutation {
                "slot" => Err(SignedDeltaSbfErrorV3::ReleaseSuperseded.into()),
                "authority" | "owner" => Err(SignedDeltaSbfErrorV3::Release.into()),
                _ => Err(SignedDeltaSbfErrorV3::ClaimsState.into()),
            });
        }
    }
    assert_eq!(
        observed, expected,
        "Core then Trading: accepted, slot, authority, owner, counterpart slot"
    );
    assert_eq!(
        public_observed, public_expected,
        "public entry must execute the release gate before Claims state decode"
    );
}
