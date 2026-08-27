//! Host-side executable evidence for the provider-neutral source/archive seam.
//!
//! This test lives in the SVM workspace so it compiles the exact `clutch-sbf`
//! library that the ELF compiles.  There is intentionally no `ProgramTest`
//! instruction yet: the live `FeedAdvance` and `Resolve` routes still own the
//! old caller-buffer ABI.  Calling this a bank/CU result before that join would
//! be false.  The only provider implementation here is a deterministic mock;
//! no production oracle ABI or authentication claim follows from it.

use clutch_sbf::{
    source::{
        ParsedPriceV1, PriceParserV1, SourceAccountView, SourceError, SourceSpecFieldsV1,
        SourceSpecV1, TrustedClockV1, ORIENTATION_QUOTE_PER_BASE,
        SELECTION_FINALIZED_BUCKET_RECORD,
    },
    source_archive::{
        append_authenticated, archived_observation, decode_source_spec_body_v1, initialize_archive,
        initialize_authenticated_source_spec_account, initialize_genesis_archive,
        initialize_source_spec_account, initialize_successor_archive, seal_archive,
        seal_archive_authenticated, verify_recorded_sealed_archive,
        verify_recorded_sealed_archive_view, verify_sealed_archive, verify_source_spec_account,
        ArchiveAccountViewV1, ArchivePredecessorV1, CoveragePolicy, DeploymentAuthenticatorV1,
        FeedIdentity, Grid, RuntimeAccountViewV1, SourceArchiveError, SourceSpecAccountViewV1,
        VerifiedSourceSpecAccountV1, WindowDomain, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
        SOURCE_SPEC_ACCOUNT_V1_BYTES,
    },
};
use clutch_solana_layout::Hash32;

const ADAPTER: [u8; 32] = [0xa1; 32];
const PROVIDER_PROGRAM: [u8; 32] = [0xb2; 32];
const PROVIDER_LOADER: [u8; 32] = [0xb3; 32];
const DEPLOYMENT: [u8; 32] = [0xb4; 32];
const DEPLOYMENT_OWNER: [u8; 32] = [0xb5; 32];
const SOURCE_ACCOUNT: [u8; 32] = [0xc3; 32];
const VERIFIER: [u8; 32] = [0xd4; 32];
const CLUTCH_PROGRAM: [u8; 32] = [0xe5; 32];
const SOURCE_SPEC_KEY: [u8; 32] = [0xe6; 32];
const ARCHIVE_KEY: [u8; 32] = [0xf6; 32];
const OTHER_ARCHIVE_KEY: [u8; 32] = [0xf7; 32];
const DEPLOYMENT_GENERATION: u64 = 9;
const RECORD_BYTES: usize = 77;

struct MockDeployment;

impl DeploymentAuthenticatorV1 for MockDeployment {
    const VERIFIER_ID: [u8; 32] = VERIFIER;
    const VERIFIER_VERSION: u32 = 2;
    const PROVIDER_PROGRAM: [u8; 32] = PROVIDER_PROGRAM;
    const PROVIDER_PROGRAM_OWNER: [u8; 32] = PROVIDER_LOADER;
    const DEPLOYMENT_ACCOUNT: [u8; 32] = DEPLOYMENT;
    const DEPLOYMENT_OWNER: [u8; 32] = DEPLOYMENT_OWNER;

    fn deployment_generation(
        provider_program_data: &[u8],
        deployment_account_data: &[u8],
    ) -> Result<u64, SourceArchiveError> {
        if provider_program_data != b"mock-provider-program-v1"
            || deployment_account_data.len() != 16
            || &deployment_account_data[..8] != b"MOCKDEP1"
        {
            return Err(SourceArchiveError::DeploymentAdapterRefused);
        }
        let mut generation = [0_u8; 8];
        generation.copy_from_slice(&deployment_account_data[8..]);
        Ok(u64::from_le_bytes(generation))
    }
}

struct MockPriceParser;

impl PriceParserV1 for MockPriceParser {
    const SOURCE_ADAPTER_ID: [u8; 32] = ADAPTER;
    const SOURCE_ADAPTER_VERSION: u32 = 7;
    const PARSER_ID: u16 = 11;
    const PARSER_VERSION: u16 = 3;

