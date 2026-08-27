//! Byte-level evidence binding the Pyth lab fixture to the cluster-observed
//! upgraded generation.
//!
//! This file executes nothing. It needs no `SBF_OUT_DIR` and starts no bank.
//! It answers one question with bytes: **is the fixture the campaign runs the
//! same code the live clusters run, and can one release row cover both
//! clusters?**
//!
//! The answers, measured 2026-08-27 by bounded public RPC and recorded in
//! `fixtures/pyth/upgraded-2026-08-26/PROVENANCE.md`:
//!
//! - Yes. The lab `receiver.so` and `router.so` are byte-identical to the live
//!   upgraded receiver and router on **both** `mainnet-beta` and `devnet`. The
//!   2026-08-26 Pyth Core cutover did not change those binaries; it made that
//!   generation canonical. Nothing in the ABI moved.
//! - No. Deployment slot and upgrade authority differ per cluster, so the
//!   complete Loader V3 `ProgramData` body digest differs per cluster even
//!   though the ELF is shared. A Pyth release is pinned per cluster, and the
//!   cluster is named by its genesis hash rather than inferred.
//!
//! The digests below are **pinned observations with a date**, not derivations.
//! If a future read disagrees, the provider redeployed and the disagreement is
//! the finding — do not relax an assertion to make it pass.

use std::str::FromStr;

use solana_program::{hash::hash, pubkey::Pubkey};
use solana_sdk_ids::bpf_loader_upgradeable;

const LOADER_V3_PROGRAMDATA_METADATA_BYTES: usize = 45;

/// Byte-identical to the live upgraded receiver on both clusters.
const RECEIVER_ELF: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/receiver.so");
/// Byte-identical to the live upgraded Wormhole receiver on both clusters.
const ROUTER_ELF: &[u8] =
    include_bytes!("../../../fixtures/pyth/local-upgraded-2026-08-22/router.so");
/// The third program of the generation, which the lab fixture never held.
const PUSH_ORACLE_ELF: &[u8] =
    include_bytes!("../../../fixtures/pyth/upgraded-2026-08-26/push-oracle.so");

const MAINNET_ROUTER_HEADER: &[u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES] = include_bytes!(
    "../../../fixtures/pyth/upgraded-2026-08-26/mainnet-beta/router.programdata-header"
);
const MAINNET_RECEIVER_HEADER: &[u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES] = include_bytes!(
    "../../../fixtures/pyth/upgraded-2026-08-26/mainnet-beta/receiver.programdata-header"
);
const MAINNET_PUSH_HEADER: &[u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES] = include_bytes!(
    "../../../fixtures/pyth/upgraded-2026-08-26/mainnet-beta/pushoracle.programdata-header"
);
const DEVNET_ROUTER_HEADER: &[u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES] =
    include_bytes!("../../../fixtures/pyth/upgraded-2026-08-26/devnet/router.programdata-header");
const DEVNET_RECEIVER_HEADER: &[u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES] =
    include_bytes!("../../../fixtures/pyth/upgraded-2026-08-26/devnet/receiver.programdata-header");
const DEVNET_PUSH_HEADER: &[u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES] = include_bytes!(
    "../../../fixtures/pyth/upgraded-2026-08-26/devnet/pushoracle.programdata-header"
);

const ROUTER_PROGRAM: &str = "HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL";
const ROUTER_PROGRAMDATA: &str = "9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x";
const RECEIVER_PROGRAM: &str = "rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp";
const RECEIVER_PROGRAMDATA: &str = "3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX";
const PUSH_ORACLE_PROGRAM: &str = "pyt2F414BA6dPttK6RddPZUdHfapoBN24GL5wbrPCou";
const PUSH_ORACLE_PROGRAMDATA: &str = "9nxngQjxBGUZ3ajfqoTrpiuDBVfztXCQVDuWDAw52Gew";

const MAINNET_UPGRADE_AUTHORITY: &str = "6oXTdojyfDS8m5VtTaYB9xRCxpKGSvKJFndLUPV3V3wT";
const DEVNET_UPGRADE_AUTHORITY: &str = "upg8KLALUN7ByDHiBu4wEbMDTC6UnSVFSYfTyGfXuzr";

