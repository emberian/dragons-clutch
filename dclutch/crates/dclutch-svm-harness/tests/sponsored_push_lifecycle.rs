/// Real-SBF coverage for the sponsored-push lifecycle extends the checked
/// successor role mapping. Provider bytes and every account value are synthetic;
/// the Resolution and Core programs are the linked SBF artifacts named by
/// `SBF_OUT_DIR`.
mod sponsored_campaign {
    use std::{env, fs, path::PathBuf};

    use dclutch_capability_contract::{
        ActivationPolicy, CAPABILITY_ENTRY_BYTES, CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
        CapabilityEntryV1, CapabilityFundingLedgerDerivationV2, CapabilityManifestV1,
        CompartmentFundingV1, ContentId as CapabilityContentId, FUNDING_STATE_BYTES,
        FundingAmountsV1, FundingLedgerV2, FundingQuoteV1, MANIFEST_HEADER_BYTES,
        MAX_DEPENDENCIES_PER_CAPABILITY, funding_ledger_bytes_v2,
    };
    use dclutch_core_contract::ContentId;
    use dclutch_market_core_codec::{
        CoreState, Identity as CoreIdentity, MarketCoreStateSeedsV2, MarketIdentity, Phase,
        Readiness, StateBumpsV1,
    };
    use dclutch_product_runtime_v2::{
        ContentId as ProductContentId, PortfolioInputV2, ResultDomainInputV2, compile_portfolio_v2,
        compile_result_domain_v2, portfolio_record_bytes, result_domain_record_bytes,
    };
    use dclutch_product_runtime_v2_admission::{
        PORTFOLIO_SCHEMA_ID_V2, PRODUCT_RECORD_BYTES_V2, PRODUCT_RECORD_SCHEMA_ID_V2,
        ProductRecordV2, RESULT_DOMAIN_SCHEMA_ID_V2,
    };
    use dclutch_pyth_svm::{
        DEVNET_CLUSTER_ID_V1, FULL_PRICE_UPDATE_V2_LEN, PythSponsoredPushReleaseV1,
        PythSponsoredPushReleaseV1Input, RECEIVER_CONFIG_V2_LEN,
        price_update::PRICE_UPDATE_V2_DISCRIMINATOR,
        sponsored_push::{
            PYTH_SPONSORED_PUSH_ADAPTER_ID_V1, PYTH_SPONSORED_PUSH_PROVIDER_FAMILY_ID_V1,
            PYTH_SPONSORED_PUSH_TRANSPORT_PROFILE_ID_V1,
        },
    };
    use dclutch_record_contract::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1};
    use dclutch_registry_contract::{
        ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1, ACTIVATION_PDA_DOMAIN_V1,
        ActivatedExecutionReleaseSetV1, ArtifactActivationInputV1, ArtifactReleaseV1,
        ArtifactUpgradePolicyV1, DeploymentObservationV1, activate_execution_role_into_v1,
        initialize_activation_cache_v1,
    };
    use dclutch_release_set_contract::{
        ArtifactReleaseIdV1, ExecutionReleaseSetV1, ExecutionRoleBindingV1, ExecutionRoleV1,
        ProgramIdentityV1,
    };
    use dclutch_resolution_codec::{
        RESOLUTION_CERTIFICATE_BYTES_V2, RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
        RESOLUTION_CONTROLLER_RELEASE_ID_V7, ResolutionCertificateKindV2, ResolutionCertificateV2,
        SPONSORED_PUSH_CANDIDATE_BYTES_V1, SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1,
        SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1, SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1,
        SponsoredPushActionV1, SponsoredPushCandidateV1, SponsoredPushHeadV1,
        SponsoredPushInstructionV1, SponsoredPushReceiptV1,
    };
    use dclutch_resolution_proof_sbf::ResolutionError;
    use dclutch_source_contract::{
        CapacityEnvelope as SourceCapacityEnvelope, ContentId as SourceContentId,
        PROVIDER_RELEASE_SCHEMA_ID_V1, PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
        PYTH_SPONSORED_PUSH_PROVIDER_EXTENSION_RELEASE_ID_V1, ProviderReleaseV1,
        PythAdapterConfigV1, RoundingBoundary, SOURCE_FAILURE_POLICY_RELEASE_ID_V2,
        SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3, STATISTIC_SPEC_SCHEMA_ID_V1, SourceAccessProfile,
        SourceCapacityProfileV1, SourceMaterialV3, SourceResolutionPhaseV1,
        SourceResolutionStateV2, SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind,
        WindowSpecV1,
    };
    use solana_account::{Account, AccountSharedData};
    use solana_program::{
        clock::Clock,
        hash::{hash, hashv},
        instruction::{AccountMeta, Instruction},
        pubkey::Pubkey,
        rent::Rent,
    };
    use solana_program_test::{BanksClientError, ProgramTest, ProgramTestContext};
    use solana_sdk::signature::{Keypair, Signer};
    use solana_sdk_ids::{bpf_loader_upgradeable, system_program, sysvar};
    use solana_transaction::{InstructionError, Transaction, TransactionError};

    const PROGRAM_ID: Pubkey = Pubkey::new_from_array([71; 32]);
    const CORE_PROGRAM_ID: Pubkey = Pubkey::new_from_array([72; 32]);
    const REGISTRY_PROGRAM_ID: Pubkey = Pubkey::new_from_array([73; 32]);
    const RENT_PROGRAM_ID: Pubkey = Pubkey::new_from_array([74; 32]);
    const GENERATION: u64 = 73;
    const CREATED_UNIX: i64 = 1_756_000_000;
    const TERMINAL_SEQUENCE: u64 = 1;
    const BOUNTY: u64 = 250_000;
    const RESOLUTION_SUCCESS_KIND: u8 = 1;
    const RESOLUTION_FAILURE_KIND: u8 = 4;
    const OUTCOME_COUNT: u32 = 2;

    struct Elves {
        core: Vec<u8>,
        resolution: Vec<u8>,
    }

    struct RecordPair {
        raw: Pubkey,
        staging: Pubkey,
    }

    impl Clone for RecordPair {
        fn clone(&self) -> Self {
            *self
        }
    }

    impl Copy for RecordPair {}

    struct ProductGraph {
        product: RecordPair,
        product_record_digest: [u8; 32],
        result_domain: RecordPair,
        portfolio: RecordPair,
        coordinate_domain_id: [u8; 32],
        result_unit_id: [u8; 32],
    }

    struct BaseFixture {
        test: Option<ProgramTest>,
        worker: Keypair,
        product: ProductGraph,
        activation: Pubkey,
        rent_beneficiary: Pubkey,
    }

    fn artifacts() -> Elves {
        let directory = PathBuf::from(
            env::var("SBF_OUT_DIR")
                .expect("SBF_OUT_DIR must contain checked Core and Resolution SBF artifacts"),
        );
        let core = fs::read(directory.join("dclutch_core_sbf.so")).expect("compiled Core ELF");
        let resolution = fs::read(directory.join("dclutch_resolution_proof_sbf.so"))
            .expect("compiled Resolution ELF");
        for (label, elf) in [
            ("Core", core.as_slice()),
            ("Resolution", resolution.as_slice()),
        ] {
            assert_eq!(
                elf.get(..4),
                Some(&[0x7f, b'E', b'L', b'F'][..]),
                "{label} ELF"
            );
            eprintln!(
                "sponsored-push {label} ELF SHA-256: {:?}",
                hash(elf).to_bytes()
            );
        }
        Elves { core, resolution }
    }

    fn programdata(program: Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0
    }

    fn add_program(test: &mut ProgramTest, name: &'static str, program: Pubkey) {
        test.add_upgradeable_program_to_genesis(name, &program);
    }

    fn protocol_account(owner: Pubkey, data: Vec<u8>) -> Account {
        Account {
            lamports: Rent::default().minimum_balance(data.len()).max(1),
            data,
            owner,
            executable: false,
            rent_epoch: 0,
        }
    }

    fn source_id(bytes: [u8; 32]) -> SourceContentId {
        SourceContentId::new(bytes).expect("nonzero Source identity")
    }

    fn capability_id(bytes: [u8; 32]) -> CapabilityContentId {
        CapabilityContentId::new(bytes).expect("nonzero capability identity")
    }

    fn add_record(
        test: &mut ProgramTest,
        schema: [u8; 32],
        data: Vec<u8>,
    ) -> (RecordPair, [u8; 32]) {
        let digest = hash(&data).to_bytes();
        let raw = Pubkey::find_program_address(
            &[RAW_RECORD_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_PROGRAM_ID,
        )
        .0;
        let staging = Pubkey::find_program_address(
            &[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest],
            &REGISTRY_PROGRAM_ID,
        )
        .0;
        test.add_account(raw, protocol_account(REGISTRY_PROGRAM_ID, data));
        (RecordPair { raw, staging }, digest)
    }

    fn program_identity(program: Pubkey) -> ProgramIdentityV1 {
        ProgramIdentityV1::new(program.to_bytes()).expect("nonzero Program identity")
    }

    fn artifact(program: Pubkey, semantic: [u8; 32], elf: &[u8]) -> ArtifactReleaseV1 {
        ArtifactReleaseV1::new(
            program_identity(program),
            program_identity(bpf_loader_upgradeable::ID),
            programdata(program).to_bytes(),
            ContentId::new(semantic).expect("semantic release"),
            hash(elf).to_bytes(),
            0,
            ArtifactUpgradePolicyV1::Immutable,
            None,
        )
        .expect("immutable artifact")
    }

    fn artifact_id(release: ArtifactReleaseV1) -> ArtifactReleaseIdV1 {
        ArtifactReleaseIdV1::new(hash(&release.to_bytes()).to_bytes()).expect("artifact release ID")
    }

    fn binding(release: ArtifactReleaseV1) -> ExecutionRoleBindingV1 {
        ExecutionRoleBindingV1::new(release.program(), artifact_id(release))
    }

    fn activation_input(release: ArtifactReleaseV1) -> ArtifactActivationInputV1 {
        ArtifactActivationInputV1::new(
            artifact_id(release),
            release,
            DeploymentObservationV1::new(
                release.program().to_bytes(),
                bpf_loader_upgradeable::ID.to_bytes(),
                true,
                release.programdata(),
                bpf_loader_upgradeable::ID.to_bytes(),
                false,
                release.programdata(),
                bpf_loader_upgradeable::ID.to_bytes(),
                release.deployment_slot(),
                release.elf_digest(),
                release.upgrade_authority(),
            )
            .expect("deployment observation"),
        )
    }

    fn activation(core: ArtifactReleaseV1, resolution: ArtifactReleaseV1) -> ([u8; 32], Vec<u8>) {
        let release_set = ExecutionReleaseSetV1::new(
            binding(core),
            binding(core),
            binding(core),
            binding(resolution),
            binding(core),
        )
        .expect("execution release set");
        let release_set_id = hash(&release_set.to_bytes()).to_bytes();
        let content = ContentId::new(release_set_id).expect("release set identity");
        let mut bytes = vec![0_u8; ACTIVATED_EXECUTION_RELEASE_SET_BYTES_V1];
        initialize_activation_cache_v1(&mut bytes, content).expect("activation cache");
        for (role, selected) in [
            (ExecutionRoleV1::Core, core),
            (ExecutionRoleV1::Claims, core),
            (ExecutionRoleV1::Trading, core),
            (ExecutionRoleV1::Resolution, resolution),
            (ExecutionRoleV1::Custody, core),
        ] {
            activate_execution_role_into_v1(
                &mut bytes,
                content,
                &release_set,
                role,
                &activation_input(selected),
            )
            .expect("activate role");
        }
        ActivatedExecutionReleaseSetV1::decode(&bytes).expect("complete activation");
        (release_set_id, bytes)
    }

    fn product_graph(test: &mut ProgramTest) -> ProductGraph {
        let product_id = ProductContentId::new([0x60; 32]).expect("Product identity");
        let coordinate_domain_id = [0x61; 32];
        let result_unit_id = [0x62; 32];
        let liability_basis_id = ProductContentId::new([0x63; 32]).expect("liability basis");
        let representation_release_id =
            ProductContentId::new([0x64; 32]).expect("representation release");
        let cuts: [i128; 0] = [];
        let mut domain_bytes =
            vec![0_u8; result_domain_record_bytes(cuts.len()).expect("domain width")];
        compile_result_domain_v2(
            ResultDomainInputV2 {
                product_id,
                coordinate_domain_id: ProductContentId::new(coordinate_domain_id)
                    .expect("coordinate domain"),
                result_unit_id: ProductContentId::new(result_unit_id).expect("result unit"),
                liability_basis_id,
                representation_release_id,
                mapping_release_id: ProductContentId::new([0x65; 32]).expect("mapping release"),
                cut_denominator: 1,
                cuts: &cuts,
            },
            &mut domain_bytes,
        )
        .expect("result domain");
        let (result_domain, domain_id) = add_record(test, RESULT_DOMAIN_SCHEMA_ID_V2, domain_bytes);
        let coefficients = [1_u64; OUTCOME_COUNT as usize];
        let mut portfolio_bytes =
            vec![0_u8; portfolio_record_bytes(coefficients.len()).expect("portfolio width")];
        compile_portfolio_v2(
            PortfolioInputV2 {
                product_id,
                result_domain_id: ProductContentId::new(domain_id).expect("domain ID"),
                claim_basis_id: ProductContentId::new([0x66; 32]).expect("claim basis"),
                liability_basis_id,
                representation_release_id,
                denominator: 1,
                coefficients: &coefficients,
            },
            &mut portfolio_bytes,
        )
        .expect("portfolio");
        let (portfolio, portfolio_id) = add_record(test, PORTFOLIO_SCHEMA_ID_V2, portfolio_bytes);
        let mut product_bytes = vec![0_u8; PRODUCT_RECORD_BYTES_V2];
        ProductRecordV2::new(
            product_id,
            ProductContentId::new(domain_id).expect("domain ID"),
            ProductContentId::new(portfolio_id).expect("portfolio ID"),
        )
        .encode_into(&mut product_bytes)
        .expect("Product root");
        let (product, product_record_digest) =
            add_record(test, PRODUCT_RECORD_SCHEMA_ID_V2, product_bytes);
        ProductGraph {
            product,
            product_record_digest,
            result_domain,
            portfolio,
            coordinate_domain_id,
            result_unit_id,
        }
    }

    fn funding_entry(config: [u8; 32], controller_release: [u8; 32]) -> CapabilityEntryV1 {
        let quote = FundingQuoteV1::new(
            FundingAmountsV1::new(
                CompartmentFundingV1::native_lamports(
                    Rent::default().minimum_balance(FUNDING_STATE_BYTES),
                )
                .expect("funding rent"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::native_lamports(BOUNTY).expect("failure bounty"),
                CompartmentFundingV1::not_applicable(),
                CompartmentFundingV1::not_applicable(),
            )
            .expect("funding amounts"),
            None,
        )
        .expect("funding quote");
        CapabilityEntryV1::new(
            capability_id(hashv(&[b"dclutch/sponsored/capability/", &config]).to_bytes()),
            capability_id(controller_release),
            capability_id(config),
            capability_id([0xb4; 32]),
            capability_id([0xb5; 32]),
            capability_id([0xb6; 32]),
            ActivationPolicy::RequiredAtFounding,
            0,
            0,
            [0; MAX_DEPENDENCIES_PER_CAPABILITY],
            quote,
        )
        .expect("funding entry")
    }

    fn add_active_funding_ledger(
        test: &mut ProgramTest,
        market: Pubkey,
        manifest: CapabilityManifestV1<'_>,
        entry_indices: [u16; 3],
    ) -> Pubkey {
        let manifest_id = capability_id(hash(manifest.as_bytes()).to_bytes());
        let mask = entry_indices
            .into_iter()
            .fold(0_u16, |current, index| current | (1_u16 << index));
        let width = funding_ledger_bytes_v2(3).expect("three-row ledger width");
        assert_eq!(width, 264, "exact three-row Resolution ledger width");
        let mut bytes = vec![0_u8; width];
        FundingLedgerV2::initialize(&mut bytes, manifest_id, manifest, mask)
            .expect("pending ledger");
        for index in entry_indices {
            FundingLedgerV2::activate_in_place(&mut bytes, manifest_id, manifest, index, 1)
                .expect("active ledger row");
        }
        let ledger = FundingLedgerV2::decode(&bytes).expect("funding ledger");
        let authenticated = ledger
            .authenticate(manifest_id, manifest)
            .expect("ledger auth");
        let principal = authenticated
            .remaining_native_lamports_total()
            .expect("bounded principal");
        let derivation = CapabilityFundingLedgerDerivationV2::new(
            PROGRAM_ID.to_bytes(),
            market.to_bytes(),
            GENERATION,
            manifest_id,
            ledger,
        )
        .expect("funding derivation");
        let key = Pubkey::find_program_address(&derivation.seed_components(), &PROGRAM_ID).0;
        test.add_account(
            key,
            Account {
                lamports: Rent::default()
                    .minimum_balance(width)
                    .checked_add(principal)
                    .expect("ledger custody"),
                data: bytes,
                owner: PROGRAM_ID,
                executable: false,
                rent_epoch: 0,
            },
        );
        key
    }

    fn base_fixture() -> BaseFixture {
        let elves = artifacts();
        let worker = Keypair::new();
        let mut test = ProgramTest::default();
        test.prefer_bpf(true);
        test.set_compute_max_units(1_400_000);
        add_program(&mut test, "dclutch_core_sbf", CORE_PROGRAM_ID);
        add_program(&mut test, "dclutch_resolution_proof_sbf", PROGRAM_ID);
        let core_release = artifact(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
        let resolution_release = artifact(
            PROGRAM_ID,
            RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            &elves.resolution,
        );
        let (release_set, activation_bytes) = activation(core_release, resolution_release);
        let activation = Pubkey::find_program_address(
            &[ACTIVATION_PDA_DOMAIN_V1, &release_set],
            &REGISTRY_PROGRAM_ID,
        )
        .0;
        test.add_account(
            activation,
            protocol_account(REGISTRY_PROGRAM_ID, activation_bytes),
        );
        let product = product_graph(&mut test);
        let (rent_beneficiary, _) = Pubkey::find_program_address(
            &[b"dclutch/test-rent-beneficiary", &[0xc7; 32]],
            &RENT_PROGRAM_ID,
        );
        test.add_account(
            rent_beneficiary,
            protocol_account(RENT_PROGRAM_ID, vec![0_u8; 128]),
        );
        test.add_account(
            worker.pubkey(),
            Account::new(1_000_000_000, 0, &system_program::ID),
        );
        BaseFixture {
            test: Some(test),
            worker,
            product,
            activation,
            rent_beneficiary,
        }
    }

    const RECEIVER_PROGRAM: Pubkey = Pubkey::new_from_array([0x91; 32]);
    const PUSH_PROGRAM: Pubkey = Pubkey::new_from_array([0x92; 32]);
    const RECEIVER_CONFIG_KEY: Pubkey = Pubkey::new_from_array([0x94; 32]);
    const PROVIDER_AUTHORITY: [u8; 32] = [0x95; 32];
    const PRICE_CODEC_ID: [u8; 32] = [0x96; 32];
    const FEED_ID: [u8; 32] = [0x93; 32];
    const RECEIVER_SLOT: u64 = 111;
    const PUSH_SLOT: u64 = 222;
    const SHARD: u16 = 0;
    const FIRST_PUBLISH: i64 = CREATED_UNIX;
    const SECOND_PUBLISH: i64 = CREATED_UNIX + 1;
    const FIRST_POSTED_SLOT: u64 = 900;
    const SECOND_POSTED_SLOT: u64 = 901;
    const FIRST_CAPTURE_SLOT: u64 = 1_000;
    const SECOND_CAPTURE_SLOT: u64 = 1_001;
    const SPONSORED_MAX_AGE: u32 = 100;
    const PRICE: i64 = 100_000_000;
    const CONFIDENCE: u64 = 10_000;
    const EXPONENT: i32 = -8;

    #[derive(Clone, Copy)]
    struct SponsoredGraph {
        material: RecordPair,
        material_id: [u8; 32],
        spec: RecordPair,
        provider: RecordPair,
        adapter: RecordPair,
        window: RecordPair,
        statistic: RecordPair,
        release_record: RecordPair,
        release_id: [u8; 32],
        release: PythSponsoredPushReleaseV1,
        window_value: WindowSpecV1,
    }

    #[derive(Clone, Copy)]
    struct SponsoredMarket {
        market: Pubkey,
        state: Pubkey,
        manifest: RecordPair,
        success_certificate: Pubkey,
        failure_certificate: Pubkey,
        receipt: Pubkey,
        funding: Pubkey,
        head: Pubkey,
        first_candidate: Pubkey,
        second_candidate: Pubkey,
    }

    struct SponsoredFixture {
        base: BaseFixture,
        graph: SponsoredGraph,
        receiver_programdata: Pubkey,
        push_programdata: Pubkey,
        receiver_programdata_account: Account,
        push_programdata_account: Account,
        price_account: Pubkey,
        first_update: [u8; FULL_PRICE_UPDATE_V2_LEN],
        second_update: [u8; FULL_PRICE_UPDATE_V2_LEN],
        success: SponsoredMarket,
        failure: SponsoredMarket,
        legacy_v5_failure: SponsoredMarket,
    }

    fn loader_programdata_account(slot: u64, elf: &[u8]) -> Account {
        let mut data = Vec::with_capacity(45 + elf.len());
        data.extend_from_slice(&3_u32.to_le_bytes());
        data.extend_from_slice(&slot.to_le_bytes());
        data.push(1);
        data.extend_from_slice(&PROVIDER_AUTHORITY);
        data.extend_from_slice(elf);
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: false,
            rent_epoch: 0,
        }
    }

    fn loader_program_account(programdata: Pubkey) -> Account {
        let mut data = vec![0_u8; 36];
        data[..4].copy_from_slice(&2_u32.to_le_bytes());
        data[4..].copy_from_slice(programdata.as_ref());
        Account {
            lamports: Rent::default().minimum_balance(data.len()),
            data,
            owner: bpf_loader_upgradeable::ID,
            executable: true,
            rent_epoch: 0,
        }
    }

    fn add_provider_program(test: &mut ProgramTest, program: Pubkey, elf: &[u8]) -> Pubkey {
        let programdata_key = programdata(program);
        test.add_genesis_account(program, loader_program_account(programdata_key));
        test.add_genesis_account(programdata_key, loader_programdata_account(0, elf));
        programdata_key
    }

    fn price_body(
        price_account: Pubkey,
        publish_time: i64,
        previous_publish_time: i64,
        posted_slot: u64,
    ) -> [u8; FULL_PRICE_UPDATE_V2_LEN] {
        let mut bytes = [0_u8; FULL_PRICE_UPDATE_V2_LEN];
        bytes[..8].copy_from_slice(&PRICE_UPDATE_V2_DISCRIMINATOR);
        bytes[8..40].copy_from_slice(price_account.as_ref());
        bytes[40] = 1;
        bytes[41..73].copy_from_slice(&FEED_ID);
        bytes[73..81].copy_from_slice(&PRICE.to_le_bytes());
        bytes[81..89].copy_from_slice(&CONFIDENCE.to_le_bytes());
        bytes[89..93].copy_from_slice(&EXPONENT.to_le_bytes());
        bytes[93..101].copy_from_slice(&publish_time.to_le_bytes());
        bytes[101..109].copy_from_slice(&previous_publish_time.to_le_bytes());
        bytes[109..117].copy_from_slice(&PRICE.to_le_bytes());
        bytes[117..125].copy_from_slice(&CONFIDENCE.to_le_bytes());
        bytes[125..133].copy_from_slice(&posted_slot.to_le_bytes());
        bytes
    }

    fn add_sponsored_graph(
        test: &mut ProgramTest,
        product: &ProductGraph,
        price_account: Pubkey,
        price_bump: u8,
        receiver_programdata: Pubkey,
        push_programdata: Pubkey,
        receiver_elf: &[u8],
        push_elf: &[u8],
        receiver_config: &[u8; RECEIVER_CONFIG_V2_LEN],
    ) -> SponsoredGraph {
        let release = PythSponsoredPushReleaseV1::new(PythSponsoredPushReleaseV1Input {
            cluster_id: DEVNET_CLUSTER_ID_V1,
            receiver_program: RECEIVER_PROGRAM.to_bytes(),
            receiver_programdata: receiver_programdata.to_bytes(),
            receiver_abi_id: hash(receiver_elf).to_bytes(),
            receiver_upgrade_authority: PROVIDER_AUTHORITY,
            push_oracle_program: PUSH_PROGRAM.to_bytes(),
            push_oracle_programdata: push_programdata.to_bytes(),
            push_oracle_abi_id: hash(push_elf).to_bytes(),
            push_oracle_upgrade_authority: PROVIDER_AUTHORITY,
            receiver_config: RECEIVER_CONFIG_KEY.to_bytes(),
            receiver_config_digest: hash(receiver_config).to_bytes(),
            price_account: price_account.to_bytes(),
            feed_id: FEED_ID,
            price_update_codec_id: PRICE_CODEC_ID,
            adapter_id: PYTH_SPONSORED_PUSH_ADAPTER_ID_V1,
            provider_family_id: PYTH_SPONSORED_PUSH_PROVIDER_FAMILY_ID_V1,
            transport_profile_id: PYTH_SPONSORED_PUSH_TRANSPORT_PROFILE_ID_V1,
            receiver_deployment_slot: RECEIVER_SLOT,
            push_oracle_deployment_slot: PUSH_SLOT,
            shard: SHARD,
            feed_account_bump: price_bump,
            activation_time: 0,
        })
        .expect("synthetic sponsored release");
        let (release_record, release_id) = add_record(
            test,
            dclutch_pyth_svm::PYTH_SPONSORED_PUSH_RELEASE_SCHEMA_ID_V1,
            release.to_bytes().to_vec(),
        );
        let provider_value = ProviderReleaseV1::new(
            source_id(PYTH_SPONSORED_PUSH_PROVIDER_FAMILY_ID_V1),
            source_id(PYTH_SPONSORED_PUSH_ADAPTER_ID_V1),
            source_id(release_id),
            source_id(PRICE_CODEC_ID),
            source_id(PYTH_SPONSORED_PUSH_TRANSPORT_PROFILE_ID_V1),
        );
        let (provider, provider_id) = add_record(
            test,
            PROVIDER_RELEASE_SCHEMA_ID_V1,
            provider_value.to_bytes().to_vec(),
        );
        let adapter_value =
            PythAdapterConfigV1::new(FEED_ID, EXPONENT, 50).expect("0.5% confidence bound");
        let (adapter, adapter_id) = add_record(
            test,
            PYTH_ADAPTER_CONFIG_SCHEMA_ID_V1,
            adapter_value.to_bytes().to_vec(),
        );
        let capacity = SourceCapacityProfileV1::new(
            SourceCapacityEnvelope::Measured,
            1,
            0,
            source_id([0xa8; 32]),
            source_id([0xa9; 32]),
            u32::try_from(SPONSORED_PUSH_CANDIDATE_BYTES_V1).expect("candidate width"),
            0,
        )
        .expect("sponsored Source capacity");
        let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
        let unit = source_id(product.result_unit_id);
        let spec_value = SourceSpecV1::new(
            source_id(product.coordinate_domain_id),
            unit,
            source_id(provider_id),
            SourceAccessProfile::PythSponsoredPushSnapshot,
            source_id(adapter_id),
            capacity_id,
        );
        assert_eq!(
            spec_value.access_profile().provider_extension_release_id(),
            PYTH_SPONSORED_PUSH_PROVIDER_EXTENSION_RELEASE_ID_V1
        );
        let (spec, spec_id) = add_record(
            test,
            dclutch_source_contract::SOURCE_SPEC_SCHEMA_ID_V1,
            spec_value.to_bytes().to_vec(),
        );
        let window_value = WindowSpecV1::new(
            source_id(spec_id),
            WindowKind::Terminal,
            FIRST_PUBLISH - 10,
            SECOND_PUBLISH,
            SPONSORED_MAX_AGE,
            5,
            source_id([0xaa; 32]),
        )
        .expect("sponsored terminal window");
        let (window, window_id) = add_record(
            test,
            dclutch_source_contract::WINDOW_SPEC_SCHEMA_ID_V1,
            window_value.to_bytes().to_vec(),
        );
        let statistic_value = StatisticSpecV1::new(
            unit,
            unit,
            StatisticKind::TerminalSample,
            RoundingBoundary::ExactRational,
            1,
            0,
            capacity_id,
            source_id([0xab; 32]),
            capacity,
        )
        .expect("sponsored terminal statistic");
        let (statistic, statistic_id) = add_record(
            test,
            STATISTIC_SPEC_SCHEMA_ID_V1,
            statistic_value.to_bytes().to_vec(),
        );
        let material_value = SourceMaterialV3::explicitly_unbounded(
            source_id(product.product_record_digest),
            source_id(spec_id),
            source_id(window_id),
            source_id(statistic_id),
            None,
            source_id(SOURCE_FAILURE_POLICY_RELEASE_ID_V2),
        );
        material_value
            .validate_source_graph(
                source_id(spec_id),
                spec_value,
                source_id(window_id),
                window_value,
                source_id(statistic_id),
                statistic_value,
                None,
                source_id(SOURCE_FAILURE_POLICY_RELEASE_ID_V2),
            )
            .expect("sponsored Source graph");
        let (material, material_id) = add_record(
            test,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            material_value.to_bytes().to_vec(),
        );
        SponsoredGraph {
            material,
            material_id,
            spec,
            provider,
            adapter,
            window,
            statistic,
            release_record,
            release_id,
            release,
            window_value,
        }
    }

    fn add_sponsored_manifest(
        test: &mut ProgramTest,
        material_id: [u8; 32],
        controller_release: [u8; 32],
    ) -> (RecordPair, Vec<u8>) {
        let mut entries = [
            funding_entry([0xb1; 32], controller_release),
            funding_entry([0xb2; 32], controller_release),
            funding_entry(material_id, controller_release),
        ];
        entries.sort_unstable_by_key(|entry| entry.kind_id().to_bytes());
        let mut bytes = vec![0_u8; MANIFEST_HEADER_BYTES + 3 * CAPABILITY_ENTRY_BYTES];
        CapabilityManifestV1::encode_into(&entries, &mut bytes).expect("sponsored manifest");
        let (record, _) = add_record(
            test,
            CAPABILITY_MANIFEST_SCHEMA_RELEASE_ID_V1,
            bytes.clone(),
        );
        (record, bytes)
    }

    #[allow(clippy::too_many_arguments)]
    fn add_sponsored_market(
        test: &mut ProgramTest,
        tag: u8,
        graph: SponsoredGraph,
        product: &ProductGraph,
        manifest_record: RecordPair,
        manifest: CapabilityManifestV1<'_>,
        selected_release_set: [u8; 32],
        rent_beneficiary: Pubkey,
        first_update: &[u8; FULL_PRICE_UPDATE_V2_LEN],
        second_update: &[u8; FULL_PRICE_UPDATE_V2_LEN],
    ) -> SponsoredMarket {
        let manifest_id = hash(manifest.as_bytes()).to_bytes();
        let mut identity = MarketIdentity {
            market_id: CoreIdentity::new([tag; 32]).expect("placeholder Market"),
            realm_id: CoreIdentity::new([tag.wrapping_add(1); 32]).expect("Realm"),
            product_record: CoreIdentity::new(product.product_record_digest)
                .expect("Product record"),
            product_id: CoreIdentity::new([33; 32]).expect("Product"),
            resolution_policy: CoreIdentity::new(graph.material_id).expect("Source material"),
            capability_manifest: CoreIdentity::new(manifest_id).expect("manifest"),
            selected_release_set: CoreIdentity::new(selected_release_set).expect("release set"),
            registry_program: CoreIdentity::new(REGISTRY_PROGRAM_ID.to_bytes()).expect("Registry"),
            generation: GENERATION,
        };
        let market = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(identity).as_slices(),
            &CORE_PROGRAM_ID,
        )
        .0;
        identity.market_id = CoreIdentity::new(market.to_bytes()).expect("Market");
        let state = CoreState {
            phase: Phase::Open,
            readiness: Readiness::Consumed,
            terminal_winner: 0,
            identity,
            outstanding_capabilities: 0,
            principal_cap_sets: u64::MAX,
            rent_beneficiary: CoreIdentity::new(rent_beneficiary.to_bytes())
                .expect("rent beneficiary"),
            terminal_receipt: None,
            bumps: StateBumpsV1::UNRECORDED,
        };
        test.add_account(
            market,
            protocol_account(
                CORE_PROGRAM_ID,
                state.encode().expect("Core state").to_vec(),
            ),
        );
        let (source_state, source_bump) = Pubkey::find_program_address(
            &[
                dclutch_source_contract::SOURCE_RESOLUTION_STATE_PDA_DOMAIN_V2,
                market.as_ref(),
                &GENERATION.to_le_bytes(),
            ],
            &PROGRAM_ID,
        );
        let fresh = SourceResolutionStateV2::fresh(
            market.to_bytes(),
            GENERATION,
            source_id(graph.material_id),
            rent_beneficiary.to_bytes(),
            source_bump,
            0,
            0,
        )
        .expect("fresh sponsored Source")
        .state();
        test.add_account(
            source_state,
            protocol_account(PROGRAM_ID, fresh.to_bytes().to_vec()),
        );
        let certificate_of = |kind: u8| {
            Pubkey::find_program_address(
                &[
                    RESOLUTION_CERTIFICATE_PDA_DOMAIN_V3,
                    source_state.as_ref(),
                    &[kind],
                    &TERMINAL_SEQUENCE.to_le_bytes(),
                ],
                &PROGRAM_ID,
            )
            .0
        };
        let success_certificate = certificate_of(RESOLUTION_SUCCESS_KIND);
        let failure_certificate = certificate_of(RESOLUTION_FAILURE_KIND);
        for certificate in [success_certificate, failure_certificate] {
            test.add_account(
                certificate,
                Account::new(
                    Rent::default().minimum_balance(RESOLUTION_CERTIFICATE_BYTES_V2),
                    0,
                    &system_program::ID,
                ),
            );
        }
        let receipt = Pubkey::find_program_address(
            &[
                SPONSORED_PUSH_RECEIPT_PDA_DOMAIN_V1,
                source_state.as_ref(),
                &TERMINAL_SEQUENCE.to_le_bytes(),
            ],
            &PROGRAM_ID,
        )
        .0;
        let head = Pubkey::find_program_address(
            &[
                SPONSORED_PUSH_HEAD_PDA_DOMAIN_V1,
                market.as_ref(),
                &GENERATION.to_le_bytes(),
                &graph.release_id,
            ],
            &PROGRAM_ID,
        )
        .0;
        let candidate_of = |update: &[u8; FULL_PRICE_UPDATE_V2_LEN]| {
            let parsed = dclutch_pyth_svm::FullPriceUpdateV2::parse(update).expect("price update");
            Pubkey::find_program_address(
                &[
                    SPONSORED_PUSH_CANDIDATE_PDA_DOMAIN_V1,
                    market.as_ref(),
                    &GENERATION.to_le_bytes(),
                    &graph.release_id,
                    &graph.release.price_account(),
                    &parsed.publish_time().to_le_bytes(),
                    &parsed.posted_slot().to_le_bytes(),
                    &hash(update).to_bytes(),
                ],
                &PROGRAM_ID,
            )
            .0
        };
        let entry_indices: Vec<u16> = (0..manifest.entry_count()).collect();
        let funding = add_active_funding_ledger(
            test,
            market,
            manifest,
            entry_indices.try_into().expect("three manifest rows"),
        );
        let _ = manifest_record;
        SponsoredMarket {
            market,
            state: source_state,
            manifest: manifest_record,
            success_certificate,
            failure_certificate,
            receipt,
            funding,
            head,
            first_candidate: candidate_of(first_update),
            second_candidate: candidate_of(second_update),
        }
    }

    impl SponsoredFixture {
        fn new() -> Self {
            let mut base = base_fixture();
            let test = base.test.as_mut().expect("ProgramTest");
            // Register valid provider executables at genesis slot zero so the
            // ProgramTest cache treats them like ordinary programs. The exact
            // nonzero release metadata is installed after the Bank starts;
            // Capture authenticates these accounts but never invokes them.
            let provider_elves = artifacts();
            let receiver_elf = provider_elves.core;
            let push_elf = provider_elves.resolution;
            let receiver_programdata = add_provider_program(test, RECEIVER_PROGRAM, &receiver_elf);
            let push_programdata = add_provider_program(test, PUSH_PROGRAM, &push_elf);
            let receiver_programdata_account =
                loader_programdata_account(RECEIVER_SLOT, &receiver_elf);
            let push_programdata_account = loader_programdata_account(PUSH_SLOT, &push_elf);
            let shard = SHARD.to_le_bytes();
            let (price_account, price_bump) =
                Pubkey::find_program_address(&[&shard, &FEED_ID], &PUSH_PROGRAM);
            let first_update = price_body(
                price_account,
                FIRST_PUBLISH,
                FIRST_PUBLISH,
                FIRST_POSTED_SLOT,
            );
            let second_update = price_body(
                price_account,
                SECOND_PUBLISH,
                FIRST_PUBLISH,
                SECOND_POSTED_SLOT,
            );
            let receiver_config = [0x5c_u8; RECEIVER_CONFIG_V2_LEN];
            test.add_account(
                RECEIVER_CONFIG_KEY,
                Account {
                    lamports: Rent::default().minimum_balance(receiver_config.len()),
                    data: receiver_config.to_vec(),
                    owner: RECEIVER_PROGRAM,
                    executable: false,
                    rent_epoch: 0,
                },
            );
            test.add_account(
                price_account,
                Account {
                    lamports: Rent::default().minimum_balance(first_update.len()),
                    data: first_update.to_vec(),
                    owner: RECEIVER_PROGRAM,
                    executable: false,
                    rent_epoch: 0,
                },
            );
            let graph = add_sponsored_graph(
                test,
                &base.product,
                price_account,
                price_bump,
                receiver_programdata,
                push_programdata,
                &receiver_elf,
                &push_elf,
                &receiver_config,
            );
            let (manifest, manifest_bytes) = add_sponsored_manifest(
                test,
                graph.material_id,
                RESOLUTION_CONTROLLER_RELEASE_ID_V7,
            );
            let manifest_view =
                CapabilityManifestV1::decode(&manifest_bytes).expect("sponsored manifest view");
            let (legacy_v5_manifest, legacy_v5_manifest_bytes) = add_sponsored_manifest(
                test,
                graph.material_id,
                dclutch_resolution_codec::RESOLUTION_CONTROLLER_RELEASE_ID_V5,
            );
            let legacy_v5_manifest_view = CapabilityManifestV1::decode(&legacy_v5_manifest_bytes)
                .expect("legacy V5 sponsored manifest view");
            let elves = artifacts();
            let core_release = artifact(CORE_PROGRAM_ID, [0x41; 32], &elves.core);
            let resolution_release = artifact(
                PROGRAM_ID,
                RESOLUTION_CONTROLLER_RELEASE_ID_V7,
                &elves.resolution,
            );
            let (selected_release_set, _) = activation(core_release, resolution_release);
            let success = add_sponsored_market(
                test,
                0xc1,
                graph,
                &base.product,
                manifest,
                manifest_view,
                selected_release_set,
                base.rent_beneficiary,
                &first_update,
                &second_update,
            );
            let failure = add_sponsored_market(
                test,
                0xc2,
                graph,
                &base.product,
                manifest,
                manifest_view,
                selected_release_set,
                base.rent_beneficiary,
                &first_update,
                &second_update,
            );
            let legacy_v5_failure = add_sponsored_market(
                test,
                0xc3,
                graph,
                &base.product,
                legacy_v5_manifest,
                legacy_v5_manifest_view,
                selected_release_set,
                base.rent_beneficiary,
                &first_update,
                &second_update,
            );
            Self {
                base,
                graph,
                receiver_programdata,
                push_programdata,
                receiver_programdata_account,
                push_programdata_account,
                price_account,
                first_update,
                second_update,
                success,
                failure,
                legacy_v5_failure,
            }
        }

        fn record_accounts(&self) -> [Pubkey; 14] {
            [
                self.graph.material.raw,
                self.graph.material.staging,
                self.graph.spec.raw,
                self.graph.spec.staging,
                self.graph.provider.raw,
                self.graph.provider.staging,
                self.graph.adapter.raw,
                self.graph.adapter.staging,
                self.graph.window.raw,
                self.graph.window.staging,
                self.graph.statistic.raw,
                self.graph.statistic.staging,
                self.graph.release_record.raw,
                self.graph.release_record.staging,
            ]
        }

        fn capture_instruction(&self, market: SponsoredMarket, candidate: Pubkey) -> Instruction {
            let records = self.record_accounts();
            let mut accounts = vec![
                AccountMeta::new(self.base.worker.pubkey(), true),
                AccountMeta::new_readonly(market.market, false),
                AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
                AccountMeta::new_readonly(self.base.activation, false),
                AccountMeta::new(market.head, false),
                AccountMeta::new(candidate, false),
                AccountMeta::new_readonly(market.state, false),
            ];
            accounts.extend(
                records
                    .into_iter()
                    .map(|key| AccountMeta::new_readonly(key, false)),
            );
            accounts.extend([
                AccountMeta::new_readonly(self.price_account, false),
                AccountMeta::new_readonly(RECEIVER_PROGRAM, false),
                AccountMeta::new_readonly(self.receiver_programdata, false),
                AccountMeta::new_readonly(PUSH_PROGRAM, false),
                AccountMeta::new_readonly(self.push_programdata, false),
                AccountMeta::new_readonly(RECEIVER_CONFIG_KEY, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ]);
            Instruction {
                program_id: PROGRAM_ID,
                accounts,
                data: SponsoredPushInstructionV1 {
                    action: SponsoredPushActionV1::Capture,
                    generation: GENERATION,
                    terminal_sequence: 0,
                }
                .to_bytes()
                .expect("capture instruction")
                .to_vec(),
            }
        }

        fn settle_instruction(&self, market: SponsoredMarket, candidate: Pubkey) -> Instruction {
            let records = self.record_accounts();
            let mut accounts = vec![
                AccountMeta::new(self.base.worker.pubkey(), true),
                AccountMeta::new_readonly(market.market, false),
                AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
                AccountMeta::new_readonly(self.base.activation, false),
                AccountMeta::new_readonly(market.head, false),
                AccountMeta::new_readonly(candidate, false),
                AccountMeta::new(market.state, false),
                AccountMeta::new(market.success_certificate, false),
                AccountMeta::new(market.receipt, false),
            ];
            accounts.extend(
                records
                    .into_iter()
                    .map(|key| AccountMeta::new_readonly(key, false)),
            );
            accounts.extend([
                AccountMeta::new_readonly(self.base.product.product.raw, false),
                AccountMeta::new_readonly(self.base.product.product.staging, false),
                AccountMeta::new_readonly(self.base.product.result_domain.raw, false),
                AccountMeta::new_readonly(self.base.product.result_domain.staging, false),
                AccountMeta::new_readonly(self.base.product.portfolio.raw, false),
                AccountMeta::new_readonly(self.base.product.portfolio.staging, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ]);
            Instruction {
                program_id: PROGRAM_ID,
                accounts,
                data: SponsoredPushInstructionV1 {
                    action: SponsoredPushActionV1::Settle,
                    generation: GENERATION,
                    terminal_sequence: TERMINAL_SEQUENCE,
                }
                .to_bytes()
                .expect("settle instruction")
                .to_vec(),
            }
        }

        fn failure_instruction(&self, market: SponsoredMarket) -> Instruction {
            let accounts = vec![
                AccountMeta::new(self.base.worker.pubkey(), true),
                AccountMeta::new_readonly(market.market, false),
                AccountMeta::new_readonly(CORE_PROGRAM_ID, false),
                AccountMeta::new_readonly(self.base.activation, false),
                AccountMeta::new(market.state, false),
                AccountMeta::new(market.failure_certificate, false),
                AccountMeta::new_readonly(self.graph.material.raw, false),
                AccountMeta::new_readonly(self.graph.material.staging, false),
                AccountMeta::new_readonly(self.graph.window.raw, false),
                AccountMeta::new_readonly(self.graph.window.staging, false),
                AccountMeta::new_readonly(self.base.product.product.raw, false),
                AccountMeta::new_readonly(self.base.product.product.staging, false),
                AccountMeta::new_readonly(self.base.product.result_domain.raw, false),
                AccountMeta::new_readonly(self.base.product.result_domain.staging, false),
                AccountMeta::new_readonly(self.base.product.portfolio.raw, false),
                AccountMeta::new_readonly(self.base.product.portfolio.staging, false),
                AccountMeta::new_readonly(market.manifest.raw, false),
                AccountMeta::new_readonly(market.manifest.staging, false),
                AccountMeta::new(market.funding, false),
                AccountMeta::new_readonly(sysvar::clock::ID, false),
                AccountMeta::new_readonly(sysvar::rent::ID, false),
                AccountMeta::new_readonly(system_program::ID, false),
                AccountMeta::new_readonly(market.head, false),
                AccountMeta::new_readonly(self.graph.spec.raw, false),
                AccountMeta::new_readonly(self.graph.spec.staging, false),
                AccountMeta::new_readonly(self.graph.provider.raw, false),
                AccountMeta::new_readonly(self.graph.provider.staging, false),
                AccountMeta::new_readonly(self.graph.release_record.raw, false),
                AccountMeta::new_readonly(self.graph.release_record.staging, false),
            ];
            Instruction {
                program_id: PROGRAM_ID,
                accounts,
                data: SponsoredPushInstructionV1 {
                    action: SponsoredPushActionV1::CommitFailure,
                    generation: GENERATION,
                    terminal_sequence: TERMINAL_SEQUENCE,
                }
                .to_bytes()
                .expect("failure instruction")
                .to_vec(),
            }
        }

        fn close_candidate_instruction(
            &self,
            market: SponsoredMarket,
            candidate: Pubkey,
        ) -> Instruction {
            Instruction {
                program_id: PROGRAM_ID,
                accounts: vec![
                    AccountMeta::new_readonly(market.market, false),
                    AccountMeta::new_readonly(market.state, false),
                    AccountMeta::new(candidate, false),
                    AccountMeta::new(self.base.worker.pubkey(), false),
                ],
                data: SponsoredPushInstructionV1 {
                    action: SponsoredPushActionV1::CloseCandidate,
                    generation: GENERATION,
                    terminal_sequence: 0,
                }
                .to_bytes()
                .expect("close candidate instruction")
                .to_vec(),
            }
        }

        fn close_head_instruction(&self, market: SponsoredMarket) -> Instruction {
            Instruction {
                program_id: PROGRAM_ID,
                accounts: vec![
                    AccountMeta::new_readonly(market.market, false),
                    AccountMeta::new_readonly(market.state, false),
                    AccountMeta::new(market.head, false),
                    AccountMeta::new(self.base.worker.pubkey(), false),
                ],
                data: SponsoredPushInstructionV1 {
                    action: SponsoredPushActionV1::CloseHead,
                    generation: GENERATION,
                    terminal_sequence: 0,
                }
                .to_bytes()
                .expect("close head instruction")
                .to_vec(),
            }
        }
    }

    fn set_sponsored_clock(context: &mut ProgramTestContext, slot: u64, unix_timestamp: i64) {
        context.set_sysvar(&Clock {
            slot,
            epoch_start_timestamp: 0,
            epoch: 0,
            leader_schedule_epoch: 0,
            unix_timestamp,
        });
    }

    fn set_account(context: &mut ProgramTestContext, key: Pubkey, account: Account) {
        context.set_account(&key, &AccountSharedData::from(account));
    }

    fn update_account(body: &[u8; FULL_PRICE_UPDATE_V2_LEN], owner: Pubkey) -> Account {
        Account {
            lamports: Rent::default().minimum_balance(body.len()),
            data: body.to_vec(),
            owner,
            executable: false,
            rent_epoch: 0,
        }
    }

    async fn optional_account(context: &mut ProgramTestContext, key: Pubkey) -> Option<Account> {
        context
            .banks_client
            .get_account(key)
            .await
            .expect("bank read")
    }

    async fn observed(context: &mut ProgramTestContext, key: Pubkey) -> Account {
        optional_account(context, key)
            .await
            .expect("account exists")
    }

    async fn submit_with_cu(
        context: &mut ProgramTestContext,
        instruction: Instruction,
        signers: &[&Keypair],
        label: &str,
    ) -> Result<u64, BanksClientError> {
        let blockhash = context
            .banks_client
            .get_latest_blockhash()
            .await
            .expect("blockhash");
        let mut all = vec![&context.payer];
        all.extend_from_slice(signers);
        let transaction = Transaction::new_signed_with_payer(
            &[instruction],
            Some(&context.payer.pubkey()),
            &all,
            blockhash,
        );
        let processed = context
            .banks_client
            .process_transaction_with_metadata(transaction)
            .await?;
        let units = processed
            .metadata
            .as_ref()
            .map_or(0, |metadata| metadata.compute_units_consumed);
        eprintln!("sponsored-push CU {label}: {units}");
        processed
            .result
            .map(|()| units)
            .map_err(BanksClientError::TransactionError)
    }

    fn assert_custom(error: BanksClientError, expected: ResolutionError, label: &str) {
        assert!(
            matches!(
                error,
                BanksClientError::TransactionError(TransactionError::InstructionError(
                    0,
                    InstructionError::Custom(code)
                )) if code == expected as u32
            ),
            "{label}: {error:?}"
        );
    }

    #[tokio::test]
    async fn sponsored_push_executes_success_failure_cleanup_and_atomic_hostiles() {
        let mut fixture = SponsoredFixture::new();
        let mut context = fixture
            .base
            .test
            .take()
            .expect("ProgramTest")
            .start_with_context()
            .await;
        context
            .warp_to_slot(PUSH_SLOT + 10)
            .expect("provider bootstrap programs are effective");
        for (label, program_id) in [
            ("fixture/warm-receiver", RECEIVER_PROGRAM),
            ("fixture/warm-push", PUSH_PROGRAM),
        ] {
            let _ = submit_with_cu(
                &mut context,
                Instruction {
                    program_id,
                    accounts: Vec::new(),
                    data: Vec::new(),
                },
                &[],
                label,
            )
            .await;
        }
        set_account(
            &mut context,
            fixture.receiver_programdata,
            fixture.receiver_programdata_account.clone(),
        );
        set_account(
            &mut context,
            fixture.push_programdata,
            fixture.push_programdata_account.clone(),
        );
        let worker = &fixture.base.worker;
        let success = fixture.success;
        let failure = fixture.failure;
        let deadline = fixture
            .graph
            .window_value
            .end_unix_seconds()
            .checked_add(i64::from(fixture.graph.window_value.max_age_seconds()))
            .expect("checked sponsored deadline");

        set_sponsored_clock(&mut context, FIRST_CAPTURE_SLOT, FIRST_PUBLISH + 1);

        // Exact account ownership is independent of body validity and also
        // refuses before any sponsored state is allocated.
        set_account(
            &mut context,
            fixture.price_account,
            update_account(&fixture.first_update, system_program::ID),
        );
        let error = submit_with_cu(
            &mut context,
            fixture.capture_instruction(success, success.first_candidate),
            &[worker],
            "Capture/owner-refusal",
        )
        .await
        .expect_err("wrong upstream owner must refuse");
        assert_custom(error, ResolutionError::ProviderRelease, "price owner");
        assert!(optional_account(&mut context, success.head).await.is_none());
        set_account(
            &mut context,
            fixture.price_account,
            update_account(&fixture.first_update, RECEIVER_PROGRAM),
        );

        let capture_one_cu = submit_with_cu(
            &mut context,
            fixture.capture_instruction(success, success.first_candidate),
            &[worker],
            "Capture/first",
        )
        .await
        .expect("first capture");
        assert!(capture_one_cu > 0);
        let first_candidate = observed(&mut context, success.first_candidate).await;
        let first_candidate_value = SponsoredPushCandidateV1::decode(&first_candidate.data)
            .expect("immutable first candidate");
        assert_eq!(first_candidate_value.update_bytes, fixture.first_update);
        assert_eq!(
            first_candidate_value.refund_recipient,
            worker.pubkey().to_bytes()
        );
        let first_head = observed(&mut context, success.head).await;
        let first_head_value = SponsoredPushHeadV1::decode(&first_head.data).expect("first head");
        assert_eq!(
            first_head_value.best_candidate,
            success.first_candidate.to_bytes()
        );
        assert_eq!(first_head_value.prior_head_digest, [0; 32]);

        // Cleanup cannot erase a submitted answer while Source is Primary.
        let error = submit_with_cu(
            &mut context,
            fixture.close_candidate_instruction(success, success.first_candidate),
            &[],
            "CloseCandidate/primary-refusal",
        )
        .await
        .expect_err("primary candidate cleanup must refuse");
        assert_custom(error, ResolutionError::Transition, "primary cleanup");
        assert_eq!(
            observed(&mut context, success.first_candidate).await,
            first_candidate
        );

        set_account(
            &mut context,
            fixture.price_account,
            update_account(&fixture.second_update, RECEIVER_PROGRAM),
        );
        set_sponsored_clock(&mut context, SECOND_CAPTURE_SLOT, SECOND_PUBLISH + 1);
        let capture_two_cu = submit_with_cu(
            &mut context,
            fixture.capture_instruction(success, success.second_candidate),
            &[worker],
            "Capture/head-advance",
        )
        .await
        .expect("second capture");
        assert!(capture_two_cu > 0);
        let second_head = observed(&mut context, success.head).await;
        let second_head_value =
            SponsoredPushHeadV1::decode(&second_head.data).expect("advanced head");
        assert_eq!(
            second_head_value.best_candidate,
            success.second_candidate.to_bytes()
        );
        assert_eq!(
            second_head_value.prior_head_digest,
            hash(&first_head.data).to_bytes()
        );

        let source_prestate = observed(&mut context, success.state).await;
        let certificate_prestate = observed(&mut context, success.success_certificate).await;
        set_sponsored_clock(&mut context, SECOND_CAPTURE_SLOT + 1, deadline);
        let error = submit_with_cu(
            &mut context,
            fixture.settle_instruction(success, success.second_candidate),
            &[worker],
            "Settle/deadline-boundary-refusal",
        )
        .await
        .expect_err("settlement at the closed deadline must refuse");
        assert_custom(error, ResolutionError::ProviderFreshness, "settle boundary");
        assert_eq!(observed(&mut context, success.state).await, source_prestate);
        assert_eq!(
            observed(&mut context, success.success_certificate).await,
            certificate_prestate
        );
        assert!(
            optional_account(&mut context, success.receipt)
                .await
                .is_none()
        );

        set_sponsored_clock(&mut context, SECOND_CAPTURE_SLOT + 2, deadline + 1);
        let error = submit_with_cu(
            &mut context,
            fixture.settle_instruction(success, success.first_candidate),
            &[worker],
            "Settle/non-head-refusal",
        )
        .await
        .expect_err("resolver cannot choose an older candidate");
        assert_custom(error, ResolutionError::SponsoredPush, "non-head candidate");
        assert_eq!(observed(&mut context, success.state).await, source_prestate);

        let error = submit_with_cu(
            &mut context,
            fixture.failure_instruction(success),
            &[worker],
            "CommitFailure/nonvacant-head-refusal",
        )
        .await
        .expect_err("submitted head prevents failure");
        assert_custom(error, ResolutionError::SponsoredPush, "nonvacant head");
        assert_eq!(observed(&mut context, success.state).await, source_prestate);

        // The upstream account has advanced, and the snapshot is now older
        // than max_age. Settlement still uses the sealed capture Clock and does
        // not re-age or re-read the mutable upstream account.
        set_sponsored_clock(&mut context, SECOND_CAPTURE_SLOT + 200, deadline + 1_000);
        let settle_cu = submit_with_cu(
            &mut context,
            fixture.settle_instruction(success, success.second_candidate),
            &[worker],
            "Settle/best-sealed",
        )
        .await
        .expect("post-deadline best-candidate settlement");
        assert!(settle_cu > 0);
        let resolved =
            SourceResolutionStateV2::decode(&observed(&mut context, success.state).await.data)
                .expect("resolved Source");
        assert_eq!(resolved.phase(), SourceResolutionPhaseV1::Resolved);
        let certificate = ResolutionCertificateV2::decode(
            &observed(&mut context, success.success_certificate)
                .await
                .data,
        )
        .expect("success certificate");
        assert_eq!(
            certificate.kind,
            ResolutionCertificateKindV2::ResolutionSuccess
        );
        assert_eq!(certificate.route, fixture.graph.release_id);
        let receipt =
            SponsoredPushReceiptV1::decode(&observed(&mut context, success.receipt).await.data)
                .expect("durable sponsored receipt");
        assert_eq!(receipt.candidate, success.second_candidate.to_bytes());
        assert_eq!(receipt.publish_time, SECOND_PUBLISH);
        assert_eq!(receipt.posted_slot, SECOND_POSTED_SLOT);
        assert_eq!(receipt.consumed_slot, SECOND_CAPTURE_SLOT + 200);
        assert_eq!(receipt.certificate, success.success_certificate.to_bytes());

        let error = submit_with_cu(
            &mut context,
            fixture.settle_instruction(success, success.second_candidate),
            &[worker],
            "Settle/replay-refusal",
        )
        .await
        .expect_err("terminal Source cannot settle twice");
        assert_custom(error, ResolutionError::Transition, "settlement replay");

        let close_first_cu = submit_with_cu(
            &mut context,
            fixture.close_candidate_instruction(success, success.first_candidate),
            &[],
            "CloseCandidate/first",
        )
        .await
        .expect("close first terminal candidate");
        let close_second_cu = submit_with_cu(
            &mut context,
            fixture.close_candidate_instruction(success, success.second_candidate),
            &[],
            "CloseCandidate/second",
        )
        .await
        .expect("close selected terminal candidate");
        let close_head_cu = submit_with_cu(
            &mut context,
            fixture.close_head_instruction(success),
            &[],
            "CloseHead/terminal",
        )
        .await
        .expect("close terminal head");
        assert!(close_first_cu > 0 && close_second_cu > 0 && close_head_cu > 0);
        assert!(
            optional_account(&mut context, success.first_candidate)
                .await
                .is_none()
        );
        assert!(optional_account(&mut context, success.head).await.is_none());

        // The independent failure market never receives a candidate. Late
        // admission refuses, exact-deadline failure refuses atomically, and
        // one second later the canonical funded failure pays its worker.
        set_sponsored_clock(&mut context, SECOND_CAPTURE_SLOT + 300, deadline + 1);
        let error = submit_with_cu(
            &mut context,
            fixture.capture_instruction(failure, failure.second_candidate),
            &[worker],
            "Capture/late-refusal",
        )
        .await
        .expect_err("late candidate admission must refuse");
        assert_custom(error, ResolutionError::ProviderFreshness, "late admission");
        assert!(optional_account(&mut context, failure.head).await.is_none());

        // The active release is V6. A byte-canonical legacy V5 subset ledger
        // remains a hostile even when every address, row status, balance, and
        // Source coordinate is otherwise valid.
        let legacy = fixture.legacy_v5_failure;
        let legacy_source_prestate = observed(&mut context, legacy.state).await;
        let legacy_funding_prestate = observed(&mut context, legacy.funding).await;
        let error = submit_with_cu(
            &mut context,
            fixture.failure_instruction(legacy),
            &[worker],
            "CommitFailure/V5-ledger-refusal",
        )
        .await
        .expect_err("legacy V5 funding ledger must refuse under Resolution V6");
        assert_custom(error, ResolutionError::Funding, "legacy V5 funding");
        assert_eq!(
            observed(&mut context, legacy.state).await,
            legacy_source_prestate
        );
        assert_eq!(
            observed(&mut context, legacy.funding).await,
            legacy_funding_prestate
        );

        let failure_source_prestate = observed(&mut context, failure.state).await;
        let failure_funding_prestate = observed(&mut context, failure.funding).await;
        set_sponsored_clock(&mut context, SECOND_CAPTURE_SLOT + 301, deadline);
        let error = submit_with_cu(
            &mut context,
            fixture.failure_instruction(failure),
            &[worker],
            "CommitFailure/deadline-boundary-refusal",
        )
        .await
        .expect_err("failure at deadline must refuse");
        assert_custom(error, ResolutionError::Transition, "failure boundary");
        assert_eq!(
            observed(&mut context, failure.state).await,
            failure_source_prestate
        );
        assert_eq!(
            observed(&mut context, failure.funding).await,
            failure_funding_prestate
        );
        let worker_before = observed(&mut context, worker.pubkey()).await.lamports;
        set_sponsored_clock(&mut context, SECOND_CAPTURE_SLOT + 302, deadline + 1);
        let failure_cu = submit_with_cu(
            &mut context,
            fixture.failure_instruction(failure),
            &[worker],
            "CommitFailure/head-vacant",
        )
        .await
        .expect("head-vacant funded failure");
        assert!(failure_cu > 0);
        let failed =
            SourceResolutionStateV2::decode(&observed(&mut context, failure.state).await.data)
                .expect("failed Source");
        assert_eq!(failed.phase(), SourceResolutionPhaseV1::FailureCommitted);
        let failure_certificate = ResolutionCertificateV2::decode(
            &observed(&mut context, failure.failure_certificate)
                .await
                .data,
        )
        .expect("failure certificate");
        assert_eq!(
            failure_certificate.kind,
            ResolutionCertificateKindV2::ResolutionFailure
        );
        assert_eq!(failure_certificate.route, [0; 32]);
        assert_eq!(failure_certificate.work_paid, BOUNTY);
        assert_eq!(
            observed(&mut context, worker.pubkey()).await.lamports,
            worker_before + BOUNTY
        );
        assert!(optional_account(&mut context, failure.head).await.is_none());

        eprintln!(
            "sponsored-push successful action CU: Capture=[{capture_one_cu},{capture_two_cu}] Settle={settle_cu} CommitFailure={failure_cu} CloseCandidate=[{close_first_cu},{close_second_cu}] CloseHead={close_head_cu}"
        );
    }
}