    fn parse(account: SourceAccountView<'_>) -> Result<ParsedPriceV1, SourceError> {
        let bytes = account.data();
        if bytes.len() != RECORD_BYTES || &bytes[..4] != b"SRC1" {
            return Err(SourceError::ParserRefused);
        }
        Ok(ParsedPriceV1 {
            deployment_generation: u64_at(bytes, 4),
            source_sequence: u64_at(bytes, 12),
            publish_slot: u64_at(bytes, 20),
            publish_time: u64_at(bytes, 28),
            canonical_bucket: u64_at(bytes, 36),
            price_atoms: u128_at(bytes, 44),
            confidence_atoms: u128_at(bytes, 60),
            finalized_bucket: bytes[76] == 1,
        })
    }
}

struct WrongPriceParser;

impl PriceParserV1 for WrongPriceParser {
    const SOURCE_ADAPTER_ID: [u8; 32] = ADAPTER;
    const SOURCE_ADAPTER_VERSION: u32 = 7;
    const PARSER_ID: u16 = 99;
    const PARSER_VERSION: u16 = 3;

    fn parse(_: SourceAccountView<'_>) -> Result<ParsedPriceV1, SourceError> {
        Err(SourceError::ParserRefused)
    }
}

fn spec() -> SourceSpecV1 {
    SourceSpecV1::new(SourceSpecFieldsV1 {
        source_adapter_id: Hash32::from_bytes(ADAPTER),
        source_adapter_version: 7,
        parser_id: 11,
        parser_version: 3,
        source_program: PROVIDER_PROGRAM,
        source_account: SOURCE_ACCOUNT,
        deployment_generation: DEPLOYMENT_GENERATION,
        base_asset_id: Hash32::from_bytes([1; 32]),
        quote_asset_id: Hash32::from_bytes([2; 32]),
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 6,
        grid_family_id: 5,
        grid_version: 2,
        bucket_seconds: 60,
        max_staleness_slots: 20,
        max_staleness_seconds: 120,
        max_future_seconds: 2,
        max_confidence_atoms: 10_000,
        max_confidence_bps: 200,
        confidence_multiplier: 2,
        selection_rule: SELECTION_FINALIZED_BUCKET_RECORD,
    })
    .expect("valid frozen mock spec")
}

fn window(spec: SourceSpecV1) -> WindowDomain {
    window_range(spec, 100, 103, 104, 6)
}

fn window_range(
    spec: SourceSpecV1,
    start: u64,
    end: u64,
    maturity: u64,
    generation: u64,
) -> WindowDomain {
    let feed = FeedIdentity::new(
        ADAPTER,
        spec.feed_id().bytes(),
        spec.source_adapter_version(),
        4,
    )
    .expect("versioned feed");
    WindowDomain::new(
        feed,
        Grid::new(5, 2, 60).expect("grid"),
        start,
        end,
        maturity,
        generation,
        CoveragePolicy::COMPLETE_REQUIRED,
    )
    .expect("three-bucket window")
}

