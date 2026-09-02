//! Canonical immutable identities for the implemented token profiles.
//!
//! These records commit SDK-free ABI interpretation and its official interface
//! provenance. They do not pin, inspect, or make a claim about any deployed
//! token-program binary. An SVM adapter separately authenticates the executable
//! program account and names any deployment trust policy.

use crate::{
    Error, ExactTransferProfileV1, LEGACY_TOKEN_PROGRAM_ID, Result, TOKEN_2022_PROGRAM_ID,
    instruction::{CLOSE_ACCOUNT_TAG, INITIALIZE_ACCOUNT3_TAG, TRANSFER_CHECKED_TAG},
};

/// Exact canonical release-preimage width.
pub const ADAPTER_RELEASE_BYTES: usize = 216;
/// Canonical release-preimage magic.
pub const ADAPTER_RELEASE_MAGIC: [u8; 8] = *b"DCLTARL1";
/// Implemented release-preimage schema.
pub const ADAPTER_RELEASE_SCHEMA_VERSION: u16 = 1;

const LEGACY_ARCHIVE_SHA256: [u8; 32] = [
    113, 67, 196, 230, 118, 152, 64, 71, 164, 0, 226, 251, 215, 172, 41, 161, 101, 159, 43, 27, 82,
    184, 151, 207, 222, 25, 49, 107, 86, 46, 69, 137,
];
// SHA-256("crates.io:spl-token-interface@3.0.0").
const LEGACY_INTERFACE_RELEASE_ID: [u8; 32] = [
    30, 103, 144, 32, 42, 219, 129, 88, 118, 5, 81, 172, 252, 25, 201, 163, 241, 120, 37, 242, 9,
    222, 100, 126, 130, 160, 251, 60, 67, 85, 123, 202,
];
const LEGACY_LIB_SHA256: [u8; 32] = [
    137, 123, 86, 82, 200, 38, 29, 162, 153, 167, 181, 18, 253, 57, 116, 73, 22, 218, 132, 169,
    252, 147, 118, 80, 172, 108, 24, 83, 170, 72, 46, 228,
];
const LEGACY_STATE_SHA256: [u8; 32] = [
    123, 11, 123, 172, 249, 97, 14, 12, 183, 182, 24, 43, 112, 185, 170, 75, 3, 106, 34, 124, 145,
    109, 122, 22, 110, 147, 174, 102, 107, 91, 140, 245,
];
const LEGACY_INSTRUCTION_SHA256: [u8; 32] = [
    79, 38, 236, 172, 9, 160, 79, 211, 7, 173, 181, 32, 36, 6, 25, 150, 255, 243, 182, 96, 193, 38,
    147, 187, 5, 3, 19, 238, 222, 237, 47, 204,
];
const TOKEN_2022_ARCHIVE_SHA256: [u8; 32] = [
    130, 29, 150, 208, 52, 234, 49, 196, 150, 93, 24, 44, 116, 33, 83, 196, 145, 174, 10, 190, 229,
    49, 51, 27, 85, 119, 16, 134, 197, 3, 13, 134,
];
// SHA-256("crates.io:spl-token-2022-interface@3.1.1").
const TOKEN_2022_INTERFACE_RELEASE_ID: [u8; 32] = [
    241, 43, 12, 169, 228, 138, 133, 44, 54, 208, 194, 1, 239, 123, 235, 128, 55, 147, 206, 61,
    144, 90, 211, 117, 70, 79, 212, 45, 133, 63, 165, 163,
];
const TOKEN_2022_LIB_SHA256: [u8; 32] = [
    91, 71, 212, 117, 250, 157, 169, 163, 170, 178, 233, 138, 223, 36, 212, 93, 230, 84, 202, 247,
    115, 217, 111, 60, 137, 251, 200, 90, 150, 234, 218, 119,
];
const TOKEN_2022_STATE_SHA256: [u8; 32] = [
    69, 102, 218, 248, 176, 111, 242, 231, 185, 117, 190, 211, 230, 49, 239, 74, 185, 89, 26, 149,
    221, 171, 212, 17, 115, 0, 202, 31, 99, 154, 89, 210,
];
const TOKEN_2022_INSTRUCTION_SHA256: [u8; 32] = [
    103, 43, 56, 144, 203, 88, 229, 112, 10, 103, 242, 2, 73, 16, 105, 237, 68, 223, 252, 191, 160,
    231, 208, 184, 58, 12, 251, 187, 5, 179, 162, 135,
];
const MINT_WIDTH_LE: [u8; 2] = [82, 0];
const ACCOUNT_WIDTH_LE: [u8; 2] = [165, 0];

