use dclutch_capability_contract::{
    ActivationPolicy, CAPABILITY_ENTRY_BYTES, CapabilityEntryV1, CapabilityManifestV1,
    CompartmentFundingV1, ContentId as CapabilityContentId, FundingAmountsV1,
    FundingCustodyObservationV1, FundingQuoteV1, MANIFEST_HEADER_BYTES, RealmCollateralBindingV1,
};
use dclutch_collateral_contract::{CreateRealmV1, FoundMarketAndFundV1};
use dclutch_product_contract::{
    ContentId as ProductContentId,
    capacity::{CapacityEnvelope, CapacityProfileId, CapacityProfileV1Input},
    claim::CategoricalUnitV1Input,
    product::InstanceV1Input,
    result_domain::FiniteResultDomainV1,
};
use dclutch_pyth_contract::funding::construct_required_resolution_funding;
use dclutch_rent_contract::{RENT_CREDIT_PDA_DOMAIN_V1, RefundAuthority, RentCreditV1};
use dclutch_source_contract::{
    CapacityEnvelope as SourceCapacityEnvelope, PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
    ProviderReleaseV1, PythAdapterConfigV1, ResolutionPolicyV1, RoundingBoundary,
    SOURCE_MATERIAL_BYTES, SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialInputV1,
    SourceSpecV1, StatisticKind, StatisticSpecV1, WindowKind, WindowSpecV1,
    encode_source_material_into_v1,
};
use solana_program::sysvar::SysvarSerialize;

use super::*;

fn observation() -> Observation {
    Observation {
        slot: 444,
        unix_timestamp: 1_800_000_000,
        finality: Finality::Finalized,
    }
}

fn native_resolution_quote(rent: u64, provider: u64, bounty: u64) -> FundingQuoteV1 {
    let native = |amount| CompartmentFundingV1::native_lamports(amount).expect("native amount");
    let not_applicable = CompartmentFundingV1::not_applicable();
    FundingQuoteV1::new(
        FundingAmountsV1::new(
            native(rent),
            not_applicable,
            not_applicable,
            if provider == 0 {
                not_applicable
            } else {
                native(provider)
            },
            if bounty == 0 {
                not_applicable
            } else {
                native(bounty)
            },
            not_applicable,
            not_applicable,
        )
        .expect("typed native resolution amounts"),
        None,
    )
    .expect("typed native resolution quote")
}

#[test]
fn resolution_sponsor_debit_refuses_realm_collateral_substitution() {
    let native = |amount| CompartmentFundingV1::native_lamports(amount).expect("native amount");
    let realm = CompartmentFundingV1::realm_collateral(1).expect("realm amount");
    let not_applicable = CompartmentFundingV1::not_applicable();
    let binding = RealmCollateralBindingV1::new(
        CapabilityContentId::new([1; 32]).expect("realm ID"),
        CapabilityContentId::new([2; 32]).expect("release ID"),
        [3; 32],
        [4; 32],
        [5; 32],
    )
    .expect("binding");
    let quote = FundingQuoteV1::new(
        FundingAmountsV1::new(
            native(100),
            not_applicable,
            not_applicable,
            not_applicable,
            native(1),
            realm,
            not_applicable,
        )
        .expect("typed amounts"),
        Some(binding),
    )
    .expect("typed quote");
    assert!(matches!(
        resolution_native_funding(quote),
        Err(FoundationError::InvalidFundingAuthority)
    ));
}

fn observed(
    key: Pubkey,
    owner: Pubkey,
    lamports: u64,
    data: Vec<u8>,
    executable: bool,
) -> ObservedAccount {
    ObservedAccount {
        observation: observation(),
        key,
        owner,
        lamports,
        executable,
        data,
    }
}

fn finalized_record(
    program_id: Pubkey,
    schema: [u8; 32],
    data: Vec<u8>,
) -> (ObservedAccount, FinalizedRecordProof) {
    let digest = hash(&data).to_bytes();
    let (raw, _) = Pubkey::find_program_address(
        &[RAW_RECORD_PDA_SEED_V1, schema.as_slice(), digest.as_slice()],
        &program_id,
    );
    let (cursor, _) = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            schema.as_slice(),
            digest.as_slice(),
        ],
        &program_id,
    );
    (
        observed(raw, program_id, u64::MAX, data, false),
        FinalizedRecordProof {
            schema_release_id: schema,
            staging_cursor: observed(cursor, system_program::ID, 0, Vec::new(), false),
        },
    )
}

fn replace_finalized_record(
    program_id: Pubkey,
    record: &mut ObservedAccount,
    proof: &mut FinalizedRecordProof,
    data: Vec<u8>,
) {
    let digest = hash(&data).to_bytes();
    record.key = Pubkey::find_program_address(
        &[
            RAW_RECORD_PDA_SEED_V1,
            proof.schema_release_id.as_slice(),
            digest.as_slice(),
        ],
        &program_id,
    )
    .0;
    proof.staging_cursor.key = Pubkey::find_program_address(
        &[
            STAGING_CURSOR_PDA_SEED_V1,
            proof.schema_release_id.as_slice(),
            digest.as_slice(),
        ],
        &program_id,
    )
    .0;
    record.data = data;
}

fn rent_account() -> ObservedAccount {
    let rent = Rent::default();
    let mut data = vec![0u8; Rent::size_of()];
    let mut lamports = 1;
    let mut info = AccountInfo::new(
        &sysvar::rent::ID,
        false,
        false,
        &mut lamports,
        &mut data,
        &sysvar::ID,
        false,
    );
    rent.to_account_info(&mut info).expect("serialize Rent");
    drop(info);
    observed(sysvar::rent::ID, sysvar::ID, 1, data, false)
}

fn system_program_account() -> ObservedAccount {
    observed(system_program::ID, native_loader::ID, 1, Vec::new(), true)
}

fn mint_data(outcome_authorities: bool) -> Vec<u8> {
    let mut data = vec![0u8; dclutch_token_svm::MINT_BYTES];
    if outcome_authorities {
        data.get_mut(0..4)
            .expect("Mint authority tag")
            .copy_from_slice(&1u32.to_le_bytes());
        data.get_mut(4..36)
            .expect("Mint authority")
            .copy_from_slice(&[31; 32]);
        data.get_mut(46..50)
            .expect("freeze authority tag")
            .copy_from_slice(&1u32.to_le_bytes());
        data.get_mut(50..82)
            .expect("freeze authority")
            .copy_from_slice(&[32; 32]);
    }
    *data.get_mut(44).expect("Mint decimals") = 6;
    *data.get_mut(45).expect("Mint initialized flag") = 1;
    data
}

fn expected_realm(program_id: Pubkey, mint: Pubkey, token_program: Pubkey) -> (RealmV1, Pubkey) {
    let release = select_token_release(token_program).expect("production release");
    let realm = RealmV1::new(RealmV1Input {
        token_program: token_program.to_bytes(),
        collateral_mint: mint.to_bytes(),
        collateral_adapter_release_id: hash(&release.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
        freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
    })
    .expect("Realm");
    let digest = hash(&realm.to_bytes()).to_bytes();
    let (key, _) = Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &digest], &program_id);
    (realm, key)
}