#[test]
fn source_spec_construction_authenticates_the_closed_release_before_writing() {
    let spec = spec();
    let body = spec.encode_canonical();
    assert_eq!(decode_source_spec_body_v1(&body), Ok(spec));
    let mut padded = body;
    padded[255] = 1;
    assert_eq!(
        decode_source_spec_body_v1(&padded),
        Err(SourceArchiveError::NonCanonicalPadding)
    );

    let deployment_data = deployment_bytes(DEPLOYMENT_GENERATION);
    let provider = provider_program(b"mock-provider-program-v1");
    let deployment = deployment(&deployment_data);
    let source = SourceAccountView::new(SOURCE_ACCOUNT, PROVIDER_PROGRAM, false, b"");
    let mut account = [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_authenticated_source_spec_account::<MockPriceParser, MockDeployment>(
        &mut account,
        spec,
        254,
        provider,
        deployment,
        source,
    )
    .expect("registered mock release constructs its exact source spec");
    assert_eq!(verified_spec(&account).spec(), spec);

    let mut refused = [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    assert_eq!(
        initialize_authenticated_source_spec_account::<WrongPriceParser, MockDeployment>(
            &mut refused,
            spec,
            254,
            provider,
            deployment,
            source,
        ),
        Err(SourceArchiveError::AdapterReleaseMismatch)
    );
    assert_eq!(refused, [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES]);

    assert_eq!(
        initialize_authenticated_source_spec_account::<MockPriceParser, MockDeployment>(
            &mut refused,
            spec,
            254,
            provider,
            deployment,
            SourceAccountView::new([0x44; 32], PROVIDER_PROGRAM, false, b""),
        ),
        Err(SourceArchiveError::Source(
            SourceError::SourceAccountMismatch
        ))
    );
    assert_eq!(refused, [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES]);
}

#[test]
fn successor_initialization_derives_exact_lineage_from_a_sealed_receipt() {
    let spec = spec();
    let first_window = window_range(spec, 100, 103, 104, 0);
    let mut spec_account = [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_source_spec_account(&mut spec_account, spec, 254).unwrap();
    let verified = verified_spec(&spec_account);
    let mut first = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_genesis_archive::<MockDeployment>(&mut first, verified, first_window, 253)
        .expect("generation-zero first archive");
    for index in 0..3_u64 {
        let bucket = 100 + index;
        append(
            &mut first,
            &spec_account,
            first_window,
            &record(bucket, 41 + index, 1_000 + index, 1_000_000, 2_000),
            1_005 + index,
            bucket * 60 + 1,
        )
        .unwrap();
    }
    seal_archive::<MockDeployment>(&mut first, verified, first_window, 104).unwrap();
    let receipt = verify_sealed_archive::<MockDeployment>(
        CLUTCH_PROGRAM,
        ARCHIVE_KEY,
        ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &first),
        verified,
        first_window,
    )
    .unwrap();
    assert_eq!(receipt.repair_generation(), 0);

    let next_window = window_range(spec, 103, 106, 107, 0);
    let mut next = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_successor_archive::<MockDeployment>(&mut next, verified, next_window, receipt, 252)
        .expect("sealed receipt supplies the only successor predecessor");
    append(
        &mut next,
        &spec_account,
        next_window,
        &record(103, 44, 1_003, 1_000_003, 2_000),
        1_008,
        6_181,
    )
    .expect("sequence 44 continues receipt sequence 43");

    let mut refused = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    let gapped = window_range(spec, 104, 107, 108, 0);
    assert_eq!(
        initialize_successor_archive::<MockDeployment>(
            &mut refused,
            verified,
            gapped,
            receipt,
            251,
        ),
        Err(SourceArchiveError::NonContiguousLineage)
    );
    assert_eq!(refused, [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES]);

    let repaired = window_range(spec, 103, 106, 107, 1);
    assert_eq!(
        initialize_successor_archive::<MockDeployment>(
            &mut refused,
            verified,
            repaired,
            receipt,
            251,
        ),
        Err(SourceArchiveError::NonContiguousLineage),
        "V1 has no rule that authorizes a repair-generation transition"
    );
    assert_eq!(refused, [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES]);

    assert_eq!(
        initialize_genesis_archive::<MockDeployment>(&mut refused, verified, repaired, 251),
        Err(SourceArchiveError::NonContiguousLineage),
        "a repair generation cannot reset to caller-authored genesis lineage"
    );
    assert_eq!(refused, [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES]);
}

#[test]
fn sealing_requires_an_authenticated_exactly_next_maturity_record() {
    let spec = spec();
    let window = window_range(spec, 100, 103, 104, 0);
    let mut spec_account = [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_source_spec_account(&mut spec_account, spec, 254).unwrap();
    let verified = verified_spec(&spec_account);
    let mut archive = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_genesis_archive::<MockDeployment>(&mut archive, verified, window, 253).unwrap();
    for index in 0..3_u64 {
        let bucket = 100 + index;
        append(
            &mut archive,
            &spec_account,
            window,
            &record(bucket, 41 + index, 1_000 + index, 1_000_000, 2_000),
            1_005 + index,
            bucket * 60 + 1,
        )
        .unwrap();
    }
    let deployment_data = deployment_bytes(DEPLOYMENT_GENERATION);
    let before = archive;
    assert_eq!(
        seal_archive_authenticated::<MockPriceParser, MockDeployment>(
            &mut archive,
            verified,
            window,
            TrustedClockV1 {
                slot: 1_008,
                unix_seconds: 6_241,
            },
            provider_program(b"mock-provider-program-v1"),
            deployment(&deployment_data),
            SourceAccountView::new(
                SOURCE_ACCOUNT,
                PROVIDER_PROGRAM,
                false,
                &record(104, 44, 1_003, 1_000_003, 2_000),
            ),
        ),
        Err(SourceArchiveError::Source(SourceError::WrongBucket))
    );
    assert_eq!(archive, before);

    seal_archive_authenticated::<MockPriceParser, MockDeployment>(
        &mut archive,
        verified,
        window,
        TrustedClockV1 {
            slot: 1_008,
            unix_seconds: 6_181,
        },
        provider_program(b"mock-provider-program-v1"),
        deployment(&deployment_data),
        SourceAccountView::new(
            SOURCE_ACCOUNT,
            PROVIDER_PROGRAM,
            false,
            &record(103, 44, 1_003, 1_000_003, 2_000),
        ),
    )
    .expect("bucket end is the unique one-bucket maturity witness");
    let receipt = verify_sealed_archive::<MockDeployment>(
        CLUTCH_PROGRAM,
        ARCHIVE_KEY,
        ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive),
        verified,
        window,
    )
    .unwrap();
    assert_eq!(receipt.sealed_feed_cursor(), 104);

    let long_horizon = window_range(spec, 100, 103, 105, 0);
    let mut long = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_genesis_archive::<MockDeployment>(&mut long, verified, long_horizon, 252).unwrap();
    for index in 0..3_u64 {
        let bucket = 100 + index;
        append(
            &mut long,
            &spec_account,
            long_horizon,
            &record(bucket, 41 + index, 1_000 + index, 1_000_000, 2_000),
            1_005 + index,
            bucket * 60 + 1,
        )
        .unwrap();
    }
    let long_before = long;
    assert_eq!(
        seal_archive_authenticated::<MockPriceParser, MockDeployment>(
            &mut long,
            verified,
            long_horizon,
            TrustedClockV1 {
                slot: 1_008,
                unix_seconds: 6_181,
            },
            provider_program(b"mock-provider-program-v1"),
            deployment(&deployment_data),
            SourceAccountView::new(
                SOURCE_ACCOUNT,
                PROVIDER_PROGRAM,
                false,
                &record(103, 44, 1_003, 1_000_003, 2_000),
            ),
        ),
        Err(SourceArchiveError::NotMature)
    );
    assert_eq!(long, long_before);
}

fn provider_program<'a>(data: &'a [u8]) -> RuntimeAccountViewV1<'a> {
    RuntimeAccountViewV1::new(PROVIDER_PROGRAM, PROVIDER_LOADER, true, data)
}

