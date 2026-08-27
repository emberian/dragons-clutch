//! Decoder evidence against the cluster-observed upgraded Pyth generation.
//!
//! This module is test-only and adds no runtime surface. It exists because the
//! rest of this crate's fixture evidence comes from one *lab* capture whose
//! Config, guardian set, and price message are synthetic. Those bytes exercise
//! the parsers' shapes but not the shapes the live clusters actually carry:
//! the lab Config names a synthetic data source (chain 1, emitter `[1; 32]`),
//! fee 1 and `minimum_signatures = 5`, and the lab guardian set has nineteen
//! keys. After the 2026-08-26 Pyth Core cutover the live generation carries one
//! Pythnet data source, fee 0, `minimum_signatures = 3`, and a five-key
//! guardian set.
//!
//! These tests therefore parse the bytes read off `mainnet-beta` and `devnet`
//! on 2026-08-27 with the same views the adapter uses, and pin the facts that
//! force a Pyth release to be pinned **per cluster**.
//!
//! Nothing here is a production release row, a provider-availability claim, or
//! a liveness guarantee. It is a dated observation, and the observation is
//! `fixtures/pyth/upgraded-2026-08-26/`, whose `PROVENANCE.md` records every
//! bounded RPC read that produced it.

use crate::{
    price_update::{FULL_PRICE_UPDATE_V2_LEN, FullPriceUpdateV2},
    receiver_config::{RECEIVER_CONFIG_V2_LEN, ReceiverConfigV2Result, ReceiverConfigV2View},
    router_accounts::{GuardianSetV1, RouterAccountErrorV1},
};

const OBSERVED_GUARDIAN_SET_LEN: usize = 124;

const MAINNET_CONFIG: &[u8; RECEIVER_CONFIG_V2_LEN] = include_bytes!(
    "../../../fixtures/pyth/upgraded-2026-08-26/mainnet-beta/receiver-config.account"
);
const DEVNET_CONFIG: &[u8; RECEIVER_CONFIG_V2_LEN] =
    include_bytes!("../../../fixtures/pyth/upgraded-2026-08-26/devnet/receiver-config.account");
const MAINNET_GUARDIAN_SET: &[u8; OBSERVED_GUARDIAN_SET_LEN] = include_bytes!(
    "../../../fixtures/pyth/upgraded-2026-08-26/mainnet-beta/guardian-set-0.account"
);
const DEVNET_GUARDIAN_SET: &[u8; OBSERVED_GUARDIAN_SET_LEN] =
    include_bytes!("../../../fixtures/pyth/upgraded-2026-08-26/devnet/guardian-set-0.account");
const MAINNET_PRICE: &[u8; FULL_PRICE_UPDATE_V2_LEN] = include_bytes!(
    "../../../fixtures/pyth/upgraded-2026-08-26/mainnet-beta/sol-usd-price-update.account"
);
const DEVNET_PRICE: &[u8; FULL_PRICE_UPDATE_V2_LEN] = include_bytes!(
    "../../../fixtures/pyth/upgraded-2026-08-26/devnet/sol-usd-price-update.account"
);

/// `6R92oFT6UiP2xWZBjTbwAkHzFCLy5BhWnNh6m83ndhZR`, which is the ASCII bytes
/// `PythnetPythnetPythnetPythnetPyth`.
const PYTHNET_EMITTER: [u8; 32] = *b"PythnetPythnetPythnetPythnetPyth";
/// Wormhole chain identifier for Pythnet.
const PYTHNET_CHAIN: u16 = 26;