fn create_realm_state() -> (Pubkey, CreateRealmState) {
    let program_id = Pubkey::new_from_array([90; 32]);
    let sponsor = Pubkey::new_from_array([91; 32]);
    let mint = Pubkey::new_from_array([92; 32]);
    let token_program = Pubkey::new_from_array(dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID);
    let (_, realm_key) = expected_realm(program_id, mint, token_program);
    (
        program_id,
        CreateRealmState {
            sponsor: observed(sponsor, system_program::ID, u64::MAX, Vec::new(), false),
            realm_destination: ObservedVacancy {
                key: realm_key,
                observation: observation(),
            },
            collateral_mint: observed(mint, token_program, u64::MAX, mint_data(false), false),
            token_program: observed(
                token_program,
                bpf_loader_upgradeable::ID,
                u64::MAX,
                Vec::new(),
                true,
            ),
            system_program: system_program_account(),
            rent_sysvar: rent_account(),
        },
    )
}

fn product_id(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("nonzero Product id")
}

fn capacity(max_partition_cells: u32) -> CapacityProfileV1 {
    CapacityProfileV1::new(CapacityProfileV1Input {
        envelope: CapacityEnvelope::Measured,
        verifier_release_id: product_id([1; 32]),
        envelope_basis_id: product_id([2; 32]),
        max_artifact_bytes: 256,
        page_payload_bytes: 64,
        max_pages: 4,
        max_partition_cells,
    })
    .expect("capacity")
}

fn source_id(bytes: [u8; 32]) -> dclutch_source_contract::ContentId {
    dclutch_source_contract::ContentId::new(bytes).expect("nonzero Source id")
}

fn source_material(
    instance: InstanceV1,
    instance_id: [u8; 32],
    domain: FiniteResultDomainV1,
) -> Vec<u8> {
    let capacity = SourceCapacityProfileV1::new(
        SourceCapacityEnvelope::Measured,
        1,
        0,
        source_id([37; 32]),
        source_id([38; 32]),
        512,
        1,
    )
    .expect("source capacity");
    let capacity_id = source_id(hash(&capacity.to_bytes()).to_bytes());
    let provider = ProviderReleaseV1::new(
        source_id([31; 32]),
        source_id(PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1),
        source_id([32; 32]),
        source_id([33; 32]),
        source_id([34; 32]),
    );
    let provider_id = source_id(hash(&provider.to_bytes()).to_bytes());
    let adapter = PythAdapterConfigV1::new([35; 32], -8, 10_000).expect("Pyth config");
    let adapter_id = source_id(hash(&adapter.to_bytes()).to_bytes());
    let source = SourceSpecV1::new(
        source_id(domain.coordinate_domain_id().to_bytes()),
        source_id(domain.result_unit_id().to_bytes()),
        provider_id,
        SourceAccessProfile::PythTerminalOneTransaction,
        adapter_id,
        capacity_id,
    );
    let primary_source_id = source_id(hash(&source.to_bytes()).to_bytes());
    let window = WindowSpecV1::new(
        primary_source_id,
        WindowKind::Terminal,
        1_800_000_010,
        1_800_000_010,
        10,
        2,
        source_id([36; 32]),
    )
    .expect("terminal window");
    let window_id = source_id(hash(&window.to_bytes()).to_bytes());
    let statistic = StatisticSpecV1::new(
        source_id(domain.result_unit_id().to_bytes()),
        source_id(domain.result_unit_id().to_bytes()),
        StatisticKind::TerminalSample,
        RoundingBoundary::ExactRational,
        1,
        0,
        capacity_id,
        source_id([39; 32]),
        capacity,
    )
    .expect("terminal statistic");
    let statistic_id = source_id(hash(&statistic.to_bytes()).to_bytes());
    let domain_bytes = domain.to_bytes();
    let domain_id = source_id(
        hashv(&[
            FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
            &[0],
            domain_bytes.as_slice(),
        ])
        .to_bytes(),
    );
    let policy = ResolutionPolicyV1::new(
        capacity_id,
        source_id(instance_id),
        primary_source_id,
        window_id,
        statistic_id,
        domain_id,
        None,
    );
    let mut material = vec![0; SOURCE_MATERIAL_BYTES];
    encode_source_material_into_v1(
        &mut material,
        SourceMaterialInputV1 {
            policy: &policy,
            capacity_profile_id: capacity_id,
            capacity_profile: &capacity,
            primary_source_id,
            primary_source: &source,
            primary_provider_release_id: provider_id,
            primary_provider_release: &provider,
            primary_adapter_config: &adapter,
            window_id,
            window: &window,
            statistic_id,
            statistic: &statistic,
            product_instance_id: source_id(instance_id),
            product_instance: &instance,
            result_domain: &domain,
            recovery: None,
        },
    )
    .expect("canonical Source material");
    material
}

fn resolution_manifest(
    release_id: [u8; 32],
    config_id: [u8; 32],
    capacity_id: [u8; 32],
    funding_quote: FundingQuoteV1,
) -> Vec<u8> {
    let funding_entry = CapabilityEntryV1::new(
        CapabilityContentId::new([20; 32]).expect("kind"),
        CapabilityContentId::new(release_id).expect("release"),
        CapabilityContentId::new(config_id).expect("config"),
        CapabilityContentId::new(capacity_id).expect("capacity"),
        CapabilityContentId::new([21; 32]).expect("child schema"),
        CapabilityContentId::new([22; 32]).expect("child derivation"),
        ActivationPolicy::RequiredAtFounding,
        0,
        0,
        [0; dclutch_capability_contract::MAX_DEPENDENCIES_PER_CAPABILITY],
        funding_quote,
    )
    .expect("resolution capability entry");
    let mut manifest_data = vec![0u8; MANIFEST_HEADER_BYTES + CAPABILITY_ENTRY_BYTES];
    CapabilityManifestV1::encode_into(&[funding_entry], &mut manifest_data).expect("manifest");
    manifest_data
}

struct FoundFixture {
    program_id: Pubkey,
    state: FoundMarketState,
    identity: MarketIdentity,
}