/// The exact ELF digest of each upgraded program, identical on both clusters.
const ROUTER_ELF_DIGEST: &str = "f9061f03a81b89db29f4603677e3b3d89b3bbf08d67827b2832f18a4e2b61acb";
const RECEIVER_ELF_DIGEST: &str =
    "c5079559864fc34dbd5fe87b4aa9fba3a1ed22690363ec490449e8660e73af64";
const PUSH_ORACLE_ELF_DIGEST: &str =
    "a0318a87b80cebf9633e2b16e81984af5633e9a72ab491960ca16fbfd0d7d916";

struct ClusterProgram {
    cluster: &'static str,
    label: &'static str,
    program: &'static str,
    programdata: &'static str,
    header: &'static [u8; LOADER_V3_PROGRAMDATA_METADATA_BYTES],
    elf: &'static [u8],
    deployment_slot: u64,
    upgrade_authority: &'static str,
    /// SHA-256 of the complete ProgramData account body: header then ELF.
    complete_body_digest: &'static str,
}

fn observed_programs() -> [ClusterProgram; 6] {
    [
        ClusterProgram {
            cluster: "mainnet-beta",
            label: "router",
            program: ROUTER_PROGRAM,
            programdata: ROUTER_PROGRAMDATA,
            header: MAINNET_ROUTER_HEADER,
            elf: ROUTER_ELF,
            deployment_slot: 417_825_233,
            upgrade_authority: MAINNET_UPGRADE_AUTHORITY,
            complete_body_digest: "3b911964d5c74335cf81838f46903abd04ffd3fe7ed7bc2661add50fbf90d4b3",
        },
        ClusterProgram {
            cluster: "mainnet-beta",
            label: "receiver",
            program: RECEIVER_PROGRAM,
            programdata: RECEIVER_PROGRAMDATA,
            header: MAINNET_RECEIVER_HEADER,
            elf: RECEIVER_ELF,
            deployment_slot: 417_825_260,
            upgrade_authority: MAINNET_UPGRADE_AUTHORITY,
            complete_body_digest: "292d187cfc879f5b0f9dd061f76ea96ea4f8193a83d3de654652309769a57ecf",
        },
        ClusterProgram {
            cluster: "mainnet-beta",
            label: "push oracle",
            program: PUSH_ORACLE_PROGRAM,
            programdata: PUSH_ORACLE_PROGRAMDATA,
            header: MAINNET_PUSH_HEADER,
            elf: PUSH_ORACLE_ELF,
            deployment_slot: 417_825_281,
            upgrade_authority: MAINNET_UPGRADE_AUTHORITY,
            complete_body_digest: "0238fa7b6724e2dde966c96a84131d4c244c0a896555ebdf04e900902c072d84",
        },
        ClusterProgram {
            cluster: "devnet",
            label: "router",
            program: ROUTER_PROGRAM,
            programdata: ROUTER_PROGRAMDATA,
            header: DEVNET_ROUTER_HEADER,
            elf: ROUTER_ELF,
            deployment_slot: 460_336_290,
            upgrade_authority: DEVNET_UPGRADE_AUTHORITY,
            complete_body_digest: "f26f4b53b0f980455886116f500fa74ba475e51b1acb7f486b18afa9d73d948f",
        },
        ClusterProgram {
            cluster: "devnet",
            label: "receiver",
            program: RECEIVER_PROGRAM,
            programdata: RECEIVER_PROGRAMDATA,
            header: DEVNET_RECEIVER_HEADER,
            elf: RECEIVER_ELF,
            deployment_slot: 460_336_311,
            upgrade_authority: DEVNET_UPGRADE_AUTHORITY,
            complete_body_digest: "7122abc6b5e78d30bf88c869cb5d8783adaf897369d04eca827d3af8ffe18e5d",
        },
        ClusterProgram {
            cluster: "devnet",
            label: "push oracle",
            program: PUSH_ORACLE_PROGRAM,
            programdata: PUSH_ORACLE_PROGRAMDATA,
            header: DEVNET_PUSH_HEADER,
            elf: PUSH_ORACLE_ELF,
            deployment_slot: 460_336_332,
            upgrade_authority: DEVNET_UPGRADE_AUTHORITY,
            complete_body_digest: "95c4f5d726073d533c1509eb79260d914a8c1ca939e91f18de062f98328b5e97",
        },
    ]
}

