use clutch_product_series::{
    AuthenticatedMarketFamilyAuthorityV1, ContentId, Error, FixedCodec,
    MarketFamilyAggregatorBindingV1, MarketFamilyAggregatorPhaseV1, MarketFamilyAggregatorV1,
    MarketFamilyExhaustiveSummaryV1, MarketFamilyStatusV1, MarketFamilyV1, MarketInstanceV2Id,
    NoMarketFamilyAuthorityV1, RegistryCapabilityProfileV3Id, RegistryProgramReleaseV1Id,
    MARKET_FAMILIES_V1, MARKET_FAMILY_AGGREGATOR_BYTES_V1,
    MARKET_FAMILY_EXHAUSTIVE_SUMMARY_BYTES_V1, MARKET_FAMILY_TERMINAL_PROJECTION_BYTES_V1,
};

#[derive(Debug)]
struct AllowAuthority;

impl AuthenticatedMarketFamilyAuthorityV1 for AllowAuthority {
    fn authenticate_initialization(
        &self,
        _binding: &MarketFamilyAggregatorBindingV1,
    ) -> clutch_product_series::Result<()> {
        Ok(())
    }

    fn authenticate_admission(
        &self,
        _current: &MarketFamilyAggregatorV1,
        _family: MarketFamilyV1,
        _family_root_id: ContentId,
        _family_admission_sequence: u32,
        _admission_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Ok(())
    }

    fn authenticate_terminal(
        &self,
        _current: &MarketFamilyAggregatorV1,
        _family: MarketFamilyV1,
        _family_root_id: ContentId,
        _family_terminal_sequence: u32,
        _terminal_receipt_id: ContentId,
    ) -> clutch_product_series::Result<()> {
        Ok(())
    }

    fn authenticate_begin_retirement(
        &self,
        _current: &MarketFamilyAggregatorV1,
    ) -> clutch_product_series::Result<()> {
        Ok(())
    }
}

fn id(byte: u8) -> ContentId {
    ContentId::from_bytes([byte; 32])
}

fn binding(enabled_family_mask: u8) -> MarketFamilyAggregatorBindingV1 {
    MarketFamilyAggregatorBindingV1 {
        market_instance_id: MarketInstanceV2Id::from_bytes([1; 32]),
        generation: 7,
        registry_release_id: RegistryProgramReleaseV1Id::from_bytes([2; 32]),
        capability_profile_id: RegistryCapabilityProfileV3Id::from_bytes([3; 32]),
        enabled_family_mask,
        family_root_ids: [id(4), id(5), id(6), id(7), id(8)],
    }
}

fn enabled(families: &[MarketFamilyV1]) -> u8 {
    families
        .iter()
        .fold(0_u8, |mask, family| mask | family.mask())
}

#[test]
fn default_deny_authority_cannot_initialize() {
    let result = MarketFamilyAggregatorV1::initialize(
        &NoMarketFamilyAuthorityV1,
        binding(MarketFamilyV1::General.mask()),
    );
    assert_eq!(result, Err(Error::UnauthenticatedAuthority));
}

