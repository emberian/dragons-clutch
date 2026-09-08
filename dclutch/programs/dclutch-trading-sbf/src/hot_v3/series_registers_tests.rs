use std::{boxed::Box, vec, vec::Vec};

use dclutch_core_contract::ContentId as CoreContentId;
use dclutch_custody::{
    CustodyReplaySeedsV1, CustodyVaultSeedsV1, PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
    ProjectedCustodyStateSeedsV2,
};
use dclutch_market::{
    Identity, MarketCoreStateSeedsV2, MarketIdentity,
    capability_program::{CapabilityRootHeaderV1, SelectedRecordBumpsV1},
    realm::{
        FreezeAuthorityPolicy, MintAuthorityPolicy, REALM_SCHEMA_RELEASE_ID_V1, RealmV1,
        RealmV1Input,
    },
    rent::{
        RefundAuthority,
        lifecycle_v2::{
            LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2, LifecycleAccountIdV2, LifecycleRentCreditV2,
        },
    },
};
use dclutch_product::{
    ContentId as ProductContentId,
    payoff::runtime_v3::BasisKindV3,
    svm_reader::{
        AuthenticatedProductRuntimeV2, AuthenticatedProductRuntimeV3, AuthenticatedRecordV2,
        ProductRecordBumpsV3,
    },
};
use dclutch_registry::{
    record::{RAW_RECORD_PDA_SEED_V1, STAGING_CURSOR_PDA_SEED_V1},
    release_set::CapabilityExecutionSelectionV1,
};
use dclutch_source::{
    BONDING_CURVE_FLOOR_DERIVATION_ID_V1, CapacityEnvelope, ContentId as SourceContentId,
    MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1, ManipulationFloorBasis, ManipulationFloorV1,
    SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1, SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
    SOURCE_SPEC_SCHEMA_ID_V1, SourceAccessProfile, SourceCapacityProfileV1, SourceMaterialV3,
    SourceSpecV1,
};
use dclutch_trading::series::{
    AccountKeyV3, AuthenticatedProductProjectionV2, OccurrenceV3,
    SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3, SERIES_SUCCESSOR_KIND_PREIMAGE_V3,
    SERIES_TICKET_SCHEMA_RELEASE_ID_V3, TemplateV3, admit_occurrence, admit_ticket, generated,
    occurrence_content_id,
    replay::{SeriesStateV3, TicketStateSeedsV3},
    template_content_id,
};
use dclutch_vm::account_profile::AccountObservationV1;
use solana_program::{
    account_info::AccountInfo,
    hash::{hash, hashv},
    pubkey::Pubkey,
    rent::Rent,
};
use solana_sdk_ids::system_program;

use super::*;
use crate::{
    TradingSbfError,
    hot_v3::frame::AuthenticatedChildProgramsV3,
    series::{
        custody_v3::SeriesCustodyPhysicalV3,
        derived_prepare_v1::{
            SeriesPrepareDerivedRequestInputV1, derive_series_prepare_requests_v1,
        },
        operator::{SeriesOccurrenceSnapshotV3, build_prepare_v3},
        prepare_funding_artifacts_v5 as prepare,
        projected_custody_v3::SeriesProjectedCustodyPhysicalV3,
    },
};

const HASH_SEPARATOR: [u8; 1] = [0];
const BLANK_KEY: [u8; 32] = [0xc8; 32];
const BLANK_OWNER: [u8; 32] = [0xc9; 32];
const SOURCE_MATERIAL_RAW: usize = 31;
const SOURCE_SPEC_RAW: usize = 33;
const SOURCE_CAPACITY_RAW: usize = 35;
const MANIPULATION_FLOOR_RAW: usize = 37;

struct PrepareFixture {
    program_id: Pubkey,
    frame: HotFrameV3<'static, 'static>,
    product: AuthenticatedProductRuntimeV3<'static, 'static>,
    programs: AuthenticatedChildProgramsV3,
    observations: Vec<AccountObservationV1<'static>>,
    template: Vec<u8>,
    occurrence: Vec<u8>,
    ticket: Vec<u8>,
    siblings: [[u8; 32]; 2],
    series: SeriesStateV3,
    family: Vec<u8>,
    projection: AuthenticatedProductProjectionV2,
    custody: SeriesCustodyPhysicalV3,
    projected: SeriesProjectedCustodyPhysicalV3,
    root: [u8; 32],
    registry: AccountKeyV3,
    cap: u64,
}

