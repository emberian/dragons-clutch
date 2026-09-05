//! Sole semantic owner of the provenance-pinned synthetic-local Pyth release.
//!
//! This module is absent unless the explicit `synthetic-local-fixture` feature
//! is enabled. Its row is not a Solana-cluster claim and cannot inhabit the
//! deliberately empty production catalog.

use crate::pyth::devnet::devnet_release_v1;
use crate::pyth::{
    PythReleaseV1Input, SyntheticLocalReleaseV1, SyntheticLocalReleaseV1Error,
    SyntheticLocalReleaseV1Input,
};

/// Domain-separated local label recorded by the fixture evidence manifest.
pub const SYNTHETIC_LOCAL_LABEL_V1: [u8; 32] = [
    0x40, 0x81, 0xd5, 0x5d, 0x40, 0x31, 0x31, 0x3f, 0xcf, 0x4b, 0x7c, 0x41, 0x31, 0x3d, 0x54, 0x7a,
    0x94, 0x41, 0xc8, 0xf9, 0xc0, 0x48, 0x74, 0x1a, 0x7a, 0x95, 0x1b, 0x3e, 0x03, 0x5e, 0x22, 0xd9,
];

/// Domain-separated label for Loader V3 accounts prepared explicitly for the
/// pinned `solana-test-validator 4.0.2` profile.
pub const LOCAL_VALIDATOR_LABEL_V1: [u8; 32] = [
    94, 101, 206, 251, 15, 89, 160, 219, 77, 108, 238, 201, 111, 77, 195, 246, 90, 56, 244, 221,
    95, 110, 127, 30, 120, 77, 122, 29, 37, 8, 176, 167,
];

/// The lab-built receiver `Config` digest the synthetic capture pinned.
///
/// Not a cluster fact: the lab regenerates the receiver config under its own
/// governance key, so this digest exists only in the fixture evidence. The
/// devnet cluster's digest lives with the production row
/// (`crate::pyth::devnet::DEVNET_RECEIVER_CONFIG_DIGEST_V1`).
pub const SYNTHETIC_LOCAL_CONFIG_DIGEST_V1: [u8; 32] = [
    0x05, 0x03, 0x8c, 0xf7, 0x07, 0xaf, 0xce, 0xac, 0x3d, 0xf1, 0xaa, 0xe7, 0x35, 0xb0, 0x96, 0x34,
    0x4a, 0xd6, 0x39, 0x50, 0x6b, 0x00, 0xf1, 0xdb, 0x0a, 0xc1, 0xc0, 0x84, 0xd6, 0xb6, 0x45, 0xaa,
];

/// Reconstruct the one checked synthetic-local release from pinned evidence.
///
/// The shared provider facts — keys, ProgramData addresses, deployment
/// slots, ABI/codec/adapter identities, upstream commit, SDK digest — have
/// ONE author: the devnet production row (`crate::pyth::devnet::devnet_release_v1`),
/// which is the same captured generation. This fixture overrides exactly what
/// the lab changes: its domain-separated label as the cluster identity, the
/// lab-built receiver config digest, and the captured 19-guardian
/// arrangement (strict majority 10) that predates devnet's current 5-key
/// set. `tests::synthetic_row_derives_from_the_devnet_row` bounds the
/// overrides byte-for-byte.
pub fn synthetic_local_release_v1() -> Result<SyntheticLocalReleaseV1, SyntheticLocalReleaseV1Error>
{
    let devnet = devnet_release_v1().expect("the devnet production row validates");
    SyntheticLocalReleaseV1::new(SyntheticLocalReleaseV1Input {
        local_label: SYNTHETIC_LOCAL_LABEL_V1,
        release: PythReleaseV1Input {
            cluster_id: SYNTHETIC_LOCAL_LABEL_V1,
            receiver_program: devnet.receiver_program(),
            receiver_programdata: devnet.receiver_programdata(),
            receiver_config: devnet.receiver_config(),
            router_program: devnet.router_program(),
            router_programdata: devnet.router_programdata(),
            config_digest: SYNTHETIC_LOCAL_CONFIG_DIGEST_V1,
            receiver_abi_id: devnet.receiver_abi_id(),
            router_abi_id: devnet.router_abi_id(),
            price_update_codec_id: devnet.price_update_codec_id(),
            adapter_id: devnet.adapter_id(),
            receiver_deployment_slot: devnet.receiver_deployment_slot(),
            router_deployment_slot: devnet.router_deployment_slot(),
            guardian_set_count: 19,
            required_guardian_count: 10,
            upstream_commit: devnet.upstream_commit(),
            sdk_crate_digest: devnet.sdk_crate_digest(),
            activation_time: devnet.activation_time(),
        },
    })
}

