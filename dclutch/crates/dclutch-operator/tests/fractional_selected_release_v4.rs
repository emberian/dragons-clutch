//! One four-action selected Fractional release: derived frames, joined
//! ProgramSetV2, canonical publication.

#![allow(clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]

use dclutch_claims::fractional::{
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
use dclutch_claims::fractional_kernel::{
    Error as KernelError, FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
    FRACTIONAL_SELECTION_CONFIG_BYTES_V1, FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1,
    FRACTIONAL_SELECTION_CONFIG_SCHEMA_PREIMAGE_V1, FractionalExposureTermsAdmissionV2,
    FractionalExposureTermsInputV2, FractionalExposureTermsV2, FractionalSelectionConfigInputV1,
    FractionalSelectionConfigV1, encode_fractional_exposure_terms_v2,
    encode_fractional_selection_config_v1, fractional_exposure_terms_bytes_v2,
    fractional_selection_config_from_terms_v1, join_fractional_selection_config_v1,
};
use dclutch_claims::{
    frame_spec_v1::SignedDeltaFrameSpecV3,
    terminal_settlement_v3::{
        TERMINAL_SETTLEMENT_CUSTODY_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_CUSTODY_REPLAY_ACCOUNT_V3, TERMINAL_SETTLEMENT_HOARD_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RECIPIENT_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_RESOLUTION_PROGRAM_ACCOUNT_V3,
        TERMINAL_SETTLEMENT_TOKEN_PROGRAM_ACCOUNT_V3,
    },
};
use dclutch_custody::token_svm::TOKEN_2022_PROGRAM_ID;
use dclutch_market::capability_program::{
    set_v2::{CapabilityProgramSetV2, SelectorWidthV2},
    v4::CapabilityProgramV4,
};
use dclutch_operator::fractional::{
    FRACTIONAL_ACTIVATION_SELECTOR_V1, FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4,
    FRACTIONAL_SELECTED_ACTION_COUNT_V4, FRACTIONAL_SELECTED_ACTIONS_V4,
    FRACTIONAL_SELECTED_PUBLICATION_BYTES_V4, FRACTIONAL_SELECTED_PUBLICATION_MAGIC_V4,
    FractionalFrameWidthsV4, FractionalSelectedReleaseErrorV4, FractionalSelectedReleaseInputV4,
    fractional_activation_request_v1, fractional_claims_frame_spec_v4,
    fractional_current_release_v4, fractional_selected_release_v4,
    validate_fractional_current_release_v4, validate_fractional_selected_release_v4,
};

const RELEASE: [u8; 32] = [1; 32];
const MARKET: [u8; 32] = [2; 32];
const PRODUCT: [u8; 32] = [3; 32];
const DOMAIN: [u8; 32] = [4; 32];
const TOKEN_BEHAVIOR: [u8; 32] = [6; 32];
/// Exposure identity, market-derived the way the chain derives it.
///
/// NOT a constant, and the reason is the finding this fixture exists to keep
/// honest. `exposure_id` is the content id of a `CompositionExposureBundleV3`,
/// and that record carries the Market at byte 16
/// (`COMPOSITION_EXPOSURE_MARKET_OFFSET_V3`), pinned equal to `terms.market()`
/// by `check_fractional_exposure_bundle_v2`. So on a real chain the exposure
/// identity MOVES when the Market moves.
///
/// A fixture that held it constant made the two-market closure control below
/// pass while the config still named a market-derived leaf -- a byte-identity
/// control is only as strong as the set of things it lets vary, and holding a
/// market-derived leaf fixed is exactly how one goes quietly vacuous. Modelling
/// the derivation is what makes the control able to fail.
fn exposure_id_for_market(market: [u8; 32]) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(b"fixture/composition-exposure-bundle-v3");
    hasher.update(market);
    hasher.update(GRAPH);
    hasher.finalize().into()
}

/// Stable source graph identity -- genuinely market-free.
const GRAPH: [u8; 32] = [34; 32];
const CAPACITY: [u8; 32] = [12; 32];
const MINTS: [[u8; 32]; 3] = [[21; 32], [22; 32], [23; 32]];
const PRODUCT_WIDTH: u32 = 258;
const DENOMINATOR: u64 = 10;

