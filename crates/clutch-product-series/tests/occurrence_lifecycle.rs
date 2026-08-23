use clutch_product_series::{
    ContentId, Error, FixedCodec, MarketInstanceV2Id, ProductOccurrenceBindingV1,
    ProductOccurrenceCapitalizationV1, ProductOccurrenceFamilyDispositionV1,
    ProductOccurrenceFamilyTerminalProjectionV1, ProductOccurrenceFamilyV1,
    ProductOccurrencePhaseV1, ProductOccurrenceRootV1, SeriesPlanV5Id, SourceOccurrenceV1Id,
    PRODUCT_OCCURRENCE_FAMILY_COUNT_V1, PRODUCT_OCCURRENCE_ROOT_BYTES_V1,
};

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn binding() -> ProductOccurrenceBindingV1 {
    ProductOccurrenceBindingV1 {
        market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
        series_plan_id: SeriesPlanV5Id::from_bytes([2; 32]),
        ordinal: 7,
        generation: 3,
        product_template_id: id(3),
        native_claim_basis_id: id(4),
        recovery_policy_id: id(5),
        price_measure_policy_id: id(6),
        market_genesis_profile_id: id(7),
        funding_terms_id: id(8),
        funding_quote_id: id(9),
        attachment_plan_id: id(10),
        compiler_output_id: id(11),
        source_occurrence_id: SourceOccurrenceV1Id::from_bytes([31; 32]),
        source_occurrence_account_id: id(12),
        source_occurrence_account_authentication_id: id(13),
        source_occurrence_receipt_id: id(14),
        source_release_manifest_id: id(15),
        source_route_id: id(16),
        source_clock_policy_id: id(17),
        source_plane_contract_id: id(18),
        source_spec_id: id(19),
        window_spec_id: id(20),
        statistic_key_id: id(21),
        source_repair_generation: 5,
        failure_interval_work_account_id: id(22),
        failure_interval_replay_account_id: id(23),
        resolution_account_id: id(24),
        failure_policy_binding_id: id(25),
        recovery_state_id: id(26),
        interval_consensus_profile_id: id(27),
        maximum_interval_width: 4_096,
        maximum_coordinates_per_advance: 64,
        registry_release_id: id(28),
        capability_profile_id: id(29),
        rent_payer: id(30),
        neutral_lamport_sink: id(32),
    }
}

fn capitalization() -> ProductOccurrenceCapitalizationV1 {
    ProductOccurrenceCapitalizationV1 {
        principal_lamports: [1_000, 2_000, 3_000, 4_000],
        donation_floor_lamports: [4, 5, 6, 7],
    }
}

const FAMILIES: [ProductOccurrenceFamilyV1; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1] = [
    ProductOccurrenceFamilyV1::ClaimLedger,
    ProductOccurrenceFamilyV1::Hoard,
    ProductOccurrenceFamilyV1::General,
    ProductOccurrenceFamilyV1::Dealer,
    ProductOccurrenceFamilyV1::Failure,
    ProductOccurrenceFamilyV1::Source,
    ProductOccurrenceFamilyV1::Position,
    ProductOccurrenceFamilyV1::Fractional,
    ProductOccurrenceFamilyV1::Structured,
    ProductOccurrenceFamilyV1::SeriesOccurrence,
];

fn terminal(
    root: ProductOccurrenceRootV1,
    family: ProductOccurrenceFamilyV1,
) -> ProductOccurrenceFamilyTerminalProjectionV1 {
    let base = 100 + family.byte() * 6;
    let terminal_state_ids = if family == ProductOccurrenceFamilyV1::Fractional {
        [id(base + 4), id(base + 5)]
    } else {
        [ContentId::ZERO; 2]
    };
    ProductOccurrenceFamilyTerminalProjectionV1::new(
        family,
        root.binding(),
        root.counts().terminal[family.index()],
        root.transition_sequence() + 1,
        id(base),
        id(base + 1),
        id(base + 2),
        id(base + 3),
        terminal_state_ids,
    )
    .unwrap()
}

#[test]
fn root_counts_one_summary_per_family_and_records_fractional_dual_close() {
    let root = ProductOccurrenceRootV1::initialize(binding(), capitalization()).unwrap();
    assert_eq!(root.phase(), ProductOccurrencePhaseV1::Active);
    assert_eq!(
        root.counts().expected,
        [1; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1]
    );
    assert_eq!(root.counts().live, [1; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1]);
    assert_eq!(root.begin_retirement().unwrap().transition_sequence(), 1);

    let mut root = root.begin_retirement().unwrap();
    for family in FAMILIES {
        root = root
            .consume_family_terminal(terminal(root, family))
            .unwrap();
    }
    assert_eq!(root.counts().live, [0; PRODUCT_OCCURRENCE_FAMILY_COUNT_V1]);
    assert_eq!(root.fractional_terminal_state_ids(), [id(146), id(147)]);
    let (terminal_root, capability) = root.finalize_terminal().unwrap();
    assert_eq!(terminal_root.phase(), ProductOccurrencePhaseV1::Terminal);
    assert_eq!(
        capability.fractional_terminal_state_ids(),
        terminal_root.fractional_terminal_state_ids()
    );
}