fn deployment<'a>(data: &'a [u8]) -> RuntimeAccountViewV1<'a> {
    RuntimeAccountViewV1::new(DEPLOYMENT, DEPLOYMENT_OWNER, false, data)
}

fn deployment_bytes(generation: u64) -> [u8; 16] {
    let mut out = [0_u8; 16];
    out[..8].copy_from_slice(b"MOCKDEP1");
    out[8..].copy_from_slice(&generation.to_le_bytes());
    out
}

fn record(
    bucket: u64,
    sequence: u64,
    publish_slot: u64,
    price: u128,
    confidence: u128,
) -> [u8; RECORD_BYTES] {
    let mut out = [0_u8; RECORD_BYTES];
    out[..4].copy_from_slice(b"SRC1");
    out[4..12].copy_from_slice(&DEPLOYMENT_GENERATION.to_le_bytes());
    out[12..20].copy_from_slice(&sequence.to_le_bytes());
    out[20..28].copy_from_slice(&publish_slot.to_le_bytes());
    out[28..36].copy_from_slice(&(bucket * 60).to_le_bytes());
    out[36..44].copy_from_slice(&bucket.to_le_bytes());
    out[44..60].copy_from_slice(&price.to_le_bytes());
    out[60..76].copy_from_slice(&confidence.to_le_bytes());
    out[76] = 1;
    out
}

fn append(
    archive: &mut [u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES],
    spec_account: &[u8; SOURCE_SPEC_ACCOUNT_V1_BYTES],
    window: WindowDomain,
    bytes: &[u8; RECORD_BYTES],
    clock_slot: u64,
    clock_time: u64,
) -> Result<(), SourceArchiveError> {
    let deployment_data = deployment_bytes(DEPLOYMENT_GENERATION);
    let verified_spec = verified_spec(spec_account);
    append_authenticated::<MockPriceParser, MockDeployment>(
        archive,
        verified_spec,
        window,
        TrustedClockV1 {
            slot: clock_slot,
            unix_seconds: clock_time,
        },
        provider_program(b"mock-provider-program-v1"),
        deployment(&deployment_data),
        SourceAccountView::new(SOURCE_ACCOUNT, PROVIDER_PROGRAM, false, bytes),
    )
}