/// Exact token profile kind committed by a release.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ProfileKind {
    /// Legacy SPL Token exact-transfer semantics.
    LegacyExactTransfer = 0,
    /// Token-2022 with exact base widths and no extension storage.
    Token2022ZeroExtensionExactTransfer = 1,
    /// Token-2022 admitting the ATA program's `ImmutableOwner` on participants.
    ///
    /// The Associated Token Account program writes `ImmutableOwner` into every
    /// account it creates under Token-2022 and no caller chooses otherwise, so
    /// a wallet's own associated account is 170 bytes and the zero-extension
    /// profile above refuses it. This kind admits exactly that account as a
    /// transfer participant and nothing else; protocol custody keeps the base
    /// width under every kind.
    Token2022ImmutableOwnerExactTransfer = 2,
}

/// Accepted state-extension storage policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ExtensionStoragePolicy {
    /// Mint and Account data must end at their exact base widths.
    ExactBaseWidthsOnly = 0,
    /// Mints end at their base width; participant Accounts may carry exactly
    /// the ATA program's empty `ImmutableOwner` entry and no other extension.
    ///
    /// APPEND-ONLY, for the same reason a refusal band is: this byte sits at
    /// offset 11 of the release preimage whose SHA-256 a realm record pins on
    /// chain as `collateral_adapter_release_id`, so a value that changes
    /// meaning makes the tree and the chain disagree about one identity.
    BaseWidthsOrImmutableOwnerAccounts = 1,
}

/// One complete immutable adapter-release preimage.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CollateralAdapterReleaseV1 {
    profile_kind: ProfileKind,
    token_program: [u8; 32],
    interface_release_id: [u8; 32],
    archive_sha256: [u8; 32],
    lib_sha256: [u8; 32],
    state_sha256: [u8; 32],
    instruction_sha256: [u8; 32],
}

impl CollateralAdapterReleaseV1 {
    /// Return exact Legacy Token V1 adapter semantics.
    pub const fn legacy_exact_transfer() -> Self {
        Self {
            profile_kind: ProfileKind::LegacyExactTransfer,
            token_program: LEGACY_TOKEN_PROGRAM_ID,
            interface_release_id: LEGACY_INTERFACE_RELEASE_ID,
            archive_sha256: LEGACY_ARCHIVE_SHA256,
            lib_sha256: LEGACY_LIB_SHA256,
            state_sha256: LEGACY_STATE_SHA256,
            instruction_sha256: LEGACY_INSTRUCTION_SHA256,
        }
    }

    /// Return exact Token-2022 zero-extension V1 adapter semantics.
    pub const fn token_2022_zero_extension_exact_transfer() -> Self {
        Self {
            profile_kind: ProfileKind::Token2022ZeroExtensionExactTransfer,
            token_program: TOKEN_2022_PROGRAM_ID,
            interface_release_id: TOKEN_2022_INTERFACE_RELEASE_ID,
            archive_sha256: TOKEN_2022_ARCHIVE_SHA256,
            lib_sha256: TOKEN_2022_LIB_SHA256,
            state_sha256: TOKEN_2022_STATE_SHA256,
            instruction_sha256: TOKEN_2022_INSTRUCTION_SHA256,
        }
    }