impl FoundFixture {
    fn new(outcome_count: u8, max_partition_cells: u32) -> Self {
        let program_id = Pubkey::new_from_array([90; 32]);
        let sponsor = Pubkey::new_from_array([91; 32]);
        let mint = Pubkey::new_from_array([92; 32]);
        let token_program = Pubkey::new_from_array(dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID);
        let (realm, _) = expected_realm(program_id, mint, token_program);
        let realm_data = realm.to_bytes().to_vec();
        let (realm, realm_finalization) = finalized_record(
            program_id,
            hash(REALM_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
            realm_data.clone(),
        );

        let capacity = capacity(max_partition_cells);
        let capacity_data = capacity.to_bytes().to_vec();
        let capacity_id_bytes = hash(&capacity_data).to_bytes();
        let capacity_id = CapacityProfileId::new(product_id(capacity_id_bytes));
        let claim = CategoricalUnitV1::new(
            CategoricalUnitV1Input {
                capacity_profile_id: capacity_id,
                outcome_count: u32::from(outcome_count),
            },
            capacity,
        )
        .expect("claim");
        let claim_data = claim.to_bytes().to_vec();
        let claim_id_bytes = hash(&claim_data).to_bytes();
        let cuts = (0..outcome_count.saturating_sub(2))
            .map(i128::from)
            .collect::<Vec<_>>();
        let result_domain =
            FiniteResultDomainV1::new(product_id([41; 32]), product_id([42; 32]), 1, &cuts)
                .expect("finite result domain");
        let result_domain_bytes = result_domain.to_bytes();
        let result_domain_id = product_id(
            hashv(&[
                FINITE_RESULT_DOMAIN_CONTENT_DOMAIN_V1,
                &[0],
                result_domain_bytes.as_slice(),
            ])
            .to_bytes(),
        );
        let instance = InstanceV1::new(InstanceV1Input {
            terms_id: product_id([3; 32]),
            occurrence_id: product_id([4; 32]),
            claim_basis_id: product_id(claim_id_bytes),
            result_domain_id,
            capacity_profile_id: capacity_id,
            partition_cell_count: u32::from(outcome_count),
        })
        .expect("instance");
        let instance_data = instance.to_bytes().to_vec();
        let (product_instance, product_instance_finalization) = finalized_record(
            program_id,
            hash(PRODUCT_INSTANCE_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
            instance_data.clone(),
        );
        let (claim_basis, claim_basis_finalization) = finalized_record(
            program_id,
            hash(CATEGORICAL_CLAIM_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
            claim_data,
        );
        let (capacity_profile, capacity_profile_finalization) = finalized_record(
            program_id,
            hash(PRODUCT_CAPACITY_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
            capacity_data,
        );

        let material = source_material(instance, hash(&instance_data).to_bytes(), result_domain);
        let material_data = material;
        let (resolution_material, resolution_material_finalization) = finalized_record(
            program_id,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1,
            material_data,
        );
        let material_id_bytes = hash(&resolution_material.data).to_bytes();
        let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);
        let funding_quote = native_resolution_quote(fund_rent, 17, 23);
        let manifest_data = resolution_manifest(
            PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
            material_id_bytes,
            [23; 32],
            funding_quote,
        );
        let (capability_manifest, capability_manifest_finalization) = finalized_record(
            program_id,
            hash(CAPABILITY_MANIFEST_SCHEMA_RELEASE_PREIMAGE_V1).to_bytes(),
            manifest_data.clone(),
        );
        let identity = MarketIdentity::new(
            core_id(hash(&realm_data).to_bytes()).expect("realm ID"),
            core_id(hash(&instance_data).to_bytes()).expect("instance ID"),
            core_id(claim_id_bytes).expect("claim ID"),
            core_id(material_id_bytes).expect("SourceMaterial ID"),
            core_id(hash(&manifest_data).to_bytes()).expect("manifest ID"),
            FOUNDATION_GENERATION,
        );
        let identity_id = hash(&identity.to_bytes()).to_bytes();
        let (market_key, _) =
            Pubkey::find_program_address(&[MARKET_SEED, &identity_id], &program_id);
        let manifest = CapabilityManifestV1::decode(&manifest_data).expect("manifest decode");
        let manifest_id =
            CapabilityContentId::new(hash(manifest.as_bytes()).to_bytes()).expect("manifest ID");
        let selected = manifest
            .required_founding_entry_for_config(
                CapabilityContentId::new(material_id_bytes).expect("SourceMaterial"),
            )
            .expect("selected");
        let funding = construct_required_resolution_funding(
            manifest_id,
            manifest,
            selected,
            fund_rent,
            observation().slot,
        )
        .expect("funding");
        let derivation = dclutch_capability_contract::CapabilityFundingDerivationV1::new(
            market_key.to_bytes(),
            FOUNDATION_GENERATION,
            manifest_id,
            manifest,
            funding,
        )
        .expect("derivation");
        let (fund_key, _) =
            Pubkey::find_program_address(&derivation.seed_components(), &program_id);
        let authority = RefundAuthority::new(sponsor.to_bytes()).expect("authority");
        let (rent_credit_key, rent_credit_bump) = Pubkey::find_program_address(
            &[RENT_CREDIT_PDA_DOMAIN_V1, sponsor.as_ref()],
            &program_id,
        );
        Self {
            program_id,
            state: FoundMarketState {
                sponsor: observed(sponsor, system_program::ID, u64::MAX, Vec::new(), false),
                market_destination: ObservedVacancy {
                    key: market_key,
                    observation: observation(),
                },
                fund_destination: ObservedVacancy {
                    key: fund_key,
                    observation: observation(),
                },
                rent_credit: observed(
                    rent_credit_key,
                    program_id,
                    u64::MAX,
                    RentCreditV1::new(authority, rent_credit_bump)
                        .to_bytes()
                        .to_vec(),
                    false,
                ),
                realm,
                realm_finalization,
                product_instance,
                product_instance_finalization,
                claim_basis,
                claim_basis_finalization,
                capacity_profile,
                capacity_profile_finalization,
                resolution_material,
                resolution_material_finalization,
                capability_manifest,
                capability_manifest_finalization,
                system_program: system_program_account(),
                rent_sysvar: rent_account(),
            },
            identity,
        }
    }
}

fn creation_input(fixture: &FoundFixture) -> ReleaseBoundCreationInputV1 {
    ReleaseBoundCreationInputV1 {
        program_id: fixture.program_id,
        sponsor: fixture.state.sponsor.key,
        realm: RealmV1::decode(&fixture.state.realm.data).expect("Realm"),
        product_capacity_profile: CapacityProfileV1::decode(&fixture.state.capacity_profile.data)
            .expect("Product capacity"),
        claim_basis: CategoricalUnitV1::decode(&fixture.state.claim_basis.data)
            .expect("claim basis"),
        product_instance: InstanceV1::decode(&fixture.state.product_instance.data)
            .expect("Product Instance"),
        source_material: fixture.state.resolution_material.data.clone(),
        capability_manifest: fixture.state.capability_manifest.data.clone(),
        rent: Rent::default(),
        current_slot: observation().slot,
    }
}

#[test]
fn release_bound_creation_compiles_exact_found_admission_and_honest_gaps() {
    let fixture = FoundFixture::new(4, 16);
    let input = creation_input(&fixture);
    let plan = compile_release_bound_creation_v1(&input).expect("creation plan");
    let found = build_found_market_and_fund_v1(fixture.program_id, &fixture.state)
        .expect("finalized Found plan");
    assert_eq!(plan.identity, fixture.identity);
    assert_eq!(plan.identity, found.identity);
    assert_eq!(plan.market_address, found.market_address);
    assert_eq!(plan.fund_address, found.fund_address);
    assert_eq!(plan.rent_credit_address, fixture.state.rent_credit.key);
    assert_eq!(plan.outcome_count, 4);
    assert_eq!(plan.resolution_funding, found.resolution_funding);
    assert_eq!(plan.debit, found.debit);
    assert_eq!(plan.records.len(), 6);
    for (obligation, (observed, proof)) in plan.records.iter().zip([
        (&fixture.state.realm, &fixture.state.realm_finalization),
        (
            &fixture.state.product_instance,
            &fixture.state.product_instance_finalization,
        ),
        (
            &fixture.state.claim_basis,
            &fixture.state.claim_basis_finalization,
        ),
        (
            &fixture.state.capacity_profile,
            &fixture.state.capacity_profile_finalization,
        ),
        (
            &fixture.state.resolution_material,
            &fixture.state.resolution_material_finalization,
        ),
        (
            &fixture.state.capability_manifest,
            &fixture.state.capability_manifest_finalization,
        ),
    ]) {
        assert_eq!(obligation.content, observed.data);
        assert_eq!(obligation.content_id, hash(&observed.data).to_bytes());
        assert_eq!(obligation.raw_record, observed.key);
        assert_eq!(obligation.schema_release_id, proof.schema_release_id);
        assert_eq!(obligation.staging_cursor, proof.staging_cursor.key);
    }
    assert_eq!(
        plan.direct_stages,
        vec![
            CreationStageReportV1 {
                stage: CreationStageV1::CompileCanonicalArtifacts,
                status: CreationStageStatusV1::Complete,
            },
            CreationStageReportV1 {
                stage: CreationStageV1::CreateRealm,
                status: CreationStageStatusV1::FinalizedObservationRequired,
            },
            CreationStageReportV1 {
                stage: CreationStageV1::CreateRentCredit,
                status: CreationStageStatusV1::FinalizedObservationRequired,
            },
            CreationStageReportV1 {
                stage: CreationStageV1::PublishImmutableRecords,
                status: CreationStageStatusV1::BuilderUnavailable(
                    CreationBuilderGapV1::ImmutableRecordPublication,
                ),
            },
            CreationStageReportV1 {
                stage: CreationStageV1::FoundMarketAndFund,
                status: CreationStageStatusV1::FinalizedObservationRequired,
            },
            CreationStageReportV1 {
                stage: CreationStageV1::OpenCollateralVault,
                status: CreationStageStatusV1::FinalizedObservationRequired,
            },
        ]
    );
    assert_eq!(
        plan.series_stages,
        vec![
            CreationStageReportV1 {
                stage: CreationStageV1::CompileCanonicalArtifacts,
                status: CreationStageStatusV1::Complete,
            },
            CreationStageReportV1 {
                stage: CreationStageV1::CreateRealm,
                status: CreationStageStatusV1::FinalizedObservationRequired,
            },
            CreationStageReportV1 {
                stage: CreationStageV1::CreateRentCredit,
                status: CreationStageStatusV1::FinalizedObservationRequired,
            },
            CreationStageReportV1 {
                stage: CreationStageV1::PublishImmutableRecords,
                status: CreationStageStatusV1::BuilderUnavailable(
                    CreationBuilderGapV1::ImmutableRecordPublication,
                ),
            },
            CreationStageReportV1 {
                stage: CreationStageV1::CreateSeries,
                status: CreationStageStatusV1::BuilderUnavailable(
                    CreationBuilderGapV1::SeriesCreate,
                ),
            },
            CreationStageReportV1 {
                stage: CreationStageV1::InstantiateSeriesOccurrence,
                status: CreationStageStatusV1::FinalizedObservationRequired,
            },
            CreationStageReportV1 {
                stage: CreationStageV1::ConsumeSeriesTicketAndFound,
                status: CreationStageStatusV1::BuilderUnavailable(
                    CreationBuilderGapV1::SeriesConsumeAndFound,
                ),
            },
            CreationStageReportV1 {
                stage: CreationStageV1::OpenCollateralVault,
                status: CreationStageStatusV1::FinalizedObservationRequired,
            },
        ]
    );
}

#[test]
fn terminal_pyth_user_inputs_compile_the_canonical_product_and_source_records() {
    let fixture = FoundFixture::new(5, 16);
    let expected = creation_input(&fixture);
    let material =
        SourceMaterialViewV1::decode(&expected.source_material).expect("canonical SourceMaterial");
    let (_, source_capacity_profile) = material.capacity_profile().expect("Source capacity");
    let (_, provider_release) = material
        .primary_provider_release()
        .expect("provider release");
    let window = material.window().expect("Source window");
    let statistic = material.statistic().expect("Source statistic");
    let input = TerminalPythCreationInputV1 {
        program_id: fixture.program_id,
        sponsor: fixture.state.sponsor.key,
        realm: expected.realm,
        product_capacity_profile: expected.product_capacity_profile,
        terms_id: expected.product_instance.terms_id(),
        occurrence_id: expected.product_instance.occurrence_id(),
        result_domain: material.result_domain().expect("result domain"),
        source_capacity_profile,
        provider_release,
        pyth_adapter_config: material
            .primary_adapter_config()
            .expect("Pyth adapter config"),
        target_unix_seconds: window.start_unix_seconds(),
        max_age_seconds: window.max_age_seconds(),
        max_future_skew_seconds: window.max_future_skew_seconds(),
        schedule_id: window.schedule_id(),
        evaluator_release_id: statistic.evaluator_release_id(),
        capability_manifest: expected.capability_manifest,
        rent: expected.rent,
        current_slot: expected.current_slot,
    };
    let compiled = compile_terminal_pyth_creation_v1(&input).expect("terminal Pyth plan");
    assert_eq!(compiled.claim_basis, expected.claim_basis);
    assert_eq!(compiled.product_instance, expected.product_instance);
    assert_eq!(compiled.source_material, expected.source_material);
    assert_eq!(compiled.found.identity, fixture.identity);
    assert_eq!(
        compiled
            .found
            .records
            .get(4)
            .map(|record| record.schema_release_id),
        Some(SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V1)
    );

    let mut unsupported_provider = input.clone();
    unsupported_provider.provider_release = ProviderReleaseV1::new(
        source_id([31; 32]),
        source_id([94; 32]),
        source_id([32; 32]),
        source_id([33; 32]),
        source_id([34; 32]),
    );
    assert_eq!(
        compile_terminal_pyth_creation_v1(&unsupported_provider),
        Err(FoundationError::ContentLinkMismatch)
    );

    let mut invalid_window = input;
    invalid_window.max_age_seconds = 0;
    assert_eq!(
        compile_terminal_pyth_creation_v1(&invalid_window),
        Err(FoundationError::InvalidRecord)
    );
}

#[test]
fn creation_preflight_refuses_product_source_and_manifest_substitution() {
    let fixture = FoundFixture::new(3, 16);
    let input = creation_input(&fixture);

    let mut wrong_product = input.clone();
    wrong_product.product_instance = InstanceV1::new(InstanceV1Input {
        terms_id: input.product_instance.terms_id(),
        occurrence_id: input.product_instance.occurrence_id(),
        claim_basis_id: input.product_instance.claim_basis_id(),
        result_domain_id: product_id([99; 32]),
        capacity_profile_id: input.product_instance.capacity_profile_id(),
        partition_cell_count: input.product_instance.partition_cell_count(),
    })
    .expect("structurally valid hostile Product");
    assert_eq!(
        compile_release_bound_creation_v1(&wrong_product),
        Err(FoundationError::ContentLinkMismatch)
    );

    let mut wrong_source = input.clone();
    let other_instance = InstanceV1::new(InstanceV1Input {
        terms_id: input.product_instance.terms_id(),
        occurrence_id: product_id([98; 32]),
        claim_basis_id: input.product_instance.claim_basis_id(),
        result_domain_id: input.product_instance.result_domain_id(),
        capacity_profile_id: input.product_instance.capacity_profile_id(),
        partition_cell_count: input.product_instance.partition_cell_count(),
    })
    .expect("other Product");
    let domain = SourceMaterialViewV1::decode(&input.source_material)
        .expect("Source material")
        .result_domain()
        .expect("result domain");
    wrong_source.source_material = source_material(
        other_instance,
        hash(&other_instance.to_bytes()).to_bytes(),
        domain,
    );
    assert_eq!(
        compile_release_bound_creation_v1(&wrong_source),
        Err(FoundationError::ContentLinkMismatch)
    );

    let mut wrong_manifest = input;
    let fund_rent = wrong_manifest.rent.minimum_balance(FUNDING_BYTES);
    wrong_manifest.capability_manifest = resolution_manifest(
        PYTH_PROVIDER_EXTENSION_RELEASE_ID_V1,
        [97; 32],
        [23; 32],
        native_resolution_quote(fund_rent, 17, 23),
    );
    assert_eq!(
        compile_release_bound_creation_v1(&wrong_manifest),
        Err(FoundationError::InvalidFundingAuthority)
    );
}

#[test]
fn create_realm_is_derived_and_reports_exact_frame_and_rent() {
    let (program_id, state) = create_realm_state();
    let report = build_create_realm_v1(program_id, &state, RealmAuthorityPolicy::STRICT)
        .expect("derived Realm plan");
    assert_eq!(report.observation, observation());
    assert_eq!(
        report.instruction.accounts.len(),
        CREATE_REALM_ACCOUNT_COUNT
    );
    assert_eq!(
        report.instruction.accounts,
        vec![
            AccountMeta::new(state.sponsor.key, true),
            AccountMeta::new(report.realm_address, false),
            AccountMeta::new_readonly(state.collateral_mint.key, false),
            AccountMeta::new_readonly(state.token_program.key, false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ]
    );
    let wire = CreateRealmV1::decode(&report.instruction.data).expect("exact wire");
    assert_eq!(wire.realm(), report.realm);
    assert_eq!(
        hash(&report.realm.to_bytes()).to_bytes(),
        report.realm_content_id
    );
    assert_eq!(report.debit.sponsor, state.sponsor.key);
    assert_eq!(report.debit.total_sponsor_debit, report.debit.realm_rent);
    assert_eq!(
        report.debit.realm_rent,
        Rent::default().minimum_balance(REALM_BYTES)
    );
}

#[test]
fn present_mint_authorities_are_reported_not_hidden() {
    let (program_id, mut state) = create_realm_state();
    state.collateral_mint.data = mint_data(true);
    let release = select_token_release(state.token_program.key).expect("release");
    let realm = RealmV1::new(RealmV1Input {
        token_program: state.token_program.key.to_bytes(),
        collateral_mint: state.collateral_mint.key.to_bytes(),
        collateral_adapter_release_id: hash(&release.to_bytes()).to_bytes(),
        mint_authority_policy: MintAuthorityPolicy::AdmitIssuerControl,
        freeze_authority_policy: FreezeAuthorityPolicy::AdmitIssuerControl,
    })
    .expect("issuer-controlled Realm");
    let digest = hash(&realm.to_bytes()).to_bytes();
    state.realm_destination.key =
        Pubkey::find_program_address(&[REALM_PDA_DOMAIN, &digest], &program_id).0;
    assert_eq!(
        build_create_realm_v1(program_id, &state, RealmAuthorityPolicy::STRICT),
        Err(FoundationError::IssuerAuthorityConsentRequired)
    );
    let explicit_consent = RealmAuthorityPolicy {
        mint_authority: MintAuthorityPolicy::AdmitIssuerControl,
        freeze_authority: FreezeAuthorityPolicy::AdmitIssuerControl,
    };
    let report = build_create_realm_v1(program_id, &state, explicit_consent)
        .expect("affirmative issuer authority consent");
    assert_eq!(
        report.realm.mint_authority_policy(),
        MintAuthorityPolicy::AdmitIssuerControl
    );
    assert_eq!(
        report.realm.freeze_authority_policy(),
        FreezeAuthorityPolicy::AdmitIssuerControl
    );
    assert_eq!(report.authority.selected_policy, explicit_consent);
    assert_eq!(report.authority.observed_mint_authority, Some([31; 32]));
    assert_eq!(report.authority.observed_freeze_authority, Some([32; 32]));
}

#[test]
fn found_market_rebuilds_identity_pdas_wire_privileges_and_debit() {
    let fixture = FoundFixture::new(2, 16);
    let report = build_found_market_and_fund_v1(fixture.program_id, &fixture.state)
        .expect("derived founding plan");
    assert_eq!(report.identity, fixture.identity);
    assert_eq!(report.identity.generation(), FOUNDATION_GENERATION);
    assert_eq!(
        report.instruction.accounts.len(),
        FOUND_MARKET_ACCOUNT_COUNT
    );
    assert_eq!(
        report.instruction.accounts,
        vec![
            AccountMeta::new(fixture.state.sponsor.key, true),
            AccountMeta::new(report.market_address, false),
            AccountMeta::new(report.fund_address, false),
            AccountMeta::new_readonly(fixture.state.rent_credit.key, false),
            AccountMeta::new_readonly(fixture.state.realm.key, false),
            AccountMeta::new_readonly(fixture.state.product_instance.key, false),
            AccountMeta::new_readonly(fixture.state.claim_basis.key, false),
            AccountMeta::new_readonly(fixture.state.capacity_profile.key, false),
            AccountMeta::new_readonly(fixture.state.resolution_material.key, false),
            AccountMeta::new_readonly(fixture.state.capability_manifest.key, false),
            AccountMeta::new_readonly(fixture.state.realm_finalization.staging_cursor.key, false),
            AccountMeta::new_readonly(
                fixture
                    .state
                    .product_instance_finalization
                    .staging_cursor
                    .key,
                false,
            ),
            AccountMeta::new_readonly(
                fixture.state.claim_basis_finalization.staging_cursor.key,
                false,
            ),
            AccountMeta::new_readonly(
                fixture
                    .state
                    .capacity_profile_finalization
                    .staging_cursor
                    .key,
                false,
            ),
            AccountMeta::new_readonly(
                fixture
                    .state
                    .resolution_material_finalization
                    .staging_cursor
                    .key,
                false,
            ),
            AccountMeta::new_readonly(
                fixture
                    .state
                    .capability_manifest_finalization
                    .staging_cursor
                    .key,
                false,
            ),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(sysvar::rent::ID, false),
        ]
    );
    let wire = FoundMarketAndFundV1::decode(&report.instruction.data).expect("exact wire");
    assert_eq!(wire.identity(), fixture.identity);
    assert_eq!(wire.outcome_count(), 2);
    assert_eq!(report.resolution_funding.index(), 0);
    assert_eq!(
        report.resolution_funding.entry().config_id().to_bytes(),
        fixture.identity.resolution_policy_id().to_bytes()
    );
    assert_eq!(
        report.resolution_funding.entry().release_id().to_bytes(),
        SourceMaterialViewV1::decode(&fixture.state.resolution_material.data)
            .expect("material")
            .primary_provider_release()
            .expect("primary provider")
            .1
            .adapter_release_id()
            .to_bytes()
    );
    assert_eq!(
        report
            .resolution_funding
            .entry()
            .capacity_profile_id()
            .to_bytes(),
        [23; 32]
    );
    assert_ne!(
        report
            .resolution_funding
            .entry()
            .capacity_profile_id()
            .to_bytes(),
        hash(&fixture.state.capacity_profile.data).to_bytes()
    );
    assert_eq!(report.debit.provider_fee_reimbursement, 17);
    assert_eq!(report.debit.resolution_success_bounty, 23);
    assert_eq!(
        report
            .resolution_funding
            .entry()
            .funding_quote()
            .amounts()
            .rent()
            .amount(),
        report.debit.fund_rent
    );
    assert_eq!(
        report.debit.total_sponsor_debit,
        report.debit.market_rent
            + report.debit.fund_rent
            + report.debit.provider_fee_reimbursement
            + report.debit.resolution_success_bounty
    );
}

#[test]
fn founding_funding_authority_refuses_wrong_quote_config_and_release() {
    let fixture = FoundFixture::new(2, 16);
    let material =
        SourceMaterialViewV1::decode(&fixture.state.resolution_material.data).expect("material");
    let release_id = material
        .primary_provider_release()
        .expect("provider")
        .1
        .adapter_release_id()
        .to_bytes();
    let config_id = hash(material.as_bytes()).to_bytes();
    let capability_capacity_id = [23; 32];
    let fund_rent = Rent::default().minimum_balance(FUNDING_BYTES);

    let mut wrong_rent = fixture.state.clone();
    let wrong_rent_quote =
        native_resolution_quote(fund_rent.checked_add(1).expect("small rent"), 17, 23);
    let wrong_rent_manifest = resolution_manifest(
        release_id,
        config_id,
        capability_capacity_id,
        wrong_rent_quote,
    );
    replace_finalized_record(
        fixture.program_id,
        &mut wrong_rent.capability_manifest,
        &mut wrong_rent.capability_manifest_finalization,
        wrong_rent_manifest,
    );
    let before = wrong_rent.clone();
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &wrong_rent),
        Err(FoundationError::InvalidFundingAuthority)
    );
    assert_eq!(wrong_rent, before);

    let exact_quote = native_resolution_quote(fund_rent, 17, 23);
    let mut wrong_config = fixture.state.clone();
    let wrong_config_manifest =
        resolution_manifest(release_id, [71; 32], capability_capacity_id, exact_quote);
    replace_finalized_record(
        fixture.program_id,
        &mut wrong_config.capability_manifest,
        &mut wrong_config.capability_manifest_finalization,
        wrong_config_manifest,
    );
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &wrong_config),
        Err(FoundationError::InvalidFundingAuthority)
    );