#[test]
fn all_four_states_and_all_five_families_are_exhaustive() {
    let mask = enabled(&[
        MarketFamilyV1::General,
        MarketFamilyV1::Fractional,
        MarketFamilyV1::Dealer,
        MarketFamilyV1::Structured,
    ]);
    let mut state = MarketFamilyAggregatorV1::initialize(&AllowAuthority, binding(mask)).unwrap();

    assert_eq!(MARKET_FAMILIES_V1[0], MarketFamilyV1::General);
    assert_eq!(MARKET_FAMILIES_V1[1], MarketFamilyV1::Direct);
    assert_eq!(MARKET_FAMILIES_V1[2], MarketFamilyV1::Fractional);
    assert_eq!(MARKET_FAMILIES_V1[3], MarketFamilyV1::Dealer);
    assert_eq!(MARKET_FAMILIES_V1[4], MarketFamilyV1::Structured);
    assert_eq!(
        state.family(MarketFamilyV1::Direct).status(),
        MarketFamilyStatusV1::CapabilityDisabled
    );
    assert_eq!(
        state.family(MarketFamilyV1::Structured).status(),
        MarketFamilyStatusV1::EnabledNeverFounded
    );
    assert!(state.admits_new_child(MarketFamilyV1::General));
    assert!(!state.admits_new_child(MarketFamilyV1::Direct));
    assert!(!state.activation_ready().unwrap());

    state = state
        .admit_child(&AllowAuthority, MarketFamilyV1::General, 0, id(20))
        .unwrap();
    state = state
        .admit_child(&AllowAuthority, MarketFamilyV1::General, 1, id(21))
        .unwrap();
    state = state
        .terminalize_child(&AllowAuthority, MarketFamilyV1::General, 0, id(22))
        .unwrap();
    assert_eq!(
        state.family(MarketFamilyV1::General).counts(),
        clutch_product_series::MarketFamilyCountsV1 {
            admitted: 2,
            live: 1,
            terminal: 1,
        }
    );
    assert_eq!(
        state.family(MarketFamilyV1::General).status(),
        MarketFamilyStatusV1::Live
    );
    assert!(state.activation_ready().unwrap());

    state = state
        .admit_child(&AllowAuthority, MarketFamilyV1::Fractional, 0, id(23))
        .unwrap();
    state = state
        .terminalize_child(&AllowAuthority, MarketFamilyV1::Fractional, 0, id(24))
        .unwrap();
    state = state
        .admit_child(&AllowAuthority, MarketFamilyV1::Dealer, 0, id(25))
        .unwrap();
    state = state
        .terminalize_child(&AllowAuthority, MarketFamilyV1::Dealer, 0, id(26))
        .unwrap();

    // Open roots keep historically founded, currently quiescent families Live:
    // future Series-linked admissions remain possible until retirement begins.
    assert_eq!(
        state.family(MarketFamilyV1::Fractional).status(),
        MarketFamilyStatusV1::Live
    );
    assert_eq!(state.family(MarketFamilyV1::Fractional).counts().live, 0);

    let summaries = state.exhaustive_summaries().unwrap();
    assert_eq!(summaries[0].status(), MarketFamilyStatusV1::Live);
    assert_eq!(
        summaries[1].status(),
        MarketFamilyStatusV1::CapabilityDisabled
    );
    assert_eq!(summaries[2].status(), MarketFamilyStatusV1::Live);
    assert_eq!(summaries[3].status(), MarketFamilyStatusV1::Live);
    assert_eq!(
        summaries[4].status(),
        MarketFamilyStatusV1::EnabledNeverFounded
    );
    assert!(summaries[0].admits_new_child());
    assert!(!summaries[1].admits_new_child());

    state = state.begin_retirement(&AllowAuthority).unwrap();
    assert_eq!(state.phase(), MarketFamilyAggregatorPhaseV1::Retiring);
    assert_eq!(
        state.family(MarketFamilyV1::Fractional).status(),
        MarketFamilyStatusV1::Terminal
    );
    assert_eq!(
        state.family(MarketFamilyV1::Dealer).status(),
        MarketFamilyStatusV1::Terminal
    );
    assert_eq!(
        state.family(MarketFamilyV1::Structured).status(),
        MarketFamilyStatusV1::EnabledNeverFounded
    );
    assert!(!state.admits_new_child(MarketFamilyV1::Structured));

    state = state
        .terminalize_child(&AllowAuthority, MarketFamilyV1::General, 1, id(27))
        .unwrap();
    let (terminal, projection) = state.finalize_terminal().unwrap();
    assert_eq!(terminal.phase(), MarketFamilyAggregatorPhaseV1::Terminal);
    assert_eq!(terminal.transition_sequence(), 10);
    assert_eq!(
        terminal.family(MarketFamilyV1::Direct).status(),
        MarketFamilyStatusV1::CapabilityDisabled
    );
    assert_eq!(
        terminal.family(MarketFamilyV1::Structured).status(),
        MarketFamilyStatusV1::EnabledNeverFounded
    );

    let summary_ids = projection.summary_ids();
    for left in 0..summary_ids.len() {
        summary_ids[left].validate().unwrap();
        for right in (left + 1)..summary_ids.len() {
            assert_ne!(summary_ids[left], summary_ids[right]);
        }
    }
    assert_eq!(
        projection.summary_id(MarketFamilyV1::Direct),
        summary_ids[MarketFamilyV1::Direct.index()]
    );
}