/// Terms identity, computed the way the chain computes it.
///
/// This was a fixture constant until the config split needed two terms records
/// that differ only in their Market. A hardcoded id made two genuinely
/// different terms records share one identity -- harmless for the tests that
/// existed, but it is also exactly the shape of a false green, and the
/// admission's own contract says `recomputed_terms_digest` is SHA-256 over the
/// exact terms bytes. Deriving it makes the fixture tell the truth.
fn terms_id_of(bytes: &[u8]) -> [u8; 32] {
    use sha2::{Digest as _, Sha256};
    Sha256::digest(bytes).into()
}

fn encoded_terms() -> Vec<u8> {
    encoded_terms_for_market(MARKET)
}

/// The same terms in every field except the Market it binds.
///
/// The two-market closure control's whole force comes from this being the
/// ONLY difference between its two inputs.
fn encoded_terms_for_market(market: [u8; 32]) -> Vec<u8> {
    let width = fractional_exposure_terms_bytes_v2(MINTS.len()).unwrap();
    let mut scratch = vec![0; width];
    let mut output = vec![0; width];
    encode_fractional_exposure_terms_v2(
        FractionalExposureTermsInputV2 {
            market,
            product_record: PRODUCT,
            result_domain: DOMAIN,
            release_set: RELEASE,
            token_program: TOKEN_2022_PROGRAM_ID,
            token_behavior: TOKEN_BEHAVIOR,
            exposure_id: exposure_id_for_market(market),
            product_basis: [32; 32],
            representation_basis: [33; 32],
            graph_id: GRAPH,
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
            selected_terms_id: terms_id_of(bytes),
            finalized_terms_id: terms_id_of(bytes),
            recomputed_terms_digest: terms_id_of(bytes),
            finalized_terms_digest: terms_id_of(bytes),
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
    assert_eq!(publication.terms, terms_id_of(&bytes));
    assert_eq!(publication.token_behavior, TOKEN_BEHAVIOR);
    assert_eq!(publication.exposure, exposure_id_for_market(MARKET));
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
fn the_current_release_appends_activation_without_moving_legacy_action_bytes() {
    let bytes = encoded_terms();
    let historical = fractional_selected_release_v4(input(&bytes)).unwrap();
    let current = fractional_current_release_v4(input(&bytes)).unwrap();
    validate_fractional_current_release_v4(&current, input(&bytes)).unwrap();

    // The legacy constructor remains the historical four-action wire exactly.
    assert_eq!(
        fractional_selected_release_v4(input(&bytes)).unwrap(),
        historical
    );
    let set = CapabilityProgramSetV2::decode(&current.program_set).unwrap();
    assert_eq!(
        usize::from(set.entry_count()),
        FRACTIONAL_SELECTED_ACTION_COUNT_V4 + 1
    );
    let activation = set
        .select_descriptor(&fractional_activation_request_v1())
        .unwrap();
    assert_eq!(
        set.entry(u16::try_from(FRACTIONAL_SELECTED_ACTION_COUNT_V4).unwrap())
            .unwrap()
            .selector(),
        FRACTIONAL_ACTIVATION_SELECTOR_V1
    );
    assert_eq!(
        activation.program().to_bytes(),
        current.activation.descriptor_id
    );

    let records = current.publication_records().unwrap();
    assert_eq!(
        records.len(),
        2 + FRACTIONAL_SELECTED_ACTION_COUNT_V4 * 7 + 3
    );
    assert_eq!(
        records[records.len() - 3].label,
        "activation-account-profile"
    );
    assert_eq!(records[records.len() - 2].label, "activation-effect");
    assert_eq!(records[records.len() - 1].label, "activation-descriptor");

    let mut hostile = current.clone();
    hostile.activation.effect[0] ^= 1;
    assert_eq!(
        validate_fractional_current_release_v4(&hostile, input(&bytes)),
        Err(
            FractionalSelectedReleaseErrorV4::FractionalActivationBundle(
                dclutch_operator::fractional::FractionalActivationBundleErrorV1::Template(
                    dclutch_market::capability_activation::ActivationBundleErrorV1::Descriptor
                )
            )
        )
    );
    let mut hostile = current.clone();
    hostile.selection_config[0] ^= 1;
    assert_eq!(
        validate_fractional_current_release_v4(&hostile, input(&bytes)),
        Err(FractionalSelectedReleaseErrorV4::SelectionConfig)
    );
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
                exposure_id: exposure_id_for_market(MARKET),
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
                exposure_id: exposure_id_for_market(MARKET),
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
        FractionalSelectedReleaseErrorV4::FractionalSelectedArtifact(
            dclutch_operator::fractional::FractionalSelectedArtifactErrorV4::CapabilityProgram(
                dclutch_market::capability_program::Error::InvalidMagic
            )
        )
    );

    let mut substituted_effect = release.clone();
    substituted_effect.bundles[0].effect[0] ^= 1;
    assert_eq!(
        validate_fractional_selected_release_v4(&substituted_effect, input(&bytes)).unwrap_err(),
        FractionalSelectedReleaseErrorV4::FractionalSelectedArtifact(
            dclutch_operator::fractional::FractionalSelectedArtifactErrorV4::EffectV4(
                dclutch_vm::effect::v4::ErrorV4::Wire
            )
        )
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

/// A release refuses to publish a capability it could never settle.
///
/// The four published actions include the two terminal ones, so a Fractional
/// capability whose representation is wider than a terminal settlement can
/// execute is a capability that admits wraps and forbids exits. The open-market
/// actions do no Product evaluation and would happily accept that Market, so
/// nothing downstream refuses until holders try to redeem. This is the earliest
/// point the trap can be closed, and it is closed by refusing to compile.
#[test]
fn a_representation_wider_than_a_terminal_settlement_is_refused_at_publication() {
    let mints: Vec<[u8; 32]> = (0..=FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4)
        .map(|index| {
            let mut bytes = [0x21_u8; 32];
            bytes[0..4].copy_from_slice(&index.to_le_bytes());
            bytes
        })
        .collect();
    assert_eq!(
        u32::try_from(mints.len()).unwrap(),
        FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4 + 1
    );
    let width = fractional_exposure_terms_bytes_v2(mints.len()).unwrap();
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
            exposure_id: exposure_id_for_market(MARKET),
            product_basis: [32; 32],
            representation_basis: [33; 32],
            graph_id: GRAPH,
            product_width: PRODUCT_WIDTH,
            denominator: DENOMINATOR,
            shard_mints: &mints,
        },
        &mut scratch,
        &mut output,
    )
    .expect("the terms codec itself admits this width");

    // The terms are canonical -- this is not a codec refusal being borrowed.
    let over = FractionalSelectedReleaseInputV4 {
        terms: terms(&output),
        capacity_profile: CAPACITY,
        widths: widths(),
    };
    assert_eq!(
        fractional_selected_release_v4(over).unwrap_err(),
        FractionalSelectedReleaseErrorV4::Unsettleable
    );

    // And the widest settleable representation still publishes.
    let at_bound = &mints[..usize::try_from(FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4).unwrap()];
    let width = fractional_exposure_terms_bytes_v2(at_bound.len()).unwrap();
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
            exposure_id: exposure_id_for_market(MARKET),
            product_basis: [32; 32],
            representation_basis: [33; 32],
            graph_id: GRAPH,
            product_width: PRODUCT_WIDTH,
            denominator: DENOMINATOR,
            shard_mints: at_bound,
        },
        &mut scratch,
        &mut output,
    )
    .unwrap();
    let release = fractional_selected_release_v4(FractionalSelectedReleaseInputV4 {
        terms: terms(&output),
        capacity_profile: CAPACITY,
        widths: widths(),
    })
    .expect("the widest settleable representation must still publish");
    assert_eq!(
        release.publication.representation_width,
        FRACTIONAL_MAX_SETTLEABLE_WIDTH_V4
    );
}