impl PrepareFixture {
    fn new() -> Self {
        let program_id = Pubkey::new_from_array([0x41; 32]);
        let registry_program = Pubkey::new_from_array([0x42; 32]);
        let core_program = Pubkey::new_from_array([0x43; 32]);
        let custody_program = Pubkey::new_from_array([0x44; 32]);
        let claims_program = Pubkey::new_from_array([0x45; 32]);
        let rent_program = Pubkey::new_from_array([0x46; 32]);
        let collateral_mint = [0x47; 32];
        let token_program = [0x48; 32];
        let product_record = source_id(0x49);

        let mut observations =
            vec![
                AccountObservationV1::new(&BLANK_KEY, &BLANK_OWNER, 0, &[], false, false, false,);
                prepare::SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize
            ];

        let capacity = SourceCapacityProfileV1::new(
            CapacityEnvelope::Provisional,
            1,
            0,
            source_id(0x4a),
            source_id(0x4b),
            208,
            0,
        )
        .expect("capacity")
        .bounding_principal(1, 4)
        .expect("principal ratio");
        let capacity_bytes = capacity.to_bytes().to_vec();
        let capacity_digest = hash(&capacity_bytes).to_bytes();
        let spec = SourceSpecV1::new(
            source_id(0x4c),
            source_id(0x4d),
            source_id(0x4e),
            SourceAccessProfile::RelayedObservationRecord,
            source_id(0x4f),
            source_digest(capacity_digest),
        );
        let spec_bytes = spec.to_bytes().to_vec();
        let spec_digest = hash(&spec_bytes).to_bytes();
        let floor = ManipulationFloorV1::new(
            ManipulationFloorBasis::CurveDerived,
            source_digest(spec_digest),
            source_id(0x4f),
            source_digest(collateral_mint),
            source_digest(BONDING_CURVE_FLOOR_DERIVATION_ID_V1),
            400,
        );
        let floor_bytes = floor.to_bytes().to_vec();
        let floor_digest = hash(&floor_bytes).to_bytes();
        let material = SourceMaterialV3::bounded_by_floor(
            product_record,
            source_digest(spec_digest),
            source_id(0x50),
            source_id(0x51),
            None,
            source_id(0x52),
            source_digest(floor_digest),
        );
        let material_bytes = material.to_bytes().to_vec();
        let material_digest = hash(&material_bytes).to_bytes();
        install_record(
            &mut observations,
            registry_program,
            SOURCE_MATERIAL_RAW,
            SOURCE_MATERIAL_SCHEMA_RELEASE_ID_V3,
            material_bytes,
        );
        install_record(
            &mut observations,
            registry_program,
            SOURCE_SPEC_RAW,
            SOURCE_SPEC_SCHEMA_ID_V1,
            spec_bytes,
        );
        install_record(
            &mut observations,
            registry_program,
            SOURCE_CAPACITY_RAW,
            SOURCE_CAPACITY_PROFILE_SCHEMA_ID_V1,
            capacity_bytes,
        );
        install_record(
            &mut observations,
            registry_program,
            MANIPULATION_FLOOR_RAW,
            MANIPULATION_FLOOR_SCHEMA_RELEASE_ID_V1,
            floor_bytes,
        );

        let realm = RealmV1::new(RealmV1Input {
            token_program,
            collateral_mint,
            collateral_adapter_release_id: [0x53; 32],
            mint_authority_policy: MintAuthorityPolicy::RequireAbsent,
            freeze_authority_policy: FreezeAuthorityPolicy::RequireAbsent,
        })
        .expect("Realm");
        let realm_bytes = realm.to_bytes().to_vec();
        let realm_digest = hash(&realm_bytes).to_bytes();
        install_record(
            &mut observations,
            registry_program,
            21,
            REALM_SCHEMA_RELEASE_ID_V1,
            realm_bytes,
        );

        let mut occurrence = generated::SERIES_EXAMPLE_OCCURRENCE_V3.to_vec();
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_PRODUCT_RECORD_OFFSET_V3,
            &product_record.to_bytes(),
        );
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_RESOLUTION_POLICY_OFFSET_V3,
            &material_digest,
        );
        let mut template = generated::SERIES_EXAMPLE_TEMPLATE_V3.to_vec();
        put(
            &mut template,
            generated::SERIES_TEMPLATE_REALM_OFFSET_V3,
            &realm_digest,
        );
        let template_value = TemplateV3::decode(&template).expect("Template");
        let occurrence_value = OccurrenceV3::decode(&occurrence).expect("Occurrence");
        let future = MarketIdentity {
            market_id: Identity::new([0x68; 32]).expect("logical Market"),
            realm_id: Identity::new(realm_digest).expect("Realm"),
            product_record: Identity::new(product_record.to_bytes()).expect("Product record"),
            product_id: Identity::new([0x56; 32]).expect("Product"),
            resolution_policy: Identity::new(material_digest).expect("Source"),
            capability_manifest: Identity::new(occurrence_value.capability_manifest().to_bytes())
                .expect("manifest"),
            selected_release_set: Identity::new(template_value.release_set().to_bytes())
                .expect("ReleaseSet"),
            registry_program: Identity::new(registry_program.to_bytes()).expect("Registry"),
            generation: u64::from(occurrence_value.occurrence()) + 1,
        };
        let market = Pubkey::find_program_address(
            &MarketCoreStateSeedsV2::new(future).as_slices(),
            &core_program,
        )
        .0;
        put(
            &mut occurrence,
            generated::SERIES_OCCURRENCE_MARKET_OFFSET_V3,
            &market.to_bytes(),
        );
        let occurrence_id = occurrence_content_id(&occurrence).expect("occurrence ID");
        let siblings = [[0x54; 32], [0x55; 32]];
        put(
            &mut template,
            generated::SERIES_TEMPLATE_PROJECTION_ROOT_OFFSET_V3,
            &projection_root(occurrence_id.to_bytes(), 1, &siblings),
        );
        let template_id = template_content_id(&template).expect("Template ID");
        let mut ticket = generated::SERIES_EXAMPLE_TICKET_V3.to_vec();
        put(
            &mut ticket,
            generated::SERIES_TICKET_TEMPLATE_OFFSET_V3,
            &template_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_OCCURRENCE_ID_OFFSET_V3,
            &occurrence_id.to_bytes(),
        );
        put(
            &mut ticket,
            generated::SERIES_TICKET_MARKET_OFFSET_V3,
            &market.to_bytes(),
        );
        install_record(
            &mut observations,
            registry_program,
            prepare::SERIES_PREPARE_OCCURRENCE_RAW_COORDINATE_V5 as usize,
            SERIES_OCCURRENCE_SCHEMA_RELEASE_ID_V3,
            occurrence.clone(),
        );
        install_record(
            &mut observations,
            registry_program,
            prepare::SERIES_PREPARE_TICKET_RAW_COORDINATE_V5 as usize,
            SERIES_TICKET_SCHEMA_RELEASE_ID_V3,
            ticket.clone(),
        );

        let series = SeriesStateV3::new(7)
            .prepare_ticket(0)
            .expect("prepare zero")
            .settle_current(1, 3)
            .expect("settle zero")
            .retire_ticket(2)
            .expect("retire zero");
        let projection = AuthenticatedProductProjectionV2::new(
            content(product_record.to_bytes()),
            content([0x56; 32]),
            content([0x57; 32]),
        );
        let admitted = admit_occurrence(&template, &occurrence, &siblings).expect("occurrence");
        let escrow = crate::series::derived_prepare_v1::derive_prefounding_escrow(
            admitted,
            &ticket,
            projection,
            AccountKeyV3::new(registry_program.to_bytes()).expect("Registry"),
        )
        .expect("escrow");
        let root = [0x58; 32];
        let payer = [0x59; 32];
        let founder_source = [0x5a; 32];
        let context = hashv(&[
            PROJECTED_HOARD_CONTEXT_DOMAIN_V1,
            &escrow.ticket_id().to_bytes(),
        ])
        .to_bytes();
        let state = Pubkey::find_program_address(
            &ProjectedCustodyStateSeedsV2::new(
                escrow.market().to_bytes(),
                escrow.release_set().to_bytes(),
                context,
            )
            .as_slices(),
            &custody_program,
        )
        .0
        .to_bytes();
        let replay = Pubkey::find_program_address(
            &CustodyReplaySeedsV1::new(
                escrow.market().to_bytes(),
                escrow.release_set().to_bytes(),
                dclutch_custody::CallerRoleV1::Trading,
                escrow.ticket_id().to_bytes(),
            )
            .as_slices(),
            &custody_program,
        )
        .0
        .to_bytes();
        let hoard_vault = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(
                escrow.market().to_bytes(),
                escrow.release_set().to_bytes(),
                context,
                dclutch_custody::CompartmentV1::HoardPrincipal,
            )
            .as_slices(),
            &custody_program,
        )
        .0
        .to_bytes();
        let escrow_vault = Pubkey::find_program_address(
            &CustodyVaultSeedsV1::new(
                escrow.market().to_bytes(),
                escrow.release_set().to_bytes(),
                escrow.ticket_id().to_bytes(),
                dclutch_custody::CompartmentV1::SeriesEscrow,
            )
            .as_slices(),
            &custody_program,
        )
        .0
        .to_bytes();

        let generation = escrow.generation();
        let generation_seed = generation.to_le_bytes();
        let (rent_credit, credit_bump) = Pubkey::find_program_address(
            &[
                LIFECYCLE_RENT_CREDIT_PDA_DOMAIN_V2,
                &escrow.market().to_bytes(),
                &generation_seed,
            ],
            &rent_program,
        );
        let credit = LifecycleRentCreditV2::new(
            RefundAuthority::new(escrow.refund_owner().to_bytes()).expect("refund"),
            LifecycleAccountIdV2::new(escrow.market().to_bytes()).expect("Market"),
            LifecycleAccountIdV2::new(escrow.release_set().to_bytes()).expect("ReleaseSet"),
            generation,
            credit_bump,
        )
        .expect("RentCredit")
        .to_bytes()
        .to_vec();

        for coordinate in prepare::SERIES_PREPARE_CHILD_CREATED_COORDINATES_V5 {
            observations[coordinate as usize] = vacant([0xa0 | coordinate as u8; 32]);
        }
        observations[7] = vacant(state);
        observations[60] = vacant(hoard_vault);
        observations[76] = vacant(replay);
        observations[91] = vacant(escrow_vault);
        observations[5] = vacant(
            Pubkey::find_program_address(
                &TicketStateSeedsV3::new(root, admit_ticket(&ticket).expect("Ticket").content_id())
                    .as_slices(),
                &program_id,
            )
            .0
            .to_bytes(),
        );
        observations[12] = observation(
            rent_credit.to_bytes(),
            rent_program.to_bytes(),
            1,
            credit,
            false,
        );
        observations[14] = observation(payer, [0x5b; 32], 1, Vec::new(), false);
        observations[20] = observation(rent_program.to_bytes(), [0x5c; 32], 1, Vec::new(), true);
        observations[106] = observation(collateral_mint, [0x5d; 32], 1, Vec::new(), false);
        observations[107] = observation(founder_source, token_program, 1, Vec::new(), false);
        observations[110] = observation(token_program, [0x5e; 32], 1, Vec::new(), true);
        observations[prepare::SERIES_PREPARE_CUSTODY_PROGRAM_COORDINATE_V5 as usize] =
            observation(custody_program.to_bytes(), [0x5f; 32], 1, Vec::new(), true);

        let template_value = TemplateV3::decode(&template).expect("Template");
        let selection = CapabilityExecutionSelectionV1::new(
            0,
            content([0x60; 32]),
            content(hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes()),
            content([0x61; 32]),
            content(hash(&template).to_bytes()),
        )
        .expect("selection");
        let header = CapabilityRootHeaderV1::new(
            template_value.release_set(),
            [0x62; 32],
            9,
            selection,
            SelectedRecordBumpsV1::default(),
        )
        .expect("root header");
        let mut root_data = header.to_bytes().to_vec();
        root_data.extend_from_slice(
            &series
                .encode(template_value.occurrence_count())
                .expect("Series state"),
        );
        observations[0] = observation(root, program_id.to_bytes(), 1, root_data.clone(), false);
        observations[1] = observation(
            hash(&template).to_bytes(),
            registry_program.to_bytes(),
            1,
            template.clone(),
            false,
        );

        let root_info = account(root, program_id.to_bytes(), root_data, false);
        let config_info = account(
            hash(&template).to_bytes(),
            registry_program.to_bytes(),
            template.clone(),
            false,
        );
        let registry_info = account(registry_program.to_bytes(), [0x63; 32], Vec::new(), true);
        let core_info = account(core_program.to_bytes(), [0x64; 32], Vec::new(), true);
        let trading_info = account(program_id.to_bytes(), [0x65; 32], Vec::new(), true);
        let inert = account([0x66; 32], [0x67; 32], Vec::new(), false);
        let frame = frame(
            root_info,
            config_info,
            registry_info,
            core_info,
            trading_info,
            inert,
        );
        let product = product_runtime(product_record.to_bytes(), inert);
        let rent = Rent::default();
        let custody = SeriesCustodyPhysicalV3 {
            caller_program: program_id.to_bytes(),
            parent_request_digest: [0; 32],
            payer,
            mint: collateral_mint,
            token_program,
            founder_source,
            escrow_vault,
            hoard_vault,
            refund_destination: [0; 32],
            rent_credit: rent_credit.to_bytes(),
            replay_rent_lamports: rent.minimum_balance(dclutch_custody::CUSTODY_REPLAY_BYTES_V1),
            vault_rent_lamports: rent.minimum_balance(dclutch_custody::token_svm::ACCOUNT_BYTES),
        };
        let projected = SeriesProjectedCustodyPhysicalV3 {
            caller_program: custody.caller_program,
            core_program: core_program.to_bytes(),
            rent_program: rent_program.to_bytes(),
            parent_capability_root: root,
            projection_receipt_digest: [0; 32],
            payer,
            rent_credit: custody.rent_credit,
            hoard_vault,
            escrow_vault,
            mint: collateral_mint,
            token_program,
            collateral_release: [0x53; 32],
            projected_state_rent_lamports: rent
                .minimum_balance(dclutch_custody::PROJECTED_CUSTODY_STATE_BYTES_V2),
            hoard_vault_rent_lamports: custody.vault_rent_lamports,
            escrow_replay_rent_lamports: custody.replay_rent_lamports,
            escrow_vault_rent_lamports: custody.vault_rent_lamports,
        };
        let snapshot = SeriesOccurrenceSnapshotV3 {
            template_bytes: &template,
            occurrence_bytes: &occurrence,
            ticket_bytes: &ticket,
            siblings: &siblings,
            series,
            ticket_state: None,
            now_slot: 110,
        };
        let family = build_prepare_v3(snapshot)
            .expect("canonical Prepare family")
            .as_bytes()
            .to_vec();

        Self {
            program_id,
            frame,
            product,
            programs: AuthenticatedChildProgramsV3 {
                claims: claims_program.to_bytes(),
                custody: custody_program.to_bytes(),
            },
            observations,
            template,
            occurrence,
            ticket,
            siblings,
            series,
            family,
            projection,
            custody,
            projected,
            root,
            registry: AccountKeyV3::new(registry_program.to_bytes()).expect("Registry"),
            cap: 10,
        }
    }

    fn snapshot(&self) -> SeriesOccurrenceSnapshotV3<'_> {
        SeriesOccurrenceSnapshotV3 {
            template_bytes: &self.template,
            occurrence_bytes: &self.occurrence,
            ticket_bytes: &self.ticket,
            siblings: &self.siblings,
            series: self.series,
            ticket_state: None,
            now_slot: 110,
        }
    }
}