    let mut wrong_release = fixture.state.clone();
    let wrong_release_manifest =
        resolution_manifest([72; 32], config_id, capability_capacity_id, exact_quote);
    replace_finalized_record(
        fixture.program_id,
        &mut wrong_release.capability_manifest,
        &mut wrong_release.capability_manifest_finalization,
        wrong_release_manifest,
    );
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &wrong_release),
        Err(FoundationError::ContentLinkMismatch)
    );

    let zero_bounty = native_resolution_quote(fund_rent, 17, 0);
    let mut missing_bounty = fixture.state.clone();
    let missing_bounty_manifest =
        resolution_manifest(release_id, config_id, capability_capacity_id, zero_bounty);
    replace_finalized_record(
        fixture.program_id,
        &mut missing_bounty.capability_manifest,
        &mut missing_bounty.capability_manifest_finalization,
        missing_bounty_manifest,
    );
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &missing_bounty),
        Err(FoundationError::InvalidFundingAuthority)
    );
}

#[test]
fn wrong_pda_owner_and_content_link_refuse_without_partial_plan() {
    let fixture = FoundFixture::new(2, 16);
    let mut wrong_pda = fixture.state.clone();
    wrong_pda.market_destination.key = Pubkey::new_from_array([70; 32]);
    let before = wrong_pda.clone();
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &wrong_pda),
        Err(FoundationError::DestinationNotVacant)
    );
    assert_eq!(wrong_pda, before);

    let mut wrong_owner = fixture.state.clone();
    wrong_owner.claim_basis.owner = system_program::ID;
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &wrong_owner),
        Err(FoundationError::InvalidOwner)
    );

    let mut wrong_link = fixture.state.clone();
    let instance = InstanceV1::decode(&wrong_link.product_instance.data).expect("instance");
    let replacement = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([3; 32]),
        occurrence_id: instance.occurrence_id(),
        claim_basis_id: product_id([99; 32]),
        result_domain_id: instance.result_domain_id(),
        capacity_profile_id: CapacityProfileId::new(product_id(
            hash(&wrong_link.capacity_profile.data).to_bytes(),
        )),
        partition_cell_count: 2,
    })
    .expect("hostile linked instance");
    replace_finalized_record(
        fixture.program_id,
        &mut wrong_link.product_instance,
        &mut wrong_link.product_instance_finalization,
        replacement.to_bytes().to_vec(),
    );
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &wrong_link),
        Err(FoundationError::ContentLinkMismatch)
    );
}