/// Reconstruct the non-production release observed when the pinned real
/// provider ELFs are installed from exact immutable Loader account JSON by
/// `solana-test-validator 4.0.2`.
///
/// That loader profile prepares both ProgramData headers with deployment slot
/// zero and exact `None` authority encoding. Every provider, ABI, codec,
/// configuration, quorum, and source fact remains owned by the
/// provenance-pinned captured release above.
pub fn local_validator_release_v1() -> Result<SyntheticLocalReleaseV1, SyntheticLocalReleaseV1Error>
{
    let captured = synthetic_local_release_v1()?;
    let release = captured.release();
    SyntheticLocalReleaseV1::new(SyntheticLocalReleaseV1Input {
        local_label: LOCAL_VALIDATOR_LABEL_V1,
        release: PythReleaseV1Input {
            cluster_id: LOCAL_VALIDATOR_LABEL_V1,
            receiver_program: release.receiver_program(),
            receiver_programdata: release.receiver_programdata(),
            receiver_config: release.receiver_config(),
            router_program: release.router_program(),
            router_programdata: release.router_programdata(),
            config_digest: release.config_digest(),
            receiver_abi_id: release.receiver_abi_id(),
            router_abi_id: release.router_abi_id(),
            price_update_codec_id: release.price_update_codec_id(),
            adapter_id: release.adapter_id(),
            receiver_deployment_slot: 0,
            router_deployment_slot: 0,
            guardian_set_count: release.guardian_set_count(),
            required_guardian_count: release.required_guardian_count(),
            upstream_commit: release.upstream_commit(),
            sdk_crate_digest: release.sdk_crate_digest(),
            activation_time: release.activation_time(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pinned_row_is_a_valid_local_marker() {
        let local = synthetic_local_release_v1().expect("pinned synthetic release");
        assert_eq!(local.local_label(), SYNTHETIC_LOCAL_LABEL_V1);
        assert_eq!(local.release().cluster_id(), SYNTHETIC_LOCAL_LABEL_V1);
        assert_eq!(local.release().guardian_set_count(), 19);
        assert_eq!(local.release().required_guardian_count(), 10);
        assert_eq!(
            local.release().upstream_commit(),
            [
                0xf5, 0x0a, 0x3f, 0xaf, 0x9f, 0xc5, 0xa2, 0x23, 0xa2, 0x28, 0x89, 0x79, 0x9b, 0x2f,
                0x77, 0x89, 0x00, 0xf1, 0x86, 0xb3,
            ]
        );
    }

    /// The synthetic row against the devnet production row, byte for byte:
    /// exactly the cluster identity `[10..42]`, the config digest
    /// `[202..234]`, and the guardian counts `[378..380]` move. Any other
    /// difference means a shared provider fact grew a second author.
    #[test]
    fn synthetic_row_derives_from_the_devnet_row() {
        let devnet = devnet_release_v1()
            .expect("devnet production row validates")
            .to_bytes();
        let synthetic = synthetic_local_release_v1()
            .expect("pinned synthetic release")
            .release()
            .to_bytes();
        let mut masked_devnet = devnet;
        let mut masked_synthetic = synthetic;
        for bytes in [&mut masked_devnet, &mut masked_synthetic] {
            bytes[10..42].fill(0);
            bytes[202..234].fill(0);
            bytes[378..380].fill(0);
        }
        assert_eq!(masked_devnet, masked_synthetic);
        assert_ne!(devnet[10..42], synthetic[10..42]);
        assert_ne!(devnet[202..234], synthetic[202..234]);
        assert_ne!(devnet[378..380], synthetic[378..380]);
    }

    #[test]
    fn validator_row_changes_only_local_identity_and_regenerated_slots() {
        let captured = synthetic_local_release_v1().expect("pinned captured release");
        let validator = local_validator_release_v1().expect("pinned validator release");
        assert_ne!(captured.local_label(), validator.local_label());
        assert_eq!(validator.local_label(), LOCAL_VALIDATOR_LABEL_V1);
        assert_eq!(validator.release().cluster_id(), LOCAL_VALIDATOR_LABEL_V1);
        assert_eq!(validator.release().receiver_deployment_slot(), 0);
        assert_eq!(validator.release().router_deployment_slot(), 0);

        let mut captured_bytes = captured.release().to_bytes();
        let mut validator_bytes = validator.release().to_bytes();
        captured_bytes[10..42].fill(0);
        validator_bytes[10..42].fill(0);
        captured_bytes[362..378].fill(0);
        validator_bytes[362..378].fill(0);
        assert_eq!(captured_bytes, validator_bytes);
    }
}