    /// Return exact Token-2022 `ImmutableOwner`-participant V1 semantics.
    ///
    /// The SAME Token-2022 interface provenance as
    /// [`Self::token_2022_zero_extension_exact_transfer`] -- the same archive,
    /// lib, state and instruction digests, the same interface release id and
    /// the same base widths. It differs from it in exactly two bytes of the
    /// preimage: the profile kind at offset 10 and the extension-storage
    /// policy at offset 11. Both existing entries are untouched, so a realm
    /// founded under either keeps matching the release it was founded under
    /// and keeps its own reading of what that identity means.
    pub const fn token_2022_immutable_owner_exact_transfer() -> Self {
        Self {
            profile_kind: ProfileKind::Token2022ImmutableOwnerExactTransfer,
            token_program: TOKEN_2022_PROGRAM_ID,
            interface_release_id: TOKEN_2022_INTERFACE_RELEASE_ID,
            archive_sha256: TOKEN_2022_ARCHIVE_SHA256,
            lib_sha256: TOKEN_2022_LIB_SHA256,
            state_sha256: TOKEN_2022_STATE_SHA256,
            instruction_sha256: TOKEN_2022_INSTRUCTION_SHA256,
        }
    }

    /// Decode only one of the exact production preimages.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != ADAPTER_RELEASE_BYTES {
            return Err(Error::InvalidLength);
        }
        for release in &PRODUCTION_ADAPTER_RELEASES {
            if bytes == release.to_bytes() {
                return Ok(*release);
            }
        }
        Err(Error::InvalidAdapterRelease)
    }

    /// Encode the exact canonical content-hash preimage.
    pub fn to_bytes(self) -> [u8; ADAPTER_RELEASE_BYTES] {
        let mut output = [0; ADAPTER_RELEASE_BYTES];
        put(&mut output, 0, &ADAPTER_RELEASE_MAGIC);
        put(
            &mut output,
            8,
            &ADAPTER_RELEASE_SCHEMA_VERSION.to_le_bytes(),
        );
        put(&mut output, 10, &[self.profile_kind.byte()]);
        put(&mut output, 11, &[self.extension_storage_policy().byte()]);
        put(&mut output, 16, &self.token_program);
        put(&mut output, 48, &MINT_WIDTH_LE);
        put(&mut output, 50, &ACCOUNT_WIDTH_LE);
        put(
            &mut output,
            52,
            &[
                CLOSE_ACCOUNT_TAG,
                TRANSFER_CHECKED_TAG,
                INITIALIZE_ACCOUNT3_TAG,
            ],
        );
        put(&mut output, 56, &self.archive_sha256);
        put(&mut output, 88, &self.lib_sha256);
        put(&mut output, 120, &self.state_sha256);
        put(&mut output, 152, &self.instruction_sha256);
        put(&mut output, 184, &self.interface_release_id);
        output
    }

    /// Return the selected executable byte-state semantics.
    pub const fn profile(self) -> ExactTransferProfileV1 {
        match self.profile_kind {
            ProfileKind::LegacyExactTransfer => ExactTransferProfileV1::LegacyExactTransferV1,
            ProfileKind::Token2022ZeroExtensionExactTransfer => {
                ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1
            }
            ProfileKind::Token2022ImmutableOwnerExactTransfer => {
                ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1
            }
        }
    }

    /// Return the extension-storage policy this release's kind commits.
    ///
    /// Derived rather than stored: the wire carries the kind at offset 10 and
    /// the policy at offset 11, and a struct holding both could contradict
    /// itself in a way no decode would catch, because `decode` matches whole
    /// preimages from the catalog.
    pub const fn extension_storage_policy(self) -> ExtensionStoragePolicy {
        match self.profile_kind {
            ProfileKind::LegacyExactTransfer | ProfileKind::Token2022ZeroExtensionExactTransfer => {
                ExtensionStoragePolicy::ExactBaseWidthsOnly
            }
            ProfileKind::Token2022ImmutableOwnerExactTransfer => {
                ExtensionStoragePolicy::BaseWidthsOrImmutableOwnerAccounts
            }
        }
    }

    /// Return the exact token-program address committed by this release.
    pub const fn token_program(self) -> [u8; 32] {
        self.token_program
    }
}

impl ProfileKind {
    const fn byte(self) -> u8 {
        match self {
            Self::LegacyExactTransfer => 0,
            Self::Token2022ZeroExtensionExactTransfer => 1,
            Self::Token2022ImmutableOwnerExactTransfer => 2,
        }
    }
}

