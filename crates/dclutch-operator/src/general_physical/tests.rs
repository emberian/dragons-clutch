use dclutch_core_contract::ContentId;
use dclutch_general_adapter_contract::{
    CandidateVerifierV1, GENERAL_CANDIDATE_PDA_DOMAIN_V1, GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
    GENERAL_PAGE_PDA_DOMAIN_V1, GENERAL_POLICY_PDA_DOMAIN_V1, GENERAL_SELECTION_PDA_DOMAIN_V1,
    GENERAL_SETTLEMENT_PDA_DOMAIN_V1, GENERAL_VERIFICATION_PDA_DOMAIN_V1,
    VERIFICATION_CURSOR_BYTES_V1, VERIFIED_CANDIDATE_BYTES_V1,
};
use dclutch_general_codec::{
    Action, CandidateV1, ControllerRequestV1, ExecutionV1, MAX_EXECUTIONS_PER_PAGE, MAX_OUTCOMES,
    MAX_SELECTION_CRITERIA, PAGE_BYTES, PageV1, SELECTION_CURSOR_BYTES, SETTLEMENT_CURSOR_BYTES,
    SelectionCriterion, SelectionCursorV1, SelectionPolicyV1,
};
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    ExecutionReleaseActivationInputsV1, activate_execution_release_set_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ProgramIdentityV1,
};
use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::bpf_loader_upgradeable;

use super::*;

