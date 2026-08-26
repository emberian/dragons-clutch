use dclutch_capability_program_contract::{
    CAPABILITY_ROOT_HEADER_BYTES_V1, CapabilityRootHeaderV1,
};
use dclutch_core_contract::ContentId;
use dclutch_custody_contract::{CallerRoleV1, CustodyReplayV1};
use dclutch_economic_slice_kernel::{
    MARKET_HEADER_BYTES, POSITION_HEADER_BYTES, Phase as ClaimsPhase, SCALAR_BYTES,
    initialize_market, initialize_position,
};
use dclutch_general_adapter_contract::{
    CandidateVerifierV1, GENERAL_CANDIDATE_PDA_DOMAIN_V1, GENERAL_CERTIFICATE_PDA_DOMAIN_V1,
    GENERAL_PAGE_PDA_DOMAIN_V1, GENERAL_POLICY_PDA_DOMAIN_V1, GENERAL_SELECTION_PDA_DOMAIN_V1,
    GENERAL_SETTLEMENT_PDA_DOMAIN_V1, GENERAL_VERIFICATION_PDA_DOMAIN_V1,
    VERIFICATION_CURSOR_BYTES_V1, VERIFIED_CANDIDATE_BYTES_V1,
};
use dclutch_general_codec::{
    Action, CandidateV1, ControllerRequestV1, ExecutionV1, MAX_EXECUTIONS_PER_PAGE, MAX_OUTCOMES,
    MAX_SELECTION_CRITERIA, PAGE_BYTES, PageV1, Phase, SELECTION_CURSOR_BYTES,
    SETTLEMENT_CURSOR_BYTES, SelectionCriterion, SelectionCursorV1, SelectionPolicyV1,
    SettlementCursorV1,
};
use dclutch_general_config_contract::{
    GENERAL_CONFIG_SCHEMA_ID_V2, GeneralConfigV2, GeneralConfigV2Input,
};
use dclutch_record_contract::RAW_RECORD_PDA_SEED_V1;
use dclutch_registry_contract::{
    ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1, ArtifactActivationInputV1,
    ArtifactReleaseV1, ArtifactUpgradePolicyV1, DeploymentObservationV1,
    ExecutionReleaseActivationInputsV1, activate_execution_release_set_v1,
};
use dclutch_release_set_contract::{
    ArtifactReleaseIdV1, CapabilityExecutionSelectionV1, ExecutionReleaseSetV1,
    ExecutionRoleBindingV1, ProgramIdentityV1,
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
    core_programdata: ObservedAccount,
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
                registry_program: core.program.clone(),
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
            core_programdata: core.programdata,
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

    fn settlement(&self) -> GeneralSettlementStateV1 {
        let candidate = candidate();
        let market = self.market.key.to_bytes();
        let settlement = SettlementCursorV1 {
            phase: Phase::Collecting,
            outcome_count: candidate.outcome_count,
            candidate_id: candidate.candidate_id,
            page_count: candidate.page_count,
            next_page: 0,
            next_execution: 0,
            revision: 0,
            claim_inventory: [0; MAX_OUTCOMES],
            quote_inventory: 0,
            quote_surplus_paid: 0,
        };
        let role = RegistryReauthenticationState {
            registry_program: self.common.trading_release.registry_program.clone(),
            cache: self.common.trading_release.cache.clone(),
            role_program: self.common.trading_release.registry_program.clone(),
            role_programdata: self.core_programdata.clone(),
        };
        let release_set = build_registry_reauthentication_v1(
            &self.common.trading_release,
            dclutch_release_set_contract::ExecutionRoleV1::Trading,
        )
        .expect("Trading release")
        .execution_release_set_id
        .to_bytes();
        let inert = |seed| {
            observed(
                Pubkey::new_from_array(id(seed)),
                self.program,
                false,
                vec![1],
            )
        };
        let claims_program = role.role_program.key;
        let count = u32::from(candidate.outcome_count);
        let mut claims_market =
            vec![0; MARKET_HEADER_BYTES + usize::from(candidate.outcome_count) * 3 * SCALAR_BYTES];
        initialize_market(
            &mut claims_market,
            market,
            release_set,
            self.registry.to_bytes(),
            count,
            ClaimsPhase::Open,
            0,
        )
        .expect("claims market");
        let mut owner_position = vec![
            0;
            POSITION_HEADER_BYTES
                + usize::from(candidate.outcome_count) * 2 * SCALAR_BYTES
        ];
        initialize_position(&mut owner_position, market, id(11), count).expect("owner position");
        let mut settlement_position =
            vec![
                0;
                POSITION_HEADER_BYTES + usize::from(candidate.outcome_count) * 2 * SCALAR_BYTES
            ];
        initialize_position(
            &mut settlement_position,
            market,
            candidate.candidate_id,
            count,
        )
        .expect("settlement position");
        let replay = CustodyReplayV1 {
            caller_role: CallerRoleV1::Trading,
            release_set,
            market,
            realm: id(90),
            context: candidate.candidate_id,
            caller_program: self.program.to_bytes(),
            rent_refund: id(91),
            open_vault_count: 2,
            next_revision: 1,
            generation: 1,
            last_request_digest: id(92),
            last_poststate_commitment: id(93),
        };
        let config = GeneralConfigV2::new(GeneralConfigV2Input {
            capacity_profile_id: id(94),
            claim_basis_id: id(95),
            capability_program_id: id(96),
            generation: 1,
            price_scale: 2,
            collection_slots: 1,
            selection_slots: 1,
            settlement_slots: 1,
            max_orders_per_candidate: 16,
            max_pages_per_candidate: 1,
            continuation_reward_lamports: 1,
            selection_policy_id: policy().policy_id,
            outcome_count: 2,
            quote_surplus_beneficiary: id(97),
        })
        .expect("config");
        let config_data = config.to_bytes().to_vec();
        let config_digest = hash(&config_data).to_bytes();
        let config_key = Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &GENERAL_CONFIG_SCHEMA_ID_V2,
                &config_digest,
            ],
            &self.registry,
        )
        .0;
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            ContentId::new(id(113)).expect("manifest ID"),
            ContentId::new(id(114)).expect("General kind"),
            ContentId::new(id(96)).expect("capability program ID"),
            ContentId::new(config_digest).expect("config ID"),
        )
        .expect("selection");
        let root_header = CapabilityRootHeaderV1::new(
            ContentId::new(release_set).expect("release-set ID"),
            market,
            1,
            selection,
        )
        .expect("root header");
        let root_seeds = root_header.seeds();
        let root_key = Pubkey::find_program_address(&root_seeds.as_slices(), &self.program).0;
        let mut root_data = vec![0; CAPABILITY_ROOT_HEADER_BYTES_V1 + 128];
        root_data
            .get_mut(..CAPABILITY_ROOT_HEADER_BYTES_V1)
            .expect("root header range")
            .copy_from_slice(&root_header.to_bytes());
        let mut state = GeneralSettlementStateV1 {
            common: self.common.clone(),
            general_root: observed(root_key, self.program, false, root_data),
            core_release: role.clone(),
            claims_release: role.clone(),
            custody_release: role,
            claims_caller_authority: observed(claims_program, self.registry, true, vec![]),
            custody_caller_authority: observed(self.registry, self.registry, true, vec![]),
            settlement: self.general_account(
                self.pda(&[
                    GENERAL_SETTLEMENT_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                ]),
                settlement.to_bytes().expect("settlement").to_vec(),
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
            page: self.general_account(
                self.pda(&[
                    GENERAL_PAGE_PDA_DOMAIN_V1,
                    &market,
                    &candidate.candidate_id,
                    &0_u32.to_le_bytes(),
                ]),
                page().to_vec(),
            ),
            claims_market: observed(inert(101).key, claims_program, false, claims_market),
            owner_position: observed(inert(102).key, claims_program, false, owner_position),
            settlement_position: observed(
                inert(103).key,
                claims_program,
                false,
                settlement_position,
            ),
            realm: inert(105),
            realm_staging: inert(104),
            custody_replay: observed(
                inert(115).key,
                self.registry,
                false,
                replay.to_bytes().expect("replay").to_vec(),
            ),
            mint: inert(106),
            collateral_source: inert(107),
            collateral_destination: inert(108),
            custody_authority: inert(109),
            token_program: observed(Pubkey::new_from_array(id(110)), self.registry, true, vec![]),
            general_config: observed(config_key, self.registry, false, config_data),
        };
        bind_authorities(&mut state, Action::Collect);
        state
    }
}