#[test]
fn found_refuses_canonical_source_substitution_and_wrong_record_release() {
    let fixture = FoundFixture::new(3, 16);
    let instance = InstanceV1::decode(&fixture.state.product_instance.data).expect("instance");
    let other_instance = InstanceV1::new(InstanceV1Input {
        terms_id: instance.terms_id(),
        occurrence_id: product_id([96; 32]),
        claim_basis_id: instance.claim_basis_id(),
        result_domain_id: instance.result_domain_id(),
        capacity_profile_id: instance.capacity_profile_id(),
        partition_cell_count: instance.partition_cell_count(),
    })
    .expect("other instance");
    let domain = SourceMaterialViewV1::decode(&fixture.state.resolution_material.data)
        .expect("material")
        .result_domain()
        .expect("result domain");
    let substituted = source_material(
        other_instance,
        hash(&other_instance.to_bytes()).to_bytes(),
        domain,
    );
    let mut wrong_source = fixture.state.clone();
    replace_finalized_record(
        fixture.program_id,
        &mut wrong_source.resolution_material,
        &mut wrong_source.resolution_material_finalization,
        substituted,
    );
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &wrong_source),
        Err(FoundationError::ContentLinkMismatch)
    );

    let mut wrong_release = fixture.state.clone();
    wrong_release
        .product_instance_finalization
        .schema_release_id = [95; 32];
    let data = wrong_release.product_instance.data.clone();
    replace_finalized_record(
        fixture.program_id,
        &mut wrong_release.product_instance,
        &mut wrong_release.product_instance_finalization,
        data,
    );
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &wrong_release),
        Err(FoundationError::AddressMismatch)
    );
}