/// A Market address must not reach ANY coordinate a capability manifest entry
/// carries — the complete closure, not just the config.
///
/// This is the executable form of the seam invariant, and it is deliberately
/// stated over the FULL closure rather than over the config alone. A
/// config-only check false-passes Rational, whose trap was in its RELEASE_ID
/// (a compact effect baking per-Market custody-owner PDAs into descriptor
/// bytes), so a Fractional control that only looked at the config would prove
/// nothing about the property that actually matters.
///
/// The seam (`selected_capability.rs::selected_manifest_entry_v1`) authors an
/// entry from exactly six identity coordinates. All six are asserted here:
///
/// | coordinate | source |
/// |---|---|
/// | `kind_id` | selected descriptor |
/// | `capacity_profile` | selected descriptor |
/// | `root_schema` | selected descriptor |
/// | `derivation_policy` | selected descriptor |
/// | `release_id` | `SHA-256(program_set)` |
/// | `config_id` | `SHA-256(selection config)` |
///
/// Two releases are compiled from terms differing in the Market and NOTHING
/// else. Every one of the six must be byte-identical, and so must every
/// artifact body underneath them — a descriptor that moved would move the
/// program set, and a program set that moved would move the release identity.
///
/// The final assertion is the anti-vacuity guard and is not decoration: the
/// two publications MUST differ, because they name their (different) Markets.
/// Without it this test would still pass if both halves had been compiled from
/// the same terms, which is exactly how a byte-identity control goes quietly
/// vacuous.
#[test]
fn no_manifest_entry_coordinate_moves_when_only_the_market_moves() {
    const OTHER_MARKET: [u8; 32] = [0xd7; 32];
    assert_ne!(MARKET, OTHER_MARKET);

    let first_bytes = encoded_terms_for_market(MARKET);
    let second_bytes = encoded_terms_for_market(OTHER_MARKET);
    assert_ne!(
        first_bytes, second_bytes,
        "the two fixtures must really differ, or the control proves nothing"
    );

    let first = fractional_selected_release_v4(input(&first_bytes)).unwrap();
    let second = fractional_selected_release_v4(input(&second_bytes)).unwrap();

    // config_id — the coordinate the split exists to free.
    assert_eq!(
        first.selection_config, second.selection_config,
        "the manifest-named config must be market-free"
    );
    assert_eq!(first.selection_config_id, second.selection_config_id);

    // release_id, and every artifact byte underneath it.
    assert_eq!(
        first.program_set, second.program_set,
        "the ProgramSetV2 bytes must be market-free"
    );
    assert_eq!(first.program_set_id, second.program_set_id);
    assert_eq!(
        first.bundles, second.bundles,
        "every compiled artifact body must be market-free"
    );

    // The four coordinates the seam reads off the selected descriptor, read
    // the same way the seam reads them rather than assumed from the bundle
    // equality above.
    for index in 0..FRACTIONAL_SELECTED_ACTION_COUNT_V4 {
        let left = CapabilityProgramV4::decode(&first.bundles[index].descriptor).unwrap();
        let right = CapabilityProgramV4::decode(&second.bundles[index].descriptor).unwrap();
        assert_eq!(left.kind(), right.kind());
        assert_eq!(left.capacity_profile(), right.capacity_profile());
        assert_eq!(left.root_schema(), right.root_schema());
        assert_eq!(left.derivation_policy(), right.derivation_policy());
        assert_eq!(
            left.config_schema().to_bytes(),
            FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1,
            "the descriptor must name the market-free selection config, not the terms"
        );
    }

    // ANTI-VACUITY: the two releases really were compiled for different
    // Markets. The publication is not a manifest coordinate, so it is free to
    // carry the Market — and here it is what proves the inputs differed.
    assert_ne!(first.publication, second.publication);
    assert_eq!(first.publication.market, MARKET);
    assert_eq!(second.publication.market, OTHER_MARKET);
    assert_ne!(
        first.publication.terms, second.publication.terms,
        "the execution terms still bind the Market, which is why they cannot be the config"
    );
}

