//! Host execution of the native occupation preflight over canonical sealed
//! SourceArchive bytes.
//!
//! These host tests isolate the source/archive capability and occupation fold
//! from the live ProgramTest campaign.  They prove old receipt replay and the
//! lifetime-bound one-verification view produce the same candidate, including
//! every v4 persistence field.

use clutch_sbf::{
    native_window::{
        preflight_sealed_archive, preflight_verified_archive, NativeWindowError,
        NativeWindowFinalizationV1, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06,
        STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07,
    },
    source::{
        ParsedPriceV1, PriceParserV1, SourceAccountView, SourceError, SourceSpecFieldsV1,
        SourceSpecV1, TrustedClockV1, ORIENTATION_QUOTE_PER_BASE,
        SELECTION_FINALIZED_BUCKET_RECORD,
    },
    source_archive::{
        append_authenticated, initialize_archive, initialize_source_spec_account, seal_archive,
        verify_recorded_sealed_archive_view, verify_sealed_archive, verify_source_spec_account,
        ArchiveAccountViewV1, ArchivePredecessorV1, CoveragePolicy, DeploymentAuthenticatorV1,
        FeedIdentity, Grid, RuntimeAccountViewV1, SourceArchiveError, SourceSpecAccountViewV1,
        VerifiedSourceSpecAccountV1, WindowDomain, SOURCE_ARCHIVE_ACCOUNT_V1_BYTES,
        SOURCE_SPEC_ACCOUNT_V1_BYTES,
    },
};
use clutch_solana_layout::{
    Hash32, PayoutVectorBytes, TermsAccount, MAX_KNOTS, MAX_OUTCOMES, MAX_PAYOUTS,
    PAYOUT_MAP_UNUSED,
};

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
            finalized_bucket: bytes[76] == 1,
            price_atoms: u128_at(bytes, 44),
            confidence_atoms: u128_at(bytes, 60),
        })
    }
}

fn hash(byte: u8) -> Hash32 {
    Hash32::from_bytes([byte; 32])
}

fn source_spec() -> SourceSpecV1 {
    SourceSpecV1::new(SourceSpecFieldsV1 {
        source_adapter_id: Hash32::from_bytes(ADAPTER),
        source_adapter_version: 7,
        parser_id: 11,
        parser_version: 3,
        source_program: PROVIDER_PROGRAM,
        source_account: SOURCE_ACCOUNT,
        deployment_generation: DEPLOYMENT_GENERATION,
        base_asset_id: hash(1),
        quote_asset_id: hash(2),
        orientation: ORIENTATION_QUOTE_PER_BASE,
        normalized_decimals: 6,
        grid_family_id: 5,
        grid_version: 2,
        bucket_seconds: 60,
        max_staleness_slots: 20,
        max_staleness_seconds: 120,
        max_future_seconds: 2,
        max_confidence_atoms: 10,
        max_confidence_bps: 10_000,
        confidence_multiplier: 1,
        selection_rule: SELECTION_FINALIZED_BUCKET_RECORD,
    })
    .expect("valid frozen mock source spec")
}

fn window(spec: SourceSpecV1) -> WindowDomain {
    let feed = FeedIdentity::new(
        ADAPTER,
        spec.feed_id().bytes(),
        spec.source_adapter_version(),
        1,
    )
    .expect("versioned feed");
    WindowDomain::new(
        feed,
        Grid::new(5, 2, 60).expect("grid"),
        100,
        102,
        103,
        6,
        CoveragePolicy::COMPLETE_REQUIRED,
    )
    .expect("two-bucket complete window")
}