#[test]
fn unsupported_outcome_width_refuses_before_instruction_construction() {
    let mut fixture = FoundFixture::new(16, 32);
    let widest_v1 = build_found_market_and_fund_v1(fixture.program_id, &fixture.state)
        .expect("profile-1 maximum outcome width");
    assert_eq!(widest_v1.outcome_count, 16);
    let capacity =
        CapacityProfileV1::decode(&fixture.state.capacity_profile.data).expect("capacity");
    let capacity_id = CapacityProfileId::new(product_id(
        hash(&fixture.state.capacity_profile.data).to_bytes(),
    ));
    let claim = CategoricalUnitV1::new(
        CategoricalUnitV1Input {
            capacity_profile_id: capacity_id,
            outcome_count: 17,
        },
        capacity,
    )
    .expect("profile admits 17 for hostile foundation input");
    let claim_data = claim.to_bytes().to_vec();
    let instance = InstanceV1::new(InstanceV1Input {
        terms_id: product_id([3; 32]),
        occurrence_id: product_id([4; 32]),
        claim_basis_id: product_id(hash(&claim_data).to_bytes()),
        result_domain_id: InstanceV1::decode(&fixture.state.product_instance.data)
            .expect("instance")
            .result_domain_id(),
        capacity_profile_id: capacity_id,
        partition_cell_count: 17,
    })
    .expect("17-cell Product remains valid outside categorical adapter profile");
    replace_finalized_record(
        fixture.program_id,
        &mut fixture.state.claim_basis,
        &mut fixture.state.claim_basis_finalization,
        claim_data,
    );
    replace_finalized_record(
        fixture.program_id,
        &mut fixture.state.product_instance,
        &mut fixture.state.product_instance_finalization,
        instance.to_bytes().to_vec(),
    );
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &fixture.state),
        Err(FoundationError::InvalidOutcomeCount)
    );
}

