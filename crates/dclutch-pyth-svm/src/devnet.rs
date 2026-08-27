//! The devnet production Pyth release row.
//!
//! This is the row `docs/design/DEVNET_DEMO_DEPLOY.md` §5.2 names as the
//! precondition for any devnet market resolved by the real receiver, and the
//! row `docs/evidence/DEVNET_SMOKE_0.md` W2 records as absent. Minted
//! 2026-08-27 under ember's SMOKE-0 deputization.
//!
//! Every value below is a measured fact with two independent confirmations:
//! pinned by `fixtures/pyth/upgraded-2026-08-26/PROVENANCE.md` (measured
//! facts 2, 3 and 5) on 2026-08-26, and re-read live off devnet on 2026-08-27
//! by `tools/release/devnet-observe.sh` twice (08:53Z, DA2 lane; 21:1xZ,
//! SMOKE-0 lane) with every byte reproducing.
//!
//! This module is the one author of the captured provider facts. The
//! synthetic-local fixture derives from this row, overriding only what the
//! lab changes (its cluster label, its lab-built receiver config, its
//! captured 19-guardian arrangement);
//! `tests::devnet_row_is_what_the_runbook_pinned` asserts the spellings and
//! the derivation tests in `synthetic_fixture` bound the overrides.

use crate::release::{PythReleaseV1, PythReleaseV1Input, PythReleaseV1Result};

/// Devnet cluster identity: the genesis hash
/// `EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG`, read from the cluster
/// (`getGenesisHash`), never inferred. A release is per cluster and this
/// field is what binds it.
pub const DEVNET_CLUSTER_ID_V1: [u8; 32] = [
    206, 89, 219, 80, 128, 252, 44, 109, 59, 207, 124, 169, 7, 18, 211, 194, 229, 230, 194, 143,
    39, 240, 223, 187, 153, 83, 189, 176, 137, 76, 3, 171,
];

/// Devnet receiver `Config` digest (370 bytes at
/// `H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye`).
///
/// Per cluster: the config differs from mainnet-beta's only in
/// `governance_authority` (`7g4Los4W…` on devnet), so the digest cannot be
/// shared across clusters (PROVENANCE.md measured fact 5).
pub const DEVNET_RECEIVER_CONFIG_DIGEST_V1: [u8; 32] = [
    0x23, 0xa7, 0xa1, 0x9c, 0xf6, 0x0c, 0x1f, 0xda, 0x8f, 0x07, 0x03, 0x23, 0xfb, 0x8f, 0x10, 0x13,
    0xa3, 0x28, 0x51, 0xb0, 0x92, 0x1f, 0xb7, 0xb2, 0xac, 0x08, 0x59, 0x90, 0xcb, 0xfa, 0xa3, 0x7a,
];

/// Devnet guardian-set cardinality: five 20-byte keys in `GuardianSet[0]`
/// with `expiration_time = 0` (PROVENANCE.md measured fact 5).
pub const DEVNET_GUARDIAN_SET_COUNT_V1: u8 = 5;

/// Strict majority of five. Equal to the receiver's own
/// `minimum_signatures = 3` under this generation — a coincidence of this
/// generation, not a rule (under the previous 19-key set they were 10 and 5).
pub const DEVNET_REQUIRED_GUARDIAN_COUNT_V1: u8 = 3;