#[test]
fn authenticated_prepare_observations_seed_the_native_bank_and_substitutions_refuse() {
    let fixture = PrepareFixture::new();
    let mut actual = vec![0_u64; prepare::SERIES_PREPARE_COMMON_SCALAR_COUNT_V5 as usize];
    assert_eq!(
        seed_authenticated_series_derived_scalars_at_slot_v1(
            hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes(),
            dclutch_trading::series::request::SeriesActionV3::Prepare as u32,
            &fixture.family,
            &fixture.program_id,
            &fixture.frame,
            &fixture.product,
            &Rent::default(),
            Some(fixture.programs),
            &fixture.observations,
            &mut actual,
            110,
        ),
        Ok(true)
    );

    let bank = derive_series_prepare_requests_v1(SeriesPrepareDerivedRequestInputV1 {
        family_request: &fixture.family,
        snapshot: fixture.snapshot(),
        product: fixture.projection,
        registry_program: fixture.registry,
        parent_root: fixture.root,
        custody: fixture.custody,
        projected_custody: fixture.projected,
        principal_cap_sets: fixture.cap,
    })
    .expect("native Prepare bank");
    let mut expected = vec![0_u64; prepare::SERIES_PREPARE_COMMON_SCALAR_COUNT_V5 as usize];
    bank.write_scalar_words(
        &fixture.family,
        fixture.root,
        &mut expected[prepare::SERIES_PREPARE_DERIVED_REQUEST_SCALAR_START_V1 as usize
            ..prepare::SERIES_PREPARE_DERIVED_REQUEST_SCALAR_END_V1 as usize],
    )
    .expect("native request words");
    bank.write_replay_scalars(
        &fixture.family,
        fixture.root,
        &mut expected[prepare::SERIES_PREPARE_RESULT_PREPARED_SCALAR_V1 as usize..],
    )
    .expect("native replay words");
    assert_eq!(actual, expected);

    let mut hostile = fixture.observations.clone();
    hostile[SOURCE_MATERIAL_RAW] = observation(
        [0x99; 32],
        fixture.frame.registry.key.to_bytes(),
        1,
        hostile[SOURCE_MATERIAL_RAW].data().to_vec(),
        false,
    );
    assert_seed_refuses(&fixture, &hostile);

    let mut hostile_product = fixture.product;
    hostile_product.runtime.product_record.content_digest = product_content([0x98; 32]);
    assert_seed_with_product_refuses(&fixture, &fixture.observations, &hostile_product);

    let mut hostile_cap = fixture.observations.clone();
    hostile_cap[SOURCE_CAPACITY_RAW] = observation(
        hostile_cap[SOURCE_CAPACITY_RAW].key(),
        hostile_cap[SOURCE_CAPACITY_RAW].owner(),
        1,
        vec![0_u8; hostile_cap[SOURCE_CAPACITY_RAW].data().len()],
        false,
    );
    assert_seed_refuses(&fixture, &hostile_cap);

    let mut hostile_root = fixture.observations.clone();
    hostile_root[0] = observation(
        [0x97; 32],
        fixture.program_id.to_bytes(),
        1,
        hostile_root[0].data().to_vec(),
        false,
    );
    assert_seed_refuses(&fixture, &hostile_root);
}

