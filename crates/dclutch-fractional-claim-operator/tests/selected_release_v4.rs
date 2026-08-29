//! One four-action selected Fractional release: derived frames, joined
//! ProgramSetV2, canonical publication.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use dclutch_capability_program_contract::set_v2::{CapabilityProgramSetV2, SelectorWidthV2};
use dclutch_claims_svm::{
    frame_spec_v1::SignedDeltaFrameSpecV3,
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3, TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3,
    },
};
use dclutch_fractional_claim_contract::{
    FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3, FRACTIONAL_ATOMIC_ACTOR_V3,
    FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3, FRACTIONAL_ATOMIC_ROOT_V3, FRACTIONAL_ATOMIC_SHARD_MINT_V3,
    FRACTIONAL_ATOMIC_TERMS_STAGING_V3, FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3,
    FRACTIONAL_CAPABILITY_ROOT_BYTES_V4, FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2,
    FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2, FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3,
    FRACTIONAL_TERMINAL_ACTOR_V3, FRACTIONAL_TERMINAL_ROOT_V3, FRACTIONAL_TERMINAL_SHARD_MINT_V3,
    FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3, FRACTIONAL_TERMINAL_TERMS_RAW_V3,
    FRACTIONAL_TERMINAL_TERMS_STAGING_V3, FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
    FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3, FractionalExposureActionV2,
};
use dclutch_fractional_claim_kernel::{
    FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsInputV2, FractionalExposureTermsV2, encode_fractional_exposure_terms_v2,
    fractional_exposure_terms_bytes_v2,
};
use dclutch_fractional_claim_operator::{
    FRACTIONAL_SELECTED_ACTION_COUNT_V4, FRACTIONAL_SELECTED_ACTIONS_V4,
    FRACTIONAL_SELECTED_PUBLICATION_BYTES_V4, FRACTIONAL_SELECTED_PUBLICATION_MAGIC_V4,
    FractionalFrameWidthsV4, FractionalSelectedReleaseErrorV4, FractionalSelectedReleaseInputV4,
    fractional_claims_frame_spec_v4, fractional_selected_release_v4,
    validate_fractional_selected_release_v4,
};
use dclutch_token_svm::TOKEN_2022_PROGRAM_ID;

const RELEASE: [u8; 32] = [1; 32];
const MARKET: [u8; 32] = [2; 32];
const PRODUCT: [u8; 32] = [3; 32];
const DOMAIN: [u8; 32] = [4; 32];
const TERMS: [u8; 32] = [5; 32];
const TOKEN_BEHAVIOR: [u8; 32] = [6; 32];
const EXPOSURE: [u8; 32] = [7; 32];
const CAPACITY: [u8; 32] = [12; 32];
const MINTS: [[u8; 32]; 3] = [[21; 32], [22; 32], [23; 32]];
const PRODUCT_WIDTH: u32 = 258;
const DENOMINATOR: u64 = 10;

fn encoded_terms() -> Vec<u8> {
    let width = fractional_exposure_terms_bytes_v2(MINTS.len()).unwrap();
    let mut scratch = vec![0; width];
    let mut output = vec![0; width];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market: MARKET,
            product_record: PRODUCT,
            result_domain: DOMAIN,
            release_set: RELEASE,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: TOKEN_BEHAVIOR,
            exposure_id: EXPOSURE,
            product_basis: [32; 32],
            representation_basis: [33; 32],
            graph_id: [34; 32],
            product_width: PRODUCT_WIDTH,
            denominator: DENOMINATOR,
            shard_mints: &MINTS,
        },
        &mut scratch,
        &mut output,
    )
    .unwrap();
    output
}

fn terms(bytes: &[u8]) -> FractionalExposureTermsV2<'_> {
    FractionalExposureTermsV2::decode(
        bytes,
        FractionalExposureTermsAdmissionV2 {
            selected_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            finalized_schema_id: FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            selected_terms_id: TERMS,
            finalized_terms_id: TERMS,
            recomputed_terms_digest: TERMS,
            finalized_terms_digest: TERMS,
            record_authenticated: true,
        },
    )
    .unwrap()
}