#[test]
fn direct_is_independent_from_general_and_can_be_the_only_live_family() {
    let mut state = MarketFamilyAggregatorV1::initialize(
        &AllowAuthority,
        binding(MarketFamilyV1::Direct.mask()),
    )
    .unwrap();
    assert_eq!(
        state.family(MarketFamilyV1::General).status(),
        MarketFamilyStatusV1::CapabilityDisabled
    );
    assert_eq!(
        state.family(MarketFamilyV1::Direct).status(),
        MarketFamilyStatusV1::EnabledNeverFounded
    );
    state = state
        .admit_child(&AllowAuthority, MarketFamilyV1::Direct, 0, id(30))
        .unwrap();
    assert_eq!(
        state.family(MarketFamilyV1::Direct).status(),
        MarketFamilyStatusV1::Live
    );
    assert_eq!(state.family(MarketFamilyV1::Direct).counts().admitted, 1);
    assert!(state.activation_ready().unwrap());
}

#[test]
fn every_enabled_primary_modality_must_be_founded_before_activation() {
    let mut state = MarketFamilyAggregatorV1::initialize(
        &AllowAuthority,
        binding(MarketFamilyV1::General.mask() | MarketFamilyV1::Direct.mask()),
    )
    .unwrap();
    state = state
        .admit_child(&AllowAuthority, MarketFamilyV1::General, 0, id(31))
        .unwrap();
    assert!(!state.activation_ready().unwrap());
    state = state
        .admit_child(&AllowAuthority, MarketFamilyV1::Direct, 0, id(32))
        .unwrap();
    assert!(state.activation_ready().unwrap());
}

#[test]
fn stale_sequences_disabled_families_and_bad_close_order_refuse() {
    let state = MarketFamilyAggregatorV1::initialize(
        &AllowAuthority,
        binding(MarketFamilyV1::General.mask()),
    )
    .unwrap();
    assert_eq!(
        state.admit_child(&AllowAuthority, MarketFamilyV1::Direct, 0, id(40)),
        Err(Error::UnsupportedCapability)
    );
    assert_eq!(
        state.terminalize_child(&AllowAuthority, MarketFamilyV1::General, 0, id(41)),
        Err(Error::InvalidParameter)
    );
    assert_eq!(state.finalize_terminal(), Err(Error::SeriesNotClosed));

    let state = state
        .admit_child(&AllowAuthority, MarketFamilyV1::General, 0, id(42))
        .unwrap();
    assert_eq!(
        state.admit_child(&AllowAuthority, MarketFamilyV1::General, 0, id(42)),
        Err(Error::InvalidParameter)
    );
    assert_eq!(
        state.terminalize_child(&AllowAuthority, MarketFamilyV1::General, 1, id(43)),
        Err(Error::InvalidParameter)
    );
    let retiring = state.begin_retirement(&AllowAuthority).unwrap();
    assert_eq!(
        retiring.admit_child(&AllowAuthority, MarketFamilyV1::General, 1, id(44)),
        Err(Error::InvalidParameter)
    );
    assert_eq!(retiring.finalize_terminal(), Err(Error::SeriesNotClosed));
    let retiring = retiring
        .terminalize_child(&AllowAuthority, MarketFamilyV1::General, 0, id(45))
        .unwrap();
    assert!(retiring.finalize_terminal().is_ok());
}