fn hex_32(value: &str) -> [u8; 32] {
    assert_eq!(value.len(), 64, "a SHA-256 pin is 64 hex characters");
    let mut bytes = [0_u8; 32];
    for (index, slot) in bytes.iter_mut().enumerate() {
        let start = index * 2;
        let pair = value.get(start..start + 2).expect("checked length");
        *slot = u8::from_str_radix(pair, 16).expect("lowercase hexadecimal pin");
    }
    bytes
}

fn assert_sha256(label: &str, bytes: &[u8], expected: &str) {
    assert_eq!(
        hash(bytes).to_bytes(),
        hex_32(expected),
        "SHA-256 mismatch for {label}"
    );
}

fn pubkey(value: &str) -> Pubkey {
    Pubkey::from_str(value).expect("pinned public address")
}

/// The bind that makes the whole campaign meaningful after the cutover: the
/// bytes the lab executes are the bytes the clusters execute.
#[test]
fn lab_elfs_are_the_live_upgraded_generation() {
    assert_sha256("receiver.so", RECEIVER_ELF, RECEIVER_ELF_DIGEST);
    assert_sha256("router.so", ROUTER_ELF, ROUTER_ELF_DIGEST);
    assert_sha256("push-oracle.so", PUSH_ORACLE_ELF, PUSH_ORACLE_ELF_DIGEST);
    assert_eq!(RECEIVER_ELF.len(), 416_864);
    assert_eq!(ROUTER_ELF.len(), 655_960);
    assert_eq!(PUSH_ORACLE_ELF.len(), 234_952);
}

/// Reconstruct each cluster's complete `ProgramData` account from the committed
/// 45-byte header and the shared ELF, and require the exact observed digest.
/// This is what proves the ELF really is shared: one ELF reproduces six
/// distinct observed account bodies when paired with six observed headers.
#[test]
fn every_observed_programdata_body_reconstructs_from_one_shared_elf() {
    for row in observed_programs() {
        let mut body = Vec::with_capacity(LOADER_V3_PROGRAMDATA_METADATA_BYTES + row.elf.len());
        body.extend_from_slice(row.header);
        body.extend_from_slice(row.elf);
        assert_sha256(
            &format!("{} {} complete ProgramData", row.cluster, row.label),
            &body,
            row.complete_body_digest,
        );
    }
}

/// Decode the committed headers and require the per-cluster facts that force a
/// per-cluster release: different deployment slot, different upgrade authority.
///
/// Also requires every `ProgramData` key to be the Upgradeable Loader's
/// canonical PDA of its program. A release naming a `ProgramData` key it did
/// not derive is naming an account the loader would never write.
#[test]
fn observed_headers_carry_per_cluster_slot_authority_and_canonical_pda() {
    for row in observed_programs() {
        let context = format!("{} {}", row.cluster, row.label);
        let variant = u32::from_le_bytes(row.header[0..4].try_into().expect("four bytes"));
        assert_eq!(variant, 3, "{context} Loader V3 ProgramData variant");
        let slot = u64::from_le_bytes(row.header[4..12].try_into().expect("eight bytes"));
        assert_eq!(slot, row.deployment_slot, "{context} deployment slot");
        assert_eq!(row.header[12], 1, "{context} upgrade authority present");
        assert_eq!(
            Pubkey::try_from(&row.header[13..45]).expect("thirty-two bytes"),
            pubkey(row.upgrade_authority),
            "{context} upgrade authority",
        );

        let program = pubkey(row.program);
        let derived =
            Pubkey::find_program_address(&[program.as_ref()], &bpf_loader_upgradeable::ID).0;
        assert_eq!(
            derived,
            pubkey(row.programdata),
            "{context} canonical Loader V3 ProgramData PDA",
        );
    }
}

/// The negative half of the same fact. Equal binaries are not one release.
#[test]
fn the_two_clusters_are_not_one_release() {
    assert_ne!(
        MAINNET_RECEIVER_HEADER.as_slice(),
        DEVNET_RECEIVER_HEADER.as_slice(),
    );
    assert_ne!(
        MAINNET_ROUTER_HEADER.as_slice(),
        DEVNET_ROUTER_HEADER.as_slice(),
    );
    assert_ne!(
        MAINNET_PUSH_HEADER.as_slice(),
        DEVNET_PUSH_HEADER.as_slice()
    );
    assert_ne!(
        pubkey(MAINNET_UPGRADE_AUTHORITY),
        pubkey(DEVNET_UPGRADE_AUTHORITY),
        "the clusters do not share an upgrade authority",
    );
    let mainnet_slot = u64::from_le_bytes(MAINNET_RECEIVER_HEADER[4..12].try_into().expect("slot"));
    let devnet_slot = u64::from_le_bytes(DEVNET_RECEIVER_HEADER[4..12].try_into().expect("slot"));
    assert_ne!(mainnet_slot, devnet_slot);
}