#[test]
fn prepare_custody_carrier_is_present_once_and_executable_at_the_canonical_tail() {
    let count = prepare::SERIES_PREPARE_FIXED_ACCOUNT_COUNT_V5 as usize;
    let coordinate = prepare::SERIES_PREPARE_CUSTODY_PROGRAM_COORDINATE_V5 as usize;
    assert_eq!(coordinate + 1, count);
    let custody = [0x44; 32];
    let inert = account([0x90; 32], [0x91; 32], Vec::new(), false).clone();
    let carrier = account(custody, [0x92; 32], Vec::new(), true).clone();
    let mut accounts = vec![inert.clone(); count];
    accounts[coordinate] = carrier;
    let aliases = (0..count).collect::<Vec<_>>();
    let resolve = |accounts: &[AccountInfo<'static>]| {
        crate::hot_v3::children::resolve_carrier_by_representative_v3(
            accounts.len(),
            &aliases,
            custody,
            |index| {
                accounts
                    .get(index)
                    .cloned()
                    .ok_or_else(|| TradingSbfError::Content.into())
            },
        )
    };
    assert_eq!(
        resolve(&accounts).expect("canonical Custody carrier").key,
        &Pubkey::new_from_array(custody)
    );

    accounts[coordinate] = inert;
    assert_eq!(
        resolve(&accounts).err(),
        Some(TradingSbfError::Release.into())
    );
    accounts[coordinate] = account(custody, [0x92; 32], Vec::new(), false).clone();
    assert_eq!(
        resolve(&accounts).err(),
        Some(TradingSbfError::Release.into())
    );
}