#[test]
fn rent_funding_finality_and_observation_mismatch_refuse() {
    let fixture = FoundFixture::new(2, 16);
    let mut bad_rent = fixture.state.clone();
    bad_rent.rent_sysvar.data.push(0);
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &bad_rent),
        Err(FoundationError::InvalidRent)
    );

    let mut underfunded_record = fixture.state.clone();
    underfunded_record.claim_basis.lamports = 0;
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &underfunded_record),
        Err(FoundationError::AccountNotRentExempt)
    );

    // Finalization closes each staging cursor to an empty System account. A
    // third party may transfer lamports to that address afterward; the SBF
    // record authenticator deliberately treats that dust as irrelevant to the
    // finalized content identity, so the operator must do the same.
    let mut dusted_finalization_cursors = fixture.state.clone();
    for cursor in [
        &mut dusted_finalization_cursors
            .realm_finalization
            .staging_cursor,
        &mut dusted_finalization_cursors
            .product_instance_finalization
            .staging_cursor,
        &mut dusted_finalization_cursors
            .claim_basis_finalization
            .staging_cursor,
        &mut dusted_finalization_cursors
            .capacity_profile_finalization
            .staging_cursor,
        &mut dusted_finalization_cursors
            .resolution_material_finalization
            .staging_cursor,
        &mut dusted_finalization_cursors
            .capability_manifest_finalization
            .staging_cursor,
    ] {
        cursor.lamports = 1;
    }
    assert!(
        build_found_market_and_fund_v1(fixture.program_id, &dusted_finalization_cursors).is_ok()
    );

    let mut nonfinal = fixture.state.clone();
    for account in [
        &mut nonfinal.sponsor,
        &mut nonfinal.rent_credit,
        &mut nonfinal.realm,
        &mut nonfinal.product_instance,
        &mut nonfinal.claim_basis,
        &mut nonfinal.capacity_profile,
        &mut nonfinal.resolution_material,
        &mut nonfinal.capability_manifest,
        &mut nonfinal.system_program,
        &mut nonfinal.rent_sysvar,
    ] {
        account.observation.finality = Finality::Confirmed;
    }
    for vacancy in [
        &mut nonfinal.realm_finalization.staging_cursor,
        &mut nonfinal.product_instance_finalization.staging_cursor,
        &mut nonfinal.claim_basis_finalization.staging_cursor,
        &mut nonfinal.capacity_profile_finalization.staging_cursor,
        &mut nonfinal.resolution_material_finalization.staging_cursor,
        &mut nonfinal.capability_manifest_finalization.staging_cursor,
    ] {
        vacancy.observation.finality = Finality::Confirmed;
    }
    nonfinal.market_destination.observation.finality = Finality::Confirmed;
    nonfinal.fund_destination.observation.finality = Finality::Confirmed;
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &nonfinal),
        Err(FoundationError::ObservationNotFinalized)
    );

    let mut mixed = fixture.state.clone();
    mixed.claim_basis.observation.slot += 1;
    assert_eq!(
        build_found_market_and_fund_v1(fixture.program_id, &mixed),
        Err(FoundationError::ObservationMismatch)
    );
}