fn widths() -> FractionalFrameWidthsV4 {
    FractionalFrameWidthsV4 {
        linked_basis_record: 96,
        product_record: 128,
        result_domain_record: 112,
        portfolio_record: 104,
        selected_config: 64,
        core_market: 256,
        activation_cache: 320,
        rent_credit: 48,
    }
}

fn input(bytes: &[u8]) -> FractionalSelectedReleaseInputV4<'_> {
    FractionalSelectedReleaseInputV4 {
        terms: terms(bytes),
        capacity_profile: CAPACITY,
        widths: widths(),
    }
}

fn probe(action: FractionalExposureActionV2) -> [u8; FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2] {
    let mut request = [0_u8; FRACTIONAL_EXPOSURE_REQUEST_BYTES_V2];
    request[FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2] = action.byte();
    request
}

#[test]
fn the_four_executable_actions_publish_one_action_selected_program_set() {
    let bytes = encoded_terms();
    let release = fractional_selected_release_v4(input(&bytes)).unwrap();
    validate_fractional_selected_release_v4(&release, input(&bytes)).unwrap();

    assert_eq!(release.bundles.len(), FRACTIONAL_SELECTED_ACTION_COUNT_V4);
    for (index, action) in FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied().enumerate() {
        assert_eq!(release.bundles[index].action, action);
    }

    let set = CapabilityProgramSetV2::decode(&release.program_set).unwrap();
    assert_eq!(
        usize::try_from(set.selector_offset()).unwrap(),
        FRACTIONAL_EXPOSURE_REQUEST_ACTION_OFFSET_V2
    );
    assert_eq!(set.selector_width(), SelectorWidthV2::U8);
    assert_eq!(
        usize::from(set.entry_count()),
        FRACTIONAL_SELECTED_ACTION_COUNT_V4
    );
    assert_eq!(set.entry(0).unwrap().selector(), 0);
    assert_eq!(set.entry(1).unwrap().selector(), 2);
    assert_eq!(set.entry(2).unwrap().selector(), 3);
    assert_eq!(set.entry(3).unwrap().selector(), 4);

    // Every published action selects its own distinct descriptor, and the three
    // unpublished actions select nothing at all.
    let mut seen = Vec::new();
    for (index, action) in FRACTIONAL_SELECTED_ACTIONS_V4.iter().copied().enumerate() {
        let selected = set.select_descriptor(&probe(action)).unwrap();
        assert_eq!(
            selected.program().to_bytes(),
            release.publication.descriptors[index]
        );
        assert!(!seen.contains(&selected.program().to_bytes()));
        seen.push(selected.program().to_bytes());
    }
    for absent in [
        FractionalExposureActionV2::Transfer,
        FractionalExposureActionV2::Terminalize,
        FractionalExposureActionV2::ZeroSupplyRetire,
    ] {
        assert!(set.select_descriptor(&probe(absent)).is_err());
    }

    let publication = release.publication;
    assert_eq!(publication.release_set, RELEASE);
    assert_eq!(publication.market, MARKET);
    assert_eq!(publication.product_record, PRODUCT);
    assert_eq!(publication.result_domain, DOMAIN);
    assert_eq!(publication.terms, TERMS);
    assert_eq!(publication.token_behavior, TOKEN_BEHAVIOR);
    assert_eq!(publication.exposure, EXPOSURE);
    assert_eq!(publication.token_program, TOKEN_2022_PROGRAM_ID);
    assert_eq!(publication.capacity_profile, CAPACITY);
    assert_eq!(publication.program_set_id, release.program_set_id);
    assert_eq!(publication.denominator, DENOMINATOR);
    assert_eq!(publication.product_width, PRODUCT_WIDTH);
    assert_eq!(
        publication.representation_width,
        u32::try_from(MINTS.len()).unwrap()
    );
    let encoded = publication.to_bytes();
    assert_eq!(encoded.len(), FRACTIONAL_SELECTED_PUBLICATION_BYTES_V4);
    assert_eq!(encoded[..8], FRACTIONAL_SELECTED_PUBLICATION_MAGIC_V4);
    assert_ne!(publication.publication_id(), [0; 32]);
}