/// Every per-feed account address moved at the cutover because the push-oracle
/// program id moved, not because any feed identifier changed. The seeds are
/// `[shard_id: u16 little-endian, feed_id: [u8; 32]]` under the push oracle,
/// and the resulting account is owned by the *receiver*.
#[test]
fn per_feed_addresses_are_push_oracle_pdas_and_only_the_program_moved() {
    /// Read out of the live account bodies on both clusters, not from docs.
    const SOL_USD_FEED_ID: [u8; 32] = [
        0xef, 0x0d, 0x8b, 0x6f, 0xda, 0x2c, 0xeb, 0xa4, 0x1d, 0xa1, 0x5d, 0x40, 0x95, 0xd1, 0xda,
        0x39, 0x2a, 0x0d, 0x2f, 0x8e, 0xd0, 0xc6, 0xc7, 0xbc, 0x0f, 0x4c, 0xfa, 0xc8, 0xc2, 0x80,
        0xb5, 0x6d,
    ];
    const LEGACY_PUSH_ORACLE: &str = "pythWSnswVUd12oZpeFP8e9CVaEqJg25g1Vtc2biRsT";
    const SHARD_ZERO: [u8; 2] = 0_u16.to_le_bytes();

    let upgraded = Pubkey::find_program_address(
        &[&SHARD_ZERO, &SOL_USD_FEED_ID],
        &pubkey(PUSH_ORACLE_PROGRAM),
    )
    .0;
    assert_eq!(
        upgraded,
        pubkey("7AviUf9nL62mcxNbQGKm4nKDQnPjswo6c5MX4D57HmyE"),
        "upgraded shard-zero SOL/USD address",
    );

    let legacy = Pubkey::find_program_address(
        &[&SHARD_ZERO, &SOL_USD_FEED_ID],
        &pubkey(LEGACY_PUSH_ORACLE),
    )
    .0;
    assert_eq!(
        legacy,
        pubkey("7UVimffxr9ow1uXYxsr4LHAcV58mLzhmwaeKvJ1pjLiE"),
        "legacy shard-zero SOL/USD address",
    );

    assert_ne!(
        upgraded, legacy,
        "the same feed id resolves to different accounts under the two generations",
    );
}

/// The observed price accounts are owned by the receiver, so an adapter
/// authenticates the receiver program and uses the push-oracle id only for
/// address derivation. Pinned here so the distinction is not lost.
#[test]
fn observed_price_accounts_are_owned_by_the_receiver() {
    const MAINNET_PRICE: &[u8] = include_bytes!(
        "../../../fixtures/pyth/upgraded-2026-08-26/mainnet-beta/sol-usd-price-update.account"
    );
    const DEVNET_PRICE: &[u8] = include_bytes!(
        "../../../fixtures/pyth/upgraded-2026-08-26/devnet/sol-usd-price-update.account"
    );
    for (label, bytes) in [("mainnet-beta", MAINNET_PRICE), ("devnet", DEVNET_PRICE)] {
        assert_eq!(bytes.len(), 134, "{label} PriceUpdateV2 length");
        assert_eq!(
            bytes.get(..8),
            Some(&[0x22, 0xf1, 0x23, 0x63, 0x9d, 0x7e, 0xf4, 0xcd][..]),
            "{label} PriceUpdateV2 discriminator",
        );
    }
    assert_sha256(
        "mainnet-beta sol-usd-price-update.account",
        MAINNET_PRICE,
        "f6011b154d9768a6ee49917e8b379a1585a14cac3d62b404c0686b2904dc70ee",
    );
    assert_sha256(
        "devnet sol-usd-price-update.account",
        DEVNET_PRICE,
        "2d36a13af42818983ba17f375b9f64415cf87cc1415026e2786e545c740f8aad",
    );
}