#[test]
fn create_realm_refuses_wrong_destination_owner_mint_rent_and_balance() {
    let (program_id, state) = create_realm_state();
    let mut wrong_destination = state.clone();
    wrong_destination.realm_destination.key = Pubkey::new_from_array([66; 32]);
    assert_eq!(
        build_create_realm_v1(program_id, &wrong_destination, RealmAuthorityPolicy::STRICT),
        Err(FoundationError::DestinationNotVacant)
    );

    let mut wrong_mint_owner = state.clone();
    wrong_mint_owner.collateral_mint.owner = system_program::ID;
    assert_eq!(
        build_create_realm_v1(program_id, &wrong_mint_owner, RealmAuthorityPolicy::STRICT),
        Err(FoundationError::InvalidOwner)
    );

    let mut hostile_mint = state.clone();
    hostile_mint.collateral_mint.data.push(0);
    assert_eq!(
        build_create_realm_v1(program_id, &hostile_mint, RealmAuthorityPolicy::STRICT),
        Err(FoundationError::InvalidMint)
    );

    let mut wrong_rent = state.clone();
    wrong_rent.rent_sysvar.owner = system_program::ID;
    assert_eq!(
        build_create_realm_v1(program_id, &wrong_rent, RealmAuthorityPolicy::STRICT),
        Err(FoundationError::InvalidRent)
    );

    let mut poor = state;
    poor.sponsor.lamports = 0;
    assert_eq!(
        build_create_realm_v1(program_id, &poor, RealmAuthorityPolicy::STRICT),
        Err(FoundationError::SponsorUnderfunded)
    );
}

fn open_state() -> (Pubkey, OpenCollateralVaultState) {
    let fixture = FoundFixture::new(2, 16);
    let sponsor = fixture.state.sponsor.key;
    let mut root = MarketRoot::founding(fixture.identity, sponsor.to_bytes()).expect("root");
    root.register_child(FOUNDATION_GENERATION, 0)
        .expect("fund child");
    root.register_child(FOUNDATION_GENERATION, 1)
        .expect("readiness child");
    let market =
        CategoricalMarketV1::<2>::new(root, 0, [0; 2], CategoricalSettlementSummaryV1::empty())
            .expect("market");
    let mut market_data = vec![0; CategoricalMarketV1::<2>::encoded_len().expect("market len")];
    market.encode(&mut market_data).expect("market encode");
    let market_key = Pubkey::find_program_address(
        &[MARKET_SEED, &hash(&fixture.identity.to_bytes()).to_bytes()],
        &fixture.program_id,
    )
    .0;
    let manifest =
        CapabilityManifestV1::decode(&fixture.state.capability_manifest.data).expect("manifest");
    let manifest_id = CapabilityContentId::new(hash(manifest.as_bytes()).to_bytes()).expect("ID");
    let mut readiness = MarketOpeningReadinessV1::begin(
        market_key.to_bytes(),
        FOUNDATION_GENERATION,
        manifest_id,
        manifest,
        sponsor.to_bytes(),
    )
    .expect("readiness");
    let selected = manifest
        .required_founding_entry_for_config(fixture.identity.resolution_policy_id())
        .expect("entry");
    let funding = construct_required_resolution_funding(
        manifest_id,
        manifest,
        selected,
        Rent::default().minimum_balance(dclutch_pyth_contract::funding::FUNDING_BYTES),
        observation().slot,
    )
    .expect("funding");
    readiness
        .advance(
            market_key.to_bytes(),
            FOUNDATION_GENERATION,
            manifest_id,
            manifest,
            0,
            funding,
            FundingCustodyObservationV1::native_only(
                dclutch_pyth_contract::funding::required_resolution_minimum_balance(funding)
                    .expect("minimum"),
                Rent::default().minimum_balance(dclutch_pyth_contract::funding::FUNDING_BYTES),
            )
            .expect("native-only custody"),
            observation().slot,
        )
        .expect("ready");
    let readiness_key = Pubkey::find_program_address(
        &[
            MARKET_OPENING_READINESS_PDA_DOMAIN,
            market_key.as_ref(),
            &FOUNDATION_GENERATION.to_le_bytes(),
        ],
        &fixture.program_id,
    )
    .0;
    let rent = Rent::default();
    let custody = Pubkey::find_program_address(
        &[COLLATERAL_CUSTODY_PDA_DOMAIN, market_key.as_ref()],
        &fixture.program_id,
    )
    .0;
    let vault = Pubkey::find_program_address(
        &[COLLATERAL_VAULT_PDA_DOMAIN, market_key.as_ref()],
        &fixture.program_id,
    )
    .0;
    (
        fixture.program_id,
        OpenCollateralVaultState {
            sponsor: observed(sponsor, system_program::ID, u64::MAX, Vec::new(), false),
            market: observed(
                market_key,
                fixture.program_id,
                rent.minimum_balance(market_data.len()),
                market_data,
                false,
            ),
            readiness: observed(
                readiness_key,
                fixture.program_id,
                rent.minimum_balance(MARKET_OPENING_READINESS_BYTES),
                readiness.to_bytes().to_vec(),
                false,
            ),
            rent_credit: fixture.state.rent_credit.clone(),
            capability_manifest: fixture.state.capability_manifest.clone(),
            capability_manifest_finalization: fixture
                .state
                .capability_manifest_finalization
                .clone(),
            realm: fixture.state.realm.clone(),
            realm_finalization: fixture.state.realm_finalization.clone(),
            custody_destination: observed(custody, system_program::ID, 0, Vec::new(), false),
            vault_destination: observed(vault, system_program::ID, 0, Vec::new(), false),
            collateral_mint: observed(
                Pubkey::new_from_array([92; 32]),
                Pubkey::new_from_array(dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID),
                u64::MAX,
                mint_data(false),
                false,
            ),
            token_program: observed(
                Pubkey::new_from_array(dclutch_token_svm::LEGACY_TOKEN_PROGRAM_ID),
                bpf_loader_upgradeable::ID,
                1,
                Vec::new(),
                true,
            ),
            system_program: system_program_account(),
            rent_sysvar: rent_account(),
        },
    )
}

#[test]
fn open_vault_derives_open14_and_refuses_hostile_destinations() {
    let (program, state) = open_state();
    let report = build_open_collateral_vault_v1(program, &state).expect("Open14");
    assert_eq!(
        report.instruction.accounts.len(),
        OPEN_COLLATERAL_VAULT_ACCOUNT_COUNT
    );
    assert_eq!(report.generation, FOUNDATION_GENERATION);
    assert_eq!(report.child_count, 2);
    assert_eq!(
        report.instruction.accounts.get(9).map(|meta| meta.pubkey),
        Some(state.capability_manifest_finalization.staging_cursor.key)
    );
    assert_eq!(
        report.instruction.accounts.get(10).map(|meta| meta.pubkey),
        Some(state.realm_finalization.staging_cursor.key)
    );
    let mut occupied = state.clone();
    occupied.custody_destination.lamports = 1;
    assert_eq!(
        build_open_collateral_vault_v1(program, &occupied),
        Err(FoundationError::DestinationNotVacant)
    );
    let mut incomplete = state;
    *incomplete
        .readiness
        .data
        .get_mut(0)
        .expect("canonical readiness has a header") ^= 1;
    assert_eq!(
        build_open_collateral_vault_v1(program, &incomplete),
        Err(FoundationError::InvalidRecord)
    );
}