/// The manifest-named config carries no Market and nothing derived from one.
///
/// Byte-identity across two Markets (above) shows the Market does not reach
/// the config. This shows the stronger thing directly: neither Market's bytes
/// appear anywhere in the config record at all, so the config is not merely
/// insensitive to the Market — it does not contain it.
#[test]
fn the_selection_config_contains_no_market_bytes() {
    const OTHER_MARKET: [u8; 32] = [0xd7; 32];
    let bytes = encoded_terms_for_market(MARKET);
    let release = fractional_selected_release_v4(input(&bytes)).unwrap();

    assert_eq!(
        release.selection_config.len(),
        FRACTIONAL_SELECTION_CONFIG_BYTES_V1
    );
    for market in [MARKET, OTHER_MARKET] {
        assert!(
            !release
                .selection_config
                .windows(32)
                .any(|window| window == market),
            "a Market address must not appear in the selection config"
        );
    }

    // And it does carry exactly the market-free facts the ruling named.
    let config = FractionalSelectionConfigV1::decode(&release.selection_config).unwrap();
    assert_eq!(config.denominator(), DENOMINATOR);
    assert_eq!(config.product_width(), PRODUCT_WIDTH);
    assert_eq!(
        config.representation_width(),
        u32::try_from(MINTS.len()).unwrap()
    );
    assert_eq!(config.token_program().unwrap(), TOKEN_2022_PROGRAM_ID);
    assert_eq!(config.graph_id().unwrap(), GRAPH);
}