fn verified_spec(spec_account: &[u8; SOURCE_SPEC_ACCOUNT_V1_BYTES]) -> VerifiedSourceSpecAccountV1 {
    verify_source_spec_account(
        CLUTCH_PROGRAM,
        SOURCE_SPEC_KEY,
        SourceSpecAccountViewV1::new(SOURCE_SPEC_KEY, CLUTCH_PROGRAM, false, spec_account),
    )
    .expect("authenticated source spec account")
}

fn complete_archive() -> (
    [u8; SOURCE_SPEC_ACCOUNT_V1_BYTES],
    [u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES],
    WindowDomain,
) {
    let spec = spec();
    let window = window(spec);
    let mut spec_account = [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_source_spec_account(&mut spec_account, spec, 254).expect("source spec account");
    let mut archive = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_archive::<MockDeployment>(
        &mut archive,
        verified_spec(&spec_account),
        window,
        ArchivePredecessorV1 {
            source_sequence: 40,
            publish_slot: 990,
            publish_time: 5_999,
            archive_commitment: Hash32::from_bytes([0x55; 32]),
        },
        253,
    )
    .expect("open archive");
    for index in 0..3_u64 {
        let bucket = 100 + index;
        append(
            &mut archive,
            &spec_account,
            window,
            &record(
                bucket,
                41 + index,
                1_000 + index,
                u128::from(1_000_000 + index),
                2_000,
            ),
            1_005 + index,
            bucket * 60 + 1,
        )
        .expect("authenticated append");
    }
    seal_archive::<MockDeployment>(&mut archive, verified_spec(&spec_account), window, 104)
        .expect("complete mature archive");
    (spec_account, archive, window)
}

#[test]
fn exact_layouts_append_seal_and_receipt_are_stable() {
    assert_eq!(SOURCE_SPEC_ACCOUNT_V1_BYTES, 292);
    assert_eq!(SOURCE_ARCHIVE_ACCOUNT_V1_BYTES, 2_560);
    let (spec_account, archive, window) = complete_archive();
    let verified = verified_spec(&spec_account);
    assert_eq!(verified.account_key(), SOURCE_SPEC_KEY);
    assert_eq!(verified.feed(), spec().feed_id());
    assert_eq!(verified.stored_bump(), 254);

    let receipt = verify_sealed_archive::<MockDeployment>(
        CLUTCH_PROGRAM,
        ARCHIVE_KEY,
        ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive),
        verified,
        window,
    )
    .expect("sealed receipt");
    assert_eq!(receipt.archive_key(), ARCHIVE_KEY);
    assert_eq!(receipt.feed(), spec().feed_id());
    assert_eq!(receipt.deployment_generation(), DEPLOYMENT_GENERATION);
    assert_eq!(
        (receipt.start_bucket(), receipt.end_bucket_exclusive()),
        (100, 103)
    );
    assert_eq!(receipt.sealed_feed_cursor(), 104);
    assert_eq!(receipt.last_source_sequence(), 43);
    assert_eq!(receipt.last_publish_slot(), 1_002);
    assert_eq!(receipt.last_publish_time(), 6_120);
    assert_eq!(
        receipt.authenticated_archive().summary_digest,
        receipt.page_commitment()
    );

    assert_eq!(
        verify_source_spec_account(
            CLUTCH_PROGRAM,
            SOURCE_SPEC_KEY,
            SourceSpecAccountViewV1::new(OTHER_ARCHIVE_KEY, CLUTCH_PROGRAM, false, &spec_account),
        ),
        Err(SourceArchiveError::SourceSpecAccountMismatch)
    );
    assert_eq!(
        verify_source_spec_account(
            CLUTCH_PROGRAM,
            SOURCE_SPEC_KEY,
            SourceSpecAccountViewV1::new(SOURCE_SPEC_KEY, [0x88; 32], false, &spec_account),
        ),
        Err(SourceArchiveError::SourceSpecOwnerMismatch)
    );
}