#[test]
fn aggregator_codec_is_exact_and_refuses_hostile_states() {
    let state = MarketFamilyAggregatorV1::initialize(
        &AllowAuthority,
        binding(enabled(&[
            MarketFamilyV1::General,
            MarketFamilyV1::Direct,
            MarketFamilyV1::Structured,
        ])),
    )
    .unwrap();
    let mut bytes = [0_u8; MARKET_FAMILY_AGGREGATOR_BYTES_V1];
    state.encode_into(&mut bytes).unwrap();
    assert_eq!(MarketFamilyAggregatorV1::decode(&bytes).unwrap(), state);
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bytes[..bytes.len() - 1]),
        Err(Error::Truncated)
    );
    let mut trailing = bytes.to_vec();
    trailing.push(0);
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&trailing),
        Err(Error::TrailingBytes)
    );

    let mut bad = bytes;
    bad[0] ^= 1;
    assert_eq!(MarketFamilyAggregatorV1::decode(&bad), Err(Error::BadMagic));
    bad = bytes;
    bad[8] = 2;
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bad),
        Err(Error::BadVersion)
    );
    bad = bytes;
    bad[12] = 1;
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bad),
        Err(Error::NonCanonicalReserved)
    );
    bad = bytes;
    bad[11] |= 0x80;
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bad),
        Err(Error::InvalidParameter)
    );
    bad = bytes;
    bad[144..176].copy_from_slice(&bytes[112..144]);
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bad),
        Err(Error::MismatchedArtifact)
    );
    bad = bytes;
    bad[280] = 1;
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bad),
        Err(Error::InvalidParameter)
    );
    bad = bytes;
    bad[288] = MarketFamilyStatusV1::Terminal.byte();
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bad),
        Err(Error::InvalidParameter)
    );
    bad = bytes;
    bad[289] = 1;
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bad),
        Err(Error::NonCanonicalReserved)
    );
    bad = bytes;
    bad[292] = 1;
    assert_eq!(
        MarketFamilyAggregatorV1::decode(&bad),
        Err(Error::InvalidParameter)
    );
}

#[test]
fn summary_and_terminal_projection_codecs_are_exact() {
    let state = MarketFamilyAggregatorV1::initialize(
        &AllowAuthority,
        binding(MarketFamilyV1::Structured.mask()),
    )
    .unwrap();
    let summaries = state.exhaustive_summaries().unwrap();
    let summary = summaries[MarketFamilyV1::Structured.index()];
    let mut summary_bytes = [0_u8; MARKET_FAMILY_EXHAUSTIVE_SUMMARY_BYTES_V1];
    summary.encode_into(&mut summary_bytes).unwrap();
    assert_eq!(
        MarketFamilyExhaustiveSummaryV1::decode(&summary_bytes).unwrap(),
        summary
    );
    let mut dirty = summary_bytes;
    dirty[13] = 1;
    assert_eq!(
        MarketFamilyExhaustiveSummaryV1::decode(&dirty),
        Err(Error::NonCanonicalReserved)
    );

    let state = state.begin_retirement(&AllowAuthority).unwrap();
    let (_, projection) = state.finalize_terminal().unwrap();
    let mut projection_bytes = [0_u8; MARKET_FAMILY_TERMINAL_PROJECTION_BYTES_V1];
    projection.encode_into(&mut projection_bytes).unwrap();
    assert_eq!(
        clutch_product_series::MarketFamilyAggregatorTerminalProjectionV1::decode(
            &projection_bytes
        )
        .unwrap(),
        projection
    );
    let mut dirty = projection_bytes;
    dirty[10] = 1;
    assert_eq!(
        clutch_product_series::MarketFamilyAggregatorTerminalProjectionV1::decode(&dirty),
        Err(Error::NonCanonicalReserved)
    );
}