fn occupation_terms(spec: SourceSpecV1, statistic: u16) -> TermsAccount {
    let mut payouts = [PayoutVectorBytes::ZERO; MAX_PAYOUTS];
    let mut anchor = [0_u64; MAX_OUTCOMES];
    anchor[0] = 7;
    payouts[0] = PayoutVectorBytes {
        denominator: 7,
        weights: anchor,
    };
    let mut knots = [0_u128; MAX_KNOTS];
    knots[..3].copy_from_slice(&[0, 8, 16]);
    let mut terms = TermsAccount {
        terms: Hash32::ZERO,
        realm: hash(0x11),
        profile: hash(0x12),
        feed: spec.feed_id(),
        price_grid: hash(0x13),
        outcome_count: 4,
        payout_count: 1,
        payouts,
        grid_family_id: 5,
        grid_version: 2,
        bucket_seconds: 60,
        expected_start_bucket: 100,
        expected_end_bucket_exclusive: 102,
        maturity_horizon_buckets: 3,
        coverage_policy_id: u32::from(CoveragePolicy::COMPLETE_REQUIRED.id()),
        repair_policy_id: 1,
        failure_policy_id: 1,
        statistic_id: statistic,
        ambiguity_policy_id: 1,
        edge_policy_id: 1,
        basis_degree: 2,
        knot_count: 3,
        uniform_log2_spacing: 3,
        failure_payout_index: 0,
        coverage_policy_parameter: 0,
        repair_generation: 6,
        source_version: 7,
        evaluator_version: 1,
        source_adapter_id: Hash32::from_bytes(ADAPTER),
        payout_map: [PAYOUT_MAP_UNUSED; MAX_OUTCOMES],
        knots,
        collateral_cap: 1_000,
        stored_bump: 9,
        flags: 0,
    };
    terms.terms = terms
        .recomputed_terms_digest()
        .expect("occupation terms digest");
    terms.validate().expect("valid occupation terms codec");
    terms
}

fn verified_spec(spec_account: &[u8; SOURCE_SPEC_ACCOUNT_V1_BYTES]) -> VerifiedSourceSpecAccountV1 {
    verify_source_spec_account(
        CLUTCH_PROGRAM,
        SOURCE_SPEC_KEY,
        SourceSpecAccountViewV1::new(SOURCE_SPEC_KEY, CLUTCH_PROGRAM, false, spec_account),
    )
    .expect("authenticated source spec account")
}

fn complete_archive(
    price_a: u128,
    price_b: u128,
    confidence: u128,
) -> (
    SourceSpecV1,
    [u8; SOURCE_SPEC_ACCOUNT_V1_BYTES],
    [u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES],
    WindowDomain,
) {
    let spec = source_spec();
    let window = window(spec);
    let mut spec_account = [0_u8; SOURCE_SPEC_ACCOUNT_V1_BYTES];
    initialize_source_spec_account(&mut spec_account, spec, 254).expect("source spec account");
    let mut archive = [0_u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES];
    initialize_archive::<MockDeployment>(
        &mut archive,
        verified_spec(&spec_account),
        window,
        ArchivePredecessorV1::GENESIS,
        253,
    )
    .expect("open archive");

    let prices = [price_a, price_b];
    let deployment_data = deployment_bytes();
    for (index, price) in prices.into_iter().enumerate() {
        let bucket = 100 + index as u64;
        let bytes = record(
            bucket,
            1 + index as u64,
            1_000 + index as u64,
            price,
            confidence,
        );
        append_authenticated::<MockPriceParser, MockDeployment>(
            &mut archive,
            verified_spec(&spec_account),
            window,
            TrustedClockV1 {
                slot: 1_005 + index as u64,
                unix_seconds: bucket * 60 + 1,
            },
            RuntimeAccountViewV1::new(
                PROVIDER_PROGRAM,
                PROVIDER_LOADER,
                true,
                b"mock-provider-program-v1",
            ),
            RuntimeAccountViewV1::new(DEPLOYMENT, DEPLOYMENT_OWNER, false, &deployment_data),
            SourceAccountView::new(SOURCE_ACCOUNT, PROVIDER_PROGRAM, false, &bytes),
        )
        .expect("authenticated append");
    }
    seal_archive::<MockDeployment>(&mut archive, verified_spec(&spec_account), window, 103)
        .expect("complete mature archive");
    (spec, spec_account, archive, window)
}

fn receipt<'a>(
    spec_account: &[u8; SOURCE_SPEC_ACCOUNT_V1_BYTES],
    archive: &'a [u8; SOURCE_ARCHIVE_ACCOUNT_V1_BYTES],
    window: WindowDomain,
) -> (
    clutch_sbf::source_archive::SealedArchiveReceiptV1,
    ArchiveAccountViewV1<'a>,
) {
    let view = ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, archive);
    let receipt = verify_sealed_archive::<MockDeployment>(
        CLUTCH_PROGRAM,
        ARCHIVE_KEY,
        view,
        verified_spec(spec_account),
        window,
    )
    .expect("sealed archive receipt");
    (receipt, view)
}