#[test]
fn the_derived_frames_are_the_frames_the_claims_child_actually_demands() {
    let bytes = encoded_terms();
    let value = terms(&bytes);

    let atomic =
        fractional_claims_frame_spec_v4(FractionalExposureActionV2::Wrap, value, widths()).unwrap();
    assert_eq!(atomic.len(), FRACTIONAL_ATOMIC_ACCOUNT_COUNT_V3);
    assert_eq!(
        fractional_claims_frame_spec_v4(FractionalExposureActionV2::WholeUnwrap, value, widths())
            .unwrap(),
        atomic
    );

    // The signed-delta prefix is not restated here; it is read back out of the
    // Claims frame contract, so a privilege drift in Claims fails this test.
    let spec = SignedDeltaFrameSpecV3::new(2).unwrap();
    for index in 0..spec.account_count().unwrap() {
        let expected = spec.account(index).unwrap().privileges();
        let observed = atomic[usize::from(index)];
        assert_eq!(observed.signer, expected.signer());
        assert_eq!(observed.writable, expected.writable());
        assert_eq!(observed.executable, expected.executable());
    }

    // The Fractional tail matches build_fractional_atomic_claims_instruction_v3.
    assert!(atomic[FRACTIONAL_ATOMIC_ROOT_V3].signer);
    assert!(atomic[FRACTIONAL_ATOMIC_ROOT_V3].writable);
    assert_eq!(
        atomic[FRACTIONAL_ATOMIC_ROOT_V3].data_length,
        u32::try_from(FRACTIONAL_CAPABILITY_ROOT_BYTES_V4).unwrap()
    );
    assert!(atomic[FRACTIONAL_ATOMIC_ACTOR_V3].signer);
    assert!(!atomic[FRACTIONAL_ATOMIC_ACTOR_V3].writable);
    assert!(atomic[FRACTIONAL_ATOMIC_SHARD_MINT_V3].writable);
    assert!(atomic[FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3].writable);
    assert!(atomic[FRACTIONAL_ATOMIC_TOKEN_PROGRAM_V3].executable);

    // A vacant staging cursor is an exact zero width, never opaque: Claims
    // requires it to be empty and the profile must say so.
    assert!(!atomic[FRACTIONAL_ATOMIC_TERMS_STAGING_V3].opaque_data);
    assert_eq!(atomic[FRACTIONAL_ATOMIC_TERMS_STAGING_V3].data_length, 0);

    // A wallet, Mint, or Token account width is not knowable by a release.
    for coordinate in [
        FRACTIONAL_ATOMIC_ACTOR_V3,
        FRACTIONAL_ATOMIC_SHARD_MINT_V3,
        FRACTIONAL_ATOMIC_HOLDER_TOKEN_V3,
    ] {
        assert!(atomic[coordinate].opaque_data);
        assert_eq!(atomic[coordinate].data_length, 0);
    }

    let terminal = fractional_claims_frame_spec_v4(
        FractionalExposureActionV2::TerminalRedeem,
        value,
        widths(),
    )
    .unwrap();
    assert_eq!(terminal.len(), FRACTIONAL_TERMINAL_ACCOUNT_COUNT_V3);
    assert_eq!(
        fractional_claims_frame_spec_v4(
            FractionalExposureActionV2::TerminalZeroBurn,
            value,
            widths()
        )
        .unwrap(),
        terminal
    );
    // The full Fractional terminal tail, matched against exactly what
    // authenticate_terminal_tail_privileges in the Claims handler demands.
    for coordinate in [
        FRACTIONAL_TERMINAL_TERMS_RAW_V3,
        FRACTIONAL_TERMINAL_TERMS_STAGING_V3,
        FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_RAW_V3,
        FRACTIONAL_TERMINAL_TOKEN_BEHAVIOR_STAGING_V3,
    ] {
        assert!(!terminal[coordinate].writable);
        assert!(!terminal[coordinate].signer);
        assert!(!terminal[coordinate].executable);
    }
    assert!(terminal[FRACTIONAL_TERMINAL_ROOT_V3].signer);
    assert!(terminal[FRACTIONAL_TERMINAL_ROOT_V3].writable);
    assert!(!terminal[FRACTIONAL_TERMINAL_ROOT_V3].executable);
    assert_eq!(
        terminal[FRACTIONAL_TERMINAL_ROOT_V3].data_length,
        u32::try_from(FRACTIONAL_CAPABILITY_ROOT_BYTES_V4).unwrap()
    );
    assert!(terminal[FRACTIONAL_TERMINAL_ACTOR_V3].signer);
    assert!(!terminal[FRACTIONAL_TERMINAL_ACTOR_V3].writable);
    assert!(!terminal[FRACTIONAL_TERMINAL_ACTOR_V3].executable);
    assert!(terminal[FRACTIONAL_TERMINAL_SHARD_MINT_V3].writable);
    assert!(!terminal[FRACTIONAL_TERMINAL_SHARD_MINT_V3].signer);
    assert!(terminal[FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3].writable);
    assert!(!terminal[FRACTIONAL_TERMINAL_SOURCE_TOKEN_V3].signer);

    // The terminal settlement span carries the Custody composition, so its
    // three program coordinates must be executable and nothing else may be.
    for coordinate in [
        TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3,
    ] {
        assert!(terminal[coordinate].executable);
        assert!(!terminal[coordinate].writable);
        assert!(!terminal[coordinate].signer);
    }
    assert!(terminal[TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3].writable);
    assert!(terminal[TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3].writable);
    assert!(terminal[TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3].writable);

    // The Product-width tail is resolved from the authenticated terms, so a
    // 258-outcome Market produces a wider Position rule than a 2-outcome one.
    let narrow_bytes = {
        let width = fractional_exposure_terms_bytes_v2(MINTS.len()).unwrap();
        let mut scratch = vec![0; width];
        let mut output = vec![0; width];
        encode_fractional_exposure_terms_v2(
            FractionalExposureTermsInputV2 {
                market: MARKET,
                product_record: PRODUCT,
                result_domain: DOMAIN,
                release_set: RELEASE,
                token_program: TOKEN_2022_PROGRAM_ID,
                token_behavior: TOKEN_BEHAVIOR,
                exposure_id: EXPOSURE,
                product_basis: [32; 32],
                representation_basis: [33; 32],
                graph_id: [34; 32],
                product_width: 2,
                denominator: DENOMINATOR,
                shard_mints: &MINTS,
            },
            &mut scratch,
            &mut output,
        )
        .unwrap();
        output
    };
    let narrow = fractional_claims_frame_spec_v4(
        FractionalExposureActionV2::Wrap,
        terms(&narrow_bytes),
        widths(),
    )
    .unwrap();
    let position = usize::from(spec.account_count().unwrap()) - 1;
    assert!(atomic[position].data_length > narrow[position].data_length);
    assert_eq!(
        atomic[position].data_length - narrow[position].data_length,
        8 * (PRODUCT_WIDTH - 2)
    );
}