fn assert_seed_refuses(fixture: &PrepareFixture, observations: &[AccountObservationV1<'_>]) {
    assert_seed_with_product_refuses(fixture, observations, &fixture.product);
}

fn assert_seed_with_product_refuses(
    fixture: &PrepareFixture,
    observations: &[AccountObservationV1<'_>],
    product: &AuthenticatedProductRuntimeV3<'_, '_>,
) {
    let mut output = vec![7_u64; prepare::SERIES_PREPARE_COMMON_SCALAR_COUNT_V5 as usize];
    assert_eq!(
        seed_authenticated_series_derived_scalars_at_slot_v1(
            hash(SERIES_SUCCESSOR_KIND_PREIMAGE_V3).to_bytes(),
            dclutch_trading::series::request::SeriesActionV3::Prepare as u32,
            &fixture.family,
            &fixture.program_id,
            &fixture.frame,
            product,
            &Rent::default(),
            Some(fixture.programs),
            observations,
            &mut output,
            110,
        ),
        Err(TradingSbfError::Content.into())
    );
    assert!(output.iter().all(|word| *word == 7));
}

fn install_record(
    observations: &mut [AccountObservationV1<'static>],
    registry: Pubkey,
    raw_coordinate: usize,
    schema: [u8; 32],
    data: Vec<u8>,
) {
    let digest = hash(&data).to_bytes();
    let raw =
        Pubkey::find_program_address(&[RAW_RECORD_PDA_SEED_V1, &schema, &digest], &registry).0;
    let staging =
        Pubkey::find_program_address(&[STAGING_CURSOR_PDA_SEED_V1, &schema, &digest], &registry).0;
    observations[raw_coordinate] = observation(raw.to_bytes(), registry.to_bytes(), 1, data, false);
    observations[raw_coordinate + 1] = vacant(staging.to_bytes());
}

fn projection_root(mut node: [u8; 32], mut index: u32, siblings: &[[u8; 32]]) -> [u8; 32] {
    for sibling in siblings {
        node = if index & 1 == 0 {
            hashv(&[
                &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                &HASH_SEPARATOR,
                &node,
                sibling,
            ])
            .to_bytes()
        } else {
            hashv(&[
                &generated::SERIES_PROJECTION_NODE_DOMAIN_V3,
                &HASH_SEPARATOR,
                sibling,
                &node,
            ])
            .to_bytes()
        };
        index >>= 1;
    }
    node
}

fn put(target: &mut [u8], offset: usize, value: &[u8; 32]) {
    target[offset..offset + 32].copy_from_slice(value);
}

fn source_id(byte: u8) -> SourceContentId {
    SourceContentId::new([byte; 32]).expect("Source identity")
}

fn source_digest(bytes: [u8; 32]) -> SourceContentId {
    SourceContentId::new(bytes).expect("Source digest")
}

fn content(bytes: [u8; 32]) -> CoreContentId {
    CoreContentId::new(bytes).expect("content identity")
}

fn product_content(bytes: [u8; 32]) -> ProductContentId {
    ProductContentId::new(bytes).expect("Product content identity")
}

fn vacant(key: [u8; 32]) -> AccountObservationV1<'static> {
    observation(key, system_program::ID.to_bytes(), 0, Vec::new(), false)
}

fn observation(
    key: [u8; 32],
    owner: [u8; 32],
    lamports: u64,
    data: Vec<u8>,
    executable: bool,
) -> AccountObservationV1<'static> {
    AccountObservationV1::new(
        Box::leak(Box::new(key)),
        Box::leak(Box::new(owner)),
        lamports,
        Box::leak(data.into_boxed_slice()),
        false,
        false,
        executable,
    )
}

fn account(
    key: [u8; 32],
    owner: [u8; 32],
    data: Vec<u8>,
    executable: bool,
) -> &'static AccountInfo<'static> {
    Box::leak(Box::new(AccountInfo::new(
        Box::leak(Box::new(Pubkey::new_from_array(key))),
        false,
        false,
        Box::leak(Box::new(1_u64)),
        Box::leak(data.into_boxed_slice()),
        Box::leak(Box::new(Pubkey::new_from_array(owner))),
        executable,
    )))
}

fn frame(
    root: &'static AccountInfo<'static>,
    config: &'static AccountInfo<'static>,
    registry: &'static AccountInfo<'static>,
    core: &'static AccountInfo<'static>,
    trading: &'static AccountInfo<'static>,
    inert: &'static AccountInfo<'static>,
) -> HotFrameV3<'static, 'static> {
    HotFrameV3 {
        market: inert,
        root,
        manifest_raw: inert,
        manifest_staging: inert,
        program_set_raw: inert,
        program_set_staging: inert,
        descriptor_raw: inert,
        descriptor_staging: inert,
        config_raw: config,
        config_staging: inert,
        account_profile_raw: inert,
        account_profile_staging: inert,
        request_profile_raw: inert,
        request_profile_staging: inert,
        transition_raw: inert,
        transition_staging: inert,
        effect_raw: inert,
        effect_staging: inert,
        lifecycle_raw: inert,
        lifecycle_staging: inert,
        strategy_raw: inert,
        strategy_staging: inert,
        activation_cache: inert,
        core_program: core,
        core_programdata: inert,
        trading_program: trading,
        trading_programdata: inert,
        registry,
        rent: inert,
        instructions: inert,
        product_raw: inert,
        product_staging: inert,
        result_domain_raw: inert,
        result_domain_staging: inert,
        portfolio_raw: inert,
        portfolio_staging: inert,
        linked_basis_raw: inert,
        linked_basis_staging: inert,
        capability_seal: inert,
    }
}

fn product_runtime(
    product_record: [u8; 32],
    inert: &'static AccountInfo<'static>,
) -> AuthenticatedProductRuntimeV3<'static, 'static> {
    let record = |tag| AuthenticatedRecordV2 {
        schema_id: product_content([tag; 32]),
        content_digest: product_content([tag + 1; 32]),
        raw_account: Pubkey::new_from_array([tag + 2; 32]),
        staging_account: Pubkey::new_from_array([tag + 3; 32]),
    };
    AuthenticatedProductRuntimeV3 {
        runtime: AuthenticatedProductRuntimeV2 {
            product_record: AuthenticatedRecordV2 {
                content_digest: product_content(product_record),
                ..record(0x70)
            },
            result_domain_record: record(0x74),
            portfolio_record: record(0x78),
            product_id: product_content([0x56; 32]),
            coordinate_domain_id: product_content([0x7c; 32]),
            result_unit_id: product_content([0x7d; 32]),
            claim_basis_id: product_content([0x7e; 32]),
            liability_basis_id: product_content([0x7f; 32]),
            representation_release_id: product_content([0x80; 32]),
            mapping_release_id: product_content([0x81; 32]),
            outcome_count: 2,
            record_bumps: ProductRecordBumpsV3::ABSENT,
        },
        linked_basis_record: record(0x82),
        linked_basis_raw: inert,
        linked_basis_staging: inert,
        semantic_basis_id: product_content([0x83; 32]),
        basis_kind: BasisKindV3::CategoricalQ1,
        basis_width: 2,
        payout_scale: 10,
        evaluator_release_id: product_content([0x84; 32]),
        record_bumps: ProductRecordBumpsV3::ABSENT,
    }
}