/// The schema identity is recomputed, never trusted as a pasted constant.
#[test]
fn the_selection_config_schema_id_is_the_digest_of_its_own_preimage() {
    use sha2::{Digest, Sha256};
    let recomputed: [u8; 32] =
        Sha256::digest(FRACTIONAL_SELECTION_CONFIG_SCHEMA_PREIMAGE_V1).into();
    assert_eq!(recomputed, FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1);
    assert_ne!(
        FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1,
        FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2
    );
}

/// A release whose config disagrees with its terms on a market-free field
/// refuses with the pinned code, at compile time.
#[test]
fn a_config_disagreeing_with_its_terms_refuses_with_the_pinned_code() {
    let bytes = encoded_terms();
    let mut release = fractional_selected_release_v4(input(&bytes)).unwrap();
    let honest = release.selection_config.clone();

    // A config built for a DIFFERENT instrument: same shape, other denominator.
    let mut foreign = vec![0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(
        FractionalSelectionConfigInputV1 {
            token_program: TOKEN_2022_PROGRAM_ID,
            graph_id: GRAPH,
            product_width: PRODUCT_WIDTH,
            representation_width: u32::try_from(MINTS.len()).unwrap(),
            denominator: DENOMINATOR + 1,
        },
        &mut foreign,
    )
    .unwrap();
    assert_ne!(foreign, honest);

    use sha2::{Digest as _, Sha256};
    release.selection_config = foreign;
    release.selection_config_id = Sha256::digest(&release.selection_config).into();
    assert_eq!(
        validate_fractional_selected_release_v4(&release, input(&bytes)),
        Err(FractionalSelectedReleaseErrorV4::SelectionConfig),
        "a substituted config must refuse with the pinned selection-config code"
    );
}

/// The kernel join refuses each market-free field independently, with one
/// pinned code — so a single field being checked cannot mask the rest.
#[test]
fn the_kernel_join_refuses_every_market_free_field_independently() {
    let bytes = encoded_terms();
    let terms_value = terms(&bytes);
    let honest = fractional_selection_config_from_terms_v1(terms_value);

    let mut buffer = vec![0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
    encode_fractional_selection_config_v1(honest, &mut buffer).unwrap();
    join_fractional_selection_config_v1(
        FractionalSelectionConfigV1::decode(&buffer).unwrap(),
        terms_value,
    )
    .expect("the honest projection must join");

    let mutations: [(&str, FractionalSelectionConfigInputV1); 5] = [
        (
            "token_program",
            FractionalSelectionConfigInputV1 {
                token_program: [0x77; 32],
                ..honest
            },
        ),
        (
            "graph_id",
            FractionalSelectionConfigInputV1 {
                graph_id: [0x79; 32],
                ..honest
            },
        ),
        (
            "product_width",
            FractionalSelectionConfigInputV1 {
                product_width: honest.product_width - 1,
                ..honest
            },
        ),
        (
            "representation_width",
            FractionalSelectionConfigInputV1 {
                representation_width: honest.representation_width + 1,
                ..honest
            },
        ),
        (
            "denominator",
            FractionalSelectionConfigInputV1 {
                denominator: honest.denominator + 1,
                ..honest
            },
        ),
    ];
    for (field, mutated) in mutations {
        let mut hostile = vec![0_u8; FRACTIONAL_SELECTION_CONFIG_BYTES_V1];
        encode_fractional_selection_config_v1(mutated, &mut hostile).unwrap();
        let decoded = FractionalSelectionConfigV1::decode(&hostile).unwrap();
        assert_eq!(
            join_fractional_selection_config_v1(decoded, terms_value),
            Err(KernelError::SelectionConfigMismatch),
            "a config disagreeing on {field} must refuse with the pinned join code"
        );
    }
}

/// The publication names every record a Registry must finalize, under schemas
/// read off the artifacts rather than restated -- and NOT the exposure terms.
///
/// The absence is the load-bearing assertion. The terms bind the Market, so a
/// founding that had to publish them could not be assembled before the Market
/// existed. Every record here is market-free, which is what lets the whole
/// publication be compiled ahead of the founding that selects it.
#[test]
fn the_publication_records_are_market_free_and_schema_faithful() {
    let bytes = encoded_terms();
    let release = fractional_selected_release_v4(input(&bytes)).unwrap();
    let records = release.publication_records().unwrap();

    // 2 shared + 7 per action.
    assert_eq!(records.len(), 2 + FRACTIONAL_SELECTED_ACTION_COUNT_V4 * 7);
    assert_eq!(records[0].label, "program-set");
    assert_eq!(records[1].label, "selection-config");
    assert_eq!(records[1].schema, FRACTIONAL_SELECTION_CONFIG_SCHEMA_ID_V1);
    assert_eq!(records[1].content_id(), release.selection_config_id);

    // No record is finalized under the TERMS schema, and no record body is the
    // terms bytes: the execution record is not part of the publication.
    for record in &records {
        assert_ne!(
            record.schema, FRACTIONAL_EXPOSURE_TERMS_SCHEMA_ID_V2,
            "the publication must not finalize anything as exposure terms"
        );
        assert_ne!(record.body, bytes.as_slice());
    }

    // Every descriptor record's content id is one the ProgramSet actually
    // selects, so a record cannot be published for a descriptor this release
    // does not name.
    let descriptors: Vec<[u8; 32]> = records
        .iter()
        .filter(|record| record.label == "descriptor")
        .map(|record| record.content_id())
        .collect();
    assert_eq!(descriptors.len(), FRACTIONAL_SELECTED_ACTION_COUNT_V4);
    for (index, descriptor) in descriptors.iter().enumerate() {
        assert_eq!(*descriptor, release.publication.descriptors[index]);
    }
}

/// The publication records do not move when only the Market moves -- the
/// closure control extended to the bytes a founding actually publishes.
#[test]
fn the_publication_records_do_not_move_with_the_market() {
    const OTHER_MARKET: [u8; 32] = [0xd7; 32];
    let first_bytes = encoded_terms_for_market(MARKET);
    let second_bytes = encoded_terms_for_market(OTHER_MARKET);
    let first = fractional_selected_release_v4(input(&first_bytes)).unwrap();
    let second = fractional_selected_release_v4(input(&second_bytes)).unwrap();

    let left = first.publication_records().unwrap();
    let right = second.publication_records().unwrap();
    assert_eq!(left.len(), right.len());
    for (a, b) in left.iter().zip(right.iter()) {
        assert_eq!(a.label, b.label);
        assert_eq!(a.schema, b.schema);
        assert_eq!(a.body, b.body, "record {} moved with the Market", a.label);
    }
}