#[test]
fn unsupported_actions_zero_widths_and_unusable_terms_refuse_before_compiling() {
    let bytes = encoded_terms();
    let value = terms(&bytes);

    for absent in [
        FractionalExposureActionV2::Transfer,
        FractionalExposureActionV2::Terminalize,
        FractionalExposureActionV2::ZeroSupplyRetire,
    ] {
        assert_eq!(
            fractional_claims_frame_spec_v4(absent, value, widths()),
            Err(FractionalSelectedReleaseErrorV4::Frame)
        );
    }

    let mut zeroed = widths();
    zeroed.core_market = 0;
    assert_eq!(
        fractional_claims_frame_spec_v4(FractionalExposureActionV2::Wrap, value, zeroed),
        Err(FractionalSelectedReleaseErrorV4::Widths)
    );
    let mut zero_release = input(&bytes);
    zero_release.widths = zeroed;
    assert_eq!(
        fractional_selected_release_v4(zero_release).unwrap_err(),
        FractionalSelectedReleaseErrorV4::Widths
    );

    let mut zero_capacity = input(&bytes);
    zero_capacity.capacity_profile = [0; 32];
    assert_eq!(
        fractional_selected_release_v4(zero_capacity).unwrap_err(),
        FractionalSelectedReleaseErrorV4::Terms
    );

    let unit_denominator = {
        let width = fractional_exposure_terms_bytes_v2(MINTS.len()).unwrap();
        let mut scratch = vec![0; width];
        let mut output = vec![0; width];
        encode_fractional_exposure_terms_v2(
            FractionalExposureTermsInputV2 {
                market: MARKET,
                product_record: PRODUCT,
                result_domain: DOMAIN,
                release_set: RELEASE,
                token_program: TOKEN_2022_PROGRAM_ID,
                token_behavior: TOKEN_BEHAVIOR,
                exposure_id: EXPOSURE,
                product_basis: [32; 32],
                representation_basis: [33; 32],
                graph_id: [34; 32],
                product_width: PRODUCT_WIDTH,
                denominator: 1,
                shard_mints: &MINTS,
            },
            &mut scratch,
            &mut output,
        )
    };
    // A denominator of one is refused by the terms codec itself; if a future
    // codec admits it, the release must still refuse.
    if unit_denominator.is_ok() {
        panic!("terms codec unexpectedly admitted denominator 1");
    }
}