#[test]
fn unrelated_same_domain_buffer_cannot_substitute_for_the_archive_account() {
    let (spec_account, archive, window) = complete_archive();
    assert_eq!(
        verify_sealed_archive::<MockDeployment>(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            ArchiveAccountViewV1::new(OTHER_ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive),
            verified_spec(&spec_account),
            window,
        ),
        Err(SourceArchiveError::ArchiveAccountMismatch)
    );
    assert_eq!(
        verify_sealed_archive::<MockDeployment>(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            ArchiveAccountViewV1::new(ARCHIVE_KEY, [0x99; 32], false, &archive),
            verified_spec(&spec_account),
            window,
        ),
        Err(SourceArchiveError::ArchiveOwnerMismatch)
    );
    assert_eq!(
        verify_sealed_archive::<MockDeployment>(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, true, &archive),
            verified_spec(&spec_account),
            window,
        ),
        Err(SourceArchiveError::ArchiveExecutable)
    );
}

#[test]
fn verified_view_reads_each_record_once_with_receipt_equivalent_results() {
    let (spec_account, archive, window) = complete_archive();
    let account = ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive);
    let receipt = verify_recorded_sealed_archive(
        CLUTCH_PROGRAM,
        ARCHIVE_KEY,
        account,
        verified_spec(&spec_account),
        window,
    )
    .expect("recorded sealed receipt");
    let verified = verify_recorded_sealed_archive_view(
        CLUTCH_PROGRAM,
        ARCHIVE_KEY,
        account,
        verified_spec(&spec_account),
        window,
    )
    .expect("lifetime-bound sealed view");
    assert_eq!(verified.receipt(), receipt);

    for index in 0..3 {
        assert_eq!(
            verified.archived_observation(index),
            archived_observation(receipt, account, index)
        );
    }
    assert_eq!(
        verified.archived_observation(3),
        Err(SourceArchiveError::MalformedRecord)
    );
    assert_eq!(
        verified.archived_observation(usize::MAX),
        Err(SourceArchiveError::MalformedRecord)
    );
}

#[test]
fn verified_view_construction_rejects_key_owner_and_mutated_bytes() {
    let (spec_account, archive, window) = complete_archive();
    assert_eq!(
        verify_recorded_sealed_archive_view(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            ArchiveAccountViewV1::new(OTHER_ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive),
            verified_spec(&spec_account),
            window,
        ),
        Err(SourceArchiveError::ArchiveAccountMismatch)
    );
    assert_eq!(
        verify_recorded_sealed_archive_view(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            ArchiveAccountViewV1::new(ARCHIVE_KEY, [0x99; 32], false, &archive),
            verified_spec(&spec_account),
            window,
        ),
        Err(SourceArchiveError::ArchiveOwnerMismatch)
    );

    let mut mutated = archive;
    mutated[512 + 8] ^= 1;
    assert_eq!(
        verify_recorded_sealed_archive_view(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &mutated),
            verified_spec(&spec_account),
            window,
        ),
        Err(SourceArchiveError::CommitmentMismatch)
    );
}