fn bind_authorities(state: &mut GeneralSettlementStateV1, action: Action) {
    let authority = authenticate_common(&state.common).expect("common authority");
    let root = authenticate_general_root(state, authority).expect("selected root");
    let config = authenticate_general_config(state, root).expect("selected config");
    let verified = VerifiedCandidateV1::decode(&state.certificate.data).expect("certificate");
    let cursor = SettlementCursorV1::decode(&state.settlement.data).expect("cursor");
    let context = ExecutionContextV1 {
        market_id: state.common.market.key.to_bytes(),
        release_set_id: authority.release_set_id,
    };
    let mut staged = state.settlement.data.clone();
    let mut children =
        OperatorChildren::new(state, verified, config, authority).expect("operator children");
    let page = if matches!(action, Action::Collect | Action::Distribute) {
        state.page.data.as_slice()
    } else {
        &[]
    };
    match action {
        Action::Collect => collect_execution(
            &mut staged,
            context,
            &verified,
            page,
            cursor.revision,
            &mut children,
        ),
        Action::Materialize => materialize(
            &mut staged,
            context,
            &verified,
            cursor.revision,
            &mut children,
        ),
        Action::Distribute => distribute_execution(
            &mut staged,
            context,
            &verified,
            page,
            cursor.revision,
            &mut children,
        ),
        Action::Close => close(
            &mut staged,
            context,
            &verified,
            cursor.revision,
            &mut children,
        ),
        _ => panic!("settlement action"),
    }
    .expect("packet projection");
    let claims = children.claims;
    let custody = children.custody;
    let (claims_key, custody_key) =
        packet_authority_keys(state, authority, claims, custody).expect("authority keys");
    state.claims_caller_authority = if claims.is_some() {
        observed(claims_key, authority.program_id, false, vec![])
    } else {
        state.claims_release.role_program.clone()
    };
    state.custody_caller_authority = if custody.is_some() {
        observed(custody_key, authority.program_id, false, vec![])
    } else {
        state.custody_release.role_program.clone()
    };
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

#[test]
fn collect_route_is_chain_derived_permissionless_and_packet_safe() {
    let fixture = Fixture::new();
    let state = fixture.settlement();
    let report = build_general_settlement_v1(&state, Action::Collect).expect("Collect plan");
    assert_eq!(report.action, Action::Collect);
    assert_eq!(report.expected_revision, 0);
    assert_eq!(
        report.instruction.accounts.len(),
        GENERAL_SETTLEMENT_ACCOUNT_COUNT_V1
    );
    assert!(
        report
            .instruction
            .accounts
            .iter()
            .all(|meta| !meta.is_signer)
    );
    let request = ControllerRequestV1::decode(&report.instruction.data).expect("request");
    assert_eq!(request.action, Action::Collect);
    assert_eq!(request.page_index, 0);
    assert_eq!(request.execution_index, 0);
    let packet = compile_general_packet_v0(
        &report,
        Pubkey::new_from_array(id(111)),
        Hash::new_from_array(id(112)),
        1_000_000,
    )
    .expect("packet");
    assert!(packet.wire_bytes <= PACKET_DATA_BYTES);
}

#[test]
fn settlement_family_suffix_has_the_controller_order_without_root_duplication() {
    let fixture = Fixture::new();
    let state = fixture.settlement();
    let suffix = general_settlement_suffix_metas(&state, Action::Collect);
    let keys = suffix.iter().map(|meta| meta.pubkey).collect::<Vec<_>>();
    assert_eq!(suffix.len(), GENERAL_SETTLEMENT_ACCOUNT_COUNT_V1);
    assert_eq!(
        keys,
        vec![
            state.common.market.key,
            state.common.trading_release.cache.key,
            state.common.trading_release.registry_program.key,
            state.common.trading_release.role_program.key,
            state.common.trading_release.role_programdata.key,
            state.core_release.role_program.key,
            state.core_release.role_programdata.key,
            state.claims_release.role_program.key,
            state.claims_release.role_programdata.key,
            state.custody_release.role_program.key,
            state.claims_caller_authority.key,
            state.custody_caller_authority.key,
            state.settlement.key,
            state.certificate.key,
            state.candidate.key,
            state.page.key,
            state.claims_market.key,
            state.owner_position.key,
            state.settlement_position.key,
            state.realm.key,
            state.realm_staging.key,
            state.custody_replay.key,
            state.mint.key,
            state.collateral_source.key,
            state.collateral_destination.key,
            state.custody_authority.key,
            state.token_program.key,
            state.general_config.key,
        ]
    );
    assert!(!keys.contains(&state.general_root.key));
}

#[test]
fn aggregate_and_distribution_routes_construct_from_their_exact_cursor_phases() {
    let fixture = Fixture::new();

    let mut materialize_state = fixture.settlement();
    materialize_state.page = materialize_state.common.market.clone();
    materialize_state.settlement.data = SettlementCursorV1 {
        phase: Phase::Materializing,
        outcome_count: 2,
        candidate_id: candidate().candidate_id,
        page_count: 1,
        next_page: 0,
        next_execution: 0,
        revision: 2,
        claim_inventory: [0; MAX_OUTCOMES],
        quote_inventory: 2,
        quote_surplus_paid: 0,
    }
    .to_bytes()
    .expect("materializing cursor")
    .to_vec();
    bind_authorities(&mut materialize_state, Action::Materialize);
    let materialize = build_general_settlement_v1(&materialize_state, Action::Materialize)
        .expect("Materialize plan");
    assert_eq!(materialize.expected_revision, 2);

    let mut distribute_state = fixture.settlement();
    distribute_state.settlement.data = SettlementCursorV1 {
        phase: Phase::Distributing,
        outcome_count: 2,
        candidate_id: candidate().candidate_id,
        page_count: 1,
        next_page: 0,
        next_execution: 0,
        revision: 3,
        claim_inventory: vector(1, 1),
        quote_inventory: 1,
        quote_surplus_paid: 0,
    }
    .to_bytes()
    .expect("distributing cursor")
    .to_vec();
    bind_authorities(&mut distribute_state, Action::Distribute);
    let distribute = build_general_settlement_v1(&distribute_state, Action::Distribute)
        .expect("Distribute plan");
    assert_eq!(distribute.expected_revision, 3);

    let mut close_state = fixture.settlement();
    close_state.page = close_state.common.market.clone();
    close_state.settlement.data = SettlementCursorV1 {
        phase: Phase::ReadyToClose,
        outcome_count: 2,
        candidate_id: candidate().candidate_id,
        page_count: 1,
        next_page: 0,
        next_execution: 0,
        revision: 5,
        claim_inventory: [0; MAX_OUTCOMES],
        quote_inventory: 1,
        quote_surplus_paid: 0,
    }
    .to_bytes()
    .expect("close cursor")
    .to_vec();
    bind_authorities(&mut close_state, Action::Close);
    let close = build_general_settlement_v1(&close_state, Action::Close).expect("Close plan");
    assert_eq!(close.expected_revision, 5);
    for report in [materialize, distribute, close] {
        assert_eq!(
            report.instruction.accounts.len(),
            GENERAL_SETTLEMENT_ACCOUNT_COUNT_V1
        );
        assert!(
            report
                .instruction
                .accounts
                .iter()
                .all(|meta| !meta.is_signer)
        );
    }
}

#[test]
fn stale_settlement_and_substituted_page_refuse_before_construction() {
    let fixture = Fixture::new();
    let mut stale = fixture.settlement();
    stale.settlement.observation.slot += 1;
    assert_eq!(
        build_general_settlement_v1(&stale, Action::Collect),
        Err(Error::ObservationMismatch)
    );

    let mut substituted = fixture.settlement();
    substituted.page.key = Pubkey::new_unique();
    assert_eq!(
        build_general_settlement_v1(&substituted, Action::Collect),
        Err(Error::ImmutableInput)
    );

    assert_eq!(
        build_general_settlement_v1(&fixture.settlement(), Action::Freeze),
        Err(Error::Encoding)
    );
}

#[test]
fn alternate_content_addressed_config_cannot_substitute_for_root_selection() {
    let fixture = Fixture::new();
    let mut state = fixture.settlement();
    let alternate = GeneralConfigV2::new(GeneralConfigV2Input {
        capacity_profile_id: id(94),
        claim_basis_id: id(95),
        capability_program_id: id(96),
        generation: 1,
        price_scale: 2,
        collection_slots: 1,
        selection_slots: 1,
        settlement_slots: 1,
        max_orders_per_candidate: 16,
        max_pages_per_candidate: 1,
        continuation_reward_lamports: 1,
        selection_policy_id: policy().policy_id,
        outcome_count: 2,
        quote_surplus_beneficiary: id(118),
    })
    .expect("alternate config");
    let data = alternate.to_bytes().to_vec();
    let digest = hash(&data).to_bytes();
    state.general_config = observed(
        Pubkey::find_program_address(
            &[
                RAW_RECORD_PDA_SEED_V1,
                &GENERAL_CONFIG_SCHEMA_ID_V2,
                &digest,
            ],
            &fixture.registry,
        )
        .0,
        fixture.registry,
        false,
        data,
    );

    assert_eq!(
        build_general_settlement_v1(&state, Action::Collect),
        Err(Error::AccountShape)
    );
}