#[test]
fn substituted_bundles_program_set_bytes_and_publication_identities_refuse() {
    let bytes = encoded_terms();
    let release = fractional_selected_release_v4(input(&bytes)).unwrap();

    let mut flipped_set = release.clone();
    flipped_set.program_set[0] ^= 1;
    assert_eq!(
        validate_fractional_selected_release_v4(&flipped_set, input(&bytes)).unwrap_err(),
        FractionalSelectedReleaseErrorV4::ProgramSet
    );

    let mut reordered = release.clone();
    reordered.bundles.swap(0, 1);
    assert_eq!(
        validate_fractional_selected_release_v4(&reordered, input(&bytes)).unwrap_err(),
        FractionalSelectedReleaseErrorV4::Bundle
    );

    let mut substituted_descriptor = release.clone();
    substituted_descriptor.bundles[2].descriptor[0] ^= 1;
    assert_eq!(
        validate_fractional_selected_release_v4(&substituted_descriptor, input(&bytes))
            .unwrap_err(),
        FractionalSelectedReleaseErrorV4::Bundle
    );

    let mut substituted_effect = release.clone();
    substituted_effect.bundles[0].effect[0] ^= 1;
    assert_eq!(
        validate_fractional_selected_release_v4(&substituted_effect, input(&bytes)).unwrap_err(),
        FractionalSelectedReleaseErrorV4::Bundle
    );

    let mut substituted_market = release.clone();
    substituted_market.publication.market = [0x77; 32];
    assert_eq!(
        validate_fractional_selected_release_v4(&substituted_market, input(&bytes)).unwrap_err(),
        FractionalSelectedReleaseErrorV4::Publication
    );

    let mut substituted_descriptor_row = release.clone();
    substituted_descriptor_row.publication.descriptors[3] = [0x66; 32];
    assert_eq!(
        validate_fractional_selected_release_v4(&substituted_descriptor_row, input(&bytes))
            .unwrap_err(),
        FractionalSelectedReleaseErrorV4::Publication
    );

    // A release compiled against different widths is a different release even
    // though every artifact in it is individually well formed.
    let mut other = widths();
    other.core_market += 8;
    let mut rewidened = input(&bytes);
    rewidened.widths = other;
    assert_eq!(
        validate_fractional_selected_release_v4(&release, rewidened).unwrap_err(),
        FractionalSelectedReleaseErrorV4::Bundle
    );
}