impl ExtensionStoragePolicy {
    const fn byte(self) -> u8 {
        match self {
            Self::ExactBaseWidthsOnly => 0,
            Self::BaseWidthsOrImmutableOwnerAccounts => 1,
        }
    }
}

/// Complete production release catalog for the implemented SDK-free profiles.
pub const PRODUCTION_ADAPTER_RELEASES: [CollateralAdapterReleaseV1; 3] = [
    CollateralAdapterReleaseV1::legacy_exact_transfer(),
    CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer(),
    CollateralAdapterReleaseV1::token_2022_immutable_owner_exact_transfer(),
];

fn put(output: &mut [u8], offset: usize, input: &[u8]) {
    for (destination, source) in output.iter_mut().skip(offset).zip(input) {
        *destination = *source;
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};

    use super::*;

    const EXPECTED_CONTENT_IDS: [[u8; 32]; 3] = [
        [
            149, 99, 149, 173, 113, 204, 32, 48, 181, 140, 253, 121, 0, 35, 60, 137, 174, 150, 255,
            4, 159, 35, 215, 219, 236, 195, 174, 143, 142, 13, 109, 63,
        ],
        [
            34, 140, 20, 249, 229, 1, 248, 97, 56, 211, 241, 158, 94, 168, 21, 175, 98, 140, 10,
            223, 73, 157, 198, 169, 61, 216, 203, 24, 92, 135, 14, 41,
        ],
        // `430369ce72f5e1dcfa19dcee63d5e15f9fbf2d6c9950c5caab53d5c028ae0a2d`
        // -- the third release, which cohort-14 founds its realm under.
        [
            67, 3, 105, 206, 114, 245, 225, 220, 250, 25, 220, 238, 99, 213, 225, 95, 159, 191, 45,
            108, 153, 80, 197, 202, 171, 83, 213, 192, 40, 174, 10, 45,
        ],
    ];

    #[test]
    fn exact_release_round_trips_and_digest_vectors_are_stable() {
        assert_eq!(
            usize::from(u16::from_le_bytes(MINT_WIDTH_LE)),
            crate::MINT_BYTES
        );
        assert_eq!(
            usize::from(u16::from_le_bytes(ACCOUNT_WIDTH_LE)),
            crate::ACCOUNT_BYTES
        );
        for (release, expected_digest) in
            PRODUCTION_ADAPTER_RELEASES.iter().zip(EXPECTED_CONTENT_IDS)
        {
            let bytes = release.to_bytes();
            assert_eq!(CollateralAdapterReleaseV1::decode(&bytes), Ok(*release));
            let actual: [u8; 32] = Sha256::digest(bytes).into();
            assert_eq!(actual, expected_digest);
        }
    }

    #[test]
    fn hostile_lengths_and_every_byte_change_refuse() {
        for release in PRODUCTION_ADAPTER_RELEASES {
            let bytes = release.to_bytes();
            for length in 0..ADAPTER_RELEASE_BYTES {
                if let Some(short) = bytes.get(..length) {
                    assert_eq!(
                        CollateralAdapterReleaseV1::decode(short),
                        Err(Error::InvalidLength)
                    );
                }
            }
            for offset in 0..ADAPTER_RELEASE_BYTES {
                let mut changed = bytes;
                if let Some(byte) = changed.get_mut(offset) {
                    *byte ^= 1;
                }
                assert_eq!(
                    CollateralAdapterReleaseV1::decode(&changed),
                    Err(Error::InvalidAdapterRelease)
                );
            }
        }
    }

    #[test]
    fn releases_select_only_their_exact_program_and_profile() {
        let legacy = PRODUCTION_ADAPTER_RELEASES
            .first()
            .copied()
            .expect("three releases");
        let token_2022 = PRODUCTION_ADAPTER_RELEASES
            .get(1)
            .copied()
            .expect("three releases");
        let immutable_owner = PRODUCTION_ADAPTER_RELEASES
            .get(2)
            .copied()
            .expect("three releases");
        assert_eq!(legacy.token_program(), LEGACY_TOKEN_PROGRAM_ID);
        assert_eq!(
            legacy.profile(),
            ExactTransferProfileV1::LegacyExactTransferV1
        );
        assert_eq!(token_2022.token_program(), TOKEN_2022_PROGRAM_ID);
        assert_eq!(
            token_2022.profile(),
            ExactTransferProfileV1::Token2022ZeroExtensionExactTransferV1
        );
        assert_eq!(immutable_owner.token_program(), TOKEN_2022_PROGRAM_ID);
        assert_eq!(
            immutable_owner.profile(),
            ExactTransferProfileV1::Token2022ImmutableOwnerTransferV1
        );
    }

    /// THE TWO BYTES, AND ONLY THE TWO BYTES.
    ///
    /// The third release is not a new adapter. It is the same Token-2022
    /// interface provenance under a different storage policy, and the whole
    /// content of "this is not an edit of the second release" is that both
    /// preimages exist, differ in exactly the kind byte and the policy byte,
    /// and hash to different identities. If this test ever finds a third
    /// differing offset, some field drifted between the two constructors and
    /// the claim in the design note is false.
    #[test]
    fn the_third_release_differs_from_the_second_in_exactly_the_kind_and_policy_bytes() {
        let second = CollateralAdapterReleaseV1::token_2022_zero_extension_exact_transfer();
        let third = CollateralAdapterReleaseV1::token_2022_immutable_owner_exact_transfer();
        let (left, right) = (second.to_bytes(), third.to_bytes());
        let differing: std::vec::Vec<usize> = (0..ADAPTER_RELEASE_BYTES)
            .filter(|offset| left.get(*offset) != right.get(*offset))
            .collect();
        assert_eq!(differing, std::vec![10, 11]);
        assert_eq!(
            left.get(10),
            Some(&ProfileKind::Token2022ZeroExtensionExactTransfer.byte())
        );
        assert_eq!(
            right.get(10),
            Some(&ProfileKind::Token2022ImmutableOwnerExactTransfer.byte())
        );
        assert_eq!(
            left.get(11),
            Some(&ExtensionStoragePolicy::ExactBaseWidthsOnly.byte())
        );
        assert_eq!(
            right.get(11),
            Some(&ExtensionStoragePolicy::BaseWidthsOrImmutableOwnerAccounts.byte())
        );
        assert_eq!(
            second.extension_storage_policy(),
            ExtensionStoragePolicy::ExactBaseWidthsOnly
        );
        assert_eq!(
            third.extension_storage_policy(),
            ExtensionStoragePolicy::BaseWidthsOrImmutableOwnerAccounts
        );
        assert_ne!(second, third);
        assert_eq!(second.token_program(), third.token_program());
    }

    /// THE TWO EXISTING IDENTITIES DID NOT MOVE.
    ///
    /// Cohort-13's realm stores `228c14f9...` on chain and its deployed Custody
    /// selects a profile by matching it. This asserts the first two catalog
    /// entries against digests written before the third release existed, so a
    /// later edit to either preimage -- which would strand every market founded
    /// under it -- fails here by name rather than on devnet.
    #[test]
    fn appending_a_release_moved_neither_released_identity() {
        const RELEASED_BEFORE_COHORT_14: [[u8; 32]; 2] = [
            [
                149, 99, 149, 173, 113, 204, 32, 48, 181, 140, 253, 121, 0, 35, 60, 137, 174, 150,
                255, 4, 159, 35, 215, 219, 236, 195, 174, 143, 142, 13, 109, 63,
            ],
            [
                34, 140, 20, 249, 229, 1, 248, 97, 56, 211, 241, 158, 94, 168, 21, 175, 98, 140,
                10, 223, 73, 157, 198, 169, 61, 216, 203, 24, 92, 135, 14, 41,
            ],
        ];
        for (index, expected) in RELEASED_BEFORE_COHORT_14.into_iter().enumerate() {
            let release = PRODUCTION_ADAPTER_RELEASES
                .get(index)
                .copied()
                .expect("a released catalog entry keeps its index");
            let actual: [u8; 32] = Sha256::digest(release.to_bytes()).into();
            assert_eq!(actual, expected, "released adapter identity {index} moved");
        }
    }
}