fn id(seed: u8) -> [u8; 32] {
    let mut value = [0; 32];
    value[0] = seed;
    value
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

fn program_bytes(programdata: Pubkey) -> Vec<u8> {
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

fn programdata_bytes(slot: u64, elf: &[u8]) -> Vec<u8> {
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

struct Deployment {
    release: ArtifactReleaseV1,
    artifact_id: ArtifactReleaseIdV1,
    program: ObservedAccount,
    programdata: ObservedAccount,
    activation: ArtifactActivationInputV1,
}

fn deployment(program: Pubkey, artifact_seed: u8, semantic_seed: u8, elf_seed: u8) -> Deployment {
    let programdata =
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
    let elf = vec![elf_seed; 96];
    let elf_digest = hash(&elf).to_bytes();
    let release = ArtifactReleaseV1::new(
        ProgramIdentityV1::new(program.to_bytes()).expect("program"),
        ProgramIdentityV1::new(bpf_loader_upgradeable::ID.to_bytes()).expect("loader"),
        programdata.to_bytes(),
        ContentId::new(id(semantic_seed)).expect("semantic release"),
        elf_digest,
        77,
        ArtifactUpgradePolicyV1::Immutable,
        None,
    )
    .expect("artifact release");
    let artifact_id = ArtifactReleaseIdV1::new(id(artifact_seed)).expect("artifact ID");
    let activation = ArtifactActivationInputV1::new(
        artifact_id,
        release,
        DeploymentObservationV1::new(
            program.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            true,
            programdata.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            false,
            programdata.to_bytes(),
            bpf_loader_upgradeable::ID.to_bytes(),
            77,
            elf_digest,
            None,
        )
        .expect("deployment observation"),
    );
    Deployment {
        release,
        artifact_id,
        program: observed(
            program,
            bpf_loader_upgradeable::ID,
            true,
            program_bytes(programdata),
        ),
        programdata: observed(
            programdata,
            bpf_loader_upgradeable::ID,
            false,
            programdata_bytes(77, &elf),
        ),
        activation,
    }
}

fn vector(first: u64, second: u64) -> [u64; MAX_OUTCOMES] {
    let mut values = [0; MAX_OUTCOMES];
    values[0] = first;
    values[1] = second;
    values
}

fn candidate() -> CandidateV1 {
    CandidateV1 {
        outcome_count: 2,
        candidate_id: id(21),
        product_id: id(31),
        batch_id: id(41),
        page_count: 1,
        price_scale: 2,
        prices: vector(1, 1),
    }
}

fn policy() -> SelectionPolicyV1 {
    let mut criteria = [SelectionCriterion::MaximizeFilledLots; MAX_SELECTION_CRITERIA];
    criteria[1] = SelectionCriterion::MinimizeQuoteSurplus;
    criteria[2] = SelectionCriterion::MinimizeCandidateId;
    SelectionPolicyV1 {
        policy_id: id(51),
        criterion_count: 3,
        criteria,
    }
}

fn page() -> [u8; PAGE_BYTES] {
    let mut rows = [ExecutionV1::EMPTY; MAX_EXECUTIONS_PER_PAGE];
    rows[0] = ExecutionV1 {
        order_id: id(1),
        owner_id: id(11),
        nonce: 1,
        max_lots: 1,
        max_quote_debit_per_lot: 1,
        lots: 1,
        quote_debit: 1,
        quote_credit: 0,
        receive_per_lot: vector(1, 0),
        deliver_per_lot: [0; MAX_OUTCOMES],
    };
    rows[1] = ExecutionV1 {
        order_id: id(2),
        owner_id: id(12),
        nonce: 1,
        max_lots: 1,
        max_quote_debit_per_lot: 1,
        lots: 1,
        quote_debit: 1,
        quote_credit: 0,
        receive_per_lot: vector(0, 1),
        deliver_per_lot: [0; MAX_OUTCOMES],
    };
    PageV1 {
        outcome_count: 2,
        candidate_id: candidate().candidate_id,
        page_index: 0,
        page_count: 1,
        execution_count: 2,
        executions: rows,
    }
    .to_bytes()
    .expect("page")
}

fn verified() -> VerifiedCandidateV1 {
    let mut verifier = CandidateVerifierV1::begin(candidate());
    verifier.ingest_page(&page()).expect("valid page");
    verifier.finish().expect("verified candidate")
}

struct Fixture {
    registry: Pubkey,
    program: Pubkey,
    market: ObservedAccount,
    common: GeneralCommonStateV1,
}

impl Fixture {
    fn new() -> Self {
        let registry = Pubkey::new_from_array(id(70));
        let program = Pubkey::new_from_array(id(71));
        let core = deployment(registry, 72, 73, 74);
        let trading = deployment(program, 75, 76, 77);
        let core_binding = ExecutionRoleBindingV1::new(core.release.program(), core.artifact_id);
        let trading_binding =
            ExecutionRoleBindingV1::new(trading.release.program(), trading.artifact_id);
        let release_set = ExecutionReleaseSetV1::new(
            core_binding,
            core_binding,
            trading_binding,
            core_binding,
            core_binding,
        )
        .expect("release set");
        let release_set_id =
            ContentId::new(hash(&release_set.to_bytes()).to_bytes()).expect("release-set ID");
        let inputs = ExecutionReleaseActivationInputsV1::new(
            core.activation,
            core.activation,
            trading.activation,
            core.activation,
            core.activation,
        );
        let activation = activate_execution_release_set_v1(release_set_id, &release_set, &inputs)
            .expect("activation");
        let cache_key = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, release_set_id.as_bytes()],
            &registry,
        )
        .0;
        let market = observed(
            Pubkey::new_from_array(id(80)),
            Pubkey::new_from_array(id(81)),
            false,
            vec![1],
        );
        let common = GeneralCommonStateV1 {
            market: market.clone(),
            trading_release: RegistryReauthenticationState {
                registry_program: core.program,
                cache: observed(cache_key, registry, false, activation.to_bytes().to_vec()),
                role_program: trading.program,
                role_programdata: trading.programdata,
            },
        };
        assert_eq!(
            common.trading_release.cache.data.len(),
            ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1
        );
        Self {
            registry,
            program,
            market,
            common,
        }
    }

    fn general_account(&self, key: Pubkey, bytes: Vec<u8>) -> ObservedAccount {
        observed(key, self.program, false, bytes)
    }

    fn pda(&self, seeds: &[&[u8]]) -> Pubkey {
        Pubkey::find_program_address(seeds, &self.program).0
    }

    fn consider(&self) -> GeneralConsiderStateV1 {
        let candidate = candidate();
        let policy = policy();
        let market = self.market.key.to_bytes();
        let page_index = 0_u32.to_le_bytes();
        GeneralConsiderStateV1 {
            common: self.common.clone(),
            selection: self.general_account(
                self.pda(&[
                    GENERAL_SELECTION_PDA_DOMAIN_V1,
                    &market,
                    &candidate.batch_id,
                ]),
                vec![0; SELECTION_CURSOR_BYTES],
            ),
            verification: self.general_account(
                self.pda(&[
                    GENERAL_VERIFICATION_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                ]),
                vec![0; VERIFICATION_CURSOR_BYTES_V1],
            ),
            certificate: self.general_account(
                self.pda(&[
                    GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                ]),
                vec![0; VERIFIED_CANDIDATE_BYTES_V1],
            ),
            candidate: self.general_account(
                self.pda(&[
                    GENERAL_CANDIDATE_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                ]),
                candidate.to_bytes().expect("candidate").to_vec(),
            ),
            policy: self.general_account(
                self.pda(&[GENERAL_POLICY_PDA_DOMAIN_V1, &market, &policy.policy_id]),
                policy.to_bytes().expect("policy").to_vec(),
            ),
            page: self.general_account(
                self.pda(&[
                    GENERAL_PAGE_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                    &page_index,
                ]),
                page().to_vec(),
            ),
            incumbent_certificate: self.market.clone(),
        }
    }

    fn freeze(&self) -> GeneralFreezeStateV1 {
        let candidate = candidate();
        let policy = policy();
        let market = self.market.key.to_bytes();
        let selection = SelectionCursorV1 {
            closed: false,
            batch_id: candidate.batch_id,
            policy_id: policy.policy_id,
            best_candidate_id: Some(candidate.candidate_id),
            revision: 1,
        };
        GeneralFreezeStateV1 {
            common: self.common.clone(),
            selection: self.general_account(
                self.pda(&[
                    GENERAL_SELECTION_PDA_DOMAIN_V1,
                    &market,
                    &candidate.batch_id,
                ]),
                selection.to_bytes().expect("selection").to_vec(),
            ),
        }
    }

    fn initialize(&self) -> GeneralInitializeStateV1 {
        let candidate = candidate();
        let policy = policy();
        let market = self.market.key.to_bytes();
        let selection = SelectionCursorV1 {
            closed: true,
            batch_id: candidate.batch_id,
            policy_id: policy.policy_id,
            best_candidate_id: Some(candidate.candidate_id),
            revision: 2,
        };
        GeneralInitializeStateV1 {
            common: self.common.clone(),
            selection: self.general_account(
                self.pda(&[
                    GENERAL_SELECTION_PDA_DOMAIN_V1,
                    &market,
                    &candidate.batch_id,
                ]),
                selection.to_bytes().expect("selection").to_vec(),
            ),
            settlement: self.general_account(
                self.pda(&[
                    GENERAL_SETTLEMENT_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                ]),
                vec![0; SETTLEMENT_CURSOR_BYTES],
            ),
            certificate: self.general_account(
                self.pda(&[
                    GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                ]),
                verified().to_bytes().expect("certificate").to_vec(),
            ),
            candidate: self.general_account(
                self.pda(&[
                    GENERAL_CANDIDATE_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                ]),
                candidate.to_bytes().expect("candidate").to_vec(),
            ),
        }
    }
}

#[test]
fn consider_matches_the_committed_frontend_frame_and_packet_bounds() {
    let fixture = Fixture::new();
    let report = build_general_consider_v1(&fixture.consider()).expect("Consider plan");
    assert_eq!(report.action, Action::Consider);
    assert_eq!(report.instruction.program_id, fixture.program);
    assert_eq!(
        report.instruction.accounts.len(),
        GENERAL_CONSIDER_ACCOUNT_COUNT_V1
    );
    assert!(
        report
            .instruction
            .accounts
            .iter()
            .all(|meta| !meta.is_signer)
    );
    assert_eq!(
        report
            .instruction
            .accounts
            .iter()
            .map(|meta| meta.is_writable)
            .collect::<Vec<_>>(),
        vec![
            false, false, false, false, false, true, true, true, false, false, false, false
        ]
    );
    let request = ControllerRequestV1::decode(&report.instruction.data).expect("request");
    assert_eq!(request.action, Action::Consider);
    assert_eq!(request.expected_revision, 0);
    assert_eq!(request.candidate_id, Some(candidate().candidate_id));
    assert_eq!(request.page_index, 0);
    assert_eq!(report.compute.matching_measured_compute_units, None);
    assert_eq!(report.compute.trading_elf_bytes_hashed, 96);

    let packet = compile_general_packet_v0(
        &report,
        Pubkey::new_from_array(id(90)),
        Hash::new_from_array(id(91)),
        500_000,
    )
    .expect("packet");
    assert_eq!(packet.required_signatures, 1);
    assert!(packet.wire_bytes <= PACKET_DATA_BYTES);
    assert_eq!(packet.compute_unit_limit, 500_000);
}

#[test]
fn freeze_and_initialize_derive_exact_revisions_roles_and_pdas() {
    let fixture = Fixture::new();
    let freeze = build_general_freeze_v1(&fixture.freeze()).expect("Freeze plan");
    assert_eq!(
        freeze.instruction.accounts.len(),
        GENERAL_FREEZE_ACCOUNT_COUNT_V1
    );
    assert_eq!(freeze.expected_revision, 1);
    assert!(
        freeze
            .instruction
            .accounts
            .get(5)
            .expect("selection meta")
            .is_writable
    );
    assert_eq!(
        ControllerRequestV1::decode(&freeze.instruction.data)
            .expect("Freeze request")
            .action,
        Action::Freeze
    );

    let initialize = build_general_initialize_v1(&fixture.initialize()).expect("Initialize plan");
    assert_eq!(
        initialize.instruction.accounts.len(),
        GENERAL_INITIALIZE_ACCOUNT_COUNT_V1
    );
    assert_eq!(initialize.expected_revision, 0);
    assert_eq!(
        initialize
            .instruction
            .accounts
            .iter()
            .map(|meta| meta.is_writable)
            .collect::<Vec<_>>(),
        vec![false, false, false, false, false, false, true, false, false]
    );
    let request =
        ControllerRequestV1::decode(&initialize.instruction.data).expect("Initialize request");
    assert_eq!(request.action, Action::InitializeSettlement);
    assert_eq!(request.candidate_id, Some(candidate().candidate_id));
}

#[test]
fn stale_substituted_and_wrong_owner_observations_refuse() {
    let fixture = Fixture::new();

    let mut stale = fixture.consider();
    stale.page.observation.slot += 1;
    assert_eq!(
        build_general_consider_v1(&stale),
        Err(Error::ObservationMismatch)
    );

    let mut substituted = fixture.consider();
    substituted.candidate.key = Pubkey::new_unique();
    assert_eq!(
        build_general_consider_v1(&substituted),
        Err(Error::ImmutableInput)
    );

    let mut wrong_owner = fixture.consider();
    wrong_owner.page.owner = fixture.registry;
    assert_eq!(
        build_general_consider_v1(&wrong_owner),
        Err(Error::AccountShape)
    );

    let mut stale_verifier = fixture.consider();
    stale_verifier
        .verification
        .data
        .get_mut(952..960)
        .expect("revision")
        .copy_from_slice(&7_u64.to_le_bytes());
    assert_eq!(
        build_general_consider_v1(&stale_verifier),
        Err(Error::Verification)
    );

    let mut stale_loader = fixture.freeze();
    stale_loader
        .common
        .trading_release
        .role_programdata
        .data
        .get_mut(4..12)
        .expect("slot")
        .copy_from_slice(&78_u64.to_le_bytes());
    assert_eq!(
        build_general_freeze_v1(&stale_loader),
        Err(Error::ReleaseAdmission)
    );

    let mut substituted_certificate = fixture.initialize();
    substituted_certificate
        .certificate
        .data
        .get_mut(16)
        .map(|byte| *byte ^= 1)
        .expect("candidate ID byte");
    assert_eq!(
        build_general_initialize_v1(&substituted_certificate),
        Err(Error::Certificate)
    );
}

#[test]
fn semantic_aliases_and_invalid_packet_plumbing_refuse() {
    let fixture = Fixture::new();
    let mut aliased = fixture.consider();
    aliased.policy.key = aliased.candidate.key;
    assert_eq!(
        build_general_consider_v1(&aliased),
        Err(Error::AccountAlias)
    );

    let report = build_general_freeze_v1(&fixture.freeze()).expect("Freeze report");
    assert_eq!(
        compile_general_packet_v0(
            &report,
            fixture.market.key,
            Hash::new_from_array(id(92)),
            500_000,
        ),
        Err(Error::FeePayerAlias)
    );
    assert_eq!(
        compile_general_packet_v0(
            &report,
            Pubkey::new_unique(),
            Hash::new_from_array(id(93)),
            0,
        ),
        Err(Error::InvalidComputeLimit)
    );
    assert_eq!(
        compile_general_packet_v0(
            &report,
            Pubkey::new_unique(),
            Hash::new_from_array(id(94)),
            TRANSACTION_COMPUTE_UNIT_LIMIT_V1 + 1,
        ),
        Err(Error::InvalidComputeLimit)
    );
}