#[test]
fn canonical_archive_drives_the_exact_v4_candidate() {
    let (spec, spec_account, archive, window) = complete_archive(4, 4, 0);
    let terms = occupation_terms(spec, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
    let (receipt, view) = receipt(&spec_account, &archive, window);
    let candidate = preflight_sealed_archive(&terms, receipt, view).expect("exact occupation");
    let verified_view = verify_recorded_sealed_archive_view(
        CLUTCH_PROGRAM,
        ARCHIVE_KEY,
        view,
        verified_spec(&spec_account),
        window,
    )
    .expect("once-verified archive view");
    assert_eq!(
        preflight_verified_archive(&terms, verified_view),
        Ok(candidate)
    );

    assert_eq!(candidate.terms(), terms.terms);
    assert_eq!(candidate.feed(), spec.feed_id());
    assert_eq!(candidate.window(), receipt.window());
    assert_eq!(candidate.archive_commitment(), receipt.page_commitment());
    assert_eq!(
        candidate.statistic(),
        STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06
    );
    assert_eq!(
        candidate.finalization(),
        NativeWindowFinalizationV1::ExactOnly
    );
    assert_eq!(
        (candidate.start_bucket(), candidate.end_bucket_exclusive()),
        (100, 102)
    );
    assert_eq!(
        (
            candidate.sample_count(),
            candidate.coverage_count(),
            candidate.gap_count()
        ),
        (2, 2, 0)
    );
    assert_eq!(candidate.vector().denominator, 7);
    assert_eq!(&candidate.vector().weights[..4], &[2, 4, 1, 0]);
    assert_eq!(candidate.basis_evaluator_version(), 1);
    assert_eq!(candidate.occupation_summary_version(), 1);
    assert_eq!(candidate.sealed_feed_cursor(), 103);
    assert_eq!(candidate.repair_generation(), 6);
}

#[test]
fn canonical_positive_width_archive_is_not_midpoint_selected() {
    let (spec, spec_account, archive, window) = complete_archive(4, 4, 1);
    let terms = occupation_terms(spec, STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07);
    let (receipt, view) = receipt(&spec_account, &archive, window);
    assert_eq!(
        preflight_sealed_archive(&terms, receipt, view),
        Err(NativeWindowError::NonPointObservation)
    );
}

#[test]
fn canonical_archive_selects_only_the_terms_named_finalizer() {
    let (spec, spec_account, archive, window) = complete_archive(1, 4, 0);
    let (receipt, view) = receipt(&spec_account, &archive, window);
    let exact = occupation_terms(spec, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
    assert!(matches!(
        preflight_sealed_archive(&exact, receipt, view),
        Err(NativeWindowError::Accumulator(_))
    ));

    let rounded = occupation_terms(spec, STAT_QUANTIZED_BASIS_OCCUPATION_LARGEST_REMAINDER_07);
    let candidate =
        preflight_sealed_archive(&rounded, receipt, view).expect("named largest remainder");
    assert_eq!(
        candidate.finalization(),
        NativeWindowFinalizationV1::LargestRemainderV1
    );
    assert_eq!(&candidate.vector().weights[..4], &[4, 3, 0, 0]);
}

#[test]
fn receipt_cannot_be_replayed_over_mutated_archive_bytes() {
    let (spec, spec_account, mut archive, window) = complete_archive(4, 4, 0);
    let terms = occupation_terms(spec, STAT_QUANTIZED_BASIS_OCCUPATION_EXACT_06);
    let (receipt, _) = receipt(&spec_account, &archive, window);
    archive[512 + 8] ^= 1;
    let mutated = ArchiveAccountViewV1::new(ARCHIVE_KEY, CLUTCH_PROGRAM, false, &archive);
    assert_eq!(
        preflight_sealed_archive(&terms, receipt, mutated),
        Err(NativeWindowError::Archive(
            SourceArchiveError::CommitmentMismatch
        ))
    );
}

fn deployment_bytes() -> [u8; 16] {
    let mut out = [0_u8; 16];
    out[..8].copy_from_slice(b"MOCKDEP1");
    out[8..].copy_from_slice(&DEPLOYMENT_GENERATION.to_le_bytes());
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