#[test]
fn root_refuses_early_finalization_replay_and_wrong_binding() {
    let active = ProductOccurrenceRootV1::initialize(binding(), capitalization()).unwrap();
    assert_eq!(active.finalize_terminal(), Err(Error::WorkIncomplete));
    let retiring = active.begin_retirement().unwrap();
    let receipt = terminal(retiring, ProductOccurrenceFamilyV1::ClaimLedger);
    let consumed = retiring.consume_family_terminal(receipt).unwrap();
    assert_eq!(
        consumed.consume_family_terminal(receipt),
        Err(Error::WorkStateMismatch)
    );

    let mut other_binding = binding();
    other_binding.ordinal += 1;
    let other = ProductOccurrenceRootV1::initialize(other_binding, capitalization())
        .unwrap()
        .begin_retirement()
        .unwrap();
    assert_eq!(
        retiring.consume_family_terminal(terminal(other, ProductOccurrenceFamilyV1::Hoard)),
        Err(Error::UnauthenticatedAuthority)
    );
}

#[test]
fn fractional_requires_two_distinct_terminal_state_ids() {
    let root = ProductOccurrenceRootV1::initialize(binding(), capitalization())
        .unwrap()
        .begin_retirement()
        .unwrap();
    let common = id(170);
    assert_eq!(
        ProductOccurrenceFamilyTerminalProjectionV1::new(
            ProductOccurrenceFamilyV1::Fractional,
            root.binding(),
            0,
            2,
            id(160),
            id(161),
            id(162),
            id(163),
            [common, common],
        ),
        Err(Error::MismatchedArtifact)
    );
    assert_eq!(
        ProductOccurrenceFamilyTerminalProjectionV1::new(
            ProductOccurrenceFamilyV1::Hoard,
            root.binding(),
            0,
            2,
            id(160),
            id(161),
            id(162),
            id(163),
            [id(164), id(165)],
        ),
        Err(Error::MismatchedArtifact)
    );
}

#[test]
fn absence_is_explicit_and_limited_to_optional_families() {
    let root = ProductOccurrenceRootV1::initialize(binding(), capitalization())
        .unwrap()
        .begin_retirement()
        .unwrap();
    let absent = ProductOccurrenceFamilyTerminalProjectionV1::absent(
        ProductOccurrenceFamilyV1::Dealer,
        root.binding(),
        0,
        2,
        id(180),
        id(181),
        id(182),
        id(183),
    )
    .unwrap();
    assert_eq!(
        absent.disposition(),
        ProductOccurrenceFamilyDispositionV1::Absent
    );
    assert_eq!(
        ProductOccurrenceFamilyTerminalProjectionV1::absent(
            ProductOccurrenceFamilyV1::ClaimLedger,
            root.binding(),
            0,
            2,
            id(184),
            id(185),
            id(186),
            id(187),
        ),
        Err(Error::UnsupportedCapability)
    );
}

#[test]
fn fixed_codec_refuses_wrong_lengths_reserved_bytes_and_tampered_counts() {
    let root = ProductOccurrenceRootV1::initialize(binding(), capitalization()).unwrap();
    let mut body = [0; PRODUCT_OCCURRENCE_ROOT_BYTES_V1];
    root.encode_into(&mut body).unwrap();
    assert_eq!(ProductOccurrenceRootV1::decode(&body).unwrap(), root);
    assert_eq!(
        ProductOccurrenceRootV1::decode(&body[..body.len() - 1]),
        Err(Error::Truncated)
    );
    let mut trailing = body.to_vec();
    trailing.push(0);
    assert_eq!(
        ProductOccurrenceRootV1::decode(&trailing),
        Err(Error::TrailingBytes)
    );
    let mut dirty_reserved = body;
    dirty_reserved[11] = 1;
    assert_eq!(
        ProductOccurrenceRootV1::decode(&dirty_reserved),
        Err(Error::NonCanonicalReserved)
    );
    let mut bad_count = body;
    let counts_offset = 16 + 32 * 32 + 48 + 8 * 8;
    bad_count[counts_offset..counts_offset + 4].copy_from_slice(&2_u32.to_le_bytes());
    assert!(ProductOccurrenceRootV1::decode(&bad_count).is_err());
}