/// Validate and construct the devnet production Pyth release row.
///
/// The trust root this row binds is disclosed by PROVENANCE.md: a 3-of-5
/// Pyth-controlled multisig, upgrade authority `upg8KLAL…` over all three
/// provider programs. "Zero new trust" for a Pyth-sourced devnet market is
/// exact; what that trust *is* belongs in the Product's disclosure.
pub fn devnet_release_v1() -> PythReleaseV1Result<PythReleaseV1> {
    PythReleaseV1::new(PythReleaseV1Input {
        cluster_id: DEVNET_CLUSTER_ID_V1,
        // rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp
        receiver_program: [
            12, 183, 250, 122, 93, 166, 40, 251, 172, 169, 154, 234, 153, 247, 191, 59, 220, 54,
            137, 104, 96, 42, 191, 65, 77, 78, 139, 165, 103, 187, 176, 191,
        ],
        // 3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX
        receiver_programdata: [
            36, 193, 217, 188, 83, 14, 128, 168, 96, 32, 44, 16, 172, 175, 215, 77, 119, 182, 74,
            169, 54, 67, 73, 241, 216, 23, 185, 252, 58, 36, 131, 42,
        ],
        // H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye
        receiver_config: [
            238, 89, 90, 195, 222, 6, 29, 79, 129, 224, 111, 41, 182, 154, 130, 148, 218, 115, 206,
            1, 195, 236, 196, 54, 206, 145, 180, 165, 98, 100, 91, 13,
        ],
        // HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL
        router_program: [
            241, 11, 10, 220, 120, 104, 244, 85, 102, 87, 169, 5, 247, 20, 69, 206, 236, 66, 7,
            172, 119, 215, 197, 194, 183, 98, 223, 19, 148, 102, 75, 135,
        ],
        // 9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x
        router_programdata: [
            129, 50, 201, 239, 143, 229, 66, 230, 102, 107, 79, 207, 240, 58, 197, 139, 124, 134,
            144, 55, 34, 39, 166, 84, 85, 21, 198, 154, 109, 140, 219, 31,
        ],
        config_digest: DEVNET_RECEIVER_CONFIG_DIGEST_V1,
        receiver_abi_id: [
            0xc5, 0x07, 0x95, 0x58, 0x64, 0xfc, 0x34, 0xdb, 0xd5, 0xfe, 0x87, 0xb4, 0xaa, 0x9f,
            0xba, 0x3a, 0x1e, 0xd2, 0x26, 0x90, 0x36, 0x3e, 0xc4, 0x90, 0x44, 0x9e, 0x86, 0x60,
            0xe7, 0x3a, 0xf6, 0x04,
        ],
        router_abi_id: [
            0xf9, 0x06, 0x1f, 0x03, 0xa8, 0x1b, 0x89, 0xdb, 0x29, 0xf4, 0x60, 0x36, 0x77, 0xe3,
            0xb3, 0xd8, 0x9b, 0x3b, 0xbf, 0x08, 0xd6, 0x78, 0x27, 0xb2, 0x83, 0x2f, 0x18, 0xa4,
            0xe2, 0xb6, 0x1a, 0xcb,
        ],
        price_update_codec_id: [
            0x12, 0xd0, 0xce, 0x8b, 0xc3, 0x90, 0x7a, 0xe2, 0x94, 0x90, 0x43, 0x39, 0x7e, 0xaf,
            0x3d, 0x5b, 0xd2, 0x5d, 0xee, 0xd9, 0x84, 0x50, 0xc6, 0x96, 0x9d, 0x95, 0x7b, 0xe4,
            0x02, 0xc8, 0x07, 0xae,
        ],
        adapter_id: [
            0x3f, 0xdf, 0xc9, 0x45, 0x89, 0xc6, 0x9b, 0x13, 0x38, 0x64, 0x46, 0x83, 0x20, 0x97,
            0x6f, 0x8e, 0x79, 0x0e, 0x7f, 0xe0, 0xf1, 0x45, 0x89, 0x7b, 0x6e, 0xab, 0xc2, 0x2b,
            0xd7, 0xc8, 0x71, 0x1b,
        ],
        receiver_deployment_slot: 460_336_311,
        router_deployment_slot: 460_336_290,
        guardian_set_count: DEVNET_GUARDIAN_SET_COUNT_V1,
        required_guardian_count: DEVNET_REQUIRED_GUARDIAN_COUNT_V1,
        upstream_commit: [
            0xf5, 0x0a, 0x3f, 0xaf, 0x9f, 0xc5, 0xa2, 0x23, 0xa2, 0x28, 0x89, 0x79, 0x9b, 0x2f,
            0x77, 0x89, 0x00, 0xf1, 0x86, 0xb3,
        ],
        sdk_crate_digest: [
            0x24, 0x5b, 0x1b, 0x03, 0xdd, 0x21, 0x77, 0x40, 0x20, 0x18, 0xb6, 0x07, 0x2f, 0xcb,
            0xb7, 0xbe, 0xa5, 0xb3, 0xd2, 0x80, 0x42, 0x7b, 0x19, 0x54, 0x79, 0x6b, 0xf1, 0xdc,
            0x18, 0x9b, 0xe4, 0x8b,
        ],
        activation_time: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Base58-decode, locally, so the pinned byte constants can be asserted
    /// against the human-readable spellings the runbook and provenance use.
    fn base58(value: &str) -> [u8; 32] {
        const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
        let mut number = [0_u8; 32];
        for &symbol in value.as_bytes() {
            let digit = u16::try_from(
                ALPHABET
                    .iter()
                    .position(|&candidate| candidate == symbol)
                    .expect("base58 symbol"),
            )
            .expect("base58 digit fits u16");
            let mut carry = digit;
            for byte in number.iter_mut().rev() {
                let widened = u16::from(*byte) * 58 + carry;
                *byte = (widened & 0xff) as u8;
                carry = widened >> 8;
            }
            assert_eq!(carry, 0, "value exceeds 32 bytes");
        }
        number
    }

    #[test]
    fn devnet_row_is_what_the_runbook_pinned() {
        let devnet = devnet_release_v1().expect("devnet release validates");
        assert_eq!(
            devnet.cluster_id(),
            base58("EtWTRABZaYq6iMfeYKouRu166VU2xqa1wcaWoxPkrZBG"),
        );
        assert_eq!(devnet.config_digest(), DEVNET_RECEIVER_CONFIG_DIGEST_V1);
        assert_eq!(devnet.guardian_set_count(), 5);
        assert_eq!(devnet.required_guardian_count(), 3);
        assert_eq!(
            devnet.receiver_program(),
            base58("rec2HHDDnjLfj4kE7VyEtFA1HPGQLK33259532cRyHp"),
        );
        assert_eq!(
            devnet.receiver_programdata(),
            base58("3UV7w2yTaqVcUAbWm1KUXdcE1Ziw8CfyyCpZvhKFkPfX"),
        );
        assert_eq!(
            devnet.receiver_config(),
            base58("H3R4M45f2gyqp6geVUruapzZdyxpgGZ96UnWkDM3ndye"),
        );
        assert_eq!(
            devnet.router_program(),
            base58("HDw2E7P8X1SkCyjvoGsfBGAVUutKcj874bXjHrpVYrVL"),
        );
        assert_eq!(
            devnet.router_programdata(),
            base58("9hLWdeVhSG9ufuQFA5d6zUoZ6qXoMRWrS8i4HGFHnR1x"),
        );
        assert_eq!(devnet.receiver_deployment_slot(), 460_336_311);
        assert_eq!(devnet.router_deployment_slot(), 460_336_290);
    }
}