#[test]
fn every_failed_admission_is_a_byte_exact_rollback() {
    let spec = spec();
    let window = window(spec);
    let mut spec_account = [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_source_spec_account(&mut spec_account, spec, 1).unwrap();
    let mut archive = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_archive::<MockDeployment>(
        &mut archive,
        verified_spec(&spec_account),
        window,
        ArchivePredecessorV1::GENESIS,
        2,
    )
    .unwrap();

    let candidate = record(100, 1, 500, 1_000_000, 2_000);
    let deployment_data = deployment_bytes(DEPLOYMENT_GENERATION);
    let before = archive;
    let wrong_source = append_authenticated::<MockPriceParser, MockDeployment>(
        &mut archive,
        verified_spec(&spec_account),
        window,
        TrustedClockV1 {
            slot: 505,
            unix_seconds: 6_001,
        },
        provider_program(b"mock-provider-program-v1"),
        deployment(&deployment_data),
        SourceAccountView::new([0x44; 32], PROVIDER_PROGRAM, false, &candidate),
    );
    assert_eq!(
        wrong_source,
        Err(SourceArchiveError::Source(
            SourceError::SourceAccountMismatch
        ))
    );
    assert_eq!(archive, before);

    let wrong_generation_data = deployment_bytes(DEPLOYMENT_GENERATION + 1);
    let wrong_generation = append_authenticated::<MockPriceParser, MockDeployment>(
        &mut archive,
        verified_spec(&spec_account),
        window,
        TrustedClockV1 {
            slot: 505,
            unix_seconds: 6_001,
        },
        provider_program(b"mock-provider-program-v1"),
        deployment(&wrong_generation_data),
        SourceAccountView::new(SOURCE_ACCOUNT, PROVIDER_PROGRAM, false, &candidate),
    );
    assert_eq!(
        wrong_generation,
        Err(SourceArchiveError::DeploymentGenerationMismatch)
    );
    assert_eq!(archive, before);

    let replayed = record(100, 0, 500, 1_000_000, 2_000);
    let replay_result = append(&mut archive, &spec_account, window, &replayed, 505, 6_001);
    assert_eq!(
        replay_result,
        Err(SourceArchiveError::Source(
            SourceError::SourceSequenceNotMonotone
        ))
    );
    assert_eq!(archive, before);

    let wrong_provider_owner = append_authenticated::<MockPriceParser, MockDeployment>(
        &mut archive,
        verified_spec(&spec_account),
        window,
        TrustedClockV1 {
            slot: 505,
            unix_seconds: 6_001,
        },
        RuntimeAccountViewV1::new(
            PROVIDER_PROGRAM,
            [0x77; 32],
            true,
            b"mock-provider-program-v1",
        ),
        deployment(&deployment_data),
        SourceAccountView::new(SOURCE_ACCOUNT, PROVIDER_PROGRAM, false, &candidate),
    );
    assert_eq!(
        wrong_provider_owner,
        Err(SourceArchiveError::ProviderProgramOwnerMismatch)
    );
    assert_eq!(archive, before);

    let stale = append(&mut archive, &spec_account, window, &candidate, 600, 6_001);
    assert_eq!(
        stale,
        Err(SourceArchiveError::Source(SourceError::SourceStaleBySlot))
    );
    assert_eq!(archive, before);

    let wide = record(100, 1, 500, 1_000_000, 20_000);
    let wide_result = append(&mut archive, &spec_account, window, &wide, 505, 6_001);
    assert_eq!(
        wide_result,
        Err(SourceArchiveError::Source(
            SourceError::ConfidenceTooWideAbsolute
        ))
    );
    assert_eq!(archive, before);

    let wrong_bucket = record(101, 1, 500, 1_000_000, 2_000);
    let wrong_bucket_result = append(
        &mut archive,
        &spec_account,
        window,
        &wrong_bucket,
        505,
        6_061,
    );
    assert_eq!(
        wrong_bucket_result,
        Err(SourceArchiveError::Source(SourceError::WrongBucket))
    );
    assert_eq!(archive, before);

    assert_eq!(
        seal_archive::<MockDeployment>(&mut archive, verified_spec(&spec_account), window, 104),
        Err(SourceArchiveError::NotMature)
    );
    assert_eq!(archive, before);
}

#[test]
fn tampering_and_post_seal_append_are_refused_without_state_change() {
    let (spec_account, archive, window) = complete_archive();
    let mut tampered = archive;
    tampered[512 + 8] ^= 1;
    assert_eq!(
        verify_sealed_archive::<MockDeployment>(
            CLUTCH_PROGRAM,
            ARCHIVE_KEY,
            ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &tampered),
            verified_spec(&spec_account),
            window,
        ),
        Err(SourceArchiveError::CommitmentMismatch)
    );

    let mut sealed = archive;
    let before = sealed;
    let result = append(
        &mut sealed,
        &spec_account,
        window,
        &record(103, 44, 1_003, 1_000_003, 2_000),
        1_008,
        6_181,
    );
    assert_eq!(result, Err(SourceArchiveError::AlreadySealed));
    assert_eq!(sealed, before);

    let mut bad_spec = spec_account;
    bad_spec[34 + 82] ^= 1;
    assert_eq!(
        verify_source_spec_account(
            CLUTCH_PROGRAM,
            SOURCE_SPEC_KEY,
            SourceSpecAccountViewV1::new(SOURCE_SPEC_KEY, CLUTCH_PROGRAM, false, &bad_spec),
        ),
        Err(SourceArchiveError::SourceSpecDigestMismatch)
    );
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    let mut out = [0_u8; 8];
    out.copy_from_slice(&bytes[offset..offset + 8]);
    u64::from_le_bytes(out)
}

fn u128_at(bytes: &[u8], offset: usize) -> u128 {
    let mut out = [0_u8; 16];
    out.copy_from_slice(&bytes[offset..offset + 16]);
    u128::from_le_bytes(out)
}