/// `HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL`.
const ROUTER_PROGRAM: [u8; 32] = [
    0xf1, 0x0b, 0x0a, 0xdc, 0x78, 0x68, 0xf4, 0x55, 0x66, 0x57, 0xa9, 0x05, 0xf7, 0x14, 0x45, 0xce,
    0xec, 0x42, 0x07, 0xac, 0x77, 0xd7, 0xc5, 0xc2, 0xb7, 0x62, 0xdf, 0x13, 0x94, 0x66, 0x4b, 0x87,
];
/// `6oXTdojyfDS8m5VtTaYB9xRCxpKGSvKJFndLUPV3V3wT`.
const MAINNET_GOVERNANCE: [u8; 32] = [
    0x56, 0x35, 0x97, 0x9a, 0x22, 0x1c, 0x34, 0x93, 0x1e, 0x32, 0x62, 0x0b, 0x92, 0x93, 0xa4, 0x63,
    0x06, 0x55, 0x55, 0xea, 0x71, 0xfe, 0x97, 0xcd, 0x62, 0x37, 0xad, 0xe8, 0x75, 0xb1, 0x2e, 0x9e,
];
/// `7g4Los4WMQnpxYiBJpU1HejBiM6xCk5RDFGCABhWE9M6`.
const DEVNET_GOVERNANCE: [u8; 32] = [
    0x63, 0x27, 0x8d, 0x27, 0x10, 0x99, 0xbf, 0xd4, 0x91, 0x95, 0x1b, 0x3e, 0x64, 0x8f, 0x08, 0xb1,
    0xc7, 0x16, 0x31, 0xe4, 0xa5, 0x36, 0x74, 0xad, 0x43, 0xe8, 0xf9, 0xf9, 0x80, 0x68, 0xc3, 0x85,
];
/// `7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE`, the shard-zero SOL/USD PDA
/// of the upgraded push oracle. It is also its own write authority.
const SOL_USD_ACCOUNT: [u8; 32] = [
    0x5b, 0xb1, 0x15, 0xb0, 0x0d, 0xd2, 0x08, 0xea, 0xac, 0x9f, 0x2a, 0x6b, 0xed, 0x58, 0xb6, 0x86,
    0xc8, 0x7d, 0xdc, 0xf5, 0x2e, 0x6d, 0xad, 0xf1, 0x58, 0x70, 0x22, 0x43, 0x32, 0x16, 0x9e, 0xcd,
];
/// SOL/USD Pyth feed identifier, read out of the live account bodies rather
/// than taken from documentation.
const SOL_USD_FEED_ID: [u8; 32] = [
    0xef, 0x0d, 0x8b, 0x6f, 0xda, 0x2c, 0xeb, 0xa4, 0x1d, 0xa1, 0x5d, 0x40, 0x95, 0xd1, 0xda, 0x39,
    0x2a, 0x0d, 0x2f, 0x8e, 0xd0, 0xc6, 0xc7, 0xbc, 0x0f, 0x4c, 0xfa, 0xc8, 0xc2, 0x80, 0xb5, 0x6d,
];

/// The five guardian keys backing Pyth on Solana after the cutover. Identical
/// key material on both clusters.
const GUARDIAN_KEYS: [[u8; 20]; 5] = [
    [
        0x41, 0x53, 0x4b, 0xb1, 0x76, 0xe4, 0x61, 0xa3, 0xfb, 0x30, 0x47, 0x94, 0x00, 0xf2, 0x10,
        0x54, 0x9e, 0xcc, 0xe6, 0x38,
    ],
    [
        0x65, 0x02, 0x98, 0x7b, 0x62, 0xf2, 0x1c, 0xab, 0x7e, 0xb5, 0xcc, 0xd8, 0xf0, 0x17, 0x30,
        0x84, 0xb6, 0x0d, 0x5b, 0x41,
    ],
    [
        0x44, 0xa3, 0xe8, 0xf6, 0xa3, 0x82, 0x41, 0x2c, 0xf6, 0xbb, 0x90, 0xa3, 0xf8, 0x10, 0x6e,
        0x68, 0x97, 0x74, 0x76, 0xc9,
    ],
    [
        0xd9, 0xd7, 0xd4, 0x52, 0x95, 0x77, 0x86, 0x43, 0x52, 0xc9, 0xa6, 0x53, 0x9a, 0x48, 0x23,
        0x8f, 0xcd, 0x44, 0x70, 0x52,
    ],
    [
        0x16, 0x63, 0xa5, 0xa8, 0x22, 0x33, 0x6e, 0xce, 0x48, 0x55, 0x9b, 0x1d, 0xfb, 0x1e, 0x93,
        0xa0, 0x17, 0xa7, 0xda, 0xc3,
    ],
];

/// Guardian-set cardinality observed on both clusters after the cutover.
const OBSERVED_GUARDIAN_COUNT: u8 = 5;
/// `Config.minimum_signatures` observed on both clusters after the cutover.
const OBSERVED_MINIMUM_SIGNATURES: u8 = 3;

#[test]
fn observed_config_names_one_pythnet_source_and_a_three_of_five_policy()
-> ReceiverConfigV2Result<()> {
    for (label, bytes) in [("mainnet-beta", MAINNET_CONFIG), ("devnet", DEVNET_CONFIG)] {
        let view = ReceiverConfigV2View::parse(bytes)?;
        assert_eq!(view.router_program(), ROUTER_PROGRAM, "{label} router");
        assert_eq!(view.data_source_count(), 1, "{label} data source count");
        let source = view.data_source(0).expect("one admitted data source");
        assert_eq!(
            source.emitter_chain(),
            PYTHNET_CHAIN,
            "{label} emitter chain"
        );
        assert_eq!(
            source.emitter_address(),
            PYTHNET_EMITTER,
            "{label} emitter address",
        );
        assert_eq!(view.data_source(1), None, "{label} second data source");
        assert_eq!(view.fee(), 0, "{label} fee");
        assert_eq!(
            view.minimum_signatures(),
            OBSERVED_MINIMUM_SIGNATURES,
            "{label} minimum signatures",
        );
        assert_eq!(
            view.target_governance_authority(),
            None,
            "{label} pending governance transfer",
        );
    }
    Ok(())
}

/// A single data source that is the Pythnet aggregation emitter is the reason a
/// devnet read is the same economic series as a mainnet read: there is no
/// second admitted source a synthetic devnet feed could arrive through.
#[test]
fn both_clusters_admit_exactly_the_same_single_emitter() -> ReceiverConfigV2Result<()> {
    let mainnet = ReceiverConfigV2View::parse(MAINNET_CONFIG)?;
    let devnet = ReceiverConfigV2View::parse(DEVNET_CONFIG)?;
    let mainnet_source = mainnet.data_source(0).expect("mainnet data source");
    let devnet_source = devnet.data_source(0).expect("devnet data source");
    assert_eq!(
        mainnet_source.emitter_chain(),
        devnet_source.emitter_chain(),
    );
    assert_eq!(
        mainnet_source.emitter_address(),
        devnet_source.emitter_address(),
    );
    assert_eq!(mainnet.minimum_signatures(), devnet.minimum_signatures());
    assert_eq!(mainnet.fee(), devnet.fee());
    assert_eq!(mainnet.router_program(), devnet.router_program());
    Ok(())
}

/// The per-cluster bind. Equal trust parameters do not make one release: the
/// governance authority differs, so the complete `Config` digest differs, so a
/// release that commits a config digest is cluster-specific by construction.
#[test]
fn observed_config_governance_authority_is_per_cluster() -> ReceiverConfigV2Result<()> {
    let mainnet = ReceiverConfigV2View::parse(MAINNET_CONFIG)?;
    let devnet = ReceiverConfigV2View::parse(DEVNET_CONFIG)?;
    assert_eq!(mainnet.governance_authority(), MAINNET_GOVERNANCE);
    assert_eq!(devnet.governance_authority(), DEVNET_GOVERNANCE);
    assert_ne!(
        mainnet.governance_authority(),
        devnet.governance_authority()
    );
    assert_ne!(
        MAINNET_CONFIG.as_slice(),
        DEVNET_CONFIG.as_slice(),
        "the two clusters' Config accounts must not be assumed interchangeable",
    );
    Ok(())
}

#[test]
fn observed_guardian_set_is_index_zero_with_five_unexpired_keys() -> Result<(), RouterAccountErrorV1>
{
    for (label, bytes) in [
        ("mainnet-beta", MAINNET_GUARDIAN_SET),
        ("devnet", DEVNET_GUARDIAN_SET),
    ] {
        let set = GuardianSetV1::parse(bytes)?;
        assert_eq!(set.index(), 0, "{label} guardian set index");
        assert_eq!(
            set.guardian_count(),
            OBSERVED_GUARDIAN_COUNT,
            "{label} guardian count",
        );
        assert_eq!(set.expiration_time(), 0, "{label} expiration");
        for (position, expected) in GUARDIAN_KEYS.iter().enumerate() {
            let start = 8 + position * 20;
            assert_eq!(
                bytes.get(start..start + 20),
                Some(expected.as_slice()),
                "{label} guardian key {position}",
            );
        }
    }
    Ok(())
}

/// The key material is shared; the accounts are not. `creation_time` differs by
/// 104 seconds, so a complete guardian-set account digest is per cluster even
/// though the trust root is one five-key set.
#[test]
fn observed_guardian_accounts_share_keys_but_not_bytes() -> Result<(), RouterAccountErrorV1> {
    let mainnet = GuardianSetV1::parse(MAINNET_GUARDIAN_SET)?;
    let devnet = GuardianSetV1::parse(DEVNET_GUARDIAN_SET)?;
    assert_eq!(mainnet.guardian_count(), devnet.guardian_count());
    assert_eq!(
        MAINNET_GUARDIAN_SET.get(8..8 + 20 * 5),
        DEVNET_GUARDIAN_SET.get(8..8 + 20 * 5),
        "guardian key material must be identical across clusters",
    );
    assert_eq!(mainnet.creation_time(), 1_778_014_551);
    assert_eq!(devnet.creation_time(), 1_778_014_447);
    assert_ne!(mainnet.creation_time(), devnet.creation_time());
    assert_ne!(
        MAINNET_GUARDIAN_SET.as_slice(),
        DEVNET_GUARDIAN_SET.as_slice(),
        "guardian-set accounts are not byte-identical across clusters",
    );
    Ok(())
}

/// Three of five is exactly the strict majority of five, so under this
/// generation the receiver's own `minimum_signatures` and the `PythReleaseV1`
/// strict-majority rule coincide. Under the superseded nineteen-key set they
/// did not (policy five against strict majority ten), which is what the
/// now-stale "quorum distinction" prose was about.
#[test]
fn observed_minimum_signatures_equals_release_strict_majority() -> ReceiverConfigV2Result<()> {
    let strict_majority = OBSERVED_GUARDIAN_COUNT / 2 + 1;
    assert_eq!(strict_majority, OBSERVED_MINIMUM_SIGNATURES);
    assert_eq!(
        ReceiverConfigV2View::parse(MAINNET_CONFIG)?.minimum_signatures(),
        strict_majority,
    );
    assert_eq!(
        ReceiverConfigV2View::parse(DEVNET_CONFIG)?.minimum_signatures(),
        strict_majority,
    );
    Ok(())
}

#[test]
fn observed_price_updates_decode_as_fully_verified_sol_usd() {
    for (label, bytes) in [("mainnet-beta", MAINNET_PRICE), ("devnet", DEVNET_PRICE)] {
        let parsed = FullPriceUpdateV2::parse(bytes);
        assert!(
            parsed.is_ok(),
            "{label} live price update must decode: {parsed:?}"
        );
        let update = parsed.expect("checked immediately above");
        assert_eq!(update.feed_id(), SOL_USD_FEED_ID, "{label} feed id");
        assert_eq!(update.write_authority(), SOL_USD_ACCOUNT, "{label} writer");
        assert_eq!(update.exponent(), -8, "{label} exponent");
        assert!(update.price() > 0, "{label} price sign");
        assert!(update.confidence() > 0, "{label} confidence");
        assert!(
            update.publish_time() >= update.prev_publish_time(),
            "{label} publish time must not go backwards",
        );
        assert!(update.posted_slot() > 0, "{label} posted slot");
    }
}

/// Both clusters carry the same feed under the same exponent. The prices are
/// not equal and must not be asserted equal: they are different samples of one
/// real series, taken minutes apart.
#[test]
fn both_clusters_carry_the_same_feed_at_different_samples() {
    let mainnet = FullPriceUpdateV2::parse(MAINNET_PRICE).expect("mainnet price");
    let devnet = FullPriceUpdateV2::parse(DEVNET_PRICE).expect("devnet price");
    assert_eq!(mainnet.feed_id(), devnet.feed_id());
    assert_eq!(mainnet.exponent(), devnet.exponent());
    assert_eq!(mainnet.write_authority(), devnet.write_authority());
    assert_ne!(
        mainnet.publish_time(),
        devnet.publish_time(),
        "the observation captured two distinct samples",
    );
}

/// The lab fixture's synthetic Config and the observed Config are genuinely
/// different shapes; this is why the observation was worth taking. If this ever
/// fails, the lab bytes have been overwritten with cluster bytes and the
/// campaign's synthetic labelling would be wrong.
#[test]
fn lab_config_and_observed_config_are_not_the_same_bytes() {
    const LAB_CONFIG: &[u8; RECEIVER_CONFIG_V2_LEN] =
        include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver-config.account");
    assert_ne!(LAB_CONFIG.as_slice(), MAINNET_CONFIG.as_slice());
    assert_ne!(LAB_CONFIG.as_slice(), DEVNET_CONFIG.as_slice());
    let lab = ReceiverConfigV2View::parse(LAB_CONFIG).expect("lab config");
    assert_eq!(lab.fee(), 1);
    assert_eq!(lab.minimum_signatures(), 5);
    assert_ne!(
        lab.data_source(0)
            .expect("lab data source")
            .emitter_address(),
        PYTHNET_EMITTER,
        "the lab Config must keep its synthetic emitter",
    );
}
